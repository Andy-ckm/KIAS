//! # KIAS Descheduler
//!
//! Cluster rebalancing system inspired by the K8S descheduler.
//!
//! The descheduler periodically analyzes the cluster state and proposes
//! agent evictions to correct imbalance, then the scheduler reschedules
//! the evicted agents to better-suited nodes.
//!
//! ## Strategies
//!
//! - **LowNodeUtilization**: Evicts agents from overloaded nodes when
//!   underutilized nodes exist to absorb them.
//! - **RemoveDuplicates**: Spreads same-type agents across nodes by
//!   evicting co-located duplicates.
//! - **RemoveAgentsViolatingAntiAffinity**: Evicts agents that violate
//!   their anti-affinity constraints.
//!
//! ## Safety
//!
//! - **PDB (AgentDisruptionBudget)**: Guarantees minimum availability
//!   per agent type — evictions that would violate PDB are blocked.
//! - **Dry-run mode**: Produces eviction plans without executing them.
//! - **Max evictions cap**: Limits evictions per cycle to prevent churn.

pub mod config;
pub mod engine;
pub mod strategies;
pub mod types;

pub use config::{DeschedulerConfig, UtilizationThresholds};
pub use engine::DeschedulerEngine;
pub use strategies::{
    AntiAffinityViolationStrategy, DeschedulerStrategy, DuplicateAgentStrategy,
    LowNodeUtilizationStrategy,
};
pub use types::{
    AgentDisruptionBudget, ClusterSnapshot, Eviction, EvictionPlan, EvictionPlanStats,
    EvictionReason,
};
