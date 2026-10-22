use serde::{Deserialize, Serialize};

use crate::model::Track;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
    pub is_host: bool,
    /// 成员当前播放进度，用于进度条上的小头像
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlaybackState {
    /// 当前曲目在队列中的下标
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub playing: bool,
    /// 服务器记录该状态的时间戳，客户端用来补偿网络延迟
    #[serde(default)]
    pub updated_at: u64,
    /// 由谁触发（成员 id），前端高亮显示
    #[serde(default)]
    pub by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub code: String,
    #[serde(default)]
    pub name: String,
    pub members: Vec<MemberInfo>,
    pub queue: Vec<Track>,
    pub state: PlaybackState,
    pub host_id: String,
    pub server_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// 进房时自报家门
    Hello {
        name: String,
        #[serde(default)]
        avatar: String,
    },
    /// 请求全量快照（刚连上 / 掉线重连）
    Sync,
    /// 心跳，带回服务器时间用于估算时钟偏移
    Ping { t: u64 },
    /// 上报自己的播放进度（成员小进度条）
    Progress { position_ms: u64, playing: bool },
    /// 整体替换队列
    SetQueue { queue: Vec<Track>, index: usize },
    /// 插入歌曲：next=true 插到当前曲目后面，否则追加到队尾
    AddTrack { track: Track, #[serde(default)] next: bool },
    RemoveTrack { index: usize },
    PlayIndex { index: usize, position_ms: u64 },
    SetPlaying { playing: bool, position_ms: u64 },
    Seek { position_ms: u64 },
    Next,
    Prev,
    Chat { text: String },
    TransferHost { member_id: String },
    Kick { member_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome { self_id: String, room: RoomSnapshot },
    Room(RoomSnapshot),
    State { state: PlaybackState, server_time: u64 },
    Queue { queue: Vec<Track>, index: usize },
    Chat { from: String, name: String, text: String, at: u64 },
    Pong { t: u64, server_time: u64 },
    Kicked { reason: String },
    Error { message: String },
}
