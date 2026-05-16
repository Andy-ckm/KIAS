//! Human-in-the-Loop (HITL) approval gate infrastructure.
//!
//! Inspired by CrewAI v1.14 HITL pre-review and OpenAI Agents SDK approval_func.
//!
//! Each workflow node can attach an [`ApprovalPolicy`] that controls whether
//! human approval is required before the node's output is accepted.  All
//! approval decisions are persisted to an audit trail via [`ApprovalStore`].
//!
//! # Design
//!
//! ```text
//! Node completes → ApprovalPolicy evaluates → Auto-approve or Wait for human
//!   AutoApprove  → record decision, continue
//!   HumanReview  → pause workflow, wait for approve/reject with timeout
//!   Threshold    → auto-approve if risk_score < threshold, else human review
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

// ─── ApprovalPolicy ──────────────────────────────────────────────────────

/// Policy that determines whether a node requires human approval.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ApprovalPolicy {
    /// Always require human approval.
    #[default]
    Always,

    /// Auto-approve if the node's computed risk score is below the threshold;
    /// otherwise require human review.
    Threshold {
        /// Risk score threshold — values >= this require human approval.
        risk_threshold: f64,
    },

    /// Automatically approve if certain conditions are met.
    /// Conditions are simple key-value matches against the node output.
    AutoApprove {
        /// All conditions must match for auto-approval.
        conditions: Vec<ApprovalCondition>,
    },

    /// Require human review with a timeout.
    /// If the timeout expires without a decision, the workflow is rejected
    /// (safe default).
    HumanReview {
        /// Maximum time to wait for a human decision.
        timeout: Duration,
        /// What to do on timeout: reject (safe default) or auto-approve.
        #[serde(default = "default_timeout_action")]
        on_timeout: TimeoutAction,
    },
}

fn default_timeout_action() -> TimeoutAction {
    TimeoutAction::Reject
}

/// A single condition for [`ApprovalPolicy::AutoApprove`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalCondition {
    /// The state/output key to check.
    pub field: String,
    /// Expected value (JSON).
    pub expected: serde_json::Value,
}

/// What happens when a [`ApprovalPolicy::HumanReview`] times out.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimeoutAction {
    /// Reject the node output (safe default — workflow fails).
    Reject,
    /// Auto-approve on timeout (lenient — workflow continues).
    Approve,
}

impl fmt::Display for ApprovalPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalPolicy::Always => write!(f, "Always"),
            ApprovalPolicy::Threshold { risk_threshold } => {
                write!(f, "Threshold(risk_threshold={})", risk_threshold)
            }
            ApprovalPolicy::AutoApprove { conditions } => {
                write!(f, "AutoApprove(conditions={})", conditions.len())
            }
            ApprovalPolicy::HumanReview {
                timeout,
                on_timeout,
            } => {
                write!(
                    f,
                    "HumanReview(timeout={:?}, on_timeout={:?})",
                    timeout, on_timeout
                )
            }
        }
    }
}

// ─── ApprovalContext ─────────────────────────────────────────────────────

/// Context passed to the approval system so it can make an informed decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalContext {
    /// The ID of the node seeking approval.
    pub node_id: String,
    /// The action/description of what the node did.
    pub action: String,
    /// Computed risk score (0.0 = safe, 1.0 = highest risk).
    pub risk_score: f64,
    /// Preview of the node's output (for human reviewer).
    pub preview_output: serde_json::Value,
    /// Previous approval history for this workflow (for context).
    pub history: Vec<ApprovalRecord>,
}

// ─── ApprovalDecision ────────────────────────────────────────────────────

/// The decision made by the approval system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApprovalDecision {
    /// Approved — workflow continues.
    Approved {
        /// Optional comment from the reviewer.
        comment: Option<String>,
    },
    /// Rejected — workflow fails or routes to a fallback.
    Rejected {
        /// Reason for rejection.
        reason: String,
    },
}

impl ApprovalDecision {
    pub fn is_approved(&self) -> bool {
        matches!(self, ApprovalDecision::Approved { .. })
    }
}

impl fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalDecision::Approved { comment } => match comment {
                Some(c) => write!(f, "Approved({})", c),
                None => write!(f, "Approved"),
            },
            ApprovalDecision::Rejected { reason } => write!(f, "Rejected({})", reason),
        }
    }
}

// ─── ApprovalRecord (audit trail) ────────────────────────────────────────

/// A persisted record of an approval decision for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    /// Unique ID for this approval record.
    pub id: String,
    /// Workflow ID.
    pub workflow_id: String,
    /// Node ID that was evaluated.
    pub node_id: String,
    /// The context at the time of decision.
    pub context: ApprovalContext,
    /// The decision that was made.
    pub decision: ApprovalDecision,
    /// Who made the decision (human user ID or "auto").
    pub decided_by: String,
    /// When the decision was made.
    pub decided_at: DateTime<Utc>,
    /// How long the decision took (from request to resolution).
    pub duration_ms: u64,
}

