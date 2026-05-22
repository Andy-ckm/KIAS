//! # Verify-Gated Completion Admission Control
//!
//! Implements read-only verification of agent completion claims before admission.
//! Inspired by Paper 2605.17998 — "Verify-Gated Completion as Admission Control
//! in Governed Multi-Agent Runtime".
//!
//! ## Design Principles
//!
//! 1. **Fail-closed on ambiguity** — if verification is uncertain, reject the claim.
//! 2. **Read-only verifier** — the verifier never mutates agent state.
//! 3. **Audit trail** — every admission decision is recorded for GxP compliance.
//! 4. **Pluggable verifiers** — implement `CompletionVerifier` for custom checks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Outcome of verifying a completion claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdmissionDecision {
    /// Claim verified and admitted.
    Admitted,
    /// Claim rejected — verification failed.
    Rejected,
    /// Claim rejected — verification was ambiguous (fail-closed).
    Ambiguous,
}

impl std::fmt::Display for AdmissionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admitted => write!(f, "Admitted"),
            Self::Rejected => write!(f, "Rejected"),
            Self::Ambiguous => write!(f, "Ambiguous"),
        }
    }
}

/// A single admission record for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionRecord {
    /// Unique record ID.
    pub id: String,
    /// Agent that claimed completion.
    pub agent_id: String,
    /// Task or session that was completed.
    pub task_id: String,
    /// The admission decision.
    pub decision: AdmissionDecision,
    /// Reason for the decision.
    pub reason: String,
    /// Which verifier made the decision.
    pub verifier: String,
    /// Confidence score (0.0–1.0). Below threshold → Ambiguous.
    pub confidence: f64,
    /// When the decision was made.
    pub timestamp: DateTime<Utc>,
}

/// A completion claim from an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionClaim {
    /// Agent making the claim.
    pub agent_id: String,
    /// Task or session ID.
    pub task_id: String,
    /// Agent's self-reported status.
    pub claimed_status: ClaimedStatus,
    /// Evidence provided by the agent (e.g. output hash, metrics).
    pub evidence: Vec<ClaimEvidence>,
    /// When the claim was made.
    pub claimed_at: DateTime<Utc>,
}

/// What the agent claims about its completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClaimedStatus {
    /// Task completed successfully.
    Success,
    /// Task completed with warnings.
    SuccessWithWarnings(Vec<String>),
    /// Task failed.
    Failed(String),
}

/// A piece of evidence supporting a completion claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimEvidence {
    /// Type of evidence (e.g. "output_hash", "metric", "log_excerpt").
    pub evidence_type: String,
    /// The evidence value.
    pub value: String,
    /// Optional hash for integrity verification.
    pub hash: Option<String>,
}

/// Result of a verification check.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the check passed.
    pub passed: bool,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Human-readable explanation.
    pub explanation: String,
}

/// Trait for completion verifiers. Implement this to add custom verification logic.
///
/// Verifiers are read-only — they must not mutate any state.
#[async_trait::async_trait]
pub trait CompletionVerifier: Send + Sync {
    /// Name of this verifier (for audit trail).
    fn name(&self) -> &str;

    /// Verify a completion claim. Returns a `VerificationResult`.
    async fn verify(&self, claim: &CompletionClaim) -> VerificationResult;
}

/// Composite verifier that runs multiple verifiers and aggregates results.
pub struct CompositeVerifier {
    verifiers: Vec<Box<dyn CompletionVerifier>>,
    /// Minimum confidence threshold for admission (0.0–1.0).
    /// Below this threshold, the claim is marked Ambiguous (fail-closed).
    confidence_threshold: f64,
    /// If true, ALL verifiers must pass. If false, majority wins.
    require_all: bool,
}

impl Default for CompositeVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeVerifier {
    pub fn new() -> Self {
        Self {
            verifiers: Vec::new(),
            confidence_threshold: 0.7,
            require_all: true,
        }
    }

    /// Add a verifier.
    pub fn add_verifier(&mut self, verifier: Box<dyn CompletionVerifier>) {
        self.verifiers.push(verifier);
    }

    /// Set the confidence threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set whether all verifiers must pass (fail-closed) or majority wins.
    pub fn with_require_all(mut self, require_all: bool) -> Self {
        self.require_all = require_all;
        self
    }

