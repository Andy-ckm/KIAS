//! # Checkpoint Audit Trail — Merkle Hash Chain
//!
//! Provides tamper-evident logging of checkpoint operations using a Merkle-style
//! hash chain. Each audit entry links to the previous entry's hash, forming an
//! immutable chain that detects any retroactive modification.
//!
//! ## Design
//!
//! ```text
//! Entry[0] ──hash──▶ Entry[1] ──hash──▶ Entry[2] ──hash──▶ ...
//!   │                  │                  │
//!   ├─ prev_hash=""    ├─ prev_hash=H0    ├─ prev_hash=H1
//!   ├─ op=Create       ├─ op=Save         ├─ op=Restore
//!   └─ data_hash=D0    └─ data_hash=D1    └─ data_hash=D2
//! ```
//!
//! ## Compliance
//!
//! - **FDA 21 CFR Part 11**: Electronic records must be tamper-evident
//! - **EU Annex 11**: Audit trail must be immutable and verifiable
//! - **ALCOA+**: Attributable, Contemporaneous, Original, Accurate

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Types ───────────────────────────────────────────────────────────────

/// Types of checkpoint operations that are audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointOperation {
    /// A new checkpoint was created.
    Create,
    /// An existing checkpoint was updated.
    Save,
    /// A checkpoint was restored (workflow resumed).
    Restore,
    /// A checkpoint was deleted.
    Delete,
    /// A WAL record was written.
    WalWrite,
    /// A WAL record was replayed during recovery.
    WalReplay,
    /// A delta was applied.
    DeltaApply,
    /// Chain integrity was verified.
    IntegrityCheck,
}

impl std::fmt::Display for CheckpointOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Save => write!(f, "save"),
            Self::Restore => write!(f, "restore"),
            Self::Delete => write!(f, "delete"),
            Self::WalWrite => write!(f, "wal_write"),
            Self::WalReplay => write!(f, "wal_replay"),
            Self::DeltaApply => write!(f, "delta_apply"),
            Self::IntegrityCheck => write!(f, "integrity_check"),
        }
    }
}

/// A single audit entry in the Merkle hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequential index in the chain.
    pub index: u64,
    /// SHA-256 hash of the previous entry (empty string for genesis).
    pub prev_hash: String,
    /// The checkpoint operation that was performed.
    pub operation: CheckpointOperation,
    /// Workflow ID this operation relates to.
    pub workflow_id: String,
    /// Checkpoint ID (if applicable).
    pub checkpoint_id: Option<String>,
    /// SHA-256 hash of the operation data (checkpoint content, WAL record, etc.).
    pub data_hash: String,
    /// Human-readable description of the operation.
    pub description: String,
    /// Who or what triggered this operation.
    pub actor: String,
    /// When the operation occurred.
    pub timestamp: DateTime<Utc>,
    /// SHA-256 hash of this entire entry (for chaining).
    pub entry_hash: String,
}

/// Merkle hash chain for checkpoint audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditChain {
    /// Unique chain identifier (typically the workflow ID or "system").
    pub chain_id: String,
    /// The audit entries (append-only).
    entries: Vec<AuditEntry>,
    /// When the chain was created.
    pub created_at: DateTime<Utc>,
}

impl AuditChain {
    /// Create a new audit chain.
    pub fn new(chain_id: &str) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            entries: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Append a new audit entry to the chain.
    pub fn append(
        &mut self,
        operation: CheckpointOperation,
        workflow_id: &str,
        checkpoint_id: Option<&str>,
        data_hash: &str,
        description: &str,
        actor: &str,
    ) -> &AuditEntry {
        let index = self.entries.len() as u64;
        let prev_hash = self
            .entries
            .last()
            .map(|e| e.entry_hash.clone())
            .unwrap_or_default();

        let entry_hash = compute_entry_hash(
            index,
            &prev_hash,
            &operation,
            workflow_id,
            data_hash,
            actor,
        );

        let entry = AuditEntry {
            index,
            prev_hash,
            operation,
            workflow_id: workflow_id.to_string(),
            checkpoint_id: checkpoint_id.map(|s| s.to_string()),
            data_hash: data_hash.to_string(),
            description: description.to_string(),
            actor: actor.to_string(),
            timestamp: Utc::now(),
            entry_hash,
        };

        self.entries.push(entry);
        self.entries.last().expect("entry just pushed")
    }

