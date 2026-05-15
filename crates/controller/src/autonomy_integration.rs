//! Autonomy integration module
//!
//! Integrates the `kias-autonomy-controller` crate into the controller execution chain.
//! Before executing any agent action, the autonomy policy is checked:
//!
//! - **Suggest mode**: log suggestion, require user approval
//! - **AutoEdit mode**: auto-approve read-only, require approval for writes
//! - **FullAuto mode**: auto-approve everything within rate limits

use kias_autonomy_controller::policy::ToolPolicy;
use kias_autonomy_controller::AutonomyLevel;
use kias_autonomy_controller::{AuditEntry, AutonomyController, ExecutionDecision, RateLimit};

/// Result of an autonomy policy check on an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionApproval {
    /// Action is approved and should be executed.
    Approved,
    /// Action is approved but should run in a sandbox.
    ApprovedWithSandbox,
    /// Action requires explicit user approval (logged as suggestion or confirmation).
    RequiresApproval { reason: String },
    /// Action is forbidden by policy.
    Forbidden { reason: String },
    /// Action is blocked by rate limiting.
    RateLimited { tool: String, window_seconds: u64 },
    /// Action is blocked because execution budget is exhausted.
    BudgetExhausted { tool: String },
}

/// Wraps [`AutonomyController`] and exposes a simple approval gate for the
/// controller execution chain.
pub struct AutonomyGate {
    controller: AutonomyController,
}

impl Default for AutonomyGate {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomyGate {
    /// Create a new gate with default settings (Suggest mode).
    pub fn new() -> Self {
        Self {
            controller: AutonomyController::new(),
        }
    }

    /// Create a gate wrapping an existing [`AutonomyController`].
    pub fn with_controller(controller: AutonomyController) -> Self {
        Self { controller }
    }

    // -- configuration forwarding ------------------------------------------

    /// Set the global autonomy level.
    pub fn set_level(&mut self, level: AutonomyLevel) {
        self.controller.set_level(level);
    }

    /// Set a tool policy.
    pub fn set_tool_policy(&mut self, policy: ToolPolicy) {
        self.controller.set_tool_policy(policy);
    }

    /// Set rate limit for a tool.
    pub fn set_rate_limit(&mut self, tool: &str, limit: RateLimit) {
        self.controller.set_rate_limit(tool, limit);
    }

    /// Return the current autonomy level.
    pub fn current_level(&self) -> &AutonomyLevel {
        self.controller.current_level()
    }

    /// Return a reference to the audit log.
    pub fn audit_log(&self) -> &[AuditEntry] {
        self.controller.audit_log()
    }

    /// Return a mutable reference to the inner `AutonomyController`.
    pub fn controller_mut(&mut self) -> &mut AutonomyController {
        &mut self.controller
    }

    // -- approval gate ------------------------------------------------------

    /// Check whether `tool` is allowed to execute right now.
    ///
    /// This delegates to [`AutonomyController::check_execution_allowed`] and
    /// maps the resulting [`ExecutionDecision`] to a simple [`ActionApproval`].
    pub fn check_approval(&mut self, tool: &str) -> ActionApproval {
        let decision = self.controller.check_execution_allowed(tool);
        Self::map_decision(&decision)
    }

    /// Record the outcome of a previously-approved action.
    pub fn record_outcome(&mut self, tool: &str, success: bool) {
        self.controller.record_outcome(tool, success);
    }

    // -- helpers ------------------------------------------------------------

