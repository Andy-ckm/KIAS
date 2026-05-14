use super::ladder::{AutonomyLadder, AutonomyLevel};
use super::policy::ToolPolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 自主度控制器（借鉴 Codex CLI 三模式执行）
///
/// 核心设计：
/// 1. 三种递进的自主级别：Suggest / AutoEdit / FullAuto
/// 2. 用户可根据信任度选择
/// 3. 可对单个工具单独设置自主级别
/// 4. 分级执行沙箱
/// 5. 执行审计日志
/// 6. 速率限制和执行预算
/// 7. 自动升级机制
pub struct AutonomyController {
    ladder: AutonomyLadder,
    policies: HashMap<String, ToolPolicy>,
    audit_log: Vec<AuditEntry>,
    rate_limits: HashMap<String, RateLimit>,
    execution_budget: Option<ExecutionBudget>,
    escalation_config: Option<EscalationConfig>,
}

/// Audit log entry for execution decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub tool: String,
    pub decision: String,
    pub reason: Option<String>,
    pub outcome: Option<String>,
}

/// Rate limit configuration per tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum executions per window
    pub max_executions: u32,
    /// Window duration in seconds
    pub window_seconds: u64,
    /// Execution timestamps in current window
    #[serde(skip)]
    pub executions: Vec<DateTime<Utc>>,
}

impl RateLimit {
    pub fn new(max_executions: u32, window_seconds: u64) -> Self {
        Self {
            max_executions,
            window_seconds,
            executions: Vec::new(),
        }
    }

    /// Check if execution is allowed under rate limit
    pub fn check_and_record(&mut self) -> bool {
        let now = Utc::now();
        let window = chrono::Duration::seconds(self.window_seconds as i64);

        // Remove expired entries
        self.executions
            .retain(|t| now.signed_duration_since(*t) < window);

        if self.executions.len() < self.max_executions as usize {
            self.executions.push(now);
            true
        } else {
            false
        }
    }

    /// Get remaining executions in current window
    pub fn remaining(&self) -> u32 {
        let now = Utc::now();
        let window = chrono::Duration::seconds(self.window_seconds as i64);
        let active = self
            .executions
            .iter()
            .filter(|t| now.signed_duration_since(**t) < window)
            .count();
        self.max_executions.saturating_sub(active as u32)
    }
}

/// Execution budget - max total executions before re-authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBudget {
    pub max_total: u32,
    pub used: u32,
}

impl ExecutionBudget {
    pub fn new(max_total: u32) -> Self {
        Self { max_total, used: 0 }
    }

    pub fn consume(&mut self) -> bool {
        if self.used < self.max_total {
            self.used += 1;
            true
        } else {
            false
        }
    }

    pub fn remaining(&self) -> u32 {
        self.max_total.saturating_sub(self.used)
    }

    pub fn reset(&mut self) {
        self.used = 0;
    }
}

/// Escalation configuration - auto-promote autonomy level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    /// Number of successful executions needed to escalate
    pub success_threshold: u32,
    /// Current success count
    pub success_count: u32,
    /// Target level to escalate to
    pub target_level: AutonomyLevel,
}

impl EscalationConfig {
    pub fn new(success_threshold: u32, target_level: AutonomyLevel) -> Self {
        Self {
            success_threshold,
            success_count: 0,
            target_level,
        }
    }

    /// Record a successful execution, returns true if escalation threshold reached
    pub fn record_success(&mut self) -> bool {
        self.success_count += 1;
        self.success_count >= self.success_threshold
    }
}

impl Default for AutonomyController {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomyController {
    pub fn new() -> Self {
        Self {
            ladder: AutonomyLadder::new(),
            policies: HashMap::new(),
            audit_log: Vec::new(),
            rate_limits: HashMap::new(),
            execution_budget: None,
            escalation_config: None,
        }
    }

    /// 设置全局自主级别
    pub fn set_level(&mut self, level: AutonomyLevel) {
        tracing::info!(level = ?level, "Autonomy level changed");
        self.ladder.set_level(level);
    }

    /// 为工具设置策略
    pub fn set_tool_policy(&mut self, policy: ToolPolicy) {
        self.policies.insert(policy.tool_name.clone(), policy);
    }

    /// Set rate limit for a tool
    pub fn set_rate_limit(&mut self, tool: &str, limit: RateLimit) {
        self.rate_limits.insert(tool.to_string(), limit);
    }