    /// Get all entries.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the tail hash (hash of the last entry).
    pub fn tail_hash(&self) -> Option<&str> {
        self.entries.last().map(|e| e.entry_hash.as_str())
    }

    /// Verify the integrity of the entire chain.
    ///
    /// Checks:
    /// 1. Each entry's prev_hash matches the previous entry's entry_hash.
    /// 2. Each entry's entry_hash is correctly computed.
    pub fn verify(&self) -> Result<(), AuditError> {
        for (i, entry) in self.entries.iter().enumerate() {
            // Check prev hash linkage
            let expected_prev = if i == 0 {
                String::new()
            } else {
                self.entries[i - 1].entry_hash.clone()
            };
            if entry.prev_hash != expected_prev {
                return Err(AuditError::HashMismatch {
                    index: i as u64,
                    expected: expected_prev,
                    got: entry.prev_hash.clone(),
                });
            }

            // Verify entry hash
            let recomputed = compute_entry_hash(
                entry.index,
                &entry.prev_hash,
                &entry.operation,
                &entry.workflow_id,
                &entry.data_hash,
                &entry.actor,
            );
            if entry.entry_hash != recomputed {
                return Err(AuditError::HashMismatch {
                    index: i as u64,
                    expected: recomputed,
                    got: entry.entry_hash.clone(),
                });
            }
        }
        Ok(())
    }

    /// Get entries for a specific workflow.
    pub fn for_workflow(&self, workflow_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.workflow_id == workflow_id)
            .collect()
    }

    /// Get entries by operation type.
    pub fn by_operation(&self, op: &CheckpointOperation) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| &e.operation == op).collect()
    }
}

/// Audit chain errors.
#[derive(Debug, Clone)]
pub enum AuditError {
    /// Hash mismatch detected — chain is tampered.
    HashMismatch {
        index: u64,
        expected: String,
        got: String,
    },
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HashMismatch {
                index,
                expected,
                got,
            } => write!(
                f,
                "Audit chain tampered at entry {index}: expected {expected}, got {got}"
            ),
        }
    }
}

impl std::error::Error for AuditError {}

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

fn compute_entry_hash(
    index: u64,
    prev_hash: &str,
    operation: &CheckpointOperation,
    workflow_id: &str,
    data_hash: &str,
    actor: &str,
) -> String {
    let data = format!("{index}|{prev_hash}|{operation}|{workflow_id}|{data_hash}|{actor}");
    sha256_hex(data.as_bytes())
}