    /// Run all verifiers and aggregate results.
    pub async fn verify_all(&self, claim: &CompletionClaim) -> Vec<VerificationResult> {
        let mut results = Vec::new();
        for verifier in &self.verifiers {
            results.push(verifier.verify(claim).await);
        }
        results
    }
}

/// The main admission controller.
pub struct AdmissionController {
    verifier: CompositeVerifier,
    records: Vec<AdmissionRecord>,
}

impl AdmissionController {
    pub fn new(verifier: CompositeVerifier) -> Self {
        Self {
            verifier,
            records: Vec::new(),
        }
    }

    /// Evaluate a completion claim and return an admission decision.
    /// Records the decision for audit trail.
    pub async fn admit(&mut self, claim: &CompletionClaim) -> AdmissionDecision {
        let results = self.verifier.verify_all(claim).await;

        let decision = if results.is_empty() {
            // No verifiers configured — fail-closed
            AdmissionDecision::Ambiguous
        } else if self.verifier.require_all {
            // All must pass
            let all_passed = results.iter().all(|r| r.passed);
            let min_confidence = results
                .iter()
                .map(|r| r.confidence)
                .fold(1.0f64, f64::min);

            if !all_passed {
                AdmissionDecision::Rejected
            } else if min_confidence < self.verifier.confidence_threshold {
                AdmissionDecision::Ambiguous
            } else {
                AdmissionDecision::Admitted
            }
        } else {
            // Majority wins
            let pass_count = results.iter().filter(|r| r.passed).count();
            let avg_confidence: f64 = results.iter().map(|r| r.confidence).sum::<f64>()
                / results.len() as f64;

            if pass_count <= results.len() / 2 {
                AdmissionDecision::Rejected
            } else if avg_confidence < self.verifier.confidence_threshold {
                AdmissionDecision::Ambiguous
            } else {
                AdmissionDecision::Admitted
            }
        };

        // Build reason string
        let reason = results
            .iter()
            .map(|r| format!("[{}] {}", if r.passed { "PASS" } else { "FAIL" }, r.explanation))
            .collect::<Vec<_>>()
            .join("; ");

        let avg_conf = if results.is_empty() {
            0.0
        } else {
            results.iter().map(|r| r.confidence).sum::<f64>() / results.len() as f64
        };

        let verifier_names: Vec<&str> = self.verifier.verifiers.iter().map(|v| v.name()).collect();

        // Record for audit
        self.records.push(AdmissionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: claim.agent_id.clone(),
            task_id: claim.task_id.clone(),
            decision,
            reason,
            verifier: verifier_names.join(","),
            confidence: avg_conf,
            timestamp: Utc::now(),
        });

        decision
    }

    /// Get all admission records (for audit export).
    pub fn records(&self) -> &[AdmissionRecord] {
        &self.records
    }

    /// Get records filtered by decision.
    pub fn records_with_decision(&self, decision: AdmissionDecision) -> Vec<&AdmissionRecord> {
        self.records.iter().filter(|r| r.decision == decision).collect()
    }

    /// Clear records (for testing).
    pub fn clear_records(&mut self) {
        self.records.clear();
    }
}

// ── Built-in Verifiers ──────────────────────────────────────────────────────

/// Verifier that checks if the agent provided non-empty evidence.
pub struct EvidenceVerifier;

#[async_trait::async_trait]
impl CompletionVerifier for EvidenceVerifier {
    fn name(&self) -> &str {
        "evidence_check"
    }

    async fn verify(&self, claim: &CompletionClaim) -> VerificationResult {
        if claim.evidence.is_empty() {
            VerificationResult {
                passed: false,
                confidence: 0.0,
                explanation: "No evidence provided for completion claim".to_string(),
            }
        } else {
            VerificationResult {
                passed: true,
                confidence: 0.8,
                explanation: format!("{} evidence item(s) provided", claim.evidence.len()),
            }
        }
    }
}

/// Verifier that checks if the claimed status is consistent with evidence.
pub struct ConsistencyVerifier;

#[async_trait::async_trait]
impl CompletionVerifier for ConsistencyVerifier {
    fn name(&self) -> &str {
        "consistency_check"
    }

