//! Scheduling strategies for the KIAS scheduler (legacy trait-based API).
//!
//! Each strategy implements [`ScheduleStrategy`] and provides a different
//! algorithm for selecting the best node for a given task.
//!
//! For the newer, richer algorithm system, see [`super::algorithms`].

pub mod least_loaded;
pub mod round_robin;

use async_trait::async_trait;
use kias_common::KiasResult;

pub use least_loaded::LeastLoaded;
pub use round_robin::RoundRobin;

/// Trait that all scheduling strategies must implement.
#[async_trait]
pub trait ScheduleStrategy: Send + Sync {
    /// Select the best node from the list of available nodes.
    async fn select_node(&self, nodes: &[String]) -> KiasResult<String>;
}
