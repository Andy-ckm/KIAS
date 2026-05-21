//! # Evidence Chain (DTF-inspired)
//!
//! Immutable, append-only evidence chain for audit compliance.
//! Records the complete lifecycle: Intent → Proof → Consensus → Execution → Result.
//!
//! Based on the DTF paper (2605.15228): "Verifiable Agentic Infrastructure"
//! Key invariant: Evidence Completeness — every intent produces exactly one evidence chain.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

// ── Errors ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// Attempted to append to a finalized chain.
    ChainFinalized,
    /// The event's previous_hash does not match the chain's tail hash.
    HashMismatch { expected: String, got: String },
    /// The chain is empty when an operation requires events.
    ChainEmpty,
    /// The event type is invalid for the current chain state.
    InvalidTransition(String),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChainFinalized => write!(f, "Evidence chain is finalized"),
            Self::HashMismatch { expected, got } => {
                write!(f, "Hash mismatch: expected {expected}, got {got}")
            }
            Self::ChainEmpty => write!(f, "Evidence chain is empty"),
            Self::InvalidTransition(msg) => write!(f, "Invalid transition: {msg}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

// ── Evidence Event ─────────────────────────────────────────────────────

/// Lifecycle stages of an agent action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceEventType {
    /// Agent expressed an intent to perform an action.
    IntentDeclared,
    /// Justification proof was constructed.
    ProofConstructed,
    /// Consensus evaluation was performed.
    ConsensusEvaluated,
    /// Execution was authorized.
    ExecutionAuthorized,
    /// Action was executed.
    ExecutionCompleted,
    /// Action failed.
    ExecutionFailed,
    /// Chain was manually escalated (e.g. to human review).
    Escalated,
}

/// A single immutable event in the evidence chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceEvent {
    /// Sequential index in the chain.
    pub index: u64,
    /// Type of lifecycle event.
    pub event_type: EvidenceEventType,
    /// SHA-256 hash of the previous event (genesis uses all zeros).
    pub previous_hash: String,
    /// SHA-256 hash of this event's payload.
    pub payload_hash: String,
    /// When this event was recorded.
    pub timestamp: DateTime<Utc>,
    /// Subject (agent/user) that triggered this event.
    pub subject: String,
    /// Human-readable description.
    pub description: String,
    /// Opaque payload (JSON-serialized).
    pub payload: serde_json::Value,
    /// SHA-256 hash of this entire event (for chaining).
    pub event_hash: String,
}

// ── Evidence Chain ─────────────────────────────────────────────────────

/// An immutable, append-only evidence chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChain {
    /// Unique chain identifier (typically the intent/action ID).
    pub chain_id: String,
    /// The events in this chain (append-only).
    events: Vec<EvidenceEvent>,
    /// Whether this chain has been finalized (no more events accepted).
    pub finalized: bool,
    /// When the chain was created.
    pub created_at: DateTime<Utc>,
}