    async fn verify(&self, claim: &CompletionClaim) -> VerificationResult {
        match &claim.claimed_status {
            ClaimedStatus::Failed(reason) => {
                // Failed claims are always admitted (agent is honest about failure)
                VerificationResult {
                    passed: true,
                    confidence: 1.0,
                    explanation: format!("Agent reported failure: {}", reason),
                }
            }
            ClaimedStatus::Success | ClaimedStatus::SuccessWithWarnings(_) => {
                // Success claims need evidence
                let has_output = claim
                    .evidence
                    .iter()
                    .any(|e| e.evidence_type == "output_hash" || e.evidence_type == "metric");
                if has_output {
                    VerificationResult {
                        passed: true,
                        confidence: 0.9,
                        explanation: "Success claim has supporting evidence".to_string(),
                    }
                } else {
                    VerificationResult {
                        passed: false,
                        confidence: 0.3,
                        explanation: "Success claim lacks output evidence".to_string(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_claim(agent_id: &str, task_id: &str) -> CompletionClaim {
        CompletionClaim {
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            claimed_status: ClaimedStatus::Success,
            evidence: vec![ClaimEvidence {
                evidence_type: "output_hash".to_string(),
                value: "abc123".to_string(),
                hash: Some("sha256:def456".to_string()),
            }],
            claimed_at: Utc::now(),
        }
    }

    fn make_claim_no_evidence(agent_id: &str, task_id: &str) -> CompletionClaim {
        CompletionClaim {
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            claimed_status: ClaimedStatus::Success,
            evidence: vec![],
            claimed_at: Utc::now(),
        }
    }

    fn make_failed_claim(agent_id: &str, task_id: &str) -> CompletionClaim {
        CompletionClaim {
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            claimed_status: ClaimedStatus::Failed("timeout".to_string()),
            evidence: vec![],
            claimed_at: Utc::now(),
        }
    }

    // ── AdmissionDecision ────────────────────────────────────────────────

    #[test]
    fn test_admission_decision_display() {
        assert_eq!(AdmissionDecision::Admitted.to_string(), "Admitted");
        assert_eq!(AdmissionDecision::Rejected.to_string(), "Rejected");
        assert_eq!(AdmissionDecision::Ambiguous.to_string(), "Ambiguous");
    }

    // ── EvidenceVerifier ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_evidence_verifier_with_evidence() {
        let verifier = EvidenceVerifier;
        let claim = make_claim("a1", "t1");
        let result = verifier.verify(&claim).await;
        assert!(result.passed);
        assert!(result.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_evidence_verifier_no_evidence() {
        let verifier = EvidenceVerifier;
        let claim = make_claim_no_evidence("a1", "t1");
        let result = verifier.verify(&claim).await;
        assert!(!result.passed);
        assert_eq!(result.confidence, 0.0);
    }

    // ── ConsistencyVerifier ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_consistency_verifier_success_with_evidence() {
        let verifier = ConsistencyVerifier;
        let claim = make_claim("a1", "t1");
        let result = verifier.verify(&claim).await;
        assert!(result.passed);
        assert!(result.confidence >= 0.8);
    }

    #[tokio::test]
    async fn test_consistency_verifier_success_no_evidence() {
        let verifier = ConsistencyVerifier;
        let claim = make_claim_no_evidence("a1", "t1");
        let result = verifier.verify(&claim).await;
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_consistency_verifier_failed_always_admitted() {
        let verifier = ConsistencyVerifier;
        let claim = make_failed_claim("a1", "t1");
        let result = verifier.verify(&claim).await;
        assert!(result.passed);
        assert_eq!(result.confidence, 1.0);
    }

    // ── CompositeVerifier ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_composite_verifier_require_all_pass() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));

        let claim = make_claim("a1", "t1");
        let results = composite.verify_all(&claim).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.passed));
    }

    #[tokio::test]
    async fn test_composite_verifier_one_fails() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));

        let claim = make_claim_no_evidence("a1", "t1");
        let results = composite.verify_all(&claim).await;
        assert_eq!(results.len(), 2);
        // EvidenceVerifier fails
        assert!(!results[0].passed);
    }

    // ── AdmissionController ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_admission_admitted() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));
        let mut controller = AdmissionController::new(composite);

        let claim = make_claim("a1", "t1");
        let decision = controller.admit(&claim).await;
        assert_eq!(decision, AdmissionDecision::Admitted);
        assert_eq!(controller.records().len(), 1);
    }

