use axum::extract::{Path, Query, State};
use axum::Json;
use kias_data_store::Repository;
use validator::Validate;

use crate::agent_persistence;
use crate::error::ApiError;
use crate::models::agent::{Agent, AgentSpec, AgentStatus, AgentSummary};
use crate::models::request::{ActionResponse, ApiResponse, ListResponse, PaginationParams};
use crate::AppState;

/// POST /api/v1/agents
pub async fn create_agent(
    State(state): State<AppState>,
    Json(spec): Json<AgentSpec>,
) -> Result<(axum::http::StatusCode, Json<ApiResponse<Agent>>), ApiError> {
    spec.validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    {
        let agents = state.agents.read().await;
        if agents.values().any(|agent| agent.spec.name == spec.name) {
            return Err(ApiError::conflict(format!(
                "Agent '{}' already exists",
                spec.name
            )));
        }
    }

    let agent = Agent::from_spec(spec);
    if let Some(repository) = &state.agent_repository {
        let row = agent_persistence::to_row(&agent)?;
        repository.create(&row).await?;
    }

    {
        let mut agents = state.agents.write().await;
        agents.insert(agent.id.clone(), agent.clone());
    }

    tracing::info!(id = %agent.id, name = %agent.spec.name, "Durable Agent created");
    publish_event(
        &state,
        crate::websocket::EventType::AgentCreated,
        serde_json::json!({
            "agent_id": agent.id,
            "name": agent.spec.name,
        }),
    )
    .await;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse { data: agent }),
    ))
}

/// GET /api/v1/agents
pub async fn list_agents(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationParams>,
) -> Json<ListResponse<AgentSummary>> {
    let agents = state.agents.read().await;
    let mut all: Vec<AgentSummary> = agents.values().map(AgentSummary::from).collect();
    all.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    let total = all.len();
    let items = all
        .into_iter()
        .skip(pagination.offset())
        .take(pagination.limit())
        .collect();
    Json(ListResponse { items, total })
}

/// GET /api/v1/agents/:id
pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Agent>>, ApiError> {
    let agents = state.agents.read().await;
    let agent = agents
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?;
    Ok(Json(ApiResponse { data: agent }))
}

/// DELETE /api/v1/agents/:id
pub async fn delete_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, ApiError> {
    let agent = {
        let agents = state.agents.read().await;
        agents
            .get(&id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?
    };

    if let Some(repository) = &state.agent_repository {
        repository.delete(&id).await?;
    }

    {
        let mut agents = state.agents.write().await;
        agents.remove(&id);
    }

    tracing::info!(id = %id, name = %agent.spec.name, "Durable Agent deleted");
    publish_event(
        &state,
        crate::websocket::EventType::AgentDeleted,
        serde_json::json!({ "agent_id": id }),
    )
    .await;

    Ok(Json(ActionResponse {
        message: format!("Agent '{}' deleted successfully", agent.spec.name),
    }))
}

/// Controller-owned status transition helper.
pub async fn update_agent_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(new_status): Json<AgentStatus>,
) -> Result<Json<ApiResponse<Agent>>, ApiError> {
    let mut updated = {
        let agents = state.agents.read().await;
        agents
            .get(&id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?
    };
    let old_status = format!("{:?}", updated.status);
    updated.status = new_status;
    updated.updated_at = chrono::Utc::now().to_rfc3339();

    if let Some(repository) = &state.agent_repository {
        let row = agent_persistence::to_row(&updated)?;
        repository.update(&row).await?;
    }

    {
        let mut agents = state.agents.write().await;
        agents.insert(id.clone(), updated.clone());
    }

    publish_event(
        &state,
        crate::websocket::EventType::AgentStatusChanged,
        serde_json::json!({
            "agent_id": id,
            "old_status": old_status,
            "new_status": format!("{:?}", updated.status),
        }),
    )
    .await;

    Ok(Json(ApiResponse { data: updated }))
}

async fn publish_event(
    state: &AppState,
    event_type: crate::websocket::EventType,
    data: serde_json::Value,
) {
    let event = crate::websocket::WsEvent {
        event_type,
        data,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn spec(name: &str) -> AgentSpec {
        AgentSpec {
            name: name.to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string(), "worker.py".to_string()],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn durable_handlers_survive_state_rehydration() {
        let database = kias_data_store::SqliteRepository::in_memory().await.unwrap();
        let repository = Arc::new(kias_data_store::AgentRepository::new(database.pool.clone()));
        let state = AppState::new(kias_common::config::KiasConfig::default())
            .await
            .with_agent_repository(repository.clone())
            .await
            .unwrap();

        let (_, created) = create_agent(State(state), Json(spec("persistent")))
            .await
            .unwrap();
        let created_id = created.data.id;

        let rehydrated = AppState::new(kias_common::config::KiasConfig::default())
            .await
            .with_agent_repository(repository)
            .await
            .unwrap();
        let restored = get_agent(State(rehydrated), Path(created_id))
            .await
            .unwrap();
        assert_eq!(restored.data.spec.name, "persistent");
    }
}
