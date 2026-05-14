//! Descheduler strategies for cluster rebalancing.

mod anti_affinity;
mod duplicates;
mod low_utilization;

pub use anti_affinity::AntiAffinityViolationStrategy;
pub use duplicates::DuplicateAgentStrategy;
pub use low_utilization::LowNodeUtilizationStrategy;

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node};

use super::types::Eviction;

/// Trait that all descheduler strategies must implement.
///
/// Each strategy analyzes a cluster snapshot and returns a list of proposed
/// evictions. The engine applies PDB guards and dry-run filtering on top.
#[async_trait]
pub trait DeschedulerStrategy: Send + Sync {
    /// Human-readable name of this strategy.
    fn name(&self) -> &str;

    /// Analyze the cluster state and propose evictions.
    ///
    /// Returns an *unordered* list of agents that should be evicted.
    /// The engine is responsible for deduplication, PDB enforcement,
    /// and ordering by priority.
    async fn propose_evictions(
        &self,
        nodes: &[Node],
        agents: &[Agent],
    ) -> Result<Vec<Eviction>, KiasError>;
}
