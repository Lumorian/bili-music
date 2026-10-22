//! BiliMusic「一起听」核心库。
//!
//! - model   ：跨端共用的曲目模型
//! - protocol：WebSocket 消息协议
//! - room    ：房间状态机（队列、播放进度、成员）
//! - server  ：可直接 serve() 的 HTTP + WebSocket 房间服务器
//! - client  ：加入房间的客户端连接器

pub mod client;
pub mod model;
pub mod protocol;
pub mod room;
pub mod server;

pub use model::Track;
pub use protocol::{ClientMsg, MemberInfo, PlaybackState, RoomSnapshot, ServerMsg};

/// 当前时间戳（毫秒）。一起听靠它对齐播放进度。
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
