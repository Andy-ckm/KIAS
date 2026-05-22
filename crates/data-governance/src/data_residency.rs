//! # Data Residency & Deletion Compliance
//!
//! Provides data residency policies, verifiable deletion proofs, and retention management.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Region ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region { US, EU, APAC, CN, GLOBAL }

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::US => write!(f, "US"),
            Region::EU => write!(f, "EU"),
            Region::APAC => write!(f, "APAC"),
            Region::CN => write!(f, "CN"),
            Region::GLOBAL => write!(f, "GLOBAL"),
        }
    }
}

// ── Data Residency Policy ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClassification { Public, Internal, Confidential, Restricted }

#[derive(Debug, Clone)]
pub struct DataResidencyPolicy {
    pub tenant_id: String,
    pub allowed_regions: Vec<Region>,
    pub restricted_regions: Vec<Region>,
    pub data_classification: DataClassification,
    pub enforce_from: chrono::DateTime<chrono::Utc>,
}

impl DataResidencyPolicy {
    pub fn new(tenant_id: &str, allowed_regions: Vec<Region>) -> Self {
        Self { tenant_id: tenant_id.to_string(), allowed_regions, restricted_regions: Vec::new(), data_classification: DataClassification::Internal, enforce_from: chrono::Utc::now() }
    }
    pub fn restrict_regions(mut self, regions: Vec<Region>) -> Self { self.restricted_regions = regions; self }
    pub fn classify(mut self, classification: DataClassification) -> Self { self.data_classification = classification; self }
    pub fn is_allowed_region(&self, region: Region) -> bool { self.allowed_regions.contains(&region) || self.allowed_regions.contains(&Region::GLOBAL) }
}

#[derive(Debug, Clone)]
pub enum ResidencyCheckResult {
    Compliant { tenant: String, node: String, region: Region },
    Violation { tenant: String, node: String, region: Region, reason: String },
    NoPolicy { tenant: String },
    UnknownNode { node: String },
}

impl ResidencyCheckResult {
    pub fn is_compliant(&self) -> bool { matches!(self, ResidencyCheckResult::Compliant { .. }) }
}

pub struct ResidencyChecker {
    policies: Arc<RwLock<HashMap<String, DataResidencyPolicy>>>,
    region_mappings: Arc<RwLock<HashMap<String, Region>>>,
}

impl Default for ResidencyChecker { fn default() -> Self { Self::new() } }

impl ResidencyChecker {
    pub fn new() -> Self {
        Self { policies: Arc::new(RwLock::new(HashMap::new())), region_mappings: Arc::new(RwLock::new(HashMap::new())) }
    }
    pub async fn register_policy(&self, policy: DataResidencyPolicy) {
        self.policies.write().await.insert(policy.tenant_id.clone(), policy);
    }
    pub async fn set_node_region(&self, node_id: &str, region: Region) {
        self.region_mappings.write().await.insert(node_id.to_string(), region);
    }
    pub async fn check_compliance(&self, tenant_id: &str, node_id: &str) -> ResidencyCheckResult {
        let policies = self.policies.read().await;
        let regions = self.region_mappings.read().await;
        match (policies.get(tenant_id), regions.get(node_id)) {
            (Some(policy), Some(node_region)) => {
                if policy.is_allowed_region(*node_region) {
                    ResidencyCheckResult::Compliant { tenant: tenant_id.to_string(), node: node_id.to_string(), region: *node_region }
                } else {
                    ResidencyCheckResult::Violation { tenant: tenant_id.to_string(), node: node_id.to_string(), region: *node_region, reason: format!("region {:?} not allowed", node_region) }
                }
            }
            (None, _) => ResidencyCheckResult::NoPolicy { tenant: tenant_id.to_string() },
            (_, None) => ResidencyCheckResult::UnknownNode { node: node_id.to_string() },
        }
    }
}

// ── Deletion Proof ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    pub witness_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionProof {
    pub proof_id: String,
    pub tenant_id: String,
    pub data_id: String,
    pub deletion_hash: String,
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub deleted_by: String,
    pub witnesses: Vec<WitnessRecord>,
    pub chain_hash: String,
}

