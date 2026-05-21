//! Core types for the descheduler.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use kias_common::{Agent, Node, Priority};

// ── Eviction Reason ─────────────────────────────────────────────────

/// Why an agent is being evicted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvictionReason {
    /// The source node is over-utilized.
    NodeOverloaded {
        node_id: String,
        cpu_utilization: f64,
        memory_utilization: f64,
    },
    /// Duplicate agent instances co-located on the same node.
    DuplicateAgent {
        agent_type_hash: u64,
        duplicate_count: usize,
    },
    /// Anti-affinity constraint violated.
    AntiAffinityViolation {
        conflicting_agent_id: String,
        constraint: String,
    },
}

// ── Eviction ────────────────────────────────────────────────────────

/// A single proposed eviction: move `agent_id` off `source_node`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eviction {
    /// Agent to evict.
    pub agent_id: String,
    /// Node the agent is currently running on.
    pub source_node: String,
    /// Why this eviction is proposed.
    pub reason: EvictionReason,
    /// Agent priority (used for ordering: lower priority evicted first).
    pub priority: Priority,
}

// ── Eviction Plan ───────────────────────────────────────────────────

/// A complete eviction plan produced by one descheduler cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionPlan {
    /// Ordered evictions (lowest priority first).
    pub evictions: Vec<Eviction>,
    /// Whether this is a dry-run (no actual evictions).
    pub dry_run: bool,
    /// When the plan was generated.
    pub generated_at: DateTime<Utc>,
    /// Summary statistics.
    pub stats: EvictionPlanStats,
}

/// Summary statistics for an eviction plan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvictionPlanStats {
    /// Total number of proposed evictions.
    pub total_evictions: usize,
    /// Evictions by reason.
    pub overloaded_evictions: usize,
    pub duplicate_evictions: usize,
    pub anti_affinity_evictions: usize,
    /// Number of evictions blocked by PDB.
    pub pdb_blocked: usize,
    /// Number of nodes affected.
    pub affected_nodes: usize,
}

// ── Agent Disruption Budget ─────────────────────────────────────────

/// Limits how many agents of a given type can be disrupted simultaneously.
///
/// Analogous to K8S PodDisruptionBudget. At least `min_available` agents
/// of `agent_type` must remain running after the eviction cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDisruptionBudget {
    /// Agent type identifier (matches `system_prompt_hash` or agent name prefix).
    pub agent_type: String,
    /// Minimum number of agents that must stay available.
    pub min_available: usize,
}

impl AgentDisruptionBudget {
    /// Check whether evicting `eviction_count` agents of this type is allowed,
    /// given that `current_count` are currently running.
    pub fn allows_eviction(&self, current_count: usize, eviction_count: usize) -> bool {
        let remaining = current_count.saturating_sub(eviction_count);
        remaining >= self.min_available
    }
}

// ── Cluster Snapshot ────────────────────────────────────────────────

/// A point-in-time snapshot of the cluster for descheduler analysis.
#[derive(Debug, Clone)]
pub struct ClusterSnapshot {
    pub nodes: Vec<Node>,
    pub agents: Vec<Agent>,
    pub budgets: Vec<AgentDisruptionBudget>,
}

