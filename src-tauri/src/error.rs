use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Message(String),
    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<String> for AppError {
    fn from(v: String) -> Self {
        AppError::Message(v)
    }
}

impl From<&str> for AppError {
    fn from(v: &str) -> Self {
        AppError::Message(v.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::error::AppError::Message(format!($($arg)*)))
    };
}
