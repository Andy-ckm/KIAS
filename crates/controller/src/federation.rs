//! # Cross-Region Federation Management
//!
//! Provides FederationCluster, ControlPlane unified view, and data-plane locality scheduling.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Region & Cluster ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region { USEast, USWest, EUWest, EUCentral, APAC, CN }

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::USEast => write!(f, "us-east"),
            Region::USWest => write!(f, "us-west"),
            Region::EUWest => write!(f, "eu-west"),
            Region::EUCentral => write!(f, "eu-central"),
            Region::APAC => write!(f, "apac"),
            Region::CN => write!(f, "cn"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterTier { Primary, Secondary, Warm }

#[derive(Debug, Clone)]
pub struct ClusterNode {
    pub node_id: String,
    pub region: Region,
    pub tier: ClusterTier,
    pub capacity: u64,
    pub current_load: u64,
    pub latency_to_primary_ms: u32,
}

impl ClusterNode {
    pub fn available_capacity(&self) -> u64 { self.capacity.saturating_sub(self.current_load) }
    pub fn load_factor(&self) -> f64 { if self.capacity == 0 { 0.0 } else { self.current_load as f64 / self.capacity as f64 } }
}

#[derive(Debug, Clone)]
pub struct FederationCluster {
    pub cluster_id: String,
    pub cluster_name: String,
    pub region: Region,
    pub nodes: Vec<ClusterNode>,
    pub is_primary: bool,
    pub sync_state: SyncState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncState { Synced, Syncing, Isolated, Unknown }

impl FederationCluster {
    pub fn new(cluster_id: &str, name: &str, region: Region, is_primary: bool) -> Self {
        Self { cluster_id: cluster_id.to_string(), cluster_name: name.to_string(), region, nodes: Vec::new(), is_primary, sync_state: SyncState::Unknown }
    }
    pub fn add_node(&mut self, node: ClusterNode) { self.nodes.push(node); }
    pub fn total_capacity(&self) -> u64 { self.nodes.iter().map(|n| n.capacity).sum() }
    pub fn total_available(&self) -> u64 { self.nodes.iter().map(|n| n.available_capacity()).sum() }
}

// ── Control Plane ────────────────────────────────────────────────────────────

pub struct ControlPlane {
    clusters: Arc<RwLock<BTreeMap<String, FederationCluster>>>,
    region_latencies: Arc<RwLock<HashMap<(Region, Region), u32>>>,
}

impl Default for ControlPlane { fn default() -> Self { Self::new() } }

impl ControlPlane {
    pub fn new() -> Self {
        Self { clusters: Arc::new(RwLock::new(BTreeMap::new())), region_latencies: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn register_cluster(&self, cluster: FederationCluster) {
        self.clusters.write().await.insert(cluster.cluster_id.clone(), cluster);
    }

    pub async fn unregister_cluster(&self, cluster_id: &str) { self.clusters.write().await.remove(cluster_id); }

    pub async fn get_cluster(&self, cluster_id: &str) -> Option<FederationCluster> {
        self.clusters.read().await.get(cluster_id).cloned()
    }

    pub async fn list_clusters(&self) -> Vec<FederationCluster> {
        self.clusters.read().await.values().cloned().collect()
    }

    pub async fn list_clusters_by_region(&self, region: Region) -> Vec<FederationCluster> {
        self.clusters.read().await.values().filter(|c| c.region == region).cloned().collect()
    }

    pub async fn set_latency(&self, from: Region, to: Region, latency_ms: u32) {
        self.region_latencies.write().await.insert((from, to), latency_ms);
    }

    pub async fn get_latency(&self, from: Region, to: Region) -> u32 {
        self.region_latencies.read().await.get(&(from, to)).copied().unwrap_or(u32::MAX)
    }

    pub async fn find_nearest_cluster(&self, from_region: Region, min_capacity: u64) -> Option<String> {
        let clusters = self.clusters.read().await;
        let mut candidates: Vec<_> = clusters.values().filter(|c| c.total_available() >= min_capacity).collect();
        candidates.sort_by_key(|c| self.region_latencies.read().await.get(&(from_region, c.region)).copied().unwrap_or(u32::MAX));
        candidates.first().map(|c| c.cluster_id.clone())
    }

    pub async fn update_sync_state(&self, cluster_id: &str, state: SyncState) {
        if let Some(cluster) = self.clusters.write().await.get_mut(cluster_id) {
            cluster.sync_state = state;
        }
    }
}

// ── Data Plane Routing ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub target_cluster_id: String,
    pub target_region: Region,
    pub reason: String,
    pub latency_estimate_ms: u32,
}

pub struct DataPlaneRouter {
    control_plane: Arc<ControlPlane>,
}

impl Default for DataPlaneRouter { fn default() -> Self { Self::new() } }

impl DataPlaneRouter {
    pub fn new() -> Self { Self { control_plane: Arc::new(ControlPlane::new()) } }
    pub fn with_control_plane(cp: Arc<ControlPlane>) -> Self { Self { control_plane: cp } }

    pub async fn route_request(&self, tenant_id: &str, required_region: Option<Region>, min_capacity: u64) -> Option<RoutingDecision> {
        let from_region = Region::USEast;
        let target_region = required_region.unwrap_or_else(|| self.find_best_region(from_region, min_capacity).await);
        let cluster_id = self.control_plane.find_nearest_cluster(from_region, min_capacity).await?;
        let latency = self.control_plane.get_latency(from_region, target_region).await;
        Some(RoutingDecision { target_cluster_id: cluster_id, target_region, reason: "best_available".to_string(), latency_estimate_ms: latency })
    }

    async fn find_best_region(&self, from: Region, min_capacity: u64) -> Region {
        let clusters = self.control_plane.list_clusters().await;
        let mut by_region: BTreeMap<Region, u64> = BTreeMap::new();
        for c in clusters { *by_region.entry(c.region).or_insert(0) += c.total_available(); }
        by_region.into_iter().find(|(_, cap)| *cap >= min_capacity).map(|(r, _)| r).unwrap_or(Region::USEast)
    }

    pub async fn get_optimal_cluster_for_workload(&self, tenant_id: &str, workload_type: &str) -> Option<String> {
        match workload_type {
            "low_latency" => self.control_plane.find_nearest_cluster(Region::USEast, 10).await,
            "high_throughput" => self.control_plane.list_clusters().await.into_iter().max_by_key(|c| c.total_capacity()).map(|c| c.cluster_id),
            _ => self.control_plane.list_clusters().await.into_iter().find(|c| c.is_primary).map(|c| c.cluster_id),
        }
    }
}

// ── Federation Manager ──────────────────────────────────────────────────────

pub struct FederationManager {
    control_plane: Arc<ControlPlane>,
    router: DataPlaneRouter,
}

impl Default for FederationManager { fn default() -> Self { Self::new() } }

impl FederationManager {
    pub fn new() -> Self {
        Self { control_plane: Arc::new(ControlPlane::new()), router: DataPlaneRouter::with_control_plane(Arc::clone(&ControlPlane::new())) }
    }

    pub fn control_plane(&self) -> &Arc<ControlPlane> { &self.control_plane }
    pub fn router(&self) -> &DataPlaneRouter { &self.router }

    pub async fn register_region(&self, cluster: FederationCluster) { self.control_plane.register_cluster(cluster).await; }

    pub async fn sync_state(&self, cluster_id: &str) -> Option<SyncState> {
        self.control_plane.get_cluster(cluster_id).await.map(|c| c.sync_state)
    }

    pub async fn cross_region_route(&self, tenant_id: &str, required_region: Option<Region>, min_capacity: u64) -> Option<RoutingDecision> {
        self.router.route_request(tenant_id, required_region, min_capacity).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_node_available_capacity() {
        let node = ClusterNode { node_id: "n1".to_string(), region: Region::USEast, tier: ClusterTier::Primary, capacity: 100, current_load: 30, latency_to_primary_ms: 0 };
        assert_eq!(node.available_capacity(), 70);
        assert!((node.load_factor() - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_federation_cluster_total() {
        let mut cluster = FederationCluster::new("c1", "us-east-1", Region::USEast, true);
        cluster.add_node(ClusterNode { node_id: "n1".to_string(), region: Region::USEast, tier: ClusterTier::Primary, capacity: 100, current_load: 20, latency_to_primary_ms: 0 });
        cluster.add_node(ClusterNode { node_id: "n2".to_string(), region: Region::USEast, tier: ClusterTier::Secondary, capacity: 50, current_load: 10, latency_to_primary_ms: 5 });
        assert_eq!(cluster.total_capacity(), 150);
        assert_eq!(cluster.total_available(), 120);
    }

    #[tokio::test]
    async fn test_control_plane_register() {
        let cp = ControlPlane::new();
        let mut cluster = FederationCluster::new("c1", "us-east-1", Region::USEast, true);
        cluster.add_node(ClusterNode { node_id: "n1".to_string(), region: Region::USEast, tier: ClusterTier::Primary, capacity: 100, current_load: 0, latency_to_primary_ms: 0 });
        cp.register_cluster(cluster).await;
        let retrieved = cp.get_cluster("c1").await.unwrap();
        assert_eq!(retrieved.cluster_name, "us-east-1");
    }

    #[tokio::test]
    async fn test_control_plane_nearest_cluster() {
        let cp = ControlPlane::new();
        let mut c1 = FederationCluster::new("c1", "us-east", Region::USEast, true);
        c1.add_node(ClusterNode { node_id: "n1".to_string(), region: Region::USEast, tier: ClusterTier::Primary, capacity: 100, current_load: 0, latency_to_primary_ms: 0 });
        cp.register_cluster(c1).await;
        let mut c2 = FederationCluster::new("c2", "eu-west", Region::EUWest, false);
        c2.add_node(ClusterNode { node_id: "n2".to_string(), region: Region::EUWest, tier: ClusterTier::Primary, capacity: 100, current_load: 0, latency_to_primary_ms: 0 });
        cp.register_cluster(c2).await;
        cp.set_latency(Region::USEast, Region::USEast, 10).await;
        cp.set_latency(Region::USEast, Region::EUWest, 100).await;
        let nearest = cp.find_nearest_cluster(Region::USEast, 10).await;
        assert_eq!(nearest, Some("c1".to_string()));
    }

    #[tokio::test]
    async fn test_data_plane_router() {
        let cp = ControlPlane::new();
        let router = DataPlaneRouter::with_control_plane(Arc::new(cp));
        let decision = router.route_request("tenant1", Some(Region::USEast), 10).await;
        assert!(decision.is_none());
    }

    #[tokio::test]
    async fn test_federation_manager() {
        let manager = FederationManager::new();
        let mut cluster = FederationCluster::new("c1", "us-east-1", Region::USEast, true);
        cluster.add_node(ClusterNode { node_id: "n1".to_string(), region: Region::USEast, tier: ClusterTier::Primary, capacity: 100, current_load: 0, latency_to_primary_ms: 0 });
        manager.register_region(cluster).await;
        let clusters = manager.control_plane().list_clusters().await;
        assert_eq!(clusters.len(), 1);
    }
}
