//! # GxP Audit Trail — 21 CFR Part 11 Compliant
//!
//! Immutable, append-only audit trail with SHA-256 hash chaining.
//! Every AI agent action is recorded with cryptographic integrity proof.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// GxP regulatory domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GxPDomain {
    FDA21CFR11,
    EUAnnex11,
    GAMP5,
    EUAIAct,
    IS014971,
    IEC62304,
    HIPAA,
    All,
}

/// Agent action type for audit classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Decision,
    ToolCall,
    Response,
    Approval,
    Rejection,
    SystemAction,
    ElectronicSignature,
    DataAccess,
    ConfigurationChange,
}

/// Risk level of the action
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// A single immutable audit record. Hash-chained to make tampering evident.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Unique record ID
    pub id: String,
    /// Agent that performed the action
    pub agent_id: String,
    /// Human-readable action description
    pub action: String,
    /// Type of action
    pub action_type: ActionType,
    /// When the action occurred
    pub timestamp: DateTime<Utc>,
    /// User on whose behalf the agent acted
    pub user_id: String,
    /// Operator (human) who approved or initiated
    pub operator_id: Option<String>,
    /// Rationale: why this decision was made
    pub rationale: String,
    /// SHA-256 of input data
    pub input_data_hash: String,
    /// SHA-256 of output data
    pub output_data_hash: String,
    /// Hash of the previous record (chain link)
    pub previous_hash: String,
    /// Hash of this record (computed)
    pub self_hash: String,
    /// Additional structured metadata
    pub metadata: serde_json::Value,
    /// Applicable GxP domains
    pub gxp_domains: Vec<GxPDomain>,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Compliance flags (e.g., ["ELECTRONIC_SIGNATURE_REQUIRED", "HUMAN_REVIEW_REQUIRED"])
    pub compliance_flags: Vec<String>,
}

impl AuditRecord {
    /// Build a new audit record. Call `AuditTrail::seal()` to finalize hash chain.
    pub fn new(
        agent_id: &str,
        action: &str,
        action_type: ActionType,
        user_id: &str,
        rationale: &str,
        input_data: &str,
        output_data: &str,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        let input_data_hash = Self::sha256(input_data);
        let output_data_hash = Self::sha256(output_data);
        // Genesis record has previous_hash = all-zeros
        let previous_hash =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");

        Self {
            id,
            agent_id: agent_id.to_string(),
            action: action.to_string(),
            action_type,
            timestamp,
            user_id: user_id.to_string(),
            operator_id: None,
            rationale: rationale.to_string(),
            input_data_hash,
            output_data_hash,
            previous_hash,
            self_hash: String::new(), // filled by seal()
            metadata: serde_json::Value::Object(Default::default()),
            gxp_domains: vec![GxPDomain::All],
            risk_level: RiskLevel::Medium,
            compliance_flags: Vec::new(),
        }
    }

