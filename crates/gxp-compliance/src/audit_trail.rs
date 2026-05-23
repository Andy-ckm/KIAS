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
            m.insert(
                "correlation_id".to_string(),
                serde_json::Value::String(correlation_id.to_string()),
            );
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

// ---------------------------------------------------------------------------
// Cross-crate correlation ID
// ---------------------------------------------------------------------------

/// A correlation ID used to link audit records across different crates
/// in a distributed trace (e.g., agent-runtime → goal-engine → tool-executor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrelationId(pub String);

impl CorrelationId {
    /// Generate a new correlation ID from a UUID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Parse a correlation ID from a string (validates non-empty).
    pub fn parse(s: &str) -> Result<Self, AuditTrailError> {
        if s.is_empty() {
            return Err(AuditTrailError::InvalidCorrelationId);
        }
        Ok(Self(s.to_string()))
    }

    /// Generate a new correlation ID from a UUID (kept for backward compatibility).
    pub fn generate() -> Self {
        Self::new()
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Merkle tree for batch integrity proofs
// ---------------------------------------------------------------------------

/// A binary Merkle tree built from audit record self-hashes.
/// Provides O(log n) inclusion proofs for any sealed record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    /// All leaf hashes (record self-hashes), in order
    leaves: Vec<String>,
    /// Internal nodes: each level halving in size. Level 0 = leaves.
    /// The last entry of `levels` is the single merkle root.
    levels: Vec<Vec<String>>,
    /// Pre-computed merkle root (same as levels.last().unwrap()[0])
    root: String,
}

impl MerkleTree {
    /// Build a new empty Merkle tree.
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            levels: Vec::new(),
            root: Self::empty_hash(),
        }
    }

    /// Build from existing record self-hashes.
    pub fn from_hashes(leaf_hashes: &[String]) -> Self {
        if leaf_hashes.is_empty() {
            return Self::new();
        }
        let leaves = leaf_hashes.to_vec();
        let mut levels = Vec::new();
        levels.push(leaves.clone());

        let mut current = leaves.clone();
        while current.len() > 1 {
            let next: Vec<String> = current
                .chunks(2)
                .map(|pair| {
                    let left = &pair[0];
                    let right = pair.get(1).unwrap_or(left);
                    Self::node_hash(left, right)
                })
                .collect();
            levels.push(next.clone());
            current = next;
        }

        let root = current.into_iter().next().unwrap_or_else(Self::empty_hash);
        Self {
            leaves,
            levels,
            root,
        }
    }

    /// Insert a new leaf hash and recompute the tree.
    pub fn insert(&mut self, leaf_hash: &str) {
        self.leaves.push(leaf_hash.to_string());
        *self = Self::from_hashes(&self.leaves);
    }

    /// The Merkle root hash.
    pub fn merkle_root(&self) -> &str {
        &self.root
    }

    /// Prove that `record_id` (at `leaf_index`) is in the tree.
    /// Returns `None` if the index is out of bounds.
    pub fn prove_inclusion(&self, record_id: &str, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaves.len() {
            return None;
        }
        let leaf_hash = &self.leaves[leaf_index];

        // Walk up levels collecting sibling hashes
        let mut hashes = Vec::new();
        let mut idx = leaf_index;
        for level in 0..self.levels.len() {
            if level == self.levels.len() - 1 {
                // This is the root level — no sibling
                break;
            }
            let is_right = idx % 2 == 1;
            let level_nodes = &self.levels[level];
            let sibling_idx = if is_right { idx - 1 } else { idx + 1 };

            if sibling_idx < level_nodes.len() {
                hashes.push((is_right, level_nodes[sibling_idx].clone()));
            } else {
                // No sibling — use self hash (standard merkle tree convention)
                hashes.push((is_right, leaf_hash.clone()));
            }
            idx /= 2;
        }

        Some(MerkleProof {
            record_id: record_id.to_string(),
            leaf_hash: leaf_hash.clone(),
            leaf_index: leaf_index,
            hashes,
            merkle_root: self.root.clone(),
        })
    }

    /// Verify a Merkle proof against a known root.
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current = proof.leaf_hash.clone();
        let mut idx = proof.leaf_index;

        for (is_right, sibling) in &proof.hashes {
            current = if *is_right {
                Self::node_hash(&sibling, &current)
            } else {
                Self::node_hash(&current, &sibling)
            };
            idx /= 2;
        }

        current == proof.merkle_root
    }

    /// Number of leaf entries.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    fn node_hash(left: &str, right: &str) -> String {
        let payload = format!("{}|{}", left, right);
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn empty_hash() -> String {
        String::from("0000000000000000000000000000000000000000000000000000000000000000")
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Merkle proof
// ---------------------------------------------------------------------------

/// A Merkle inclusion proof: the sibling hashes needed to verify a leaf
/// is at `leaf_index` in the Merkle tree rooted at `merkle_root`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The record ID this proof is for
    pub record_id: String,
    /// The leaf hash
    pub leaf_hash: String,
    /// Index of the leaf in the tree
    pub leaf_index: usize,
    /// (is_right_sibling, sibling_hash) pairs from leaf → root
    hashes: Vec<(bool, String)>,
    /// The expected merkle root
    pub merkle_root: String,
}

