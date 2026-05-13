use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::SchedulingAlgorithm;

/// Round-Robin scheduler: cycles through available nodes in order.
///
/// Simple and fair distribution. Good for homogeneous clusters where
/// all nodes have similar capabilities.
pub struct RoundRobinScheduler {
    index: AtomicUsize,
}

impl RoundRobinScheduler {
    pub fn new() -> Self {
        Self {
            index: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchedulingAlgorithm for RoundRobinScheduler {
    fn name(&self) -> &str {
        "round-robin"
    }

    async fn schedule(
        &self,
        agent: &Agent,
        nodes: &[Node],
    ) -> Result<ScheduleResult, KiasError> {
        let available: Vec<&Node> = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Ready)
            .collect();

        if available.is_empty() {
            return Err(KiasError::NoAvailableNodes);
        }

        let idx = self.index.fetch_add(1, Ordering::Relaxed) % available.len();
        let selected = available[idx];

        tracing::info!(
            agent_id = %agent.id,
            node_id = %selected.id,
            algorithm = "round-robin",
            "Agent scheduled"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: selected.id.clone(),
            algorithm: "round-robin".to_string(),
            score: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Resources, NodeStatus};

    fn make_nodes(n: usize) -> Vec<Node> {
        (0..n)
            .map(|i| Node {
                id: format!("node-{}", i),
                status: NodeStatus::Ready,
                total_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels: Default::default(),
            })
            .collect()
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
    async fn test_round_robin_cycles() {
        let scheduler = RoundRobinScheduler::new();
        let nodes = make_nodes(3);
        let mut results = Vec::new();

        for i in 0..6 {
            let agent = make_agent(&format!("agent-{}", i));
            let r = scheduler.schedule(&agent, &nodes).await.unwrap();
            results.push(r.node_id);
        }

        assert_eq!(results[0], "node-0");
        assert_eq!(results[1], "node-1");
        assert_eq!(results[2], "node-2");
        assert_eq!(results[3], "node-0"); // cycles back
        assert_eq!(results[4], "node-1");
        assert_eq!(results[5], "node-2");
    }

    #[tokio::test]
    async fn test_round_robin_skips_not_ready() {
        let mut nodes = make_nodes(3);
        nodes[1].status = NodeStatus::NotReady;
        let scheduler = RoundRobinScheduler::new();

        let agent = make_agent("a1");
        let r1 = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(r1.node_id, "node-0");

        let agent = make_agent("a2");
        let r2 = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(r2.node_id, "node-2"); // skips node-1
    }

    #[tokio::test]
    async fn test_no_available_nodes() {
        let mut nodes = make_nodes(2);
        nodes[0].status = NodeStatus::NotReady;
        nodes[1].status = NodeStatus::NotReady;
        let scheduler = RoundRobinScheduler::new();

        let agent = make_agent("a1");
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }
}
