use serde::{Deserialize, Serialize};

/// 曲目模型：一条 B 站视频的音频轨
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// 目前只有 "bilibili"
    pub source: String,
    /// bvid 或 歌曲 id
    pub id: String,
    pub title: String,
    pub artist: String,
    pub cover: String,
    pub duration_ms: u64,
    /// 分 P 的 cid，解析音轨时要用
    #[serde(default)]
    pub extra: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    /// 播放量等附加信息，界面上作为副标题
    #[serde(default)]
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub has_more: bool,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// 本地代理地址，直接塞进 <audio src>
    pub stream_url: String,
    /// 上游真实地址（调试用）
    pub url: String,
    pub mime: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub logged_in: bool,
    pub name: String,
    pub avatar: String,
    pub mid: String,
}

impl UserInfo {
    pub fn anonymous() -> Self {
        Self { logged_in: false, name: String::new(), avatar: String::new(), mid: String::new() }
    }
}
