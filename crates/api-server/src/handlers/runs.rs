use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::error::ApiError;
use crate::models::run::{ReplayRunRequest, StartRunRequest};
use crate::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct RunListQuery {
    pub agent_id: Option<String>,
    pub status: Option<String>,
}

fn run_service(
    state: &AppState,
) -> Result<std::sync::Arc<crate::run_service::RunService>, ApiError> {
    state.run_service.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Agent Run service is not configured",
        )
    })
}

pub async fn create_run(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<StartRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state
        .agents
        .read()
        .await
        .get(&agent_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("Agent {agent_id} not found")))?;
    let run = run_service(&state)?.create_run(&agent, request).await?;
    Ok((StatusCode::ACCEPTED, Json(run)))
}

pub async fn list_runs(
    State(state): State<AppState>,
    Query(query): Query<RunListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut runs = run_service(&state)?.list_runs().await?;
    if let Some(agent_id) = query.agent_id {
        runs.retain(|run| run.agent_id == agent_id);
    }
    if let Some(status) = query.status {
        runs.retain(|run| run.status.as_storage() == status);
    }
    Ok(Json(serde_json::json!({
        "total": runs.len(),
        "data": runs,
    })))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::run::RunRecord>, ApiError> {
    Ok(Json(run_service(&state)?.get_run(&id).await?))
}

pub async fn get_run_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::run::RunLogs>, ApiError> {
    Ok(Json(run_service(&state)?.get_logs(&id).await?))
}

pub async fn get_run_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::run::RunEvidence>, ApiError> {
    Ok(Json(run_service(&state)?.get_evidence(&id).await?))
}

pub async fn get_run_checkpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::run::RunCheckpoint>, ApiError> {
    Ok(Json(run_service(&state)?.checkpoint(&id).await?))
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::run::RunRecord>, ApiError> {
    Ok(Json(run_service(&state)?.cancel(&id).await?))
}

pub async fn retry_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ReplayRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let run = run_service(&state)?.retry(&id, request).await?;
    Ok((StatusCode::ACCEPTED, Json(run)))
}

pub async fn recover_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ReplayRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let run = run_service(&state)?.recover(&id, request).await?;
    Ok((StatusCode::ACCEPTED, Json(run)))
}
