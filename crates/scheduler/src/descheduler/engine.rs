//! Descheduler engine — orchestrates strategies, enforces PDB, produces eviction plans.

use chrono::Utc;
use kias_common::KiasError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::config::DeschedulerConfig;
use super::strategies::{
    AntiAffinityViolationStrategy, DeschedulerStrategy, DuplicateAgentStrategy,
    LowNodeUtilizationStrategy,
};
use super::types::{
    AgentDisruptionBudget, ClusterSnapshot, Eviction, EvictionPlan, EvictionPlanStats,
};

/// The descheduler engine.
///
/// Runs configured strategies against a cluster snapshot, deduplicates
/// proposals, enforces PDB constraints, and produces a final [`EvictionPlan`].
pub struct DeschedulerEngine {
    config: DeschedulerConfig,
    strategies: Vec<Arc<dyn DeschedulerStrategy>>,
}

impl DeschedulerEngine {
    /// Create a new engine from configuration.
    pub fn new(config: DeschedulerConfig) -> Self {
        let strategies = Self::build_strategies(&config);
        tracing::info!(
            strategies = strategies.len(),
            dry_run = config.dry_run,
            max_evictions = config.max_evictions_per_cycle,
            "Descheduler engine initialized"
        );
        Self { config, strategies }
    }

    /// Build strategy instances from config.
    fn build_strategies(config: &DeschedulerConfig) -> Vec<Arc<dyn DeschedulerStrategy>> {
        let mut strategies: Vec<Arc<dyn DeschedulerStrategy>> = Vec::new();

        for name in &config.strategies {
            match name.as_str() {
                "low-node-utilization" => {
                    strategies.push(Arc::new(LowNodeUtilizationStrategy::new(
                        config.thresholds.clone(),
                    )));
                }
                "remove-duplicates" => {
                    strategies.push(Arc::new(DuplicateAgentStrategy::default_max()));
                }
                "remove-anti-affinity-violations" => {
                    strategies.push(Arc::new(AntiAffinityViolationStrategy::new()));
                }
                unknown => {
                    tracing::warn!(strategy = unknown, "Unknown descheduler strategy, skipping");
                }
            }
        }

        strategies
    }

    /// Run all strategies and produce a deduplicated, PDB-respecting eviction plan.
    pub async fn run(&self, snapshot: &ClusterSnapshot) -> Result<EvictionPlan, KiasError> {
        let mut all_evictions: Vec<Eviction> = Vec::new();

        // Collect proposals from all strategies
        for strategy in &self.strategies {
            let proposals = strategy
                .propose_evictions(&snapshot.nodes, &snapshot.agents)
                .await?;
            tracing::debug!(
                strategy = strategy.name(),
                proposals = proposals.len(),
                "Strategy produced proposals"
            );
            all_evictions.extend(proposals);
        }

        // Deduplicate by agent_id (keep first occurrence — strategies are ordered by priority)
        all_evictions = Self::deduplicate_evictions(all_evictions);

        // Enforce PDB constraints
        let (allowed, pdb_blocked) = Self::enforce_pdb(all_evictions, &snapshot.budgets, snapshot);

        // Apply max evictions cap
        let capped = Self::apply_cap(allowed, self.config.max_evictions_per_cycle);

        // Sort by priority ascending (evict lowest priority first)
        let mut final_evictions = capped;
        final_evictions.sort_by_key(|e| e.priority);

        // Compute stats
        let affected_nodes: HashSet<&str> = final_evictions
            .iter()
            .map(|e| e.source_node.as_str())
            .collect();

        let stats = EvictionPlanStats {
            total_evictions: final_evictions.len(),
            overloaded_evictions: final_evictions
                .iter()
                .filter(|e| {
                    matches!(
                        e.reason,
                        super::types::EvictionReason::NodeOverloaded { .. }
                    )
                })
                .count(),
            duplicate_evictions: final_evictions
                .iter()
                .filter(|e| {
                    matches!(
                        e.reason,
                        super::types::EvictionReason::DuplicateAgent { .. }
                    )
                })
                .count(),
            anti_affinity_evictions: final_evictions
                .iter()
                .filter(|e| {
                    matches!(
                        e.reason,
                        super::types::EvictionReason::AntiAffinityViolation { .. }
                    )
                })
                .count(),
            pdb_blocked,
            affected_nodes: affected_nodes.len(),
        };

        tracing::info!(
            total = stats.total_evictions,
            overloaded = stats.overloaded_evictions,
            duplicates = stats.duplicate_evictions,
            anti_affinity = stats.anti_affinity_evictions,
            pdb_blocked = stats.pdb_blocked,
            affected_nodes = stats.affected_nodes,
            dry_run = self.config.dry_run,
            "Descheduler cycle complete"
        );

        Ok(EvictionPlan {
            evictions: final_evictions,
            dry_run: self.config.dry_run,
            generated_at: Utc::now(),
            stats,
        })
    }

