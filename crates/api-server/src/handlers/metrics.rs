use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::error::ApiError;
use crate::models::agent::AgentStatus;
use crate::AppState;

/// System-wide metrics summary
#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub agent_count: usize,
    pub node_count: usize,
    pub task_stats: TaskStats,
}

/// Task statistics broken down by agent status
#[derive(Debug, Serialize)]
pub struct TaskStats {
    pub pending: usize,
    pub scheduled: usize,
    pub running: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub unknown: usize,
}

/// Per-agent metrics
#[derive(Debug, Serialize)]
pub struct AgentMetrics {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub node_id: Option<String>,
    pub restart_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub start_time: Option<String>,
}

/// Cluster health overview
#[derive(Debug, Serialize)]
pub struct ClusterStatus {
    pub overall: String,
    pub nodes: Vec<NodeHealth>,
    pub total_agents: usize,
    pub running_agents: usize,
}

/// Health info for a single node
#[derive(Debug, Serialize)]
pub struct NodeHealth {
    pub id: String,
    pub name: String,
    pub status: String,
    pub cpu: String,
    pub memory: String,
    pub gpu: String,
}

/// GET /api/v1/metrics/summary
/// Returns system-wide metrics summary: agent count, node count, task stats.
pub async fn metrics_summary(State(state): State<AppState>) -> Json<MetricsSummary> {
    let agents = state.agents.read().await;
    let nodes = state.nodes.read().await;

    let mut task_stats = TaskStats {
        pending: 0,
        scheduled: 0,
        running: 0,
        succeeded: 0,
        failed: 0,
        unknown: 0,
    };

    for agent in agents.values() {
        match agent.status {
            AgentStatus::Pending => task_stats.pending += 1,
            AgentStatus::Scheduled => task_stats.scheduled += 1,
            AgentStatus::Running => task_stats.running += 1,
            AgentStatus::Succeeded => task_stats.succeeded += 1,
            AgentStatus::Failed => task_stats.failed += 1,
            AgentStatus::Unknown => task_stats.unknown += 1,
        }
    }

    Json(MetricsSummary {
        agent_count: agents.len(),
        node_count: nodes.len(),
        task_stats,
    })
}

/// GET /api/v1/metrics/agents/:id
/// Returns metrics for a specific agent.
pub async fn agent_metrics(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentMetrics>, ApiError> {
    let agents = state.agents.read().await;
    let agent = agents
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?;

    Ok(Json(AgentMetrics {
        id: agent.id.clone(),
        name: agent.spec.name.clone(),
        status: agent.status.clone(),
        node_id: agent.node_id.clone(),
        restart_count: agent.restart_count,
        created_at: agent.created_at.clone(),
        updated_at: agent.updated_at.clone(),
        start_time: agent.start_time.clone(),
    }))
}

/// GET /api/v1/cluster/status
/// Returns cluster health overview including node health and agent distribution.
pub async fn cluster_status(State(state): State<AppState>) -> Json<ClusterStatus> {
    let agents = state.agents.read().await;
    let nodes = state.nodes.read().await;

    let node_health: Vec<NodeHealth> = nodes
        .values()
        .map(|n| NodeHealth {
            id: n.id.clone(),
            name: n.name.clone(),
            status: format!("{:?}", n.status),
            cpu: n.resources.cpu.clone(),
            memory: n.resources.memory.clone(),
            gpu: n.resources.gpu.clone(),
        })
        .collect();

    let all_healthy = node_health.iter().all(|n| n.status == "Ready");
    let overall = if all_healthy {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };

    let running_agents = agents
        .values()
        .filter(|a| a.status == AgentStatus::Running)
        .count();

    Json(ClusterStatus {
        overall,
        nodes: node_health,
        total_agents: agents.len(),
        running_agents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_summary_serialize() {
        let summary = MetricsSummary {
            agent_count: 5,
            node_count: 3,
            task_stats: TaskStats {
                pending: 1,
                scheduled: 0,
                running: 2,
                succeeded: 1,
                failed: 1,
                unknown: 0,
            },
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("agent_count"));
        assert!(json.contains("5"));
        assert!(json.contains("node_count"));
        assert!(json.contains("3"));
        assert!(json.contains("running"));
        assert!(json.contains("2"));
    }

    #[test]
    fn test_agent_metrics_serialize() {
        let metrics = AgentMetrics {
            id: "a1".to_string(),
            name: "test-agent".to_string(),
            status: AgentStatus::Running,
            node_id: Some("n1".to_string()),
            restart_count: 2,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-02".to_string(),
            start_time: Some("2026-01-01T10:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("restart_count"));
        assert!(json.contains("2"));
        assert!(json.contains("Running"));
    }

    #[test]
    fn test_cluster_status_serialize() {
        let status = ClusterStatus {
            overall: "healthy".to_string(),
            nodes: vec![NodeHealth {
                id: "n1".to_string(),
                name: "node-1".to_string(),
                status: "Ready".to_string(),
                cpu: "4".to_string(),
                memory: "8Gi".to_string(),
                gpu: "0".to_string(),
            }],
            total_agents: 3,
            running_agents: 2,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("overall"));
        assert!(json.contains("healthy"));
        assert!(json.contains("total_agents"));
        assert!(json.contains("3"));
        assert!(json.contains("running_agents"));
        assert!(json.contains("2"));
    }

    #[test]
    fn test_node_health_serialize() {
        let nh = NodeHealth {
            id: "n1".to_string(),
            name: "worker".to_string(),
            status: "Ready".to_string(),
            cpu: "8".to_string(),
            memory: "16Gi".to_string(),
            gpu: "1".to_string(),
        };
        let json = serde_json::to_string(&nh).unwrap();
        assert!(json.contains("gpu"));
        assert!(json.contains("1"));
    }

    #[test]
    fn test_task_stats_all_zeros() {
        let stats = TaskStats {
            pending: 0,
            scheduled: 0,
            running: 0,
            succeeded: 0,
            failed: 0,
            unknown: 0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("pending"));
        assert!(json.contains("0"));
    }
}
