//! # AgentGuard 企业文档管理模块
//!
//! 医药企业文档合规管理，包括：
//! - 文档生命周期管理（创建→审批→发布→归档→销毁）
//! - 版本控制（Git-like 版本追踪）
//! - 电子签名（21 CFR Part 11 合规）
//! - 审计追踪（ALCOA+ 原则）
//! - 权限控制（RBAC）
//! - 文档检索（全文搜索 + 元数据搜索）

pub mod audit;
pub mod document;
pub mod error;
pub mod repository;
pub mod signature;
pub mod storage;
pub mod version;

pub use document::*;
pub use error::{DocumentError, Result};
pub use repository::DocumentRepository;
pub use signature::SignatureService;
pub use storage::DocumentStorage;
pub use version::VersionControl;

/// 文档管理系统
pub struct DocumentManagement {
    repository: DocumentRepository,
    storage: DocumentStorage,
    version_control: VersionControl,
    signature_service: SignatureService,
}

impl DocumentManagement {
    /// 创建新的文档管理系统
    pub fn new(db_path: &std::path::Path, storage_path: &std::path::Path) -> Result<Self> {
        let repository = DocumentRepository::new(db_path)?;
        let storage = DocumentStorage::new(storage_path)?;
        let version_control = VersionControl::new(db_path)?;
        let signature_service = SignatureService::new(db_path)?;

        Ok(Self {
            repository,
            storage,
            version_control,
            signature_service,
        })
    }

    /// 创建新文档
    pub fn create_document(&self, request: CreateDocumentRequest) -> Result<Document> {
        let created_by = request.created_by.clone();

        // 1. 创建文档记录
        let doc = self.repository.create(request)?;

        // 2. 存储文档内容
        self.storage.store(&doc.id, &doc.content)?;

        // 3. 创建初始版本
        self.version_control
            .create_version(&doc.id, &doc.content, "初始版本")?;

        // 4. 审计日志
        audit::record_audit(
            &self.repository.db_path,
            &doc.id,
            "created",
            &created_by,
            Some(format!("创建文档: {}", doc.title)),
        )?;

        Ok(doc)
    }

    /// 获取文档
    pub fn get_document(&self, id: &str) -> Result<Document> {
        self.repository.get(id)
    }

    /// 更新文档
    pub fn update_document(&self, id: &str, request: UpdateDocumentRequest) -> Result<Document> {
        // 1. 获取当前文档
        let current = self.repository.get(id)?;

        // 2. 检查状态（只有 Draft 状态可以更新）
        if current.status != DocumentStatus::Draft {
            return Err(DocumentError::InvalidStatus(format!(
                "文档状态 {:?} 不允许更新",
                current.status
            )));
        }

        // 保存 updated_by 用于后续版本创建
        let updated_by = request.updated_by.clone();

        // 3. 更新文档
        let updated = self.repository.update(id, request)?;

        // 4. 存储新内容
        self.storage.store(id, &updated.content)?;

        // 5. 创建新版本
        self.version_control
            .create_version_with_author(id, &updated.content, "更新文档", &updated_by)?;

        // 6. 审计日志
        audit::record_audit(
            &self.repository.db_path,
            id,
            "updated",
            &updated_by,
            Some(format!("更新文档, 版本 {}", updated.version)),
        )?;

        Ok(updated)
    }

    /// 提交审批
    pub fn submit_for_approval(&self, id: &str, submitted_by: &str) -> Result<Document> {
        let doc = self.repository.get(id)?;

        if doc.status != DocumentStatus::Draft {
            return Err(DocumentError::InvalidStatus(format!(
                "文档状态 {:?} 不允许提交审批",
                doc.status
            )));
        }

        let result = self.repository
            .update_status(id, DocumentStatus::UnderReview, submitted_by)?;

        // 审计日志
        audit::record_audit(
            &self.repository.db_path,
            id,
            "submitted_for_approval",
            submitted_by,
            Some("文档提交审批".to_string()),
        )?;

        Ok(result)
    }

    /// 审批文档
    pub fn approve_document(
        &self,
        id: &str,
        approved_by: &str,
        signature: Option<String>,
    ) -> Result<Document> {
        let doc = self.repository.get(id)?;

        if doc.status != DocumentStatus::UnderReview {
            return Err(DocumentError::InvalidStatus(format!(
                "文档状态 {:?} 不允许审批",
                doc.status
            )));
        }

        // 如果有电子签名，验证并记录
        let has_signature = signature.is_some();
        if let Some(sig) = signature {
            self.signature_service.sign(id, approved_by, &sig)?;
        }

        let result = self.repository
            .update_status(id, DocumentStatus::Approved, approved_by)?;

        // 审计日志
        audit::record_audit(
            &self.repository.db_path,
            id,
            "approved",
            approved_by,
            Some(if has_signature {
                "文档审批通过(含电子签名)".to_string()
            } else {
                "文档审批通过".to_string()
            }),
        )?;

        Ok(result)
    }