    /// Set execution budget
    pub fn set_execution_budget(&mut self, budget: ExecutionBudget) {
        self.execution_budget = Some(budget);
    }

    /// Set escalation configuration
    pub fn set_escalation_config(&mut self, config: EscalationConfig) {
        self.escalation_config = Some(config);
    }

    /// Record a successful execution outcome
    pub fn record_outcome(&mut self, tool: &str, success: bool) {
        // Update audit log
        if let Some(entry) = self.audit_log.last_mut() {
            if entry.tool == tool && entry.outcome.is_none() {
                entry.outcome = Some(if success {
                    "success".to_string()
                } else {
                    "failure".to_string()
                });
            }
        }

        // Check escalation
        if success {
            if let Some(ref mut config) = self.escalation_config {
                if config.record_success() {
                    tracing::info!(
                        target_level = ?config.target_level,
                        "Autonomy escalation threshold reached"
                    );
                    self.ladder.set_level(config.target_level.clone());
                    self.escalation_config = None; // Disable after escalation
                }
            }
        }
    }

    /// Check if execution is allowed (full pipeline: policy → ladder → rate limit → budget)
    pub fn check_execution_allowed(&mut self, tool: &str) -> ExecutionDecision {
        // 1. Check tool policy
        let policy = self.policies.get(tool);
        let autonomy_level = self.ladder.get_tool_level(tool);

        if let Some(p) = policy {
            if !p.is_allowed() {
                self.log_decision(tool, "Forbidden", Some("Tool policy forbids execution"));
                return ExecutionDecision::Forbidden {
                    reason: format!("Tool '{}' is forbidden by policy", tool),
                };
            }
            if p.needs_confirmation() {
                self.log_decision(
                    tool,
                    "RequiresConfirmation",
                    Some("Tool policy requires confirmation"),
                );
                return ExecutionDecision::RequiresConfirmation {
                    tool: tool.to_string(),
                    reason: format!("Tool '{}' requires confirmation", tool),
                };
            }
        }

        // 2. Check rate limit
        let rate_limited = if let Some(ref mut limit) = self.rate_limits.get_mut(tool) {
            if !limit.check_and_record() {
                let window = limit.window_seconds;
                self.log_decision(tool, "RateLimited", Some("Rate limit exceeded"));
                return ExecutionDecision::RateLimited {
                    tool: tool.to_string(),
                    remaining: 0,
                    window_seconds: window,
                };
            }
            true
        } else {
            false
        };
        let _ = rate_limited;

        // 3. Check execution budget
        if let Some(ref mut budget) = self.execution_budget {
            if !budget.consume() {
                self.log_decision(tool, "BudgetExhausted", Some("Execution budget exhausted"));
                return ExecutionDecision::BudgetExhausted {
                    tool: tool.to_string(),
                    remaining: 0,
                };
            }
        }

        // 4. Check autonomy level
        let decision = match autonomy_level {
            AutonomyLevel::Suggest => ExecutionDecision::SuggestOnly {
                tool: tool.to_string(),
                suggestion: "In suggest mode, provide recommendation only".to_string(),
            },
            AutonomyLevel::AutoEdit => {
                if self.is_write_operation(tool) {
                    ExecutionDecision::AutoExecute {
                        tool: tool.to_string(),
                        requires_sandbox: true,
                    }
                } else {
                    ExecutionDecision::RequiresConfirmation {
                        tool: tool.to_string(),
                        reason: "Non-edit operations require confirmation in AutoEdit mode"
                            .to_string(),
                    }
                }
            }
            AutonomyLevel::FullAuto => ExecutionDecision::AutoExecute {
                tool: tool.to_string(),
                requires_sandbox: self
                    .policies
                    .get(tool)
                    .map(|p| p.requires_sandbox)
                    .unwrap_or(true),
            },
        };

        let decision_str = match &decision {
            ExecutionDecision::SuggestOnly { .. } => "SuggestOnly",
            ExecutionDecision::RequiresConfirmation { .. } => "RequiresConfirmation",
            ExecutionDecision::AutoExecute { .. } => "AutoExecute",
            ExecutionDecision::Forbidden { .. } => "Forbidden",
            ExecutionDecision::RateLimited { .. } => "RateLimited",
            ExecutionDecision::BudgetExhausted { .. } => "BudgetExhausted",
        };
        self.log_decision(tool, decision_str, None);

        decision
    }

