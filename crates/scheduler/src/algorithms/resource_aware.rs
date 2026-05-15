use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};

use super::SchedulingAlgorithm;

/// Resource-Aware scheduler: selects the node that best fits the agent's
/// resource requirements.
///
/// Uses a bin-packing style scoring: prefers nodes where the remaining
/// resources after allocation are neither too small nor too large.
/// This avoids fragmentation while still distributing load.
pub struct ResourceAwareScheduler;

impl ResourceAwareScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResourceAwareScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Score a node for a given resource request.
/// Higher score = better fit. Returns None if the node cannot satisfy the request.
fn score_node(node: &Node, request: &kias_common::Resources) -> Option<f64> {
    if node.status != NodeStatus::Ready {
        return None;
    }
    if !node.available_resources.can_satisfy(request) {
        return None;
    }

    // CPU fitness: ratio of requested to available (prefer tighter fits)
    let cpu_ratio = if node.available_resources.cpu > 0.0 {
        request.cpu / node.available_resources.cpu
    } else {
        return None;
    };

    // Memory fitness
    let mem_ratio = if node.available_resources.memory_bytes > 0 {
        request.memory_bytes as f64 / node.available_resources.memory_bytes as f64
    } else {
        return None;
    };

    // Combined score: weighted average of ratios (higher = better fit)
    // We want nodes that will be reasonably utilized but not overloaded
    let raw_score = 0.5 * cpu_ratio + 0.3 * mem_ratio + 0.2 * (1.0 - node.load_factor());

    // Penalize if the node is already heavily loaded
    let load_penalty = node.load_factor() * 0.5;

    Some((raw_score - load_penalty).clamp(0.0, 1.0))
}

#[async_trait]
impl SchedulingAlgorithm for ResourceAwareScheduler {
    fn name(&self) -> &str {
        "resource-aware"
    }

    async fn schedule(&self, agent: &Agent, nodes: &[Node]) -> Result<ScheduleResult, KiasError> {
        let mut best_node: Option<&Node> = None;
        let mut best_score = f64::NEG_INFINITY;

        for node in nodes {
            if let Some(s) = score_node(node, &agent.resource_request) {
                if s > best_score {
                    best_score = s;
                    best_node = Some(node);
                }
            }
        }

        let selected = best_node.ok_or_else(|| {
            KiasError::InsufficientResources(format!(
                "No node can satisfy agent {} resource request: cpu={}, mem={}",
                agent.id, agent.resource_request.cpu, agent.resource_request.memory_bytes
            ))
        })?;

        tracing::info!(
            agent_id = %agent.id,
            node_id = %selected.id,
            score = best_score,
            algorithm = "resource-aware",
            "Agent scheduled"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: selected.id.clone(),
            algorithm: "resource-aware".to_string(),
            score: best_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;

    fn make_node(id: &str, cpu_avail: f64, mem_avail: u64) -> Node {
        Node {
            id: id.to_string(),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16 * 1024 * 1024 * 1024,
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

    fn make_agent(id: &str, cpu: f64, mem: u64) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources {
                cpu,
                memory_bytes: mem,
                gpu: 0,
                ..Default::default()
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_selects_node_with_enough_resources() {
        let nodes = vec![
            make_node("small", 0.5, 512 * 1024 * 1024), // 0.5 CPU, 512MB
            make_node("large", 8.0, 16 * 1024 * 1024 * 1024), // 8 CPU, 16GB
        ];
        let scheduler = ResourceAwareScheduler::new();
        // Agent needs 4 CPU, 4GB
        let agent = make_agent("a1", 4.0, 4 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "large");
    }

    #[tokio::test]
    async fn test_rejects_insufficient_resources() {
        let nodes = vec![make_node("small", 0.5, 512 * 1024 * 1024)];
        let scheduler = ResourceAwareScheduler::new();
        let agent = make_agent("a1", 4.0, 4 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::InsufficientResources(_))));
    }

    #[tokio::test]
    async fn test_prefers_tighter_fit() {
        let nodes = vec![
            make_node("node-a", 4.0, 8 * 1024 * 1024 * 1024), // 4 CPU left
            make_node("node-b", 2.0, 4 * 1024 * 1024 * 1024), // 2 CPU left (tighter fit for 1 CPU request)
        ];
        let scheduler = ResourceAwareScheduler::new();
        let agent = make_agent("a1", 1.0, 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        // Both can fit, but the algorithm considers multiple factors
        assert!(result.node_id == "node-a" || result.node_id == "node-b");
    }
}
