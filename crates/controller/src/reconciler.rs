use super::state::{AgentConfig, AgentInfo, AgentStatus, ControllerState};
use async_trait::async_trait;
use chrono::Utc;
use kias_common::KiasResult;
use std::sync::Arc;

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
        }
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
                let agent_id = self.spawner.spawn(&state.desired.agent_config).await?;
                let agent_name = format!(
                    "{}-{}",
                    state.desired.agent_config.name,
                    actual_tracked + i + 1
                );
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
}
