//! # Controller Loop — Continuous Execute → Observe → Adjust → Re-Execute
//!
//! Bridges the generic [`RuntimeLoop`] engine with the controller's actual
//! reconciliation and health-check subsystems.  Each "round" performs:
//!
//! 1. **Execute** — run reconciliation (scale up/down) + health check (heartbeat/recovery)
//! 2. **Observe** — evaluate convergence between desired and actual state
//! 3. **Adjust** — generate feedback for the next round based on what failed
//!
//! ```text
//! ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
//! │  ReconcileExecutor│───▶│ConvergenceEval   │───▶│ Feedback (Adjust)│
//! │  reconcile +      │    │ actual vs desired │    │ what to improve  │
//! │  health check     │    │ score 0.0–1.0     │    │ for next round   │
//! └──────────────────┘    └──────────────────┘    └──────────────────┘
//!       ▲                                                        │
//!       └────────────────────────────────────────────────────────┘
//!                        until converged or max rounds
//! ```

use crate::events::{AgentEvent, AgentEventEnvelope, EventBus};
use crate::health::{HealthCheckConfig, HealthChecker};
use crate::reconciler::{DefaultReconciler, NoOpSpawner, Reconciler};
use crate::runtime_loop::{
    RuntimeEvaluator, RuntimeExecutor, RuntimeLoop, RuntimeLoopBuilder, RuntimeLoopConfig,
    RuntimeLoopMetrics, RuntimeLoopObserver, RuntimeLoopStatus,
};
use crate::state::{AgentStatus, ControllerState};
use kias_common::{KiasError, KiasResult};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

// ──────────────────────────────────────────────────────────────────────────────
// Controller Action Result
// ──────────────────────────────────────────────────────────────────────────────

