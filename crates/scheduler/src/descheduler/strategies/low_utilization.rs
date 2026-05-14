//! LowNodeUtilization strategy.
//!
//! Detects overloaded nodes and proposes evictions to redistribute agents
//! to underutilized nodes. Inspired by K8S descheduler `LowNodeUtilization`.

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node};

use super::DeschedulerStrategy;
use crate::descheduler::config::UtilizationThresholds;
use crate::descheduler::types::{Eviction, EvictionReason};

/// Proposes evictions from nodes whose utilization exceeds the high threshold,
/// provided there are underutilized nodes available to absorb the load.
pub struct LowNodeUtilizationStrategy {
    thresholds: UtilizationThresholds,
}

impl LowNodeUtilizationStrategy {
    pub fn new(thresholds: UtilizationThresholds) -> Self {
        Self { thresholds }
    }

    /// Classify nodes as overloaded, underutilized, or appropriately loaded.
    fn classify_nodes<'a>(&self, nodes: &'a [Node]) -> (Vec<&'a Node>, Vec<&'a Node>) {
        let mut overloaded = Vec::new();
        let mut underutilized = Vec::new();

        for node in nodes {
            if node.status != kias_common::NodeStatus::Ready {
                continue;
            }

            let cpu_util = node.load_factor();
            let mem_util = if node.total_resources.memory_bytes > 0 {
                1.0 - (node.available_resources.memory_bytes as f64
                    / node.total_resources.memory_bytes as f64)
            } else {
                1.0
            };

            if cpu_util > self.thresholds.high_cpu || mem_util > self.thresholds.high_memory {
                overloaded.push(node);
            } else if cpu_util < self.thresholds.low_cpu && mem_util < self.thresholds.low_memory {
                underutilized.push(node);
            }
        }

        (overloaded, underutilized)
    }

    /// Select agents to evict from an overloaded node (lowest priority first).
    fn agents_to_evict<'a>(
        &self,
        node: &Node,
        agents: &'a [Agent],
        max_evictions: usize,
    ) -> Vec<&'a Agent> {
        let mut node_agents: Vec<&Agent> = agents
            .iter()
            .filter(|a| node.allocated_agents.contains(&a.id))
            .collect();

        // Sort by priority ascending (evict lowest priority first)
        node_agents.sort_by_key(|a| a.priority);

        node_agents.into_iter().take(max_evictions).collect()
    }
}

#[async_trait]
impl DeschedulerStrategy for LowNodeUtilizationStrategy {
    fn name(&self) -> &str {
        "low-node-utilization"
    }

    async fn propose_evictions(
        &self,
        nodes: &[Node],
        agents: &[Agent],
    ) -> Result<Vec<Eviction>, KiasError> {
        let (overloaded, underutilized) = self.classify_nodes(nodes);

        // Only evict if there are underutilized nodes to absorb the load
        if underutilized.is_empty() {
            tracing::debug!("No underutilized nodes; skipping low-utilization strategy");
            return Ok(Vec::new());
        }

        let mut evictions = Vec::new();

        for node in &overloaded {
            let cpu_util = node.load_factor();
            let mem_util = if node.total_resources.memory_bytes > 0 {
                1.0 - (node.available_resources.memory_bytes as f64
                    / node.total_resources.memory_bytes as f64)
            } else {
                1.0
            };

            // Evict up to 2 agents per overloaded node per cycle
            let candidates = self.agents_to_evict(node, agents, 2);

            for agent in candidates {
                evictions.push(Eviction {
                    agent_id: agent.id.clone(),
                    source_node: node.id.clone(),
                    reason: EvictionReason::NodeOverloaded {
                        node_id: node.id.clone(),
                        cpu_utilization: cpu_util,
                        memory_utilization: mem_util,
                    },
                    priority: agent.priority,
                });
            }
        }

        tracing::info!(
            overloaded = overloaded.len(),
            underutilized = underutilized.len(),
            evictions = evictions.len(),
            "LowNodeUtilization analysis complete"
        );

        Ok(evictions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Priority, Resources};

    fn make_node(id: &str, cpu_total: f64, cpu_avail: f64, mem_total: u64, mem_avail: u64) -> Node {
        Node {
            id: id.to_string(),
            status: kias_common::NodeStatus::Ready,
            total_resources: Resources {
                cpu: cpu_total,
                memory_bytes: mem_total,
                gpu: 0,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: cpu_avail,
                memory_bytes: mem_avail,
                gpu: 0,
                ..Default::default()
            },
            allocated_agents: vec![],
            labels: Default::default(),
        }
    }

    fn make_agent(id: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
        }
    }

