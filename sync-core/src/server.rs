//! 一起听的房间服务器：HTTP 建房 + WebSocket 实时同步。
//! 单个进程可跑在自己电脑 / 云主机 / 局域网上，客户端连上就能一起听。

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::protocol::{ClientMsg, ServerMsg};
use crate::room::{gen_code, Member, Rooms};
use crate::now_ms;

#[derive(Clone)]
pub struct ServerState {
    pub rooms: Rooms,
}

#[derive(Deserialize)]
struct CreateReq {
    #[serde(default)]
    name: String,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Deserialize)]
struct WsQuery {
    #[serde(default)]
    name: String,
    #[serde(default)]
    avatar: String,
}

fn err_json(code: StatusCode, msg: &str) -> impl IntoResponse {
    (code, Json(json!({ "ok": false, "error": msg })))
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true, "service": "bili-music-sync", "time": now_ms() }))
}

async fn create_room(State(st): State<ServerState>, Json(req): Json<CreateReq>) -> impl IntoResponse {
    let name = if req.name.trim().is_empty() { "一起听".to_string() } else { req.name.clone() };
    // 允许客户端指定房间号（方便念给朋友），没指定就随机生成
    let snap = match req.code.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(code) => match st.rooms.create_with_code(code, &name) {
            Some(snap) => snap,
            None => return err_json(StatusCode::CONFLICT, "该房间号已被占用").into_response(),
        },
        None => st.rooms.create(&name),
    };
    Json(json!({ "ok": true, "room": snap })).into_response()
}

async fn room_info(State(st): State<ServerState>, Path(code): Path<String>) -> impl IntoResponse {
    match st.rooms.with_room(&code, |r| r.snapshot()) {
        Some(snap) => (StatusCode::OK, Json(json!({ "ok": true, "room": snap }))).into_response(),
        None => err_json(StatusCode::NOT_FOUND, "房间不存在").into_response(),
    }
}

async fn join_ws(
    State(st): State<ServerState>,
    Path(code): Path<String>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if !st.rooms.exists(&code) {
        return err_json(StatusCode::NOT_FOUND, "房间不存在，检查房间号").into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, st, code, q))
}

async fn handle_socket(socket: WebSocket, st: ServerState, code: String, q: WsQuery) {
    let self_id = gen_code() + "-" + &uuid::Uuid::new_v4().simple().to_string()[..6];
    let name = if q.name.trim().is_empty() { "听友".to_string() } else { q.name.clone() };

    // 注册成员，同时拿到广播订阅
    let sub = st.rooms.with_room(&code, |room| {
        let was_empty = room.members.is_empty();
        room.members.insert(
            self_id.clone(),
            Member {
                id: self_id.clone(),
                name: name.clone(),
                avatar: q.avatar.clone(),
                position_ms: 0,
                playing: false,
            },
        );
        if was_empty || room.host_id.is_empty() {
            room.host_id = self_id.clone();
        }
        room.last_active = now_ms();
        room.tx.subscribe()
    });

    let Some(mut rx) = sub else { return };

    let (mut sink, mut stream) = socket.split();

    // 先推一份完整快照给新来的
    if let Some(snap) = st.rooms.with_room(&code, |r| r.snapshot()) {
        let msg = ServerMsg::Welcome { self_id: self_id.clone(), room: snap };
        if let Ok(txt) = serde_json::to_string(&msg) {
            let _ = sink.send(Message::Text(txt.into())).await;
        }
    }
    // 再通知房间里的其他人：成员列表变了
    st.rooms.with_room(&code, |r| {
        r.broadcast_room();
    });

    // 转发广播给本连接
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    match serde_json::to_string(&msg) {
                        Ok(txt) => {
                            if sink.send(Message::Text(txt.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => continue,
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });

    // 处理本连接发来的消息
    let st2 = st.clone();
    let code2 = code.clone();
    let id2 = self_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(frame)) = stream.next().await {
            let text = match frame {
                Message::Text(t) => t.to_string(),
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => continue,
            };
            let Ok(msg) = serde_json::from_str::<ClientMsg>(&text) else { continue };
            let keep_going = apply(&st2, &code2, &id2, msg);
            if !keep_going {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // 清理：退出房间、移交房主、通知剩下的人
    st.rooms.with_room(&code, |room| {
        room.members.remove(&self_id);
        if room.host_id == self_id {
            room.host_id = room.members.keys().next().cloned().unwrap_or_default();
        }
        room.last_active = now_ms();
        room.broadcast_room();
    });
}

/// 处理一条客户端消息，返回 false 表示该断开连接
fn apply(st: &ServerState, code: &str, self_id: &str, msg: ClientMsg) -> bool {
    st.rooms.with_room(code, |room| crate::room::apply_client_msg(room, self_id, msg));
    true
}

/// 组装路由器（单独暴露，方便挂到已有服务里）
pub fn router(st: ServerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/{code}", get(room_info))
        .route("/ws/{code}", get(join_ws))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(st)
}

/// 启动服务器，阻塞直到进程结束
pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let st = ServerState { rooms: Rooms::default() };

    // 后台清理空房间
    let sweep_rooms = st.rooms.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            sweep_rooms.sweep();
        }
    });

    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("一起听服务器已启动: http://{addr}");
    axum::serve(listener, router(st)).await
}
