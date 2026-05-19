//! # 企业文件处理模块
//!
//! 医药/医疗器械企业文档管理

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 文档类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentType {
    Sop,
    BatchRecord,
    ValidationReport,
    DeviationReport,
    CapaReport,
    TrainingMaterial,
    EquipmentLog,
    CalibrationRecord,
    StabilityStudy,
    Other(String),
}

/// 文档状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentStatus {
    Draft,
    UnderReview,
    Approved,
    Published,
    Archived,
    Obsolete,
}

/// 文档分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentClassification {
    Critical,
    Important,
    General,
}

/// 文档元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: String,
    pub document_number: String,
    pub title: String,
    pub version: String,
    pub doc_type: DocumentType,
    pub status: DocumentStatus,
    pub classification: DocumentClassification,
    pub author: String,
    pub department: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub effective_until: Option<DateTime<Utc>>,
    pub keywords: Vec<String>,
    pub related_documents: Vec<String>,
    pub file_hash: String,
    pub file_size_bytes: u64,
    pub file_path: String,
    pub mime_type: String,
    pub signatures: Vec<DocumentSignature>,
    pub version_history: Vec<VersionRecord>,
    pub audit_trail: Vec<DocumentAuditEntry>,
}

/// 文档电子签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSignature {
    pub id: String,
    pub signer: String,
    pub signer_title: String,
    pub meaning: SignatureMeaning,
    pub signed_at: DateTime<Utc>,
    pub signature_hash: String,
}

/// 签名含义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignatureMeaning {
    Written,
    Reviewed,
    Approved,
    Acknowledged,
}

/// 版本记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    pub version: String,
    pub modified_by: String,
    pub modified_at: DateTime<Utc>,
    pub reason: String,
    pub file_hash: String,
}

/// 文档审计条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAuditEntry {
    pub id: String,
    pub action: DocumentAction,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
    pub detail: String,
}

/// 文档操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentAction {
    Created,
    Updated,
    Reviewed,
    Approved,
    Published,
    Archived,
    Obsoleted,
    Viewed,
    Downloaded,
    Signed,
}

/// 合规检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheckResult {
    pub document_id: String,
    pub check_time: DateTime<Utc>,
    pub is_compliant: bool,
    pub checks: Vec<ComplianceCheck>,
    pub score: f64,
}

/// 单项合规检查
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
    pub reference: String,
}

/// 企业文件管理器
pub struct EnterpriseDocumentManager {
    documents: HashMap<String, DocumentMetadata>,
    document_counter: u64,
}

