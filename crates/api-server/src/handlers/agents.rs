use axum::extract::{Path, Query, State};
use axum::Json;
use validator::Validate;

use crate::error::ApiError;
use crate::models::agent::{Agent, AgentSpec, AgentStatus, AgentSummary};
use crate::models::request::{ActionResponse, ApiResponse, ListResponse, PaginationParams};
use crate::AppState;

/// POST /api/v1/agents
/// Create a new agent
pub async fn create_agent(
    State(state): State<AppState>,
    Json(spec): Json<AgentSpec>,
) -> Result<(axum::http::StatusCode, Json<ApiResponse<Agent>>), ApiError> {
    // Validate input
    spec.validate()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    // Check for duplicate name
    {
        let agents = state.agents.read().await;
        if agents.values().any(|a| a.spec.name == spec.name) {
            return Err(ApiError::conflict(format!(
                "Agent '{}' already exists",
                spec.name
            )));
        }
    }

    tracing::info!(name = %spec.name, image = %spec.image, "Creating agent");

    let agent = Agent::from_spec(spec);
    let agent_clone = agent.clone();

    // Store agent
    let mut agents = state.agents.write().await;
    agents.insert(agent.id.clone(), agent);

    tracing::info!(id = %agent_clone.id, name = %agent_clone.spec.name, "Agent created");

    // Publish WebSocket event + buffer for replay
    let event = crate::websocket::WsEvent {
        event_type: crate::websocket::EventType::AgentCreated,
        data: serde_json::json!({
            "agent_id": agent_clone.id,
            "name": agent_clone.spec.name,
        }),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse { data: agent_clone }),
    ))
}

/// GET /api/v1/agents
/// List all agents
pub async fn list_agents(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationParams>,
) -> Json<ListResponse<AgentSummary>> {
    let agents = state.agents.read().await;
    let all: Vec<AgentSummary> = agents.values().map(AgentSummary::from).collect();
    let total = all.len();

    let offset = pagination.offset();
    let limit = pagination.limit();
    let items: Vec<AgentSummary> = all.into_iter().skip(offset).take(limit).collect();

    Json(ListResponse { items, total })
}

/// GET /api/v1/agents/:id
/// Get agent by ID
pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Agent>>, ApiError> {
    let agents = state.agents.read().await;
    let agent = agents
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?;

    Ok(Json(ApiResponse {
        data: agent.clone(),
    }))
}

/// DELETE /api/v1/agents/:id
/// Delete an agent
pub async fn delete_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, ApiError> {
    let mut agents = state.agents.write().await;
    let agent = agents
        .remove(&id)
        .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?;

    tracing::info!(id = %id, name = %agent.spec.name, "Agent deleted");

    // Publish WebSocket event + buffer for replay
    let event = crate::websocket::WsEvent {
        event_type: crate::websocket::EventType::AgentDeleted,
        data: serde_json::json!({ "agent_id": id }),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;

    Ok(Json(ActionResponse {
        message: format!("Agent '{}' deleted successfully", agent.spec.name),
    }))
}

/// PATCH /api/v1/agents/:id/status
/// Update agent status (internal use by controller)
pub async fn update_agent_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(new_status): Json<AgentStatus>,
) -> Result<Json<ApiResponse<Agent>>, ApiError> {
    let mut agents = state.agents.write().await;
    let agent = agents
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?;

    let old_status = format!("{:?}", agent.status);
    let old_status_clone = old_status.clone();
    agent.status = new_status;
    agent.updated_at = chrono::Utc::now().to_rfc3339();
    let new_status_str = format!("{:?}", agent.status);

    tracing::info!(id = %id, status = ?agent.status, "Agent status updated");

    let agent_clone = agent.clone();

    // Publish WebSocket event + buffer for replay
    let event = crate::websocket::WsEvent {
        event_type: crate::websocket::EventType::AgentStatusChanged,
        data: serde_json::json!({
            "agent_id": id,
            "old_status": old_status_clone,
            "new_status": new_status_str,
        }),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;

    Ok(Json(ApiResponse { data: agent_clone }))
}
