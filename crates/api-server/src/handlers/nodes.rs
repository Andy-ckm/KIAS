use axum::extract::{Path, Query, State};
use axum::Json;

use crate::error::ApiError;
use crate::models::node::Node;
use crate::models::request::{ApiResponse, ListResponse, PaginationParams};
use crate::AppState;

/// GET /api/v1/nodes
/// List all nodes
pub async fn list_nodes(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationParams>,
) -> Json<ListResponse<Node>> {
    tracing::info!(page = ?pagination.page, per_page = ?pagination.per_page, "Listing nodes");
    let nodes = state.nodes.read().await;
    let all: Vec<Node> = nodes.values().cloned().collect();
    let total = all.len();

    let offset = pagination.offset();
    let limit = pagination.limit();
    let items: Vec<Node> = all.into_iter().skip(offset).take(limit).collect();

    tracing::debug!(total, returned = items.len(), "Node list retrieved");
    Json(ListResponse { items, total })
}

/// GET /api/v1/nodes/:id
/// Get node by ID
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Node>>, ApiError> {
    tracing::info!(node_id = %id, "Getting node");
    let nodes = state.nodes.read().await;
    let node = nodes
        .get(&id)
        .ok_or_else(|| {
            tracing::warn!(node_id = %id, "Node not found");
            ApiError::not_found(format!("Node '{id}' not found"))
        })?;

    Ok(Json(ApiResponse { data: node.clone() }))
}

/// GET /api/v1/nodes/:id/agents
/// List agents running on a specific node
pub async fn list_node_agents(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<ListResponse<crate::models::agent::AgentSummary>>, ApiError> {
    tracing::info!(node_id = %node_id, "Listing agents on node");
    // Verify node exists
    {
        let nodes = state.nodes.read().await;
        if !nodes.contains_key(&node_id) {
            tracing::warn!(node_id = %node_id, "Node not found for agent listing");
            return Err(ApiError::not_found(format!("Node '{node_id}' not found")));
        }
    }

    let agents = state.agents.read().await;
    let all: Vec<crate::models::agent::AgentSummary> = agents
        .values()
        .filter(|a| a.node_id.as_deref() == Some(&node_id))
        .map(crate::models::agent::AgentSummary::from)
        .collect();
    let total = all.len();

    let offset = pagination.offset();
    let limit = pagination.limit();
    let items: Vec<_> = all.into_iter().skip(offset).take(limit).collect();

    tracing::debug!(node_id = %node_id, total, returned = items.len(), "Node agents retrieved");
    Ok(Json(ListResponse { items, total }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::node::{Node, NodeStatus, ResourceCapacity};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn test_node(id: &str, name: &str) -> Node {
        Node {
            id: id.to_string(),
            name: name.to_string(),
            status: NodeStatus::Ready,
            resources: ResourceCapacity::default(),
            allocatable: ResourceCapacity::default(),
            labels: HashMap::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_heartbeat: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    async fn test_state_with_nodes(nodes: Vec<Node>) -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let mut node_map = HashMap::new();
        for node in nodes {
            node_map.insert(node.id.clone(), node);
        }

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(node_map)),
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
        }
    }

    #[tokio::test]
    async fn test_list_nodes_empty() {
        let state = test_state_with_nodes(vec![]).await;
        let params = PaginationParams { page: None, per_page: None };
        let result = list_nodes(State(state), Query(params)).await;
        assert_eq!(result.total, 0);
        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_list_nodes_with_data() {
        let state = test_state_with_nodes(vec![
            test_node("n1", "node-1"),
            test_node("n2", "node-2"),
        ])
        .await;
        let params = PaginationParams { page: None, per_page: None };
        let result = list_nodes(State(state), Query(params)).await;
        assert_eq!(result.total, 2);
        assert_eq!(result.items.len(), 2);
    }

    #[tokio::test]
    async fn test_list_nodes_pagination() {
        let state = test_state_with_nodes(vec![
            test_node("n1", "node-1"),
            test_node("n2", "node-2"),
            test_node("n3", "node-3"),
        ])
        .await;
        let params = PaginationParams {
            page: Some(2),
            per_page: Some(1),
        };
        let result = list_nodes(State(state), Query(params)).await;
        assert_eq!(result.total, 3);
        assert_eq!(result.items.len(), 1);
    }

    #[tokio::test]
    async fn test_get_node_found() {
        let state = test_state_with_nodes(vec![test_node("n1", "node-1")]).await;
        let result = get_node(State(state), Path("n1".to_string())).await;
        assert!(result.is_ok());
        let node = result.unwrap();
        assert_eq!(node.data.id, "n1");
        assert_eq!(node.data.name, "node-1");
    }

    #[tokio::test]
    async fn test_get_node_not_found() {
        let state = test_state_with_nodes(vec![]).await;
        let result = get_node(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_node_agents_empty() {
        let state = test_state_with_nodes(vec![test_node("n1", "node-1")]).await;
        let params = PaginationParams { page: None, per_page: None };
        let result = list_node_agents(
            State(state),
            Path("n1".to_string()),
            Query(params),
        )
        .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.total, 0);
        assert!(resp.items.is_empty());
    }

    #[tokio::test]
    async fn test_list_node_agents_node_not_found() {
        let state = test_state_with_nodes(vec![]).await;
        let params = PaginationParams { page: None, per_page: None };
        let result = list_node_agents(
            State(state),
            Path("nonexistent".to_string()),
            Query(params),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_node_agents_with_matching_agents() {
        let state = test_state_with_nodes(vec![test_node("n1", "node-1")]).await;

        // Add agents assigned to node n1
        let agent = crate::models::agent::Agent {
            id: "a1".to_string(),
            spec: crate::models::agent::AgentSpec {
                name: "agent-1".to_string(),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            },
            status: crate::models::agent::AgentStatus::Running,
            node_id: Some("n1".to_string()),
            resource_usage: crate::models::agent::ResourceRequest::default(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            start_time: None,
            restart_count: 0,
        };
        state.agents.write().await.insert("a1".to_string(), agent);

        let params = PaginationParams { page: None, per_page: None };
        let result = list_node_agents(
            State(state),
            Path("n1".to_string()),
            Query(params),
        )
        .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.total, 1);
        assert_eq!(resp.items.len(), 1);
    }

    #[tokio::test]
    async fn test_list_node_agents_filters_by_node() {
        let state = test_state_with_nodes(vec![
            test_node("n1", "node-1"),
            test_node("n2", "node-2"),
        ])
        .await;

        // Add agents to different nodes
        for (id, node_id) in [("a1", "n1"), ("a2", "n2"), ("a3", "n1")] {
            let agent = crate::models::agent::Agent {
                id: id.to_string(),
                spec: crate::models::agent::AgentSpec {
                    name: format!("agent-{id}"),
                    image: "python:3.11".to_string(),
                    command: vec![],
                    resource_request: None,
                    labels: HashMap::new(),
                    priority: "medium".to_string(),
                    env: HashMap::new(),
                },
                status: crate::models::agent::AgentStatus::Running,
                node_id: Some(node_id.to_string()),
                resource_usage: crate::models::agent::ResourceRequest::default(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                start_time: None,
                restart_count: 0,
            };
            state.agents.write().await.insert(id.to_string(), agent);
        }

        let params = PaginationParams { page: None, per_page: None };
        let result = list_node_agents(
            State(state),
            Path("n1".to_string()),
            Query(params),
        )
        .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.total, 2); // a1 and a3 are on n1
    }
}
