use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::models::agent::AgentStatus;
use crate::AppState;

/// Current scheduler algorithm
#[derive(Debug, Serialize)]
pub struct SchedulerAlgorithm {
    pub name: String,
    pub description: String,
}

/// Queue depth snapshot
#[derive(Debug, Serialize)]
pub struct QueueDepth {
    pub pending: usize,
    pub scheduled: usize,
    pub running: usize,
}

/// Scheduling throughput metrics
#[derive(Debug, Serialize)]
pub struct SchedulingThroughput {
    pub total_scheduled: usize,
    pub total_completed: usize,
    pub total_failed: usize,
    pub success_rate: f64,
    pub avg_restart_count: f64,
}

/// Recent scheduling decision
#[derive(Debug, Serialize)]
pub struct SchedulingDecision {
    pub agent_id: String,
    pub agent_name: String,
    pub assigned_node: Option<String>,
    pub status: String,
    pub priority: String,
    pub timestamp: String,
}

/// Full scheduler status response
#[derive(Debug, Serialize)]
pub struct SchedulerStatus {
    pub current_algorithm: SchedulerAlgorithm,
    pub queue_depth: QueueDepth,
    pub throughput: SchedulingThroughput,
    pub node_utilization: Vec<NodeUtilization>,
    pub recent_decisions: Vec<SchedulingDecision>,
}

/// Node utilization info
#[derive(Debug, Serialize)]
pub struct NodeUtilization {
    pub node_id: String,
    pub node_name: String,
    pub agent_count: usize,
    pub running_count: usize,
    pub status: String,
}

/// GET /api/v1/scheduler/status
/// Returns comprehensive scheduler status including algorithm, queue, throughput, and node utilization.
pub async fn scheduler_status(State(state): State<AppState>) -> Json<SchedulerStatus> {
    let agents = state.agents.read().await;
    let nodes = state.nodes.read().await;

    // Queue depth
    let pending = agents
        .values()
        .filter(|a| a.status == AgentStatus::Pending)
        .count();
    let scheduled = agents
        .values()
        .filter(|a| a.status == AgentStatus::Scheduled)
        .count();
    let running = agents
        .values()
        .filter(|a| a.status == AgentStatus::Running)
        .count();

    // Throughput
    let total_scheduled = agents.len();
    let total_completed = agents
        .values()
        .filter(|a| a.status == AgentStatus::Succeeded)
        .count();
    let total_failed = agents
        .values()
        .filter(|a| a.status == AgentStatus::Failed)
        .count();
    let success_rate = if total_scheduled > 0 {
        total_completed as f64 / total_scheduled as f64 * 100.0
    } else {
        100.0
    };
    let avg_restart = if !agents.is_empty() {
        agents.values().map(|a| a.restart_count as f64).sum::<f64>() / agents.len() as f64
    } else {
        0.0
    };

    // Node utilization
    let node_utilization: Vec<NodeUtilization> = nodes
        .values()
        .map(|n| {
            let node_agents: Vec<_> = agents
                .values()
                .filter(|a| a.node_id.as_deref() == Some(&n.id))
                .collect();
            let running_count = node_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Running)
                .count();
            NodeUtilization {
                node_id: n.id.clone(),
                node_name: n.name.clone(),
                agent_count: node_agents.len(),
                running_count,
                status: format!("{:?}", n.status),
            }
        })
        .collect();

    // Recent decisions (last 10 agents by updated_at)
    let mut recent: Vec<SchedulingDecision> = agents
        .values()
        .map(|a| SchedulingDecision {
            agent_id: a.id.clone(),
            agent_name: a.spec.name.clone(),
            assigned_node: a.node_id.clone(),
            status: format!("{:?}", a.status),
            priority: a.spec.priority.clone(),
            timestamp: a.updated_at.clone(),
        })
        .collect();
    recent.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    recent.truncate(10);

    Json(SchedulerStatus {
        current_algorithm: SchedulerAlgorithm {
            name: "Weighted Round Robin".to_string(),
            description: "Priority-aware weighted round-robin with cache optimization and affinity constraints"
                .to_string(),
        },
        queue_depth: QueueDepth {
            pending,
            scheduled,
            running,
        },
        throughput: SchedulingThroughput {
            total_scheduled,
            total_completed,
            total_failed,
            success_rate,
            avg_restart_count: avg_restart,
        },
        node_utilization,
        recent_decisions: recent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::collections::HashMap;

    fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        AppState::new(config)
    }

    #[tokio::test]
    async fn test_scheduler_status_empty() {
        let state = test_state();
        let result = scheduler_status(State(state)).await;
        assert_eq!(result.queue_depth.pending, 0);
        assert_eq!(result.queue_depth.running, 0);
        assert_eq!(result.throughput.success_rate, 100.0);
        assert_eq!(result.current_algorithm.name, "Weighted Round Robin");
        assert_eq!(result.node_utilization.len(), 2); // 2 default nodes
    }

    #[tokio::test]
    async fn test_scheduler_status_with_agents() {
        use crate::models::agent::{Agent, AgentSpec};

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // Create agents in different states
        for (name, status) in [
            ("pending-agent", AgentStatus::Pending),
            ("running-agent", AgentStatus::Running),
            ("succeeded-agent", AgentStatus::Succeeded),
            ("failed-agent", AgentStatus::Failed),
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = status;
            agents.insert(agent.id.clone(), agent);
        }

        let state = AppState {
            config: Arc::new(config),
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.queue_depth.pending, 1);
        assert_eq!(result.queue_depth.running, 1);
        assert_eq!(result.throughput.total_scheduled, 4);
        assert_eq!(result.throughput.total_completed, 1);
        assert_eq!(result.throughput.total_failed, 1);
        assert_eq!(result.recent_decisions.len(), 4);
    }

    #[tokio::test]
    async fn test_node_utilization() {
        let state = test_state();
        let result = scheduler_status(State(state)).await;
        // Default state has 2 nodes
        assert_eq!(result.node_utilization.len(), 2);
        for nu in &result.node_utilization {
            assert_eq!(nu.agent_count, 0);
            assert_eq!(nu.running_count, 0);
        }
    }

    use std::sync::Arc;
    use tokio::sync::RwLock;
}
