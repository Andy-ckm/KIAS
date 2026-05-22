//! Decision Record — governance-by-design audit trail for every
//! scheduling, routing, and autonomy decision.
//!
//! Each record captures inputs, rules evaluated, weights, candidates,
//! confidence score, result, and a human-readable explanation so that
//! any decision can be reproduced and justified at any later point.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// A single decision made by any AgentGuard component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Unique identifier for this decision.
    pub decision_id: Uuid,
    /// Wall-clock time when the decision was made.
    pub timestamp: DateTime<Utc>,
    /// Which agent made the decision (or "scheduler" / "router" / "autonomy").
    pub agent_id: String,
    /// Semantic type of the decision.
    pub action: DecisionAction,
    /// Full input payload that was available when the decision was made.
    pub inputs: DecisionInputs,
    /// Ordered list of rule identifiers that were evaluated.
    pub rules_applied: Vec<String>,
    /// Named weights used by the scoring function.
    pub weights: HashMap<String, f64>,
    /// Candidate options that were considered.
    pub candidates: Vec<Candidate>,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// The chosen result (typically the winning candidate id).
    pub result: DecisionResult,
    /// Human-readable explanation of why this result was chosen.
    pub explanation: String,
    /// SHA-256 of the previous record (hash-chain for tamper evidence).
    pub prev_hash: String,
    /// SHA-256 of this record's content (excluding `prev_hash` itself).
    pub self_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DecisionAction {
    Schedule,
    Route,
    AutonomyEscalate,
    AutonomyDemote,
    Retry,
    Reject,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionInputs {
    /// Arbitrary JSON-serialisable key-value map.
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub score: f64,
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub selected_id: String,
    pub rejected_ids: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Parameters for creating a DecisionRecord.
#[derive(Debug, Clone)]
pub struct DecisionRecordParams {
    pub agent_id: String,
    pub action: DecisionAction,
    pub inputs: DecisionInputs,
    pub rules_applied: Vec<String>,
    pub weights: HashMap<String, f64>,
    pub candidates: Vec<Candidate>,
    pub confidence: f64,
    pub result: DecisionResult,
    pub explanation: String,
    pub prev_hash: String,
}

impl DecisionRecord {
    /// Create a new record, automatically computing hashes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(params: DecisionRecordParams) -> Self {
        let DecisionRecordParams {
            agent_id,
            action,
            inputs,
            rules_applied,
            weights,
            candidates,
            confidence,
            result,
            explanation,
            prev_hash,
        } = params;
        let decision_id = Uuid::new_v4();
        let timestamp = Utc::now();

        // Build content for self-hash (everything except prev_hash and self_hash)
        let content = format!(
            "{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}",
            decision_id,
            timestamp,
            agent_id,
            action,
            inputs,
            rules_applied,
            weights,
            candidates,
            confidence,
            explanation
        );
        let self_hash = hex_hash(&content);

        Self {
            decision_id,
            timestamp,
            agent_id,
            action,
            inputs,
            rules_applied,
            weights,
            candidates,
            confidence,
            result,
            explanation,
            prev_hash,
            self_hash,
        }
    }

    /// Verify the hash chain integrity of this record.
    pub fn verify_integrity(&self) -> bool {
        let content = format!(
            "{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}",
            self.decision_id,
            self.timestamp,
            self.agent_id,
            self.action,
            self.inputs,
            self.rules_applied,
            self.weights,
            self.candidates,
            self.confidence,
            self.explanation
        );
        self.self_hash == hex_hash(&content)
    }

    /// Export the record as a JSON byte vector.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Import a record from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// SHA-256 hex digest helper.
fn hex_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Records decisions to an in-memory append-only log.
#[derive(Debug, Clone, Default)]
pub struct DecisionRecorder {
    records: Vec<DecisionRecord>,
    last_hash: String,
}

impl DecisionRecorder {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            last_hash: hex_hash("genesis"),
        }
    }

    /// Record a new decision, chaining it to the previous one.
    pub fn record(&mut self, record: DecisionRecord) {
        let mut r = record;
        r.prev_hash = self.last_hash.clone();
        // Recompute self_hash now that prev_hash is set
        let content = format!(
            "{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}",
            r.decision_id,
            r.timestamp,
            r.agent_id,
            r.action,
            r.inputs,
            r.rules_applied,
            r.weights,
            r.candidates,
            r.confidence,
            r.explanation
        );
        r.self_hash = hex_hash(&content);
        self.last_hash = r.self_hash.clone();
        self.records.push(r);
    }

    /// Build a record via builder pattern then record it in one step.
    #[allow(clippy::too_many_arguments)]
    pub fn record_decision(&mut self, params: DecisionRecordParams) -> Uuid {
        let rec = DecisionRecord::new(DecisionRecordParams {
            prev_hash: self.last_hash.clone(),
            ..params
        });
        let id = rec.decision_id;
        self.record(rec);
        id
    }

    /// Retrieve all records.
    pub fn all(&self) -> &[DecisionRecord] {
        &self.records
    }

    /// Verify integrity of the entire chain.
    pub fn verify_chain(&self) -> Vec<(Uuid, bool)> {
        let mut prev_expected = hex_hash("genesis");
        let mut results = Vec::new();
        for rec in &self.records {
            let self_ok = rec.verify_integrity();
            let chain_ok = rec.prev_hash == prev_expected;
            results.push((rec.decision_id, self_ok && chain_ok));
            prev_expected = rec.self_hash.clone();
        }
        results
    }

    /// Export all records as JSON.
    pub fn export_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self.records).unwrap_or_default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_candidates() -> Vec<Candidate> {
        vec![
            Candidate {
                id: "node-1".into(),
                score: 0.95,
                attributes: HashMap::new(),
            },
            Candidate {
                id: "node-2".into(),
                score: 0.80,
                attributes: HashMap::new(),
            },
        ]
    }

    fn make_inputs() -> DecisionInputs {
        DecisionInputs {
            params: HashMap::from([("resource_request".into(), serde_json::json!("2cpu"))]),
        }
    }

    fn make_result(winner: &str) -> DecisionResult {
        DecisionResult {
            selected_id: winner.into(),
            rejected_ids: vec!["node-2".into()],
            metadata: HashMap::new(),
        }
    }

    fn make_weights() -> HashMap<String, f64> {
        HashMap::from([("cpu".into(), 0.6), ("memory".into(), 0.4)])
    }

    fn make_params(agent: &str, action: DecisionAction, explanation: &str) -> DecisionRecordParams {
        DecisionRecordParams {
            agent_id: agent.into(),
            action,
            inputs: make_inputs(),
            rules_applied: vec![],
            weights: HashMap::new(),
            candidates: make_candidates(),
            confidence: 0.9,
            result: make_result("node-1"),
            explanation: explanation.into(),
            prev_hash: String::new(),
        }
    }

    #[test]
    fn test_record_creation() {
        let mut rec = DecisionRecorder::new();
        let id = rec.record_decision(DecisionRecordParams {
            rules_applied: vec!["rule-cpu".into(), "rule-mem".into()],
            weights: make_weights(),
            confidence: 0.95,
            explanation: "node-1 has most CPU".into(),
            ..make_params("scheduler", DecisionAction::Schedule, "")
        });
        assert!(!id.is_nil());
        assert_eq!(rec.all().len(), 1);
    }

    #[test]
    fn test_chain_integrity() {
        let mut rec = DecisionRecorder::new();
        rec.record_decision(make_params("scheduler", DecisionAction::Schedule, "test"));
        rec.record_decision(DecisionRecordParams {
            agent_id: "router".into(),
            action: DecisionAction::Route,
            confidence: 0.8,
            result: make_result("node-2"),
            explanation: "test2".into(),
            ..make_params("router", DecisionAction::Route, "test2")
        });
        let results = rec.verify_chain();
        assert!(results.iter().all(|(_, ok)| *ok));
    }

    #[test]
    fn test_verify_integrity_false_on_tamper() {
        let mut rec = DecisionRecorder::new();
        rec.record_decision(make_params("scheduler", DecisionAction::Schedule, "test"));
        if let Some(r) = rec.records.first_mut() {
            r.explanation = "TAMPERED".into();
        }
        let results = rec.verify_chain();
        assert!(!results[0].1);
    }

    #[test]
    fn test_export_import_json() {
        let mut rec = DecisionRecorder::new();
        rec.record_decision(DecisionRecordParams {
            agent_id: "autonomy".into(),
            action: DecisionAction::AutonomyEscalate,
            rules_applied: vec!["risk-check".into()],
            weights: make_weights(),
            confidence: 0.99,
            explanation: "risk score below threshold".into(),
            ..make_params("autonomy", DecisionAction::AutonomyEscalate, "")
        });
        let json = rec.export_json();
        let loaded: Vec<DecisionRecord> = serde_json::from_slice(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].agent_id, "autonomy");
    }

    #[test]
    fn test_decision_action_variants() {
        let actions = vec![
            DecisionAction::Schedule,
            DecisionAction::Route,
            DecisionAction::AutonomyEscalate,
            DecisionAction::AutonomyDemote,
            DecisionAction::Retry,
            DecisionAction::Reject,
            DecisionAction::Other("custom".into()),
        ];
        for a in actions {
            let json = serde_json::to_string(&a).unwrap();
            let back: DecisionAction = serde_json::from_str(&json).unwrap();
            assert_eq!(a, back);
        }
    }

    #[test]
    fn test_self_hash_deterministic() {
        let h1 = hex_hash("hello world");
        let h2 = hex_hash("hello world");
        assert_eq!(h1, h2);
        let h3 = hex_hash("hello world!");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_decision_id_unique() {
        let mut rec = DecisionRecorder::new();
        let ids: Vec<Uuid> = (0..100)
            .map(|_| {
                rec.record_decision(DecisionRecordParams {
                    agent_id: "test".into(),
                    action: DecisionAction::Other("x".into()),
                    inputs: DecisionInputs {
                        params: HashMap::new(),
                    },
                    rules_applied: vec![],
                    weights: HashMap::new(),
                    candidates: vec![],
                    confidence: 1.0,
                    result: DecisionResult {
                        selected_id: "x".into(),
                        rejected_ids: vec![],
                        metadata: HashMap::new(),
                    },
                    explanation: "".into(),
                    prev_hash: String::new(),
                })
            })
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "UUIDs must be unique");
    }
}
