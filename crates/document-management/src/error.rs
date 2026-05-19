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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_found() {
        let err = DocumentError::NotFound("doc-123".to_string());
        assert_eq!(err.to_string(), "文档未找到: doc-123");
    }

    #[test]
    fn test_display_invalid_status() {
        let err = DocumentError::InvalidStatus("Archived→Draft".to_string());
        assert!(err.to_string().contains("无效状态"));
        assert!(err.to_string().contains("Archived→Draft"));
    }

    #[test]
    fn test_display_permission_denied() {
        let err = DocumentError::PermissionDenied("read".to_string());
        assert!(err.to_string().contains("权限不足"));
        assert!(err.to_string().contains("read"));
    }

    #[test]
    fn test_display_signature_verification() {
        let err = DocumentError::SignatureVerification("invalid sig".to_string());
        assert!(err.to_string().contains("签名验证失败"));
        assert!(err.to_string().contains("invalid sig"));
    }

    #[test]
    fn test_display_version_conflict() {
        let err = DocumentError::VersionConflict("v2 vs v3".to_string());
        assert!(err.to_string().contains("版本冲突"));
        assert!(err.to_string().contains("v2 vs v3"));
    }

    #[test]
    fn test_display_validation() {
        let err = DocumentError::Validation("missing field".to_string());
        assert!(err.to_string().contains("验证失败"));
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn test_display_other() {
        let err = DocumentError::Other("unknown error".to_string());
        assert!(err.to_string().contains("其他错误"));
        assert!(err.to_string().contains("unknown error"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let doc_err: DocumentError = io_err.into();
        assert!(doc_err.to_string().contains("IO"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_str = "{invalid json";
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let doc_err: DocumentError = json_err.into();
        assert!(doc_err.to_string().contains("序列化错误"));
    }

    #[test]
    fn test_debug_trait() {
        let err = DocumentError::NotFound("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_result_type_alias_ok() {
        let res: Result<i32> = Ok(42);
        assert!(res.is_ok());
        if let Ok(val) = res {
            assert_eq!(val, 42);
        }
    }

    #[test]
    fn test_result_type_alias_err() {
        let res: Result<i32> = Err(DocumentError::Other("fail".to_string()));
        assert!(res.is_err());
    }
}
