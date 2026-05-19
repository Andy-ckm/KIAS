//! 错误处理

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("文档未找到: {0}")]
    NotFound(String),

    #[error("无效状态: {0}")]
    InvalidStatus(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("签名验证失败: {0}")]
    SignatureVerification(String),

    #[error("版本冲突: {0}")]
    VersionConflict(String),

    #[error("验证失败: {0}")]
    Validation(String),

    #[error("其他错误: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DocumentError>;
