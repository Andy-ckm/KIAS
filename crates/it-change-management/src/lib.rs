//! # IT Change Management Module
//!
//! 医药/医疗器械企业IT系统变更管理模块
//! 符合 FDA 21 CFR Part 11, EU Annex 11, GAMP 5
//!
//! ## 核心特性
//! - GxP 影响分级（直接影响/间接影响/无影响）
//! - 紧急变更通道（事后补充审批）
//! - CAPA 联动（变更中发现的问题触发 CAPA）
//! - 验证管理（IQ/OQ/PQ）
//! - 电子签名（21 CFR Part 11 合规）
//! - SLA 跟踪与超时升级
//! - 审计追踪哈希链（防篡改）

pub mod api;
pub mod document;
pub mod linux_auto;
pub mod service;
pub mod storage;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// GxP 影响分级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GxpImpact {
    /// 直接影响：直接影响产品质量、患者安全
    Direct,
    /// 间接影响：间接影响 GxP 系统
    Indirect,
    /// 无影响：不影响 GxP 系统
    None,
}

/// 变更分类（基于 ITIL + GxP）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeCategory {
    /// 标准变更：预审批，低风险
    Standard,
    /// 普通变更：需要 CAB 审批
    Normal,
    /// 紧急变更：生产中断，事后补充文档
    Emergency,
}

/// 变更类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    /// 基础设施变更：服务器、网络、存储
    Infrastructure,
    /// 应用系统升级：ERP/LIMS/MES
    Application,
    /// 配置变更：系统参数、权限
    Configuration,
    /// 数据迁移：数据库迁移、系统切换
    DataMigration,
    /// 接口变更：系统间集成接口
    Interface,
    /// 安全变更：补丁、防火墙规则
    Security,
    /// 软件部署
    Software,
    /// 硬件变更
    Hardware,
    /// 数据变更
    Data,
}

/// 风险等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    /// 低风险：IV 级标准变更
    Low,
    /// 中风险：III 级一般变更
    Medium,
    /// 高风险：II 级重要变更
    High,
    /// 关键风险：I 级重大变更
    Critical,
}

/// 变更状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeStatus {
    /// 草稿
    Draft,
    /// 已提交
    Submitted,
    /// 审核中
    UnderReview,
    /// 已批准
    Approved,
    /// 已拒绝
    Rejected,
    /// 实施中
    Implementing,
    /// 已实施
    Implemented,
    /// 验证中
    Verifying,
    /// 已验证
    Verified,
    /// 已关闭
    Closed,
    /// 已回滚
    RolledBack,
    /// 紧急实施（事后补充审批）
    EmergencyImplemented,
    /// 等待 CAPA
    PendingCapa,
}

/// 审批决策
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Decision {
    Approved,
    Rejected,
    RequestChanges,
}

/// 审批人
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approver {
    pub user_id: String,
    pub name: String,
    pub role: String,
    pub decision: Option<Decision>,
    pub signed_at: Option<DateTime<Utc>>,
    pub signature: Option<ElectronicSignature>,
}

/// 电子签名（21 CFR Part 11 §11.50/§11.70/§11.100/§11.200）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectronicSignature {
    /// 签名含义声明（§11.50）
    pub meaning: SignatureMeaning,
    /// 签名日期时间（§11.200）
    pub signed_at: DateTime<Utc>,
    /// 身份认证方式1：密码哈希
    pub auth_factor1_hash: String,
    /// 身份认证方式2：2FA 令牌哈希（§11.200 要求至少两种认证）
    pub auth_factor2_hash: String,
    /// 签名链接到的记录 ID
    pub linked_record_id: String,
    /// 签名者姓名（打印）
    pub signer_name: String,
    /// 签名者职务
    pub signer_title: String,
}

/// 签名含义（§11.50）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignatureMeaning {
    /// 审批：我审阅并批准此变更
    Approval,
    /// 拒绝：我审阅并拒绝此变更
    Rejection,
    /// 实施：我确认已按计划实施此变更
    Implementation,
    /// 验证：我确认此变更已通过验证
    Verification,
    /// 关闭：我确认此变更已完成
    Closure,
    /// 自定义含义
    Custom(String),
}

/// 影响评估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// 受影响的系统
    pub affected_systems: Vec<String>,
    /// 受影响的用户群
    pub affected_users: Vec<String>,
    /// 预计停机时间（分钟）
    pub downtime_estimate_minutes: u32,
    /// 风险缓解措施
    pub risk_mitigation: Vec<String>,
    /// 测试要求
    pub testing_requirements: Vec<String>,
    /// GxP 影响分级
    pub gxp_impact: GxpImpact,
    /// 是否需要 CSV 验证
    pub requires_csv_validation: bool,
    /// 是否影响数据完整性
    pub affects_data_integrity: bool,
}

/// 验证级别（基于 GAMP 5）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationLevel {
    /// IQ - 安装确认
    InstallationQualification,
    /// OQ - 运行确认
    OperationalQualification,
    /// PQ - 性能确认
    PerformanceQualification,
    /// 回归测试
    RegressionTest,
    /// 功能测试
    FunctionalTest,
}

/// 验证计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationPlan {
    pub id: String,
    pub change_id: String,
    pub validation_level: ValidationLevel,
    pub test_cases: Vec<TestCase>,
    pub status: ValidationStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// 验证状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationStatus {
    Planned,
    InProgress,
    Passed,
    Failed,
    Deviation,
}

/// 测试用例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub expected_result: String,
    pub actual_result: Option<String>,
    pub status: TestStatus,
    pub executed_by: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
}