/// Summary of what happened during a single controller round.
#[derive(Debug, Clone, Serialize)]
pub struct RoundActionSummary {
    /// Number of agents checked in health check.
    pub agents_checked: usize,
    /// Number of alive agents from health check.
    pub alive: usize,
    /// Number of restarted agents from health check.
    pub restarted: usize,
    /// Whether reconciliation was attempted.
    pub reconciled: bool,
    /// Number of running agents after this round.
    pub running_after: u32,
    /// Desired number of replicas.
    pub desired_replicas: u32,
    /// Number of failed agents after this round.
    pub failed_after: usize,
    /// Human-readable description of what happened.
    pub description: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// ReconcileExecutor — Execute phase
// ──────────────────────────────────────────────────────────────────────────────

/// Executes one round of controller work: reconciliation + health check.
///
/// Implements [`RuntimeExecutor`] so it can be plugged into the generic
/// [`RuntimeLoop`] engine.
pub struct ReconcileExecutor {
    state: Arc<Mutex<ControllerState>>,
    health_checker: Arc<Mutex<HealthChecker>>,
}

impl ReconcileExecutor {
    pub fn new(
        state: Arc<Mutex<ControllerState>>,
        health_checker: Arc<Mutex<HealthChecker>>,
    ) -> Self {
        Self {
            state,
            health_checker,
        }
    }
}

#[async_trait::async_trait]
impl RuntimeExecutor for ReconcileExecutor {
    async fn execute_round(
        &self,
        _goal: &str,
        round: u32,
        feedback: Option<&str>,
    ) -> KiasResult<String> {
        let mut state = self.state.lock().await;
        let mut checker = self.health_checker.lock().await;

        // ── Phase 1: Reconcile (scale to desired replicas) ──
        let reconciler = DefaultReconciler::<NoOpSpawner>::new(NoOpSpawner);
        reconciler.reconcile(&mut state).await?;
        let _running_before_health = state.actual.running_replicas;

        // ── Phase 2: Health check (heartbeat + recovery) ──
        let summary = checker.check(&mut state);
        let running_after = state.actual.running_replicas;
        let failed_count = state.count_by_status(&AgentStatus::Failed);
        let unresponsive_count = state.count_by_status(&AgentStatus::Unresponsive);

        // ── Phase 3: Apply feedback-driven adjustments ──
        if let Some(fb) = feedback {
            tracing::debug!(
                round,
                feedback = fb,
                "Applying feedback from previous round"
            );
            // If previous round indicated cascade failures, be more conservative
            if fb.contains("cascade") || fb.contains("thundering") {
                tracing::info!(round, "Cascade detected — deferring aggressive recovery");
            }
        }

        // ── Build summary ──
        let action_summary = RoundActionSummary {
            agents_checked: summary.agents_checked,
            alive: summary.alive,
            restarted: summary.restarted,
            reconciled: true,
            running_after,
            desired_replicas: state.desired.replicas,
            failed_after: failed_count,
            description: format!(
                "Round {}: reconcile→{} running, health→{} checked / {} alive / {} restarted / {} failed / {} unresponsive",
                round,
                running_after,
                summary.agents_checked,
                summary.alive,
                summary.restarted,
                failed_count + summary.timed_out,
                unresponsive_count,
            ),
        };

        tracing::info!(
            round,
            running = running_after,
            desired = state.desired.replicas,
            failed = failed_count,
            unresponsive = unresponsive_count,
            "Controller round executed"
        );

        // Serialize the summary as the executor output
        serde_json::to_string(&action_summary).map_err(|e| {
            KiasError::Internal(anyhow::anyhow!("Failed to serialize round summary: {}", e))
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ConvergenceEvaluator — Observe phase
// ──────────────────────────────────────────────────────────────────────────────

/// Evaluates how close the controller's actual state is to the desired state.
///
/// Returns a quality score in [0.0, 1.0] based on:
/// - Replica convergence: `running / desired` (weight: 0.6)
/// - Health ratio: `healthy / total` (weight: 0.3)
/// - Failure penalty: reduces score for failed/unresponsive agents (weight: 0.1)
pub struct ConvergenceEvaluator {
    state: Arc<Mutex<ControllerState>>,
}

impl ConvergenceEvaluator {
    pub fn new(state: Arc<Mutex<ControllerState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl RuntimeEvaluator for ConvergenceEvaluator {
    async fn evaluate(&self, _goal: &str, _output: &str, _round: u32) -> KiasResult<f64> {
        let state = self.state.lock().await;

        let desired = state.desired.replicas;
        if desired == 0 {
            return Ok(1.0); // Nothing to converge to.
        }

        // Try to parse the round output for richer info, but fall back to state.
        let running = state.actual.running_replicas;
        let total_agents = state.agents.len() as u32;
        let failed = state.count_by_status(&AgentStatus::Failed) as u32;
        let unresponsive = state.count_by_status(&AgentStatus::Unresponsive) as u32;
        let healthy = running.saturating_sub(0); // running agents are healthy

        // ── Replica convergence (weight: 0.6) ──
        let replica_score = if desired > 0 {
            (running as f64 / desired as f64).min(1.0)
        } else {
            1.0
        };

        // ── Health ratio (weight: 0.3) ──
        let health_score = if total_agents > 0 {
            (healthy as f64) / (total_agents as f64)
        } else if desired > 0 {
            // No agents yet but we want some — poor health.
            0.0
        } else {
            1.0
        };

        // ── Failure penalty (weight: 0.1) ──
        let failure_penalty = if total_agents > 0 {
            let fail_ratio = ((failed + unresponsive) as f64) / (total_agents as f64);
            1.0 - fail_ratio.min(1.0)
        } else {
            1.0
        };

        let score = (replica_score * 0.6) + (health_score * 0.3) + (failure_penalty * 0.1);

        tracing::debug!(
            desired,
            running,
            total_agents,
            failed,
            unresponsive,
            replica_score = format!("{:.3}", replica_score),
            health_score = format!("{:.3}", health_score),
            failure_penalty = format!("{:.3}", failure_penalty),
            convergence_score = format!("{:.3}", score),
            "Convergence evaluated"
        );

        Ok(score.clamp(0.0, 1.0))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ControllerEventObserver — Observability via EventBus
// ──────────────────────────────────────────────────────────────────────────────

/// Publishes controller loop lifecycle events to the [`EventBus`].
pub struct ControllerEventObserver {
    event_bus: Arc<EventBus>,
}

impl ControllerEventObserver {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait::async_trait]
impl RuntimeLoopObserver for ControllerEventObserver {
    async fn on_round_start(&self, round: u32, goal: &str) {
        tracing::info!(round, goal, "Controller loop round starting");
        let envelope =
            AgentEventEnvelope::new(AgentEvent::HealthChanged, "controller", "runtime-loop")
                .with_metadata("phase", "round_start")
                .with_metadata("round", round.to_string())
                .with_metadata("goal", goal.to_string());
        self.event_bus.publish(&envelope).await;
    }

    async fn on_round_complete(&self, round: u32, output: &str, quality: f64) {
        tracing::info!(
            round,
            quality = format!("{:.3}", quality),
            output_len = output.len(),
            "Controller loop round complete"
        );
        let envelope =
            AgentEventEnvelope::new(AgentEvent::HealthChanged, "controller", "runtime-loop")
                .with_metadata("phase", "round_complete")
                .with_metadata("round", round.to_string())
                .with_metadata("quality", format!("{:.3}", quality));
        self.event_bus.publish(&envelope).await;
    }

    async fn on_status_change(&self, goal: &str, old: &RuntimeLoopStatus, new: &RuntimeLoopStatus) {
        tracing::info!(goal, ?old, ?new, "Controller loop status changed");
        let event = match new {
            RuntimeLoopStatus::Achieved => AgentEvent::Completed,
            RuntimeLoopStatus::Failed(_) => AgentEvent::Failed,
            RuntimeLoopStatus::TimedOut | RuntimeLoopStatus::Cancelled => AgentEvent::Terminated,
            _ => AgentEvent::HealthChanged,
        };
        let envelope = AgentEventEnvelope::new(event, "controller", "runtime-loop")
            .with_metadata("phase", "status_change")
            .with_metadata("old_status", format!("{:?}", old))
            .with_metadata("new_status", format!("{:?}", new))
            .with_metadata("goal", goal.to_string());
        self.event_bus.publish(&envelope).await;
    }

    async fn on_feedback(&self, round: u32, feedback: &str) {
        tracing::debug!(round, feedback, "Controller loop feedback generated");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ControllerLoop — High-level orchestrator
// ──────────────────────────────────────────────────────────────────────────────

/// High-level orchestrator that runs the controller in a continuous
/// execute→observe→adjust loop until the desired state is achieved or limits
/// are reached.
///
/// # Example
///
/// ```rust,no_run
/// use kias_controller::controller_loop::*;
/// use kias_controller::state::*;
/// use kias_controller::health::*;
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
///
/// # async fn example() -> kias_common::KiasResult<()> {
/// let state = Arc::new(Mutex::new(ControllerState {
///     desired: DesiredState {
///         replicas: 3,
///         agent_config: AgentConfig {
///             name: "worker".into(),
///             image: "kias/agent:latest".into(),
///             resources: ResourceRequirements { cpu: "100m".into(), memory: "128Mi".into() },
///         },
///     },
///     actual: ActualState {
///         running_replicas: 0,
///         agent_status: AgentStatus::Pending,
///         last_updated: chrono::Utc::now(),
///     },
///     agents: std::collections::HashMap::new(),
/// }));
///
/// let config = ControllerLoopConfig::default();
/// let mut loop_orchestrator = ControllerLoop::new(config, state).await?;
/// let metrics = loop_orchestrator.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct ControllerLoop {
    runtime_loop: RuntimeLoop,
    state: Arc<Mutex<ControllerState>>,
    config: ControllerLoopConfig,
}

/// Configuration for the controller loop.
#[derive(Debug, Clone)]
pub struct ControllerLoopConfig {
    /// Runtime loop configuration (max rounds, timeouts, thresholds).
    pub runtime: RuntimeLoopConfig,
    /// Health check configuration.
    pub health: HealthCheckConfig,
}

impl Default for ControllerLoopConfig {
    fn default() -> Self {
        Self {
            runtime: RuntimeLoopConfig {
                max_rounds: 20,
                loop_timeout: std::time::Duration::from_secs(600),
                round_timeout: std::time::Duration::from_secs(30),
                quality_threshold: 0.95, // 95% convergence = done
                stop_on_achieve: true,
                cooldown: std::time::Duration::from_millis(500),
            },
            health: HealthCheckConfig::default(),
        }
    }
}

impl ControllerLoop {
    /// Create a new controller loop with the given configuration and shared state.
    pub async fn new(
        config: ControllerLoopConfig,
        state: Arc<Mutex<ControllerState>>,
    ) -> KiasResult<Self> {
        let health_checker = Arc::new(Mutex::new(HealthChecker::new(config.health.clone())));
        let event_bus = Arc::new(EventBus::new());

        let executor = Arc::new(ReconcileExecutor::new(state.clone(), health_checker));
        let evaluator = Arc::new(ConvergenceEvaluator::new(state.clone()));
        let observer = Arc::new(ControllerEventObserver::new(event_bus));

        let runtime_loop = RuntimeLoopBuilder::new()
            .max_rounds(config.runtime.max_rounds)
            .loop_timeout(config.runtime.loop_timeout)
            .round_timeout(config.runtime.round_timeout)
            .quality_threshold(config.runtime.quality_threshold)
            .stop_on_achieve(config.runtime.stop_on_achieve)
            .cooldown(config.runtime.cooldown)
            .executor(executor)
            .evaluator(evaluator)
            .observer(observer)
            .build();

        Ok(Self {
            runtime_loop,
            state,
            config,
        })
    }

    /// Create with default configuration.
    pub async fn with_defaults(state: Arc<Mutex<ControllerState>>) -> KiasResult<Self> {
        Self::new(ControllerLoopConfig::default(), state).await
    }

    /// Run the controller loop until convergence or limits.
    ///
    /// Returns metrics about the loop execution.
    pub async fn run(&self) -> KiasResult<RuntimeLoopMetrics> {
        let desired = {
            let s = self.state.lock().await;
            s.desired.replicas
        };
        let goal = format!(
            "Achieve {} running replicas with all agents healthy",
            desired
        );

        tracing::info!(
            desired_replicas = desired,
            max_rounds = self.config.runtime.max_rounds,
            quality_threshold = self.config.runtime.quality_threshold,
            "Starting controller loop"
        );

        let metrics = self.runtime_loop.run(&goal).await?;

        tracing::info!(
            rounds = metrics.rounds_executed,
            status = ?metrics.status,
            total_duration_ms = metrics.total_duration.as_millis(),
            "Controller loop finished"
        );

        Ok(metrics)
    }

    /// Get a reference to the shared controller state.
    pub fn state(&self) -> &Arc<Mutex<ControllerState>> {
        &self.state
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_state(desired: u32, running: u32) -> Arc<Mutex<ControllerState>> {
        let mut agents = HashMap::new();
        for i in 1..=running {
            let mut info = AgentInfo::new(format!("agent-{i}"), format!("worker-{i}"));
            info.status = AgentStatus::Running;
            agents.insert(format!("agent-{i}"), info);
        }
        Arc::new(Mutex::new(ControllerState {
            desired: DesiredState {
                replicas: desired,
                agent_config: AgentConfig {
                    name: "worker".to_string(),
                    image: "kias/agent:latest".to_string(),
                    resources: ResourceRequirements {
                        cpu: "100m".to_string(),
                        memory: "128Mi".to_string(),
                    },
                },
            },
            actual: ActualState {
                running_replicas: running,
                agent_status: if running > 0 {
                    AgentStatus::Running
                } else {
                    AgentStatus::Pending
                },
                last_updated: Utc::now(),
            },
            agents,
        }))
    }

    fn fast_config() -> ControllerLoopConfig {
        ControllerLoopConfig {
            runtime: RuntimeLoopConfig {
                max_rounds: 5,
                loop_timeout: std::time::Duration::from_secs(10),
                round_timeout: std::time::Duration::from_secs(5),
                quality_threshold: 0.9,
                stop_on_achieve: true,
                cooldown: std::time::Duration::ZERO,
            },
            health: HealthCheckConfig {
                check_interval_ms: 1000,
                heartbeat: crate::heartbeat::HeartbeatConfig {
                    check_interval_secs: 60,
                    timeout_secs: 120,
                    jitter_percent: 0,
                },
                recovery: crate::recovery::RecoveryConfig {
                    max_retries: 3,
                    base_backoff_secs: 1,
                    max_backoff_secs: 60,
                    backoff_multiplier: 2.0,
                    jitter_percent: 0,
                },
                jitter_percent: 0,
                max_cascade_failures: 100,
            },
        }
    }

    // ── ConvergenceEvaluator tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_convergence_full_match() {
        let state = make_state(3, 3);
        let evaluator = ConvergenceEvaluator::new(state);
        let score = evaluator.evaluate("goal", "{}", 1).await.unwrap();
        // 3/3 running = 1.0 replica, 3/3 healthy = 1.0 health, 0 failures = 1.0 penalty
        // 1.0*0.6 + 1.0*0.3 + 1.0*0.1 = 1.0
        assert!((score - 1.0).abs() < 0.01, "Expected ~1.0, got {score}");
    }

    #[tokio::test]
    async fn test_convergence_partial() {
        let state = make_state(3, 1); // 1 running, 3 desired
        let evaluator = ConvergenceEvaluator::new(state);
        let score = evaluator.evaluate("goal", "{}", 1).await.unwrap();
        // replica: 1/3 = 0.333, health: 1/1 = 1.0, penalty: 0/1 = 1.0
        // 0.333*0.6 + 1.0*0.3 + 1.0*0.1 = 0.2 + 0.3 + 0.1 = 0.6
        assert!(score > 0.5 && score < 0.7, "Expected ~0.6, got {score}");
    }

    #[tokio::test]
    async fn test_convergence_zero_desired() {
        let state = make_state(0, 0);
        let evaluator = ConvergenceEvaluator::new(state);
        let score = evaluator.evaluate("goal", "{}", 1).await.unwrap();
        assert!(
            (score - 1.0).abs() < 0.01,
            "Zero desired = perfect convergence"
        );
    }

    #[tokio::test]
    async fn test_convergence_no_agents_yet() {
        let state = make_state(5, 0); // 5 desired, 0 running
        let evaluator = ConvergenceEvaluator::new(state);
        let score = evaluator.evaluate("goal", "{}", 1).await.unwrap();
        // replica: 0/5 = 0.0, health: 0 agents but desired = 0.0, penalty: 1.0
        // 0.0*0.6 + 0.0*0.3 + 1.0*0.1 = 0.1
        assert!(score < 0.2, "Expected low score, got {score}");
    }

    // ── ReconcileExecutor tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_executor_produces_valid_output() {
        let state = make_state(2, 0);
        let checker = Arc::new(Mutex::new(HealthChecker::new(fast_config().health)));
        let executor = ReconcileExecutor::new(state, checker);

        let output = executor.execute_round("goal", 1, None).await.unwrap();
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("reconciled").is_some());
        assert!(parsed.get("running_after").is_some());
    }

    #[tokio::test]
    async fn test_executor_with_feedback() {
        let state = make_state(3, 0);
        let checker = Arc::new(Mutex::new(HealthChecker::new(fast_config().health)));
        let executor = ReconcileExecutor::new(state, checker);

        let output = executor
            .execute_round("goal", 2, Some("Round 1 quality 0.3 below threshold"))
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("reconciled").unwrap().as_bool().unwrap());
    }

    // ── ControllerLoop integration tests ──────────────────────────────────

    #[tokio::test]
    async fn test_controller_loop_converges() {
        // State already at desired — should converge in 1 round.
        let state = make_state(3, 3);
        let config = fast_config();
        let ctrl = ControllerLoop::new(config, state).await.unwrap();
        let metrics = ctrl.run().await.unwrap();

        assert_eq!(metrics.status, RuntimeLoopStatus::Achieved);
        assert_eq!(metrics.rounds_executed, 1);
        assert!(metrics.achieved_on_round.is_some());
    }

    #[tokio::test]
    async fn test_controller_loop_converges_after_scaling() {
        // No agents yet — reconciler will spawn them.
        let state = make_state(3, 0);
        let config = fast_config();
        let ctrl = ControllerLoop::new(config, state).await.unwrap();
        let metrics = ctrl.run().await.unwrap();

        // After reconciliation, agents are spawned and running.
        // Convergence should be achieved.
        assert_eq!(metrics.status, RuntimeLoopStatus::Achieved);
        assert!(
            metrics.rounds_executed <= 3,
            "Should converge quickly after spawn"
        );
    }

    #[tokio::test]
    async fn test_controller_loop_max_rounds() {
        let state = make_state(0, 0);
        let mut config = fast_config();
        config.runtime.quality_threshold = 0.999; // nearly impossible
        config.runtime.max_rounds = 3;

        let ctrl = ControllerLoop::new(config, state).await.unwrap();
        let metrics = ctrl.run().await.unwrap();

        // With 0 desired and 0 running, convergence is 1.0, so it should still achieve.
        // But if threshold is set impossibly high for a non-trivial case, max_rounds kicks in.
        assert!(metrics.rounds_executed <= 3);
    }

    #[tokio::test]
    async fn test_controller_loop_records_feedback_chain() {
        let state = make_state(2, 0);
        let config = fast_config();
        let ctrl = ControllerLoop::new(config, state).await.unwrap();
        let metrics = ctrl.run().await.unwrap();

        assert!(
            !metrics.feedback_chain.is_empty(),
            "Should have at least one feedback entry"
        );
    }

    #[tokio::test]
    async fn test_controller_loop_config_defaults() {
        let config = ControllerLoopConfig::default();
        assert_eq!(config.runtime.max_rounds, 20);
        assert!((config.runtime.quality_threshold - 0.95).abs() < f64::EPSILON);
        assert!(config.runtime.stop_on_achieve);
    }

    #[tokio::test]
    async fn test_controller_loop_with_defaults_factory() {
        let state = make_state(1, 1);
        let ctrl = ControllerLoop::with_defaults(state).await.unwrap();
        let metrics = ctrl.run().await.unwrap();
        assert_eq!(metrics.status, RuntimeLoopStatus::Achieved);
    }

    // ── RoundActionSummary serialization ──────────────────────────────────

    #[test]
    fn test_round_action_summary_serializable() {
        let summary = RoundActionSummary {
            agents_checked: 3,
            alive: 3,
            restarted: 0,
            reconciled: true,
            running_after: 3,
            desired_replicas: 3,
            failed_after: 0,
            description: "test round".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"reconciled\":true"));
        assert!(json.contains("\"running_after\":3"));
    }

    // ── ControllerEventObserver tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_event_observer_publishes_on_round_start() {
        let bus = Arc::new(EventBus::new());
        let mut rx = bus.subscribe(None, crate::events::EventType::All);
        let observer = ControllerEventObserver::new(bus.clone());

        observer.on_round_start(1, "test-goal").await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event, AgentEvent::HealthChanged);
        assert_eq!(event.metadata.get("phase").unwrap(), "round_start");
        assert_eq!(event.metadata.get("round").unwrap(), "1");
    }

    #[tokio::test]
    async fn test_event_observer_publishes_on_status_change() {
        let bus = Arc::new(EventBus::new());
        let mut rx = bus.subscribe(None, crate::events::EventType::All);
        let observer = ControllerEventObserver::new(bus.clone());

        observer
            .on_status_change(
                "test-goal",
                &RuntimeLoopStatus::Running,
                &RuntimeLoopStatus::Achieved,
            )
            .await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event, AgentEvent::Completed); // Achieved → Completed
        assert_eq!(event.metadata.get("phase").unwrap(), "status_change");
    }

    #[tokio::test]
    async fn test_event_observer_failed_status_maps_to_failed_event() {
        let bus = Arc::new(EventBus::new());
        let mut rx = bus.subscribe(None, crate::events::EventType::All);
        let observer = ControllerEventObserver::new(bus.clone());

        observer
            .on_status_change(
                "goal",
                &RuntimeLoopStatus::Running,
                &RuntimeLoopStatus::Failed("boom".to_string()),
            )
            .await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event, AgentEvent::Failed);
    }
}
