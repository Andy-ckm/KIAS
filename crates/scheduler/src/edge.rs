//! # Edge Node Scheduling
//!
//! Supports scheduling agents on edge/fog nodes in addition to cloud nodes.
//! Inspired by YoMo edge computing and K8S node affinity patterns.
//!
//! Key concepts:
//! - Node tiers: Cloud, Edge, Fog, IoT
//! - Latency-aware scheduling: prefer nodes closer to data sources
//! - Bandwidth-aware scheduling: consider network constraints
//! - Location affinity: pin workloads to geographic regions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node tier classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeTier {
    /// IoT device (sensor, actuator)
    IoT,
    /// Fog node (local gateway)
    Fog,
    /// Edge node (regional compute)
    Edge,
    /// Cloud node (centralized compute)
    Cloud,
}

impl NodeTier {
    /// Typical latency range in milliseconds
    pub fn typical_latency_ms(&self) -> (u64, u64) {
        match self {
            NodeTier::IoT => (1, 10),
            NodeTier::Fog => (5, 50),
            NodeTier::Edge => (10, 100),
            NodeTier::Cloud => (50, 500),
        }
    }

    /// Typical bandwidth in Mbps
    pub fn typical_bandwidth_mbps(&self) -> (f64, f64) {
        match self {
            NodeTier::IoT => (0.01, 1.0),
            NodeTier::Fog => (1.0, 100.0),
            NodeTier::Edge => (10.0, 1000.0),
            NodeTier::Cloud => (100.0, 10000.0),
        }
    }
}

/// Location information for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLocation {
    /// Region identifier (e.g., "us-east-1")
    pub region: String,
    /// Zone identifier (e.g., "zone-a")
    pub zone: Option<String>,
    /// Latitude
    pub latitude: Option<f64>,
    /// Longitude
    pub longitude: Option<f64>,
}

/// Edge node descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeNode {
    /// Node identifier
    pub id: String,
    /// Node tier
    pub tier: NodeTier,
    /// Location
    pub location: NodeLocation,
    /// Available CPU cores
    pub cpu_cores: u32,
    /// Available memory in MB
    pub memory_mb: u64,
    /// Current load (0.0 - 1.0)
    pub load: f64,
    /// Network latency to control plane in ms
    pub latency_to_control_plane_ms: u64,
    /// Available bandwidth in Mbps
    pub bandwidth_mbps: f64,
    /// Labels for affinity matching
    pub labels: HashMap<String, String>,
    /// Whether the node is currently reachable
    pub reachable: bool,
}

/// Edge scheduling constraints
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EdgeSchedulingConstraints {
    /// Maximum acceptable latency in ms
    pub max_latency_ms: Option<u64>,
    /// Minimum required bandwidth in Mbps
    pub min_bandwidth_mbps: Option<f64>,
    /// Preferred node tier
    pub preferred_tier: Option<NodeTier>,
    /// Required region
    pub required_region: Option<String>,
    /// Required labels (all must match)
    pub required_labels: HashMap<String, String>,
    /// Data locality: pin to node with data
    pub data_locality_node: Option<String>,
    /// Maximum acceptable load
    pub max_load: Option<f64>,
}

/// Edge scheduler - selects the best edge node for a workload
pub struct EdgeScheduler;