/// 测试状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestStatus {
    NotStarted,
    Passed,
    Failed,
    Blocked,
    Skipped,
}

/// CAPA（纠正预防措施）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapaRecord {
    pub id: String,
    pub change_id: String,
    pub title: String,
    pub description: String,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub status: CapaStatus,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// CAPA 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CapaStatus {
    Open,
    Investigation,
    ActionPlan,
    Implementation,
    Verification,
    Closed,
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub change_id: String,
    pub actor: String,
    pub action: AuditAction,
    pub detail: String,
    pub timestamp: DateTime<Utc>,
    /// 前一条审计记录的哈希
    pub previous_hash: String,
    /// 当前记录的哈希
    pub hash: String,
    /// IP 地址
    pub ip_address: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
}

/// 审计操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditAction {
    Created,
    Submitted,
    Reviewed,
    Approved,
    Rejected,
    Implemented,
    Verified,
    Closed,
    RolledBack,
    Signed,
    EmergencyApproved,
    CapaTriggered,
    ValidationCompleted,
    SlaEscalated,
    AttachmentAdded,
    CommentAdded,
}

/// IT 变更请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItChangeRequest {
    pub id: String,
    pub title: String,
    pub description: String,
    pub change_type: ChangeType,
    pub change_category: ChangeCategory,
    pub risk_level: RiskLevel,
    pub status: ChangeStatus,
    pub requester: String,
    pub requester_department: String,
    pub approvers: Vec<Approver>,
    pub impact_assessment: ImpactAssessment,
    pub rollback_plan: String,
    pub implementation_plan: String,
    pub verification_steps: Vec<String>,
    pub validation_plan: Option<ValidationPlan>,
    pub capa_records: Vec<CapaRecord>,
    pub attachments: Vec<Attachment>,
    pub comments: Vec<Comment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub implemented_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    /// SLA 截止时间
    pub sla_deadline: Option<DateTime<Utc>>,
    /// 紧急变更：事后补充审批截止时间
    pub emergency_approval_deadline: Option<DateTime<Utc>>,
    /// 变更编号（人类可读）
    pub change_number: String,
}

/// 附件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub file_size_bytes: u64,
    pub storage_path: String,
    pub uploaded_by: String,
    pub uploaded_at: DateTime<Utc>,
    pub hash_sha256: String,
}

/// 评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub is_internal: bool,
}

/// SLA 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaConfig {
    /// 关键变更 SLA（小时）
    pub critical_hours: u32,
    /// 高风险变更 SLA（小时）
    pub high_hours: u32,
    /// 中风险变更 SLA（小时）
    pub medium_hours: u32,
    /// 低风险变更 SLA（小时）
    pub low_hours: u32,
    /// 紧急变更事后补充审批期限（小时）
    pub emergency_approval_hours: u32,
}

impl Default for SlaConfig {
    fn default() -> Self {
        Self {
            critical_hours: 720,          // 30 天
            high_hours: 336,              // 14 天
            medium_hours: 168,            // 7 天
            low_hours: 72,                // 3 天
            emergency_approval_hours: 72, // 3 天内补充审批
        }
    }
}

/// IT 变更管理器
pub struct ItChangeManager {
    changes: Vec<ItChangeRequest>,
    audit_log: Vec<AuditEntry>,
    sla_config: SlaConfig,
    change_counter: u64,
}

