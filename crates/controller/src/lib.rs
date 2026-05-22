pub mod autonomy_integration;
pub mod cluster;
pub mod connection_migration;
pub mod controller_loop;
pub mod events;
pub mod handoff;
pub mod health;
pub mod health_model;
pub mod heartbeat;
pub mod lifecycle;
pub mod process_supervisor;
pub mod reconciler;
pub mod recovery;
pub mod runtime_loop;
pub mod state;

pub use autonomy_integration::{ActionApproval, AutonomyGate};
pub use controller_loop::{
    ControllerEventObserver, ControllerLoop, ControllerLoopConfig, ConvergenceEvaluator,
    ReconcileExecutor, RoundActionSummary,
};
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
pub use runtime_loop::{
    NoOpObserver, RuntimeLoop, RuntimeLoopBuilder, RuntimeLoopConfig, RuntimeLoopMetrics,
    RuntimeLoopObserver, RuntimeLoopStatus, TracingObserver,
};
pub use state::{
    ActualState, AgentConfig, AgentInfo, AgentStatus, ControllerState, DesiredState,
    ResourceRequirements,
};

// pub // mod federation; // TODO: fix compilation // TODO: fix compilation
// pub // mod cluster_link; // TODO: fix compilation // TODO: fix compilation