impl DeletionProof {
    pub fn new(tenant_id: &str, data_id: &str, deleted_by: &str) -> Self {
        let proof_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now();
        let hash_input = format!("{}/{}/{}", tenant_id, data_id, timestamp.timestamp());
        let deletion_hash = format!("{:x}", blake3::hash(hash_input.as_bytes()));
        let chain_hash = format!("{:x}", blake3::hash(deletion_hash.as_bytes()));
        Self { proof_id, tenant_id: tenant_id.to_string(), data_id: data_id.to_string(), deletion_hash, deleted_at: timestamp, deleted_by: deleted_by.to_string(), witnesses: Vec::new(), chain_hash }
    }
    pub fn add_witness(&mut self, witness_id: &str, signature: &str) {
        self.witnesses.push(WitnessRecord { witness_id: witness_id.to_string(), timestamp: chrono::Utc::now(), signature: signature.to_string() });
        let input = format!("{}{}", self.chain_hash, signature);
        self.chain_hash = format!("{:x}", blake3::hash(input.as_bytes()));
    }
    pub fn verify(&self) -> bool {
        let hash_input = format!("{}/{}/{}", self.tenant_id, self.data_id, self.deleted_at.timestamp());
        let expected = format!("{:x}", blake3::hash(hash_input.as_bytes()));
        expected == self.deletion_hash
    }
}

pub struct DeletionProofStore {
    proofs: Arc<RwLock<HashMap<String, DeletionProof>>>,
}

impl Default for DeletionProofStore { fn default() -> Self { Self::new() } }

impl DeletionProofStore {
    pub fn new() -> Self { Self { proofs: Arc::new(RwLock::new(HashMap::new())) } }
    pub async fn store(&self, proof: DeletionProof) { self.proofs.write().await.insert(proof.proof_id.clone(), proof); }
    pub async fn get(&self, proof_id: &str) -> Option<DeletionProof> { self.proofs.read().await.get(proof_id).cloned() }
    pub async fn verify(&self, proof_id: &str) -> Option<bool> { self.proofs.read().await.get(proof_id).map(|p| p.verify()) }
    pub async fn list_for_tenant(&self, tenant_id: &str) -> Vec<DeletionProof> { self.proofs.read().await.values().filter(|p| p.tenant_id == tenant_id).cloned().collect() }
}

// ── Retention Policy ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub policy_id: String,
    pub tenant_id: String,
    pub data_type: String,
    pub retention_days: i64,
    pub auto_delete: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl RetentionPolicy {
    pub fn new(tenant_id: &str, data_type: &str, retention_days: i64) -> Self {
        Self { policy_id: uuid::Uuid::new_v4().to_string(), tenant_id: tenant_id.to_string(), data_type: data_type.to_string(), retention_days, auto_delete: true, created_at: chrono::Utc::now() }
    }
    pub fn with_manual_delete(mut self) -> Self { self.auto_delete = false; self }
    pub fn expires_at(&self) -> chrono::DateTime<chrono::Utc> { self.created_at + chrono::Duration::days(self.retention_days) }
    pub fn is_expired(&self) -> bool { chrono::Utc::now() > self.expires_at() }
}

#[derive(Debug, Clone)]
pub struct RetainedData {
    pub data_id: String,
    pub tenant_id: String,
    pub data_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub policy_id: String,
}

impl RetainedData {
    pub fn is_expired(&self, retention_days: i64) -> bool {
        let cutoff = self.created_at + chrono::Duration::days(retention_days);
        chrono::Utc::now() > cutoff
    }
}

pub struct RetentionManager {
    policies: Arc<RwLock<HashMap<String, RetentionPolicy>>>,
    data_items: Arc<RwLock<HashMap<String, RetainedData>>>,
    deletion_store: Arc<DeletionProofStore>,
}

impl Default for RetentionManager { fn default() -> Self { Self::new() } }