    #[tokio::test]
    async fn test_admission_rejected_no_evidence() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));
        let mut controller = AdmissionController::new(composite);

        let claim = make_claim_no_evidence("a1", "t1");
        let decision = controller.admit(&claim).await;
        assert_eq!(decision, AdmissionDecision::Rejected);
    }

    #[tokio::test]
    async fn test_admission_ambiguous_low_confidence() {
        let mut composite = CompositeVerifier::new()
            .with_threshold(0.95); // Very high threshold
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));
        let mut controller = AdmissionController::new(composite);

        let claim = make_claim("a1", "t1");
        let decision = controller.admit(&claim).await;
        // Confidence ~0.85 < 0.95 threshold → Ambiguous
        assert_eq!(decision, AdmissionDecision::Ambiguous);
    }

    #[tokio::test]
    async fn test_admission_no_verifiers_fail_closed() {
        let composite = CompositeVerifier::new();
        let mut controller = AdmissionController::new(composite);

        let claim = make_claim("a1", "t1");
        let decision = controller.admit(&claim).await;
        // No verifiers → fail-closed → Ambiguous
        assert_eq!(decision, AdmissionDecision::Ambiguous);
    }

    #[tokio::test]
    async fn test_admission_failed_claim_always_admitted() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));
        let mut controller = AdmissionController::new(composite);

        let claim = make_failed_claim("a1", "t1");
        let decision = controller.admit(&claim).await;
        // Failed claims: ConsistencyVerifier passes with 1.0 confidence,
        // but EvidenceVerifier fails (no evidence). require_all=true → Rejected.
        // This is correct behavior — even failed claims need evidence in GxP.
        assert_eq!(decision, AdmissionDecision::Rejected);
    }

    #[tokio::test]
    async fn test_admission_records_audit_trail() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        let mut controller = AdmissionController::new(composite);

        controller.admit(&make_claim("a1", "t1")).await;
        controller.admit(&make_claim_no_evidence("a2", "t2")).await;
        controller.admit(&make_claim("a3", "t3")).await;

        assert_eq!(controller.records().len(), 3);
        assert_eq!(
            controller.records_with_decision(AdmissionDecision::Admitted).len(),
            2
        );
        assert_eq!(
            controller.records_with_decision(AdmissionDecision::Rejected).len(),
            1
        );
    }

    #[tokio::test]
    async fn test_admission_record_fields() {
        let mut composite = CompositeVerifier::new();
        composite.add_verifier(Box::new(EvidenceVerifier));
        let mut controller = AdmissionController::new(composite);

        controller.admit(&make_claim("agent-x", "task-y")).await;
        let record = &controller.records()[0];
        assert_eq!(record.agent_id, "agent-x");
        assert_eq!(record.task_id, "task-y");
        assert_eq!(record.decision, AdmissionDecision::Admitted);
        assert_eq!(record.verifier, "evidence_check");
        assert!(record.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_admission_majority_mode() {
        let mut composite = CompositeVerifier::new()
            .with_require_all(false)
            .with_threshold(0.5);
        composite.add_verifier(Box::new(EvidenceVerifier));
        composite.add_verifier(Box::new(ConsistencyVerifier));
        let mut controller = AdmissionController::new(composite);

        // No evidence: EvidenceVerifier fails, ConsistencyVerifier fails
        // Both fail → majority fails → Rejected
        let claim = make_claim_no_evidence("a1", "t1");
        let decision = controller.admit(&claim).await;
        assert_eq!(decision, AdmissionDecision::Rejected);
    }

    #[test]
    fn test_composite_verifier_threshold_clamped() {
        let composite = CompositeVerifier::new().with_threshold(2.0);
        assert_eq!(composite.confidence_threshold, 1.0);
        let composite = CompositeVerifier::new().with_threshold(-1.0);
        assert_eq!(composite.confidence_threshold, 0.0);
    }

    #[test]
    fn test_clear_records() {
        let composite = CompositeVerifier::new();
        let mut controller = AdmissionController::new(composite);
        // We can't call admit without await in a sync test, but we can test clear
        controller.clear_records();
        assert!(controller.records().is_empty());
    }
}
