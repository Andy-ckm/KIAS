pub mod health;
pub mod heartbeat;
pub mod handoff;
pub mod events;
pub mod lifecycle;
pub mod reconciler;
pub mod recovery;
pub mod state;

pub use health::{HealthCheckConfig, HealthCheckSummary, HealthChecker};
pub use heartbeat::{HeartbeatAction, HeartbeatConfig, HeartbeatMonitor};
pub use handoff::{HandoffManager, HandoffPolicy, HandoffRecord, HandoffStatus, HandoffStats, HandoffCandidate};
pub use events::{
    AgentEvent, AgentEventEnvelope, AlertProcessor, EventBus, EventProcessor, EventType,
    LoggingProcessor, MetricsProcessor,
};
pub use lifecycle::{AgentLifecycleManager, LifecycleHooks, LifecycleState};
pub use reconciler::{DefaultReconciler, Reconciler};
pub use recovery::{RecoveryAction, RecoveryConfig, RecoveryManager};
pub use state::{
    ActualState, AgentConfig, AgentInfo, AgentStatus, ControllerState, DesiredState,
    ResourceRequirements,
};
