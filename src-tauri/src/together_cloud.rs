//! 云房间在 Tauri 侧的接入层。
//!
//! 对上层（commands / 前端）暴露的接口和自建服务器模式完全一样：
//! 一个 `send(ClientMsg)` 加一条 `SyncEvent` 流。所以前端不需要知道
//! 当前用的是哪条链路，切换模式只是换一个 Transport。

use std::sync::Arc;

use bili_music_sync::client::SyncEvent;
use bili_music_sync::cloud::{self, Envelope};
use bili_music_sync::mqtt::{CloudConfig, MqttConn};
use bili_music_sync::protocol::ServerMsg;
use bili_music_sync::room::{Member, Room};
use bili_music_sync::{now_ms, ClientMsg};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::error::{AppError, AppResult};

/// 云房间的一个连接（房主和成员共用这个结构，只是内部行为不同）
pub struct CloudLink {
    conn: Arc<MqttConn>,
    topic: String,
    pub code: String,
    pub self_id: String,
    pub is_host: bool,
    /// 房主：把操作投给本地状态机；成员：直接把操作发到 broker
    to_host: Option<UnboundedSender<(String, ClientMsg)>>,
    /// 房主退出时用它停掉后台状态机，并通知房间里其他人
    close: Option<tokio::sync::watch::Sender<bool>>,
}

impl CloudLink {
    /// 主动离开：房主要广播"房主已离开"并停掉后台循环；
    /// 成员要告诉房主"我走了"，否则成员列表会留下幽灵条目。
    pub fn shutdown(&self) {
        match &self.close {
            Some(tx) => {
                announce_host_left(&self.conn, &self.topic);
                let _ = tx.send(true);
            }
            None => {
                self.conn.publish(&self.topic, &cloud::leave_envelope(&self.self_id));
            }
        }
    }
}

impl CloudLink {
    /// 房间号
    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn send(&self, msg: ClientMsg) -> bool {
        if let Some(tx) = &self.to_host {
            // 房主自己点的按钮，走本地状态机，不绕一圈公共网络
            return tx.send((self.self_id.clone(), msg)).is_ok();
        }
        self.conn
            .publish(&self.topic, &cloud::client_envelope(&self.self_id, msg))
    }
}

/// 建房并成为房主。
/// 返回：连接句柄 + 事件流（把房主的 ServerMsg 翻译成 SyncEvent）
pub async fn host(
    broker: &str,
    topic_prefix: &str,
    name: &str,
) -> AppResult<(CloudLink, UnboundedReceiver<SyncEvent>)> {
    let (conn, code, inbox) = cloud::host(broker, topic_prefix).await.map_err(|e| AppError::Message(e.to_string()))?;
    let topic = cloud::room_topic(topic_prefix, &code);

    let self_id = format!("h{}", now_ms() % 100_000_000);
    let mut room = Room::new(code.clone(), format!("{name} 的房间"));
    room.host_id = self_id.clone();
    room.members.insert(
        self_id.clone(),
        Member {
            id: self_id.clone(),
            name: name.to_string(),
            avatar: String::new(),
            position_ms: 0,
            playing: false,
        },
    );

    let conn = Arc::new(conn);
    let (close_tx, close_rx) = tokio::sync::watch::channel(false);
    let host = cloud::spawn_host_loop_controlled(room, conn.clone(), topic.clone(), inbox, close_rx);

    let (ev_tx, ev_rx) = unbounded_channel::<SyncEvent>();
    // 一上来先给前端一份快照，UI 才能立刻显示房间号与成员
    let snap_room = bili_music_sync::protocol::RoomSnapshot {
        code: code.clone(),
        name: format!("{name} 的房间"),
        members: Vec::new(),
        queue: Vec::new(),
        state: Default::default(),
        host_id: self_id.clone(),
        server_time: now_ms(),
    };
    let _ = ev_tx.send(SyncEvent::Connected { self_id: self_id.clone(), room: snap_room });

    let mut out_rx = host.out_rx;
    let ev_tx_task = ev_tx.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if let Some(ev) = server_msg_to_event(msg) {
                if ev_tx_task.send(ev).is_err() {
                    break;
                }
            }
        }
    });

    Ok((
        CloudLink {
            conn,
            topic,
            code,
            self_id,
            is_host: true,
            to_host: Some(host.cmd_tx),
            close: Some(close_tx),
        },
        ev_rx,
    ))
}

