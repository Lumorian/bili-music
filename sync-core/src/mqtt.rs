//! 极简 MQTT 3.1.1 客户端（只覆盖云房间需要的那一小撮功能）。
//!
//! 为什么手写而不是引 mqtt crate：
//! - 只需要 CONNECT / SUBSCRIBE / PUBLISH / PINGREQ 四种包，而且都是 QoS 0；
//! - 复用已有的 tokio-tungstenite，不往依赖树里加东西（Android 交叉编译更快）；
//! - 代码量小，出问题能直接读。
//!
//! 只走 WebSocket 传输（`mqtt` 子协议）：桌面端和手机都能直连，也不用额外开端口。

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use crate::error::{CloudError, CloudResult};

/// MQTT 的变长整数（剩余长度）
fn encode_len(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            break;
        }
    }
    out
}

/// 只返回长度字段占用了几个字节（我们不需要具体数值，跳过即可）
fn decode_len(buf: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    loop {
        let byte = *buf.get(i)?;
        i += 1;
        if byte & 0x80 == 0 {
            return Some(i - start);
        }
        if i - start > 4 {
            return None; // MQTT 的剩余长度最多 4 字节
        }
    }
}

/// MQTT 字符串：2 字节大端长度 + UTF-8 内容
fn mqtt_str(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(2 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

fn connect_packet(client_id: &str, keep_alive_secs: u16) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&mqtt_str("MQTT"));
    body.push(4); // 协议级别 4 = 3.1.1
    body.push(0b0000_0010); // clean session
    body.extend_from_slice(&keep_alive_secs.to_be_bytes());
    body.extend_from_slice(&mqtt_str(client_id));
    let mut pkt = vec![0x10];
    pkt.extend_from_slice(&encode_len(body.len()));
    pkt.extend_from_slice(&body);
    pkt
}

fn subscribe_packet(packet_id: u16, topic: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&packet_id.to_be_bytes());
    body.extend_from_slice(&mqtt_str(topic));
    body.push(0x00); // 请求 QoS 0
    let mut pkt = vec![0x82]; // SUBSCRIBE 的低 4 位必须是 0b0010
    pkt.extend_from_slice(&encode_len(body.len()));
    pkt.extend_from_slice(&body);
    pkt
}

pub fn publish_packet(topic: &str, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&mqtt_str(topic));
    body.extend_from_slice(payload);
    let mut pkt = vec![0x30]; // PUBLISH, QoS 0
    pkt.extend_from_slice(&encode_len(body.len()));
    pkt.extend_from_slice(&body);
    pkt
}

fn pingreq_packet() -> Vec<u8> {
    vec![0xc0, 0x00]
}

/// 从 PUBLISH 包里取出 payload（顺便跳过 QoS>0 时的 packet id）
fn parse_publish(buf: &[u8]) -> Option<Vec<u8>> {
    let header = *buf.first()?;
    if header & 0xf0 != 0x30 {
        return None;
    }
    let len_bytes = decode_len(buf, 1)?;
    let mut i = 1 + len_bytes;
    let qos = (header >> 1) & 0x03;
    let tlen = u16::from_be_bytes([*buf.get(i)?, *buf.get(i + 1)?]) as usize;
    i += 2 + tlen;
    if qos > 0 {
        i += 2;
    }
    Some(buf.get(i..)?.to_vec())
}

/// 云房间的服务器配置。
/// 默认用 EMQX 的公共 broker —— 它免费、无需注册、支持 WSS，
/// 适合"不想自建服务器"的用法；也可以填自己的 broker。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub broker: String,
    pub topic_prefix: String,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            broker: "wss://broker.emqx.io:8084/mqtt".to_string(),
            topic_prefix: "bilimusic/v2".to_string(),
        }
    }
}

/// 已建立的 MQTT 连接。发消息是同步入队，不会阻塞 UI。
pub struct MqttConn {
    tx: UnboundedSender<Vec<u8>>,
}

impl MqttConn {
    /// 连接 broker 并订阅 topic，返回句柄与下行 payload 流。
    pub async fn connect(
        broker: &str,
        client_id: &str,
        topic: &str,
    ) -> CloudResult<(Self, UnboundedReceiver<Vec<u8>>)> {
        let mut req = broker
            .into_client_request()
            .map_err(|e| CloudError::Message(format!("broker 地址无效: {e}")))?;
        req.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            "mqtt".parse().map_err(|_| CloudError::Message("协议头构造失败".into()))?,
        );