    /// Log an execution decision
    fn log_decision(&mut self, tool: &str, decision: &str, reason: Option<&str>) {
        self.audit_log.push(AuditEntry {
            timestamp: Utc::now(),
            tool: tool.to_string(),
            decision: decision.to_string(),
            reason: reason.map(|s| s.to_string()),
            outcome: None,
        });

        // Keep only last 1000 entries
        if self.audit_log.len() > 1000 {
            self.audit_log.remove(0);
        }
    }

    /// Get audit log
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Get current autonomy level
    pub fn current_level(&self) -> &AutonomyLevel {
        &self.ladder.current_level
    }

    /// 检查是否为写操作
    fn is_write_operation(&self, tool: &str) -> bool {
        matches!(
            tool,
            "file_write" | "file_edit" | "file_patch" | "terminal_write"
        )
    }
}

/// 执行决策
#[derive(Debug, Clone)]
pub enum ExecutionDecision {
    /// 仅提供建议
    SuggestOnly { tool: String, suggestion: String },
    /// 需要确认
    RequiresConfirmation { tool: String, reason: String },
    /// 自动执行
    AutoExecute {
        tool: String,
        requires_sandbox: bool,
    },
    /// 禁止执行
    Forbidden { reason: String },
    /// 速率限制
    RateLimited {
        tool: String,
        remaining: u32,
        window_seconds: u64,
    },
    /// 预算耗尽
    BudgetExhausted { tool: String, remaining: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{ToolPermission, ToolPolicy};

    #[test]
    fn test_autonomy_controller_default() {
        let mut controller = AutonomyController::new();
        let decision = controller.check_execution_allowed("file_write");
        assert!(matches!(decision, ExecutionDecision::SuggestOnly { .. }));
    }

    #[test]
    fn test_autonomy_full_auto() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);
        let decision = controller.check_execution_allowed("file_write");
        assert!(matches!(decision, ExecutionDecision::AutoExecute { .. }));
    }

