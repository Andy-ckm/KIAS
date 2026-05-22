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
    /// Degrade to a safe fallback path on timeout.
    /// The workflow continues but routes to the specified safe path.
    DegradeTo {
        /// The fallback path/route to take when timeout occurs.
        fallback_path: String,
    },
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

// ─── TimeoutDegradation ─────────────────────────────────────────────────

/// Result of a timeout-driven degradation, indicating the safe path the
/// workflow should follow after an approval timeout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutDegradation {
    /// The original approval record id that timed out.
    pub record_id: String,
    /// The fallback path chosen by the degradation policy.
    pub fallback_path: String,
    /// Timestamp when the degradation was triggered.
    pub triggered_at: DateTime<Utc>,
}

/// Evaluate what happens when an approval times out, returning the appropriate
/// timeout action and, if degradation is configured, a [`TimeoutDegradation`].
pub fn evaluate_timeout(
    policy: &ApprovalPolicy,
    record_id: &str,
) -> (TimeoutAction, Option<TimeoutDegradation>) {
    match policy {
        ApprovalPolicy::HumanReview { on_timeout, .. } => match on_timeout {
            TimeoutAction::DegradeTo { fallback_path } => {
                let degradation = TimeoutDegradation {
                    record_id: record_id.to_string(),
                    fallback_path: fallback_path.clone(),
                    triggered_at: Utc::now(),
                };
                (on_timeout.clone(), Some(degradation))
            }
            other => (other.clone(), None),
        },
        _ => (TimeoutAction::Reject, None),
    }
}

// ─── ApprovalHistoryTracker ─────────────────────────────────────────────

/// Tracks the multi-level approval history for a single workflow.
/// Each level can be reviewed by a different approver; the tracker
/// accumulates records and provides convenience queries.
#[derive(Debug, Default, Clone)]
pub struct ApprovalHistoryTracker {
    records: Vec<ApprovalRecord>,
}

impl ApprovalHistoryTracker {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Add a new approval record to the tracker.
    pub fn push(&mut self, record: ApprovalRecord) {
        self.records.push(record);
    }

    /// Get all records sorted by approval level.
    pub fn by_level(&self) -> Vec<&ApprovalRecord> {
        let mut sorted: Vec<_> = self.records.iter().collect();
        sorted.sort_by_key(|r| r.approver_level.unwrap_or(0));
        sorted
    }

    /// Get the latest decision (by level).
    pub fn latest_decision(&self) -> Option<&ApprovalRecord> {
        self.by_level().into_iter().next_back()
    }

    /// Check if any level has rejected.
    pub fn has_rejection(&self) -> bool {
        self.records
            .iter()
            .any(|r| matches!(r.decision, ApprovalDecision::Rejected { .. }))
    }

    /// Total number of approval levels recorded.
    pub fn level_count(&self) -> usize {
        self.records.len()
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
    /// Pre-review preview: a human-readable summary shown before the reviewer
    /// inspects the full output.  Inspired by CrewAI HITL pre-review.
    #[serde(default)]
    pub pre_review_preview: Option<String>,
    /// Whether the reviewer has confirmed the distilled/knowledge summary
    /// before proceeding.  Used in RAG / distillation pipelines.
    #[serde(default)]
    pub distillation_confirm: Option<bool>,
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
    /// The approval level/tier (0 = first reviewer, 1 = escalation, …).
    #[serde(default)]
    pub approver_level: Option<u32>,
    /// Free-text opinion from the reviewer.
    #[serde(default)]
    pub opinion: Option<String>,
    /// Structured decision reasoning for audit.
    #[serde(default)]
    pub decision_reason: Option<String>,
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
            pre_review_preview: None,
            distillation_confirm: None,
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
            approver_level: Some(0),
            opinion: None,
            decision_reason: None,
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
                approver_level: Some(i as u32),
                opinion: None,
                decision_reason: None,
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
            pre_review_preview: None,
            distillation_confirm: None,
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

    // ── New enhanced tests ────────────────────────────────────────────────────

    #[test]
    fn test_timeout_degradation_with_fallback_path() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(60),
            on_timeout: TimeoutAction::DegradeTo {
                fallback_path: "safe-mode".to_string(),
            },
        };

