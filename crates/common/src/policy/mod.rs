//! Policy-as-Code framework for KIAS
//!
//! Provides a policy engine with in-memory store and immutable audit logging.

pub mod audit;
pub mod condition;
pub mod engine;
pub mod rule;
pub mod store;

pub use audit::PolicyAuditLog;
pub use condition::{Condition, ConditionOperator};
pub use engine::{PolicyEngine, PolicyEvaluationResult};
pub use rule::{Effect, PolicyRule};
pub use store::InMemoryPolicyStore;