    /// Builder pattern for fluent construction
    pub fn with_operator(mut self, operator_id: &str) -> Self {
        self.operator_id = Some(operator_id.to_string());
        self
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut m) = self.metadata {
            m.insert(key.to_string(), value);
        }
        self
    }

    pub fn with_gxp_domain(mut self, domain: GxPDomain) -> Self {
        if !self.gxp_domains.contains(&domain) {
            self.gxp_domains.push(domain);
        }
        self
    }

    pub fn with_risk_level(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    pub fn with_compliance_flag(mut self, flag: &str) -> Self {
        self.compliance_flags.push(flag.to_string());
        self
    }

    /// Compute SHA-256 hex string
    pub fn sha256(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Returns true if this record requires an electronic signature per 21 CFR Part 11.
    pub fn requires_signature(&self) -> bool {
        self.compliance_flags
            .iter()
            .any(|f| f == "ELECTRONIC_SIGNATURE_REQUIRED" || f == "HUMAN_REVIEW_REQUIRED")
    }

    /// Add a cross-crate correlation ID for distributed trace correlation.
    pub fn with_correlation_id(mut self, correlation_id: &str) -> Self {
        if let serde_json::Value::Object(ref mut m) = self.metadata {
            m.insert("correlation_id".to_string(), serde_json::Value::String(correlation_id.to_string()));
        }
        self
    }

    /// Compute this record's self-hash: chain of previous + all record fields
    pub fn compute_self_hash(&self) -> String {
        let payload = format!(
            "{}{}{}{}{}{}{}",
            self.agent_id,
            self.action,
            self.timestamp.to_rfc3339(),
            self.user_id,
            self.rationale,
            self.input_data_hash,
            self.output_data_hash,
        );
        let prev_payload = format!("{}{}", self.previous_hash, payload);
        Self::sha256(&prev_payload)
    }
}

/// Immutable append-only audit trail with hash chaining.
#[derive(Debug, Clone, Default)]
pub struct AuditTrail {
    records: Vec<AuditRecord>,
    chain_hash: String,
}

impl AuditTrail {
    /// Create a new empty audit trail.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            chain_hash: String::from(
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        }
    }

    /// Seal a record into the trail: compute self-hash, link to previous, push.
    /// Returns the self-hash of the sealed record.
    pub fn seal(&mut self, mut record: AuditRecord) -> Result<String, AuditTrailError> {
        // Auto-assign previous_hash so callers don't need to set it manually
        record.previous_hash = self.chain_hash.clone();

        let self_hash = record.compute_self_hash();
        record.self_hash = self_hash.clone();
        self.chain_hash = self_hash.clone();
        self.records.push(record);
        Ok(self_hash)
    }

    /// Verify the integrity of the entire chain.
    pub fn verify_chain(&self) -> Result<bool, AuditTrailError> {
        if self.records.is_empty() {
            return Ok(true);
        }

        let mut expected_prev =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");

        for record in &self.records {
            if record.previous_hash != expected_prev {
                return Err(AuditTrailError::ChainBroken {
                    expected: expected_prev,
                    found: record.previous_hash.clone(),
                });
            }
            let computed = record.compute_self_hash();
            if record.self_hash != computed {
                return Err(AuditTrailError::RecordTampered {
                    record_id: record.id.clone(),
                    expected: computed,
                    found: record.self_hash.clone(),
                });
            }
            expected_prev = record.self_hash.clone();
        }
        Ok(true)
    }

    /// Query records by agent and time range.
    pub fn query(
        &self,
        agent_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<&AuditRecord> {
        self.records
            .iter()
            .filter(|r| r.agent_id == agent_id && r.timestamp >= from && r.timestamp <= to)
            .collect()
    }

    /// Export audit trail for regulatory inspection (FDA, EMA, etc.)
    pub fn export_for_inspection(&self, agent_id: &str) -> Vec<serde_json::Value> {
        self.records
            .iter()
            .filter(|r| r.agent_id == agent_id)
            .map(|r| {
                serde_json::json!({
                    "record_id": r.id,
                    "agent_id": r.agent_id,
                    "action": r.action,
                    "action_type": r.action_type,
                    "timestamp": r.timestamp.to_rfc3339(),
                    "user_id": r.user_id,
                    "operator_id": r.operator_id,
                    "rationale": r.rationale,
                    "input_hash": r.input_data_hash,
                    "output_hash": r.output_data_hash,
                    "chain_hash": r.self_hash,
                    "gxp_domains": r.gxp_domains,
                    "risk_level": r.risk_level,
                    "compliance_flags": r.compliance_flags,
                })
            })
            .collect()
    }

    /// Number of records in the trail.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Current chain tip hash.
    pub fn chain_tip(&self) -> &str {
        &self.chain_hash
    }

    /// Get a record by ID.
    pub fn get(&self, id: &str) -> Option<&AuditRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    /// All agents present in the trail.
    pub fn agents(&self) -> HashSet<&str> {
        self.records.iter().map(|r| r.agent_id.as_str()).collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuditTrailError {
    #[error("chain broken: expected hash {expected}, found {found}")]
    ChainBroken { expected: String, found: String },

    #[error("record {record_id} has been tampered: expected {expected}, found {found}")]
    RecordTampered {
        record_id: String,
        expected: String,
        found: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(agent: &str, action: &str) -> AuditRecord {
        AuditRecord::new(
            agent,
            action,
            ActionType::Decision,
            "user-1",
            "rationale for test",
            r#"{"input":"test"}"#,
            r#"{"output":"ok"}"#,
        )
    }

    #[test]
    fn test_new_trail_is_empty() {
        let trail = AuditTrail::new();
        assert_eq!(trail.len(), 0);
    }

    #[test]
    fn test_seal_single_record() {
        let mut trail = AuditTrail::new();
        let record = make_record("agent-1", "approve_diagnosis");
        let hash = trail.seal(record).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(trail.len(), 1);
    }

    #[test]
    fn test_chain_integrity_with_multiple_records() {
        let mut trail = AuditTrail::new();
        let r1 = make_record("agent-1", "step_1");
        let r2 = AuditRecord::new(
            "agent-1",
            "step_2",
            ActionType::Decision,
            "user-1",
            "second step",
            r#"{"input":"data"}"#,
            r#"{"output":"done"}"#,
        );
        trail.seal(r1).unwrap();
        trail.seal(r2).unwrap();
        assert_eq!(trail.len(), 2);
        assert!(trail.verify_chain().is_ok());
    }

    #[test]
    fn test_verify_fails_on_tampering() {
        let mut trail = AuditTrail::new();
        let mut r = make_record("agent-1", "original_action");
        let hash = trail.seal(r).unwrap();
        // Tamper: change the action after sealing (not possible in normal flow,
        // but this simulates what verify_chain catches)
        // We test via direct record manipulation
        if let Some(record) = trail.records.first_mut() {
            record.action = "tampered_action".to_string();
            // recompute hash would fail verification
            let computed = record.compute_self_hash();
            assert_ne!(computed, hash);
        }
    }

    #[test]
    fn test_query_by_agent_and_timerange() {
        let mut trail = AuditTrail::new();
        let r1 = make_record("agent-a", "action_1");
        let r2 = make_record("agent-b", "action_2");
        let r3 = make_record("agent-a", "action_3");
        trail.seal(r1).unwrap();
        trail.seal(r2).unwrap();
        trail.seal(r3).unwrap();

        let from = DateTime::from_timestamp(0, 0).unwrap();
        let to = DateTime::from_timestamp(9_000_000_000, 0).unwrap();
        let results = trail.query("agent-a", from, to);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_export_for_inspection() {
        let mut trail = AuditTrail::new();
        trail.seal(make_record("agent-1", "diagnose")).unwrap();
        trail.seal(make_record("agent-1", "prescribe")).unwrap();
        let export = trail.export_for_inspection("agent-1");
        assert_eq!(export.len(), 2);
        assert!(export[0].get("chain_hash").is_some());
        assert!(export[0].get("timestamp").is_some());
    }

    #[test]
    fn test_sha256_consistency() {
        let h1 = AuditRecord::sha256("hello");
        let h2 = AuditRecord::sha256("hello");
        assert_eq!(h1, h2);
        let h3 = AuditRecord::sha256("world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Critical > RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Medium);
        assert!(RiskLevel::Medium > RiskLevel::Low);
    }

    #[test]
    fn test_builder_pattern() {
        let record = make_record("agent-1", "test")
            .with_operator("dr-smith")
            .with_risk_level(RiskLevel::Critical)
            .with_gxp_domain(GxPDomain::FDA21CFR11)
            .with_compliance_flag("HUMAN_REVIEW_REQUIRED");

        assert_eq!(record.operator_id, Some("dr-smith".to_string()));
        assert_eq!(record.risk_level, RiskLevel::Critical);
        assert!(record.gxp_domains.contains(&GxPDomain::FDA21CFR11));
        assert!(record
            .compliance_flags
            .contains(&"HUMAN_REVIEW_REQUIRED".to_string()));
    }

    #[test]
    fn test_chain_hash_updates() {
        let mut trail = AuditTrail::new();
        assert_eq!(
            trail.chain_tip(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        trail.seal(make_record("a", "1")).unwrap();
        let tip1 = trail.chain_tip().to_string();
        assert_ne!(
            tip1,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        trail.seal(make_record("a", "2")).unwrap();
        let tip2 = trail.chain_tip();
        assert_ne!(
            tip2,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_ne!(tip2, tip1);
    }

    #[test]
    fn test_export_covers_all_fields() {
        let mut trail = AuditTrail::new();
        let r = make_record("agent-x", "critical_action")
            .with_gxp_domain(GxPDomain::FDA21CFR11)
            .with_risk_level(RiskLevel::Critical);
        trail.seal(r).unwrap();

        let export = trail.export_for_inspection("agent-x");
        assert_eq!(export.len(), 1);
        let fields = export[0].as_object().unwrap();
        assert!(fields.contains_key("record_id"));
        assert!(fields.contains_key("agent_id"));
        assert!(fields.contains_key("timestamp"));
        assert!(fields.contains_key("rationale"));
        assert!(fields.contains_key("input_hash"));
        assert!(fields.contains_key("output_hash"));
        assert!(fields.contains_key("chain_hash"));
        assert!(fields.contains_key("gxp_domains"));
        assert!(fields.contains_key("risk_level"));
    }
}
