pub mod events;
pub mod handoff;
pub mod health;
pub mod heartbeat;
pub mod lifecycle;
pub mod reconciler;
pub mod recovery;
pub mod state;

pub use events::{
    AgentEvent, AgentEventEnvelope, AlertProcessor, EventBus, EventProcessor, EventType,
    LoggingProcessor, MetricsProcessor,
};
pub use handoff::{
    HandoffCandidate, HandoffManager, HandoffPolicy, HandoffRecord, HandoffStats, HandoffStatus,
};
pub use health::{HealthCheckConfig, HealthCheckSummary, HealthChecker};
pub use heartbeat::{HeartbeatAction, HeartbeatConfig, HeartbeatMonitor};
pub use lifecycle::{AgentLifecycleManager, LifecycleHooks, LifecycleState};
pub use reconciler::{AgentSpawner, DefaultReconciler, NoOpSpawner, Reconciler};
pub use recovery::{RecoveryAction, RecoveryConfig, RecoveryManager};
pub use state::{
    ActualState, AgentConfig, AgentInfo, AgentStatus, ControllerState, DesiredState,
    ResourceRequirements,
};