impl RetentionManager {
    pub fn new() -> Self {
        Self { policies: Arc::new(RwLock::new(HashMap::new())), data_items: Arc::new(RwLock::new(HashMap::new())), deletion_store: Arc::new(DeletionProofStore::new()) }
    }
    pub fn deletion_store(&self) -> &Arc<DeletionProofStore> { &self.deletion_store }
    pub async fn register_policy(&self, policy: RetentionPolicy) { self.policies.write().await.insert(policy.policy_id.clone(), policy); }
    pub async fn register_data(&self, data: RetainedData) { self.data_items.write().await.insert(data.data_id.clone(), data); }
    pub async fn check_expired(&self, data_id: &str) -> Option<bool> {
        let data_guard = self.data_items.read().await;
        let data = data_guard.get(data_id).cloned();
        drop(data_guard);
        match data {
            Some(d) => {
                let policies = self.policies.read().await;
                let result = policies.get(&d.policy_id).map(|p| p.is_expired()).unwrap_or(false);
                Some(result)
            }
            None => None,
        }
    }
    pub async fn schedule_deletion(&self, data_id: &str, deleted_by: &str) -> Option<DeletionProof> {
        let data = self.data_items.read().await.get(data_id).cloned();
        data.map(|d| DeletionProof::new(&d.tenant_id, &d.data_id, deleted_by))
    }
    pub async fn list_expired(&self) -> Vec<String> {
        let data = self.data_items.read().await;
        let policies = self.policies.read().await;
        data.values().filter(|d| {
            policies.get(&d.policy_id).map(|p| p.is_expired()).unwrap_or(false)
        }).map(|d| d.data_id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_residency_policy_allowed() {
        let policy = DataResidencyPolicy::new("tenant1", vec![Region::US, Region::EU]);
        assert!(policy.is_allowed_region(Region::US));
        assert!(!policy.is_allowed_region(Region::CN));
    }

    #[test]
    fn test_residency_policy_global() {
        let policy = DataResidencyPolicy::new("tenant1", vec![Region::GLOBAL]);
        assert!(policy.is_allowed_region(Region::US));
        assert!(policy.is_allowed_region(Region::CN));
    }

    #[tokio::test]
    async fn test_residency_checker_compliant() {
        let checker = ResidencyChecker::new();
        checker.register_policy(DataResidencyPolicy::new("tenant1", vec![Region::US])).await;
        checker.set_node_region("node1", Region::US).await;
        let result = checker.check_compliance("tenant1", "node1").await;
        assert!(result.is_compliant());
    }

    #[tokio::test]
    async fn test_residency_checker_violation() {
        let checker = ResidencyChecker::new();
        checker.register_policy(DataResidencyPolicy::new("tenant1", vec![Region::EU])).await;
        checker.set_node_region("node1", Region::US).await;
        let result = checker.check_compliance("tenant1", "node1").await;
        assert!(!result.is_compliant());
    }

    #[test]
    fn test_deletion_proof_creation() {
        let proof = DeletionProof::new("tenant1", "data1", "admin");
        assert!(!proof.proof_id.is_empty());
        assert!(proof.verify());
    }

    #[test]
    fn test_deletion_proof_witness() {
        let mut proof = DeletionProof::new("tenant1", "data1", "admin");
        proof.add_witness("witness1", "sig1");
        assert_eq!(proof.witnesses.len(), 1);
    }

    #[tokio::test]
    async fn test_deletion_proof_store() {
        let store = DeletionProofStore::new();
        let proof = DeletionProof::new("tenant1", "data1", "admin");
        let proof_id = proof.proof_id.clone();
        store.store(proof).await;
        assert!(store.get(&proof_id).await.is_some());
        assert_eq!(store.verify(&proof_id).await, Some(true));
    }

    #[test]
    fn test_retention_policy_expiry() {
        let policy = RetentionPolicy::new("tenant1", "logs", 30);
        assert!(!policy.is_expired());
    }

    #[tokio::test]
    async fn test_retention_manager_list_expired() {
        let manager = RetentionManager::new();
        let policy = RetentionPolicy { created_at: chrono::Utc::now() - chrono::Duration::days(31), ..RetentionPolicy::new("t1", "logs", 30) };
        manager.register_policy(policy.clone()).await;
        let data = RetainedData { data_id: "d1".to_string(), tenant_id: "t1".to_string(), data_type: "logs".to_string(), created_at: chrono::Utc::now() - chrono::Duration::days(31), policy_id: policy.policy_id.clone() };
        manager.register_data(data).await;
        let expired = manager.list_expired().await;
        assert_eq!(expired.len(), 1);
    }
}