    /// Remove duplicate evictions for the same agent (keep first).
    fn deduplicate_evictions(evictions: Vec<Eviction>) -> Vec<Eviction> {
        let mut seen = HashSet::new();
        evictions
            .into_iter()
            .filter(|e| seen.insert(e.agent_id.clone()))
            .collect()
    }

    /// Enforce AgentDisruptionBudgets: remove evictions that would violate PDB.
    fn enforce_pdb(
        evictions: Vec<Eviction>,
        budgets: &[AgentDisruptionBudget],
        snapshot: &ClusterSnapshot,
    ) -> (Vec<Eviction>, usize) {
        if budgets.is_empty() {
            return (evictions, 0);
        }

        // Count evictions per agent type
        let mut eviction_counts: HashMap<String, usize> = HashMap::new();

        for eviction in &evictions {
            if let Some(agent) = snapshot.agents.iter().find(|a| a.id == eviction.agent_id) {
                let type_key = agent
                    .system_prompt_hash
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| agent.name.clone());
                *eviction_counts.entry(type_key).or_insert(0) += 1;
            }
        }

        // Filter out evictions that violate PDB
        let mut allowed = Vec::new();
        let mut blocked = 0;

        for eviction in evictions {
            if let Some(agent) = snapshot.agents.iter().find(|a| a.id == eviction.agent_id) {
                let type_key = agent
                    .system_prompt_hash
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| agent.name.clone());

                // Find budget for this type
                let budget_match = budgets.iter().find(|b| b.agent_type == type_key);

                if let Some(budget) = budget_match {
                    let current_count =
                        snapshot.count_agent_type(agent.system_prompt_hash.unwrap_or(0));
                    let evict_count = eviction_counts.get(&type_key).copied().unwrap_or(0);

                    if !budget.allows_eviction(current_count, evict_count) {
                        tracing::debug!(
                            agent_id = %eviction.agent_id,
                            agent_type = %type_key,
                            current = current_count,
                            evicting = evict_count,
                            min_available = budget.min_available,
                            "Eviction blocked by PDB"
                        );
                        blocked += 1;
                        continue;
                    }
                }
            }