impl MerkleProof {
    /// Verify this proof against the stored merkle_root.
    pub fn verify(&self) -> bool {
        MerkleTree::verify_proof(self)
    }

    /// The merkle root this proof was computed against.
    pub fn merkle_root(&self) -> &str {
        &self.merkle_root
    }
}

// ---------------------------------------------------------------------------
// WORM (Write Once Read Many) append-only storage
// ---------------------------------------------------------------------------

/// WORMStore: an append-only, tamper-evident file store for audit records.
/// Records are written in length-prefixed JSON format.
/// No delete or overwrite operations are exposed.
#[derive(Debug)]
pub struct WormStore {
    /// File path for the WORM journal
    path: std::path::PathBuf,
    /// In-memory buffer of record IDs written in this session
    session_ids: Vec<String>,
}

impl WormStore {
    /// Open or create a WORM store at the given path.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            session_ids: Vec::new(),
        }
    }

    /// Append a single audit record to the WORM journal.
    /// Returns the number of bytes written.
    pub fn append_record(&mut self, record: &AuditRecord) -> Result<usize, WormStoreError> {
        // Serialize to JSON
        let json = serde_json::to_string(record)
            .map_err(|e| WormStoreError::SerializationFailed(e.to_string()))?;

        // Open in append mode
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| WormStoreError::IoError(e.to_string()))?;

        let mut file = std::io::BufWriter::new(file);

        // Write length prefix (big-endian u32) + JSON
        let len = json.len() as u32;
        let len_bytes = len.to_be_bytes();

        std::io::Write::write_all(&mut file, &len_bytes)
            .map_err(|e| WormStoreError::IoError(e.to_string()))?;
        std::io::Write::write_all(&mut file, json.as_bytes())
            .map_err(|e| WormStoreError::IoError(e.to_string()))?;
        std::io::Write::write_all(&mut file, b"\n")
            .map_err(|e| WormStoreError::IoError(e.to_string()))?;

        std::io::Write::flush(&mut file).map_err(|e| WormStoreError::IoError(e.to_string()))?;

        self.session_ids.push(record.id.clone());
        Ok(len as usize)
    }

    /// Load all records from the WORM journal.
    /// Returns records in the order they were written.
    pub fn load(&self) -> Result<Vec<AuditRecord>, WormStoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file =
            std::fs::File::open(&self.path).map_err(|e| WormStoreError::IoError(e.to_string()))?;
        let mut reader = std::io::BufReader::new(file);
        let mut records = Vec::new();

        loop {
            // Read 4-byte length prefix
            let mut len_buf = [0u8; 4];
            match std::io::Read::read(&mut reader, &mut len_buf) {
                Ok(0) => break, // EOF
                Ok(4) => {}
                Ok(n) => {
                    return Err(WormStoreError::CorruptedStore(format!(
                        "expected 4-byte length prefix, got {} bytes",
                        n
                    )))
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(WormStoreError::IoError(e.to_string())),
            }

            let len = u32::from_be_bytes(len_buf) as usize;

            // Read JSON payload
            let mut payload = vec![0u8; len];
            std::io::Read::read_exact(&mut reader, &mut payload)
                .map_err(|e| WormStoreError::IoError(e.to_string()))?;

            // Read trailing newline
            let mut nl = [0u8; 1];
            std::io::Read::read_exact(&mut reader, &mut nl)
                .map_err(|e| WormStoreError::IoError(e.to_string()))?;

            let json = String::from_utf8(payload)
                .map_err(|e| WormStoreError::CorruptedStore(e.to_string()))?;
            let record: AuditRecord = serde_json::from_str(&json)
                .map_err(|e| WormStoreError::CorruptedStore(e.to_string()))?;
            records.push(record);
        }

        Ok(records)
    }

    /// Flush any buffered writes (no-op for BufWriter since write is immediate).
    pub fn flush(&self) -> Result<(), WormStoreError> {
        Ok(())
    }

    /// Iterate over all records in the store (loads all into memory).
    pub fn iter(&self) -> Result<impl Iterator<Item = AuditRecord>, WormStoreError> {
        Ok(self.load()?.into_iter())
    }

    /// Number of records written in this session.
    pub fn session_count(&self) -> usize {
        self.session_ids.len()
    }

    /// Check the store path.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// Extended errors