impl ClusterSnapshot {
    /// Agents running on a specific node.
    pub fn agents_on_node(&self, node_id: &str) -> Vec<&Agent> {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|node| {
                self.agents
                    .iter()
                    .filter(|a| node.allocated_agents.contains(&a.id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Count agents of a given type (by system_prompt_hash).
    pub fn count_agent_type(&self, hash: u64) -> usize {
        self.agents
            .iter()
            .filter(|a| a.system_prompt_hash == Some(hash))
            .count()
    }

    /// Find the budget for an agent type, if any.
    pub fn budget_for_type(&self, agent_type: &str) -> Option<&AgentDisruptionBudget> {
        self.budgets.iter().find(|b| b.agent_type == agent_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{NodeStatus, Resources};

    fn make_agent(id: &str, hash: Option<u64>) -> Agent {
        Agent {
            id: id.to_string(),
            name: format!("agent-{id}"),
            resource_request: Resources {
                cpu: 1.0,
                memory_bytes: 512_000_000,
                gpu: 0,
                custom: Default::default(),
            },
            priority: Priority::Medium,
            system_prompt_hash: hash,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    fn make_node(id: &str, agents: Vec<&str>) -> Node {
        Node {
            id: id.to_string(),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16_000_000_000,
                gpu: 0,
                custom: Default::default(),
            },
            available_resources: Resources {
                cpu: 4.0,
                memory_bytes: 8_000_000_000,
                gpu: 0,
                custom: Default::default(),
            },
            allocated_agents: agents.into_iter().map(String::from).collect(),
            labels: Default::default(),
        }
    }

    // ── EvictionReason ──────────────────────────────────────────

    #[test]
    fn test_eviction_reason_equality() {
        let a = EvictionReason::NodeOverloaded {
            node_id: "n1".into(),
            cpu_utilization: 0.9,
            memory_utilization: 0.8,
        };
        let b = EvictionReason::NodeOverloaded {
            node_id: "n1".into(),
            cpu_utilization: 0.9,
            memory_utilization: 0.8,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn test_eviction_reason_inequality() {
        let a = EvictionReason::DuplicateAgent {
            agent_type_hash: 1,
            duplicate_count: 2,
        };
        let b = EvictionReason::AntiAffinityViolation {
            conflicting_agent_id: "x".into(),
            constraint: "c".into(),
        };
        assert_ne!(a, b);
    }

    // ── AgentDisruptionBudget ───────────────────────────────────

    #[test]
    fn test_allows_eviction_basic() {
        let budget = AgentDisruptionBudget {
            agent_type: "web".into(),
            min_available: 2,
        };
        assert!(budget.allows_eviction(5, 2)); // 5-2=3 >= 2
        assert!(!budget.allows_eviction(5, 4)); // 5-4=1 < 2
    }

    #[test]
    fn test_allows_eviction_exact() {
        let budget = AgentDisruptionBudget {
            agent_type: "api".into(),
            min_available: 3,
        };
        assert!(budget.allows_eviction(3, 0)); // 3-0=3 >= 3
        assert!(!budget.allows_eviction(3, 1)); // 3-1=2 < 3
    }

    #[test]
    fn test_allows_eviction_saturating_sub() {
        let budget = AgentDisruptionBudget {
            agent_type: "x".into(),
            min_available: 1,
        };
        // eviction_count > current_count: saturating_sub gives 0
        assert!(!budget.allows_eviction(1, 5));
    }

    // ── ClusterSnapshot ─────────────────────────────────────────

    #[test]
    fn test_agents_on_node() {
        let snapshot = ClusterSnapshot {
            nodes: vec![
                make_node("n1", vec!["a1", "a2"]),
                make_node("n2", vec!["a3"]),
            ],
            agents: vec![
                make_agent("a1", None),
                make_agent("a2", None),
                make_agent("a3", None),
            ],
            budgets: vec![],
        };
        assert_eq!(snapshot.agents_on_node("n1").len(), 2);
        assert_eq!(snapshot.agents_on_node("n2").len(), 1);
        assert_eq!(snapshot.agents_on_node("n3").len(), 0);
    }

    #[test]
    fn test_count_agent_type() {
        let snapshot = ClusterSnapshot {
            nodes: vec![],
            agents: vec![
                make_agent("a1", Some(100)),
                make_agent("a2", Some(100)),
                make_agent("a3", Some(200)),
                make_agent("a4", None),
            ],
            budgets: vec![],
        };
        assert_eq!(snapshot.count_agent_type(100), 2);
        assert_eq!(snapshot.count_agent_type(200), 1);
        assert_eq!(snapshot.count_agent_type(999), 0);
    }

    #[test]
    fn test_budget_for_type() {
        let snapshot = ClusterSnapshot {
            nodes: vec![],
            agents: vec![],
            budgets: vec![
                AgentDisruptionBudget {
                    agent_type: "web".into(),
                    min_available: 2,
                },
                AgentDisruptionBudget {
                    agent_type: "api".into(),
                    min_available: 3,
                },
            ],
        };
        assert!(snapshot.budget_for_type("web").is_some());
        assert_eq!(snapshot.budget_for_type("web").unwrap().min_available, 2);
        assert!(snapshot.budget_for_type("unknown").is_none());
    }

    // ── Serialization ───────────────────────────────────────────

    #[test]
    fn test_eviction_plan_serde() {
        let plan = EvictionPlan {
            evictions: vec![],
            dry_run: true,
            generated_at: Utc::now(),
            stats: EvictionPlanStats::default(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: EvictionPlan = serde_json::from_str(&json).unwrap();
        assert!(back.dry_run);
        assert!(back.evictions.is_empty());
    }

    #[test]
    fn test_eviction_plan_stats_default() {
        let stats = EvictionPlanStats::default();
        assert_eq!(stats.total_evictions, 0);
        assert_eq!(stats.overloaded_evictions, 0);
        assert_eq!(stats.duplicate_evictions, 0);
        assert_eq!(stats.anti_affinity_evictions, 0);
        assert_eq!(stats.pdb_blocked, 0);
        assert_eq!(stats.affected_nodes, 0);
    }

    #[test]
    fn test_agent_disruption_budget_serde() {
        let budget = AgentDisruptionBudget {
            agent_type: "web".into(),
            min_available: 3,
        };
        let json = serde_json::to_string(&budget).unwrap();
        let back: AgentDisruptionBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_type, "web");
        assert_eq!(back.min_available, 3);
    }
}
