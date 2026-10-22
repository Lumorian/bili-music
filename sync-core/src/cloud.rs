//! 「云房间」：不需要自建服务器的一起听。
//!
//! 思路：把**房主的客户端**当成服务器。
//! 所有人连同一个公共 MQTT broker（默认 EMQX 的免费公共节点），
//! 订阅同一个按房间号命名的主题。房主收到别人的操作后，用和自建服务器
//! **完全同一份**状态机（`room::apply_client_msg`）算出新状态，再广播出去，
//! 所以两种模式的同步语义完全一致。
//!
//! 代价（写在明面上）：
//! - 房主是单点，房主退出房间就散了（会通知其他人）；
//! - 公共 broker 上是明文话题，猜到房间号的人都能监听，别在房间里聊隐私；
//! - 依赖第三方服务的可用性。想稳定就自建，客户端两条路随时可切。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::error::{CloudError, CloudResult};
use crate::mqtt::MqttConn;
use crate::now_ms;
use crate::protocol::{ClientMsg, ServerMsg};
use crate::room::{Member, Room};

/// 房间主题：所有消息都发到这里
pub fn room_topic(prefix: &str, code: &str) -> String {
    format!("{prefix}/room/{code}")
}

/// 6 位数字房间号。云模式没有服务器帮忙去重，用纳秒 + pid 降低碰撞概率。
pub fn gen_cloud_code() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let n = (nanos / 7 + (std::process::id() as u64) * 31) % 900_000 + 100_000;
    format!("{n}")
}

/// 信封：把「谁发的」和「发的什么」包在一起。
/// 公共 broker 上谁都能发，所以必须带 from，房主才知道代谁执行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum Envelope {
    /// 客户端 → 房主：一条操作
    #[serde(rename = "c")]
    Client { from: String, msg: ClientMsg },
    /// 进房打招呼；房主据此登记成员并回 Welcome
    #[serde(rename = "h")]
    Hello {
        from: String,
        name: String,
        #[serde(default)]
        avatar: String,
    },
    /// 房主 → 某个成员：只发给指定的人（Welcome / Kicked）
    #[serde(rename = "t")]
    Direct {
        to: String,
        msg: ServerMsg,
    },
    /// 房主 → 所有人：状态广播
    #[serde(rename = "s")]
    Server { msg: ServerMsg },
    /// 房主在线宣告；主动离开时发一次 `host: ""`
    #[serde(rename = "o")]
    Host { host: String },
}

pub fn encode(env: &Envelope) -> Vec<u8> {
    serde_json::to_vec(env).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<Envelope> {
    serde_json::from_slice(bytes).ok()
}

/// 建房：成为房主。返回 (连接, 房间号, 下行流)
pub async fn host(
    broker: &str,
    topic_prefix: &str,
) -> CloudResult<(MqttConn, String, UnboundedReceiver<Vec<u8>>)> {
    let mut last = CloudError::Message("建房失败".into());
    for _ in 0..4 {
        let code = gen_cloud_code();
        let topic = room_topic(topic_prefix, &code);
        let client_id = format!("bm-host-{code}-{}", now_ms());
        match MqttConn::connect(broker, &client_id, &topic).await {
            Ok((conn, rx)) => return Ok((conn, code, rx)),
            Err(e) => {
                last = e;
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
        }
    }
    Err(last)
}

/// 进房：成为普通成员。返回 (连接, 自己的 id, 下行流)
pub async fn join(
    broker: &str,
    topic_prefix: &str,
    code: &str,
    name: &str,
    avatar: &str,
) -> CloudResult<(MqttConn, String, UnboundedReceiver<Vec<u8>>)> {
    let topic = room_topic(topic_prefix, code);
    let self_id = format!("m{}", now_ms() % 100_000_000);
    let client_id = format!("bm-{code}-{self_id}");
    let (conn, rx) = MqttConn::connect(broker, &client_id, &topic).await?;
    // 打招呼，房主会用 Direct(Welcome) 回应
    conn.publish(
        &topic,
        &encode(&Envelope::Hello {
            from: self_id.clone(),
            name: name.to_string(),
            avatar: avatar.to_string(),
        }),
    );
    Ok((conn, self_id, rx))
}

/// 开跑房主侧的状态机。
///
/// - `inbox`：broker 下行流量（别人的 Hello / Client 信封）
/// - `local`：本地 UI 产生的操作（房主自己点播放、切歌等）
/// - 返回值：`cmd_tx` 给本地 UI 投递操作，`out_rx` 把需要应用到 UI 的 ServerMsg 吐出来
pub struct HostLoop {
    pub cmd_tx: UnboundedSender<(String, ClientMsg)>,
    pub out_rx: UnboundedReceiver<ServerMsg>,
    pub host_id: String,
}

/// 启动房主循环。房主自己也是成员，`host_id` 由调用方给定。
pub fn spawn_host_loop(
    room: Room,
    conn: Arc<MqttConn>,
    topic: String,
    inbox: UnboundedReceiver<Vec<u8>>,
) -> HostLoop {
    let host_id = room.host_id.clone();
    let mut room = room;
    let mut room_rx = room.tx.subscribe();
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<(String, ClientMsg)>();
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<ServerMsg>();
    let mut inbox = inbox;

    let out_for_task = out_tx.clone();
    let conn_task = conn.clone();
    let topic_task = topic.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                // ① 本地操作：房主自己点的按钮
                Some((from, msg)) = cmd_rx.recv() => {
                    crate::room::apply_client_msg(&mut room, &from, msg);
                }

                // ② 远端流量：别人的 Hello / 操作
                Some(bytes) = inbox.recv() => {
                    match decode(&bytes) {
                        Some(Envelope::Hello { from, name, avatar }) => {
                            if !room.members.contains_key(&from) {
                                room.members.insert(
                                    from.clone(),
                                    Member {
                                        id: from.clone(),
                                        name: name.clone(),
                                        avatar,
                                        position_ms: 0,
                                        playing: false,
                                    },
                                );
                                room.broadcast_room();
                                // 只回给新来的人：完整快照
                                let welcome = Envelope::Direct {
                                    to: from.clone(),
                                    msg: ServerMsg::Welcome {
                                        self_id: from.clone(),
                                        room: room.snapshot(),
                                    },
                                };
                                conn_task.publish(&topic_task, &encode(&welcome));
                            }
                        }
                        Some(Envelope::Client { from, msg }) => {
                            // 未知成员直接补登记（掉线重连场景）
                            if !room.members.contains_key(&from) {
                                room.members.insert(
                                    from.clone(),
                                    Member {
                                        id: from.clone(),
                                        name: format!("听友 {}", &from[..from.len().min(4)]),
                                        avatar: String::new(),
                                        position_ms: 0,
                                        playing: false,
                                    },
                                );
                                room.broadcast_room();
                            }
                            crate::room::apply_client_msg(&mut room, &from, msg);
                        }
                        _ => {}
                    }
                }

                // ③ 房间状态变化：广播到公共主题
                Ok(server_msg) = room_rx.recv() => {
                    // Kicked 只发给被踢的人
                    if let ServerMsg::Kicked { .. } = &server_msg {
                        let _ = out_for_task.send(server_msg);
                        continue;
                    }
                    conn_task.publish(&topic_task, &encode(&Envelope::Server { msg: server_msg.clone() }));
                    let _ = out_for_task.send(server_msg);
                }

                else => break,
            }
        }
    });

    HostLoop { cmd_tx, out_rx, host_id }
}