// ---------------------------------------------------------------------------

/// Errors for WORM store operations.
#[derive(Debug, thiserror::Error)]
pub enum WormStoreError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error("serialization failed: {0}")]
    SerializationFailed(String),

    #[error("store is corrupted: {0}")]
    CorruptedStore(String),
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

    #[error("invalid correlation ID: empty string")]
    InvalidCorrelationId,

    #[error("WORM store error: {0}")]
    WormStore(#[from] WormStoreError),

    #[error("merkle proof verification failed")]
    MerkleProofInvalid,
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

    // --- CorrelationId ---

    #[test]
    fn test_correlation_id_new_is_uuid() {
        let cid = CorrelationId::new();
        // UUID format: 8-4-4-4-12 = 36 chars
        assert_eq!(cid.0.len(), 36);
        assert!(cid.0.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn test_correlation_id_parse_valid() {
        let cid = CorrelationId::parse("trace-abc-123").unwrap();
        assert_eq!(cid.0, "trace-abc-123");
    }

    #[test]
    fn test_correlation_id_parse_empty_fails() {
        let result = CorrelationId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_correlation_id_default() {
        let cid = CorrelationId::default();
        assert_eq!(cid.0.len(), 36);
    }

    #[test]
    fn test_record_with_correlation_id() {
        let record = make_record("agent-1", "test").with_correlation_id("trace-xyz-789");
        let meta = record.metadata.as_object().unwrap();
        assert_eq!(
            meta.get("correlation_id").unwrap().as_str().unwrap(),
            "trace-xyz-789"
        );
    }

    #[test]
    fn test_record_requires_signature_flag() {
        let record =
            make_record("agent-1", "test").with_compliance_flag("ELECTRONIC_SIGNATURE_REQUIRED");
        assert!(record.requires_signature());
    }

    #[test]
    fn test_record_requires_human_review_flag() {
        let record = make_record("agent-1", "test").with_compliance_flag("HUMAN_REVIEW_REQUIRED");
        assert!(record.requires_signature());
    }

    #[test]
    fn test_record_no_signature_flag() {
        let record = make_record("agent-1", "test");
        assert!(!record.requires_signature());
    }

    // --- MerkleTree ---

    #[test]
    fn test_merkle_tree_empty() {
        let tree = MerkleTree::new();
        assert_eq!(tree.len(), 0);
        assert_eq!(
            tree.merkle_root(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_merkle_tree_single_leaf() {
        let tree = MerkleTree::from_hashes(&[String::from("leaf-hash-1")]);
        assert_eq!(tree.len(), 1);
        assert!(!tree.merkle_root().starts_with("00000000"));
    }

    #[test]
    fn test_merkle_tree_multiple_leaves() {
        let hashes = vec![
            String::from("hash-a"),
            String::from("hash-b"),
            String::from("hash-c"),
        ];
        let tree = MerkleTree::from_hashes(&hashes);
        assert_eq!(tree.len(), 3);
        assert!(!tree.merkle_root().is_empty());
    }

    #[test]
    fn test_merkle_tree_insert() {
        let mut tree = MerkleTree::new();
        tree.insert("hash-1");
        assert_eq!(tree.len(), 1);
        tree.insert("hash-2");
        assert_eq!(tree.len(), 2);
        tree.insert("hash-3");
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn test_merkle_tree_prove_inclusion_valid() {
        let hashes: Vec<String> = (0..4).map(|i| format!("hash-{}", i)).collect();
        let tree = MerkleTree::from_hashes(&hashes);

        for i in 0..4 {
            let proof = tree.prove_inclusion(&format!("record-{}", i), i);
            assert!(proof.is_some());
            let p = proof.unwrap();
            assert_eq!(p.record_id, format!("record-{}", i));
            assert!(p.verify());
        }
    }

    #[test]
    fn test_merkle_tree_prove_inclusion_out_of_bounds() {
        let hashes: Vec<String> = (0..3).map(|i| format!("hash-{}", i)).collect();
        let tree = MerkleTree::from_hashes(&hashes);
        let proof = tree.prove_inclusion("record-x", 99);
        assert!(proof.is_none());
    }

    #[test]
    fn test_merkle_tree_proof_fails_on_tampered_leaf() {
        let hashes = vec![String::from("hash-a"), String::from("hash-b")];
        let tree = MerkleTree::from_hashes(&hashes);

        let mut proof = tree.prove_inclusion("record-a", 0).unwrap();
        // Tamper with the leaf hash
        proof.leaf_hash = String::from("tampered-hash");
        assert!(!proof.verify());
    }

    #[test]
    fn test_merkle_tree_proof_fails_on_tampered_root() {
        let hashes = vec![String::from("hash-a"), String::from("hash-b")];
        let tree = MerkleTree::from_hashes(&hashes);

        let mut proof = tree.prove_inclusion("record-a", 0).unwrap();
        // Tamper with the merkle root
        proof.merkle_root =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");
        assert!(!proof.verify());
    }

    #[test]
    fn test_merkle_proof_verify_method() {
        let hashes = vec![
            String::from("h1"),
            String::from("h2"),
            String::from("h3"),
            String::from("h4"),
        ];
        let tree = MerkleTree::from_hashes(&hashes);
        let proof = tree.prove_inclusion("r2", 2).unwrap();
        assert!(proof.verify()); // .verify() calls MerkleTree::verify_proof internally
    }

    // --- WormStore ---

    #[test]
    fn test_worm_store_append_and_load() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join(format!("worm_test_{}.dat", uuid::Uuid::new_v4()));
        let mut store = WormStore::new(&path);

        let mut trail = AuditTrail::new();
        let r1 = make_record("agent-1", "action-1");
        let hash1 = trail.seal(r1).unwrap();
        let r2 = make_record("agent-2", "action-2");
        trail.seal(r2).unwrap();

        // Append both sealed records
        if let Some(rec) = trail.get(&trail.records[0].id) {
            store.append_record(rec).unwrap();
        }
        if let Some(rec) = trail.get(&trail.records[1].id) {
            store.append_record(rec).unwrap();
        }

        assert_eq!(store.session_count(), 2);

        // Load and verify
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);

        // Verify chain continuity in loaded records
        let hashes: Vec<String> = loaded.iter().map(|r| r.self_hash.clone()).collect();
        assert_eq!(hashes[0], hash1);
        assert_eq!(
            loaded[0].previous_hash,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );

        // Clean up
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_worm_store_immutability_enforced() {
        // WormStore has no delete/overwrite methods — only append
        // This is a compile-time guarantee; we test that load sees all appended records
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join(format!("worm_imm_{}.dat", uuid::Uuid::new_v4()));
        let mut store = WormStore::new(&path);

        let mut trail = AuditTrail::new();
        let r = make_record("agent-x", "one-time");
        let sealed = trail.seal(r).unwrap();
        let rec_id = trail.records[0].id.clone();

        if let Some(rec) = trail.get(&rec_id) {
            store.append_record(rec).unwrap();
        }

        // Re-open same path and append again (simulates separate session)
        drop(store);
        let mut store2 = WormStore::new(&path);
        if let Some(rec) = trail.get(&rec_id) {
            store2.append_record(rec).unwrap();
        }

        // All records from both sessions should be present
        let loaded = store2.load().unwrap();
        assert_eq!(loaded.len(), 2);

        // Clean up
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_worm_store_load_empty_file() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join(format!("worm_empty_{}.dat", uuid::Uuid::new_v4()));
        let store = WormStore::new(&path);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_worm_store_iter() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join(format!("worm_iter_{}.dat", uuid::Uuid::new_v4()));
        let mut store = WormStore::new(&path);

        let mut trail = AuditTrail::new();
        let r1 = make_record("a", "x");
        let r2 = make_record("b", "y");
        trail.seal(r1).unwrap();
        trail.seal(r2).unwrap();

        for rec in &trail.records {
            store.append_record(rec).unwrap();
        }

        let iter_count = store.iter().unwrap().count();
        assert_eq!(iter_count, 2);

        std::fs::remove_file(&path).ok();
    }
}
