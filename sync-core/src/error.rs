use std::fmt;

/// sync-core 的统一错误。房间里没有复杂错误语义，
/// 只要能把"为什么连不上"讲清楚就够。
#[derive(Debug, Clone)]
pub enum CloudError {
    Message(String),
    WebSocket(String),
    Timeout(&'static str),
}

impl fmt::Display for CloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloudError::Message(m) => write!(f, "{m}"),
            CloudError::WebSocket(m) => write!(f, "网络错误: {m}"),
            CloudError::Timeout(what) => write!(f, "{what} 超时"),
        }
    }
}

impl std::error::Error for CloudError {}

impl From<&str> for CloudError {
    fn from(v: &str) -> Self {
        CloudError::Message(v.to_string())
    }
}

impl From<String> for CloudError {
    fn from(v: String) -> Self {
        CloudError::Message(v)
    }
}

pub type CloudResult<T> = Result<T, CloudError>;
