//! 错误处理

use thiserror::Error;

/// 自动化错误类型
#[derive(Error, Debug)]
pub enum AutomationError {
    #[error("SSH 连接失败: {0}")]
    SshConnection(String),

    #[error("命令执行失败: {0}")]
    CommandExecution(String),

    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("任务未找到: {0}")]
    TaskNotFound(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("超时: {0}")]
    Timeout(String),

    #[error("合规扫描失败: {0}")]
    ComplianceScan(String),

    #[error("其他错误: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AutomationError>;