// ─── ApprovalStore trait ─────────────────────────────────────────────────

/// Trait for persisting approval decisions (audit trail).
#[async_trait]
pub trait ApprovalStore: Send + Sync + fmt::Debug {
    /// Persist an approval record.
    async fn save_record(&self, record: ApprovalRecord) -> anyhow::Result<()>;

    /// Load all approval records for a workflow (ordered by time).
    async fn list_records(&self, workflow_id: &str) -> anyhow::Result<Vec<ApprovalRecord>>;

    /// Load records for a specific node in a workflow.
    async fn get_records_for_node(
        &self,
        workflow_id: &str,
        node_id: &str,
    ) -> anyhow::Result<Vec<ApprovalRecord>>;
}

// ─── InMemoryApprovalStore ───────────────────────────────────────────────

/// In-memory approval store for testing and ephemeral use.
#[derive(Debug, Default)]
pub struct InMemoryApprovalStore {
    records: dashmap::DashMap<String, Vec<ApprovalRecord>>,
}

impl InMemoryApprovalStore {
    pub fn new() -> Self {
        Self {
            records: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl ApprovalStore for InMemoryApprovalStore {
    async fn save_record(&self, record: ApprovalRecord) -> anyhow::Result<()> {
        let mut entry = self.records.entry(record.workflow_id.clone()).or_default();
        entry.push(record);
        Ok(())
    }

    async fn list_records(&self, workflow_id: &str) -> anyhow::Result<Vec<ApprovalRecord>> {
        Ok(self
            .records
            .get(workflow_id)
            .map(|r| r.clone())
            .unwrap_or_default())
    }

    async fn get_records_for_node(
        &self,
        workflow_id: &str,
        node_id: &str,
    ) -> anyhow::Result<Vec<ApprovalRecord>> {
        Ok(self
            .records
            .get(workflow_id)
            .map(|r| {
                r.iter()
                    .filter(|rec| rec.node_id == node_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }
}

// ─── ApprovalEvaluator ──────────────────────────────────────────────────

/// Evaluates an [`ApprovalPolicy`] against an [`ApprovalContext`] and returns
/// whether human approval is needed or if the node can be auto-approved.
#[derive(Debug, Clone)]
pub enum ApprovalEvaluation {
    /// Auto-approved — no human needed.
    AutoApproved(ApprovalDecision),
    /// Requires human review.
    RequiresHumanReview,
}

/// Evaluate an approval policy against the given context.
///
/// Returns `AutoApproved` if the policy allows automatic approval,
/// or `RequiresHumanReview` if human input is needed.
pub fn evaluate_policy(policy: &ApprovalPolicy, ctx: &ApprovalContext) -> ApprovalEvaluation {
    match policy {
        ApprovalPolicy::Always => ApprovalEvaluation::RequiresHumanReview,

        ApprovalPolicy::Threshold { risk_threshold } => {
            if ctx.risk_score < *risk_threshold {
                ApprovalEvaluation::AutoApproved(ApprovalDecision::Approved {
                    comment: Some(format!(
                        "Auto-approved: risk_score {} < threshold {}",
                        ctx.risk_score, risk_threshold
                    )),
                })
            } else {
                ApprovalEvaluation::RequiresHumanReview
            }
        }

        ApprovalPolicy::AutoApprove { conditions } => {
            let all_match = conditions.iter().all(|cond| {
                ctx.preview_output
                    .get(&cond.field)
                    .map(|v| *v == cond.expected)
                    .unwrap_or(false)
            });

            if all_match {
                ApprovalEvaluation::AutoApproved(ApprovalDecision::Approved {
                    comment: Some("Auto-approved: all conditions matched".into()),
                })
            } else {
                ApprovalEvaluation::RequiresHumanReview
            }
        }

        ApprovalPolicy::HumanReview { .. } => ApprovalEvaluation::RequiresHumanReview,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(node_id: &str, risk_score: f64) -> ApprovalContext {
        ApprovalContext {
            node_id: node_id.to_string(),
            action: "test action".to_string(),
            risk_score,
            preview_output: serde_json::json!({"status": "ok"}),
            history: vec![],
        }
    }

    #[test]
    fn test_always_policy_requires_human() {
        let policy = ApprovalPolicy::Always;
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_threshold_policy_below_threshold() {
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n1", 0.3);
        let result = evaluate_policy(&policy, &ctx);
        assert!(matches!(
            result,
            ApprovalEvaluation::AutoApproved(ApprovalDecision::Approved { .. })
        ));
    }

    #[test]
    fn test_threshold_policy_above_threshold() {
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n1", 0.8);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_threshold_policy_at_threshold() {
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n1", 0.5);
        // At threshold = not below, so requires human review
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_auto_approve_all_conditions_match() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![ApprovalCondition {
                field: "status".to_string(),
                expected: serde_json::json!("ok"),
            }],
        };
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::AutoApproved(_)
        ));
    }

    #[test]
    fn test_auto_approve_condition_mismatch() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![ApprovalCondition {
                field: "status".to_string(),
                expected: serde_json::json!("error"),
            }],
        };
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_auto_approve_missing_field() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![ApprovalCondition {
                field: "nonexistent".to_string(),
                expected: serde_json::json!("anything"),
            }],
        };
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_human_review_policy() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(300),
            on_timeout: TimeoutAction::Reject,
        };
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_approval_decision_display() {
        let approved = ApprovalDecision::Approved {
            comment: Some("looks good".into()),
        };
        assert_eq!(approved.to_string(), "Approved(looks good)");

        let approved_no_comment = ApprovalDecision::Approved { comment: None };
        assert_eq!(approved_no_comment.to_string(), "Approved");

        let rejected = ApprovalDecision::Rejected {
            reason: "too risky".into(),
        };
        assert_eq!(rejected.to_string(), "Rejected(too risky)");
    }

    #[test]
    fn test_approval_decision_is_approved() {
        assert!(ApprovalDecision::Approved { comment: None }.is_approved());
        assert!(!ApprovalDecision::Rejected {
            reason: "no".into()
        }
        .is_approved());
    }

    #[test]
    fn test_approval_policy_display() {
        assert_eq!(ApprovalPolicy::Always.to_string(), "Always");
        assert_eq!(
            ApprovalPolicy::Threshold {
                risk_threshold: 0.7
            }
            .to_string(),
            "Threshold(risk_threshold=0.7)"
        );
    }

    #[test]
    fn test_approval_policy_default_is_always() {
        let policy = ApprovalPolicy::default();
        assert!(matches!(policy, ApprovalPolicy::Always));
    }

    #[test]
    fn test_timeout_action_default_is_reject() {
        assert_eq!(default_timeout_action(), TimeoutAction::Reject);
    }

    #[tokio::test]
    async fn test_in_memory_approval_store() {
        let store = InMemoryApprovalStore::new();

        let ctx = make_ctx("n1", 0.5);
        let record = ApprovalRecord {
            id: "ar-1".to_string(),
            workflow_id: "wf-1".to_string(),
            node_id: "n1".to_string(),
            context: ctx,
            decision: ApprovalDecision::Approved { comment: None },
            decided_by: "human-1".to_string(),
            decided_at: Utc::now(),
            duration_ms: 5000,
        };

        store.save_record(record).await.unwrap();

        let records = store.list_records("wf-1").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "ar-1");

        let node_records = store.get_records_for_node("wf-1", "n1").await.unwrap();
        assert_eq!(node_records.len(), 1);

        let empty = store.get_records_for_node("wf-1", "n2").await.unwrap();
        assert!(empty.is_empty());

        let empty_wf = store.list_records("unknown").await.unwrap();
        assert!(empty_wf.is_empty());
    }

    #[tokio::test]
    async fn test_approval_store_multiple_records() {
        let store = InMemoryApprovalStore::new();

        for i in 0..3 {
            let record = ApprovalRecord {
                id: format!("ar-{i}"),
                workflow_id: "wf-1".to_string(),
                node_id: format!("n{}", i % 2),
                context: make_ctx(&format!("n{}", i % 2), 0.5),
                decision: ApprovalDecision::Approved { comment: None },
                decided_by: "auto".to_string(),
                decided_at: Utc::now(),
                duration_ms: 100,
            };
            store.save_record(record).await.unwrap();
        }

        let all = store.list_records("wf-1").await.unwrap();
        assert_eq!(all.len(), 3);

        let n0 = store.get_records_for_node("wf-1", "n0").await.unwrap();
        assert_eq!(n0.len(), 2); // i=0 and i=2

        let n1 = store.get_records_for_node("wf-1", "n1").await.unwrap();
        assert_eq!(n1.len(), 1); // i=1
    }

    #[test]
    fn test_approval_context_serialization() {
        let ctx = ApprovalContext {
            node_id: "n1".to_string(),
            action: "deploy".to_string(),
            risk_score: 0.7,
            preview_output: serde_json::json!({"env": "production"}),
            history: vec![],
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: ApprovalContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_id, "n1");
        assert_eq!(deserialized.risk_score, 0.7);
    }

    #[test]
    fn test_approval_policy_serialization() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(600),
            on_timeout: TimeoutAction::Reject,
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ApprovalPolicy = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ApprovalPolicy::HumanReview {
                on_timeout: TimeoutAction::Reject,
                ..
            }
        ));
    }
}
