use async_trait::async_trait;
use kias_common::KiasResult;
use super::state::{ControllerState, AgentStatus};
use chrono::Utc;

#[async_trait]
pub trait Reconciler: Send + Sync {
    async fn reconcile(&self, state: &mut ControllerState) -> KiasResult<()>;
}

pub struct DefaultReconciler;

impl Default for DefaultReconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultReconciler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Reconciler for DefaultReconciler {
    async fn reconcile(&self, state: &mut ControllerState) -> KiasResult<()> {
        tracing::info!("Reconciling controller state");
        
        // Simplified reconciliation logic
        if state.actual.running_replicas < state.desired.replicas {
            tracing::info!(
                current = state.actual.running_replicas,
                desired = state.desired.replicas,
                "Scaling up"
            );
            // TODO: Actually create agents
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
        let reconciler = DefaultReconciler::new();
        let mut state = make_test_state(3, 1);

        reconciler.reconcile(&mut state).await.unwrap();

        assert_eq!(state.actual.running_replicas, 3);
        assert!(matches!(state.actual.agent_status, AgentStatus::Running));
    }

    #[tokio::test]
    async fn test_reconcile_already_at_desired() {
        let reconciler = DefaultReconciler::new();
        let mut state = make_test_state(2, 2);

        reconciler.reconcile(&mut state).await.unwrap();

        assert_eq!(state.actual.running_replicas, 2);
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
