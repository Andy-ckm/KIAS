//! # Side-effect Gating (Dry-Run Harness)
//!
//! 把所有副作用操作拆成 preview + execute 两阶段。
//! preview 阶段不产生真实副作用，生成可审批的预演结果。
//! 高风险操作必须经过人工/自动审批后才能 execute。
//!
//! ## 设计来源
//! all-agentic-architectures #17 Dry-Run Harness
//!
//! ## 与 workflow-engine/approval.rs 的关系
//! - approval.rs: 处理工作流节点审批（知识生命周期）
//! - side_effect_gate: 处理动作审批（执行生命周期）
//! - 共享 ApprovalPolicy 的审批策略

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Execution Mode ────────────────────────────────────────────────────

/// 副作用操作的执行模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 仅预演，不产生真实副作用
    DryRun,
    /// 正式执行，产生真实副作用
    Execute,
}

// ─── Action Type ───────────────────────────────────────────────────────

/// 动作类型分类（决定风险等级）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ActionType {
    /// 写文件（本地）
    FileWrite,
    /// 删文件（本地）
    FileDelete,
    /// 执行 shell 命令
    CommandExec,
    /// 外部 HTTP 请求
    NetworkRequest,
    /// 数据库写操作
    DataMutation,
    /// 发送外部通知（邮件/消息）
    Notification,
    /// 修改配置
    ConfigChange,
    /// Git 操作（push/force-push）
    GitPush,
}

impl ActionType {
    /// 基础风险等级（可被上下文覆盖）
    pub fn base_risk(&self) -> RiskLevel {
        match self {
            Self::FileWrite => RiskLevel::Low,
            Self::FileDelete => RiskLevel::Medium,
            Self::CommandExec => RiskLevel::Medium,
            Self::NetworkRequest => RiskLevel::High,
            Self::DataMutation => RiskLevel::High,
            Self::Notification => RiskLevel::High,
            Self::ConfigChange => RiskLevel::High,
            Self::GitPush => RiskLevel::Medium,
        }
    }
}

// ─── Risk Level ────────────────────────────────────────────────────────

/// 风险等级（决定审批策略）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// 只读操作、本地文件写入
    Low,
    /// 写本地文件、修改配置、git push
    Medium,
    /// 外部请求、数据删除、通知发送
    High,
    /// 生产环境、不可逆操作
    Critical,
}

// ─── Side-effect Action ────────────────────────────────────────────────

/// 一个待执行的副作用操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffectAction {
    /// 唯一 ID
    pub id: Uuid,
    /// 动作类型
    pub action_type: ActionType,
    /// 目标资源（文件路径/URL/表名等）
    pub target: String,
    /// 操作参数
    pub parameters: serde_json::Value,
    /// 执行模式
    pub mode: ExecutionMode,
    /// dry-run 预演结果（仅在 DryRun 模式下填充）
    pub preview_result: Option<ExecutionPreview>,
    /// 正式执行结果
    pub actual_result: Option<ExecutionResult>,
    /// 审批决策
    pub approval: Option<ApprovalDecision>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl SideEffectAction {
    /// 创建新的副作用操作（默认 DryRun 模式）
    pub fn new(action_type: ActionType, target: String, parameters: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            action_type,
            target,
            parameters,
            mode: ExecutionMode::DryRun,
            preview_result: None,
            actual_result: None,
            approval: None,
            created_at: Utc::now(),
        }
    }

    /// 计算综合风险等级
    pub fn risk_level(&self) -> RiskLevel {
        let base = self.action_type.base_risk();
        // target 包含 "production" 或 "prod" 升级为 Critical
        if self.target.contains("production") || self.target.contains("/prod/") {
            return RiskLevel::Critical;
        }
        base
    }
}

// ─── Execution Preview ─────────────────────────────────────────────────

/// dry-run 预演结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreview {
    /// 受影响的资源列表
    pub would_affect: Vec<String>,
    /// 风险等级
    pub risk_level: RiskLevel,
    /// 变更差异（如有）
    pub diff: Option<String>,
    /// 影响描述
    pub estimated_impact: String,
    /// 是否可逆
    pub reversible: bool,
}

// ─── Execution Result ──────────────────────────────────────────────────

/// 正式执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 输出信息
    pub output: String,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

// ─── Approval Decision ─────────────────────────────────────────────────

/// 审批决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// 审批人
    pub approver: String,
    /// 决策
    pub decision: ApprovalOutcome,
    /// 原因
    pub reason: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

/// 审批结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalOutcome {
    /// 批准执行
    Approved,
    /// 拒绝执行
    Rejected,
    /// 修改后执行
    Modified { notes: String },
}

// ─── Gate Policy ───────────────────────────────────────────────────────

