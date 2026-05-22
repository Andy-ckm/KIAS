//! # Evidence-Gated Skill Updates
//!
//! Implements a SkillsVote-inspired mechanism where skill updates require
//! sufficient evidence before being accepted. Prevents flapping between
//! versions and ensures updates are validated.
//!
//! ## Evidence Types
//!
//! - **Test pass**: Skill's test suite passes
//! - **Validation**: Output matches expected schema
//! - **Usage success**: Successful execution in production
//! - **Peer review**: Human or agent review approved
//! - **Performance**: No regression in latency/cost
//!
//! ## Gating Logic
//!
//! An update is accepted when: `evidence_score >= required_threshold`
//!
//! ```text
//! evidence_score = Σ(evidence.weight * evidence.confidence) / Σ(evidence.weight)
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===========================================================================
// Evidence Types
// ===========================================================================

/// Type of evidence supporting a skill update
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Skill's automated test suite passed
    TestPass,
    /// Output validated against expected schema
    SchemaValidation,
    /// Successful production execution
    UsageSuccess,
    /// Human or agent peer review approved
    PeerReview,
    /// No performance regression detected
    PerformanceBaseline,
    /// Security scan passed
    SecurityScan,
    /// Custom evidence type
    Custom(String),
}

impl EvidenceType {
    /// Default weight for this evidence type
    pub fn default_weight(&self) -> f64 {
        match self {
            Self::TestPass => 1.0,
            Self::SchemaValidation => 0.8,
            Self::UsageSuccess => 0.9,
            Self::PeerReview => 1.0,
            Self::PerformanceBaseline => 0.7,
            Self::SecurityScan => 0.8,
            Self::Custom(_) => 0.5,
        }
    }
}

/// A single piece of evidence supporting a skill update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Type of evidence
    pub evidence_type: EvidenceType,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f64,
    /// Weight override (uses default_weight if None)
    pub weight: Option<f64>,
    /// Who/what produced this evidence
    pub source: String,
    /// Human-readable description
    pub description: String,
    /// When the evidence was collected
    pub timestamp: DateTime<Utc>,
    /// Optional structured data (test results, metrics, etc.)
    pub data: Option<serde_json::Value>,
}

impl Evidence {
    pub fn new(evidence_type: EvidenceType, confidence: f64, source: impl Into<String>) -> Self {
        Self {
            evidence_type,
            confidence: confidence.clamp(0.0, 1.0),
            weight: None,
            source: source.into(),
            description: String::new(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Effective weight (explicit override or default)
    pub fn effective_weight(&self) -> f64 {
        self.weight
            .unwrap_or_else(|| self.evidence_type.default_weight())
    }

    /// Weighted score contribution
    pub fn weighted_score(&self) -> f64 {
        self.effective_weight() * self.confidence
    }
}

// ===========================================================================
// Update Proposal
// ===========================================================================

/// Status of a skill update proposal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Collecting evidence
    Pending,
    /// Evidence threshold met, update accepted
    Accepted,
    /// Evidence threshold not met within deadline
    Rejected,
    /// Manually approved/rejected by operator
    ManualOverride,
}

/// A proposed skill update pending evidence collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProposal {
    /// Unique proposal ID
    pub id: String,
    /// Skill name being updated
    pub skill_name: String,
    /// Current version
    pub from_version: String,
    /// Proposed new version
    pub to_version: String,
    /// Description of changes
    pub change_description: String,
    /// Required evidence threshold (0.0 - 1.0)
    pub required_threshold: f64,
    /// Collected evidence
    pub evidence: Vec<Evidence>,
    /// Current status
    pub status: ProposalStatus,
    /// When the proposal was created
    pub created_at: DateTime<Utc>,
    /// Deadline for evidence collection
    pub deadline: DateTime<Utc>,
    /// Who proposed the update
    pub proposer: String,
}

impl UpdateProposal {
    pub fn new(
        id: impl Into<String>,
        skill_name: impl Into<String>,
        from_version: impl Into<String>,
        to_version: impl Into<String>,
        required_threshold: f64,
        deadline: DateTime<Utc>,
        proposer: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            skill_name: skill_name.into(),
            from_version: from_version.into(),
            to_version: to_version.into(),
            change_description: String::new(),
            required_threshold: required_threshold.clamp(0.0, 1.0),
            evidence: Vec::new(),
            status: ProposalStatus::Pending,
            created_at: Utc::now(),
            deadline,
            proposer: proposer.into(),
        }
    }

