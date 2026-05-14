//! KIAS Scheduler — resource-aware agent scheduling.

pub mod algorithms;
pub mod config;
pub mod descheduler;
pub mod edge;
pub mod engine;
pub mod optimizer;
pub mod policies;
pub mod scheduler;
pub mod strategy;

// Legacy API (trait-based)
pub use engine::SchedulerEngine;
pub use strategy::{LeastLoaded, RoundRobin, ScheduleStrategy};

// New API (rich algorithm + policy pipeline)
pub use algorithms::{AffinityScheduler, CacheAwareScheduler, LeastLoadedScheduler, PriorityAwareScheduler, ResourceAwareScheduler, RoundRobinScheduler, SchedulingAlgorithm};
pub use config::SchedulerConfig;
pub use scheduler::Scheduler;

// Edge scheduling
pub use edge::{EdgeNode, EdgeScheduler, EdgeSchedulingConstraints, EdgeClusterStats, NodeTier, NodeLocation};
