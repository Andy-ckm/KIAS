use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};

use super::SchedulingAlgorithm;

/// Least-Loaded scheduler: picks the node with the lowest load factor.
///
/// Load factor = (total_cpu - available_cpu) / total_cpu.
/// Good for distributing work evenly across heterogeneous nodes.
pub struct LeastLoadedScheduler;

impl LeastLoadedScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LeastLoadedScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchedulingAlgorithm for LeastLoadedScheduler {
    fn name(&self) -> &str {
        "least-loaded"
    }

    async fn schedule(&self, agent: &Agent, nodes: &[Node]) -> Result<ScheduleResult, KiasError> {
        let selected = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Ready)
            .min_by(|a, b| {
                a.load_factor()
                    .partial_cmp(&b.load_factor())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or(KiasError::NoAvailableNodes)?;

        let score = 1.0 - selected.load_factor();

        tracing::info!(
            agent_id = %agent.id,
            node_id = %selected.id,
            load_factor = selected.load_factor(),
            algorithm = "least-loaded",
            "Agent scheduled"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: selected.id.clone(),
            algorithm: "least-loaded".to_string(),
            score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;

    fn make_node(id: &str, cpu_total: f64, cpu_avail: f64) -> Node {
        Node {
            id: id.to_string(),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu: cpu_total,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 0,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: cpu_avail,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 0,
                ..Default::default()
            },
            allocated_agents: vec![],
            labels: Default::default(),
        }
    }

    fn make_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
        }
    }

    #[tokio::test]
    async fn test_picks_least_loaded() {
        let nodes = vec![
            make_node("node-0", 4.0, 1.0), // 75% loaded
            make_node("node-1", 4.0, 3.0), // 25% loaded
            make_node("node-2", 4.0, 2.0), // 50% loaded
        ];
        let scheduler = LeastLoadedScheduler::new();
        let agent = make_agent("a1");
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_skips_not_ready() {
        let mut nodes = vec![
            make_node("node-0", 4.0, 4.0), // idle
            make_node("node-1", 4.0, 4.0), // idle
        ];
        nodes[0].status = NodeStatus::NotReady;
        let scheduler = LeastLoadedScheduler::new();
        let agent = make_agent("a1");
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
    }
}
