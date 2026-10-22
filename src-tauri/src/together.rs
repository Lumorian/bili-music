//! 一起听：把 sync-core 接进 Tauri。
//!
//! 支持两种传输，对上层完全透明：
//! - 自建服务器（Transport::Server）：连自己的房间服务器，状态由服务器维护；
//! - 云房间（Transport::Cloud）：连公共 MQTT broker，房主客户端代行服务器职责，
//!   不需要任何人部署东西。
//!
//! 前端只监听 together://event，调用同一批命令，不需要知道当前是哪种模式。

use std::sync::Arc;

use bili_music_sync::client::{self, SyncEvent, SyncHandle};
use bili_music_sync::{ClientMsg, RoomSnapshot};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};

use crate::error::{AppError, AppResult};
use crate::session;
use crate::together_cloud::{self, CloudLink};

/// 当前连接用的是哪条链路
pub enum Transport {
    /// 自建房间服务器
    Server(SyncHandle),
    /// 公共 broker + 房主代行
    Cloud(CloudLink),
}

/// 全局连接状态（一个人同时只在一个房间里）
#[derive(Default)]
pub struct Together {
    pub transport: Mutex<Option<Transport>>,
    pub room: Mutex<Option<RoomSnapshot>>,
    pub self_id: Mutex<String>,
    /// 自建模式下的服务器地址
    pub server: Mutex<String>,
}

pub type SharedTogether = Arc<Together>;

pub fn emit(app: &AppHandle, payload: Value) {
    let _ = app.emit("together://event", payload);
}

pub fn to_json_event(ev: SyncEvent) -> Value {
    match ev {
        SyncEvent::Connected { self_id, room } => json!({ "kind": "connected", "selfId": self_id, "room": room }),
        SyncEvent::Room(room) => json!({ "kind": "room", "room": room }),
        SyncEvent::State { index, position_ms, playing, server_time, by } => json!({
            "kind": "state",
            "index": index,
            "positionMs": position_ms,
            "playing": playing,
            "serverTime": server_time,
            "by": by,
        }),
        SyncEvent::Queue { queue, index } => json!({ "kind": "queue", "queue": queue, "index": index }),
        SyncEvent::Chat { from, name, text, at } => json!({
            "kind": "chat",
            "from": from,
            "name": name,
            "text": text,
            "at": at,
        }),
        SyncEvent::Disconnected { reason } => json!({ "kind": "disconnected", "reason": reason }),
        SyncEvent::Error { message } => json!({ "kind": "error", "message": message }),
    }
}

/// 后台任务：把 sync 事件转发到前端
pub fn spawn_forwarder(app: AppHandle, state: SharedTogether, mut rx: UnboundedReceiver<SyncEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match &ev {
                SyncEvent::Connected { self_id, room } => {
                    *state.self_id.lock().await = self_id.clone();
                    *state.room.lock().await = Some(room.clone());
                }
                SyncEvent::Room(room) => {
                    *state.room.lock().await = Some(room.clone());
                }
                SyncEvent::Disconnected { .. } => {
                    *state.transport.lock().await = None;
                    *state.room.lock().await = None;
                }
                _ => {}
            }
            emit(&app, to_json_event(ev));
        }
    });
}

// ---------- 自建服务器模式 ----------

pub async fn create_server_room(server: &str, name: &str) -> AppResult<RoomSnapshot> {
    client::create_room(server, name).await.map_err(AppError::Message)
}

pub async fn join_server(
    app: AppHandle,
    state: SharedTogether,
    server: String,
    code: String,
    name: String,
    avatar: String,
) -> AppResult<Value> {
    {
        let mut t = state.transport.lock().await;
        *t = None;
    }
    let (handle, rx) = client::join(&server, &code, &name, &avatar)
        .await
        .map_err(AppError::Message)?;
    *state.transport.lock().await = Some(Transport::Server(handle));
    *state.server.lock().await = server;
    spawn_forwarder(app, state, rx);
    Ok(json!({ "ok": true, "mode": "server" }))
}

// ---------- 云房间模式 ----------

/// 建云房间：不需要任何服务器，返回 6 位房间号
pub async fn host_cloud(
    app: AppHandle,
    state: SharedTogether,
    broker: Option<String>,
    topic_prefix: Option<String>,
    name: String,
) -> AppResult<Value> {
    {
        let mut t = state.transport.lock().await;
        *t = None;
    }
    let cfg = together_cloud::default_config();
    let broker_url = broker.unwrap_or(cfg.broker);
    let prefix = topic_prefix.unwrap_or(cfg.topic_prefix);

    let (link, rx) = together_cloud::host(&broker_url, &prefix, &name).await?;
    let code = link.code().to_string();
    *state.transport.lock().await = Some(Transport::Cloud(link));
    spawn_forwarder(app, state, rx);
    Ok(json!({ "code": code, "mode": "cloud" }))
}

pub async fn join_cloud(
    app: AppHandle,
    state: SharedTogether,
    broker: Option<String>,
    topic_prefix: Option<String>,
    code: String,
    name: String,
    avatar: String,
) -> AppResult<Value> {
    {
        let mut t = state.transport.lock().await;
        *t = None;
    }
    let cfg = together_cloud::default_config();
    let broker_url = broker.unwrap_or(cfg.broker);
    let prefix = topic_prefix.unwrap_or(cfg.topic_prefix);

    let (link, rx) = together_cloud::join(&broker_url, &prefix, &code, &name, &avatar).await?;
    *state.transport.lock().await = Some(Transport::Cloud(link));
    spawn_forwarder(app, state, rx);
    Ok(json!({ "ok": true, "mode": "cloud" }))
}

// ---------- 通用 ----------

pub async fn snapshot(state: &SharedTogether) -> AppResult<Value> {
    let room = state.room.lock().await.clone();
    let self_id = state.self_id.lock().await.clone();
    Ok(json!({ "selfId": self_id, "room": room }))
}

/// 发一条操作。自建模式发给服务器；云模式房主走本地状态机、成员发给房主。
pub async fn send(state: &SharedTogether, msg: ClientMsg) -> AppResult<()> {
    let guard = state.transport.lock().await;
    let Some(t) = guard.as_ref() else {
        return Err(AppError::Message("尚未加入一起听房间".into()));
    };
    let ok = match t {
        Transport::Server(h) => h.send(msg),
        Transport::Cloud(c) => c.send(msg),
    };
    if ok {
        Ok(())
    } else {
        Err(AppError::Message("连接已断开".into()))
    }
}

pub async fn leave(state: &SharedTogether) -> AppResult<()> {
    *state.transport.lock().await = None;
    *state.room.lock().await = None;
    *state.self_id.lock().await = String::new();
    Ok(())
}

pub async fn push_queue(
    state: &SharedTogether,
    queue: Vec<bili_music_sync::Track>,
    index: usize,
) -> AppResult<()> {
    send(state, ClientMsg::SetQueue { queue, index }).await
}

/// 当前是不是房主（云模式下只有房主能踢人 / 移交）
pub async fn am_host(state: &SharedTogether) -> bool {
    let guard = state.transport.lock().await;
    matches!(guard.as_ref(), Some(Transport::Cloud(c)) if c.is_host)
}

pub fn default_server() -> String {
    let s = session::get();
    if s.together_server.trim().is_empty() {
        "http://127.0.0.1:8787".to_string()
    } else {
        s.together_server
    }
}