impl EvidenceChain {
    /// Create a new evidence chain.
    pub fn new(chain_id: &str) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            events: Vec::new(),
            finalized: false,
            created_at: Utc::now(),
        }
    }

    /// Append a new event to the chain.
    ///
    /// Validates that:
    /// 1. The chain is not finalized.
    /// 2. The event's previous_hash matches the tail hash (or all-zeros for genesis).
    /// 3. The event type follows a valid transition.
    pub fn append(
        &mut self,
        event_type: EvidenceEventType,
        subject: &str,
        description: &str,
        payload: serde_json::Value,
    ) -> Result<&EvidenceEvent, EvidenceError> {
        if self.finalized {
            return Err(EvidenceError::ChainFinalized);
        }

        // Validate transition
        self.validate_transition(&event_type)?;

        let index = self.events.len() as u64;
        let previous_hash = self
            .events
            .last()
            .map(|e| e.event_hash.clone())
            .unwrap_or_else(|| "0".repeat(64));

        let payload_hash = sha256_hex(
            serde_json::to_string(&payload)
                .unwrap_or_default()
                .as_bytes(),
        );

        let event_hash = compute_event_hash(
            index,
            &event_type,
            &previous_hash,
            &payload_hash,
            subject,
            description,
        );

        let event = EvidenceEvent {
            index,
            event_type,
            previous_hash,
            payload_hash,
            timestamp: Utc::now(),
            subject: subject.to_string(),
            description: description.to_string(),
            payload,
            event_hash,
        };

        self.events.push(event);
        Ok(self.events.last().unwrap())
    }

    /// Finalize the chain (make it immutable).
    pub fn finalize(&mut self) {
        self.finalized = true;
    }

    /// Get all events in the chain.
    pub fn events(&self) -> &[EvidenceEvent] {
        &self.events
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Verify the integrity of the entire chain.
    ///
    /// Checks:
    /// 1. Each event's previous_hash matches the previous event's event_hash.
    /// 2. Each event's event_hash is correctly computed.
    pub fn verify_integrity(&self) -> Result<(), EvidenceError> {
        for (i, event) in self.events.iter().enumerate() {
            // Check previous hash linkage
            let expected_prev = if i == 0 {
                "0".repeat(64)
            } else {
                self.events[i - 1].event_hash.clone()
            };
            if event.previous_hash != expected_prev {
                return Err(EvidenceError::HashMismatch {
                    expected: expected_prev,
                    got: event.previous_hash.clone(),
                });
            }

            // Verify event hash
            let recomputed = compute_event_hash(
                event.index,
                &event.event_type,
                &event.previous_hash,
                &event.payload_hash,
                &event.subject,
                &event.description,
            );
            if event.event_hash != recomputed {
                return Err(EvidenceError::HashMismatch {
                    expected: recomputed,
                    got: event.event_hash.clone(),
                });
            }
        }
        Ok(())
    }

    /// Get the tail hash (hash of the last event).
    pub fn tail_hash(&self) -> Option<&str> {
        self.events.last().map(|e| e.event_hash.as_str())
    }

    /// Validate event type transition.
    fn validate_transition(&self, new_type: &EvidenceEventType) -> Result<(), EvidenceError> {
        use EvidenceEventType::*;
        let last_type = self.events.last().map(|e| &e.event_type);

        match (last_type, new_type) {
            // Genesis: first event must be IntentDeclared
            (None, IntentDeclared) => Ok(()),
            (None, _) => Err(EvidenceError::InvalidTransition(
                "Chain must start with IntentDeclared".to_string(),
            )),
            // Valid transitions
            (Some(IntentDeclared), ProofConstructed) => Ok(()),
            (Some(ProofConstructed), ConsensusEvaluated) => Ok(()),
            (Some(ConsensusEvaluated), ExecutionAuthorized) => Ok(()),
            (Some(ExecutionAuthorized), ExecutionCompleted) => Ok(()),
            (Some(ExecutionAuthorized), ExecutionFailed) => Ok(()),
            // Escalation can happen from any state
            (Some(_), Escalated) => Ok(()),
            // Invalid
            (Some(from), to) => Err(EvidenceError::InvalidTransition(format!(
                "Cannot transition from {:?} to {:?}",
                from, to
            ))),
        }
    }
}

// ── Evidence Store ─────────────────────────────────────────────────────

/// In-memory store for evidence chains.
#[derive(Debug, Default)]
pub struct EvidenceStore {
    chains: std::collections::HashMap<String, EvidenceChain>,
}

impl EvidenceStore {
    pub fn new() -> Self {
        Self {
            chains: std::collections::HashMap::new(),
        }
    }

    /// Create a new chain.
    pub fn create_chain(&mut self, chain_id: &str) -> &mut EvidenceChain {
        self.chains
            .entry(chain_id.to_string())
            .or_insert_with(|| EvidenceChain::new(chain_id))
    }

    /// Get a chain by ID.
    pub fn get_chain(&self, chain_id: &str) -> Option<&EvidenceChain> {
        self.chains.get(chain_id)
    }

    /// Get a mutable chain by ID.
    pub fn get_chain_mut(&mut self, chain_id: &str) -> Option<&mut EvidenceChain> {
        self.chains.get_mut(chain_id)
    }

    /// List all chain IDs.
    pub fn list_chains(&self) -> Vec<String> {
        self.chains.keys().cloned().collect()
    }

