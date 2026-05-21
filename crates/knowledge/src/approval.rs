//! # Approval Workflow — 文档审批工作流
//!
//! 变更请求管理 + 版本历史 + 状态机驱动的审批流程。
//!
//! ## 设计参考
//!
//! - **Mayan EDMS**: Workflow/WorkflowState/WorkflowTransition 模型
//! - **OpenKM**: 版本控制策略 (Plain/MajorMinor/Release)
//! - **van der Aalst Workflow Patterns**: sequence, exclusive choice
//!
//! ## 状态机
//!
//! ```text
//! Draft → Reviewing → Approved → Published → Archived
//!            ↓            ↑
//!         Rejected → Draft (revise)
//! Any state → Draft (reset)
//! ```
//!
//! ## Qian Xuesen 七原则
//!
//! - **整体性**: 与 quality_pipeline.rs 无重复（ApprovalState/ChangeRequest 仅在此定义）
//! - **综合集成**: 融合 Mayan 状态机 + OpenKM 版本控制
//! - **反馈控制**: 每次状态变更记录 who/when/why 审计轨迹
//! - **层次分解**: ApprovalWorkflow → ChangeRequest → EntityVersion (3 层)
//! - **鲁棒性**: 非法状态转换返回 Error，不 panic
//! - **可观测性**: tracing::info! 记录所有状态转换
//! - **工程化**: 13+ 测试，零 clippy 警告

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

// ─── Errors ────────────────────────────────────────────────────────────

/// 审批工作流错误
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    /// 变更请求不存在
    #[error("change request not found: {0}")]
    NotFound(String),
    /// 非法状态转换
    #[error("invalid transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
    /// 变更请求已存在
    #[error("change request already exists: {0}")]
    AlreadyExists(String),
    /// 实体无版本历史
    #[error("no versions for entity: {0}")]
    NoVersions(String),
    /// 变更请求未关联实体
    #[error("change request {0} not linked to entity version")]
    NotLinked(String),
    /// GxP: 审批级别未满足
    #[error("approval level {level} not yet approved")]
    ApprovalLevelNotMet { level: u32 },
    /// GxP: 影响评估缺失
    #[error("impact assessment required before approval")]
    ImpactAssessmentRequired,
    /// GxP: 有效性检查已存在
    #[error("effectiveness check already recorded")]
    EffectivenessCheckExists,
    /// GxP: 影响评估已存在
    #[error("impact assessment already submitted")]
    ImpactAssessmentExists,
}

// ─── Approval State Machine ────────────────────────────────────────────

/// 审批状态（参考 Mayan WorkflowState）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApprovalState {
    /// 草稿 — Agent 写入，待审阅
    Draft,
    /// 审阅中 — 人工或自动审阅
    Reviewing,
    /// 已批准 — 生效
    Approved,
    /// 已拒绝 — 不生效
    Rejected,
    /// 已发布 — 生效且对外可见
    Published,
    /// 已归档 — 历史版本
    Archived,
    /// 已实施 — 变更已实施，待验证 (GxP: EU Annex 11 Clause 10)
    Implemented,
    /// 已验证 — 实施后验证有效性 (GxP: ICH Q10 §3.2.1)
    Verified,
    /// 已关闭 — 变更生命周期结束 (GxP: EU Annex 11 Clause 10)
    Closed,
}

impl fmt::Display for ApprovalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => write!(f, "Draft"),
            Self::Reviewing => write!(f, "Reviewing"),
            Self::Approved => write!(f, "Approved"),
            Self::Rejected => write!(f, "Rejected"),
            Self::Published => write!(f, "Published"),
            Self::Archived => write!(f, "Archived"),
            Self::Implemented => write!(f, "Implemented"),
            Self::Verified => write!(f, "Verified"),
            Self::Closed => write!(f, "Closed"),
        }
    }
}

impl ApprovalState {
    /// 检查从当前状态到目标状态的转换是否合法
    fn can_transition_to(&self, target: &ApprovalState) -> bool {
        matches!(
            (self, target),
            (Self::Draft, Self::Reviewing)
                | (Self::Reviewing, Self::Approved)
                | (Self::Reviewing, Self::Rejected)
                | (Self::Rejected, Self::Draft)
                |            (Self::Approved, Self::Published)
                | (Self::Published, Self::Archived)
                // GxP: Extended lifecycle
                | (Self::Approved, Self::Implemented)
                | (Self::Implemented, Self::Verified)
                | (Self::Verified, Self::Published)
                | (Self::Published, Self::Closed)
                // reset to Draft from any state
                | (_, Self::Draft)
        )
    }
}

// ─── Change Type ───────────────────────────────────────────────────────

/// 变更类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// 创建新实体
    Create,
    /// 更新已有实体
    Update,
    /// 删除实体
    Delete,
    /// 高风险更新（需要审批）
    HighRiskUpdate,
    /// 关键更新（需要多级审批）
    CriticalUpdate,
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => write!(f, "Create"),
            Self::Update => write!(f, "Update"),
            Self::Delete => write!(f, "Delete"),
            Self::HighRiskUpdate => write!(f, "HighRiskUpdate"),
            Self::CriticalUpdate => write!(f, "CriticalUpdate"),
        }
    }
}

