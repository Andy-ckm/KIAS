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
                | (Self::Approved, Self::Published)
                | (Self::Published, Self::Archived)
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
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => write!(f, "Create"),
            Self::Update => write!(f, "Update"),
            Self::Delete => write!(f, "Delete"),
        }
    }
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

        Ok(self.requests.get(change_request_id).unwrap())
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

        Ok(self.requests.get(change_request_id).unwrap())
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

        Ok(self.requests.get(change_request_id).unwrap())
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

        Ok(self.requests.get(change_request_id).unwrap())
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

        Ok(self.requests.get(change_request_id).unwrap())
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

        Ok(self.requests.get(change_request_id).unwrap())
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
}