    #[test]
    fn test_autonomy_auto_edit_write_ops() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::AutoEdit);

        for tool in &["file_write", "file_edit", "file_patch", "terminal_write"] {
            let decision = controller.check_execution_allowed(tool);
            assert!(
                matches!(decision, ExecutionDecision::AutoExecute { .. }),
                "Expected AutoExecute for {}",
                tool
            );
        }
    }

    #[test]
    fn test_autonomy_auto_edit_non_write_ops() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::AutoEdit);

        let decision = controller.check_execution_allowed("web_search");
        assert!(matches!(
            decision,
            ExecutionDecision::RequiresConfirmation { .. }
        ));
    }

    #[test]
    fn test_tool_policy_forbidden() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);
        controller.set_tool_policy(ToolPolicy::new("dangerous_tool", ToolPermission::Forbidden));

        let decision = controller.check_execution_allowed("dangerous_tool");
        assert!(matches!(decision, ExecutionDecision::Forbidden { .. }));
    }

    #[test]
    fn test_tool_policy_requires_confirmation() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);
        controller.set_tool_policy(ToolPolicy::new(
            "sensitive_tool",
            ToolPermission::RequireConfirmation,
        ));

        let decision = controller.check_execution_allowed("sensitive_tool");
        assert!(matches!(
            decision,
            ExecutionDecision::RequiresConfirmation { .. }
        ));
    }

    #[test]
    fn test_autonomy_ladder_tool_override() {
        let mut ladder = AutonomyLadder::new();
        assert_eq!(ladder.current_level, AutonomyLevel::Suggest);

        ladder.set_level(AutonomyLevel::FullAuto);
        assert_eq!(ladder.current_level, AutonomyLevel::FullAuto);

        ladder.set_tool_level("file_write", AutonomyLevel::Suggest);
        assert_eq!(ladder.get_tool_level("file_write"), &AutonomyLevel::Suggest);
        assert_eq!(ladder.get_tool_level("other"), &AutonomyLevel::FullAuto);
    }

    #[test]
    fn test_autonomy_ladder_capabilities() {
        let mut ladder = AutonomyLadder::new();
        assert!(!ladder.can_auto_execute("file_write"));
        assert!(!ladder.can_auto_edit("file_write"));

        ladder.set_level(AutonomyLevel::FullAuto);
        assert!(ladder.can_auto_execute("file_write"));
        assert!(ladder.can_auto_edit("file_write"));

        ladder.set_level(AutonomyLevel::AutoEdit);
        assert!(!ladder.can_auto_execute("file_write"));
        assert!(ladder.can_auto_edit("file_write"));
    }

    #[test]
    fn test_audit_log() {
        let mut controller = AutonomyController::new();
        controller.check_execution_allowed("file_write");
        controller.check_execution_allowed("web_search");

        let log = controller.audit_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].tool, "file_write");
        assert_eq!(log[1].tool, "web_search");
    }

    #[test]
    fn test_audit_log_outcome_recording() {
        let mut controller = AutonomyController::new();
        controller.check_execution_allowed("file_write");
        controller.record_outcome("file_write", true);

        let log = controller.audit_log();
        assert_eq!(log[0].outcome, Some("success".to_string()));
    }

    #[test]
    fn test_rate_limiting() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);
        controller.set_rate_limit("api_call", RateLimit::new(2, 60));

        // First two should succeed
        assert!(matches!(
            controller.check_execution_allowed("api_call"),
            ExecutionDecision::AutoExecute { .. }
        ));
        assert!(matches!(
            controller.check_execution_allowed("api_call"),
            ExecutionDecision::AutoExecute { .. }
        ));

        // Third should be rate limited
        assert!(matches!(
            controller.check_execution_allowed("api_call"),
            ExecutionDecision::RateLimited { .. }
        ));
    }

    #[test]
    fn test_rate_limit_remaining() {
        let mut limit = RateLimit::new(5, 60);
        assert_eq!(limit.remaining(), 5);
        limit.check_and_record();
        assert_eq!(limit.remaining(), 4);
    }

    #[test]
    fn test_execution_budget() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);
        controller.set_execution_budget(ExecutionBudget::new(2));

        assert!(matches!(
            controller.check_execution_allowed("file_write"),
            ExecutionDecision::AutoExecute { .. }
        ));
        assert!(matches!(
            controller.check_execution_allowed("file_write"),
            ExecutionDecision::AutoExecute { .. }
        ));
        assert!(matches!(
            controller.check_execution_allowed("file_write"),
            ExecutionDecision::BudgetExhausted { .. }
        ));
    }

    #[test]
    fn test_execution_budget_remaining() {
        let mut budget = ExecutionBudget::new(10);
        assert_eq!(budget.remaining(), 10);
        budget.consume();
        assert_eq!(budget.remaining(), 9);
    }

    #[test]
    fn test_execution_budget_reset() {
        let mut budget = ExecutionBudget::new(5);
        for _ in 0..5 {
            budget.consume();
        }
        assert_eq!(budget.remaining(), 0);
        budget.reset();
        assert_eq!(budget.remaining(), 5);
    }

    #[test]
    fn test_escalation() {
        let mut controller = AutonomyController::new();
        assert_eq!(*controller.current_level(), AutonomyLevel::Suggest);

        controller.set_escalation_config(EscalationConfig::new(3, AutonomyLevel::AutoEdit));

        // Record 2 successes - not enough
        controller.record_outcome("tool", true);
        controller.record_outcome("tool", true);
        assert_eq!(*controller.current_level(), AutonomyLevel::Suggest);

        // Third success triggers escalation
        controller.record_outcome("tool", true);
        assert_eq!(*controller.current_level(), AutonomyLevel::AutoEdit);
    }

    #[test]
    fn test_escalation_resets_after_trigger() {
        let mut controller = AutonomyController::new();
        controller.set_escalation_config(EscalationConfig::new(2, AutonomyLevel::FullAuto));

        controller.record_outcome("tool", true);
        controller.record_outcome("tool", true);
        assert_eq!(*controller.current_level(), AutonomyLevel::FullAuto);

        // Config should be consumed after escalation
        assert!(controller.escalation_config.is_none());
    }

    #[test]
    fn test_policy_builder_chain() {
        let policy = ToolPolicy::new("test", ToolPermission::AutoApprove)
            .with_sandbox(false)
            .with_network(true)
            .with_timeout(120);

        assert!(!policy.requires_sandbox);
        assert!(policy.requires_network);
        assert_eq!(policy.max_execution_time, Some(120));
    }

    #[test]
    fn test_unknown_tool_uses_global_level() {
        let mut controller = AutonomyController::new();
        controller.set_level(AutonomyLevel::FullAuto);

        let decision = controller.check_execution_allowed("unknown_tool_xyz");
        assert!(matches!(decision, ExecutionDecision::AutoExecute { .. }));
    }
}