    pub fn with_change_description(mut self, desc: impl Into<String>) -> Self {
        self.change_description = desc.into();
        self
    }

    /// Add evidence to this proposal
    pub fn add_evidence(&mut self, evidence: Evidence) {
        if self.status == ProposalStatus::Pending {
            self.evidence.push(evidence);
        }
    }

    /// Compute the aggregate evidence score
    pub fn evidence_score(&self) -> f64 {
        if self.evidence.is_empty() {
            return 0.0;
        }
        let total_weight: f64 = self.evidence.iter().map(|e| e.effective_weight()).sum();
        if total_weight == 0.0 {
            return 0.0;
        }
        let weighted_sum: f64 = self.evidence.iter().map(|e| e.weighted_score()).sum();
        weighted_sum / total_weight
    }

    /// Check if evidence threshold is met
    pub fn is_threshold_met(&self) -> bool {
        self.evidence_score() >= self.required_threshold
    }

    /// Check if the proposal has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.deadline
    }

    /// Try to finalize the proposal (call after adding evidence)
    pub fn try_finalize(&mut self) -> &ProposalStatus {
        if self.status != ProposalStatus::Pending {
            return &self.status;
        }

        if self.is_threshold_met() {
            self.status = ProposalStatus::Accepted;
        } else if self.is_expired() {
            self.status = ProposalStatus::Rejected;
        }

        &self.status
    }

    /// Get evidence breakdown by type
    pub fn evidence_by_type(&self) -> HashMap<String, Vec<&Evidence>> {
        let mut by_type: HashMap<String, Vec<&Evidence>> = HashMap::new();
        for e in &self.evidence {
            let key = format!("{:?}", e.evidence_type);
            by_type.entry(key).or_default().push(e);
        }
        by_type
    }
}

// ===========================================================================
// Evidence Gate
// ===========================================================================

/// Configuration for the evidence gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceGateConfig {
    /// Default evidence threshold (0.0 - 1.0)
    pub default_threshold: f64,
    /// Default deadline duration (seconds)
    pub default_deadline_secs: i64,
    /// Minimum number of evidence items required
    pub min_evidence_count: usize,
    /// Minimum number of distinct evidence types required
    pub min_evidence_types: usize,
    /// Auto-reject on deadline (vs keep pending)
    pub auto_reject_on_deadline: bool,
}

impl Default for EvidenceGateConfig {
    fn default() -> Self {
        Self {
            default_threshold: 0.7,
            default_deadline_secs: 3600, // 1 hour
            min_evidence_count: 2,
            min_evidence_types: 2,
            auto_reject_on_deadline: true,
        }
    }
}

/// Evidence gate — manages update proposals and evidence collection
pub struct EvidenceGate {
    config: EvidenceGateConfig,
    proposals: HashMap<String, UpdateProposal>,
    /// History of completed proposals
    history: Vec<UpdateProposal>,
}

impl EvidenceGate {
    pub fn new(config: EvidenceGateConfig) -> Self {
        Self {
            config,
            proposals: HashMap::new(),
            history: Vec::new(),
        }
    }

    /// Create a new update proposal
    pub fn propose(
        &mut self,
        id: impl Into<String>,
        skill_name: impl Into<String>,
        from_version: impl Into<String>,
        to_version: impl Into<String>,
    ) -> String {
        let id_str = id.into();
        let deadline = Utc::now() + chrono::Duration::seconds(self.config.default_deadline_secs);

        let proposal = UpdateProposal::new(
            &id_str,
            skill_name,
            from_version,
            to_version,
            self.config.default_threshold,
            deadline,
            "system",
        );

        self.proposals.insert(id_str.clone(), proposal);
        id_str
    }

