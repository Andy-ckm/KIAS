use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::AppState;

/// Workflow status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub enum WorkflowStatus {
    #[default]
    Draft,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A node in the workflow DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: String, // "shell", "http", "llm", "subworkflow"
    pub config: serde_json::Value,
    pub dependencies: Vec<String>, // IDs of nodes this depends on
}

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: WorkflowStatus,
    pub nodes: Vec<WorkflowNode>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub execution_count: u32,
}

/// Request to create a new workflow
#[derive(Debug, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
}

/// Workflow execution summary
#[derive(Debug, Serialize)]
pub struct WorkflowExecution {
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub nodes_total: usize,
    pub nodes_completed: usize,
    pub nodes_failed: usize,
    pub duration_ms: Option<u64>,
}

/// Workflow summary for list view
#[derive(Debug, Serialize)]
pub struct WorkflowSummary {
    pub total: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub draft: usize,
    pub workflows: Vec<Workflow>,
}

/// GET /api/v1/workflows
/// List all workflows with summary statistics.
pub async fn list_workflows(State(state): State<AppState>) -> Json<WorkflowSummary> {
    let workflows = state.workflows.read().await;

    let running = workflows
        .values()
        .filter(|w| w.status == WorkflowStatus::Running)
        .count();
    let completed = workflows
        .values()
        .filter(|w| w.status == WorkflowStatus::Completed)
        .count();
    let failed = workflows
        .values()
        .filter(|w| w.status == WorkflowStatus::Failed)
        .count();
    let draft = workflows
        .values()
        .filter(|w| w.status == WorkflowStatus::Draft)
        .count();

    let mut workflow_list: Vec<Workflow> = workflows.values().cloned().collect();
    workflow_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Json(WorkflowSummary {
        total: workflows.len(),
        running,
        completed,
        failed,
        draft,
        workflows: workflow_list,
    })
}

/// POST /api/v1/workflows
/// Create a new workflow definition.
pub async fn create_workflow(
    State(state): State<AppState>,
    Json(req): Json<CreateWorkflowRequest>,
) -> Result<Json<Workflow>, ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("Workflow name cannot be empty"));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();

    let workflow = Workflow {
        id: id.clone(),
        name: req.name,
        description: req.description,
        status: WorkflowStatus::Draft,
        nodes: req.nodes,
        created_at: now.clone(),
        updated_at: now,
        started_at: None,
        completed_at: None,
        execution_count: 0,
    };

    let mut workflows = state.workflows.write().await;
    workflows.insert(id.clone(), workflow.clone());

    // Publish WebSocket event + buffer for replay
    let event = crate::websocket::WsEvent {
        event_type: crate::websocket::EventType::WorkflowUpdate,
        data: serde_json::json!({
            "workflow_id": id,
            "workflow_name": workflow.name,
            "status": "Draft",
            "action": "created",
        }),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;

    Ok(Json(workflow))
}

/// GET /api/v1/workflows/:id
/// Get a specific workflow by ID.
pub async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Workflow>, ApiError> {
    let workflows = state.workflows.read().await;
    let workflow = workflows
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("Workflow '{id}' not found")))?;

    Ok(Json(workflow.clone()))
}

/// DELETE /api/v1/workflows/:id
/// Delete a workflow by ID.
pub async fn delete_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut workflows = state.workflows.write().await;
    let removed = workflows.remove(&id);
    if removed.is_none() {
        return Err(ApiError::not_found(format!("Workflow '{id}' not found")));
    }

    let workflow_name = removed.map(|w| w.name).unwrap_or_default();

    // Publish WebSocket event + buffer for replay
    let event = crate::websocket::WsEvent {
        event_type: crate::websocket::EventType::WorkflowUpdate,
        data: serde_json::json!({
            "workflow_id": id,
            "workflow_name": workflow_name,
            "status": "Deleted",
            "action": "deleted",
        }),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    state.event_bus.publish(event.clone());
    state.event_replay_buffer.push(event).await;

    Ok(Json(serde_json::json!({
        "message": format!("Workflow '{}' deleted", id)
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        AppState::new_async(config).await
    }

    #[tokio::test]
    async fn test_list_workflows_empty() {
        let state = test_state().await;
        let result = list_workflows(State(state)).await;
        assert_eq!(result.total, 0);
        assert!(result.workflows.is_empty());
    }

    #[tokio::test]
    async fn test_create_and_get_workflow() {
        let state = test_state().await;

        let req = CreateWorkflowRequest {
            name: "test-workflow".to_string(),
            description: "A test workflow".to_string(),
            nodes: vec![WorkflowNode {
                id: "node-1".to_string(),
                name: "start".to_string(),
                node_type: "shell".to_string(),
                config: serde_json::json!({"command": "echo hello"}),
                dependencies: vec![],
            }],
        };

        let created = create_workflow(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert_eq!(created.name, "test-workflow");
        assert_eq!(created.status, WorkflowStatus::Draft);
        assert_eq!(created.nodes.len(), 1);

        let fetched = get_workflow(State(state.clone()), Path(created.id.clone()))
            .await
            .unwrap();
        assert_eq!(fetched.id, created.id);

        let summary = list_workflows(State(state.clone())).await;
        assert_eq!(summary.total, 1);
        assert_eq!(summary.draft, 1);
    }

    #[tokio::test]
    async fn test_delete_workflow() {
        let state = test_state().await;

        let req = CreateWorkflowRequest {
            name: "to-delete".to_string(),
            description: "".to_string(),
            nodes: vec![],
        };

        let created = create_workflow(State(state.clone()), Json(req))
            .await
            .unwrap();
        let _ = delete_workflow(State(state.clone()), Path(created.id.clone()))
            .await
            .unwrap();

        let summary = list_workflows(State(state)).await;
        assert_eq!(summary.total, 0);
    }

    #[tokio::test]
    async fn test_get_nonexistent_workflow() {
        let state = test_state().await;
        let result = get_workflow(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_empty_name_fails() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "".to_string(),
            description: "".to_string(),
            nodes: vec![],
        };
        let result = create_workflow(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_fails() {
        let state = test_state().await;
        let result = delete_workflow(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }
}