impl Default for EnterpriseDocumentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnterpriseDocumentManager {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            document_counter: 0,
        }
    }

    fn generate_document_number(&mut self, doc_type: &DocumentType) -> String {
        self.document_counter += 1;
        let prefix = match doc_type {
            DocumentType::Sop => "SOP",
            DocumentType::BatchRecord => "BR",
            DocumentType::ValidationReport => "VR",
            DocumentType::DeviationReport => "DR",
            DocumentType::CapaReport => "CAPA",
            DocumentType::TrainingMaterial => "TM",
            DocumentType::EquipmentLog => "EL",
            DocumentType::CalibrationRecord => "CR",
            DocumentType::StabilityStudy => "SS",
            DocumentType::Other(_) => "DOC",
        };
        format!("{}-{:04}", prefix, self.document_counter)
    }

    pub fn calculate_file_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_document(
        &mut self,
        title: String,
        doc_type: DocumentType,
        classification: DocumentClassification,
        author: String,
        department: String,
        file_path: String,
        file_content: &[u8],
        mime_type: String,
        keywords: Vec<String>,
    ) -> DocumentMetadata {
        let id = uuid::Uuid::new_v4().to_string();
        let document_number = self.generate_document_number(&doc_type);
        let file_hash = Self::calculate_file_hash(file_content);
        let now = Utc::now();

        let document = DocumentMetadata {
            id: id.clone(),
            document_number,
            title,
            version: "1.0".to_string(),
            doc_type,
            status: DocumentStatus::Draft,
            classification,
            author: author.clone(),
            department,
            created_at: now,
            updated_at: now,
            approved_at: None,
            published_at: None,
            archived_at: None,
            effective_until: None,
            keywords,
            related_documents: Vec::new(),
            file_hash: file_hash.clone(),
            file_size_bytes: file_content.len() as u64,
            file_path,
            mime_type,
            signatures: Vec::new(),
            version_history: vec![VersionRecord {
                version: "1.0".to_string(),
                modified_by: author.clone(),
                modified_at: now,
                reason: "初始版本".to_string(),
                file_hash: file_hash.clone(),
            }],
            audit_trail: vec![DocumentAuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                action: DocumentAction::Created,
                actor: author,
                timestamp: now,
                detail: "文档创建".to_string(),
            }],
        };

        self.documents.insert(id.clone(), document.clone());
        document
    }

    pub fn get_document(&self, document_id: &str) -> Option<&DocumentMetadata> {
        self.documents.get(document_id)
    }

    pub fn list_documents(&self) -> Vec<&DocumentMetadata> {
        self.documents.values().collect()
    }

    pub fn list_documents_by_status(&self, status: &DocumentStatus) -> Vec<&DocumentMetadata> {
        self.documents
            .values()
            .filter(|d| d.status == *status)
            .collect()
    }

    pub fn submit_for_review(&mut self, document_id: &str, submitter: &str) -> Result<(), String> {
        let doc = self.documents.get_mut(document_id).ok_or("文档未找到")?;
        if doc.status != DocumentStatus::Draft {
            return Err("只有草稿状态的文档才能提交审核".to_string());
        }
        doc.status = DocumentStatus::UnderReview;
        doc.updated_at = Utc::now();
        doc.audit_trail.push(DocumentAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action: DocumentAction::Reviewed,
            actor: submitter.to_string(),
            timestamp: Utc::now(),
            detail: "提交审核".to_string(),
        });
        Ok(())
    }

    pub fn approve_document(
        &mut self,
        document_id: &str,
        approver: &str,
        approver_title: &str,
    ) -> Result<(), String> {
        let doc = self.documents.get_mut(document_id).ok_or("文档未找到")?;
        if doc.status != DocumentStatus::UnderReview {
            return Err("只有审核中的文档才能批准".to_string());
        }
        let now = Utc::now();
        doc.status = DocumentStatus::Approved;
        doc.approved_at = Some(now);
        doc.updated_at = now;
        doc.signatures.push(DocumentSignature {
            id: uuid::Uuid::new_v4().to_string(),
            signer: approver.to_string(),
            signer_title: approver_title.to_string(),
            meaning: SignatureMeaning::Approved,
            signed_at: now,
            signature_hash: Self::calculate_file_hash(approver.as_bytes()),
        });
        doc.audit_trail.push(DocumentAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action: DocumentAction::Approved,
            actor: approver.to_string(),
            timestamp: now,
            detail: "文档已批准".to_string(),
        });
        Ok(())
    }

    pub fn publish_document(&mut self, document_id: &str, publisher: &str) -> Result<(), String> {
        let doc = self.documents.get_mut(document_id).ok_or("文档未找到")?;
        if doc.status != DocumentStatus::Approved {
            return Err("只有已批准的文档才能发布".to_string());
        }
        let now = Utc::now();
        doc.status = DocumentStatus::Published;
        doc.published_at = Some(now);
        doc.updated_at = now;
        doc.audit_trail.push(DocumentAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action: DocumentAction::Published,
            actor: publisher.to_string(),
            timestamp: now,
            detail: "文档已发布".to_string(),
        });
        Ok(())
    }

    pub fn archive_document(&mut self, document_id: &str, archiver: &str) -> Result<(), String> {
        let doc = self.documents.get_mut(document_id).ok_or("文档未找到")?;
        if doc.status != DocumentStatus::Published && doc.status != DocumentStatus::Obsolete {
            return Err("只有已发布或已废弃的文档才能归档".to_string());
        }
        let now = Utc::now();
        doc.status = DocumentStatus::Archived;
        doc.archived_at = Some(now);
        doc.updated_at = now;
        doc.audit_trail.push(DocumentAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action: DocumentAction::Archived,
            actor: archiver.to_string(),
            timestamp: now,
            detail: "文档已归档".to_string(),
        });
        Ok(())
    }

    pub fn obsolete_document(
        &mut self,
        document_id: &str,
        obsoleter: &str,
        reason: &str,
    ) -> Result<(), String> {
        let doc = self.documents.get_mut(document_id).ok_or("文档未找到")?;
        if doc.status == DocumentStatus::Archived {
            return Err("已归档的文档不能废弃".to_string());
        }
        let now = Utc::now();
        doc.status = DocumentStatus::Obsolete;
        doc.updated_at = now;
        doc.audit_trail.push(DocumentAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action: DocumentAction::Obsoleted,
            actor: obsoleter.to_string(),
            timestamp: now,
            detail: format!("文档已废弃: {}", reason),
        });
        Ok(())
    }

    /// 文档合规检查（FDA 21 CFR Part 11）
    pub fn check_compliance(&self, document_id: &str) -> Result<ComplianceCheckResult, String> {
        let doc = self.documents.get(document_id).ok_or("文档未找到")?;
        let mut checks = Vec::new();

        checks.push(ComplianceCheck {
            name: "文档编号".to_string(),
            passed: !doc.document_number.is_empty(),
            detail: format!("编号: {}", doc.document_number),
            reference: "FDA 21 CFR Part 11 §11.10(a)".to_string(),
        });
        checks.push(ComplianceCheck {
            name: "版本控制".to_string(),
            passed: !doc.version.is_empty() && !doc.version_history.is_empty(),
            detail: format!("版本: {}, 历史: {}", doc.version, doc.version_history.len()),
            reference: "FDA 21 CFR Part 11 §11.10(a)".to_string(),
        });
        checks.push(ComplianceCheck {
            name: "审计追踪".to_string(),
            passed: !doc.audit_trail.is_empty(),
            detail: format!("审计条目: {}", doc.audit_trail.len()),
            reference: "FDA 21 CFR Part 11 §11.10(e)".to_string(),
        });
        checks.push(ComplianceCheck {
            name: "文件完整性".to_string(),
            passed: !doc.file_hash.is_empty(),
            detail: format!(
                "SHA-256: {}...",
                &doc.file_hash[..16.min(doc.file_hash.len())]
            ),
            reference: "FDA 21 CFR Part 11 §11.10(c)".to_string(),
        });
        checks.push(ComplianceCheck {
            name: "电子签名".to_string(),
            passed: doc.status != DocumentStatus::Published || !doc.signatures.is_empty(),
            detail: format!("签名数: {}", doc.signatures.len()),
            reference: "FDA 21 CFR Part 11 §11.50".to_string(),
        });
        checks.push(ComplianceCheck {
            name: "文档分类".to_string(),
            passed: true,
            detail: format!("分类: {:?}", doc.classification),
            reference: "GAMP 5".to_string(),
        });

        let score =
            (checks.iter().filter(|c| c.passed).count() as f64 / checks.len() as f64) * 100.0;

        Ok(ComplianceCheckResult {
            document_id: document_id.to_string(),
            check_time: Utc::now(),
            is_compliant: checks.iter().all(|c| c.passed),
            checks,
            score,
        })
    }

    pub fn get_statistics(&self) -> DocumentStatistics {
        let total = self.documents.len();
        let by_status = |status: &DocumentStatus| -> usize {
            self.documents
                .values()
                .filter(|d| d.status == *status)
                .count()
        };
        DocumentStatistics {
            total,
            draft: by_status(&DocumentStatus::Draft),
            under_review: by_status(&DocumentStatus::UnderReview),
            approved: by_status(&DocumentStatus::Approved),
            published: by_status(&DocumentStatus::Published),
            archived: by_status(&DocumentStatus::Archived),
            obsolete: by_status(&DocumentStatus::Obsolete),
        }
    }
}