impl EdgeScheduler {
    /// Schedule a workload to the best edge node
    pub fn schedule<'a>(
        nodes: &'a [EdgeNode],
        constraints: &EdgeSchedulingConstraints,
    ) -> Option<&'a EdgeNode> {
        let candidates: Vec<&EdgeNode> = nodes
            .iter()
            .filter(|n| n.reachable)
            .filter(|n| Self::meets_constraints(n, constraints))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Score each candidate
        candidates
            .iter()
            .max_by(|a, b| {
                let score_a = Self::score_node(a, constraints);
                let score_b = Self::score_node(b, constraints);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Check if a node meets all constraints
    fn meets_constraints(node: &EdgeNode, constraints: &EdgeSchedulingConstraints) -> bool {
        if let Some(max_lat) = constraints.max_latency_ms {
            if node.latency_to_control_plane_ms > max_lat {
                return false;
            }
        }

        if let Some(min_bw) = constraints.min_bandwidth_mbps {
            if node.bandwidth_mbps < min_bw {
                return false;
            }
        }

        if let Some(ref region) = constraints.required_region {
            if node.location.region != *region {
                return false;
            }
        }

        for (key, value) in &constraints.required_labels {
            match node.labels.get(key) {
                Some(v) if v == value => {}
                _ => return false,
            }
        }

        if let Some(max_load) = constraints.max_load {
            if node.load > max_load {
                return false;
            }
        }

        true
    }

    /// Score a node (higher is better)
    fn score_node(node: &EdgeNode, constraints: &EdgeSchedulingConstraints) -> f64 {
        let mut score = 0.0;

        // Tier preference bonus
        if let Some(ref preferred) = constraints.preferred_tier {
            if node.tier == *preferred {
                score += 100.0;
            }
        }

        // Lower load is better
        score += (1.0 - node.load) * 50.0;

        // Lower latency is better
        let latency_score = 50.0 - (node.latency_to_control_plane_ms as f64 / 10.0).min(50.0);
        score += latency_score;

        // Higher bandwidth is better
        let bw_score = (node.bandwidth_mbps / 100.0).min(50.0);
        score += bw_score;

        // Data locality bonus
        if let Some(ref locality_node) = constraints.data_locality_node {
            if node.id == *locality_node {
                score += 200.0; // Strong preference for data locality
            }
        }

        // More resources available is better
        score += (node.cpu_cores as f64) * 2.0;
        score += (node.memory_mb as f64 / 1024.0) * 1.0;

        score
    }

    /// Get scheduling statistics for a set of nodes
    pub fn cluster_stats(nodes: &[EdgeNode]) -> EdgeClusterStats {
        let total = nodes.len();
        let reachable = nodes.iter().filter(|n| n.reachable).count();
        let by_tier = |tier: &NodeTier| nodes.iter().filter(|n| n.tier == *tier).count();

        let avg_load = if !nodes.is_empty() {
            nodes.iter().map(|n| n.load).sum::<f64>() / nodes.len() as f64
        } else {
            0.0
        };

        EdgeClusterStats {
            total_nodes: total,
            reachable_nodes: reachable,
            cloud_nodes: by_tier(&NodeTier::Cloud),
            edge_nodes: by_tier(&NodeTier::Edge),
            fog_nodes: by_tier(&NodeTier::Fog),
            iot_nodes: by_tier(&NodeTier::IoT),
            average_load: avg_load,
        }
    }
}

/// Cluster statistics for edge nodes
#[derive(Debug, Clone, Default)]
pub struct EdgeClusterStats {
    pub total_nodes: usize,
    pub reachable_nodes: usize,
    pub cloud_nodes: usize,
    pub edge_nodes: usize,
    pub fog_nodes: usize,
    pub iot_nodes: usize,
    pub average_load: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, tier: NodeTier, region: &str, load: f64) -> EdgeNode {
        EdgeNode {
            id: id.to_string(),
            tier,
            location: NodeLocation {
                region: region.to_string(),
                zone: None,
                latitude: None,
                longitude: None,
            },
            cpu_cores: 4,
            memory_mb: 8192,
            load,
            latency_to_control_plane_ms: 50,
            bandwidth_mbps: 100.0,
            labels: HashMap::new(),
            reachable: true,
        }
    }

    #[test]
    fn test_node_tier_ordering() {
        assert!(NodeTier::IoT < NodeTier::Fog);
        assert!(NodeTier::Fog < NodeTier::Edge);
        assert!(NodeTier::Edge < NodeTier::Cloud);
    }

    #[test]
    fn test_node_tier_latency() {
        let (min, max) = NodeTier::Cloud.typical_latency_ms();
        assert!(min < max);
        assert!(min >= 50);
    }

    #[test]
    fn test_basic_scheduling() {
        let nodes = vec![
            make_node("cloud-1", NodeTier::Cloud, "us-east-1", 0.3),
            make_node("edge-1", NodeTier::Edge, "us-east-1", 0.5),
            make_node("fog-1", NodeTier::Fog, "us-west-2", 0.1),
        ];

        let constraints = EdgeSchedulingConstraints::default();
        let selected = EdgeScheduler::schedule(&nodes, &constraints);
        assert!(selected.is_some());
    }

    #[test]
    fn test_region_constraint() {
        let nodes = vec![
            make_node("cloud-1", NodeTier::Cloud, "us-east-1", 0.3),
            make_node("edge-1", NodeTier::Edge, "eu-west-1", 0.5),
        ];

        let constraints = EdgeSchedulingConstraints {
            required_region: Some("eu-west-1".to_string()),
            ..Default::default()
        };

        let selected = EdgeScheduler::schedule(&nodes, &constraints).unwrap();
        assert_eq!(selected.id, "edge-1");
    }

    #[test]
    fn test_unreachable_node_filtered() {
        let mut node = make_node("edge-1", NodeTier::Edge, "us-east-1", 0.1);
        node.reachable = false;
        let nodes = vec![node];

        let constraints = EdgeSchedulingConstraints::default();
        assert!(EdgeScheduler::schedule(&nodes, &constraints).is_none());
    }

    #[test]
    fn test_label_constraint() {
        let mut node = make_node("gpu-1", NodeTier::Edge, "us-east-1", 0.2);
        node.labels.insert("gpu".to_string(), "nvidia-a100".to_string());
        let nodes = vec![node];

        let constraints = EdgeSchedulingConstraints {
            required_labels: HashMap::from([("gpu".to_string(), "nvidia-a100".to_string())]),
            ..Default::default()
        };

        assert!(EdgeScheduler::schedule(&nodes, &constraints).is_some());

        let bad_constraints = EdgeSchedulingConstraints {
            required_labels: HashMap::from([("gpu".to_string(), "nvidia-h100".to_string())]),
            ..Default::default()
        };

        assert!(EdgeScheduler::schedule(&nodes, &bad_constraints).is_none());
    }

    #[test]
    fn test_data_locality_preference() {
        let nodes = vec![
            make_node("node-a", NodeTier::Cloud, "us-east-1", 0.5),
            make_node("node-b", NodeTier::Edge, "us-east-1", 0.5),
        ];

        let constraints = EdgeSchedulingConstraints {
            data_locality_node: Some("node-b".to_string()),
            ..Default::default()
        };

        let selected = EdgeScheduler::schedule(&nodes, &constraints).unwrap();
        assert_eq!(selected.id, "node-b");
    }

    #[test]
    fn test_cluster_stats() {
        let nodes = vec![
            make_node("c1", NodeTier::Cloud, "us-east-1", 0.3),
            make_node("e1", NodeTier::Edge, "us-east-1", 0.5),
            make_node("f1", NodeTier::Fog, "us-west-2", 0.1),
        ];

        let stats = EdgeScheduler::cluster_stats(&nodes);
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.reachable_nodes, 3);
        assert_eq!(stats.cloud_nodes, 1);
        assert_eq!(stats.edge_nodes, 1);
        assert_eq!(stats.fog_nodes, 1);
        assert_eq!(stats.iot_nodes, 0);
    }

    #[test]
    fn test_max_load_constraint() {
        let nodes = vec![make_node("busy", NodeTier::Cloud, "us-east-1", 0.95)];

        let constraints = EdgeSchedulingConstraints {
            max_load: Some(0.8),
            ..Default::default()
        };

        assert!(EdgeScheduler::schedule(&nodes, &constraints).is_none());
    }

    #[test]
    fn test_empty_cluster_stats() {
        let stats = EdgeScheduler::cluster_stats(&[]);
        assert_eq!(stats.total_nodes, 0);
        assert!((stats.average_load).abs() < f64::EPSILON);
    }
}
