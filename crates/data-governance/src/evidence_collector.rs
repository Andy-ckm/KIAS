//! # Compliance Evidence Collector
//!
//! Auto-gathers audit artifacts per-task with chain-of-custody metadata.
//! Integrates with the existing [`EvidenceChain`] to produce GxP-compliant
//! evidence trails (FDA 21 CFR Part 11, EU Annex 11, ALCOA+).
//!
//! ## Usage
//!
//! ```rust
//! use kias_data_governance::evidence_collector::*;
//! use kias_data_governance::evidence_chain::*;
//!
//! let mut collector = EvidenceCollector::new("task-001", "agent-1", "deploy-service");
//! collector.set_compliance_context(ComplianceContext {
//!     regulation: "FDA 21 CFR Part 11".to_string(),
//!     impact_level: ImpactLevel::High,
//!     ..Default::default()
//! });
//!
//! collector.begin_intent("Deploy to production", serde_json::json!({"env": "prod"})).unwrap();
//! collector.record_proof("Policy check passed", serde_json::json!({"policy": "deploy-ok"})).unwrap();
//! collector.record_consensus("2/3 approvers", serde_json::json!({"votes": 2})).unwrap();
//! collector.authorize_execution("system", serde_json::json!({})).unwrap();
//! collector.complete_execution(serde_json::json!({"deploy_id": "d-001"})).unwrap();
//!
//! let bundle = collector.finalize().unwrap();
//! assert!(bundle.verify().is_ok());
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::evidence_chain::{EvidenceChain, EvidenceError, EvidenceEventType};

// ── Types ───────────────────────────────────────────────────────────────

/// Impact level for compliance classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImpactLevel {
    /// No impact on product quality or patient safety.
    Low,
    /// Potential impact on data integrity.
    Medium,
    /// Direct impact on product quality or patient safety.
    High,
    /// Critical impact — requires human approval.
    Critical,
}

impl Default for ImpactLevel {
    fn default() -> Self {
        Self::Medium
    }
}

impl std::fmt::Display for ImpactLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Compliance context attached to every evidence bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceContext {
    /// Applicable regulation (e.g., "FDA 21 CFR Part 11", "EU Annex 11").
    pub regulation: String,
    /// Impact classification.
    pub impact_level: ImpactLevel,
    /// Business justification for the action.
    pub justification: Option<String>,
    /// Human approver (required for Critical impact).
    pub approver: Option<String>,
    /// Associated change request ID.
    pub change_request_id: Option<String>,
    /// Tags for filtering and reporting.
    pub tags: Vec<String>,
}

impl Default for ComplianceContext {
    fn default() -> Self {
        Self {
            regulation: "General".to_string(),
            impact_level: ImpactLevel::Medium,
            justification: None,
            approver: None,
            change_request_id: None,
            tags: Vec::new(),
        }
    }
}

/// An artifact captured during task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Unique artifact identifier.
    pub artifact_id: String,
    /// Artifact type (e.g., "input", "output", "log", "screenshot", "config").
    pub artifact_type: String,
    /// Human-readable name.
    pub name: String,
    /// SHA-256 hash of the artifact content.
    pub content_hash: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// When the artifact was captured.
    pub captured_at: DateTime<Utc>,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Storage reference (path, URL, or inline).
    pub storage_ref: String,
    /// Additional metadata.
    pub metadata: serde_json::Value,
}

/// Chain-of-custody entry — tracks who handled an artifact and when.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodyEntry {
    /// Who handled the artifact.
    pub actor: String,
    /// What action was performed (e.g., "created", "verified", "transferred", "archived").
    pub action: String,
    /// When the action occurred.
    pub timestamp: DateTime<Utc>,
    /// Hash of the artifact at this point.
    pub artifact_hash: String,
    /// Optional notes.
    pub notes: Option<String>,
}

/// A complete evidence bundle for a single task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// Task identifier.
    pub task_id: String,
    /// Agent that executed the task.
    pub agent_id: String,
    /// Task description.
    pub task_description: String,
    /// The immutable evidence chain.
    pub chain: EvidenceChain,
    /// Captured artifacts.
    pub artifacts: Vec<Artifact>,
    /// Chain-of-custody records.
    pub custody_log: Vec<CustodyEntry>,
    /// Compliance context.
    pub compliance: ComplianceContext,
    /// When the bundle was created.
    pub created_at: DateTime<Utc>,
    /// When the bundle was finalized.
    pub finalized_at: Option<DateTime<Utc>>,
    /// Bundle hash (computed on finalization).
    pub bundle_hash: Option<String>,
}

