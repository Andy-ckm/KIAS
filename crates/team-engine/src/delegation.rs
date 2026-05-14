//! # Delegation Protocol
//!
//! CrewAI-style agent-to-agent delegation protocol.
//!
//! Agents can autonomously delegate tasks to better-suited peers through
//! a structured message-passing protocol. This enables hierarchical
//! orchestration where a lead agent decomposes work and delegates subtasks
//! to specialized workers.
//!
//! ## Protocol Flow
//!
//! ```text
//! Agent A (delegator)          Agent B (delegatee)
//!       │                            │
//!       │──── DelegateRequest ──────▶│
//!       │                            │── Evaluate capability
//!       │◀─── DelegateResponse ─────│
//!       │      (Accept/Reject/       │
//!       │       CounterPropose)      │
//!       │                            │
//!       │──── TaskPayload ──────────▶│
//!       │                            │── Execute
//!       │◀─── TaskResult ───────────│
//!       │                            │
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a delegation session
pub type DelegationId = String;

/// Unique identifier for an agent
pub type AgentId = String;

/// Priority level for delegation requests
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum DelegationPriority {
    /// Low priority - best effort
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority - should be processed soon
    High = 2,
    /// Critical priority - immediate attention required
    Critical = 3,
}

/// A message in the delegation protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegationMessage {
    /// Request to delegate a task
    Request(DelegateRequest),
    /// Response to a delegation request
    Response(DelegateResponse),
    /// Update on task progress
    Progress(ProgressUpdate),
    /// Final result from delegatee
    Result(DelegationResult),
    /// Cancellation of a delegation
    Cancel(CancelDelegation),
}

/// Request from delegator to delegatee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateRequest {
    /// Unique delegation session ID
    pub delegation_id: DelegationId,
    /// Who is asking
    pub from_agent: AgentId,
    /// Who should do it
    pub to_agent: AgentId,
    /// Task description
    pub task_description: String,
    /// Required capabilities for this task
    pub required_capabilities: Vec<String>,
    /// Task context (previous results, constraints, etc.)
    pub context: serde_json::Value,
    /// Priority
    pub priority: DelegationPriority,
    /// Maximum time allowed (seconds)
    pub timeout_secs: u64,
    /// Maximum retries allowed
    pub max_retries: u32,
    /// When the request was created
    pub created_at: DateTime<Utc>,
}

/// Response from delegatee back to delegator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegateResponse {
    /// Accept the delegation
    Accept {
        delegation_id: DelegationId,
        /// Estimated time to complete (seconds)
        estimated_secs: u64,
        /// Any notes from the delegatee
        notes: Option<String>,
    },
    /// Reject the delegation
    Reject {
        delegation_id: DelegationId,
        /// Reason for rejection
        reason: String,
        /// Suggested alternative agent (if known)
        suggest_alternative: Option<AgentId>,
    },
    /// Counter-propose with modified parameters
    CounterPropose {
        delegation_id: DelegationId,
        /// Modified task description
        modified_description: Option<String>,
        /// Modified timeout
        modified_timeout_secs: Option<u64>,
        /// Reason for the counter-proposal
        reason: String,
    },
}

/// Progress update from delegatee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub delegation_id: DelegationId,
    pub agent_id: AgentId,
    /// Progress percentage (0-100)
    pub progress_pct: u8,
    /// Human-readable status
    pub status_message: String,
    /// Timestamp
    pub updated_at: DateTime<Utc>,
}

/// Final result from delegatee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    pub delegation_id: DelegationId,
    pub agent_id: AgentId,
    /// Whether the task succeeded
    pub success: bool,
    /// Output payload
    pub output: serde_json::Value,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp
    pub completed_at: DateTime<Utc>,
}

/// Cancel a delegation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelDelegation {
    pub delegation_id: DelegationId,
    pub from_agent: AgentId,
    pub reason: String,
    pub cancelled_at: DateTime<Utc>,
}

/// Delegation state machine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DelegationState {
    /// Request sent, waiting for response
    Pending,
    /// Delegatee accepted
    Accepted,
    /// Delegatee rejected
    Rejected,
    /// Delegatee counter-proposed
    CounterProposed,
    /// Task is being executed
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Delegation was cancelled
    Cancelled,
    /// Timed out
    TimedOut,
}

/// Active delegation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRecord {
    /// Delegation ID
    pub id: DelegationId,
    /// Original request
    pub request: DelegateRequest,
    /// Current state
    pub state: DelegationState,
    /// Response (if any)
    pub response: Option<DelegateResponse>,
    /// Progress updates
    pub progress: Vec<ProgressUpdate>,
    /// Final result (if any)
    pub result: Option<DelegationResult>,
    /// When delegation was created
    pub created_at: DateTime<Utc>,
    /// When delegation was last updated
    pub updated_at: DateTime<Utc>,
}

impl DelegationRecord {
    /// Create a new delegation record from a request
    pub fn from_request(request: DelegateRequest) -> Self {
        let now = Utc::now();
        Self {
            id: request.delegation_id.clone(),
            request,
            state: DelegationState::Pending,
            response: None,
            progress: Vec::new(),
            result: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if the delegation is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            DelegationState::Completed
                | DelegationState::Failed
                | DelegationState::Cancelled
                | DelegationState::TimedOut
        )
    }

