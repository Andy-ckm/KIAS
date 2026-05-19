//! # 企业文件处理模块
//!
//! 医药/医疗器械企业文档管理，包括：
//! - 文档生命周期管理（创建→审批→发布→归档→销毁）
//! - SOP（标准操作规程）管理
//! - 批记录管理
//! - 文档版本控制
//! - 合规检查（FDA 21 CFR Part 11）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 文档类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentType {
    /// 标准操作规程
    Sop,
    /// 批记录
    BatchRecord,
    /// 验证报告
    ValidationReport,
    /// 偏差报告
    DeviationReport,
    /// CAPA 报告
    CapaReport,
    /// 培训材料
    TrainingMaterial,
    /// 设备日志
    EquipmentLog,
    /// 校准记录
    CalibrationRecord,
    /// 稳定性研究
    StabilityStudy,
    /// 其他
    Other(String),
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

/// 文档分类（基于GxP影响）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentClassification {
    /// 关键文档（直接影响产品质量）
    Critical,
    /// 重要文档（间接影响）
    Important,
    /// 一般文档（无直接影响）
    General,
}

/// 文档元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// 文档ID
    pub id: String,
    /// 文档编号（人类可读）
    pub document_number: String,
    /// 标题
    pub title: String,
    /// 版本
    pub version: String,
    /// 文档类型
    pub doc_type: DocumentType,
    /// 文档状态
    pub status: DocumentStatus,
    /// 文档分类
    pub classification: DocumentClassification,
    /// 作者
    pub author: String,
    /// 部门
    pub department: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后修改时间
    pub updated_at: DateTime<Utc>,
    /// 批准时间
    pub approved_at: Option<DateTime<Utc>>,
    /// 发布时间
    pub published_at: Option<DateTime<Utc>>,
    /// 归档时间
    pub archived_at: Option<DateTime<Utc>>,
    /// 失效时间
    pub effective_until: Option<DateTime<Utc>>,
    /// 关键词
    pub keywords: Vec<String>,
    /// 关联文档
    pub related_documents: Vec<String>,
    /// 文件哈希（SHA-256）
    pub file_hash: String,
    /// 文件大小（字节）
    pub file_size_bytes: u64,
    /// 文件路径
    pub file_path: String,
    /// MIME类型
    pub mime_type: String,
    /// 电子签名
    pub signatures: Vec<DocumentSignature>,
    /// 版本历史
    pub version_history: Vec<VersionRecord>,
    /// 审计日志
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
    pub ip_address: Option<String>,
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
    /// 创建新的文件管理器
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            document_counter: 0,
        }
    }

    /// 生成文档编号
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

    /// 计算文件哈希
    pub fn calculate_file_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// 创建新文档
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
            file_hash,
            file_size_bytes: file_content.len() as u64,
            file_path,
            mime_type,
            signatures: Vec::new(),
            version_history: vec![VersionRecord {
                version: "1.0".to_string(),
                modified_by: author.clone(),
                modified_at: now,
                reason: "初始版本".to_string(),
                file_hash,
            }],
            audit_trail: vec![DocumentAuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                action: DocumentAction::Created,
                actor: author,
                timestamp: now,
                detail: "文档创建".to_string(),
                ip_address: None,
            }],
        };

        self.documents.insert(id.clone(), document.clone());
        document
    }

    /// 获取文档
    pub fn get_document(&self, document_id: &str) -> Option<&DocumentMetadata> {
        self.documents.get(document_id)
    }

    /// 列出所有文档
    pub fn list_documents(&self) -> Vec<&DocumentMetadata> {
        self.documents.values().collect()
    }

    /// 按状态筛选文档
    pub fn list_documents_by_status(&self, status: &DocumentStatus) -> Vec<&DocumentMetadata> {
        self.documents.values().filter(|d| d.status == *status).collect()
    }

    /// 提交审核
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
            ip_address: None,
        });

        Ok(())
    }

    /// 批准文档
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
            ip_address: None,
        });

        Ok(())
    }

    /// 发布文档
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
            ip_address: None,
        });

        Ok(())
    }

    /// 归档文档
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
            ip_address: None,
        });

        Ok(())
    }

    /// 废弃文档
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
            ip_address: None,
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
            detail: format!("文档编号: {}", doc.document_number),
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
            detail: format!("SHA-256: {}", &doc.file_hash[..16]),
            reference: "FDA 21 CFR Part 11 §11.10(c)".to_string(),
        });

        let has_signature = doc.status != DocumentStatus::Published || !doc.signatures.is_empty();
        checks.push(ComplianceCheck {
            name: "电子签名".to_string(),
            passed: has_signature,
            detail: format!("签名数: {}", doc.signatures.len()),
            reference: "FDA 21 CFR Part 11 §11.50".to_string(),
        });

        checks.push(ComplianceCheck {
            name: "文档分类".to_string(),
            passed: true,
            detail: format!("分类: {:?}", doc.classification),
            reference: "GAMP 5".to_string(),
        });

        let passed_count = checks.iter().filter(|c| c.passed).count() as f64;
        let score = (passed_count / checks.len() as f64) * 100.0;

        Ok(ComplianceCheckResult {
            document_id: document_id.to_string(),
            check_time: Utc::now(),
            is_compliant: checks.iter().all(|c| c.passed),
            checks,
            score,
        })
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> DocumentStatistics {
        let total = self.documents.len();
        let by_status = |status: &DocumentStatus| -> usize {
            self.documents.values().filter(|d| d.status == *status).count()
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

    #[test]
    fn test_create_document() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "清洁验证SOP".to_string(),
            DocumentType::Sop,
            DocumentClassification::Critical,
            "张三".to_string(),
            "QA部门".to_string(),
            "/docs/sop-001.pdf".to_string(),
            b"SOP内容",
            "application/pdf".to_string(),
            vec!["清洁".to_string(), "验证".to_string()],
        );

        assert_eq!(doc.status, DocumentStatus::Draft);
        assert!(doc.document_number.starts_with("SOP-"));
        assert!(!doc.file_hash.is_empty());
        assert_eq!(doc.version, "1.0");
    }

    #[test]
    fn test_document_lifecycle() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "批记录模板".to_string(),
            DocumentType::BatchRecord,
            DocumentClassification::Critical,
            "李四".to_string(),
            "生产部门".to_string(),
            "/docs/br-001.pdf".to_string(),
            b"批记录内容",
            "application/pdf".to_string(),
            vec![],
        );

        manager.submit_for_review(&doc.id, "李四").unwrap();
        assert_eq!(manager.get_document(&doc.id).unwrap().status, DocumentStatus::UnderReview);

        manager.approve_document(&doc.id, "王五", "QA经理").unwrap();
        assert_eq!(manager.get_document(&doc.id).unwrap().status, DocumentStatus::Approved);

        manager.publish_document(&doc.id, "王五").unwrap();
        assert_eq!(manager.get_document(&doc.id).unwrap().status, DocumentStatus::Published);

        manager.archive_document(&doc.id, "系统").unwrap();
        assert_eq!(manager.get_document(&doc.id).unwrap().status, DocumentStatus::Archived);
    }

    #[test]
    fn test_compliance_check() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "验证报告".to_string(),
            DocumentType::ValidationReport,
            DocumentClassification::Critical,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/vr-001.pdf".to_string(),
            b"验证内容",
            "application/pdf".to_string(),
            vec![],
        );

        let result = manager.check_compliance(&doc.id).unwrap();
        assert!(result.is_compliant);
        assert_eq!(result.checks.len(), 6);
    }

    #[test]
    fn test_list_documents_by_status() {
        let mut manager = EnterpriseDocumentManager::new();

        manager.create_document(
            "SOP1".to_string(),
            DocumentType::Sop,
            DocumentClassification::General,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/sop1.pdf".to_string(),
            b"content",
            "application/pdf".to_string(),
            vec![],
        );

        manager.create_document(
            "SOP2".to_string(),
            DocumentType::Sop,
            DocumentClassification::General,
            "李四".to_string(),
            "QA".to_string(),
            "/docs/sop2.pdf".to_string(),
            b"content",
            "application/pdf".to_string(),
            vec![],
        );

        let drafts = manager.list_documents_by_status(&DocumentStatus::Draft);
        assert_eq!(drafts.len(), 2);
    }

    #[test]
    fn test_get_statistics() {
        let mut manager = EnterpriseDocumentManager::new();

        manager.create_document(
            "SOP".to_string(),
            DocumentType::Sop,
            DocumentClassification::Critical,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/sop.pdf".to_string(),
            b"content",
            "application/pdf".to_string(),
            vec![],
        );

        let stats = manager.get_statistics();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.draft, 1);
    }

    #[test]
    fn test_audit_trail() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "测试文档".to_string(),
            DocumentType::Sop,
            DocumentClassification::General,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/test.pdf".to_string(),
            b"content",
            "application/pdf".to_string(),
            vec![],
        );

        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();

        let doc = manager.get_document(&doc.id).unwrap();
        assert_eq!(doc.audit_trail.len(), 3); // Created + Reviewed + Approved
    }

    #[test]
    fn test_file_hash_integrity() {
        let hash1 = EnterpriseDocumentManager::calculate_file_hash(b"content1");
        let hash2 = EnterpriseDocumentManager::calculate_file_hash(b"content2");
        let hash3 = EnterpriseDocumentManager::calculate_file_hash(b"content1");

        assert_ne!(hash1, hash2);
        assert_eq!(hash1, hash3);
    }

    #[test]
    fn test_invalid_status_transition() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "测试".to_string(),
            DocumentType::Sop,
            DocumentClassification::General,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/test.pdf".to_string(),
            b"content",
            "application/pdf".to_string(),
            vec![],
        );

        let result = manager.approve_document(&doc.id, "李四", "QA经理");
        assert!(result.is_err());
    }

    #[test]
    fn test_obsolete_document() {
        let mut manager = EnterpriseDocumentManager::new();
        let doc = manager.create_document(
            "旧版SOP".to_string(),
            DocumentType::Sop,
            DocumentClassification::General,
            "张三".to_string(),
            "QA".to_string(),
            "/docs/old-sop.pdf".to_string(),
            b"旧内容",
            "application/pdf".to_string(),
            vec![],
        );

        manager.submit_for_review(&doc.id, "张三").unwrap();
        manager.approve_document(&doc.id, "李四", "QA经理").unwrap();
        manager.publish_document(&doc.id, "李四").unwrap();
        manager.obsolete_document(&doc.id, "系统", "新版已发布").unwrap();

        assert_eq!(manager.get_document(&doc.id).unwrap().status, DocumentStatus::Obsolete);
    }
}