    /// 发布文档
    pub fn publish_document(&self, id: &str, published_by: &str) -> Result<Document> {
        let doc = self.repository.get(id)?;

        if doc.status != DocumentStatus::Approved {
            return Err(DocumentError::InvalidStatus(format!(
                "文档状态 {:?} 不允许发布",
                doc.status
            )));
        }

        let result = self.repository
            .update_status(id, DocumentStatus::Published, published_by)?;

        // 审计日志
        audit::record_audit(
            &self.repository.db_path,
            id,
            "published",
            published_by,
            Some("文档发布".to_string()),
        )?;

        Ok(result)
    }

    /// 归档文档
    pub fn archive_document(&self, id: &str, archived_by: &str) -> Result<Document> {
        let doc = self.repository.get(id)?;

        if doc.status != DocumentStatus::Published {
            return Err(DocumentError::InvalidStatus(format!(
                "文档状态 {:?} 不允许归档",
                doc.status
            )));
        }

        let result = self.repository
            .update_status(id, DocumentStatus::Archived, archived_by)?;

        // 审计日志
        audit::record_audit(
            &self.repository.db_path,
            id,
            "archived",
            archived_by,
            Some("文档归档".to_string()),
        )?;

        Ok(result)
    }

    /// 搜索文档
    pub fn search_documents(&self, query: &str) -> Result<Vec<Document>> {
        self.repository.search(query)
    }

    /// 获取文档版本历史
    pub fn get_version_history(&self, doc_id: &str) -> Result<Vec<DocumentVersion>> {
        self.version_control.get_history(doc_id)
    }

    /// 获取审计日志
    pub fn get_audit_log(&self, doc_id: &str) -> Result<Vec<audit::AuditEntry>> {
        audit::get_audit_log(&self.repository.db_path, doc_id)
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> Result<DocumentStatistics> {
        self.repository.get_statistics()
    }
}

/// 文档统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentStatistics {
    pub total_documents: usize,
    pub draft_count: usize,
    pub under_review_count: usize,
    pub approved_count: usize,
    pub published_count: usize,
    pub archived_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_system() -> (DocumentManagement, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let storage_path = tmp.path().join("storage");
        let system = DocumentManagement::new(&db_path, &storage_path).unwrap();
        (system, tmp)
    }

    #[test]
    fn test_create_document() {
        let (system, _tmp) = create_test_system();

        let request = CreateDocumentRequest {
            title: "测试文档".to_string(),
            content: "这是测试内容".to_string(),
            doc_type: DocumentType::Policy,
            category: "测试".to_string(),
            created_by: "test-user".to_string(),
            tags: vec!["test".to_string()],
        };

        let doc = system.create_document(request).unwrap();
        assert_eq!(doc.title, "测试文档");
        assert_eq!(doc.status, DocumentStatus::Draft);
    }

    #[test]
    fn test_document_lifecycle() {
        let (system, _tmp) = create_test_system();

        let request = CreateDocumentRequest {
            title: "生命周期测试".to_string(),
            content: "测试内容".to_string(),
            doc_type: DocumentType::Procedure,
            category: "SOP".to_string(),
            created_by: "user1".to_string(),
            tags: vec![],
        };

        let doc = system.create_document(request).unwrap();
        assert_eq!(doc.status, DocumentStatus::Draft);

        // 提交审批
        let doc = system.submit_for_approval(&doc.id, "user1").unwrap();
        assert_eq!(doc.status, DocumentStatus::UnderReview);

        // 审批
        let doc = system.approve_document(&doc.id, "approver1", None).unwrap();
        assert_eq!(doc.status, DocumentStatus::Approved);

        // 发布
        let doc = system.publish_document(&doc.id, "publisher1").unwrap();
        assert_eq!(doc.status, DocumentStatus::Published);

        // 归档
        let doc = system.archive_document(&doc.id, "admin").unwrap();
        assert_eq!(doc.status, DocumentStatus::Archived);
    }

    #[test]
    fn test_search_documents() {
        let (system, _tmp) = create_test_system();

        system
            .create_document(CreateDocumentRequest {
                title: "质量方针".to_string(),
                content: "这是质量方针内容".to_string(),
                doc_type: DocumentType::Policy,
                category: "质量".to_string(),
                created_by: "user1".to_string(),
                tags: vec!["quality".to_string()],
            })
            .unwrap();

        let results = system.search_documents("质量").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "质量方针");
    }

    #[test]
    fn test_get_statistics() {
        let (system, _tmp) = create_test_system();

        system
            .create_document(CreateDocumentRequest {
                title: "文档1".to_string(),
                content: "内容1".to_string(),
                doc_type: DocumentType::Policy,
                category: "测试".to_string(),
                created_by: "user1".to_string(),
                tags: vec![],
            })
            .unwrap();

        let stats = system.get_statistics().unwrap();
        assert_eq!(stats.total_documents, 1);
        assert_eq!(stats.draft_count, 1);
    }
}