    /// Add evidence to a proposal
    pub fn add_evidence(&mut self, proposal_id: &str, evidence: Evidence) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("Proposal '{proposal_id}' not found"))?;

        if proposal.status != ProposalStatus::Pending {
            return Err(format!("Proposal is already {:?}", proposal.status));
        }

        proposal.add_evidence(evidence);
        Ok(())
    }

    /// Try to finalize a proposal
    pub fn finalize(&mut self, proposal_id: &str) -> Result<ProposalStatus, String> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("Proposal '{proposal_id}' not found"))?;

        // Check minimum evidence requirements
        let unique_types: std::collections::HashSet<String> = proposal
            .evidence
            .iter()
            .map(|e| format!("{:?}", e.evidence_type))
            .collect();

        if proposal.evidence.len() < self.config.min_evidence_count {
            return Err(format!(
                "Need at least {} evidence items (have {})",
                self.config.min_evidence_count,
                proposal.evidence.len()
            ));
        }

        if unique_types.len() < self.config.min_evidence_types {
            return Err(format!(
                "Need at least {} distinct evidence types (have {})",
                self.config.min_evidence_types,
                unique_types.len()
            ));
        }

        proposal.try_finalize();
        let status = proposal.status.clone();

        // Move to history if finalized
        if status != ProposalStatus::Pending {
            if let Some(p) = self.proposals.remove(proposal_id) {
                self.history.push(p);
            }
        }

        Ok(status)
    }

    /// Manually approve a proposal
    pub fn manual_approve(
        &mut self,
        proposal_id: &str,
        approver: impl Into<String>,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("Proposal '{proposal_id}' not found"))?;

        proposal.status = ProposalStatus::ManualOverride;
        proposal
            .evidence
            .push(Evidence::new(EvidenceType::PeerReview, 1.0, approver));

        if let Some(p) = self.proposals.remove(proposal_id) {
            self.history.push(p);
        }
        Ok(())
    }

    /// Get a proposal by ID
    pub fn get_proposal(&self, proposal_id: &str) -> Option<&UpdateProposal> {
        self.proposals.get(proposal_id)
    }

    /// List all pending proposals
    pub fn pending_proposals(&self) -> Vec<&UpdateProposal> {
        self.proposals.values().collect()
    }

    /// Get proposal history
    pub fn history(&self) -> &[UpdateProposal] {
        &self.history
    }

    /// Get statistics
    pub fn stats(&self) -> EvidenceGateStats {
        let accepted = self
            .history
            .iter()
            .filter(|p| p.status == ProposalStatus::Accepted)
            .count();
        let rejected = self
            .history
            .iter()
            .filter(|p| p.status == ProposalStatus::Rejected)
            .count();
        let overridden = self
            .history
            .iter()
            .filter(|p| p.status == ProposalStatus::ManualOverride)
            .count();

        EvidenceGateStats {
            pending_count: self.proposals.len(),
            accepted_count: accepted,
            rejected_count: rejected,
            manual_override_count: overridden,
            total_historical: self.history.len(),
        }
    }

    /// Expire overdue proposals
    pub fn expire_overdue(&mut self) -> usize {
        if !self.config.auto_reject_on_deadline {
            return 0;
        }

        let expired_ids: Vec<String> = self
            .proposals
            .values()
            .filter(|p| p.is_expired() && p.status == ProposalStatus::Pending)
            .map(|p| p.id.clone())
            .collect();

        let count = expired_ids.len();
        for id in expired_ids {
            if let Some(mut proposal) = self.proposals.remove(&id) {
                proposal.status = ProposalStatus::Rejected;
                self.history.push(proposal);
            }
        }
        count
    }
}

