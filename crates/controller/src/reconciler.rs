use super::autonomy_integration::{ActionApproval, AutonomyGate};
use super::state::{AgentConfig, AgentInfo, AgentStatus, ControllerState};
use async_trait::async_trait;
use chrono::Utc;
use kias_common::KiasResult;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Callback trait for spawning actual agents — keeps the reconciler testable and
/// allows production code to wire in real HTTP/CLI agent creation.
#[async_trait]
pub trait AgentSpawner: Send + Sync {
    /// Spawn a single agent instance and return its ID.
    async fn spawn(&self, config: &AgentConfig) -> KiasResult<String>;
}

/// No-op spawner for testing and dry-run scenarios.
pub struct NoOpSpawner;

#[async_trait]
impl AgentSpawner for NoOpSpawner {
    async fn spawn(&self, config: &AgentConfig) -> KiasResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        tracing::debug!(agent_id = %id, name = %config.name, "NoOpSpawner: would spawn agent");
        Ok(id)
    }
}

#[async_trait]
pub trait Reconciler: Send + Sync {
    async fn reconcile(&self, state: &mut ControllerState) -> KiasResult<()>;
}

pub struct DefaultReconciler<S: AgentSpawner> {
    spawner: Arc<S>,
    autonomy_gate: Option<Arc<Mutex<AutonomyGate>>>,
}

impl Default for DefaultReconciler<NoOpSpawner> {
    fn default() -> Self {
        Self::new(NoOpSpawner)
    }
}

impl<S: AgentSpawner> DefaultReconciler<S> {
    pub fn new(spawner: S) -> Self {
        Self {
            spawner: Arc::new(spawner),
            autonomy_gate: None,
        }
    }

    /// Attach an [`AutonomyGate`] — spawns will be policy-checked before execution.
    pub fn with_autonomy_gate(mut self, gate: AutonomyGate) -> Self {
        self.autonomy_gate = Some(Arc::new(Mutex::new(gate)));
        self
    }
}

#[async_trait]
impl<S: AgentSpawner> Reconciler for DefaultReconciler<S> {
    async fn reconcile(&self, state: &mut ControllerState) -> KiasResult<()> {
        tracing::info!("Reconciling controller state");

        // Count how many agents we actually have tracked
        let actual_tracked = state
            .agents
            .values()
            .filter(|a| matches!(a.status, AgentStatus::Running))
            .count() as u32;

        if actual_tracked < state.desired.replicas {
            let to_spawn = state.desired.replicas - actual_tracked;
            tracing::info!(
                tracked_running = actual_tracked,
                desired = state.desired.replicas,
                to_spawn = to_spawn,
                "Scaling up"
            );

            for i in 0..to_spawn {
                // Autonomy gate check before spawning
                if let Some(ref gate) = self.autonomy_gate {
                    let mut g = gate.lock().await;
                    let approval = g.check_approval("agent_spawn");
                    match approval {
                        ActionApproval::Approved | ActionApproval::ApprovedWithSandbox => {
                            tracing::debug!(tool = "agent_spawn", "Autonomy gate: approved");
                        }
                        other => {
                            tracing::warn!(
                                tool = "agent_spawn",
                                ?other,
                                "Autonomy gate: spawn blocked"
                            );
                            // Record failure and skip this spawn
                            g.record_outcome("agent_spawn", false);
                            continue;
                        }
                    }
                    drop(g);
                }

                let agent_id = self.spawner.spawn(&state.desired.agent_config).await?;
                let agent_name = format!(
                    "{}-{}",
                    state.desired.agent_config.name,
                    actual_tracked + i + 1
                );
                // Record successful spawn with autonomy gate
                if let Some(ref gate) = self.autonomy_gate {
                    gate.lock().await.record_outcome("agent_spawn", true);
                }

                let mut info = AgentInfo::new(&agent_id, &agent_name);
                info.status = AgentStatus::Running;
                state.agents.insert(agent_id, info);
            }

            state.actual.running_replicas = state.desired.replicas;
            state.actual.agent_status = AgentStatus::Running;
            state.actual.last_updated = Utc::now();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::*;
    use std::collections::HashMap;

    fn make_test_state(desired_replicas: u32, actual_replicas: u32) -> ControllerState {
        ControllerState {
            desired: DesiredState {
                replicas: desired_replicas,
                agent_config: AgentConfig {
                    name: "test-agent".to_string(),
                    image: "python:3.11".to_string(),
                    resources: ResourceRequirements {
                        cpu: "100m".to_string(),
                        memory: "128Mi".to_string(),
                    },
                },
            },
            actual: ActualState {
                running_replicas: actual_replicas,
                agent_status: AgentStatus::Pending,
                last_updated: Utc::now(),
            },
            agents: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_reconcile_scale_up() {
        let reconciler = DefaultReconciler::<NoOpSpawner>::default();
        let mut state = make_test_state(3, 1);

        reconciler.reconcile(&mut state).await.unwrap();

        assert_eq!(state.actual.running_replicas, 3);
        assert!(matches!(state.actual.agent_status, AgentStatus::Running));
        // Verify agents were actually spawned
        assert_eq!(state.agents.len(), 3);
    }

    #[tokio::test]
    async fn test_reconcile_already_at_desired() {
        let reconciler = DefaultReconciler::<NoOpSpawner>::default();
        // Pre-populate agents to match desired replicas
        let mut state = make_test_state(2, 2);
        for i in 1..=2 {
            let mut info = AgentInfo::new(&format!("agent-{}", i), &format!("test-agent-{}", i));
            info.status = AgentStatus::Running;
            state.agents.insert(format!("agent-{}", i), info);
        }

        reconciler.reconcile(&mut state).await.unwrap();

        assert_eq!(state.actual.running_replicas, 2);
        // No new agents spawned since HashMap already matches desired
        assert_eq!(state.agents.len(), 2);
    }

    #[tokio::test]
    async fn test_reconcile_spawns_correct_count() {
        let reconciler = DefaultReconciler::<NoOpSpawner>::default();
        let mut state = make_test_state(5, 0);

        reconciler.reconcile(&mut state).await.unwrap();

        assert_eq!(state.actual.running_replicas, 5);
        assert_eq!(state.agents.len(), 5);
        // Each agent should have a unique ID and Running status
        for (_id, info) in &state.agents {
            assert!(matches!(info.status, AgentStatus::Running));
        }
    }

    #[test]
    fn test_agent_status_variants() {
        assert_ne!(AgentStatus::Pending, AgentStatus::Running);
        assert_ne!(AgentStatus::Failed, AgentStatus::Succeeded);
        assert_ne!(AgentStatus::Running, AgentStatus::Unresponsive);
    }

    #[test]
    fn test_desired_state_creation() {
        let state = make_test_state(5, 0);
        assert_eq!(state.desired.replicas, 5);
        assert_eq!(state.desired.agent_config.name, "test-agent");
    }

    #[tokio::test]
    async fn test_reconcile_with_autonomy_gate_full_auto() {
        use super::super::autonomy_integration::AutonomyGate;
        use kias_autonomy_controller::AutonomyLevel;

        let mut gate = AutonomyGate::new();
        gate.set_level(AutonomyLevel::FullAuto);
        let reconciler = DefaultReconciler::<NoOpSpawner>::default().with_autonomy_gate(gate);
        let mut state = make_test_state(3, 0);

        reconciler.reconcile(&mut state).await.unwrap();

        // FullAuto should allow all spawns
        assert_eq!(state.agents.len(), 3);
    }
}