        let connect = tokio_tungstenite::connect_async(req);
        let (ws, _) = tokio::time::timeout(Duration::from_secs(15), connect)
            .await
            .map_err(|_| CloudError::Timeout("连接公共服务器"))?
            .map_err(|e| CloudError::WebSocket(e.to_string()))?;

        let (mut sink, mut stream) = ws.split();
        sink.send(Message::Binary(connect_packet(client_id, 60).into()))
            .await
            .map_err(|e| CloudError::WebSocket(e.to_string()))?;

        let connack = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .map_err(|_| CloudError::Timeout("等待服务器握手"))?
            .ok_or_else(|| CloudError::Message("连接被服务器关闭".into()))?
            .map_err(|e| CloudError::WebSocket(e.to_string()))?;

        let bytes = match connack {
            Message::Binary(b) => b.to_vec(),
            _ => return Err(CloudError::Message("握手回应格式异常".into())),
        };
        if bytes.first() != Some(&0x20) {
            return Err(CloudError::Message("未能建立 MQTT 会话".into()));
        }
        let rc = bytes.get(3).copied().unwrap_or(255);
        if rc != 0 {
            return Err(CloudError::Message(format!("公共服务器拒绝连接（返回码 {rc}）")));
        }

        sink.send(Message::Binary(subscribe_packet(1, topic).into()))
            .await
            .map_err(|e| CloudError::WebSocket(e.to_string()))?;

        let (up_tx, mut up_rx) = unbounded_channel::<Vec<u8>>();
        let (down_tx, down_rx) = unbounded_channel::<Vec<u8>>();

        // 写方向
        tokio::spawn(async move {
            while let Some(pkt) = up_rx.recv().await {
                if sink.send(Message::Binary(pkt.into())).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        // 读方向
        let down_for_read = down_tx;
        tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                match frame {
                    Ok(Message::Binary(b)) => {
                        if let Some(payload) = parse_publish(&b) {
                            if down_for_read.send(payload).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => continue,
                }
            }
        });

        // 心跳：keep_alive 60s，每 30s 发一次 PINGREQ
        let ping_tx = up_tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                if ping_tx.send(pingreq_packet()).is_err() {
                    break;
                }
            }
        });

        Ok((Self { tx: up_tx }, down_rx))
    }

    pub fn publish(&self, topic: &str, payload: &[u8]) -> bool {
        self.tx.send(publish_packet(topic, payload)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_encoding_matches_spec() {
        assert_eq!(encode_len(0), vec![0]);
        assert_eq!(encode_len(127), vec![127]);
        // 128 -> 0x80 0x01（小端 7 位分组）
        assert_eq!(encode_len(128), vec![0x80, 0x01]);
        assert_eq!(encode_len(16383), vec![0xff, 0x7f]);
        assert_eq!(encode_len(16384), vec![0x80, 0x80, 0x01]);
    }

    #[test]
    fn subscribe_packet_has_correct_flags() {
        let pkt = subscribe_packet(1, "a/b");
        assert_eq!(pkt[0], 0x82, "SUBSCRIBE 固定头低 4 位必须是 0b0010");
        // 剩余长度 = 2(packet id) + 2(主题长度前缀) + 3("a/b") + 1(qos)
        assert_eq!(pkt[1], 8, "SUBSCRIBE 剩余长度算错了");
    }

    #[test]
    fn publish_roundtrip() {
        let pkt = publish_packet("x/y", b"hello");
        assert_eq!(pkt[0], 0x30);
        assert_eq!(parse_publish(&pkt).unwrap(), b"hello".to_vec());
    }

    #[test]
    fn parse_publish_ignores_other_packet_types() {
        assert!(parse_publish(&[0x20, 0x02, 0x00, 0x00]).is_none());
        assert!(parse_publish(&[]).is_none());
    }

    #[test]
    fn parse_publish_handles_qos1() {
        // 手工拼一个 QoS1 的 PUBLISH：主题 a，packet id 0x1234，payload "hi"
        let mut body = mqtt_str("a");
        body.extend_from_slice(&[0x12, 0x34]);
        body.extend_from_slice(b"hi");
        let mut pkt = vec![0x32]; // QoS 1
        pkt.extend_from_slice(&encode_len(body.len()));
        pkt.extend_from_slice(&body);
        assert_eq!(parse_publish(&pkt).unwrap(), b"hi".to_vec());
    }
}