impl EvidenceBundle {
    /// Verify the integrity of the evidence bundle.
    ///
    /// Checks:
    /// 1. Evidence chain integrity.
    /// 2. All artifacts have custody entries.
    /// 3. Bundle hash matches content.
    pub fn verify(&self) -> Result<(), String> {
        // Verify chain
        self.chain
            .verify_integrity()
            .map_err(|e| format!("Chain integrity failed: {e}"))?;

        // Verify all artifacts have at least one custody entry
        for artifact in &self.artifacts {
            let has_custody = self
                .custody_log
                .iter()
                .any(|c| c.artifact_hash == artifact.content_hash);
            if !has_custody {
                return Err(format!(
                    "Artifact '{}' has no custody entry",
                    artifact.artifact_id
                ));
            }
        }

        // Verify bundle hash if finalized
        if let Some(ref hash) = self.bundle_hash {
            let computed = self.compute_bundle_hash();
            if *hash != computed {
                return Err(format!(
                    "Bundle hash mismatch: expected {computed}, got {hash}"
                ));
            }
        }

        Ok(())
    }

    /// Get artifacts by type.
    pub fn artifacts_by_type(&self, artifact_type: &str) -> Vec<&Artifact> {
        self.artifacts
            .iter()
            .filter(|a| a.artifact_type == artifact_type)
            .collect()
    }

    /// Get the full custody trail for an artifact.
    pub fn custody_trail(&self, artifact_hash: &str) -> Vec<&CustodyEntry> {
        self.custody_log
            .iter()
            .filter(|c| c.artifact_hash == artifact_hash)
            .collect()
    }