/// Hash arbitrary data to a hex string (for computing data_hash).
pub fn hash_data(data: &[u8]) -> String {
    sha256_hex(data)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_chain() {
        let chain = AuditChain::new("workflow-1");
        assert_eq!(chain.chain_id, "workflow-1");
        assert!(chain.is_empty());
        assert!(chain.tail_hash().is_none());
    }

    #[test]
    fn test_append_genesis() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            Some("cp-1"),
            "abc123",
            "Created checkpoint",
            "system",
        );
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.entries()[0].index, 0);
        assert_eq!(chain.entries()[0].prev_hash, "");
        assert_eq!(chain.entries()[0].operation, CheckpointOperation::Create);
    }

    #[test]
    fn test_chain_linkage() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            Some("cp-1"),
            "hash1",
            "Create",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            Some("cp-1"),
            "hash2",
            "Save",
            "system",
        );
        chain.append(
            CheckpointOperation::Restore,
            "wf-1",
            Some("cp-1"),
            "hash3",
            "Restore",
            "agent-1",
        );

        assert_eq!(chain.len(), 3);
        // Each entry's prev_hash should match previous entry's entry_hash
        assert_eq!(chain.entries()[1].prev_hash, chain.entries()[0].entry_hash);
        assert_eq!(chain.entries()[2].prev_hash, chain.entries()[1].entry_hash);
    }

    #[test]
    fn test_verify_integrity() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            Some("cp-1"),
            "hash1",
            "Create",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            Some("cp-1"),
            "hash2",
            "Save",
            "system",
        );
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn test_verify_tampered_chain() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            Some("cp-1"),
            "hash1",
            "Create",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            Some("cp-1"),
            "hash2",
            "Save",
            "system",
        );

        // Tamper with the chain
        let mut tampered = chain.clone();
        tampered.entries[1].data_hash = "TAMPERED".to_string();
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn test_tail_hash() {
        let mut chain = AuditChain::new("wf-1");
        assert!(chain.tail_hash().is_none());

        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            None,
            "hash1",
            "Create",
            "system",
        );
        let h1 = chain.tail_hash().unwrap().to_string();

        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            None,
            "hash2",
            "Save",
            "system",
        );
        let h2 = chain.tail_hash().unwrap().to_string();

        assert_ne!(h1, h2);
    }

    #[test]
    fn test_for_workflow() {
        let mut chain = AuditChain::new("system");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            None,
            "h1",
            "Create wf-1",
            "system",
        );
        chain.append(
            CheckpointOperation::Create,
            "wf-2",
            None,
            "h2",
            "Create wf-2",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            None,
            "h3",
            "Save wf-1",
            "system",
        );

        let wf1_entries = chain.for_workflow("wf-1");
        assert_eq!(wf1_entries.len(), 2);
        let wf2_entries = chain.for_workflow("wf-2");
        assert_eq!(wf2_entries.len(), 1);
    }

    #[test]
    fn test_by_operation() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            None,
            "h1",
            "Create",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            None,
            "h2",
            "Save",
            "system",
        );
        chain.append(
            CheckpointOperation::Save,
            "wf-1",
            None,
            "h3",
            "Save again",
            "system",
        );

        let saves = chain.by_operation(&CheckpointOperation::Save);
        assert_eq!(saves.len(), 2);
        let creates = chain.by_operation(&CheckpointOperation::Create);
        assert_eq!(creates.len(), 1);
    }

    #[test]
    fn test_operation_display() {
        assert_eq!(CheckpointOperation::Create.to_string(), "create");
        assert_eq!(CheckpointOperation::WalWrite.to_string(), "wal_write");
        assert_eq!(CheckpointOperation::IntegrityCheck.to_string(), "integrity_check");
    }

    #[test]
    fn test_hash_data() {
        let h1 = hash_data(b"hello");
        let h2 = hash_data(b"hello");
        let h3 = hash_data(b"world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_empty_chain_verify() {
        let chain = AuditChain::new("wf-1");
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn test_long_chain_integrity() {
        let mut chain = AuditChain::new("wf-1");
        for i in 0..100 {
            chain.append(
                CheckpointOperation::Save,
                "wf-1",
                Some(&format!("cp-{i}")),
                &format!("hash-{i}"),
                &format!("Save checkpoint {i}"),
                "system",
            );
        }
        assert_eq!(chain.len(), 100);
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut chain = AuditChain::new("wf-1");
        chain.append(
            CheckpointOperation::Create,
            "wf-1",
            Some("cp-1"),
            "hash1",
            "Create",
            "system",
        );

        let json = serde_json::to_string(&chain).unwrap();
        let back: AuditChain = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chain_id, "wf-1");
        assert_eq!(back.len(), 1);
        assert!(back.verify().is_ok());
    }
}