impl Default for ItChangeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ItChangeManager {
    /// 创建新的变更管理器
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            audit_log: Vec::new(),
            sla_config: SlaConfig::default(),
            change_counter: 0,
        }
    }

    /// 使用自定义 SLA 配置创建
    pub fn with_sla_config(sla_config: SlaConfig) -> Self {
        Self {
            changes: Vec::new(),
            audit_log: Vec::new(),
            sla_config,
            change_counter: 0,
        }
    }

    /// 生成变更编号
    fn generate_change_number(&mut self) -> String {
        self.change_counter += 1;
        let now = Utc::now();
        format!("CHG-{}-{:04}", now.format("%Y%m"), self.change_counter)
    }

    /// 计算 SLA 截止时间
    fn calculate_sla_deadline(&self, risk_level: &RiskLevel) -> DateTime<Utc> {
        let hours = match risk_level {
            RiskLevel::Critical => self.sla_config.critical_hours,
            RiskLevel::High => self.sla_config.high_hours,
            RiskLevel::Medium => self.sla_config.medium_hours,
            RiskLevel::Low => self.sla_config.low_hours,
        };
        Utc::now() + Duration::hours(hours as i64)
    }

    /// 创建变更请求
    #[allow(clippy::too_many_arguments)]
    pub fn create_change_request(
        &mut self,
        title: String,
        description: String,
        change_type: ChangeType,
        change_category: ChangeCategory,
        risk_level: RiskLevel,
        requester: String,
        requester_department: String,
        rollback_plan: String,
        implementation_plan: String,
        impact_assessment: ImpactAssessment,
    ) -> ItChangeRequest {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let change_number = self.generate_change_number();
        let sla_deadline = self.calculate_sla_deadline(&risk_level);

        let change = ItChangeRequest {
            id: id.clone(),
            title,
            description,
            change_type,
            change_category,
            risk_level,
            status: ChangeStatus::Draft,
            requester: requester.clone(),
            requester_department,
            approvers: Vec::new(),
            impact_assessment,
            rollback_plan,
            implementation_plan,
            verification_steps: Vec::new(),
            validation_plan: None,
            capa_records: Vec::new(),
            attachments: Vec::new(),
            comments: Vec::new(),
            created_at: now,
            updated_at: now,
            submitted_at: None,
            approved_at: None,
            implemented_at: None,
            verified_at: None,
            closed_at: None,
            sla_deadline: Some(sla_deadline),
            emergency_approval_deadline: None,
            change_number,
        };

        self.changes.push(change.clone());
        self.add_audit_entry(
            &id,
            &requester,
            AuditAction::Created,
            &format!("变更请求 {} 已创建", change.change_number),
            None,
            None,
        );

        change
    }

    /// 提交审批
    pub fn submit_for_review(
        &mut self,
        change_id: &str,
        submitter: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Draft {
            return Err("只有草稿状态的变更才能提交审批".to_string());
        }

        change.status = ChangeStatus::Submitted;
        change.submitted_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            submitter,
            AuditAction::Submitted,
            "变更已提交审批",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 添加审批人
    pub fn add_approver(
        &mut self,
        change_id: &str,
        user_id: String,
        name: String,
        role: String,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Submitted && change.status != ChangeStatus::UnderReview {
            return Err("只有已提交或审核中的变更才能添加审批人".to_string());
        }

        change.approvers.push(Approver {
            user_id,
            name,
            role,
            decision: None,
            signed_at: None,
            signature: None,
        });

        change.status = ChangeStatus::UnderReview;
        change.updated_at = Utc::now();

        Ok(())
    }

    /// 审批变更（带电子签名）
    #[allow(clippy::too_many_arguments)]
    pub fn approve_change(
        &mut self,
        change_id: &str,
        approver_id: &str,
        decision: Decision,
        signature: ElectronicSignature,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let (should_reject, should_approve) = {
            let change = self.get_change_mut(change_id)?;

            if change.status != ChangeStatus::UnderReview {
                return Err("只有审核中的变更才能审批".to_string());
            }

            let approver = change
                .approvers
                .iter_mut()
                .find(|a| a.user_id == approver_id)
                .ok_or("未找到该审批人")?;

            approver.decision = Some(decision.clone());
            approver.signed_at = Some(signature.signed_at);
            approver.signature = Some(signature);

            let all_approved = change
                .approvers
                .iter()
                .all(|a| a.decision == Some(Decision::Approved));

            let any_rejected = change
                .approvers
                .iter()
                .any(|a| a.decision == Some(Decision::Rejected));

            (any_rejected, all_approved)
        };

        if should_reject {
            let change = self.get_change_mut(change_id)?;
            change.status = ChangeStatus::Rejected;
            change.updated_at = Utc::now();
            self.add_audit_entry(
                change_id,
                approver_id,
                AuditAction::Rejected,
                "变更已被拒绝",
                ip_address,
                user_agent,
            );
        } else if should_approve {
            let change = self.get_change_mut(change_id)?;
            change.status = ChangeStatus::Approved;
            change.approved_at = Some(Utc::now());
            change.updated_at = Utc::now();
            self.add_audit_entry(
                change_id,
                approver_id,
                AuditAction::Approved,
                "变更已批准",
                ip_address,
                user_agent,
            );
        } else {
            let change = self.get_change_mut(change_id)?;
            change.updated_at = Utc::now();
        }

        Ok(())
    }

    /// 紧急变更实施（事后补充审批）
    pub fn emergency_implement(
        &mut self,
        change_id: &str,
        implementer: &str,
        reason: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let emergency_hours = self.sla_config.emergency_approval_hours;
        let change = self.get_change_mut(change_id)?;

        if change.change_category != ChangeCategory::Emergency {
            return Err("只有紧急变更才能走紧急实施通道".to_string());
        }

        // 紧急变更可以跳过审批直接实施
        if change.status != ChangeStatus::Submitted
            && change.status != ChangeStatus::UnderReview
            && change.status != ChangeStatus::Approved
        {
            return Err("变更状态不允许紧急实施".to_string());
        }

        change.status = ChangeStatus::EmergencyImplemented;
        change.implemented_at = Some(Utc::now());
        change.updated_at = Utc::now();
        change.emergency_approval_deadline =
            Some(Utc::now() + Duration::hours(emergency_hours as i64));

        self.add_audit_entry(
            change_id,
            implementer,
            AuditAction::EmergencyApproved,
            &format!("紧急变更已实施，原因: {}", reason),
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 实施变更
    pub fn implement_change(
        &mut self,
        change_id: &str,
        implementer: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Approved {
            return Err("只有已批准的变更才能实施".to_string());
        }

        change.status = ChangeStatus::Implementing;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            implementer,
            AuditAction::Implemented,
            "变更开始实施",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 完成实施
    pub fn complete_implementation(
        &mut self,
        change_id: &str,
        implementer: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implementing {
            return Err("只有实施中的变更才能完成实施".to_string());
        }

        change.status = ChangeStatus::Implemented;
        change.implemented_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            implementer,
            AuditAction::Implemented,
            "变更实施完成",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 开始验证
    pub fn verify_change(
        &mut self,
        change_id: &str,
        verifier: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implemented
            && change.status != ChangeStatus::EmergencyImplemented
        {
            return Err("只有已实施的变更才能验证".to_string());
        }

        change.status = ChangeStatus::Verifying;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            verifier,
            AuditAction::Verified,
            "变更开始验证",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 完成验证
    pub fn complete_verification(
        &mut self,
        change_id: &str,
        verifier: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Verifying {
            return Err("只有验证中的变更才能完成验证".to_string());
        }

        change.status = ChangeStatus::Verified;
        change.verified_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            verifier,
            AuditAction::Verified,
            "变更验证完成",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 关闭变更
    pub fn close_change(
        &mut self,
        change_id: &str,
        closer: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Verified {
            return Err("只有已验证的变更才能关闭".to_string());
        }

        change.status = ChangeStatus::Closed;
        change.closed_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            closer,
            AuditAction::Closed,
            "变更已关闭",
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 回滚变更
    pub fn rollback_change(
        &mut self,
        change_id: &str,
        rollback_by: &str,
        reason: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implementing
            && change.status != ChangeStatus::Implemented
            && change.status != ChangeStatus::Verifying
        {
            return Err("只有实施中、已实施或验证中的变更才能回滚".to_string());
        }

        change.status = ChangeStatus::RolledBack;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            rollback_by,
            AuditAction::RolledBack,
            &format!("变更已回滚: {}", reason),
            ip_address,
            user_agent,
        );

        Ok(())
    }

    /// 触发 CAPA
    pub fn trigger_capa(
        &mut self,
        change_id: &str,
        triggered_by: &str,
        title: String,
        description: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<String, String> {
        let change = self.get_change_mut(change_id)?;

        let capa_id = Uuid::new_v4().to_string();
        let capa = CapaRecord {
            id: capa_id.clone(),
            change_id: change_id.to_string(),
            title,
            description,
            root_cause: None,
            corrective_action: None,
            preventive_action: None,
            status: CapaStatus::Open,
            created_at: Utc::now(),
            closed_at: None,
        };

        change.capa_records.push(capa);
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            triggered_by,
            AuditAction::CapaTriggered,
            &format!("CAPA {} 已触发", capa_id),
            ip_address,
            user_agent,
        );

        Ok(capa_id)
    }

    /// 添加评论
    pub fn add_comment(
        &mut self,
        change_id: &str,
        author: &str,
        content: String,
        is_internal: bool,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        change.comments.push(Comment {
            id: Uuid::new_v4().to_string(),
            author: author.to_string(),
            content,
            created_at: Utc::now(),
            is_internal,
        });

        change.updated_at = Utc::now();
        Ok(())
    }

    /// 添加附件
    #[allow(clippy::too_many_arguments)]
    pub fn add_attachment(
        &mut self,
        change_id: &str,
        filename: String,
        file_type: String,
        file_size_bytes: u64,
        storage_path: String,
        uploaded_by: &str,
        hash_sha256: String,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        change.attachments.push(Attachment {
            id: Uuid::new_v4().to_string(),
            filename,
            file_type,
            file_size_bytes,
            storage_path,
            uploaded_by: uploaded_by.to_string(),
            uploaded_at: Utc::now(),
            hash_sha256,
        });

        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            uploaded_by,
            AuditAction::AttachmentAdded,
            "附件已添加",
            None,
            None,
        );

        Ok(())
    }

    /// 检查 SLA 是否超期
    pub fn check_sla_violations(&self) -> Vec<&ItChangeRequest> {
        let now = Utc::now();
        self.changes
            .iter()
            .filter(|c| {
                if let Some(deadline) = c.sla_deadline {
                    c.status != ChangeStatus::Closed
                        && c.status != ChangeStatus::Rejected
                        && c.status != ChangeStatus::RolledBack
                        && now > deadline
                } else {
                    false
                }
            })
            .collect()
    }

    /// 获取变更详情
    pub fn get_change(&self, change_id: &str) -> Result<&ItChangeRequest, String> {
        self.changes
            .iter()
            .find(|c| c.id == change_id)
            .ok_or_else(|| format!("未找到变更请求: {}", change_id))
    }

    /// 获取变更列表
    pub fn list_changes(&self) -> Vec<&ItChangeRequest> {
        self.changes.iter().collect()
    }

    /// 按状态筛选变更
    pub fn list_changes_by_status(&self, status: &ChangeStatus) -> Vec<&ItChangeRequest> {
        self.changes
            .iter()
            .filter(|c| c.status == *status)
            .collect()
    }

    /// 按风险等级筛选变更
    pub fn list_changes_by_risk(&self, risk_level: &RiskLevel) -> Vec<&ItChangeRequest> {
        self.changes
            .iter()
            .filter(|c| c.risk_level == *risk_level)
            .collect()
    }

    /// 获取变更的审计日志
    pub fn get_audit_log(&self, change_id: &str) -> Vec<&AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.change_id == change_id)
            .collect()
    }

    /// 获取全部审计日志
    pub fn get_all_audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// 获取统计数据
    pub fn get_statistics(&self) -> ChangeStatistics {
        let total = self.changes.len();
        let by_status = |status: &ChangeStatus| -> usize {
            self.changes.iter().filter(|c| c.status == *status).count()
        };
        let by_risk = |risk: &RiskLevel| -> usize {
            self.changes
                .iter()
                .filter(|c| c.risk_level == *risk)
                .count()
        };

        ChangeStatistics {
            total,
            draft: by_status(&ChangeStatus::Draft),
            submitted: by_status(&ChangeStatus::Submitted),
            under_review: by_status(&ChangeStatus::UnderReview),
            approved: by_status(&ChangeStatus::Approved),
            implementing: by_status(&ChangeStatus::Implementing),
            implemented: by_status(&ChangeStatus::Implemented),
            verifying: by_status(&ChangeStatus::Verifying),
            verified: by_status(&ChangeStatus::Verified),
            closed: by_status(&ChangeStatus::Closed),
            rejected: by_status(&ChangeStatus::Rejected),
            rolled_back: by_status(&ChangeStatus::RolledBack),
            emergency_implemented: by_status(&ChangeStatus::EmergencyImplemented),
            low_risk: by_risk(&RiskLevel::Low),
            medium_risk: by_risk(&RiskLevel::Medium),
            high_risk: by_risk(&RiskLevel::High),
            critical_risk: by_risk(&RiskLevel::Critical),
            sla_violations: self.check_sla_violations().len(),
        }
    }

    // 内部方法

    fn get_change_mut(&mut self, change_id: &str) -> Result<&mut ItChangeRequest, String> {
        self.changes
            .iter_mut()
            .find(|c| c.id == change_id)
            .ok_or_else(|| format!("未找到变更请求: {}", change_id))
    }

    fn add_audit_entry(
        &mut self,
        change_id: &str,
        actor: &str,
        action: AuditAction,
        detail: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let previous_hash = self
            .audit_log
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| "0".repeat(64));

        let hash_input = format!(
            "{}{}{}{}{}{}",
            id,
            change_id,
            actor,
            serde_json::to_string(&action).unwrap_or_default(),
            detail,
            now.to_rfc3339()
        );
        let hash = sha256_hash(&hash_input);

        let entry = AuditEntry {
            id,
            change_id: change_id.to_string(),
            actor: actor.to_string(),
            action,
            detail: detail.to_string(),
            timestamp: now,
            previous_hash,
            hash,
            ip_address,
            user_agent,
        };

        self.audit_log.push(entry);
    }
}

/// 变更统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeStatistics {
    pub total: usize,
    pub draft: usize,
    pub submitted: usize,
    pub under_review: usize,
    pub approved: usize,
    pub implementing: usize,
    pub implemented: usize,
    pub verifying: usize,
    pub verified: usize,
    pub closed: usize,
    pub rejected: usize,
    pub rolled_back: usize,
    pub emergency_implemented: usize,
    pub low_risk: usize,
    pub medium_risk: usize,
    pub high_risk: usize,
    pub critical_risk: usize,
    pub sla_violations: usize,
}

/// SHA-256 哈希函数
fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_impact_assessment() -> ImpactAssessment {
        ImpactAssessment {
            affected_systems: vec!["LIMS".to_string()],
            affected_users: vec!["QC部门".to_string()],
            downtime_estimate_minutes: 30,
            risk_mitigation: vec!["备份原配置".to_string()],
            testing_requirements: vec!["验证新阈值生效".to_string()],
            gxp_impact: GxpImpact::Direct,
            requires_csv_validation: true,
            affects_data_integrity: true,
        }
    }

    fn create_test_signature(meaning: SignatureMeaning) -> ElectronicSignature {
        ElectronicSignature {
            meaning,
            signed_at: Utc::now(),
            auth_factor1_hash: sha256_hash("password123"),
            auth_factor2_hash: sha256_hash("token456"),
            linked_record_id: "test-record".to_string(),
            signer_name: "张三".to_string(),
            signer_title: "QA主管".to_string(),
        }
    }

    #[test]
    fn test_create_change_request() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "更新LIMS配置".to_string(),
            "更新LIMS系统的样品检测阈值参数".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::High,
            "zhang.qa".to_string(),
            "QA部门".to_string(),
            "回滚到原配置文件".to_string(),
            "1. 停止LIMS服务\n2. 修改配置文件\n3. 重启服务".to_string(),
            create_test_impact_assessment(),
        );

        assert_eq!(change.status, ChangeStatus::Draft);
        assert_eq!(change.change_type, ChangeType::Configuration);
        assert_eq!(change.risk_level, RiskLevel::High);
        assert!(change.change_number.starts_with("CHG-"));
        assert!(change.sla_deadline.is_some());
    }

    #[test]
    fn test_submit_for_review() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            create_test_impact_assessment(),
        );

        let result = manager.submit_for_review(
            &change.id,
            "test.user",
            Some("192.168.1.1".to_string()),
            Some("Mozilla/5.0".to_string()),
        );
        assert!(result.is_ok());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::Submitted);
        assert!(change.submitted_at.is_some());
    }

    #[test]
    fn test_approval_workflow_with_electronic_signature() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "test.user", None, None)
            .unwrap();

        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人1".to_string(),
                "QA主管".to_string(),
            )
            .unwrap();

        let signature = create_test_signature(SignatureMeaning::Approval);
        let result = manager.approve_change(
            &change.id,
            "approver1",
            Decision::Approved,
            signature,
            None,
            None,
        );
        assert!(result.is_ok());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::Approved);
        assert!(change.approved_at.is_some());

        // 验证审计日志
        let audit_log = manager.get_audit_log(&change.id);
        assert!(audit_log.iter().any(|e| e.action == AuditAction::Approved));
    }

    #[test]
    fn test_emergency_change_path() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "紧急修复LIMS".to_string(),
            "LIMS系统宕机，需要紧急修复".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Emergency,
            RiskLevel::Critical,
            "ops.lead".to_string(),
            "运维部门".to_string(),
            "恢复备份".to_string(),
            "紧急修复步骤".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "ops.lead", None, None)
            .unwrap();

        let result = manager.emergency_implement(
            &change.id,
            "ops.lead",
            "生产系统宕机，需要立即修复",
            None,
            None,
        );
        assert!(result.is_ok());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::EmergencyImplemented);
        assert!(change.emergency_approval_deadline.is_some());
    }

    #[test]
    fn test_capa_trigger() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            create_test_impact_assessment(),
        );

        let capa_id = manager
            .trigger_capa(
                &change.id,
                "qa.inspector",
                "发现配置偏差".to_string(),
                "验证过程中发现配置参数超出预期范围".to_string(),
                None,
                None,
            )
            .unwrap();

        assert!(!capa_id.is_empty());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.capa_records.len(), 1);
        assert_eq!(change.capa_records[0].status, CapaStatus::Open);
    }

    #[test]
    fn test_audit_log_hash_chain() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "test.user", None, None)
            .unwrap();

        let audit_log = manager.get_audit_log(&change.id);
        assert_eq!(audit_log.len(), 2);

        // 验证哈希链
        for i in 1..audit_log.len() {
            assert_eq!(audit_log[i].previous_hash, audit_log[i - 1].hash);
        }
    }

    #[test]
    fn test_sla_violations() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            create_test_impact_assessment(),
        );

        // 手动设置 SLA 截止时间为过去
        let change = manager.get_change_mut(&change.id).unwrap();
        change.sla_deadline = Some(Utc::now() - Duration::hours(1));

        let violations = manager.check_sla_violations();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_full_change_lifecycle() {
        let mut manager = ItChangeManager::new();

        // 1. 创建
        let change = manager.create_change_request(
            "升级LIMS到v3.0".to_string(),
            "将LIMS系统从v2.5升级到v3.0".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::High,
            "it.admin".to_string(),
            "IT部门".to_string(),
            "回滚到v2.5备份".to_string(),
            "1. 备份数据\n2. 停止服务\n3. 安装v3.0\n4. 迁移数据\n5. 启动服务".to_string(),
            ImpactAssessment {
                affected_systems: vec!["LIMS".to_string(), "MES".to_string()],
                affected_users: vec!["QC部门".to_string(), "生产部门".to_string()],
                downtime_estimate_minutes: 120,
                risk_mitigation: vec!["完整备份".to_string(), "回滚计划".to_string()],
                testing_requirements: vec!["功能测试".to_string(), "性能测试".to_string()],
                gxp_impact: GxpImpact::Direct,
                requires_csv_validation: true,
                affects_data_integrity: true,
            },
        );
        assert_eq!(change.status, ChangeStatus::Draft);

        // 2. 提交
        manager
            .submit_for_review(&change.id, "it.admin", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Submitted
        );

        // 3. 添加审批人
        manager
            .add_approver(
                &change.id,
                "qa.head".to_string(),
                "QA主管".to_string(),
                "QA审批".to_string(),
            )
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::UnderReview
        );

        // 4. 审批通过
        let sig = create_test_signature(SignatureMeaning::Approval);
        manager
            .approve_change(&change.id, "qa.head", Decision::Approved, sig, None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Approved
        );

        // 5. 实施
        manager
            .implement_change(&change.id, "it.admin", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Implementing
        );

        // 6. 完成实施
        manager
            .complete_implementation(&change.id, "it.admin", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Implemented
        );

        // 7. 验证
        manager
            .verify_change(&change.id, "qa.tester", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Verifying
        );

        // 8. 完成验证
        manager
            .complete_verification(&change.id, "qa.tester", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Verified
        );

        // 9. 关闭
        manager
            .close_change(&change.id, "it.admin", None, None)
            .unwrap();
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Closed
        );

        // 验证审计日志完整
        let audit_log = manager.get_audit_log(&change.id);
        assert_eq!(audit_log.len(), 8); // created, submitted, approved, implementing, implemented, verifying, verified, closed
    }

    #[test]
    fn test_statistics() {
        let mut manager = ItChangeManager::new();

        manager.create_change_request(
            "变更1".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager.create_change_request(
            "变更2".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::High,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        let stats = manager.get_statistics();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.draft, 2);
        assert_eq!(stats.low_risk, 1);
        assert_eq!(stats.high_risk, 1);
    }

    #[test]
    fn test_rejection() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "test.user", None, None)
            .unwrap();
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人".to_string(),
                "QA".to_string(),
            )
            .unwrap();

        let sig = create_test_signature(SignatureMeaning::Rejection);
        manager
            .approve_change(&change.id, "approver1", Decision::Rejected, sig, None, None)
            .unwrap();

        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Rejected
        );
    }

    #[test]
    fn test_rollback() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "test.user", None, None)
            .unwrap();
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人".to_string(),
                "QA".to_string(),
            )
            .unwrap();

        let sig = create_test_signature(SignatureMeaning::Approval);
        manager
            .approve_change(&change.id, "approver1", Decision::Approved, sig, None, None)
            .unwrap();

        manager
            .implement_change(&change.id, "test.user", None, None)
            .unwrap();
        manager
            .rollback_change(&change.id, "test.user", "发现异常", None, None)
            .unwrap();

        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::RolledBack
        );
    }

    #[test]
    fn test_add_comment() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .add_comment(&change.id, "user1", "这是一个公开评论".to_string(), false)
            .unwrap();
        manager
            .add_comment(&change.id, "user2", "内部备注".to_string(), true)
            .unwrap();

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.comments.len(), 2);
        assert_eq!(change.comments[0].author, "user1");
        assert!(!change.comments[0].is_internal);
        assert_eq!(change.comments[1].author, "user2");
        assert!(change.comments[1].is_internal);
    }

    #[test]
    fn test_add_comment_not_found() {
        let mut manager = ItChangeManager::new();
        let result = manager.add_comment("nonexistent", "user", "content".to_string(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_attachment() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .add_attachment(
                &change.id,
                "config.xml".to_string(),
                "application/xml".to_string(),
                1024,
                "/uploads/config.xml".to_string(),
                "user1",
                "abc123def456".to_string(),
            )
            .unwrap();

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.attachments.len(), 1);
        assert_eq!(change.attachments[0].filename, "config.xml");
        assert_eq!(change.attachments[0].file_size_bytes, 1024);
    }

    #[test]
    fn test_add_attachment_not_found() {
        let mut manager = ItChangeManager::new();
        let result = manager.add_attachment(
            "nonexistent",
            "file.txt".to_string(),
            "text/plain".to_string(),
            100,
            "/path".to_string(),
            "user",
            "hash".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_list_changes_by_status() {
        let mut manager = ItChangeManager::new();

        let c1 = manager.create_change_request(
            "变更1".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let _c2 = manager.create_change_request(
            "变更2".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&c1.id, "user", None, None)
            .unwrap();

        let drafts = manager.list_changes_by_status(&ChangeStatus::Draft);
        let submitted = manager.list_changes_by_status(&ChangeStatus::Submitted);
        assert_eq!(drafts.len(), 1);
        assert_eq!(submitted.len(), 1);
    }

    #[test]
    fn test_list_changes_by_risk() {
        let mut manager = ItChangeManager::new();

        manager.create_change_request(
            "低风险".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager.create_change_request(
            "高风险".to_string(),
            "描述".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Normal,
            RiskLevel::High,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager.create_change_request(
            "低风险2".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        let low = manager.list_changes_by_risk(&RiskLevel::Low);
        let high = manager.list_changes_by_risk(&RiskLevel::High);
        assert_eq!(low.len(), 2);
        assert_eq!(high.len(), 1);
    }

    #[test]
    fn test_get_all_audit_log() {
        let mut manager = ItChangeManager::new();

        let c1 = manager.create_change_request(
            "变更1".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let c2 = manager.create_change_request(
            "变更2".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        let all_audit = manager.get_all_audit_log();
        assert_eq!(all_audit.len(), 2); // one per change creation

        manager
            .submit_for_review(&c1.id, "user", None, None)
            .unwrap();
        manager
            .submit_for_review(&c2.id, "user", None, None)
            .unwrap();

        let all_audit = manager.get_all_audit_log();
        assert_eq!(all_audit.len(), 4); // 2 created + 2 submitted
    }

    #[test]
    fn test_get_change_not_found() {
        let manager = ItChangeManager::new();
        let result = manager.get_change("nonexistent-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_without_submit() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        // Try to approve without submitting (still Draft)
        let sig = create_test_signature(SignatureMeaning::Approval);
        let result =
            manager.approve_change(&change.id, "approver", Decision::Approved, sig, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_unknown_approver() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "user", None, None)
            .unwrap();
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人".to_string(),
                "QA".to_string(),
            )
            .unwrap();

        // Try to approve with unknown approver
        let sig = create_test_signature(SignatureMeaning::Approval);
        let result = manager.approve_change(
            &change.id,
            "wrong_approver",
            Decision::Approved,
            sig,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_multi_approver_all_must_approve() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Normal,
            RiskLevel::Critical,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "user", None, None)
            .unwrap();
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人1".to_string(),
                "QA".to_string(),
            )
            .unwrap();
        manager
            .add_approver(
                &change.id,
                "approver2".to_string(),
                "审批人2".to_string(),
                "IT".to_string(),
            )
            .unwrap();

        // Only approver1 approves
        let sig = create_test_signature(SignatureMeaning::Approval);
        manager
            .approve_change(&change.id, "approver1", Decision::Approved, sig, None, None)
            .unwrap();

        // Still UnderReview because approver2 hasn't approved
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::UnderReview
        );

        // approver2 approves
        let sig = create_test_signature(SignatureMeaning::Approval);
        manager
            .approve_change(&change.id, "approver2", Decision::Approved, sig, None, None)
            .unwrap();

        // Now approved
        assert_eq!(
            manager.get_change(&change.id).unwrap().status,
            ChangeStatus::Approved
        );
    }

    #[test]
    fn test_emergency_approval_deadline_set() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "紧急变更".to_string(),
            "描述".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Emergency,
            RiskLevel::Critical,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .submit_for_review(&change.id, "user", None, None)
            .unwrap();
        manager
            .emergency_implement(&change.id, "user", "紧急情况", None, None)
            .unwrap();

        let change = manager.get_change(&change.id).unwrap();
        assert!(change.emergency_approval_deadline.is_some());
        // Emergency approval deadline should be in the future
        assert!(change.emergency_approval_deadline.unwrap() > Utc::now());
    }

    #[test]
    fn test_sla_deadline_on_create() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        assert!(change.sla_deadline.is_some());
        assert!(change.sla_deadline.unwrap() > Utc::now());
    }

    #[test]
    fn test_rollback_not_implementing() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        // Try rollback from Draft status (not Implementing)
        let result = manager.rollback_change(&change.id, "user", "原因", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_with_sla_config() {
        let sla = SlaConfig {
            low_hours: 48,
            medium_hours: 24,
            high_hours: 12,
            critical_hours: 4,
            emergency_approval_hours: 24,
        };
        let manager = ItChangeManager::with_sla_config(sla);

        // Verify it doesn't panic and can create changes
        let mut manager = manager;
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Critical,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        assert!(change.sla_deadline.is_some());
        // Critical risk should have shorter SLA
        let deadline = change.sla_deadline.unwrap();
        let four_hours_from_now = Utc::now() + Duration::hours(4);
        // Deadline should be roughly 4 hours from now (within 1 minute tolerance)
        let diff = (deadline - four_hours_from_now).num_seconds().abs();
        assert!(diff < 60, "SLA deadline should be ~4 hours from now");
    }

    #[test]
    fn test_statistics_with_mixed_statuses() {
        let mut manager = ItChangeManager::new();

        // Create changes with different risk levels
        manager.create_change_request(
            "低风险".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager.create_change_request(
            "中风险".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::Medium,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager.create_change_request(
            "高风险".to_string(),
            "描述".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Normal,
            RiskLevel::High,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager.create_change_request(
            "极高风险".to_string(),
            "描述".to_string(),
            ChangeType::Infrastructure,
            ChangeCategory::Emergency,
            RiskLevel::Critical,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        let stats = manager.get_statistics();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.draft, 4);
        assert_eq!(stats.low_risk, 1);
        assert_eq!(stats.medium_risk, 1);
        assert_eq!(stats.high_risk, 1);
        assert_eq!(stats.critical_risk, 1);
    }

    #[test]
    fn test_capa_audit_entry() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .trigger_capa(
                &change.id,
                "inspector",
                "偏差".to_string(),
                "描述".to_string(),
                None,
                None,
            )
            .unwrap();

        let audit = manager.get_audit_log(&change.id);
        // Should have 2 entries: Created + CAPA triggered
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[1].action, AuditAction::CapaTriggered);
    }

    #[test]
    fn test_multiple_capa_records() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );

        manager
            .trigger_capa(
                &change.id,
                "user",
                "CAPA1".to_string(),
                "描述1".to_string(),
                None,
                None,
            )
            .unwrap();
        manager
            .trigger_capa(
                &change.id,
                "user",
                "CAPA2".to_string(),
                "描述2".to_string(),
                None,
                None,
            )
            .unwrap();

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.capa_records.len(), 2);
        assert_eq!(change.capa_records[0].title, "CAPA1");
        assert_eq!(change.capa_records[1].title, "CAPA2");
    }

    #[test]
    fn test_list_changes_empty() {
        let manager = ItChangeManager::new();
        assert_eq!(manager.list_changes().len(), 0);
    }

    #[test]
    fn test_get_audit_log_empty() {
        let manager = ItChangeManager::new();
        assert_eq!(manager.get_audit_log("nonexistent").len(), 0);
    }

    #[test]
    fn test_implement_change_not_approved() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        // Draft status — should fail
        let result = manager.implement_change(&change.id, "user", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已批准"));
    }

    #[test]
    fn test_complete_implementation_not_implementing() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let result = manager.complete_implementation(&change.id, "user", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("实施中"));
    }

    #[test]
    fn test_verify_change_not_implementing() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let result = manager.verify_change(&change.id, "user", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已实施"));
    }

    #[test]
    fn test_close_change_wrong_status() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let result = manager.close_change(&change.id, "user", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已验证"));
    }

    #[test]
    fn test_add_approver_wrong_status() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        // Draft status — should fail
        let result = manager.add_approver(
            &change.id,
            "approver".to_string(),
            "审批人".to_string(),
            "QA".to_string(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已提交或审核中"));
    }

    #[test]
    fn test_approve_change_wrong_status() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        // Draft status — should fail
        let result = manager.approve_change(
            &change.id,
            "approver",
            Decision::Approved,
            create_test_signature(SignatureMeaning::Approval),
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("审核中"));
    }

    #[test]
    fn test_emergency_implement_wrong_category() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "测试变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low, // Normal, not Emergency
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager
            .submit_for_review(&change.id, "user", None, None)
            .unwrap();
        let result = manager.emergency_implement(&change.id, "user", "紧急原因", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("紧急变更"));
    }

    #[test]
    fn test_statistics_empty() {
        let manager = ItChangeManager::new();
        let stats = manager.get_statistics();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.draft, 0);
        assert_eq!(stats.submitted, 0);
        assert_eq!(stats.approved, 0);
        assert_eq!(stats.closed, 0);
        assert_eq!(stats.sla_violations, 0);
    }

    #[test]
    fn test_change_number_unique() {
        let mut manager = ItChangeManager::new();
        let c1 = manager.create_change_request(
            "变更1".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        let c2 = manager.create_change_request(
            "变更2".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        assert_ne!(c1.change_number, c2.change_number);
        assert!(c1.change_number.starts_with("CHG-"));
        assert!(c2.change_number.starts_with("CHG-"));
    }

    #[test]
    fn test_submit_nonexistent_change() {
        let mut manager = ItChangeManager::new();
        let result = manager.submit_for_review("nonexistent", "user", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_change_nonexistent() {
        let manager = ItChangeManager::new();
        let result = manager.get_change("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_changes_by_risk_empty() {
        let manager = ItChangeManager::new();
        let changes = manager.list_changes_by_risk(&RiskLevel::High);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_list_changes_by_status_empty() {
        let manager = ItChangeManager::new();
        let changes = manager.list_changes_by_status(&ChangeStatus::Draft);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_full_emergency_lifecycle() {
        let mut manager = ItChangeManager::new();
        let change = manager.create_change_request(
            "紧急修复".to_string(),
            "生产环境紧急修复".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Emergency,
            RiskLevel::Critical,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            create_test_impact_assessment(),
        );
        manager
            .submit_for_review(&change.id, "user", None, None)
            .unwrap();
        manager
            .emergency_implement(&change.id, "user", "生产事故", None, None)
            .unwrap();

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::EmergencyImplemented);
        assert!(change.implemented_at.is_some());
        assert!(change.emergency_approval_deadline.is_some());
    }
}
