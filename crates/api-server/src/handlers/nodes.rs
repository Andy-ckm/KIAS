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
    let nodes = state.nodes.read().await;
    let all: Vec<Node> = nodes.values().cloned().collect();
    let total = all.len();

    let offset = pagination.offset();
    let limit = pagination.limit();
    let items: Vec<Node> = all.into_iter().skip(offset).take(limit).collect();

    Json(ListResponse { items, total })
}

/// GET /api/v1/nodes/:id
/// Get node by ID
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Node>>, ApiError> {
    let nodes = state.nodes.read().await;
    let node = nodes
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("Node '{id}' not found")))?;

    Ok(Json(ApiResponse { data: node.clone() }))
}

/// GET /api/v1/nodes/:id/agents
/// List agents running on a specific node
pub async fn list_node_agents(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<ListResponse<crate::models::agent::AgentSummary>>, ApiError> {
    // Verify node exists
    {
        let nodes = state.nodes.read().await;
        if !nodes.contains_key(&node_id) {
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
    let items = all.into_iter().skip(offset).take(limit).collect();

    Ok(Json(ListResponse { items, total }))
}