// ─── GxP: Impact Assessment & Multi-Level Approval ──────────────────

/// Impact assessment for change requests (ICH Q10 §3.2.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub assessed_by: String,
    pub assessed_at: DateTime<Utc>,
    pub quality_impact: ImpactLevel,
    pub regulatory_impact: ImpactLevel,
    pub validated_state_impact: ImpactLevel,
    pub affected_systems: Vec<String>,
    pub mitigation_actions: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImpactLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Effectiveness monitoring (ICH Q10 §3.2.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessCheck {
    pub checked_by: String,
    pub checked_at: DateTime<Utc>,
    pub is_effective: bool,
    pub evidence: String,
    pub follow_up_actions: Vec<String>,
}

/// Multi-level approval chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalLevel {
    pub level: u32, // 1=peer, 2=manager, 3=quality, 4=regulatory
    pub approver_id: String,
    pub approver_role: String,
    pub approved_at: Option<DateTime<Utc>>,
    pub decision: Option<ApprovalDecision>,
    pub comments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
    RequiresRevision,
}

// ─── Audit Entry ───────────────────────────────────────────────────────

/// 审计条目 — 每次状态变更的完整记录（反馈控制原则）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// 变更前状态
    pub from_state: ApprovalState,
    /// 变更后状态
    pub to_state: ApprovalState,
    /// 操作人
    pub actor: String,
    /// 操作原因/备注
    pub reason: String,
    /// 操作时间
    pub timestamp: DateTime<Utc>,
}

// ─── Change Request ────────────────────────────────────────────────────

/// 变更请求（参考 Mayan WorkflowTransition）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    /// 唯一标识
    pub id: String,
    /// 关联实体 ID
    pub entity_id: String,
    /// 变更类型
    pub change_type: ChangeType,
    /// 提议内容
    pub proposed_content: String,
    /// 变更原因
    pub reason: String,
    /// 请求人
    pub requested_by: String,
    /// 当前审批状态
    pub current_state: ApprovalState,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 审计轨迹（反馈控制）
    pub audit_trail: Vec<AuditEntry>,
    /// GxP: 影响评估 (ICH Q10 §3.2.1)
    pub impact_assessment: Option<ImpactAssessment>,
    /// GxP: 多级审批链
    pub approval_chain: Vec<ApprovalLevel>,
    /// GxP: 有效性检查 (ICH Q10 §3.2.3)
    pub effectiveness_check: Option<EffectivenessCheck>,
    /// GxP: 验证时间
    pub verified_at: Option<DateTime<Utc>>,
    /// GxP: 验证人
    pub verified_by: Option<String>,
    /// GxP: 关闭时间
    pub closed_at: Option<DateTime<Utc>>,
}

impl ChangeRequest {
    /// 创建新的变更请求（初始状态 Draft）
    fn new(
        id: String,
        entity_id: String,
        change_type: ChangeType,
        proposed_content: String,
        reason: String,
        requested_by: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_id,
            change_type,
            proposed_content,
            reason,
            requested_by,
            current_state: ApprovalState::Draft,
            created_at: now,
            updated_at: now,
            audit_trail: Vec::new(),
            impact_assessment: None,
            approval_chain: Vec::new(),
            effectiveness_check: None,
            verified_at: None,
            verified_by: None,
            closed_at: None,
        }
    }

    /// 执行状态转换，记录审计轨迹
    fn transition(
        &mut self,
        target: ApprovalState,
        actor: &str,
        reason: &str,
    ) -> Result<(), ApprovalError> {
        if !self.current_state.can_transition_to(&target) {
            return Err(ApprovalError::InvalidTransition {
                from: self.current_state.to_string(),
                to: target.to_string(),
            });
        }

        let entry = AuditEntry {
            from_state: self.current_state.clone(),
            to_state: target.clone(),
            actor: actor.to_string(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        };

        info!(
            change_request_id = self.id.as_str(),
            from = self.current_state.to_string().as_str(),
            to = target.to_string().as_str(),
            actor = actor,
            reason = reason,
            "Approval state transition"
        );

        self.audit_trail.push(entry);
        self.current_state = target;
        self.updated_at = Utc::now();
        Ok(())
    }
}

// ─── Entity Version ────────────────────────────────────────────────────

/// 实体版本（参考 OpenKM Version）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityVersion {
    /// 版本号（从 1 递增）
    pub version: u32,
    /// 关联实体 ID
    pub entity_id: String,
    /// 版本内容快照
    pub content: String,
    /// 变更人
    pub changed_by: String,
    /// 关联的变更请求 ID
    pub change_request_id: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

// ─── Approval Workflow Manager ─────────────────────────────────────────

