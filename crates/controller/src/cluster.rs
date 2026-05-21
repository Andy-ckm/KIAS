//! Cluster Interconnect — multi-node cluster federation.
//!
//! Enables AgentGuard instances to form clusters for:
//! - Cross-node agent discovery
//! - Shared state synchronization
//! - Distributed task coordination
//! - Failover and redundancy
//!
//! Inspired by:
//! - EMQX Cluster (Erlang distributed, mnesia)
//! - Redis Cluster (gossip protocol)
//! - Consul/Serf (SWIM protocol for membership)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Node status in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Active,
    Suspect,
    Down,
    Leaving,
}

/// A cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub address: String,
    pub port: u16,
    pub status: NodeStatus,
    /// Millis since last heartbeat.
    pub last_heartbeat_ms: u64,
    /// Node metadata (region, role, capabilities).
    pub metadata: HashMap<String, String>,
    /// Number of agents hosted on this node.
    pub agent_count: u32,
}

/// Cluster event for membership changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterEvent {
    NodeJoined { node_id: String },
    NodeLeft { node_id: String },
    NodeSuspect { node_id: String },
    NodeFailed { node_id: String },
    LeaderElected { node_id: String },
}

/// Cluster configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// This node's ID.
    pub node_id: String,
    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval_ms: u64,
    /// How long before a node is marked suspect (ms).
    pub suspect_timeout_ms: u64,
    /// How long before a suspect node is marked down (ms).
    pub down_timeout_ms: u64,
    /// Gossip fanout (number of nodes to gossip to per round).
    pub gossip_fanout: usize,
    /// Maximum cluster size.
    pub max_nodes: usize,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id: uuid_v4(),
            heartbeat_interval_ms: 5000,
            suspect_timeout_ms: 15000,
            down_timeout_ms: 30000,
            gossip_fanout: 3,
            max_nodes: 100,
        }
    }
}

/// Cluster manager — handles node membership and coordination.
pub struct ClusterManager {
    config: ClusterConfig,
    nodes: HashMap<String, ClusterNode>,
    events: Vec<ClusterEvent>,
    leader_id: Option<String>,
}

