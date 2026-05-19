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

    #[error("备份操作失败: {0}")]
    BackupOperation(String),

    #[error("备份未找到: {0}")]
    BackupNotFound(String),

    #[error("备份验证失败: {0}")]
    BackupVerificationFailed(String),

    #[error("恢复失败: {0}")]
    RestoreFailed(String),

    #[error("锁中毒: {0}")]
    LockPoisoned(String),

    #[error("其他错误: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AutomationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_connection_error_display() {
        let err = AutomationError::SshConnection("timeout".to_string());
        assert_eq!(format!("{}", err), "SSH 连接失败: timeout");
    }

    #[test]
    fn test_command_execution_error_display() {
        let err = AutomationError::CommandExecution("exit code 1".to_string());
        assert_eq!(format!("{}", err), "命令执行失败: exit code 1");
    }

    #[test]
    fn test_config_error_display() {
        let err = AutomationError::Config("missing field".to_string());
        assert_eq!(format!("{}", err), "配置错误: missing field");
    }

    #[test]
    fn test_task_not_found_error_display() {
        let err = AutomationError::TaskNotFound("task-123".to_string());
        assert_eq!(format!("{}", err), "任务未找到: task-123");
    }

    #[test]
    fn test_permission_denied_error_display() {
        let err = AutomationError::PermissionDenied("root required".to_string());
        assert_eq!(format!("{}", err), "权限不足: root required");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = AutomationError::Timeout("30s".to_string());
        assert_eq!(format!("{}", err), "超时: 30s");
    }

    #[test]
    fn test_compliance_scan_error_display() {
        let err = AutomationError::ComplianceScan("CIS benchmark failed".to_string());
        assert_eq!(format!("{}", err), "合规扫描失败: CIS benchmark failed");
    }

    #[test]
    fn test_other_error_display() {
        let err = AutomationError::Other("unknown".to_string());
        assert_eq!(format!("{}", err), "其他错误: unknown");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: AutomationError = io_err.into();
        assert!(matches!(err, AutomationError::Io(_)));
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: AutomationError = json_err.into();
        assert!(matches!(err, AutomationError::Serialization(_)));
    }

    #[test]
    fn test_error_debug_trait() {
        let err = AutomationError::SshConnection("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("SshConnection"));
    }

    #[test]
    fn test_result_type_alias() {
        let ok: Result<i32> = Ok(42);
        assert!(ok.is_ok());
        if let Ok(val) = ok {
            assert_eq!(val, 42);
        }
        let err: Result<i32> = Err(AutomationError::Other("fail".to_string()));
        assert!(err.is_err());
    }
}