/// 文档统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatistics {
    pub total: usize,
    pub draft: usize,
    pub under_review: usize,
    pub approved: usize,
    pub published: usize,
    pub archived: usize,
    pub obsolete: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doc(manager: &mut EnterpriseDocumentManager) -> DocumentMetadata {
        manager.create_document(
            "测试SOP".to_string(),
            DocumentType::Sop,
            DocumentClassification::Critical,
            "张三".to_string(),
            "QA部门".to_string(),
            "/docs/sop-001.pdf".to_string(),
            b"SOP content",
            "application/pdf".to_string(),
            vec!["清洁".to_string()],
        )
    }

    #[test]
    fn test_create_document() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        assert_eq!(doc.status, DocumentStatus::Draft);
        assert!(doc.document_number.starts_with("SOP-"));
        assert!(!doc.file_hash.is_empty());
    }

    #[test]
    fn test_document_lifecycle() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        manager.submit_for_review(&doc.id, "张三").unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::UnderReview
        );
        manager.approve_document(&doc.id, "王五", "QA经理").unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::Approved
        );
        manager.publish_document(&doc.id, "王五").unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::Published
        );
        manager.archive_document(&doc.id, "系统").unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::Archived
        );
    }

    #[test]
    fn test_compliance_check() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        let result = manager.check_compliance(&doc.id).unwrap();
        assert!(result.is_compliant);
        assert_eq!(result.checks.len(), 6);
    }

    #[test]
    fn test_list_documents() {
        let mut manager = EnterpriseDocumentManager::new();
        create_test_doc(&mut manager);
        create_test_doc(&mut manager);
        assert_eq!(manager.list_documents().len(), 2);
        assert_eq!(
            manager
                .list_documents_by_status(&DocumentStatus::Draft)
                .len(),
            2
        );
    }

    #[test]
    fn test_get_statistics() {
        let mut manager = EnterpriseDocumentManager::new();
        create_test_doc(&mut manager);
        let stats = manager.get_statistics();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.draft, 1);
    }

    #[test]
    fn test_audit_trail() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();
        assert_eq!(manager.get_document(&doc.id).unwrap().audit_trail.len(), 3);
    }

    #[test]
    fn test_file_hash() {
        let h1 = EnterpriseDocumentManager::calculate_file_hash(b"content1");
        let h2 = EnterpriseDocumentManager::calculate_file_hash(b"content2");
        let h3 = EnterpriseDocumentManager::calculate_file_hash(b"content1");
        assert_ne!(h1, h2);
        assert_eq!(h1, h3);
    }

    #[test]
    fn test_invalid_transition() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        assert!(manager.approve_document(&doc.id, "李四", "QA").is_err());
    }

    #[test]
    fn test_obsolete_document() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();
        manager.publish_document(&doc.id, "李四").unwrap();
        manager
            .obsolete_document(&doc.id, "系统", "新版已发布")
            .unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::Obsolete
        );
    }

    #[test]
    fn test_get_document_not_found() {
        let manager = EnterpriseDocumentManager::new();
        assert!(manager.get_document("nonexistent").is_none());
    }

    #[test]
    fn test_list_documents_empty() {
        let manager = EnterpriseDocumentManager::new();
        assert!(manager.list_documents().is_empty());
    }

    #[test]
    fn test_list_documents_by_status_no_match() {
        let mut manager = EnterpriseDocumentManager::new();
        create_test_doc(&mut manager); // creates Draft
        assert!(manager
            .list_documents_by_status(&DocumentStatus::Published)
            .is_empty());
    }

    #[test]
    fn test_submit_review_not_found() {
        let mut manager = EnterpriseDocumentManager::new();
        let result = manager.submit_for_review("nonexistent", "张三");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_submit_review_wrong_status() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        manager.submit_for_review(&doc.id, "张三").unwrap();
        // Try submitting again when already UnderReview
        let result = manager.submit_for_review(&doc.id, "李四");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "只有草稿状态的文档才能提交审核");
    }

    #[test]
    fn test_approve_not_found() {
        let mut manager = EnterpriseDocumentManager::new();
        let result = manager.approve_document("nonexistent", "张三", "QA经理");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_approve_wrong_status() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager); // Draft
        let result = manager.approve_document(&doc.id, "张三", "QA经理");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "只有审核中的文档才能批准");
    }

    #[test]
    fn test_publish_not_found() {
        let mut manager = EnterpriseDocumentManager::new();
        let result = manager.publish_document("nonexistent", "张三");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_publish_wrong_status() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager); // Draft
        let result = manager.publish_document(&doc.id, "张三");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "只有已批准的文档才能发布");
    }

    #[test]
    fn test_archive_not_found() {
        let mut manager = EnterpriseDocumentManager::new();
        let result = manager.archive_document("nonexistent", "张三");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_archive_wrong_status() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager); // Draft
        let result = manager.archive_document(&doc.id, "张三");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "只有已发布或已废弃的文档才能归档");
    }

    #[test]
    fn test_obsolete_not_found() {
        let mut manager = EnterpriseDocumentManager::new();
        let result = manager.obsolete_document("nonexistent", "张三", "reason");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_obsolete_archived_doc() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        // Draft → UnderReview → Approved → Published → Archived
        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();
        manager.publish_document(&doc.id, "王五").unwrap();
        manager.archive_document(&doc.id, "系统").unwrap();
        let result = manager.obsolete_document(&doc.id, "赵六", "reason");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "已归档的文档不能废弃");
    }

    #[test]
    fn test_archive_from_obsolete() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        // Draft → UnderReview → Approved → Published → Obsolete → Archived
        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();
        manager.publish_document(&doc.id, "王五").unwrap();
        manager.obsolete_document(&doc.id, "赵六", "old").unwrap();
        manager.archive_document(&doc.id, "系统").unwrap();
        assert_eq!(
            manager.get_document(&doc.id).unwrap().status,
            DocumentStatus::Archived
        );
    }

    #[test]
    fn test_compliance_not_found() {
        let manager = EnterpriseDocumentManager::new();
        let result = manager.check_compliance("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "文档未找到");
    }

    #[test]
    fn test_statistics_all_statuses() {
        let mut manager = EnterpriseDocumentManager::new();
        let _doc1 = create_test_doc(&mut manager);
        let doc2 = create_test_doc(&mut manager);
        // Move doc2 to UnderReview
        manager.submit_for_review(&doc2.id, "张三").unwrap();
        let stats = manager.get_statistics();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.draft, 1);
        assert_eq!(stats.under_review, 1);
        assert_eq!(stats.approved, 0);
        assert_eq!(stats.published, 0);
        assert_eq!(stats.archived, 0);
        assert_eq!(stats.obsolete, 0);
    }

    #[test]
    fn test_create_document_types() {
        let mut manager = EnterpriseDocumentManager::new();
        let types = vec![
            DocumentType::BatchRecord,
            DocumentType::ValidationReport,
            DocumentType::DeviationReport,
            DocumentType::CapaReport,
            DocumentType::TrainingMaterial,
            DocumentType::EquipmentLog,
            DocumentType::CalibrationRecord,
            DocumentType::StabilityStudy,
            DocumentType::Other("custom".to_string()),
        ];
        let prefixes = ["BR", "VR", "DR", "CAPA", "TM", "EL", "CR", "SS", "DOC"];
        for (i, doc_type) in types.into_iter().enumerate() {
            let doc = manager.create_document(
                format!("Doc {}", i),
                doc_type,
                DocumentClassification::General,
                "author".to_string(),
                "dept".to_string(),
                "/path".to_string(),
                b"content",
                "text/plain".to_string(),
                vec![],
            );
            assert!(doc.document_number.starts_with(prefixes[i]));
        }
    }

    #[test]
    fn test_version_history_recorded() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = create_test_doc(&mut manager);
        let meta = manager.get_document(&doc.id).unwrap();
        assert_eq!(meta.version_history.len(), 1);
        assert_eq!(meta.version_history[0].version, "1.0");
        assert_eq!(meta.version_history[0].reason, "初始版本");
    }
}
