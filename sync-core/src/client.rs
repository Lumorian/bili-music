//! 一起听客户端：连接服务器、收发协议消息。
//! 应用侧拿到 `SyncHandle` 后，所有操作都是非阻塞的，
//! 服务器下行消息通过一个 mpsc 频道回调给 UI。

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ClientMsg, RoomSnapshot, ServerMsg};

#[derive(Debug, Clone)]
pub enum SyncEvent {
    Connected { self_id: String, room: RoomSnapshot },
    Room(RoomSnapshot),
    State { index: usize, position_ms: u64, playing: bool, server_time: u64, by: String },
    Queue { queue: Vec<crate::model::Track>, index: usize },
    Chat { from: String, name: String, text: String, at: u64 },
    Disconnected { reason: String },
    Error { message: String },
}

pub struct SyncHandle {
    tx: UnboundedSender<ClientMsg>,
}

impl SyncHandle {
    pub fn send(&self, msg: ClientMsg) -> bool {
        self.tx.send(msg).is_ok()
    }

    pub fn ping(&self) {
        let _ = self.tx.send(ClientMsg::Ping { t: crate::now_ms() });
    }

    pub fn request_sync(&self) {
        let _ = self.tx.send(ClientMsg::Sync);
    }

    pub fn chat(&self, text: &str) {
        let _ = self.tx.send(ClientMsg::Chat { text: text.to_string() });
    }
}

fn ws_url(server: &str, code: &str, name: &str, avatar: &str) -> String {
    let base = server.trim_end_matches('/');
    let base = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if base.starts_with("ws://") || base.starts_with("wss://") {
        base.to_string()
    } else {
        format!("ws://{base}")
    };
    format!(
        "{base}/ws/{code}?name={}&avatar={}",
        urlencoding::encode(name),
        urlencoding::encode(avatar)
    )
}

/// 在服务器上建房，返回房间快照
pub async fn create_room(server: &str, name: &str) -> Result<RoomSnapshot, String> {
    let base = server.trim_end_matches('/');
    let url = format!("{base}/api/rooms");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&json!({ "name": name }))
        .send()
        .await
        .map_err(|e| format!("连接服务器失败: {e}"))?;
    let v: serde_json::Value = resp.json().await.map_err(|e| format!("服务器返回异常: {e}"))?;
    if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        return Err(v
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("建房失败")
            .to_string());
    }
    serde_json::from_value(v["room"].clone()).map_err(|e| format!("房间数据解析失败: {e}"))
}

pub async fn fetch_room(server: &str, code: &str) -> Result<RoomSnapshot, String> {
    let base = server.trim_end_matches('/');
    let url = format!("{base}/api/rooms/{code}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("连接服务器失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("服务器返回异常: {e}"))?;
    if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        return Err("房间不存在".to_string());
    }
    serde_json::from_value(v["room"].clone()).map_err(|e| e.to_string())
}

/// 加入房间，返回 (句柄, 事件流)
pub async fn join(
    server: &str,
    code: &str,
    name: &str,
    avatar: &str,
) -> Result<(SyncHandle, UnboundedReceiver<SyncEvent>), String> {
    let url = ws_url(server, code, name, avatar);
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|e| format!("连接房间失败: {e}"))?;
    let (mut sink, mut stream) = ws.split();
    let (in_tx, mut in_rx) = unbounded_channel::<ClientMsg>();
    let (out_tx, out_rx) = unbounded_channel::<SyncEvent>();

    // 写方向：把本地操作变成 JSON 帧
    let tx_out = out_tx.clone();
    let hello = serde_json::to_string(&ClientMsg::Hello {
        name: name.to_string(),
        avatar: avatar.to_string(),
    })
    .unwrap_or_default();
    sink.send(Message::Text(hello.into())).await.map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        while let Some(msg) = in_rx.recv().await {
            let Ok(txt) = serde_json::to_string(&msg) else { continue };
            if sink.send(Message::Text(txt.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
        let _ = tx_out.send(SyncEvent::Disconnected { reason: "连接已关闭".into() });
    });

    // 读方向：把服务器帧翻译成事件
    let handle_tx = in_tx.clone();
    tokio::spawn(async move {
        while let Some(frame) = stream.next().await {
            let Ok(frame) = frame else { break };
            let text = match frame {
                Message::Text(t) => t.to_string(),
                Message::Close(_) => break,
                _ => continue,
            };
            let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) else { continue };
            let ev = match msg {
                ServerMsg::Welcome { self_id, room } => SyncEvent::Connected { self_id, room },
                ServerMsg::Room(room) => SyncEvent::Room(room),
                ServerMsg::State { state, server_time } => SyncEvent::State {
                    index: state.index,
                    position_ms: state.position_ms,
                    playing: state.playing,
                    server_time,
                    by: state.by,
                },
                ServerMsg::Queue { queue, index } => SyncEvent::Queue { queue, index },
                ServerMsg::Chat { from, name, text, at } => SyncEvent::Chat { from, name, text, at },
                ServerMsg::Error { message } => SyncEvent::Error { message },
                ServerMsg::Kicked { reason } => {
                    let _ = out_tx.send(SyncEvent::Disconnected { reason });
                    break;
                }
                ServerMsg::Pong { .. } => continue,
            };
            if out_tx.send(ev).is_err() {
                break;
            }
        }
        let _ = out_tx.send(SyncEvent::Disconnected { reason: "与房间失联".into() });
    });

    Ok((SyncHandle { tx: handle_tx }, out_rx))
}

/// 心跳任务：每 15 秒 ping 一次，顺便探测连接存活
pub fn spawn_heartbeat(handle: std::sync::Arc<Mutex<Option<SyncHandle>>>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        loop {
            tick.tick().await;
            if let Some(h) = handle.lock().await.as_ref() {
                h.ping();
            } else {
                break;
            }
        }
    });
}
