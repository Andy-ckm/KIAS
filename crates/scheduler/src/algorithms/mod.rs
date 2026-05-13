pub mod affinity;
pub mod cache_aware;
pub mod least_loaded;
pub mod priority_aware;
pub mod resource_aware;
pub mod round_robin;

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, ScheduleResult};

/// Trait that all scheduling algorithms must implement
#[async_trait]
pub trait SchedulingAlgorithm: Send + Sync {
    /// Name of the algorithm
    fn name(&self) -> &str;

    /// Select a node for the given agent from available nodes.
    /// Returns None if no suitable node is found.
    async fn schedule(
        &self,
        agent: &Agent,
        nodes: &[Node],
    ) -> Result<ScheduleResult, KiasError>;
}

pub use affinity::AffinityScheduler;
pub use cache_aware::CacheAwareScheduler;
pub use least_loaded::LeastLoadedScheduler;
pub use priority_aware::PriorityAwareScheduler;
pub use resource_aware::ResourceAwareScheduler;
pub use round_robin::RoundRobinScheduler;
