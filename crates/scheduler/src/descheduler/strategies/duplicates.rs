//! RemoveDuplicates strategy.
//!
//! When multiple agents sharing the same `system_prompt_hash` are co-located
//! on a single node, evict duplicates to spread them across the cluster for
//! better fault tolerance.

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node};
use std::collections::HashMap;

use super::DeschedulerStrategy;
use crate::descheduler::types::{Eviction, EvictionReason};

/// Proposes evictions for duplicate agents co-located on the same node.
pub struct DuplicateAgentStrategy {
    /// Maximum number of same-type agents allowed per node before eviction.
    pub max_per_node: usize,
}

impl DuplicateAgentStrategy {
    pub fn new(max_per_node: usize) -> Self {
        Self { max_per_node }
    }

    pub fn default_max() -> Self {
        Self::new(1)
    }
}

#[async_trait]
impl DeschedulerStrategy for DuplicateAgentStrategy {
    fn name(&self) -> &str {
        "remove-duplicates"
    }

    async fn propose_evictions(
        &self,
        nodes: &[Node],
        agents: &[Agent],
    ) -> Result<Vec<Eviction>, KiasError> {
        let mut evictions = Vec::new();

        // Build agent lookup
        let agent_map: HashMap<&str, &Agent> = agents.iter().map(|a| (a.id.as_str(), a)).collect();

        for node in nodes {
            if node.status != kias_common::NodeStatus::Ready {
                continue;
            }

            // Group agents on this node by system_prompt_hash
            let mut by_hash: HashMap<u64, Vec<&Agent>> = HashMap::new();

            for agent_id in &node.allocated_agents {
                if let Some(agent) = agent_map.get(agent_id.as_str()) {
                    if let Some(hash) = agent.system_prompt_hash {
                        by_hash.entry(hash).or_default().push(agent);
                    }
                }
            }

            // For each group exceeding max_per_node, evict excess (lowest priority first)
            for (hash, group) in &by_hash {
                if group.len() <= self.max_per_node {
                    continue;
                }

                let mut sorted = group.clone();
                sorted.sort_by_key(|a| a.priority);

                let excess = sorted.len() - self.max_per_node;
                for agent in sorted.iter().take(excess) {
                    evictions.push(Eviction {
                        agent_id: agent.id.clone(),
                        source_node: node.id.clone(),
                        reason: EvictionReason::DuplicateAgent {
                            agent_type_hash: *hash,
                            duplicate_count: group.len(),
                        },
                        priority: agent.priority,
                    });
                }
            }
        }

        tracing::info!(
            evictions = evictions.len(),
            "RemoveDuplicates analysis complete"
        );

        Ok(evictions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;

    fn make_node(id: &str, agent_ids: Vec<&str>) -> Node {
        Node {
            id: id.to_string(),
            status: kias_common::NodeStatus::Ready,
            total_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16_000_000_000,
                gpu: 0,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: 4.0,
                memory_bytes: 8_000_000_000,
                gpu: 0,
                ..Default::default()
            },
            allocated_agents: agent_ids.into_iter().map(String::from).collect(),
            labels: Default::default(),
        }
    }

    fn make_agent(id: &str, hash: u64, priority: kias_common::Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: Some(hash),
            affinity: None,
            anti_affinity: None,
        }
    }

    #[tokio::test]
    async fn test_no_evictions_when_no_duplicates() {
        let nodes = vec![make_node("n1", vec!["a1", "a2"])];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Medium),
            make_agent("a2", 200, kias_common::Priority::Medium),
        ];

        let strategy = DuplicateAgentStrategy::default_max();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_evicts_duplicate_on_same_node() {
        let nodes = vec![make_node("n1", vec!["a1", "a2"])];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Low),
            make_agent("a2", 100, kias_common::Priority::High),
        ];

        let strategy = DuplicateAgentStrategy::default_max();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert_eq!(evictions.len(), 1);
        // Low priority evicted first
        assert_eq!(evictions[0].agent_id, "a1");
        assert_eq!(evictions[0].source_node, "n1");
    }

    #[tokio::test]
    async fn test_no_eviction_when_different_nodes() {
        let nodes = vec![make_node("n1", vec!["a1"]), make_node("n2", vec!["a2"])];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Medium),
            make_agent("a2", 100, kias_common::Priority::Medium),
        ];

        let strategy = DuplicateAgentStrategy::default_max();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_max_per_node_greater_than_one() {
        let nodes = vec![make_node("n1", vec!["a1", "a2", "a3"])];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Low),
            make_agent("a2", 100, kias_common::Priority::Medium),
            make_agent("a3", 100, kias_common::Priority::High),
        ];

        let strategy = DuplicateAgentStrategy::new(2);
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert_eq!(evictions.len(), 1);
        assert_eq!(evictions[0].agent_id, "a1"); // lowest priority
    }

    #[tokio::test]
    async fn test_skips_not_ready_nodes() {
        let mut node = make_node("n1", vec!["a1", "a2"]);
        node.status = kias_common::NodeStatus::NotReady;
        let nodes = vec![node];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Low),
            make_agent("a2", 100, kias_common::Priority::High),
        ];

        let strategy = DuplicateAgentStrategy::default_max();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_hash_groups() {
        let nodes = vec![make_node("n1", vec!["a1", "a2", "a3"])];
        let agents = vec![
            make_agent("a1", 100, kias_common::Priority::Low),
            make_agent("a2", 100, kias_common::Priority::High),
            make_agent("a3", 200, kias_common::Priority::Medium),
        ];

        let strategy = DuplicateAgentStrategy::default_max();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        // Only hash=100 has duplicates (a1, a2), a3 is unique hash=200
        assert_eq!(evictions.len(), 1);
        assert_eq!(evictions[0].agent_id, "a1");
    }
}