    /// Verify all chains' integrity.
    pub fn verify_all(&self) -> Vec<(String, Result<(), EvidenceError>)> {
        self.chains
            .iter()
            .map(|(id, chain)| (id.clone(), chain.verify_integrity()))
            .collect()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn compute_event_hash(
    index: u64,
    event_type: &EvidenceEventType,
    previous_hash: &str,
    payload_hash: &str,
    subject: &str,
    description: &str,
) -> String {
    let data = format!(
        "{}|{:?}|{}|{}|{}|{}",
        index, event_type, previous_hash, payload_hash, subject, description
    );
    sha256_hex(data.as_bytes())
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_chain() {
        let chain = EvidenceChain::new("action-001");
        assert_eq!(chain.chain_id, "action-001");
        assert!(chain.is_empty());
        assert!(!chain.finalized);
    }

    #[test]
    fn test_append_genesis_event() {
        let mut chain = EvidenceChain::new("action-001");
        let event = chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "User requested file deletion",
                serde_json::json!({"file": "/tmp/test.txt"}),
            )
            .unwrap();

        assert_eq!(event.index, 0);
        assert_eq!(event.event_type, EvidenceEventType::IntentDeclared);
        assert_eq!(event.previous_hash, "0".repeat(64));
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_full_lifecycle() {
        let mut chain = EvidenceChain::new("action-002");

        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent to deploy",
                serde_json::json!({"target": "prod"}),
            )
            .unwrap();

        chain
            .append(
                EvidenceEventType::ProofConstructed,
                "agent-1",
                "Proof: resource available, policy allows",
                serde_json::json!({"proof_id": "jp-001"}),
            )
            .unwrap();

        chain
            .append(
                EvidenceEventType::ConsensusEvaluated,
                "evaluator-1",
                "2 of 3 evaluators approved",
                serde_json::json!({"votes": {"approve": 2, "reject": 1}}),
            )
            .unwrap();

        chain
            .append(
                EvidenceEventType::ExecutionAuthorized,
                "system",
                "Authorization granted",
                serde_json::json!({"auth_id": "auth-001"}),
            )
            .unwrap();

        chain
            .append(
                EvidenceEventType::ExecutionCompleted,
                "agent-1",
                "Deployment successful",
                serde_json::json!({"deploy_id": "dep-001"}),
            )
            .unwrap();

        assert_eq!(chain.len(), 5);
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_invalid_transition() {
        let mut chain = EvidenceChain::new("action-003");
        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent",
                serde_json::json!({}),
            )
            .unwrap();

        // Cannot go directly from IntentDeclared to ExecutionCompleted
        let result = chain.append(
            EvidenceEventType::ExecutionCompleted,
            "agent-1",
            "Skip steps",
            serde_json::json!({}),
        );
        assert!(matches!(
            result.unwrap_err(),
            EvidenceError::InvalidTransition(_)
        ));
    }

    #[test]
    fn test_escalation_from_any_state() {
        let mut chain = EvidenceChain::new("action-004");
        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent",
                serde_json::json!({}),
            )
            .unwrap();

        // Can escalate from IntentDeclared
        let result = chain.append(
            EvidenceEventType::Escalated,
            "human-1",
            "Needs human review",
            serde_json::json!({}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_chain_finalized() {
        let mut chain = EvidenceChain::new("action-005");
        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent",
                serde_json::json!({}),
            )
            .unwrap();
        chain.finalize();

        let result = chain.append(
            EvidenceEventType::ProofConstructed,
            "agent-1",
            "Proof",
            serde_json::json!({}),
        );
        assert_eq!(result.unwrap_err(), EvidenceError::ChainFinalized);
    }

    #[test]
    fn test_integrity_verification() {
        let mut chain = EvidenceChain::new("action-006");
        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent",
                serde_json::json!({}),
            )
            .unwrap();
        assert!(chain.verify_integrity().is_ok());

        // Tamper with the event
        chain.events[0].description = "TAMPERED".to_string();
        assert!(chain.verify_integrity().is_err());
    }

    #[test]
    fn test_must_start_with_intent() {
        let mut chain = EvidenceChain::new("action-007");
        let result = chain.append(
            EvidenceEventType::ProofConstructed,
            "agent-1",
            "Proof first",
            serde_json::json!({}),
        );
        assert!(matches!(
            result.unwrap_err(),
            EvidenceError::InvalidTransition(_)
        ));
    }

    #[test]
    fn test_tail_hash() {
        let mut chain = EvidenceChain::new("action-008");
        assert!(chain.tail_hash().is_none());

        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "agent-1",
                "Intent",
                serde_json::json!({}),
            )
            .unwrap();
        assert!(chain.tail_hash().is_some());
    }

    #[test]
    fn test_evidence_store() {
        let mut store = EvidenceStore::new();
        store.create_chain("c1");
        store.create_chain("c2");
        assert_eq!(store.list_chains().len(), 2);

        let chain = store.get_chain_mut("c1").unwrap();
        chain
            .append(
                EvidenceEventType::IntentDeclared,
                "a1",
                "test",
                serde_json::json!({}),
            )
            .unwrap();

        let results = store.verify_all();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
    }

    #[test]
    fn test_error_display() {
        let err = EvidenceError::HashMismatch {
            expected: "abc".to_string(),
            got: "xyz".to_string(),
        };
        assert!(err.to_string().contains("abc"));
        assert!(err.to_string().contains("xyz"));
    }
}