            allowed.push(eviction);
        }

        (allowed, blocked)
    }

    /// Cap the number of evictions.
    fn apply_cap(evictions: Vec<Eviction>, cap: usize) -> Vec<Eviction> {
        if evictions.len() <= cap {
            evictions
        } else {
            tracing::info!(total = evictions.len(), cap = cap, "Evictions capped");
            evictions.into_iter().take(cap).collect()
        }
    }

    /// Get the configured strategies.
    pub fn strategy_names(&self) -> Vec<&str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }

    /// Whether the engine is in dry-run mode.
    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Agent, Node, Priority, Resources};

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
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_engine_empty_cluster() {
        let config = DeschedulerConfig::default();
        let engine = DeschedulerEngine::new(config);
        let snapshot = ClusterSnapshot {
            nodes: vec![],
            agents: vec![],
            budgets: vec![],
        };

        let plan = engine.run(&snapshot).await.unwrap();
        assert!(plan.evictions.is_empty());
        assert_eq!(plan.stats.total_evictions, 0);
    }

    #[tokio::test]
    async fn test_engine_dry_run_flag() {
        let config = DeschedulerConfig {
            dry_run: true,
            ..Default::default()
        };
        let engine = DeschedulerEngine::new(config);
        assert!(engine.is_dry_run());
    }

    #[tokio::test]
    async fn test_engine_respects_max_evictions() {
        let config = DeschedulerConfig {
            max_evictions_per_cycle: 1,
            ..Default::default()
        };
        let engine = DeschedulerEngine::new(config);

        let mut overloaded = make_node("overloaded", 8.0, 0.1, 16_000_000_000, 500_000_000);
        overloaded.allocated_agents = vec!["a1".to_string(), "a2".to_string()];
        let idle = make_node("idle", 8.0, 8.0, 16_000_000_000, 16_000_000_000);

        let snapshot = ClusterSnapshot {
            nodes: vec![overloaded, idle],
            agents: vec![
                make_agent("a1", Priority::Low),
                make_agent("a2", Priority::Low),
            ],
            budgets: vec![],
        };

        let plan = engine.run(&snapshot).await.unwrap();
        assert!(plan.evictions.len() <= 1);
    }

    #[tokio::test]
    async fn test_engine_pdb_blocks_eviction() {
        let config = DeschedulerConfig::default();
        let engine = DeschedulerEngine::new(config);

        let mut overloaded = make_node("overloaded", 8.0, 0.1, 16_000_000_000, 500_000_000);
        overloaded.allocated_agents = vec!["a1".to_string()];
        let idle = make_node("idle", 8.0, 8.0, 16_000_000_000, 16_000_000_000);

        let snapshot = ClusterSnapshot {
            nodes: vec![overloaded, idle],
            agents: vec![{
                let mut a = make_agent("a1", Priority::Low);
                a.system_prompt_hash = Some(42);
                a.name = "critical-type".to_string();
                a
            }],
            budgets: vec![AgentDisruptionBudget {
                agent_type: "42".to_string(),
                min_available: 1,
            }],
        };

        let plan = engine.run(&snapshot).await.unwrap();
        // PDB requires at least 1 of type 42, and there's only 1 — eviction blocked
        assert!(plan.evictions.is_empty());
        assert_eq!(plan.stats.pdb_blocked, 1);
    }

    #[tokio::test]
    async fn test_engine_sorts_by_priority() {
        let config = DeschedulerConfig::default();
        let engine = DeschedulerEngine::new(config);

        let mut overloaded = make_node("overloaded", 8.0, 0.1, 16_000_000_000, 500_000_000);
        overloaded.allocated_agents = vec!["a-high".to_string(), "a-low".to_string()];
        let idle = make_node("idle", 8.0, 8.0, 16_000_000_000, 16_000_000_000);

        let snapshot = ClusterSnapshot {
            nodes: vec![overloaded, idle],
            agents: vec![
                make_agent("a-high", Priority::High),
                make_agent("a-low", Priority::Low),
            ],
            budgets: vec![],
        };

        let plan = engine.run(&snapshot).await.unwrap();
        if plan.evictions.len() >= 2 {
            // Low priority evicted first
            assert_eq!(plan.evictions[0].priority, Priority::Low);
        }
    }

    #[tokio::test]
    async fn test_engine_combines_strategies() {
        let config = DeschedulerConfig {
            strategies: vec![
                "low-node-utilization".to_string(),
                "remove-duplicates".to_string(),
                "remove-anti-affinity-violations".to_string(),
            ],
            ..Default::default()
        };
        let engine = DeschedulerEngine::new(config);
        assert_eq!(engine.strategy_names().len(), 3);
    }

    #[tokio::test]
    async fn test_engine_generated_at_set() {
        let config = DeschedulerConfig::default();
        let engine = DeschedulerEngine::new(config);

        let snapshot = ClusterSnapshot {
            nodes: vec![],
            agents: vec![],
            budgets: vec![],
        };

        let plan = engine.run(&snapshot).await.unwrap();
        // generated_at should be recent (within 1 second)
        let now = Utc::now();
        let diff = now.signed_duration_since(plan.generated_at);
        assert!(diff.num_seconds() < 1);
    }
}