/// 加入别人的云房间
pub async fn join(
    broker: &str,
    topic_prefix: &str,
    code: &str,
    name: &str,
    avatar: &str,
) -> AppResult<(CloudLink, UnboundedReceiver<SyncEvent>)> {
    let (conn, self_id, mut inbox) =
        cloud::join(broker, topic_prefix, code, name, avatar)
            .await
            .map_err(|e| AppError::Message(e.to_string()))?;
    let topic = cloud::room_topic(topic_prefix, code);
    let conn = Arc::new(conn);
    let (ev_tx, ev_rx) = unbounded_channel::<SyncEvent>();

    let ev_tx_task = ev_tx.clone();
    let self_id_task = self_id.clone();
    let conn_task = conn.clone();
    let topic_task = topic.clone();
    let name_task = name.to_string();
    let avatar_task = avatar.to_string();
    tauri::async_runtime::spawn(async move {
        // 公共 broker 是 QoS 0，Hello 有可能丢。收不到 Welcome 就每隔 3 秒重发一次，
        // 累计 15 秒还没回应才判定「房间号不对 / 房主不在」。
        let mut welcomed = false;
        let mut attempts = 0u32;
        loop {
            let next = if welcomed {
                inbox.recv().await
            } else {
                let wait = tokio::time::Duration::from_secs(3);
                match tokio::time::timeout(wait, inbox.recv()).await {
                    Ok(v) => v,
                    Err(_) => {
                        attempts += 1;
                        if attempts >= 5 {
                            let _ = ev_tx_task.send(SyncEvent::Disconnected {
                                reason: "没有收到房主的回应，检查房间号，或房主已离开".into(),
                            });
                            break;
                        }
                        // 重发打招呼
                        conn_task.publish(
                            &topic_task,
                            &cloud::encode(&Envelope::Hello {
                                from: self_id_task.clone(),
                                name: name_task.clone(),
                                avatar: avatar_task.clone(),
                            }),
                        );
                        continue;
                    }
                }
            };
            let Some(bytes) = next else { break };
            let Some(env) = cloud::decode(&bytes) else { continue };
            match env {
                Envelope::Direct { to, msg } if to == self_id_task => {
                    if let ServerMsg::Welcome { room, .. } = &msg {
                        welcomed = true;
                        let _ = ev_tx_task.send(SyncEvent::Connected {
                            self_id: self_id_task.clone(),
                            room: room.clone(),
                        });
                        continue;
                    }
                    if let Some(ev) = server_msg_to_event(msg) {
                        let _ = ev_tx_task.send(ev);
                    }
                }
                Envelope::Server { msg } => {
                    // 广播里不会有 Kicked（房主改成 Direct 单发了），
                    // 万一收到也忽略，别把全房间踢下线。
                    if matches!(msg, ServerMsg::Kicked { .. }) {
                        continue;
                    }
                    if let Some(ev) = server_msg_to_event(msg) {
                        if ev_tx_task.send(ev).is_err() {
                            break;
                        }
                    }
                }
                Envelope::Host { host } if host.is_empty() => {
                    let _ = ev_tx_task.send(SyncEvent::Disconnected {
                        reason: "房主已离开房间".into(),
                    });
                    break;
                }
                _ => {}
            }
        }
    });

    Ok((
        CloudLink {
            conn,
            topic,
            code: code.to_string(),
            self_id,
            is_host: false,
            to_host: None,
            close: None,
        },
        ev_rx,
    ))
}

/// ServerMsg → 前端事件。和自建模式共用同一套事件类型，前端无需区分。
fn server_msg_to_event(msg: ServerMsg) -> Option<SyncEvent> {
    match msg {
        ServerMsg::Welcome { self_id, room } => Some(SyncEvent::Connected { self_id, room }),
        ServerMsg::Room(room) => Some(SyncEvent::Room(room)),
        ServerMsg::Members { members } => Some(SyncEvent::Members(members)),
        ServerMsg::State { state, server_time } => Some(SyncEvent::State {
            index: state.index,
            position_ms: state.position_ms,
            playing: state.playing,
            server_time,
            by: state.by,
        }),
        ServerMsg::Queue { queue, index } => Some(SyncEvent::Queue { queue, index }),
        ServerMsg::Chat { from, name, text, at } => Some(SyncEvent::Chat { from, name, text, at }),
        // 云房间下 Kicked 只以 Direct 形式发给目标成员，
        // join 循环已按 to == self 过滤，这里不会再误伤别人。
        ServerMsg::Kicked { reason, .. } => Some(SyncEvent::Disconnected { reason }),
        ServerMsg::Error { message } => Some(SyncEvent::Error { message }),
        ServerMsg::Pong { .. } => None,
    }
}

/// 房主离开时主动广播一次，让其他人立刻知道房间没了
pub fn announce_host_left(conn: &MqttConn, topic: &str) {
    conn.publish(topic, &cloud::host_left_envelope());
}

/// 默认公共 broker 配置
pub fn default_config() -> CloudConfig {
    CloudConfig::default()
}