/// 闸门策略（决定是否需要审批）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum GatePolicy {
    /// Low 风险自动通过，其余需要审批
    AutoLow,
    /// Low + Medium 自动通过，High/Critical 需要审批
    #[default]
    AutoMedium,
    /// 所有都需要审批
    Always,
    /// 自定义阈值
    Threshold { max_auto_risk: RiskLevel },
}

impl GatePolicy {
    /// 判断是否需要人工审批
    pub fn requires_approval(&self, risk: RiskLevel) -> bool {
        match self {
            Self::AutoLow => risk > RiskLevel::Low,
            Self::AutoMedium => risk > RiskLevel::Medium,
            Self::Always => true,
            Self::Threshold { max_auto_risk } => risk > *max_auto_risk,
        }
    }
}

// ─── Gate Result ───────────────────────────────────────────────────────

/// 闸门处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateResult {
    /// 自动通过，可以直接执行
    AutoApproved {
        action: SideEffectAction,
        preview: ExecutionPreview,
    },
    /// 需要人工审批
    RequiresApproval {
        action: SideEffectAction,
        preview: ExecutionPreview,
        risk: RiskLevel,
    },
    /// 被拒绝（高风险 + 策略为拒绝高风险）
    Rejected {
        action: SideEffectAction,
        reason: String,
    },
}

// ─── Side-effect Gate ──────────────────────────────────────────────────

/// 副作用闸门
pub struct SideEffectGate {
    /// 审批策略
    policy: GatePolicy,
    /// 操作历史（用于审计）
    history: Vec<GateAuditEntry>,
}

/// 审计条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateAuditEntry {
    /// 操作 ID
    pub action_id: Uuid,
    /// 动作类型
    pub action_type: ActionType,
    /// 目标
    pub target: String,
    /// 风险等级
    pub risk_level: RiskLevel,
    /// 处理结果
    pub result_type: String, // "auto_approved" | "requires_approval" | "rejected"
    /// 审批决策（如果有）
    pub approval: Option<ApprovalDecision>,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

impl SideEffectGate {
    /// 创建新的闸门
    pub fn new(policy: GatePolicy) -> Self {
        Self {
            policy,
            history: Vec::new(),
        }
    }

    /// 处理一个副作用操作（dry-run 阶段）
    ///
    /// 1. 计算风险等级
    /// 2. 根据策略决定是否需要审批
    /// 3. 生成预演结果
    /// 4. 记录审计日志
    pub fn process(&mut self, mut action: SideEffectAction) -> GateResult {
        let risk = action.risk_level();

        // 生成预演结果
        let preview = ExecutionPreview {
            would_affect: vec![action.target.clone()],
            risk_level: risk,
            diff: None,
            estimated_impact: format!(
                "{:?} on {} with params {}",
                action.action_type, action.target, action.parameters
            ),
            reversible: matches!(
                action.action_type,
                ActionType::FileWrite | ActionType::ConfigChange
            ),
        };

        action.preview_result = Some(preview.clone());
        action.mode = ExecutionMode::DryRun;

        // 审计记录
        let entry = GateAuditEntry {
            action_id: action.id,
            action_type: action.action_type.clone(),
            target: action.target.clone(),
            risk_level: risk,
            result_type: if self.policy.requires_approval(risk) {
                "requires_approval".to_string()
            } else {
                "auto_approved".to_string()
            },
            approval: None,
            timestamp: Utc::now(),
        };
        self.history.push(entry);

        if self.policy.requires_approval(risk) {
            GateResult::RequiresApproval {
                action,
                preview,
                risk,
            }
        } else {
            GateResult::AutoApproved { action, preview }
        }
    }

    /// 记录审批决策
    pub fn record_approval(&mut self, action_id: Uuid, decision: ApprovalDecision) {
        if let Some(entry) = self.history.iter_mut().find(|e| e.action_id == action_id) {
            entry.approval = Some(decision);
        }
    }

    /// 获取审计历史
    pub fn audit_history(&self) -> &[GateAuditEntry] {
        &self.history
    }

    /// 获取统计
    pub fn stats(&self) -> GateStats {
        let total = self.history.len();
        let auto_approved = self
            .history
            .iter()
            .filter(|e| e.result_type == "auto_approved")
            .count();
        let requires_approval = self
            .history
            .iter()
            .filter(|e| e.result_type == "requires_approval")
            .count();
        let approved = self
            .history
            .iter()
            .filter(|e| {
                e.approval
                    .as_ref()
                    .map(|a| matches!(a.decision, ApprovalOutcome::Approved))
                    .unwrap_or(false)
            })
            .count();
        let rejected = self
            .history
            .iter()
            .filter(|e| {
                e.approval
                    .as_ref()
                    .map(|a| matches!(a.decision, ApprovalOutcome::Rejected))
                    .unwrap_or(false)
            })
            .count();

        GateStats {
            total,
            auto_approved,
            requires_approval,
            approved,
            rejected,
        }
    }
}