impl ClusterManager {
    pub fn new(config: ClusterConfig) -> Self {
        let node_id = config.node_id.clone();
        let mut nodes = HashMap::new();
        nodes.insert(
            node_id.clone(),
            ClusterNode {
                node_id: node_id.clone(),
                address: "127.0.0.1".to_string(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            },
        );

        Self {
            config,
            nodes,
            events: Vec::new(),
            leader_id: Some(node_id),
        }
    }

    /// Join the cluster by connecting to a seed node.
    pub fn join(&mut self, seed_address: &str, seed_port: u16) -> Result<(), String> {
        // In production, this would send a join request via gRPC/HTTP
        // For now, register the seed as a known node
        let seed_id = format!("{}:{}", seed_address, seed_port);
        if !self.nodes.contains_key(&seed_id) {
            self.nodes.insert(
                seed_id.clone(),
                ClusterNode {
                    node_id: seed_id.clone(),
                    address: seed_address.to_string(),
                    port: seed_port,
                    status: NodeStatus::Active,
                    last_heartbeat_ms: now_ms(),
                    metadata: HashMap::new(),
                    agent_count: 0,
                },
            );
            self.events
                .push(ClusterEvent::NodeJoined { node_id: seed_id });
        }
        Ok(())
    }

    /// Add a node to the cluster (called when a node joins).
    pub fn add_node(&mut self, node: ClusterNode) -> Result<(), String> {
        if self.nodes.len() >= self.config.max_nodes {
            return Err("Cluster is at maximum capacity".to_string());
        }

        let node_id = node.node_id.clone();
        self.nodes.insert(node_id.clone(), node);
        self.events.push(ClusterEvent::NodeJoined { node_id });

        // Elect leader if needed (simple: lowest active node_id)
        if self.leader_id.is_none() {
            self.elect_leader();
        }

        Ok(())
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&mut self, node_id: &str) -> Result<(), String> {
        self.nodes
            .remove(node_id)
            .ok_or_else(|| format!("Node '{}' not found", node_id))?;
        self.events.push(ClusterEvent::NodeLeft {
            node_id: node_id.to_string(),
        });

        if self.leader_id.as_deref() == Some(node_id) {
            self.elect_leader();
        }

        Ok(())
    }

    /// Process heartbeats and check for failed nodes.
    pub fn tick(&mut self) -> Vec<ClusterEvent> {
        let now = now_ms();
        let mut new_events = Vec::new();

        let node_ids: Vec<String> = self.nodes.keys().cloned().collect();
        for node_id in node_ids {
            if node_id == self.config.node_id {
                continue; // Skip self
            }

            if let Some(node) = self.nodes.get_mut(&node_id) {
                let elapsed = now.saturating_sub(node.last_heartbeat_ms);

                match node.status {
                    NodeStatus::Active if elapsed > self.config.suspect_timeout_ms => {
                        node.status = NodeStatus::Suspect;
                        new_events.push(ClusterEvent::NodeSuspect {
                            node_id: node_id.clone(),
                        });
                    }
                    NodeStatus::Suspect if elapsed > self.config.down_timeout_ms => {
                        node.status = NodeStatus::Down;
                        new_events.push(ClusterEvent::NodeFailed {
                            node_id: node_id.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        self.events.extend(new_events.clone());
        new_events
    }

    /// Receive a heartbeat from a node.
    pub fn heartbeat(&mut self, node_id: &str) -> Result<(), String> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| format!("Node '{}' not found", node_id))?;
        node.last_heartbeat_ms = now_ms();
        if node.status == NodeStatus::Suspect {
            node.status = NodeStatus::Active;
        }
        Ok(())
    }

    /// Get all active nodes.
    pub fn active_nodes(&self) -> Vec<&ClusterNode> {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .collect()
    }

    /// Get all nodes.
    pub fn all_nodes(&self) -> Vec<&ClusterNode> {
        self.nodes.values().collect()
    }

    /// Get a specific node.
    pub fn get_node(&self, node_id: &str) -> Option<&ClusterNode> {
        self.nodes.get(node_id)
    }

    /// Get the current leader.
    pub fn leader(&self) -> Option<&ClusterNode> {
        self.leader_id.as_ref().and_then(|id| self.nodes.get(id))
    }

    /// Get cluster size.
    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    /// Get recent events.
    pub fn events(&self) -> &[ClusterEvent] {
        &self.events
    }

    /// Get this node's ID.
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// Simple leader election: lowest active node_id.
    fn elect_leader(&mut self) {
        let mut active_ids: Vec<&String> = self
            .nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .map(|n| &n.node_id)
            .collect();
        active_ids.sort();

        let new_leader = active_ids.first().map(|id| id.to_string());
        if new_leader != self.leader_id {
            self.leader_id = new_leader.clone();
            if let Some(leader_id) = new_leader {
                self.events
                    .push(ClusterEvent::LeaderElected { node_id: leader_id });
            }
        }
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        d.as_secs() as u32,
        d.subsec_millis() as u16,
        (d.subsec_micros() % 10000) as u16,
        (d.subsec_nanos() % 10000) as u16,
        d.subsec_nanos() as u64 % 0xFFFFFFFFFFFF,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ClusterConfig {
        ClusterConfig {
            node_id: "node-1".to_string(),
            heartbeat_interval_ms: 1000,
            suspect_timeout_ms: 3000,
            down_timeout_ms: 5000,
            gossip_fanout: 2,
            max_nodes: 10,
        }
    }

    #[test]
    fn test_single_node_cluster() {
        let cluster = ClusterManager::new(make_config());
        assert_eq!(cluster.size(), 1);
        assert_eq!(cluster.node_id(), "node-1");
    }

    #[test]
    fn test_add_node() {
        let mut cluster = ClusterManager::new(make_config());
        cluster
            .add_node(ClusterNode {
                node_id: "node-2".to_string(),
                address: "10.0.0.2".to_string(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();
        assert_eq!(cluster.size(), 2);
    }

    #[test]
    fn test_remove_node() {
        let mut cluster = ClusterManager::new(make_config());
        cluster
            .add_node(ClusterNode {
                node_id: "node-2".to_string(),
                address: "10.0.0.2".to_string(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();
        cluster.remove_node("node-2").unwrap();
        assert_eq!(cluster.size(), 1);
    }

    #[test]
    fn test_heartbeat() {
        let mut cluster = ClusterManager::new(make_config());
        cluster
            .add_node(ClusterNode {
                node_id: "node-2".to_string(),
                address: "10.0.0.2".to_string(),
                port: 7100,
                status: NodeStatus::Suspect,
                last_heartbeat_ms: now_ms() - 10000,
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();

        cluster.heartbeat("node-2").unwrap();
        let node = cluster.get_node("node-2").unwrap();
        assert_eq!(node.status, NodeStatus::Active);
    }

    #[test]
    fn test_leader_election() {
        let mut cluster = ClusterManager::new(make_config());
        // Remove self to force leader change
        cluster.nodes.remove("node-1");
        cluster.leader_id = None;

        cluster
            .add_node(ClusterNode {
                node_id: "node-3".to_string(),
                address: "10.0.0.3".to_string(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();

        assert!(cluster.leader().is_some());
    }

    #[test]
    fn test_max_nodes() {
        let config = ClusterConfig {
            max_nodes: 2,
            ..make_config()
        };
        let mut cluster = ClusterManager::new(config);

        cluster
            .add_node(ClusterNode {
                node_id: "n2".into(),
                address: "10.0.0.2".into(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();

        let result = cluster.add_node(ClusterNode {
            node_id: "n3".into(),
            address: "10.0.0.3".into(),
            port: 7100,
            status: NodeStatus::Active,
            last_heartbeat_ms: now_ms(),
            metadata: HashMap::new(),
            agent_count: 0,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_active_nodes() {
        let mut cluster = ClusterManager::new(make_config());
        cluster
            .add_node(ClusterNode {
                node_id: "n2".into(),
                address: "10.0.0.2".into(),
                port: 7100,
                status: NodeStatus::Down,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();
        cluster
            .add_node(ClusterNode {
                node_id: "n3".into(),
                address: "10.0.0.3".into(),
                port: 7100,
                status: NodeStatus::Active,
                last_heartbeat_ms: now_ms(),
                metadata: HashMap::new(),
                agent_count: 0,
            })
            .unwrap();

        let active = cluster.active_nodes();
        assert_eq!(active.len(), 2); // self + n3
    }
}