    #[tokio::test]
    async fn test_no_evictions_when_balanced() {
        let nodes = vec![
            make_node("n1", 8.0, 6.0, 16_000_000_000, 12_000_000_000),
            make_node("n2", 8.0, 6.0, 16_000_000_000, 12_000_000_000),
        ];
        let agents = vec![make_agent("a1", Priority::Medium)];
        let strategy = LowNodeUtilizationStrategy::new(UtilizationThresholds::default());

        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_evicts_from_overloaded_node() {
        let mut overloaded = make_node("overloaded", 8.0, 0.5, 16_000_000_000, 1_000_000_000);
        overloaded.allocated_agents = vec!["a1".to_string(), "a2".to_string()];
        let underutilized = make_node("underutilized", 8.0, 7.5, 16_000_000_000, 15_000_000_000);

        let nodes = vec![overloaded, underutilized];
        let agents = vec![
            make_agent("a1", Priority::Low),
            make_agent("a2", Priority::High),
        ];

        let strategy = LowNodeUtilizationStrategy::new(UtilizationThresholds::default());
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert!(!evictions.is_empty());
        // Low priority should be evicted first
        assert_eq!(evictions[0].agent_id, "a1");
        assert_eq!(evictions[0].source_node, "overloaded");
    }

    #[tokio::test]
    async fn test_no_evictions_without_underutilized_nodes() {
        let mut overloaded = make_node("overloaded", 8.0, 0.5, 16_000_000_000, 1_000_000_000);
        overloaded.allocated_agents = vec!["a1".to_string()];
        // Also overloaded — no underutilized node to absorb
        let also_busy = make_node("busy", 8.0, 1.0, 16_000_000_000, 2_000_000_000);

        let nodes = vec![overloaded, also_busy];
        let agents = vec![make_agent("a1", Priority::Low)];

        let strategy = LowNodeUtilizationStrategy::new(UtilizationThresholds::default());
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_skips_not_ready_nodes() {
        let mut not_ready = make_node("not-ready", 8.0, 0.5, 16_000_000_000, 1_000_000_000);
        not_ready.status = kias_common::NodeStatus::NotReady;
        not_ready.allocated_agents = vec!["a1".to_string()];
        let underutilized = make_node("underutilized", 8.0, 7.5, 16_000_000_000, 15_000_000_000);

        let nodes = vec![not_ready, underutilized];
        let agents = vec![make_agent("a1", Priority::Low)];

        let strategy = LowNodeUtilizationStrategy::new(UtilizationThresholds::default());
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_custom_thresholds() {
        // With very low thresholds, even moderate load triggers eviction
        let thresholds = UtilizationThresholds {
            high_cpu: 0.30,
            high_memory: 0.30,
            low_cpu: 0.10,
            low_memory: 0.10,
        };

        let mut node = make_node("n1", 8.0, 5.0, 16_000_000_000, 10_000_000_000);
        node.allocated_agents = vec!["a1".to_string()];
        let idle = make_node("n2", 8.0, 8.0, 16_000_000_000, 16_000_000_000);

        let nodes = vec![node, idle];
        let agents = vec![make_agent("a1", Priority::Medium)];

        let strategy = LowNodeUtilizationStrategy::new(thresholds);
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert!(!evictions.is_empty());
    }

    #[tokio::test]
    async fn test_max_two_evictions_per_node() {
        let mut node = make_node("overloaded", 8.0, 0.1, 16_000_000_000, 500_000_000);
        node.allocated_agents = vec![
            "a1".to_string(),
            "a2".to_string(),
            "a3".to_string(),
            "a4".to_string(),
        ];
        let idle = make_node("idle", 8.0, 8.0, 16_000_000_000, 16_000_000_000);

        let nodes = vec![node, idle];
        let agents = vec![
            make_agent("a1", Priority::Low),
            make_agent("a2", Priority::Low),
            make_agent("a3", Priority::Medium),
            make_agent("a4", Priority::High),
        ];

        let strategy = LowNodeUtilizationStrategy::new(UtilizationThresholds::default());
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        // At most 2 per node
        assert!(evictions.len() <= 2);
    }
}