/// 闸门统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStats {
    pub total: usize,
    pub auto_approved: usize,
    pub requires_approval: usize,
    pub approved: usize,
    pub rejected: usize,
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_action_type_base_risk() {
        assert_eq!(ActionType::FileWrite.base_risk(), RiskLevel::Low);
        assert_eq!(ActionType::FileDelete.base_risk(), RiskLevel::Medium);
        assert_eq!(ActionType::NetworkRequest.base_risk(), RiskLevel::High);
        assert_eq!(ActionType::DataMutation.base_risk(), RiskLevel::High);
    }

    #[test]
    fn test_production_target_upgrades_risk() {
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/production/config.yaml".to_string(),
            serde_json::json!({}),
        );
        assert_eq!(action.risk_level(), RiskLevel::Critical);
    }

    #[test]
    fn test_gate_auto_low_policy() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoLow);

        // Low risk → auto approved
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        let result = gate.process(action);
        assert!(matches!(result, GateResult::AutoApproved { .. }));

        // High risk → requires approval
        let action = SideEffectAction::new(
            ActionType::NetworkRequest,
            "https://api.example.com".to_string(),
            serde_json::json!({}),
        );
        let result = gate.process(action);
        assert!(matches!(result, GateResult::RequiresApproval { .. }));
    }

    #[test]
    fn test_gate_auto_medium_policy() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoMedium);

        // Low → auto
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::AutoApproved { .. }
        ));

        // Medium → auto
        let action = SideEffectAction::new(
            ActionType::GitPush,
            "origin main".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::AutoApproved { .. }
        ));

        // High → requires approval
        let action = SideEffectAction::new(
            ActionType::Notification,
            "user@example.com".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::RequiresApproval { .. }
        ));
    }

    #[test]
    fn test_gate_always_policy() {
        let mut gate = SideEffectGate::new(GatePolicy::Always);

        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::RequiresApproval { .. }
        ));
    }

    #[test]
    fn test_gate_stats() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoLow);

        gate.process(SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/a".to_string(),
            serde_json::json!({}),
        ));
        gate.process(SideEffectAction::new(
            ActionType::NetworkRequest,
            "https://api.example.com".to_string(),
            serde_json::json!({}),
        ));

        let stats = gate.stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.auto_approved, 1);
        assert_eq!(stats.requires_approval, 1);
    }

    #[test]
    fn test_audit_history_recorded() {
        let mut gate = SideEffectGate::new(GatePolicy::Always);
        let action = SideEffectAction::new(
            ActionType::DataMutation,
            "users_table".to_string(),
            serde_json::json!({"id": 1}),
        );
        let action_id = action.id;
        gate.process(action);

        assert_eq!(gate.audit_history().len(), 1);
        assert_eq!(gate.audit_history()[0].action_id, action_id);
    }

    #[test]
    fn test_gate_threshold_policy() {
        let mut gate = SideEffectGate::new(GatePolicy::Threshold {
            max_auto_risk: RiskLevel::High,
        });

        // Low → auto
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::AutoApproved { .. }
        ));

        // Medium → auto
        let action = SideEffectAction::new(
            ActionType::GitPush,
            "origin main".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::AutoApproved { .. }
        ));

        // High → auto (equal to max_auto_risk, not greater)
        let action = SideEffectAction::new(
            ActionType::NetworkRequest,
            "https://api.example.com".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::AutoApproved { .. }
        ));

        // Critical → requires approval (greater than max_auto_risk)
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/production/config.yaml".to_string(),
            serde_json::json!({}),
        );
        assert!(matches!(
            gate.process(action),
            GateResult::RequiresApproval { .. }
        ));
    }

    #[test]
    fn test_record_approval_updates_history() {
        let mut gate = SideEffectGate::new(GatePolicy::Always);
        let action = SideEffectAction::new(
            ActionType::NetworkRequest,
            "https://api.example.com".to_string(),
            serde_json::json!({}),
        );
        let action_id = action.id;
        gate.process(action);

        // Initially no approval
        assert!(gate.audit_history()[0].approval.is_none());

        // Record approval
        let decision = ApprovalDecision {
            approver: "admin".to_string(),
            decision: ApprovalOutcome::Approved,
            reason: "safe endpoint".to_string(),
            timestamp: Utc::now(),
        };
        gate.record_approval(action_id, decision);

        // Now has approval
        let entry = &gate.audit_history()[0];
        assert!(entry.approval.is_some());
        assert!(matches!(
            entry.approval.as_ref().unwrap().decision,
            ApprovalOutcome::Approved
        ));
    }

    #[test]
    fn test_record_approval_nonexistent_id() {
        let mut gate = SideEffectGate::new(GatePolicy::Always);
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        gate.process(action);

        // Try to record approval for a non-existent ID
        let decision = ApprovalDecision {
            approver: "admin".to_string(),
            decision: ApprovalOutcome::Approved,
            reason: "test".to_string(),
            timestamp: Utc::now(),
        };
        gate.record_approval(Uuid::new_v4(), decision);

        // History unchanged — no approval recorded
        assert!(gate.audit_history()[0].approval.is_none());
    }

    #[test]
    fn test_execution_mode_defaults_to_dry_run() {
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        assert_eq!(action.mode, ExecutionMode::DryRun);
        assert!(action.preview_result.is_none());
        assert!(action.actual_result.is_none());
        assert!(action.approval.is_none());
    }

    #[test]
    fn test_preview_reversible_field() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoLow);

        // FileWrite → reversible
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        let result = gate.process(action);
        if let GateResult::AutoApproved { preview, .. } = result {
            assert!(preview.reversible);
        } else {
            panic!("Expected AutoApproved");
        }

        // FileDelete → not reversible
        let action = SideEffectAction::new(
            ActionType::FileDelete,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        let result = gate.process(action);
        if let GateResult::RequiresApproval { preview, .. } = result {
            assert!(!preview.reversible);
        } else {
            panic!("Expected RequiresApproval");
        }
    }

    #[test]
    fn test_stats_with_mixed_operations() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoMedium);

        // 2 low (auto), 1 medium (auto), 1 high (requires)
        gate.process(SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/a".to_string(),
            serde_json::json!({}),
        ));
        gate.process(SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/b".to_string(),
            serde_json::json!({}),
        ));
        gate.process(SideEffectAction::new(
            ActionType::GitPush,
            "origin main".to_string(),
            serde_json::json!({}),
        ));
        gate.process(SideEffectAction::new(
            ActionType::NetworkRequest,
            "https://api.example.com".to_string(),
            serde_json::json!({}),
        ));

        let stats = gate.stats();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.auto_approved, 3);
        assert_eq!(stats.requires_approval, 1);
        assert_eq!(stats.approved, 0);
        assert_eq!(stats.rejected, 0);
    }

    #[test]
    fn test_prod_path_in_target() {
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/app/prod/config.json".to_string(),
            serde_json::json!({}),
        );
        assert_eq!(action.risk_level(), RiskLevel::Critical);
    }

    #[test]
    fn test_approval_outcome_rejected() {
        let mut gate = SideEffectGate::new(GatePolicy::Always);
        let action = SideEffectAction::new(
            ActionType::DataMutation,
            "users_table".to_string(),
            serde_json::json!({"delete": true}),
        );
        let action_id = action.id;
        gate.process(action);

        let decision = ApprovalDecision {
            approver: "security-team".to_string(),
            decision: ApprovalOutcome::Rejected,
            reason: "too risky".to_string(),
            timestamp: Utc::now(),
        };
        gate.record_approval(action_id, decision);

        let stats = gate.stats();
        assert_eq!(stats.rejected, 1);
        assert_eq!(stats.approved, 0);
    }

    #[test]
    fn test_action_type_variants() {
        let types = [
            ActionType::FileWrite,
            ActionType::FileDelete,
            ActionType::GitPush,
            ActionType::DataMutation,
            ActionType::NetworkRequest,
            ActionType::CommandExec,
        ];
        // All variants should be distinct
        let unique: std::collections::HashSet<_> = types.iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn test_gate_policy_variants() {
        let policies = [
            GatePolicy::AutoLow,
            GatePolicy::AutoMedium,
            GatePolicy::Always,
        ];
        assert_eq!(policies.len(), 3);
    }

    #[test]
    fn test_approval_decision_serialization() {
        let decision = ApprovalDecision {
            approver: "admin".to_string(),
            decision: ApprovalOutcome::Approved,
            reason: "looks good".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("admin"));
        assert!(json.contains("Approved"));
    }

    #[test]
    fn test_side_effect_action_serialization() {
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test".to_string(),
            serde_json::json!({"content": "hello"}),
        );
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("FileWrite"));
        assert!(json.contains("/tmp/test"));
    }

    #[test]
    fn test_gate_stats_clone() {
        let stats = GateStats {
            total: 10,
            auto_approved: 5,
            requires_approval: 3,
            approved: 1,
            rejected: 1,
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total, 10);
        assert_eq!(cloned.auto_approved, 5);
    }

    #[test]
    fn test_low_risk_auto_approve() {
        let mut gate = SideEffectGate::new(GatePolicy::AutoLow);
        let action = SideEffectAction::new(
            ActionType::FileWrite,
            "/tmp/test.txt".to_string(),
            serde_json::json!({}),
        );
        let result = gate.process(action);
        // Low risk with HighRiskOnly policy should auto-approve
        assert!(matches!(result, GateResult::AutoApproved { .. }));
    }
}