    fn compute_bundle_hash(&self) -> String {
        let data = format!(
            "{}|{}|{}|{}|{}",
            self.task_id,
            self.agent_id,
            self.chain.tail_hash().unwrap_or(""),
            self.artifacts.len(),
            self.custody_log.len()
        );
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

// ── Evidence Collector ───────────────────────────────────────────────────

/// Collects evidence artifacts during task execution and produces
/// an immutable [`EvidenceBundle`].
pub struct EvidenceCollector {
    task_id: String,
    agent_id: String,
    task_description: String,
    chain: EvidenceChain,
    artifacts: Vec<Artifact>,
    custody_log: Vec<CustodyEntry>,
    compliance: ComplianceContext,
    created_at: DateTime<Utc>,
    artifact_counter: u64,
    /// Custom metadata attached to the bundle.
    metadata: HashMap<String, serde_json::Value>,
}

impl EvidenceCollector {
    /// Create a new evidence collector for a task.
    pub fn new(task_id: &str, agent_id: &str, task_description: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            task_description: task_description.to_string(),
            chain: EvidenceChain::new(task_id),
            artifacts: Vec::new(),
            custody_log: Vec::new(),
            compliance: ComplianceContext::default(),
            created_at: Utc::now(),
            artifact_counter: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set the compliance context.
    pub fn set_compliance_context(&mut self, ctx: ComplianceContext) {
        self.compliance = ctx;
    }

    /// Add custom metadata.
    pub fn set_metadata(&mut self, key: &str, value: serde_json::Value) {
        self.metadata.insert(key.to_string(), value);
    }

    // ── Chain Events ──────────────────────────────────────────────

    /// Record the intent to perform an action.
    pub fn begin_intent(
        &mut self,
        description: &str,
        payload: serde_json::Value,
    ) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::IntentDeclared,
            &self.agent_id,
            description,
            payload,
        )?;
        Ok(())
    }

    /// Record a proof/justification for the action.
    pub fn record_proof(
        &mut self,
        description: &str,
        payload: serde_json::Value,
    ) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::ProofConstructed,
            &self.agent_id,
            description,
            payload,
        )?;
        Ok(())
    }

    /// Record a consensus evaluation.
    pub fn record_consensus(
        &mut self,
        description: &str,
        payload: serde_json::Value,
    ) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::ConsensusEvaluated,
            &self.agent_id,
            description,
            payload,
        )?;
        Ok(())
    }

    /// Record execution authorization.
    pub fn authorize_execution(
        &mut self,
        authorizer: &str,
        payload: serde_json::Value,
    ) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::ExecutionAuthorized,
            authorizer,
            &format!("Execution authorized by {authorizer}"),
            payload,
        )?;
        Ok(())
    }

    /// Record successful execution completion.
    pub fn complete_execution(&mut self, payload: serde_json::Value) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::ExecutionCompleted,
            &self.agent_id,
            &format!("Task {} completed", self.task_id),
            payload,
        )?;
        Ok(())
    }

    /// Record execution failure.
    pub fn fail_execution(
        &mut self,
        reason: &str,
        payload: serde_json::Value,
    ) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::ExecutionFailed,
            &self.agent_id,
            &format!("Task {} failed: {reason}", self.task_id),
            payload,
        )?;
        Ok(())
    }

    /// Escalate the chain (e.g., to human review).
    pub fn escalate(&mut self, reviewer: &str, reason: &str) -> Result<(), EvidenceError> {
        self.chain.append(
            EvidenceEventType::Escalated,
            reviewer,
            reason,
            serde_json::json!({"escalated_to": reviewer, "reason": reason}),
        )?;
        Ok(())
    }

    // ── Artifact Capture ──────────────────────────────────────────

    /// Capture an artifact with its content hash.
    pub fn capture_artifact(
        &mut self,
        artifact_type: &str,
        name: &str,
        content: &[u8],
        mime_type: Option<&str>,
        storage_ref: &str,
    ) -> Artifact {
        self.artifact_counter += 1;
        let content_hash = sha256_hex(content);
        let artifact_id = format!("{}-art-{:04}", self.task_id, self.artifact_counter);

        let artifact = Artifact {
            artifact_id: artifact_id.clone(),
            artifact_type: artifact_type.to_string(),
            name: name.to_string(),
            content_hash: content_hash.clone(),
            size_bytes: content.len() as u64,
            captured_at: Utc::now(),
            mime_type: mime_type.map(|s| s.to_string()),
            storage_ref: storage_ref.to_string(),
            metadata: serde_json::json!({}),
        };

        // Record custody entry
        self.custody_log.push(CustodyEntry {
            actor: self.agent_id.clone(),
            action: "created".to_string(),
            timestamp: Utc::now(),
            artifact_hash: content_hash,
            notes: Some(format!("Captured as {artifact_type}: {name}")),
        });

        self.artifacts.push(artifact.clone());
        artifact
    }

    /// Record a custody transfer for an existing artifact.
    pub fn record_custody_transfer(
        &mut self,
        artifact_hash: &str,
        new_actor: &str,
        action: &str,
        notes: Option<&str>,
    ) {
        self.custody_log.push(CustodyEntry {
            actor: new_actor.to_string(),
            action: action.to_string(),
            timestamp: Utc::now(),
            artifact_hash: artifact_hash.to_string(),
            notes: notes.map(|s| s.to_string()),
        });
    }

    // ── Finalization ──────────────────────────────────────────────

    /// Finalize the evidence bundle, making it immutable.
    ///
    /// Returns the complete [`EvidenceBundle`] with chain-of-custody metadata.
    pub fn finalize(mut self) -> Result<EvidenceBundle, EvidenceError> {
        self.chain.finalize();

        let mut bundle = EvidenceBundle {
            task_id: self.task_id,
            agent_id: self.agent_id,
            task_description: self.task_description,
            chain: self.chain,
            artifacts: self.artifacts,
            custody_log: self.custody_log,
            compliance: self.compliance,
            created_at: self.created_at,
            finalized_at: Some(Utc::now()),
            bundle_hash: None,
        };

        bundle.bundle_hash = Some(bundle.compute_bundle_hash());
        Ok(bundle)
    }
}

// ── Evidence Registry ───────────────────────────────────────────────────

/// Registry for managing multiple evidence bundles.
#[derive(Debug, Default)]
pub struct EvidenceRegistry {
    bundles: std::collections::HashMap<String, EvidenceBundle>,
}

impl EvidenceRegistry {
    pub fn new() -> Self {
        Self {
            bundles: std::collections::HashMap::new(),
        }
    }

    /// Register a finalized evidence bundle.
    pub fn register(&mut self, bundle: EvidenceBundle) {
        self.bundles.insert(bundle.task_id.clone(), bundle);
    }

    /// Get a bundle by task ID.
    pub fn get(&self, task_id: &str) -> Option<&EvidenceBundle> {
        self.bundles.get(task_id)
    }

    /// List all registered task IDs.
    pub fn list_task_ids(&self) -> Vec<String> {
        self.bundles.keys().cloned().collect()
    }

    /// Verify all registered bundles.
    pub fn verify_all(&self) -> Vec<(String, Result<(), String>)> {
        self.bundles
            .iter()
            .map(|(id, bundle)| (id.clone(), bundle.verify()))
            .collect()
    }

