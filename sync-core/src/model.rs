use serde::{Deserialize, Serialize};

/// 曲目模型：一条 B 站视频的音频轨，
/// 一起听的队列也直接复用它，所以两端字段必须保持稳定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    /// 目前只有 "bilibili"
    pub source: String,
    /// bvid 或 歌曲 id
    pub id: String,
    pub title: String,
    pub artist: String,
    pub cover: String,
    pub duration_ms: u64,
    /// bilibili 用来记 cid
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// 播放量之类的副标题
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
}

impl Track {
    pub fn key(&self) -> String {
        format!("{}:{}", self.source, self.id)
    }
}