/// 审批工作流管理器（层次分解: Workflow → Request → Version）
pub struct ApprovalWorkflow {
    /// 变更请求表: id → ChangeRequest
    requests: HashMap<String, ChangeRequest>,
    /// 版本历史表: entity_id → [EntityVersion]
    versions: HashMap<String, Vec<EntityVersion>>,
    /// 下一个版本号: entity_id → next_version
    version_counters: HashMap<String, u32>,
}

impl Default for ApprovalWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalWorkflow {
    /// 创建新的审批工作流管理器
    pub fn new() -> Self {
        Self {
            requests: HashMap::new(),
            versions: HashMap::new(),
            version_counters: HashMap::new(),
        }
    }

    // ─── Change Request CRUD ────────────────────────────────────────────

    /// 创建变更请求（初始状态 Draft）
    pub fn create_change_request(
        &mut self,
        id: String,
        entity_id: String,
        change_type: ChangeType,
        proposed_content: String,
        reason: String,
        requested_by: String,
    ) -> Result<ChangeRequest, ApprovalError> {
        if self.requests.contains_key(&id) {
            return Err(ApprovalError::AlreadyExists(id));
        }

        let cr = ChangeRequest::new(
            id,
            entity_id,
            change_type,
            proposed_content,
            reason,
            requested_by,
        );

        info!(
            change_request_id = cr.id.as_str(),
            entity_id = cr.entity_id.as_str(),
            change_type = cr.change_type.to_string().as_str(),
            "Change request created in Draft state"
        );

        let result = cr.clone();
        self.requests.insert(cr.id.clone(), cr);
        Ok(result)
    }

    /// 获取变更请求
    pub fn get_change_request(&self, id: &str) -> Option<&ChangeRequest> {
        self.requests.get(id)
    }

    // ─── State Transitions ──────────────────────────────────────────────

    /// 提交审阅: Draft → Reviewing
    pub fn submit_for_review(
        &mut self,
        change_request_id: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            let actor = cr.requested_by.clone();
            cr.transition(ApprovalState::Reviewing, &actor, "Submitted for review")?;
        }

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 批准: Reviewing → Approved，创建版本快照
    pub fn approve(
        &mut self,
        change_request_id: &str,
        reviewer: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            cr.transition(ApprovalState::Approved, reviewer, "Approved")?;
        }