    /// Get bundles by regulation.
    pub fn by_regulation(&self, regulation: &str) -> Vec<&EvidenceBundle> {
        self.bundles
            .values()
            .filter(|b| b.compliance.regulation == regulation)
            .collect()
    }

    /// Get bundles by impact level.
    pub fn by_impact_level(&self, level: &ImpactLevel) -> Vec<&EvidenceBundle> {
        self.bundles
            .values()
            .filter(|b| &b.compliance.impact_level == level)
            .collect()
    }

    /// Generate a compliance summary report.
    pub fn compliance_summary(&self) -> ComplianceSummary {
        let total = self.bundles.len();
        let mut by_level = HashMap::new();
        let mut all_verified = true;
        let mut failed_verifications = Vec::new();

        for (id, bundle) in &self.bundles {
            *by_level
                .entry(bundle.compliance.impact_level.clone())
                .or_insert(0usize) += 1;
            if let Err(e) = bundle.verify() {
                all_verified = false;
                failed_verifications.push((id.clone(), e));
            }
        }

        ComplianceSummary {
            total_bundles: total,
            by_impact_level: by_level,
            all_verified,
            failed_verifications,
        }
    }
}

/// Summary of compliance evidence status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_bundles: usize,
    pub by_impact_level: HashMap<ImpactLevel, usize>,
    pub all_verified: bool,
    pub failed_verifications: Vec<(String, String)>,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_collector() -> EvidenceCollector {
        EvidenceCollector::new("task-001", "agent-1", "Test task")
    }

    #[test]
    fn test_collector_full_lifecycle() {
        let mut collector = make_collector();
        collector.set_compliance_context(ComplianceContext {
            regulation: "FDA 21 CFR Part 11".to_string(),
            impact_level: ImpactLevel::High,
            justification: Some("Deploy critical fix".to_string()),
            approver: Some("admin@example.com".to_string()),
            change_request_id: Some("CR-001".to_string()),
            tags: vec!["deploy".to_string(), "critical".to_string()],
        });

        collector
            .begin_intent("Deploy to prod", serde_json::json!({"env": "prod"}))
            .unwrap();
        collector
            .record_proof(
                "All tests passed",
                serde_json::json!({"tests": 100, "passed": 100}),
            )
            .unwrap();
        collector
            .record_consensus(
                "2/3 approved",
                serde_json::json!({"approve": 2, "reject": 1}),
            )
            .unwrap();
        collector
            .authorize_execution("admin", serde_json::json!({}))
            .unwrap();
        collector
            .complete_execution(serde_json::json!({"deploy_id": "d-001"}))
            .unwrap();

        let bundle = collector.finalize().unwrap();
        assert_eq!(bundle.task_id, "task-001");
        assert_eq!(bundle.agent_id, "agent-1");
        assert!(bundle.verify().is_ok());
        assert!(bundle.bundle_hash.is_some());
        assert!(bundle.finalized_at.is_some());
    }

    #[test]
    fn test_collector_with_artifacts() {
        let mut collector = make_collector();
        collector
            .begin_intent("Generate report", serde_json::json!({}))
            .unwrap();
        collector
            .record_proof("Template ready", serde_json::json!({}))
            .unwrap();
        collector
            .record_consensus("Approved", serde_json::json!({}))
            .unwrap();
        collector
            .authorize_execution("admin", serde_json::json!({}))
            .unwrap();

        let art = collector.capture_artifact(
            "output",
            "report.pdf",
            b"fake-pdf-content",
            Some("application/pdf"),
            "/tmp/report.pdf",
        );
        assert_eq!(art.artifact_type, "output");
        assert!(!art.content_hash.is_empty());
        assert_eq!(art.size_bytes, 16);

        collector.complete_execution(serde_json::json!({})).unwrap();

        let bundle = collector.finalize().unwrap();
        assert_eq!(bundle.artifacts.len(), 1);
        assert_eq!(bundle.custody_log.len(), 1);
        assert!(bundle.verify().is_ok());
    }

    #[test]
    fn test_custody_transfer() {
        let mut collector = make_collector();
        collector
            .begin_intent("Process data", serde_json::json!({}))
            .unwrap();
        collector
            .record_proof("Validated", serde_json::json!({}))
            .unwrap();
        collector
            .record_consensus("OK", serde_json::json!({}))
            .unwrap();
        collector
            .authorize_execution("admin", serde_json::json!({}))
            .unwrap();

        let art = collector.capture_artifact("input", "data.csv", b"a,b,c", None, "/tmp/data.csv");

        collector.record_custody_transfer(
            &art.content_hash,
            "verifier-1",
            "verified",
            Some("Data integrity confirmed"),
        );

        collector.complete_execution(serde_json::json!({})).unwrap();

        let bundle = collector.finalize().unwrap();
        assert_eq!(bundle.custody_log.len(), 2);
        assert_eq!(bundle.custody_log[0].action, "created");
        assert_eq!(bundle.custody_log[1].action, "verified");
        assert_eq!(bundle.custody_log[1].actor, "verifier-1");
    }

    #[test]
    fn test_failure_path() {
        let mut collector = make_collector();
        collector
            .begin_intent("Risky operation", serde_json::json!({}))
            .unwrap();
        collector
            .record_proof("Risk assessment", serde_json::json!({"risk": "high"}))
            .unwrap();
        collector
            .record_consensus("Reviewed", serde_json::json!({}))
            .unwrap();
        collector
            .authorize_execution("system", serde_json::json!({}))
            .unwrap();
        collector
            .fail_execution(
                "Connection timeout",
                serde_json::json!({"error": "ETIMEDOUT"}),
            )
            .unwrap();

        let bundle = collector.finalize().unwrap();
        assert_eq!(bundle.chain.len(), 5);
        assert!(bundle.verify().is_ok());
    }

    #[test]
    fn test_escalation() {
        let mut collector = make_collector();
        collector
            .begin_intent("Delete user data", serde_json::json!({}))
            .unwrap();
        collector
            .escalate("dpo", "GDPR deletion requires DPO approval")
            .unwrap();

        let bundle = collector.finalize().unwrap();
        assert_eq!(bundle.chain.len(), 2);
    }

    #[test]
    fn test_invalid_transition() {
        let mut collector = make_collector();
        // Cannot skip to ExecutionCompleted without prior events
        let result = collector.complete_execution(serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_evidence_registry() {
        let mut registry = EvidenceRegistry::new();
        assert!(registry.list_task_ids().is_empty());

        let mut collector1 = EvidenceCollector::new("t1", "a1", "Task 1");
        collector1
            .begin_intent("Intent", serde_json::json!({}))
            .unwrap();
        collector1
            .record_proof("Proof", serde_json::json!({}))
            .unwrap();
        collector1
            .record_consensus("OK", serde_json::json!({}))
            .unwrap();
        collector1
            .authorize_execution("admin", serde_json::json!({}))
            .unwrap();
        collector1
            .complete_execution(serde_json::json!({}))
            .unwrap();
        registry.register(collector1.finalize().unwrap());

        let mut collector2 = EvidenceCollector::new("t2", "a2", "Task 2");
        collector2.set_compliance_context(ComplianceContext {
            regulation: "EU Annex 11".to_string(),
            impact_level: ImpactLevel::Critical,
            ..Default::default()
        });
        collector2
            .begin_intent("Intent", serde_json::json!({}))
            .unwrap();
        collector2
            .record_proof("Proof", serde_json::json!({}))
            .unwrap();
        collector2
            .record_consensus("OK", serde_json::json!({}))
            .unwrap();
        collector2
            .authorize_execution("admin", serde_json::json!({}))
            .unwrap();
        collector2
            .complete_execution(serde_json::json!({}))
            .unwrap();
        registry.register(collector2.finalize().unwrap());

        assert_eq!(registry.list_task_ids().len(), 2);
        assert!(registry.get("t1").is_some());

        let eu_bundles = registry.by_regulation("EU Annex 11");
        assert_eq!(eu_bundles.len(), 1);

        let critical = registry.by_impact_level(&ImpactLevel::Critical);
        assert_eq!(critical.len(), 1);

        let summary = registry.compliance_summary();
        assert_eq!(summary.total_bundles, 2);
        assert!(summary.all_verified);
    }

    #[test]
    fn test_compliance_context_default() {
        let ctx = ComplianceContext::default();
        assert_eq!(ctx.regulation, "General");
        assert_eq!(ctx.impact_level, ImpactLevel::Medium);
        assert!(ctx.justification.is_none());
    }

    #[test]
    fn test_impact_level_display() {
        assert_eq!(ImpactLevel::Low.to_string(), "low");
        assert_eq!(ImpactLevel::Medium.to_string(), "medium");
        assert_eq!(ImpactLevel::High.to_string(), "high");
        assert_eq!(ImpactLevel::Critical.to_string(), "critical");
    }
}