/// 把一个人的操作发给房主
pub fn client_envelope(from: &str, msg: ClientMsg) -> Vec<u8> {
    encode(&Envelope::Client {
        from: from.to_string(),
        msg,
    })
}

/// 房主离开时通知房间清空
pub fn host_left_envelope() -> Vec<u8> {
    encode(&Envelope::Host { host: String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PlaybackState;

    #[test]
    fn topic_uses_prefix_and_code() {
        assert_eq!(room_topic("bilimusic/v2", "123456"), "bilimusic/v2/room/123456");
    }

    #[test]
    fn cloud_code_is_six_digits() {
        for _ in 0..200 {
            let c = gen_cloud_code();
            assert_eq!(c.len(), 6, "房间号必须是 6 位: {c}");
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
            assert!(!c.starts_with('0'), "不该以 0 开头: {c}");
        }
    }

    #[test]
    fn envelope_roundtrip_client() {
        let env = Envelope::Client {
            from: "m-1".into(),
            msg: ClientMsg::Next,
        };
        let back = decode(&encode(&env)).expect("应能解析");
        match back {
            Envelope::Client { from, msg } => {
                assert_eq!(from, "m-1");
                assert!(matches!(msg, ClientMsg::Next));
            }
            _ => panic!("解析成了错误的变体"),
        }
    }

    #[test]
    fn envelope_server_is_compact() {
        let env = Envelope::Server {
            msg: ServerMsg::State {
                state: PlaybackState::default(),
                server_time: 1,
            },
        };
        let json = String::from_utf8(encode(&env)).unwrap();
        // 用单字母 tag，公共 broker 上流量越省越好
        assert!(json.starts_with("{\"k\":\"s\""), "信封 tag 格式变了: {json}");
    }

    #[test]
    fn envelope_direct_carries_target() {
        let env = Envelope::Direct {
            to: "m-9".into(),
            msg: ServerMsg::Kicked { reason: "测试".into() },
        };
        let back = decode(&encode(&env)).unwrap();
        match back {
            Envelope::Direct { to, .. } => assert_eq!(to, "m-9"),
            _ => panic!("变体错误"),
        }
    }

    #[test]
    fn envelope_hello_carries_name() {
        let env = Envelope::Hello {
            from: "m-2".into(),
            name: "小明".into(),
            avatar: String::new(),
        };
        let back = decode(&encode(&env)).unwrap();
        match back {
            Envelope::Hello { name, .. } => assert_eq!(name, "小明"),
            _ => panic!("变体错误"),
        }
    }

    #[test]
    fn host_left_is_recognisable() {
        let back = decode(&host_left_envelope()).unwrap();
        match back {
            Envelope::Host { host } => assert!(host.is_empty()),
            _ => panic!("变体错误"),
        }
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode(b"not json").is_none());
        assert!(decode(b"{}").is_none());
        assert!(decode(b"").is_none());
    }
}
