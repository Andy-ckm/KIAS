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