        let (action, degradation) = evaluate_timeout(&policy, "rec-42");
        assert!(matches!(action, TimeoutAction::DegradeTo { .. }));
        let deg = degradation.expect("expected degradation record");
        assert_eq!(deg.record_id, "rec-42");
        assert_eq!(deg.fallback_path, "safe-mode");
    }

    #[test]
    fn test_timeout_evaluation_reject_on_non_human_review_policy() {
        // For non-HumanReview policies, timeout evaluation returns Reject
        let policy = ApprovalPolicy::Always;
        let (action, degradation) = evaluate_timeout(&policy, "rec-1");
        assert_eq!(action, TimeoutAction::Reject);
        assert!(degradation.is_none());

        let policy2 = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let (action2, deg2) = evaluate_timeout(&policy2, "rec-2");
        assert_eq!(action2, TimeoutAction::Reject);
        assert!(deg2.is_none());
    }

    #[test]
    fn test_timeout_evaluation_human_review_approve_on_timeout() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(300),
            on_timeout: TimeoutAction::Approve,
        };
        let (action, degradation) = evaluate_timeout(&policy, "rec-3");
        assert_eq!(action, TimeoutAction::Approve);
        assert!(degradation.is_none());
    }

    #[test]
    fn test_timeout_evaluation_human_review_reject_on_timeout() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(300),
            on_timeout: TimeoutAction::Reject,
        };
        let (action, degradation) = evaluate_timeout(&policy, "rec-4");
        assert_eq!(action, TimeoutAction::Reject);
        assert!(degradation.is_none());
    }

    #[test]
    fn test_approval_history_tracker_multi_level() {
        let mut tracker = ApprovalHistoryTracker::new();

        tracker.push(ApprovalRecord {
            id: "ar-0".to_string(),
            workflow_id: "wf-x".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.1),
            decision: ApprovalDecision::Approved {
                comment: Some("L0 OK".into()),
            },
            decided_by: "reviewer-0".to_string(),
            decided_at: Utc::now(),
            duration_ms: 1000,
            approver_level: Some(0),
            opinion: Some("looks fine".into()),
            decision_reason: Some("low risk".into()),
        });

        tracker.push(ApprovalRecord {
            id: "ar-1".to_string(),
            workflow_id: "wf-x".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.6),
            decision: ApprovalDecision::Approved {
                comment: Some("escalation approved".into()),
            },
            decided_by: "reviewer-1".to_string(),
            decided_at: Utc::now(),
            duration_ms: 2000,
            approver_level: Some(1),
            opinion: Some("acceptable with caution".into()),
            decision_reason: Some("medium risk, proceed".into()),
        });

        assert_eq!(tracker.level_count(), 2);
        assert!(!tracker.has_rejection());

        let latest = tracker.latest_decision().expect("should have latest");
        assert_eq!(latest.approver_level, Some(1));
        assert_eq!(latest.opinion.as_deref(), Some("acceptable with caution"));
    }

    #[test]
    fn test_approval_history_tracker_rejection_detection() {
        let mut tracker = ApprovalHistoryTracker::new();

        tracker.push(ApprovalRecord {
            id: "ar-rej".to_string(),
            workflow_id: "wf-rej".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.9),
            decision: ApprovalDecision::Rejected {
                reason: "too dangerous".into(),
            },
            decided_by: "senior-reviewer".to_string(),
            decided_at: Utc::now(),
            duration_ms: 500,
            approver_level: Some(0),
            opinion: Some("rejecting".into()),
            decision_reason: Some("risk too high".into()),
        });

        assert!(tracker.has_rejection());
        assert_eq!(tracker.level_count(), 1);
    }

    #[test]
    fn test_approval_history_tracker_by_level_ordering() {
        let mut tracker = ApprovalHistoryTracker::new();

        // Insert out of order
        tracker.push(ApprovalRecord {
            id: "ar-2".to_string(),
            workflow_id: "wf-order".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.5),
            decision: ApprovalDecision::Approved { comment: None },
            decided_by: "auto".to_string(),
            decided_at: Utc::now(),
            duration_ms: 100,
            approver_level: Some(2),
            opinion: None,
            decision_reason: None,
        });

        tracker.push(ApprovalRecord {
            id: "ar-0".to_string(),
            workflow_id: "wf-order".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.1),
            decision: ApprovalDecision::Approved { comment: None },
            decided_by: "auto".to_string(),
            decided_at: Utc::now(),
            duration_ms: 100,
            approver_level: Some(0),
            opinion: None,
            decision_reason: None,
        });

        tracker.push(ApprovalRecord {
            id: "ar-1".to_string(),
            workflow_id: "wf-order".to_string(),
            node_id: "n1".to_string(),
            context: make_ctx("n1", 0.3),
            decision: ApprovalDecision::Approved { comment: None },
            decided_by: "auto".to_string(),
            decided_at: Utc::now(),
            duration_ms: 100,
            approver_level: Some(1),
            opinion: None,
            decision_reason: None,
        });

        let sorted = tracker.by_level();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].approver_level, Some(0));
        assert_eq!(sorted[1].approver_level, Some(1));
        assert_eq!(sorted[2].approver_level, Some(2));
    }

    #[test]
    fn test_approval_context_with_pre_review_and_distillation() {
        let ctx = ApprovalContext {
            node_id: "n-rag".to_string(),
            action: "rag_retrieve".to_string(),
            risk_score: 0.2,
            preview_output: serde_json::json!({"documents": 5, "status": "ok"}),
            history: vec![],
            pre_review_preview: Some("Retrieved 5 relevant docs about X".to_string()),
            distillation_confirm: Some(true),
        };

        assert!(ctx.pre_review_preview.is_some());
        assert_eq!(
            ctx.pre_review_preview.as_deref().unwrap(),
            "Retrieved 5 relevant docs about X"
        );
        assert_eq!(ctx.distillation_confirm, Some(true));

        // Verify serialization roundtrip
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: ApprovalContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pre_review_preview, ctx.pre_review_preview);
        assert_eq!(deserialized.distillation_confirm, ctx.distillation_confirm);
    }

    #[test]
    fn test_auto_approve_multiple_conditions_all_match() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![
                ApprovalCondition {
                    field: "status".to_string(),
                    expected: serde_json::json!("ok"),
                },
                ApprovalCondition {
                    field: "severity".to_string(),
                    expected: serde_json::json!("low"),
                },
            ],
        };
        let ctx = ApprovalContext {
            node_id: "n-multi".to_string(),
            action: "scan".to_string(),
            risk_score: 0.0,
            preview_output: serde_json::json!({"status": "ok", "severity": "low", "count": 3}),
            history: vec![],
            pre_review_preview: None,
            distillation_confirm: None,
        };

        let result = evaluate_policy(&policy, &ctx);
        assert!(matches!(result, ApprovalEvaluation::AutoApproved(_)));
    }

    #[test]
    fn test_auto_approve_multiple_conditions_partial_match() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![
                ApprovalCondition {
                    field: "status".to_string(),
                    expected: serde_json::json!("ok"),
                },
                ApprovalCondition {
                    field: "severity".to_string(),
                    expected: serde_json::json!("low"),
                },
            ],
        };
        let ctx = ApprovalContext {
            node_id: "n-partial".to_string(),
            action: "scan".to_string(),
            risk_score: 0.0,
            preview_output: serde_json::json!({"status": "ok", "severity": "high", "count": 3}),
            history: vec![],
            pre_review_preview: None,
            distillation_confirm: None,
        };

        // severity is "high" != "low", so should require human review
        let result = evaluate_policy(&policy, &ctx);
        assert!(matches!(result, ApprovalEvaluation::RequiresHumanReview));
    }

    #[test]
    fn test_threshold_auto_approved_with_exact_threshold_boundary() {
        // Risk exactly at threshold → below threshold is false → requires review
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n-boundary", 0.499999);
        let result = evaluate_policy(&policy, &ctx);
        assert!(matches!(result, ApprovalEvaluation::AutoApproved(_)));

        let ctx2 = make_ctx("n-boundary2", 0.5);
        let result2 = evaluate_policy(&policy, &ctx2);
        assert!(matches!(result2, ApprovalEvaluation::RequiresHumanReview));
    }

    #[test]
    fn test_timeout_degradation_triggered_at_is_set() {
        let policy = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(10),
            on_timeout: TimeoutAction::DegradeTo {
                fallback_path: "fallback-v2".to_string(),
            },
        };
        let before = Utc::now();
        let (_, degradation) = evaluate_timeout(&policy, "rec-timing");
        let after = Utc::now();

        let deg = degradation.expect("degradation must be present");
        assert!(deg.triggered_at >= before && deg.triggered_at <= after);
    }

    // ── New enhanced HITL tests ─────────────────────────────────────────────────

    #[test]
    fn test_approval_policy_threshold_boundary_exactly_at() {
        // Exact threshold → below threshold check fails → requires human review
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n-boundary", 0.5);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_approval_policy_threshold_boundary_just_below() {
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.5,
        };
        let ctx = make_ctx("n-boundary", 0.4999);
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::AutoApproved(_)
        ));
    }

    #[test]
    fn test_approval_policy_autoapprove_empty_conditions() {
        // Empty conditions → all conditions match vacuously → auto-approve
        let policy = ApprovalPolicy::AutoApprove { conditions: vec![] };
        let ctx = make_ctx("n-empty", 0.0);
        let result = evaluate_policy(&policy, &ctx);
        assert!(matches!(result, ApprovalEvaluation::AutoApproved(_)));
    }

    #[test]
    fn test_approval_policy_autoapprove_multiple_conditions_all_match() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![
                ApprovalCondition {
                    field: "status".into(),
                    expected: serde_json::json!("ok"),
                },
                ApprovalCondition {
                    field: "verified".into(),
                    expected: serde_json::json!(true),
                },
            ],
        };
        let ctx = ApprovalContext {
            node_id: "n-multi".into(),
            action: "multi-check".into(),
            risk_score: 0.0,
            preview_output: serde_json::json!({"status": "ok", "verified": true, "extra": "field"}),
            history: vec![],
            pre_review_preview: None,
            distillation_confirm: None,
        };
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::AutoApproved(_)
        ));
    }

    #[test]
    fn test_approval_policy_autoapprove_multiple_conditions_one_mismatch() {
        let policy = ApprovalPolicy::AutoApprove {
            conditions: vec![
                ApprovalCondition {
                    field: "status".into(),
                    expected: serde_json::json!("ok"),
                },
                ApprovalCondition {
                    field: "verified".into(),
                    expected: serde_json::json!(true),
                },
            ],
        };
        let ctx = ApprovalContext {
            node_id: "n-partial".into(),
            action: "multi-check".into(),
            risk_score: 0.0,
            preview_output: serde_json::json!({"status": "ok", "verified": false}),
            history: vec![],
            pre_review_preview: None,
            distillation_confirm: None,
        };
        // verified=false != true → requires human review
        assert!(matches!(
            evaluate_policy(&policy, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_approval_record_full_fields_serialization() {
        let record = ApprovalRecord {
            id: "ar-full".into(),
            workflow_id: "wf-full".into(),
            node_id: "n-full".into(),
            context: make_ctx("n-full", 0.3),
            decision: ApprovalDecision::Approved {
                comment: Some("LGTM".into()),
            },
            decided_by: "senior-reviewer".into(),
            decided_at: Utc::now(),
            duration_ms: 3500,
            approver_level: Some(2),
            opinion: Some("looks good after second look".into()),
            decision_reason: Some("all checks passed".into()),
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ApprovalRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "ar-full");
        assert_eq!(deserialized.approver_level, Some(2));
        assert_eq!(
            deserialized.opinion.as_deref(),
            Some("looks good after second look")
        );
        assert_eq!(
            deserialized.decision_reason.as_deref(),
            Some("all checks passed")
        );
    }

    #[test]
    fn test_approval_context_empty_history() {
        let ctx = ApprovalContext {
            node_id: "n-new".into(),
            action: "new-action".into(),
            risk_score: 0.1,
            preview_output: serde_json::json!({"result": "ok"}),
            history: vec![],
            pre_review_preview: None,
            distillation_confirm: None,
        };
        assert!(ctx.history.is_empty());
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: ApprovalContext = serde_json::from_str(&json).unwrap();
        assert!(deserialized.history.is_empty());
    }

    #[test]
    fn test_approval_context_with_full_history() {
        let prior = ApprovalRecord {
            id: "ar-prior".into(),
            workflow_id: "wf-hist".into(),
            node_id: "n-hist".into(),
            context: make_ctx("n-hist", 0.2),
            decision: ApprovalDecision::Approved {
                comment: Some("prior approved".into()),
            },
            decided_by: "prior-reviewer".into(),
            decided_at: Utc::now(),
            duration_ms: 1000,
            approver_level: Some(0),
            opinion: None,
            decision_reason: None,
        };
        let ctx = ApprovalContext {
            node_id: "n-hist".into(),
            action: "hist-action".into(),
            risk_score: 0.5,
            preview_output: serde_json::json!({"result": "updated"}),
            history: vec![prior],
            pre_review_preview: None,
            distillation_confirm: None,
        };
        assert_eq!(ctx.history.len(), 1);
        assert_eq!(ctx.history[0].approver_level, Some(0));
    }

    #[test]
    fn test_timeout_action_serde_roundtrip() {
        let actions = vec![
            TimeoutAction::Reject,
            TimeoutAction::Approve,
            TimeoutAction::DegradeTo {
                fallback_path: "safe".into(),
            },
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let decoded: TimeoutAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, decoded);
        }
    }

    #[test]
    fn test_approval_evaluation_auto_approved_comment() {
        let policy = ApprovalPolicy::Threshold {
            risk_threshold: 0.9,
        };
        let ctx = make_ctx("n-low-risk", 0.1);
        let result = evaluate_policy(&policy, &ctx);
        if let ApprovalEvaluation::AutoApproved(ApprovalDecision::Approved { comment }) = result {
            assert!(comment.is_some());
            assert!(comment.unwrap().contains("Auto-approved"));
        } else {
            panic!("Expected AutoApproved");
        }
    }

    #[test]
    fn test_human_review_policy_always_requires_review() {
        // Any HumanReview policy regardless of timeout or on_timeout always returns RequiresHumanReview
        let policy1 = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(1),
            on_timeout: TimeoutAction::Approve,
        };
        let ctx = make_ctx("n1", 0.0);
        assert!(matches!(
            evaluate_policy(&policy1, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));

        let policy2 = ApprovalPolicy::HumanReview {
            timeout: Duration::from_secs(9999),
            on_timeout: TimeoutAction::DegradeTo {
                fallback_path: "x".into(),
            },
        };
        assert!(matches!(
            evaluate_policy(&policy2, &ctx),
            ApprovalEvaluation::RequiresHumanReview
        ));
    }

    #[test]
    fn test_always_policy_regardless_of_context() {
        // Always policy requires human regardless of risk_score or output
        let policy = ApprovalPolicy::Always;
        for risk in [0.0, 0.3, 0.5, 0.9, 1.0] {
            let ctx = make_ctx("n-any", risk);
            assert!(matches!(
                evaluate_policy(&policy, &ctx),
                ApprovalEvaluation::RequiresHumanReview
            ));
        }
    }

    #[test]
    fn test_approval_history_tracker_latest_decision_returns_highest_level() {
        let mut tracker = ApprovalHistoryTracker::new();
        tracker.push(ApprovalRecord {
            id: "ar-l0".into(),
            workflow_id: "wf".into(),
            node_id: "n".into(),
            context: make_ctx("n", 0.1),
            decision: ApprovalDecision::Approved { comment: None },
            decided_by: "auto".into(),
            decided_at: Utc::now(),
            duration_ms: 100,
            approver_level: Some(0),
            opinion: None,
            decision_reason: None,
        });
        tracker.push(ApprovalRecord {
            id: "ar-l1".into(),
            workflow_id: "wf".into(),
            node_id: "n".into(),
            context: make_ctx("n", 0.5),
            decision: ApprovalDecision::Approved {
                comment: Some("escalated".into()),
            },
            decided_by: "senior".into(),
            decided_at: Utc::now(),
            duration_ms: 200,
            approver_level: Some(1),
            opinion: None,
            decision_reason: None,
        });
        tracker.push(ApprovalRecord {
            id: "ar-l2".into(),
            workflow_id: "wf".into(),
            node_id: "n".into(),
            context: make_ctx("n", 0.8),
            decision: ApprovalDecision::Approved {
                comment: Some("final".into()),
            },
            decided_by: "admin".into(),
            decided_at: Utc::now(),
            duration_ms: 300,
            approver_level: Some(2),
            opinion: None,
            decision_reason: None,
        });

        let latest = tracker.latest_decision().expect("must have latest");
        assert_eq!(latest.approver_level, Some(2));
        assert_eq!(latest.opinion.as_deref(), None); // opinion is None, decision comment is "final"
                                                     // Check via decision
        if let ApprovalDecision::Approved { comment } = &latest.decision {
            assert_eq!(comment.as_deref(), Some("final"));
        } else {
            panic!("Expected Approved decision");
        }
    }

    #[test]
    fn test_approval_history_tracker_empty_returns_none() {
        let tracker = ApprovalHistoryTracker::new();
        assert!(tracker.latest_decision().is_none());
        assert!(!tracker.has_rejection());
        assert_eq!(tracker.level_count(), 0);
    }
    //
    //     #[test]
    //     fn test_persist_config_auto_save() {
    //         let config = crate::flow_persistence::persist("test-flow").with_auto_save();
    //         assert!(config.auto_save);
    //         assert_eq!(config.key, "test-flow");
    //     }
    //
    //     #[test]
    //     fn test_persist_config_version() {
    //         let config = crate::flow_persistence::persist("test-flow").with_version("3.0.0");
    //         assert_eq!(config.version, Some("3.0.0".to_string()));
    //     }
}
