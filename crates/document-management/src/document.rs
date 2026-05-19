//! 文档数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub doc_type: DocumentType,
    pub category: String,
    pub status: DocumentStatus,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub tags: Vec<String>,
    pub metadata: DocumentMetadata,
}

/// 文档类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentType {
    /// 政策文件
    Policy,
    /// 标准操作程序
    Procedure,
    /// 工作指导书
    WorkInstruction,
    /// 表单
    Form,
    /// 记录
    Record,
    /// 报告
    Report,
    /// 其他
    Other,
}

/// 文档状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentStatus {
    /// 草稿
    Draft,
    /// 审核中
    UnderReview,
    /// 已批准
    Approved,
    /// 已发布
    Published,
    /// 已归档
    Archived,
    /// 已废弃
    Obsolete,
}

/// 文档元数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentMetadata {
    pub file_size: Option<u64>,
    pub file_type: Option<String>,
    pub checksum: Option<String>,
    pub retention_period: Option<u32>, // 天
    pub confidential: bool,
    pub gxp_relevant: bool,
}


/// 创建文档请求
#[derive(Debug, Clone)]
pub struct CreateDocumentRequest {
    pub title: String,
    pub content: String,
    pub doc_type: DocumentType,
    pub category: String,
    pub created_by: String,
    pub tags: Vec<String>,
}

/// 更新文档请求
#[derive(Debug, Clone)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub updated_by: String,
}

/// 文档版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub id: String,
    pub document_id: String,
    pub version: u32,
    pub content: String,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub comment: String,
}

/// 文档变更历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChange {
    pub id: String,
    pub document_id: String,
    pub change_type: ChangeType,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_at: DateTime<Utc>,
    pub changed_by: String,
}

/// 变更类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Updated,
    StatusChanged,
    Signed,
    Archived,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_variants() {
        let types = vec![
            DocumentType::Policy,
            DocumentType::Procedure,
            DocumentType::WorkInstruction,
            DocumentType::Form,
            DocumentType::Record,
            DocumentType::Report,
            DocumentType::Other,
        ];
        assert_eq!(types.len(), 7);
        assert_eq!(types[0], DocumentType::Policy);
        assert_ne!(types[0], types[6]);
    }

    #[test]
    fn test_document_status_variants() {
        let statuses = vec![
            DocumentStatus::Draft,
            DocumentStatus::UnderReview,
            DocumentStatus::Approved,
            DocumentStatus::Published,
            DocumentStatus::Archived,
            DocumentStatus::Obsolete,
        ];
        assert_eq!(statuses.len(), 6);
        assert_eq!(statuses[0], DocumentStatus::Draft);
        assert_ne!(statuses[0], statuses[5]);
    }

    #[test]
    fn test_document_metadata_default() {
        let metadata = DocumentMetadata::default();
        assert!(metadata.file_size.is_none());
        assert!(metadata.file_type.is_none());
        assert!(metadata.checksum.is_none());
        assert!(metadata.retention_period.is_none());
        assert!(!metadata.confidential);
        assert!(!metadata.gxp_relevant);
    }

    #[test]
    fn test_change_type_variants() {
        let types = vec![
            ChangeType::Created,
            ChangeType::Updated,
            ChangeType::StatusChanged,
            ChangeType::Signed,
            ChangeType::Archived,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_create_document_request() {
        let req = CreateDocumentRequest {
            title: "Test".to_string(),
            content: "Content".to_string(),
            doc_type: DocumentType::Policy,
            category: "General".to_string(),
            created_by: "admin".to_string(),
            tags: vec!["tag1".to_string()],
        };
        assert_eq!(req.title, "Test");
        assert_eq!(req.doc_type, DocumentType::Policy);
        assert_eq!(req.tags.len(), 1);
    }

    #[test]
    fn test_update_document_request() {
        let req = UpdateDocumentRequest {
            title: Some("New Title".to_string()),
            content: None,
            tags: None,
            updated_by: "admin".to_string(),
        };
        assert!(req.title.is_some());
        assert!(req.content.is_none());
        assert!(req.tags.is_none());
    }
}
