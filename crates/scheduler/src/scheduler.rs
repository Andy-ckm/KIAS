use std::sync::Arc;

use kias_common::{Agent, KiasError, Node, ScheduleResult};
use tracing;

use crate::algorithms::{
    CacheAwareScheduler, LeastLoadedScheduler, ResourceAwareScheduler, RoundRobinScheduler,
    SchedulingAlgorithm,
};
use crate::config::SchedulerConfig;
use crate::optimizer::CacheOptimizer;
use crate::policies::{AffinityFilter, PrioritySorter};

/// The main scheduler entry point.
///
/// Orchestrates the full scheduling pipeline:
/// 1. Sort agents by priority
/// 2. Filter nodes by affinity / anti-affinity
/// 3. Apply the configured scheduling algorithm
/// 4. Track cache state via the optimizer
pub struct Scheduler {
    config: SchedulerConfig,
    algorithm: Box<dyn SchedulingAlgorithm>,
    cache_optimizer: Arc<CacheOptimizer>,
}

impl Scheduler {
    /// Create a new scheduler from configuration.
    pub fn new(config: SchedulerConfig) -> Self {
        let algorithm = Self::build_algorithm(&config);
        let cache_optimizer = Arc::new(CacheOptimizer::new());

        tracing::info!(
            algorithm = %config.algorithm,
            cache_weight = config.cache_weight,
            preemption = config.preemption_enabled,
            "Scheduler initialized"
        );

        Self {
            config,
            algorithm,
            cache_optimizer,
        }
    }

    /// Create a scheduler with a shared cache optimizer (for use by other components).
    pub fn with_cache_optimizer(
        config: SchedulerConfig,
        cache_optimizer: Arc<CacheOptimizer>,
    ) -> Self {
        let algorithm = Self::build_algorithm(&config);

        tracing::info!(
            algorithm = %config.algorithm,
            "Scheduler initialized with shared cache optimizer"
        );

        Self {
            config,
            algorithm,
            cache_optimizer,
        }
    }

    /// Build the scheduling algorithm from config.
    fn build_algorithm(config: &SchedulerConfig) -> Box<dyn SchedulingAlgorithm> {
        match config.algorithm.as_str() {
            "round-robin" => Box::new(RoundRobinScheduler::new()),
            "least-loaded" => Box::new(LeastLoadedScheduler::new()),
            "resource-aware" => Box::new(ResourceAwareScheduler::new()),
            "cache-aware" => Box::new(CacheAwareScheduler::new(config.cache_weight)),
            other => {
                tracing::warn!(
                    requested = other,
                    "Unknown algorithm, falling back to round-robin"
                );
                Box::new(RoundRobinScheduler::new())
            }
        }
    }

    /// Schedule a single agent onto a node.
    ///
    /// Applies affinity filtering, then delegates to the configured algorithm.
    pub async fn schedule_agent(
        &self,
        agent: &Agent,
        nodes: &[Node],
    ) -> Result<ScheduleResult, KiasError> {
        // Step 1: Filter by affinity / anti-affinity
        let candidates = AffinityFilter::apply(agent, nodes);
        let candidate_nodes: Vec<Node> = candidates.iter().map(|(n, _)| (*n).clone()).collect();

        if candidate_nodes.is_empty() {
            tracing::warn!(
                agent_id = %agent.id,
                "No nodes satisfy affinity constraints"
            );
            return Err(KiasError::NoAvailableNodes);
        }

        // Step 2: Run scheduling algorithm
        let mut result = self.algorithm.schedule(agent, &candidate_nodes).await?;

        // Step 3: Blend affinity score into the result
        if let Some((_, affinity_score)) = candidates.iter().find(|(n, _)| n.id == result.node_id) {
            // Combine: 70% algorithm score + 30% affinity score
            result.score = 0.7 * result.score + 0.3 * affinity_score;
        }

        tracing::debug!(
            agent_id = %agent.id,
            node_id = %result.node_id,
            algorithm = %result.algorithm,
            score = result.score,
            "Scheduling complete"
        );

        Ok(result)
    }

    /// Schedule a batch of agents in priority order.
    ///
    /// Higher-priority agents are scheduled first and get first pick of nodes.
    pub async fn schedule_batch(
        &self,
        agents: &mut [Agent],
        nodes: &[Node],
    ) -> Vec<Result<ScheduleResult, KiasError>> {
        // Sort by priority (high first)
        PrioritySorter::sort_agents(agents);

        let mut results = Vec::with_capacity(agents.len());

        for agent in agents.iter() {
            let result = self.schedule_agent(agent, nodes).await;
            results.push(result);
        }

        let successes = results.iter().filter(|r| r.is_ok()).count();
        tracing::info!(
            total = agents.len(),
            scheduled = successes,
            failed = agents.len() - successes,
            "Batch scheduling complete"
        );

        results
    }

    /// Get a reference to the cache optimizer.
    pub fn cache_optimizer(&self) -> &CacheOptimizer {
        &self.cache_optimizer
    }

    /// Get the current algorithm name.
    pub fn algorithm_name(&self) -> &str {
        self.algorithm.name()
    }

    /// Get the scheduler configuration.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Affinity, AntiAffinity, Priority, Resources};
    use std::collections::HashMap;

    fn make_nodes(n: usize) -> Vec<Node> {
        (0..n)
            .map(|i| Node {
                id: format!("node-{}", i),
                status: kias_common::NodeStatus::Ready,
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
                labels: HashMap::new(),
            })
            .collect()
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
    async fn test_schedule_single_agent() {
        let config = SchedulerConfig {
            algorithm: "round-robin".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(3);
        let agent = make_agent("a1", Priority::Medium);

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert!(!result.node_id.is_empty());
        assert_eq!(result.algorithm, "round-robin");
    }

    #[tokio::test]
    async fn test_schedule_batch_priority_order() {
        let config = SchedulerConfig {
            algorithm: "least-loaded".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(2);

        let mut agents = vec![
            make_agent("low", Priority::Low),
            make_agent("high", Priority::High),
            make_agent("med", Priority::Medium),
        ];

        let results = scheduler.schedule_batch(&mut agents, &nodes).await;
        assert_eq!(results.len(), 3);
        // All should succeed since we have enough nodes
        assert!(results.iter().all(|r| r.is_ok()));
        // After sorting, high priority agent should be first
        assert_eq!(agents[0].id, "high");
    }

    #[tokio::test]
    async fn test_schedule_with_affinity() {
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);

        let mut labels = HashMap::new();
        labels.insert("zone".to_string(), "us-east".to_string());

        let mut nodes = make_nodes(2);
        nodes[0].labels = labels.clone();
        nodes[1].labels = HashMap::new();

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: labels,
                preferred: vec![],
            }),
            anti_affinity: None,
        };

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-0");
    }

    #[tokio::test]
    async fn test_schedule_with_anti_affinity() {
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);

        let mut avoid_labels = HashMap::new();
        avoid_labels.insert("zone".to_string(), "eu-west".to_string());

        let mut labels1 = HashMap::new();
        labels1.insert("zone".to_string(), "eu-west".to_string());
        let mut labels2 = HashMap::new();
        labels2.insert("zone".to_string(), "us-east".to_string());

        let mut nodes = make_nodes(2);
        nodes[0].labels = labels1;
        nodes[1].labels = labels2;

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: Some(AntiAffinity {
                avoid_labels,
                avoid_agent_types: vec![],
            }),
        };

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_fallback_algorithm() {
        let config = SchedulerConfig {
            algorithm: "unknown-algo".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        assert_eq!(scheduler.algorithm_name(), "round-robin");
    }
}
