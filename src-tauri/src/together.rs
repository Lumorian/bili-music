//! 一起听：把 sync-core 的客户端接进 Tauri。
//! 下行事件通过 Tauri 事件 "together://event" 推给前端，
//! 前端只需要监听事件 + 调命令。

use std::sync::Arc;

use bili_music_sync::client::{self, SyncEvent, SyncHandle};
use bili_music_sync::{ClientMsg, RoomSnapshot, Track};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};

use crate::error::{AppError, AppResult};
use crate::session;

/// 全局连接状态（单房间：一个人同时在只有一个一起听房间）
#[derive(Default)]
pub struct Together {
    pub handle: Mutex<Option<SyncHandle>>,
    pub room: Mutex<Option<RoomSnapshot>>,
    pub self_id: Mutex<String>,
    pub server: Mutex<String>,
}

pub type SharedTogether = Arc<Together>;

fn emit(app: &AppHandle, payload: Value) {
    let _ = app.emit("together://event", payload);
}

fn to_json_event(ev: SyncEvent) -> Value {
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

/// 起一个后台任务，把 sync 事件转发到前端
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
                    *state.handle.lock().await = None;
                    *state.room.lock().await = None;
                }
                _ => {}
            }
            emit(&app, to_json_event(ev));
        }
    });
}

pub async fn create(server: &str, name: &str) -> AppResult<RoomSnapshot> {
    client::create_room(server, name).await.map_err(AppError::Message)
}

pub async fn join(
    app: AppHandle,
    state: SharedTogether,
    server: String,
    code: String,
    name: String,
    avatar: String,
) -> AppResult<Value> {
    // 已经有连接就先断掉
    {
        let mut h = state.handle.lock().await;
        *h = None;
    }
    let (handle, rx) = client::join(&server, &code, &name, &avatar)
        .await
        .map_err(AppError::Message)?;
    *state.handle.lock().await = Some(handle);
    *state.server.lock().await = server;
    spawn_forwarder(app, state, rx);
    Ok(json!({ "ok": true }))
}

pub async fn snapshot(state: &SharedTogether) -> AppResult<Value> {
    let room = state.room.lock().await.clone();
    let self_id = state.self_id.lock().await.clone();
    Ok(json!({ "selfId": self_id, "room": room }))
}

pub async fn send(state: &SharedTogether, msg: ClientMsg) -> AppResult<()> {
    let guard = state.handle.lock().await;
    let Some(h) = guard.as_ref() else {
        return Err(AppError::Message("尚未加入一起听房间".into()));
    };
    if h.send(msg) {
        Ok(())
    } else {
        Err(AppError::Message("连接已断开".into()))
    }
}

pub async fn leave(state: &SharedTogether) -> AppResult<()> {
    *state.handle.lock().await = None;
    *state.room.lock().await = None;
    *state.self_id.lock().await = String::new();
    Ok(())
}

/// 把当前队列塞给房间（房主建房后同步过去）
pub async fn push_queue(state: &SharedTogether, queue: Vec<Track>, index: usize) -> AppResult<()> {
    send(state, ClientMsg::SetQueue { queue, index }).await
}

pub fn default_server() -> String {
    let s = session::get();
    if s.together_server.trim().is_empty() {
        "http://127.0.0.1:8787".to_string()
    } else {
        s.together_server
    }
}