    /// Check if the delegation is still active
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> DelegateRequest {
        DelegateRequest {
            delegation_id: "dlg-1".to_string(),
            from_agent: "owner-1".to_string(),
            to_agent: "worker-1".to_string(),
            task_description: "Analyze log files".to_string(),
            required_capabilities: vec!["log_analysis".to_string()],
            context: serde_json::json!({"file": "/var/log/app.log"}),
            priority: DelegationPriority::Normal,
            timeout_secs: 300,
            max_retries: 3,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_delegation_record_from_request() {
        let req = make_request();
        let record = DelegationRecord::from_request(req);
        assert_eq!(record.state, DelegationState::Pending);
        assert!(record.response.is_none());
        assert!(record.result.is_none());
        assert!(record.is_active());
    }

    #[test]
    fn test_delegation_state_transitions() {
        let req = make_request();
        let mut record = DelegationRecord::from_request(req);

        assert_eq!(record.state, DelegationState::Pending);
        assert!(!record.is_terminal());

        record.state = DelegationState::Accepted;
        assert!(record.is_active());

        record.state = DelegationState::InProgress;
        assert!(record.is_active());

        record.state = DelegationState::Completed;
        assert!(record.is_terminal());
    }

    #[test]
    fn test_delegation_priority_ordering() {
        assert!(DelegationPriority::Low < DelegationPriority::Normal);
        assert!(DelegationPriority::Normal < DelegationPriority::High);
        assert!(DelegationPriority::High < DelegationPriority::Critical);
    }

    #[test]
    fn test_delegation_message_variants() {
        let req = make_request();
        let msg = DelegationMessage::Request(req.clone());
        assert!(matches!(msg, DelegationMessage::Request(_)));

        let accept = DelegateResponse::Accept {
            delegation_id: "dlg-1".to_string(),
            estimated_secs: 60,
            notes: Some("Will start immediately".to_string()),
        };
        let msg = DelegationMessage::Response(accept);
        assert!(matches!(msg, DelegationMessage::Response(_)));
    }

    #[test]
    fn test_delegate_response_reject_with_alternative() {
        let reject = DelegateResponse::Reject {
            delegation_id: "dlg-1".to_string(),
            reason: "I don't have log analysis capability".to_string(),
            suggest_alternative: Some("worker-2".to_string()),
        };
        if let DelegateResponse::Reject {
            suggest_alternative,
            ..
        } = reject
        {
            assert_eq!(suggest_alternative, Some("worker-2".to_string()));
        } else {
            panic!("Expected Reject variant");
        }
    }

    #[test]
    fn test_counter_propose() {
        let counter = DelegateResponse::CounterPropose {
            delegation_id: "dlg-1".to_string(),
            modified_description: Some("Analyze only error logs".to_string()),
            modified_timeout_secs: Some(600),
            reason: "Full log analysis would take too long".to_string(),
        };
        assert!(matches!(counter, DelegateResponse::CounterPropose { .. }));
    }

    #[test]
    fn test_progress_update() {
        let update = ProgressUpdate {
            delegation_id: "dlg-1".to_string(),
            agent_id: "worker-1".to_string(),
            progress_pct: 50,
            status_message: "Halfway done".to_string(),
            updated_at: Utc::now(),
        };
        assert_eq!(update.progress_pct, 50);
    }

    #[test]
    fn test_delegation_result_success() {
        let result = DelegationResult {
            delegation_id: "dlg-1".to_string(),
            agent_id: "worker-1".to_string(),
            success: true,
            output: serde_json::json!({"errors_found": 3}),
            error: None,
            duration_ms: 1500,
            completed_at: Utc::now(),
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_cancel_delegation() {
        let cancel = CancelDelegation {
            delegation_id: "dlg-1".to_string(),
            from_agent: "owner-1".to_string(),
            reason: "Task no longer needed".to_string(),
            cancelled_at: Utc::now(),
        };
        assert_eq!(cancel.reason, "Task no longer needed");
    }

    #[test]
    fn test_all_terminal_states() {
        let states = [
            DelegationState::Completed,
            DelegationState::Failed,
            DelegationState::Cancelled,
            DelegationState::TimedOut,
        ];
        for state in states {
            let req = make_request();
            let mut record = DelegationRecord::from_request(req);
            record.state = state.clone();
            assert!(record.is_terminal(), "Expected {:?} to be terminal", state);
            assert!(!record.is_active());
        }
    }

    #[test]
    fn test_all_non_terminal_states() {
        let states = [
            DelegationState::Pending,
            DelegationState::Accepted,
            DelegationState::Rejected,
            DelegationState::CounterProposed,
            DelegationState::InProgress,
        ];
        for state in states {
            let req = make_request();
            let mut record = DelegationRecord::from_request(req);
            record.state = state.clone();
            assert!(record.is_active(), "Expected {:?} to be active", state);
        }
    }
}