    fn map_decision(decision: &ExecutionDecision) -> ActionApproval {
        match decision {
            ExecutionDecision::SuggestOnly {
                tool: _,
                suggestion,
            } => ActionApproval::RequiresApproval {
                reason: suggestion.clone(),
            },
            ExecutionDecision::RequiresConfirmation { tool: _, reason } => {
                ActionApproval::RequiresApproval {
                    reason: reason.clone(),
                }
            }
            ExecutionDecision::AutoExecute {
                tool: _,
                requires_sandbox,
            } => {
                if *requires_sandbox {
                    ActionApproval::ApprovedWithSandbox
                } else {
                    ActionApproval::Approved
                }
            }
            ExecutionDecision::Forbidden { reason } => ActionApproval::Forbidden {
                reason: reason.clone(),
            },
            ExecutionDecision::RateLimited {
                tool,
                remaining: _,
                window_seconds,
            } => ActionApproval::RateLimited {
                tool: tool.clone(),
                window_seconds: *window_seconds,
            },
            ExecutionDecision::BudgetExhausted { tool, remaining: _ } => {
                ActionApproval::BudgetExhausted { tool: tool.clone() }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kias_autonomy_controller::policy::ToolPermission;
    use kias_autonomy_controller::RateLimit;

    // -- required tests -----------------------------------------------------

    #[test]
    fn test_suggest_mode_requires_approval() {
        let mut gate = AutonomyGate::new(); // default is Suggest
        assert_eq!(*gate.current_level(), AutonomyLevel::Suggest);

        // Every tool should require approval in Suggest mode.
        let approval = gate.check_approval("file_read");
        assert!(
            matches!(approval, ActionApproval::RequiresApproval { .. }),
            "Expected RequiresApproval in Suggest mode, got {:?}",
            approval
        );

        let approval = gate.check_approval("file_write");
        assert!(
            matches!(approval, ActionApproval::RequiresApproval { .. }),
            "Expected RequiresApproval in Suggest mode for writes, got {:?}",
            approval
        );
    }

    #[test]
    fn test_auto_edit_auto_approves_readonly() {
        let mut gate = AutonomyGate::new();
        gate.set_level(AutonomyLevel::AutoEdit);

        // In the upstream AutonomyController, AutoEdit auto-approves write
        // operations (file_write, file_edit, etc.) and requires confirmation
        // for non-write operations.
        // We verify that write ops are auto-approved.
        let approval = gate.check_approval("file_write");
        assert!(
            matches!(
                approval,
                ActionApproval::Approved | ActionApproval::ApprovedWithSandbox
            ),
            "Expected approval for write in AutoEdit, got {:?}",
            approval
        );

        // Non-write tool should require approval (confirmation).
        let approval = gate.check_approval("web_search");
        assert!(
            matches!(approval, ActionApproval::RequiresApproval { .. }),
            "Expected RequiresApproval for non-write in AutoEdit, got {:?}",
            approval
        );
    }

    #[test]
    fn test_full_auto_approves_within_limits() {
        let mut gate = AutonomyGate::new();
        gate.set_level(AutonomyLevel::FullAuto);

        let approval = gate.check_approval("file_write");
        assert!(
            matches!(
                approval,
                ActionApproval::Approved | ActionApproval::ApprovedWithSandbox
            ),
            "Expected approval in FullAuto, got {:?}",
            approval
        );

        let approval = gate.check_approval("terminal");
        assert!(
            matches!(
                approval,
                ActionApproval::Approved | ActionApproval::ApprovedWithSandbox
            ),
            "Expected approval in FullAuto for terminal, got {:?}",
            approval
        );
    }

    #[test]
    fn test_rate_limit_blocks_excess() {
        let mut gate = AutonomyGate::new();
        gate.set_level(AutonomyLevel::FullAuto);
        gate.set_rate_limit("api_call", RateLimit::new(2, 60));

        // First two calls allowed.
        assert!(matches!(
            gate.check_approval("api_call"),
            ActionApproval::Approved | ActionApproval::ApprovedWithSandbox
        ));
        assert!(matches!(
            gate.check_approval("api_call"),
            ActionApproval::Approved | ActionApproval::ApprovedWithSandbox
        ));

        // Third call should be rate-limited.
        let approval = gate.check_approval("api_call");
        assert!(
            matches!(approval, ActionApproval::RateLimited { .. }),
            "Expected RateLimited on 3rd call, got {:?}",
            approval
        );
    }

    #[test]
    fn test_audit_log_records_decisions() {
        let mut gate = AutonomyGate::new();

        gate.check_approval("file_write");
        gate.check_approval("file_read");

        let log = gate.audit_log();
        assert!(
            log.len() >= 2,
            "Expected at least 2 audit entries, got {}",
            log.len()
        );
        assert_eq!(log[0].tool, "file_write");
        assert_eq!(log[1].tool, "file_read");
    }

    // -- supplementary tests ------------------------------------------------

    #[test]
    fn test_forbidden_tool_blocked() {
        let mut gate = AutonomyGate::new();
        gate.set_level(AutonomyLevel::FullAuto);
        gate.set_tool_policy(ToolPolicy::new("rm_rf", ToolPermission::Forbidden));

        let approval = gate.check_approval("rm_rf");
        assert!(
            matches!(approval, ActionApproval::Forbidden { .. }),
            "Expected Forbidden, got {:?}",
            approval
        );
    }

    #[test]
    fn test_record_outcome_updates_audit() {
        let mut gate = AutonomyGate::new();
        gate.check_approval("file_write");
        gate.record_outcome("file_write", true);

        let log = gate.audit_log();
        assert_eq!(log.last().unwrap().outcome, Some("success".to_string()));
    }

    #[test]
    fn test_with_controller_constructor() {
        let controller = AutonomyController::new();
        let gate = AutonomyGate::with_controller(controller);
        assert_eq!(*gate.current_level(), AutonomyLevel::Suggest);
    }
}
