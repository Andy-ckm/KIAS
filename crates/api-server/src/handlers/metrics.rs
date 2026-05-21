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

    use axum::extract::State;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::models::agent::{Agent, AgentSpec, AgentStatus};

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: crate::handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: crate::handlers::auth_gxp::create_gxp_auth_state(
                kias_common::gxp_auth::PasswordPolicy::default(),
            ),
            jwt_config: crate::auth::JwtConfig::new(
                "kias-default-jwt-secret-change-me",
                "kias",
                24,
            ),
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    fn make_agent(id: &str, name: &str, status: AgentStatus) -> Agent {
        Agent {
            id: id.to_string(),
            spec: AgentSpec {
                name: name.to_string(),
                image: "python:3.11".to_string(),
                command: vec!["python".to_string()],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            },
            status,
            node_id: Some("node-1".to_string()),
            resource_usage: Default::default(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            start_time: Some("2026-01-01T00:00:00Z".to_string()),
            restart_count: 0,
        }
    }

    #[tokio::test]
    async fn test_metrics_summary_empty() {
        let state = test_state().await;
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.agent_count, 0);
        assert_eq!(result.node_count, 0);
        assert_eq!(result.task_stats.pending, 0);
        assert_eq!(result.task_stats.running, 0);
    }

    #[tokio::test]
    async fn test_metrics_summary_with_agents() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "agent-1", AgentStatus::Running),
            );
            agents.insert(
                "a2".into(),
                make_agent("a2", "agent-2", AgentStatus::Pending),
            );
            agents.insert(
                "a3".into(),
                make_agent("a3", "agent-3", AgentStatus::Failed),
            );
            agents.insert(
                "a4".into(),
                make_agent("a4", "agent-4", AgentStatus::Running),
            );
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.agent_count, 4);
        assert_eq!(result.task_stats.running, 2);
        assert_eq!(result.task_stats.pending, 1);
        assert_eq!(result.task_stats.failed, 1);
        assert_eq!(result.task_stats.succeeded, 0);
    }

    #[tokio::test]
    async fn test_metrics_summary_all_statuses() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert("a1".into(), make_agent("a1", "a1", AgentStatus::Pending));
            agents.insert("a2".into(), make_agent("a2", "a2", AgentStatus::Scheduled));
            agents.insert("a3".into(), make_agent("a3", "a3", AgentStatus::Running));
            agents.insert("a4".into(), make_agent("a4", "a4", AgentStatus::Succeeded));
            agents.insert("a5".into(), make_agent("a5", "a5", AgentStatus::Failed));
            agents.insert("a6".into(), make_agent("a6", "a6", AgentStatus::Unknown));
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.agent_count, 6);
        assert_eq!(result.task_stats.pending, 1);
        assert_eq!(result.task_stats.scheduled, 1);
        assert_eq!(result.task_stats.running, 1);
        assert_eq!(result.task_stats.succeeded, 1);
        assert_eq!(result.task_stats.failed, 1);
        assert_eq!(result.task_stats.unknown, 1);
    }

    #[tokio::test]
    async fn test_agent_metrics_found() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "agent-42".into(),
                make_agent("agent-42", "my-agent", AgentStatus::Running),
            );
        }
        let result = agent_metrics(State(state), Path("agent-42".to_string())).await;
        assert!(result.is_ok());
        let m = result.unwrap();
        assert_eq!(m.id, "agent-42");
        assert_eq!(m.name, "my-agent");
        assert_eq!(m.status, AgentStatus::Running);
        assert_eq!(m.restart_count, 0);
    }

    #[tokio::test]
    async fn test_agent_metrics_not_found() {
        let state = test_state().await;
        let result = agent_metrics(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_cluster_status_empty() {
        let state = test_state().await;
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "healthy");
        assert_eq!(result.nodes.len(), 0);
        assert_eq!(result.total_agents, 0);
        assert_eq!(result.running_agents, 0);
    }

    #[tokio::test]
    async fn test_cluster_status_all_healthy() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "node-1".to_string(),
                    status: NodeStatus::Ready,
                    resources: ResourceCapacity {
                        cpu: "4".into(),
                        memory: "8Gi".into(),
                        gpu: "0".into(),
                    },
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "healthy");
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].status, "Ready");
    }

    #[tokio::test]
    async fn test_cluster_status_degraded() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "node-1".to_string(),
                    status: NodeStatus::Ready,
                    resources: ResourceCapacity {
                        cpu: "4".into(),
                        memory: "8Gi".into(),
                        gpu: "0".into(),
                    },
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
            nodes.insert(
                "n2".into(),
                Node {
                    id: "n2".to_string(),
                    name: "node-2".to_string(),
                    status: NodeStatus::NotReady,
                    resources: ResourceCapacity {
                        cpu: "4".into(),
                        memory: "8Gi".into(),
                        gpu: "0".into(),
                    },
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "agent-1", AgentStatus::Running),
            );
            agents.insert(
                "a2".into(),
                make_agent("a2", "agent-2", AgentStatus::Running),
            );
            agents.insert(
                "a3".into(),
                make_agent("a3", "agent-3", AgentStatus::Pending),
            );
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "degraded");
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.total_agents, 3);
        assert_eq!(result.running_agents, 2);
    }

    #[tokio::test]
    async fn test_metrics_summary_node_count() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            for i in 0..3 {
                nodes.insert(
                    format!("n{}", i),
                    Node {
                        id: format!("n{}", i),
                        name: format!("node-{}", i),
                        status: NodeStatus::Ready,
                        resources: ResourceCapacity::default(),
                        allocatable: ResourceCapacity::default(),
                        labels: HashMap::new(),
                        created_at: "2026-01-01".into(),
                        last_heartbeat: "2026-01-01".into(),
                    },
                );
            }
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.node_count, 3);
        assert_eq!(result.agent_count, 0);
    }

    #[tokio::test]
    async fn test_agent_metrics_with_restart_count() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut agent = make_agent("a1", "flaky-agent", AgentStatus::Running);
            agent.restart_count = 7;
            agent.start_time = Some("2026-05-20T10:00:00Z".to_string());
            agent.node_id = Some("node-2".to_string());
            agents.insert("a1".into(), agent);
        }
        let result = agent_metrics(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.restart_count, 7);
        assert_eq!(result.start_time, Some("2026-05-20T10:00:00Z".to_string()));
        assert_eq!(result.node_id, Some("node-2".to_string()));
    }

    #[tokio::test]
    async fn test_agent_metrics_no_start_time() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut agent = make_agent("a1", "pending-agent", AgentStatus::Pending);
            agent.start_time = None;
            agent.node_id = None;
            agents.insert("a1".into(), agent);
        }
        let result = agent_metrics(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.status, AgentStatus::Pending);
        assert!(result.start_time.is_none());
        assert!(result.node_id.is_none());
    }

    #[tokio::test]
    async fn test_agent_metrics_failed_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "crashed", AgentStatus::Failed),
            );
        }
        let result = agent_metrics(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.status, AgentStatus::Failed);
        assert_eq!(result.name, "crashed");
    }

    #[tokio::test]
    async fn test_cluster_status_no_agents_with_nodes() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "node-1".to_string(),
                    status: NodeStatus::Ready,
                    resources: ResourceCapacity {
                        cpu: "8".into(),
                        memory: "16Gi".into(),
                        gpu: "2".into(),
                    },
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "healthy");
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].cpu, "8");
        assert_eq!(result.nodes[0].gpu, "2");
        assert_eq!(result.total_agents, 0);
        assert_eq!(result.running_agents, 0);
    }

    #[tokio::test]
    async fn test_cluster_status_all_not_ready() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            for i in 0..3 {
                nodes.insert(
                    format!("n{}", i),
                    Node {
                        id: format!("n{}", i),
                        name: format!("node-{}", i),
                        status: NodeStatus::NotReady,
                        resources: ResourceCapacity::default(),
                        allocatable: ResourceCapacity::default(),
                        labels: HashMap::new(),
                        created_at: "2026-01-01".into(),
                        last_heartbeat: "2026-01-01".into(),
                    },
                );
            }
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "degraded");
        assert_eq!(result.nodes.len(), 3);
        for node in &result.nodes {
            assert_eq!(node.status, "NotReady");
        }
    }

    #[tokio::test]
    async fn test_metrics_summary_scheduled_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "sched-1", AgentStatus::Scheduled),
            );
            agents.insert(
                "a2".into(),
                make_agent("a2", "sched-2", AgentStatus::Scheduled),
            );
            agents.insert("a3".into(), make_agent("a3", "run-1", AgentStatus::Running));
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.task_stats.scheduled, 2);
        assert_eq!(result.task_stats.running, 1);
        assert_eq!(result.task_stats.pending, 0);
        assert_eq!(result.agent_count, 3);
    }

    #[tokio::test]
    async fn test_metrics_summary_unknown_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "unknown-1", AgentStatus::Unknown),
            );
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.task_stats.unknown, 1);
        assert_eq!(result.task_stats.pending, 0);
        assert_eq!(result.task_stats.running, 0);
    }

    #[tokio::test]
    async fn test_metrics_summary_nodes_only_no_agents() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "node-1".to_string(),
                    status: NodeStatus::Ready,
                    resources: ResourceCapacity::default(),
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.node_count, 1);
        assert_eq!(result.agent_count, 0);
        assert_eq!(result.task_stats.pending, 0);
        assert_eq!(result.task_stats.running, 0);
    }

    #[tokio::test]
    async fn test_agent_metrics_succeeded_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".into(),
                make_agent("a1", "done-agent", AgentStatus::Succeeded),
            );
        }
        let result = agent_metrics(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.status, AgentStatus::Succeeded);
        assert_eq!(result.name, "done-agent");
    }

    #[tokio::test]
    async fn test_agent_metrics_not_found_returns_404() {
        let state = test_state().await;
        let result = agent_metrics(State(state), Path("ghost-agent".to_string())).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
        assert!(err.message.contains("ghost-agent"));
    }

    #[tokio::test]
    async fn test_metrics_summary_task_stats_sum_equals_agent_count() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert("a1".into(), make_agent("a1", "a1", AgentStatus::Running));
            agents.insert("a2".into(), make_agent("a2", "a2", AgentStatus::Pending));
            agents.insert("a3".into(), make_agent("a3", "a3", AgentStatus::Failed));
            agents.insert("a4".into(), make_agent("a4", "a4", AgentStatus::Succeeded));
        }
        let result = metrics_summary(State(state)).await;
        let stats_sum = result.task_stats.pending
            + result.task_stats.scheduled
            + result.task_stats.running
            + result.task_stats.succeeded
            + result.task_stats.failed
            + result.task_stats.unknown;
        assert_eq!(stats_sum, result.agent_count);
    }

    #[tokio::test]
    async fn test_agent_metrics_timestamps_preserved() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut agent = make_agent("a1", "timed-agent", AgentStatus::Running);
            agent.created_at = "2026-01-15T08:00:00Z".to_string();
            agent.updated_at = "2026-05-20T21:00:00Z".to_string();
            agent.start_time = Some("2026-01-15T08:05:00Z".to_string());
            agents.insert("a1".into(), agent);
        }
        let result = agent_metrics(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.created_at, "2026-01-15T08:00:00Z");
        assert_eq!(result.updated_at, "2026-05-20T21:00:00Z");
        assert_eq!(result.start_time, Some("2026-01-15T08:05:00Z".to_string()));
    }

    #[tokio::test]
    async fn test_cluster_status_node_resources_in_health() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "gpu-node".to_string(),
                    status: NodeStatus::Ready,
                    resources: ResourceCapacity {
                        cpu: "32".into(),
                        memory: "128Gi".into(),
                        gpu: "4".into(),
                    },
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].cpu, "32");
        assert_eq!(result.nodes[0].memory, "128Gi");
        assert_eq!(result.nodes[0].gpu, "4");
        assert_eq!(result.nodes[0].name, "gpu-node");
    }

    #[tokio::test]
    async fn test_metrics_summary_single_agent() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "solo".into(),
                make_agent("solo", "solo-agent", AgentStatus::Running),
            );
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.agent_count, 1);
        assert_eq!(result.task_stats.running, 1);
        assert_eq!(result.task_stats.pending, 0);
    }

    #[tokio::test]
    async fn test_cluster_status_draining_nodes() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let state = test_state().await;
        {
            let mut nodes = state.nodes.write().await;
            nodes.insert(
                "n1".into(),
                Node {
                    id: "n1".to_string(),
                    name: "draining-node".to_string(),
                    status: NodeStatus::Draining,
                    resources: ResourceCapacity::default(),
                    allocatable: ResourceCapacity::default(),
                    labels: HashMap::new(),
                    created_at: "2026-01-01".into(),
                    last_heartbeat: "2026-01-01".into(),
                },
            );
        }
        let result = cluster_status(State(state)).await;
        assert_eq!(result.overall, "degraded");
        assert_eq!(result.nodes[0].status, "Draining");
    }

    #[tokio::test]
    async fn test_metrics_summary_no_double_count() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            // Insert same ID twice — second should overwrite
            agents.insert("a1".into(), make_agent("a1", "first", AgentStatus::Pending));
            agents.insert(
                "a1".into(),
                make_agent("a1", "second", AgentStatus::Running),
            );
        }
        let result = metrics_summary(State(state)).await;
        assert_eq!(result.agent_count, 1);
        assert_eq!(result.task_stats.running, 1);
        assert_eq!(result.task_stats.pending, 0);
    }

    #[test]
    fn test_task_stats_debug_format() {
        let stats = TaskStats {
            pending: 1,
            scheduled: 2,
            running: 3,
            succeeded: 4,
            failed: 5,
            unknown: 6,
        };
        let debug = format!("{:?}", stats);
        assert!(debug.contains("pending"));
        assert!(debug.contains("scheduled"));
        assert!(debug.contains("running"));
        assert!(debug.contains("succeeded"));
        assert!(debug.contains("failed"));
        assert!(debug.contains("unknown"));
    }

    #[test]
    fn test_cluster_status_serialize_overall_field() {
        let status = ClusterStatus {
            overall: "degraded".to_string(),
            nodes: vec![],
            total_agents: 0,
            running_agents: 0,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["overall"], "degraded");
        assert_eq!(json["total_agents"], 0);
        assert_eq!(json["running_agents"], 0);
    }
}