/// Statistics for the evidence gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceGateStats {
    pub pending_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub manual_override_count: usize,
    pub total_historical: usize,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> EvidenceGateConfig {
        EvidenceGateConfig {
            default_threshold: 0.7,
            default_deadline_secs: 3600,
            min_evidence_count: 2,
            min_evidence_types: 2,
            auto_reject_on_deadline: true,
        }
    }

    // --- Evidence Tests ---

    #[test]
    fn test_evidence_weighted_score() {
        let e = Evidence::new(EvidenceType::TestPass, 0.9, "ci");
        assert!((e.weighted_score() - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_evidence_custom_weight() {
        let e = Evidence::new(EvidenceType::Custom("x".into()), 0.8, "test").with_weight(2.0);
        assert!((e.weighted_score() - 1.6).abs() < 0.01);
    }

    #[test]
    fn test_evidence_clamp_confidence() {
        let e = Evidence::new(EvidenceType::TestPass, 1.5, "ci");
        assert_eq!(e.confidence, 1.0);

        let e2 = Evidence::new(EvidenceType::TestPass, -0.1, "ci");
        assert_eq!(e2.confidence, 0.0);
    }

    #[test]
    fn test_evidence_default_weights() {
        assert_eq!(EvidenceType::TestPass.default_weight(), 1.0);
        assert_eq!(EvidenceType::SchemaValidation.default_weight(), 0.8);
        assert_eq!(EvidenceType::UsageSuccess.default_weight(), 0.9);
        assert_eq!(EvidenceType::PeerReview.default_weight(), 1.0);
        assert_eq!(EvidenceType::PerformanceBaseline.default_weight(), 0.7);
        assert_eq!(EvidenceType::SecurityScan.default_weight(), 0.8);
        assert_eq!(EvidenceType::Custom("x".into()).default_weight(), 0.5);
    }

    // --- Proposal Tests ---

    #[test]
    fn test_proposal_evidence_score_empty() {
        let p = UpdateProposal::new("p1", "skill", "1.0", "2.0", 0.7, Utc::now(), "sys");
        assert_eq!(p.evidence_score(), 0.0);
    }

    #[test]
    fn test_proposal_evidence_score_weighted() {
        let mut p = UpdateProposal::new("p1", "skill", "1.0", "2.0", 0.7, Utc::now(), "sys");
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.9, "ci"));
        p.add_evidence(Evidence::new(EvidenceType::UsageSuccess, 0.8, "prod"));

        let score = p.evidence_score();
        // (1.0*0.9 + 0.9*0.8) / (1.0 + 0.9) = 1.62 / 1.9 ≈ 0.853
        assert!(score > 0.8);
        assert!(score < 0.9);
    }

    #[test]
    fn test_proposal_threshold_met() {
        let mut p = UpdateProposal::new("p1", "skill", "1.0", "2.0", 0.5, Utc::now(), "sys");
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.8, "ci"));
        p.add_evidence(Evidence::new(EvidenceType::PeerReview, 0.9, "reviewer"));
        assert!(p.is_threshold_met());
    }

    #[test]
    fn test_proposal_threshold_not_met() {
        let mut p = UpdateProposal::new("p1", "skill", "1.0", "2.0", 0.9, Utc::now(), "sys");
        p.add_evidence(Evidence::new(
            EvidenceType::Custom("weak".into()),
            0.3,
            "test",
        ));
        assert!(!p.is_threshold_met());
    }

    #[test]
    fn test_proposal_finalize_accepted() {
        let mut p = UpdateProposal::new(
            "p1",
            "skill",
            "1.0",
            "2.0",
            0.5,
            Utc::now() + chrono::Duration::hours(1),
            "sys",
        );
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.9, "ci"));
        p.add_evidence(Evidence::new(EvidenceType::PeerReview, 0.9, "reviewer"));

        let status = p.try_finalize().clone();
        assert_eq!(status, ProposalStatus::Accepted);
    }

    #[test]
    fn test_proposal_finalize_rejected_expired() {
        let mut p = UpdateProposal::new(
            "p1",
            "skill",
            "1.0",
            "2.0",
            0.99,
            Utc::now() - chrono::Duration::hours(1), // already expired
            "sys",
        );
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.5, "ci"));

        let status = p.try_finalize().clone();
        assert_eq!(status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_proposal_evidence_by_type() {
        let mut p = UpdateProposal::new("p1", "skill", "1.0", "2.0", 0.7, Utc::now(), "sys");
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.9, "ci-1"));
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.8, "ci-2"));
        p.add_evidence(Evidence::new(EvidenceType::PeerReview, 1.0, "reviewer"));

        let by_type = p.evidence_by_type();
        assert_eq!(by_type.get("TestPass").map(|v| v.len()), Some(2));
        assert_eq!(by_type.get("PeerReview").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_proposal_no_evidence_after_finalized() {
        let mut p = UpdateProposal::new(
            "p1",
            "skill",
            "1.0",
            "2.0",
            0.3,
            Utc::now() + chrono::Duration::hours(1),
            "sys",
        );
        p.add_evidence(Evidence::new(EvidenceType::TestPass, 0.9, "ci"));
        p.add_evidence(Evidence::new(EvidenceType::PeerReview, 0.9, "reviewer"));
        p.try_finalize(); // accepted

        let old_count = p.evidence.len();
        p.add_evidence(Evidence::new(EvidenceType::SecurityScan, 0.8, "sec"));
        assert_eq!(p.evidence.len(), old_count); // not added
    }

    // --- Evidence Gate Tests ---

    #[test]
    fn test_gate_propose_and_get() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "my-skill", "1.0", "2.0");

        let proposal = gate.get_proposal(&id);
        assert!(proposal.is_some());
        assert_eq!(proposal.unwrap().skill_name, "my-skill");
    }

    #[test]
    fn test_gate_add_evidence() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        let result = gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.9, "ci"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_gate_add_evidence_not_found() {
        let mut gate = EvidenceGate::new(test_config());
        let result = gate.add_evidence(
            "nonexistent",
            Evidence::new(EvidenceType::TestPass, 0.9, "ci"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_gate_finalize_accepted() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.9, "ci"))
            .unwrap();
        gate.add_evidence(
            &id,
            Evidence::new(EvidenceType::PeerReview, 0.9, "reviewer"),
        )
        .unwrap();

        let status = gate.finalize(&id).unwrap();
        assert_eq!(status, ProposalStatus::Accepted);

        // Moved to history
        assert!(gate.get_proposal(&id).is_none());
        assert_eq!(gate.history().len(), 1);
    }

    #[test]
    fn test_gate_finalize_insufficient_evidence_count() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.9, "ci"))
            .unwrap();

        let result = gate.finalize(&id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("evidence items"));
    }

    #[test]
    fn test_gate_finalize_insufficient_evidence_types() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        // Same type twice
        gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.9, "ci-1"))
            .unwrap();
        gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.8, "ci-2"))
            .unwrap();

        let result = gate.finalize(&id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("evidence types"));
    }

    #[test]
    fn test_gate_manual_approve() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        gate.manual_approve(&id, "admin").unwrap();

        assert!(gate.get_proposal(&id).is_none());
        assert_eq!(gate.history().len(), 1);
        assert_eq!(gate.history()[0].status, ProposalStatus::ManualOverride);
    }

    #[test]
    fn test_gate_pending_proposals() {
        let mut gate = EvidenceGate::new(test_config());
        gate.propose("p1", "skill-a", "1.0", "2.0");
        gate.propose("p2", "skill-b", "1.0", "2.0");

        assert_eq!(gate.pending_proposals().len(), 2);
    }

    #[test]
    fn test_gate_stats() {
        let mut gate = EvidenceGate::new(test_config());
        let id = gate.propose("p1", "skill", "1.0", "2.0");

        gate.add_evidence(&id, Evidence::new(EvidenceType::TestPass, 0.9, "ci"))
            .unwrap();
        gate.add_evidence(
            &id,
            Evidence::new(EvidenceType::PeerReview, 0.9, "reviewer"),
        )
        .unwrap();
        gate.finalize(&id).unwrap();

        let stats = gate.stats();
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.accepted_count, 1);
    }

    #[test]
    fn test_gate_expire_overdue() {
        let config = EvidenceGateConfig {
            default_deadline_secs: -1, // already expired
            ..test_config()
        };
        let mut gate = EvidenceGate::new(config);
        gate.propose("p1", "skill", "1.0", "2.0");

        let expired = gate.expire_overdue();
        assert_eq!(expired, 1);
        assert_eq!(gate.stats().rejected_count, 1);
    }

    #[test]
    fn test_gate_expire_disabled() {
        let config = EvidenceGateConfig {
            default_deadline_secs: -1,
            auto_reject_on_deadline: false,
            ..test_config()
        };
        let mut gate = EvidenceGate::new(config);
        gate.propose("p1", "skill", "1.0", "2.0");

        let expired = gate.expire_overdue();
        assert_eq!(expired, 0);
    }

    // --- Serialization ---

    #[test]
    fn test_evidence_type_serialization() {
        let types = [
            EvidenceType::TestPass,
            EvidenceType::SchemaValidation,
            EvidenceType::UsageSuccess,
            EvidenceType::PeerReview,
            EvidenceType::PerformanceBaseline,
            EvidenceType::SecurityScan,
            EvidenceType::Custom("test".into()),
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let deser: EvidenceType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deser);
        }
    }

    #[test]
    fn test_proposal_status_serialization() {
        let statuses = [
            ProposalStatus::Pending,
            ProposalStatus::Accepted,
            ProposalStatus::Rejected,
            ProposalStatus::ManualOverride,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let deser: ProposalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, deser);
        }
    }
}