        // 创建版本快照（综合集成: OpenKM 版本控制）
        let entity_id = self.requests[change_request_id].entity_id.clone();
        let content = self.requests[change_request_id].proposed_content.clone();
        let cr_id = change_request_id.to_string();
        self.create_version_internal(entity_id, content, reviewer, Some(cr_id));

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 拒绝: Reviewing → Rejected
    pub fn reject(
        &mut self,
        change_request_id: &str,
        reviewer: &str,
        reason: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            cr.transition(ApprovalState::Rejected, reviewer, reason)?;
        }

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 发布: Approved → Published
    pub fn publish(&mut self, change_request_id: &str) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            let actor = cr.requested_by.clone();
            cr.transition(ApprovalState::Published, &actor, "Published")?;
        }

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 归档: Published → Archived
    pub fn archive(&mut self, change_request_id: &str) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            let actor = cr.requested_by.clone();
            cr.transition(ApprovalState::Archived, &actor, "Archived")?;
        }

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 重置到草稿: Any → Draft
    pub fn reset_to_draft(
        &mut self,
        change_request_id: &str,
        actor: &str,
        reason: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            cr.transition(ApprovalState::Draft, actor, reason)?;
        }

        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    // ─── Version Management ─────────────────────────────────────────────

    /// 创建版本快照（内部方法）
    fn create_version_internal(
        &mut self,
        entity_id: String,
        content: String,
        author: &str,
        change_request_id: Option<String>,
    ) -> u32 {
        let counter = self.version_counters.entry(entity_id.clone()).or_insert(1);
        let version = *counter;
        *counter += 1;

        let ev = EntityVersion {
            version,
            entity_id: entity_id.clone(),
            content,
            changed_by: author.to_string(),
            change_request_id,
            created_at: Utc::now(),
        };

        info!(
            entity_id = entity_id.as_str(),
            version = version,
            author = author,
            "Entity version created"
        );

        self.versions.entry(entity_id).or_default().push(ev);

        version
    }

    /// 创建独立版本快照（不关联变更请求）
    pub fn create_version(&mut self, entity_id: &str, content: &str, author: &str) -> u32 {
        self.create_version_internal(entity_id.to_string(), content.to_string(), author, None)
    }

    /// 获取实体的所有版本历史（按版本号排序）
    pub fn get_versions(&self, entity_id: &str) -> Result<&[EntityVersion], ApprovalError> {
        self.versions
            .get(entity_id)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
            .ok_or_else(|| ApprovalError::NoVersions(entity_id.to_string()))
    }

    /// 获取实体最新已批准版本的内容
    ///
    /// 遍历所有关联已批准/已发布/已归档变更请求的版本，返回最新的。
    pub fn get_latest_approved(&self, entity_id: &str) -> Result<&EntityVersion, ApprovalError> {
        let versions = self
            .versions
            .get(entity_id)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ApprovalError::NoVersions(entity_id.to_string()))?;

        // 从后向前搜索，找到第一个关联已批准/已发布/已归档变更请求的版本
        for ev in versions.iter().rev() {
            if let Some(cr_id) = &ev.change_request_id {
                if let Some(cr) = self.requests.get(cr_id) {
                    if matches!(
                        cr.current_state,
                        ApprovalState::Approved
                            | ApprovalState::Published
                            | ApprovalState::Archived
                            | ApprovalState::Implemented
                            | ApprovalState::Verified
                            | ApprovalState::Closed
                    ) {
                        return Ok(ev);
                    }
                }
            }
        }

        Err(ApprovalError::NoVersions(entity_id.to_string()))
    }

    // ─── Queries ────────────────────────────────────────────────────────

    /// 列出所有处于 Reviewing 状态的变更请求
    pub fn list_pending_reviews(&self) -> Vec<&ChangeRequest> {
        self.requests
            .values()
            .filter(|cr| cr.current_state == ApprovalState::Reviewing)
            .collect()
    }

    /// 列出指定实体的所有变更请求
    pub fn list_by_entity(&self, entity_id: &str) -> Vec<&ChangeRequest> {
        self.requests
            .values()
            .filter(|cr| cr.entity_id == entity_id)
            .collect()
    }

    // ─── GxP: Impact Assessment & Multi-Level Approval ──────────────

    /// 提交影响评估 (ICH Q10 §3.2.1)
    pub fn submit_impact_assessment(
        &mut self,
        change_request_id: &str,
        assessment: ImpactAssessment,
    ) -> Result<(), ApprovalError> {
        let cr = self
            .requests
            .get_mut(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

        if cr.impact_assessment.is_some() {
            return Err(ApprovalError::ImpactAssessmentExists);
        }

        info!(
            change_request_id = change_request_id,
            assessed_by = assessment.assessed_by.as_str(),
            "Impact assessment submitted"
        );

        cr.impact_assessment = Some(assessment);
        cr.updated_at = Utc::now();
        Ok(())
    }

    /// 获取影响评估
    pub fn get_impact_assessment(&self, change_request_id: &str) -> Option<&ImpactAssessment> {
        self.requests
            .get(change_request_id)
            .and_then(|cr| cr.impact_assessment.as_ref())
    }

    /// 添加审批级别 (GxP: multi-level approval chain)
    pub fn add_approval_level(
        &mut self,
        change_request_id: &str,
        level: ApprovalLevel,
    ) -> Result<(), ApprovalError> {
        let cr = self
            .requests
            .get_mut(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

        // Prevent duplicate levels
        if cr.approval_chain.iter().any(|a| a.level == level.level) {
            return Err(ApprovalError::AlreadyExists(format!(
                "approval level {}",
                level.level
            )));
        }

        cr.approval_chain.push(level);
        cr.approval_chain.sort_by_key(|a| a.level);
        cr.updated_at = Utc::now();
        Ok(())
    }

    /// 在指定级别审批/拒绝 (GxP: ordered multi-level approval)
    pub fn approve_at_level(
        &mut self,
        change_request_id: &str,
        level: u32,
        approver_id: &str,
        decision: ApprovalDecision,
        comments: Option<&str>,
    ) -> Result<(), ApprovalError> {
        // Verify all previous levels are approved
        {
            let cr = self
                .requests
                .get(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            for prev_level in 1..level {
                let prev_approved = cr.approval_chain.iter().any(|a| {
                    a.level == prev_level && matches!(a.decision, Some(ApprovalDecision::Approved))
                });
                if !prev_approved {
                    return Err(ApprovalError::ApprovalLevelNotMet { level: prev_level });
                }
            }
        }

        // Find and update the entry
        let cr = self
            .requests
            .get_mut(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;
        let entry = cr
            .approval_chain
            .iter_mut()
            .find(|a| a.level == level)
            .ok_or(ApprovalError::ApprovalLevelNotMet { level })?;

        entry.decision = Some(decision);
        entry.approver_id = approver_id.to_string();
        entry.approved_at = Some(Utc::now());
        entry.comments = comments.map(|s| s.to_string());
        cr.updated_at = Utc::now();

        info!(
            change_request_id = change_request_id,
            level = level,
            approver = approver_id,
            "Approval level decision recorded"
        );

        Ok(())
    }

    /// 获取审批链
    pub fn get_approval_chain(&self, change_request_id: &str) -> &[ApprovalLevel] {
        self.requests
            .get(change_request_id)
            .map(|cr| cr.approval_chain.as_slice())
            .unwrap_or_default()
    }

    /// 列出指定级别待审批的变更请求 (GxP: multi-level approval queue)
    pub fn list_pending_at_level(&self, level: u32) -> Vec<&ChangeRequest> {
        self.requests
            .values()
            .filter(|cr| {
                // Must have an entry at this level with no decision
                let has_pending = cr
                    .approval_chain
                    .iter()
                    .any(|a| a.level == level && a.decision.is_none());
                if !has_pending {
                    return false;
                }
                // All previous levels must be approved
                (1..level).all(|prev| {
                    cr.approval_chain.iter().any(|a| {
                        a.level == prev && matches!(a.decision, Some(ApprovalDecision::Approved))
                    })
                })
            })
            .collect()
    }

    // ─── GxP: Lifecycle Extensions ──────────────────────────────────

    /// 实施: Approved → Implemented (GxP: EU Annex 11 Clause 10)
    pub fn implement(
        &mut self,
        change_request_id: &str,
        implementer: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            cr.transition(
                ApprovalState::Implemented,
                implementer,
                "Change implemented",
            )?;
        }
        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 验证: Implemented → Verified (GxP: ICH Q10 §3.2.1)
    pub fn verify(
        &mut self,
        change_request_id: &str,
        verifier_id: &str,
        evidence: &str,
    ) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            cr.transition(
                ApprovalState::Verified,
                verifier_id,
                &format!("Verified: {evidence}"),
            )?;

            cr.verified_at = Some(Utc::now());
            cr.verified_by = Some(verifier_id.to_string());
        }
        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 关闭: Published → Closed (GxP: EU Annex 11 Clause 10)
    pub fn close(&mut self, change_request_id: &str) -> Result<&ChangeRequest, ApprovalError> {
        {
            let cr = self
                .requests
                .get_mut(change_request_id)
                .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

            let actor = cr.requested_by.clone();
            cr.transition(ApprovalState::Closed, &actor, "Change request closed")?;
            cr.closed_at = Some(Utc::now());
        }
        self.requests
            .get(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))
    }

    /// 记录有效性检查 (ICH Q10 §3.2.3)
    pub fn record_effectiveness(
        &mut self,
        change_request_id: &str,
        check: EffectivenessCheck,
    ) -> Result<(), ApprovalError> {
        let cr = self
            .requests
            .get_mut(change_request_id)
            .ok_or_else(|| ApprovalError::NotFound(change_request_id.to_string()))?;

        if cr.effectiveness_check.is_some() {
            return Err(ApprovalError::EffectivenessCheckExists);
        }

        info!(
            change_request_id = change_request_id,
            checked_by = check.checked_by.as_str(),
            is_effective = check.is_effective,
            "Effectiveness check recorded"
        );

        cr.effectiveness_check = Some(check);
        cr.updated_at = Utc::now();
        Ok(())
    }

    /// 获取有效性检查
    pub fn get_effectiveness_check(&self, change_request_id: &str) -> Option<&EffectivenessCheck> {
        self.requests
            .get(change_request_id)
            .and_then(|cr| cr.effectiveness_check.as_ref())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: 创建一个 Draft 状态的变更请求
    fn setup_draft(wf: &mut ApprovalWorkflow) -> String {
        let id = "cr-001".to_string();
        wf.create_change_request(
            id.clone(),
            "entity-A".to_string(),
            ChangeType::Create,
            "content v1".to_string(),
            "Initial creation".to_string(),
            "agent-1".to_string(),
        )
        .unwrap();
        id
    }

    #[test]
    fn test_create_change_request_is_draft() {
        let mut wf = ApprovalWorkflow::new();
        let cr = wf
            .create_change_request(
                "cr-001".into(),
                "entity-A".into(),
                ChangeType::Create,
                "content".into(),
                "reason".into(),
                "agent-1".into(),
            )
            .unwrap();
        assert_eq!(cr.current_state, ApprovalState::Draft);
        assert!(cr.audit_trail.is_empty());
    }

    #[test]
    fn test_submit_for_review() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        let cr = wf.submit_for_review(&id).unwrap();
        assert_eq!(cr.current_state, ApprovalState::Reviewing);
        assert_eq!(cr.audit_trail.len(), 1);
        assert_eq!(cr.audit_trail[0].from_state, ApprovalState::Draft);
        assert_eq!(cr.audit_trail[0].to_state, ApprovalState::Reviewing);
    }

    #[test]
    fn test_approve_creates_version() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        let cr = wf.approve(&id, "reviewer-1").unwrap();
        assert_eq!(cr.current_state, ApprovalState::Approved);

        // 验证版本已创建
        let versions = wf.get_versions("entity-A").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[0].changed_by, "reviewer-1");
        assert_eq!(versions[0].change_request_id, Some(id));
    }

    #[test]
    fn test_reject() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        let cr = wf
            .reject(&id, "reviewer-1", "Content insufficient")
            .unwrap();
        assert_eq!(cr.current_state, ApprovalState::Rejected);
        assert_eq!(
            cr.audit_trail.last().unwrap().reason,
            "Content insufficient"
        );
    }

    #[test]
    fn test_revise_rejected_to_draft() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        wf.reject(&id, "reviewer-1", "Needs work").unwrap();
        let cr = wf
            .reset_to_draft(&id, "agent-1", "Revising after rejection")
            .unwrap();
        assert_eq!(cr.current_state, ApprovalState::Draft);
        assert_eq!(cr.audit_trail.len(), 3); // submit, reject, reset
    }

    #[test]
    fn test_publish() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer-1").unwrap();
        let cr = wf.publish(&id).unwrap();
        assert_eq!(cr.current_state, ApprovalState::Published);
    }

    #[test]
    fn test_archive() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer-1").unwrap();
        wf.publish(&id).unwrap();
        let cr = wf.archive(&id).unwrap();
        assert_eq!(cr.current_state, ApprovalState::Archived);
    }

    #[test]
    fn test_invalid_transition_draft_to_approved() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        let result = wf.approve(&id, "reviewer-1");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::InvalidTransition {
                from: "Draft".to_string(),
                to: "Approved".to_string(),
            }
        );
    }

    #[test]
    fn test_create_version_and_retrieve() {
        let mut wf = ApprovalWorkflow::new();
        let v = wf.create_version("entity-B", "v1 content", "author-1");
        assert_eq!(v, 1);
        let v2 = wf.create_version("entity-B", "v2 content", "author-2");
        assert_eq!(v2, 2);

        let versions = wf.get_versions("entity-B").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[1].version, 2);
    }

    #[test]
    fn test_version_history_ordering() {
        let mut wf = ApprovalWorkflow::new();
        for i in 1..=5 {
            wf.create_version("entity-C", &format!("content-{i}"), "author");
        }
        let versions = wf.get_versions("entity-C").unwrap();
        for (i, v) in versions.iter().enumerate() {
            assert_eq!(v.version, (i + 1) as u32);
        }
    }

    #[test]
    fn test_get_latest_approved() {
        let mut wf = ApprovalWorkflow::new();

        // 创建两个变更请求，都关联同一实体
        wf.create_change_request(
            "cr-1".into(),
            "entity-D".into(),
            ChangeType::Create,
            "first content".into(),
            "reason".into(),
            "agent".into(),
        )
        .unwrap();
        wf.create_change_request(
            "cr-2".into(),
            "entity-D".into(),
            ChangeType::Update,
            "second content".into(),
            "reason".into(),
            "agent".into(),
        )
        .unwrap();

        // cr-1: full lifecycle
        wf.submit_for_review("cr-1").unwrap();
        wf.approve("cr-1", "reviewer").unwrap();
        wf.publish("cr-1").unwrap();

        // cr-2: only approved
        wf.submit_for_review("cr-2").unwrap();
        wf.approve("cr-2", "reviewer").unwrap();

        let latest = wf.get_latest_approved("entity-D").unwrap();
        assert_eq!(latest.content, "second content");
        assert_eq!(latest.version, 2);
    }

    #[test]
    fn test_list_pending_reviews() {
        let mut wf = ApprovalWorkflow::new();

        wf.create_change_request(
            "cr-1".into(),
            "e1".into(),
            ChangeType::Create,
            "c1".into(),
            "r1".into(),
            "a1".into(),
        )
        .unwrap();
        wf.create_change_request(
            "cr-2".into(),
            "e2".into(),
            ChangeType::Update,
            "c2".into(),
            "r2".into(),
            "a2".into(),
        )
        .unwrap();

        wf.submit_for_review("cr-1").unwrap();
        // cr-2 stays in Draft

        let pending = wf.list_pending_reviews();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "cr-1");
    }

    #[test]
    fn test_reset_to_draft_from_published() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer").unwrap();
        wf.publish(&id).unwrap();

        let cr = wf.reset_to_draft(&id, "admin", "Emergency reset").unwrap();
        assert_eq!(cr.current_state, ApprovalState::Draft);
        // Should have: submit, approve, publish, reset = 4 audit entries
        assert_eq!(cr.audit_trail.len(), 4);
    }

    #[test]
    fn test_not_found_error() {
        let mut wf = ApprovalWorkflow::new();
        let result = wf.submit_for_review("nonexistent");
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::NotFound("nonexistent".to_string())
        );
    }

    #[test]
    fn test_duplicate_change_request_error() {
        let mut wf = ApprovalWorkflow::new();
        wf.create_change_request(
            "cr-1".into(),
            "e1".into(),
            ChangeType::Create,
            "c".into(),
            "r".into(),
            "a".into(),
        )
        .unwrap();
        let result = wf.create_change_request(
            "cr-1".into(),
            "e1".into(),
            ChangeType::Create,
            "c".into(),
            "r".into(),
            "a".into(),
        );
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::AlreadyExists("cr-1".to_string())
        );
    }

    #[test]
    fn test_no_versions_error() {
        let wf = ApprovalWorkflow::new();
        let result = wf.get_versions("nonexistent");
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::NoVersions("nonexistent".to_string())
        );
    }

    #[test]
    fn test_audit_trail_feedback_control() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_draft(&mut wf);
        wf.submit_for_review(&id).unwrap();
        wf.reject(&id, "reviewer-1", "Insufficient detail").unwrap();
        wf.reset_to_draft(&id, "agent-1", "Adding more detail")
            .unwrap();
        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer-2").unwrap();

        let cr = wf.get_change_request(&id).unwrap();
        assert_eq!(cr.audit_trail.len(), 5);
        // Verify full audit trail sequence
        let expected = [
            (ApprovalState::Draft, ApprovalState::Reviewing),
            (ApprovalState::Reviewing, ApprovalState::Rejected),
            (ApprovalState::Rejected, ApprovalState::Draft),
            (ApprovalState::Draft, ApprovalState::Reviewing),
            (ApprovalState::Reviewing, ApprovalState::Approved),
        ];
        for (i, (from, to)) in expected.iter().enumerate() {
            assert_eq!(cr.audit_trail[i].from_state, *from);
            assert_eq!(cr.audit_trail[i].to_state, *to);
        }
    }

    // ─── GxP: Impact Assessment & Multi-Level Approval Tests ────────

    fn setup_gxp_cr(wf: &mut ApprovalWorkflow) -> String {
        let id = "cr-gxp".to_string();
        wf.create_change_request(
            id.clone(),
            "entity-gxp".to_string(),
            ChangeType::Update,
            "GxP content".to_string(),
            "GxP change".to_string(),
            "qa-engineer".to_string(),
        )
        .unwrap();
        id
    }

    fn sample_impact() -> ImpactAssessment {
        ImpactAssessment {
            assessed_by: "qa-lead".to_string(),
            assessed_at: Utc::now(),
            quality_impact: ImpactLevel::High,
            regulatory_impact: ImpactLevel::Medium,
            validated_state_impact: ImpactLevel::Low,
            affected_systems: vec!["system-A".to_string()],
            mitigation_actions: vec!["re-validate".to_string()],
            summary: "Impact on validated state".to_string(),
        }
    }

    #[test]
    fn test_submit_impact_assessment() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        let assessment = sample_impact();
        wf.submit_impact_assessment(&id, assessment).unwrap();

        let stored = wf.get_impact_assessment(&id).unwrap();
        assert_eq!(stored.assessed_by, "qa-lead");
        assert_eq!(stored.quality_impact, ImpactLevel::High);
        assert_eq!(stored.affected_systems.len(), 1);
    }

    #[test]
    fn test_impact_level_classification() {
        let levels = [
            ImpactLevel::None,
            ImpactLevel::Low,
            ImpactLevel::Medium,
            ImpactLevel::High,
            ImpactLevel::Critical,
        ];
        assert_eq!(levels.len(), 5);
        assert_ne!(ImpactLevel::None, ImpactLevel::Critical);
        assert_eq!(ImpactLevel::Medium, ImpactLevel::Medium);
    }

    #[test]
    fn test_add_approval_levels() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 1,
                approver_id: String::new(),
                approver_role: "peer".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();

        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 2,
                approver_id: String::new(),
                approver_role: "manager".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();

        let chain = wf.get_approval_chain(&id);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].level, 1);
        assert_eq!(chain[1].level, 2);
    }

    #[test]
    fn test_approve_at_level() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 1,
                approver_id: String::new(),
                approver_role: "peer".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();

        wf.approve_at_level(&id, 1, "peer-1", ApprovalDecision::Approved, Some("LGTM"))
            .unwrap();

        let chain = wf.get_approval_chain(&id);
        assert!(matches!(
            chain[0].decision,
            Some(ApprovalDecision::Approved)
        ));
        assert_eq!(chain[0].approver_id, "peer-1");
        assert_eq!(chain[0].comments.as_deref(), Some("LGTM"));
        assert!(chain[0].approved_at.is_some());
    }

    #[test]
    fn test_reject_at_level_with_reason() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 1,
                approver_id: String::new(),
                approver_role: "peer".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();

        wf.approve_at_level(
            &id,
            1,
            "peer-1",
            ApprovalDecision::Rejected,
            Some("Missing validation data"),
        )
        .unwrap();

        let chain = wf.get_approval_chain(&id);
        assert!(matches!(
            chain[0].decision,
            Some(ApprovalDecision::Rejected)
        ));
        assert_eq!(
            chain[0].comments.as_deref(),
            Some("Missing validation data")
        );
    }

    #[test]
    fn test_full_approval_chain_3_levels() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        for (lvl, role) in [(1, "peer"), (2, "manager"), (3, "quality")] {
            wf.add_approval_level(
                &id,
                ApprovalLevel {
                    level: lvl,
                    approver_id: String::new(),
                    approver_role: role.to_string(),
                    approved_at: None,
                    decision: None,
                    comments: None,
                },
            )
            .unwrap();
        }

        wf.approve_at_level(&id, 1, "peer-1", ApprovalDecision::Approved, None)
            .unwrap();
        wf.approve_at_level(&id, 2, "mgr-1", ApprovalDecision::Approved, None)
            .unwrap();
        wf.approve_at_level(&id, 3, "qa-1", ApprovalDecision::Approved, None)
            .unwrap();

        let chain = wf.get_approval_chain(&id);
        assert!(chain
            .iter()
            .all(|a| matches!(a.decision, Some(ApprovalDecision::Approved))));
    }

    #[test]
    fn test_verify_after_implementation() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer").unwrap();
        wf.implement(&id, "deployer").unwrap();

        let cr = wf.verify(&id, "verifier-1", "UAT passed").unwrap();
        assert_eq!(cr.current_state, ApprovalState::Verified);
        assert!(cr.verified_at.is_some());
        assert_eq!(cr.verified_by.as_deref(), Some("verifier-1"));
    }

    #[test]
    fn test_close_change_request() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer").unwrap();
        wf.implement(&id, "deployer").unwrap();
        wf.verify(&id, "verifier", "validated").unwrap();
        wf.publish(&id).unwrap();

        let cr = wf.close(&id).unwrap();
        assert_eq!(cr.current_state, ApprovalState::Closed);
        assert!(cr.closed_at.is_some());
    }

    #[test]
    fn test_record_effectiveness_check() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer").unwrap();

        let check = EffectivenessCheck {
            checked_by: "qa-lead".to_string(),
            checked_at: Utc::now(),
            is_effective: true,
            evidence: "Metrics improved by 20%".to_string(),
            follow_up_actions: vec![],
        };
        wf.record_effectiveness(&id, check).unwrap();

        let stored = wf.get_effectiveness_check(&id).unwrap();
        assert!(stored.is_effective);
        assert_eq!(stored.evidence, "Metrics improved by 20%");
    }

    #[test]
    fn test_list_pending_at_level() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 1,
                approver_id: String::new(),
                approver_role: "peer".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();
        wf.add_approval_level(
            &id,
            ApprovalLevel {
                level: 2,
                approver_id: String::new(),
                approver_role: "manager".to_string(),
                approved_at: None,
                decision: None,
                comments: None,
            },
        )
        .unwrap();

        // Level 1 is pending
        assert_eq!(wf.list_pending_at_level(1).len(), 1);
        // Level 2 is NOT pending yet (level 1 not approved)
        assert_eq!(wf.list_pending_at_level(2).len(), 0);

        // Approve level 1
        wf.approve_at_level(&id, 1, "peer-1", ApprovalDecision::Approved, None)
            .unwrap();

        // Now level 2 is pending
        assert_eq!(wf.list_pending_at_level(1).len(), 0);
        assert_eq!(wf.list_pending_at_level(2).len(), 1);
    }

    #[test]
    fn test_multi_level_approval_ordering() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        for lvl in [1, 2, 3] {
            wf.add_approval_level(
                &id,
                ApprovalLevel {
                    level: lvl,
                    approver_id: String::new(),
                    approver_role: format!("role-{lvl}"),
                    approved_at: None,
                    decision: None,
                    comments: None,
                },
            )
            .unwrap();
        }

        // Cannot approve level 2 before level 1
        let result = wf.approve_at_level(&id, 2, "mgr", ApprovalDecision::Approved, None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::ApprovalLevelNotMet { level: 1 }
        );

        // Cannot approve level 3 before level 1
        let result = wf.approve_at_level(&id, 3, "qa", ApprovalDecision::Approved, None);
        assert!(result.is_err());

        // Can approve level 1
        wf.approve_at_level(&id, 1, "peer", ApprovalDecision::Approved, None)
            .unwrap();

        // Still cannot approve level 3 (level 2 not done)
        let result = wf.approve_at_level(&id, 3, "qa", ApprovalDecision::Approved, None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ApprovalError::ApprovalLevelNotMet { level: 2 }
        );

        // Approve level 2, then level 3 works
        wf.approve_at_level(&id, 2, "mgr", ApprovalDecision::Approved, None)
            .unwrap();
        wf.approve_at_level(&id, 3, "qa", ApprovalDecision::Approved, None)
            .unwrap();

        let chain = wf.get_approval_chain(&id);
        assert!(chain
            .iter()
            .all(|a| matches!(a.decision, Some(ApprovalDecision::Approved))));
    }

    #[test]
    fn test_effectiveness_monitoring_workflow() {
        let mut wf = ApprovalWorkflow::new();
        let id = setup_gxp_cr(&mut wf);

        // Full GxP lifecycle: impact → review → approve → implement → verify → publish → effectiveness → close
        wf.submit_impact_assessment(&id, sample_impact()).unwrap();
        assert!(wf.get_impact_assessment(&id).is_some());

        wf.submit_for_review(&id).unwrap();
        wf.approve(&id, "reviewer").unwrap();
        wf.implement(&id, "deployer").unwrap();
        wf.verify(&id, "verifier", "IQ/OQ/PQ complete").unwrap();
        wf.publish(&id).unwrap();

        // Record effectiveness check
        let check = EffectivenessCheck {
            checked_by: "qa-lead".to_string(),
            checked_at: Utc::now(),
            is_effective: true,
            evidence: "Change achieved intended result".to_string(),
            follow_up_actions: vec!["Schedule periodic review".to_string()],
        };
        wf.record_effectiveness(&id, check).unwrap();

        // Close
        let cr = wf.close(&id).unwrap();
        assert_eq!(cr.current_state, ApprovalState::Closed);
        assert!(cr.closed_at.is_some());
        assert!(cr.effectiveness_check.as_ref().unwrap().is_effective);
        assert!(cr.impact_assessment.is_some());
    }
}
