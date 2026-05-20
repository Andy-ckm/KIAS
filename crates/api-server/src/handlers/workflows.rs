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

    #[tokio::test]
    async fn test_create_with_multiple_nodes() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "multi-node".to_string(),
            description: "workflow with dependencies".to_string(),
            nodes: vec![
                WorkflowNode {
                    id: "n1".to_string(),
                    name: "fetch".to_string(),
                    node_type: "http".to_string(),
                    config: serde_json::json!({"url": "https://example.com"}),
                    dependencies: vec![],
                },
                WorkflowNode {
                    id: "n2".to_string(),
                    name: "process".to_string(),
                    node_type: "llm".to_string(),
                    config: serde_json::json!({"prompt": "summarize"}),
                    dependencies: vec!["n1".to_string()],
                },
                WorkflowNode {
                    id: "n3".to_string(),
                    name: "save".to_string(),
                    node_type: "shell".to_string(),
                    config: serde_json::json!({"command": "cat > output.txt"}),
                    dependencies: vec!["n2".to_string()],
                },
            ],
        };
        let created = create_workflow(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert_eq!(created.nodes.len(), 3);
        assert_eq!(created.nodes[2].dependencies, vec!["n2"]);
    }

    #[tokio::test]
    async fn test_create_description_only() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "desc-only".to_string(),
            description: "just a description, no nodes".to_string(),
            nodes: vec![],
        };
        let created = create_workflow(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert!(created.nodes.is_empty());
        assert_eq!(created.description, "just a description, no nodes");
    }

    #[tokio::test]
    async fn test_list_multiple_workflows_status_counts() {
        let state = test_state().await;

        // Create 3 workflows
        for name in ["wf-1", "wf-2", "wf-3"] {
            let req = CreateWorkflowRequest {
                name: name.to_string(),
                description: String::new(),
                nodes: vec![],
            };
            let _ = create_workflow(State(state.clone()), Json(req)).await;
        }

        let summary = list_workflows(State(state)).await;
        assert_eq!(summary.total, 3);
        assert_eq!(summary.draft, 3);
        assert_eq!(summary.running, 0);
        assert_eq!(summary.completed, 0);
        assert_eq!(summary.failed, 0);
    }

    #[tokio::test]
    async fn test_delete_one_of_many() {
        let state = test_state().await;

        let mut ids = Vec::new();
        for name in ["keep-1", "delete-me", "keep-2"] {
            let req = CreateWorkflowRequest {
                name: name.to_string(),
                description: String::new(),
                nodes: vec![],
            };
            let wf = create_workflow(State(state.clone()), Json(req))
                .await
                .unwrap();
            ids.push((name, wf.id.clone()));
        }

        // Delete the middle one
        let delete_id = &ids[1].1;
        let _ = delete_workflow(State(state.clone()), Path(delete_id.clone()))
            .await
            .unwrap();

        let summary = list_workflows(State(state)).await;
        assert_eq!(summary.total, 2);
    }

    #[tokio::test]
    async fn test_create_whitespace_name_fails() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "   ".to_string(),
            description: String::new(),
            nodes: vec![],
        };
        let result = create_workflow(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_created_workflow_has_valid_id() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "id-test".to_string(),
            description: String::new(),
            nodes: vec![],
        };
        let wf = create_workflow(State(state), Json(req)).await.unwrap();
        // ID should be a valid UUID
        assert!(uuid::Uuid::parse_str(&wf.id).is_ok());
    }

    #[tokio::test]
    async fn test_created_workflow_timestamps() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "ts-test".to_string(),
            description: String::new(),
            nodes: vec![],
        };
        let wf = create_workflow(State(state), Json(req)).await.unwrap();
        assert!(!wf.created_at.is_empty());
        assert!(!wf.updated_at.is_empty());
        assert!(wf.started_at.is_none());
        assert!(wf.completed_at.is_none());
        assert_eq!(wf.execution_count, 0);
    }

    #[tokio::test]
    async fn test_get_after_delete_fails() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "temp".to_string(),
            description: String::new(),
            nodes: vec![],
        };
        let wf = create_workflow(State(state.clone()), Json(req))
            .await
            .unwrap();
        let _ = delete_workflow(State(state.clone()), Path(wf.id.clone()))
            .await
            .unwrap();
        let result = get_workflow(State(state), Path(wf.id.clone())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_workflow_status_default_is_draft() {
        let status = WorkflowStatus::default();
        assert_eq!(status, WorkflowStatus::Draft);
    }

    // ── Serialization / model tests ──────────────────────────────────

    #[test]
    fn test_workflow_status_serialize_pascal_case() {
        assert_eq!(serde_json::to_string(&WorkflowStatus::Draft).unwrap(), "\"Draft\"");
        assert_eq!(serde_json::to_string(&WorkflowStatus::Running).unwrap(), "\"Running\"");
        assert_eq!(serde_json::to_string(&WorkflowStatus::Completed).unwrap(), "\"Completed\"");
        assert_eq!(serde_json::to_string(&WorkflowStatus::Failed).unwrap(), "\"Failed\"");
        assert_eq!(serde_json::to_string(&WorkflowStatus::Cancelled).unwrap(), "\"Cancelled\"");
    }

    #[test]
    fn test_workflow_status_deserialize_pascal_case() {
        assert_eq!(serde_json::from_str::<WorkflowStatus>("\"Draft\"").unwrap(), WorkflowStatus::Draft);
        assert_eq!(serde_json::from_str::<WorkflowStatus>("\"Running\"").unwrap(), WorkflowStatus::Running);
        assert_eq!(serde_json::from_str::<WorkflowStatus>("\"Completed\"").unwrap(), WorkflowStatus::Completed);
        assert_eq!(serde_json::from_str::<WorkflowStatus>("\"Failed\"").unwrap(), WorkflowStatus::Failed);
        assert_eq!(serde_json::from_str::<WorkflowStatus>("\"Cancelled\"").unwrap(), WorkflowStatus::Cancelled);
    }

    #[test]
    fn test_workflow_status_invalid_variant_fails() {
        assert!(serde_json::from_str::<WorkflowStatus>("\"Pending\"").is_err());
        assert!(serde_json::from_str::<WorkflowStatus>("\"unknown\"").is_err());
    }

    #[test]
    fn test_workflow_node_roundtrip() {
        let node = WorkflowNode {
            id: "n1".to_string(),
            name: "fetch".to_string(),
            node_type: "http".to_string(),
            config: serde_json::json!({"url": "https://example.com", "method": "GET"}),
            dependencies: vec!["n0".to_string()],
        };
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: WorkflowNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "n1");
        assert_eq!(deserialized.name, "fetch");
        assert_eq!(deserialized.node_type, "http");
        assert_eq!(deserialized.dependencies, vec!["n0"]);
    }

    #[test]
    fn test_workflow_node_empty_dependencies() {
        let node = WorkflowNode {
            id: "root".to_string(),
            name: "start".to_string(),
            node_type: "shell".to_string(),
            config: serde_json::json!({"command": "echo ok"}),
            dependencies: vec![],
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"dependencies\":[]"));
    }

    #[test]
    fn test_workflow_roundtrip() {
        let wf = Workflow {
            id: "wf-123".to_string(),
            name: "test".to_string(),
            description: "desc".to_string(),
            status: WorkflowStatus::Running,
            nodes: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            started_at: Some("2026-01-01T00:01:00Z".to_string()),
            completed_at: None,
            execution_count: 5,
        };
        let json = serde_json::to_string(&wf).unwrap();
        let deserialized: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "wf-123");
        assert_eq!(deserialized.status, WorkflowStatus::Running);
        assert_eq!(deserialized.execution_count, 5);
        assert!(deserialized.started_at.is_some());
        assert!(deserialized.completed_at.is_none());
    }

    #[test]
    fn test_create_workflow_request_defaults() {
        let json_str = r#"{"name":"minimal"}"#;
        let req: CreateWorkflowRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.name, "minimal");
        assert!(req.description.is_empty());
        assert!(req.nodes.is_empty());
    }

    #[test]
    fn test_create_workflow_request_with_nodes() {
        let json_str = r#"{
            "name":"pipeline",
            "description":"data pipeline",
            "nodes":[{"id":"a","name":"step","node_type":"shell","config":{},"dependencies":[]}]
        }"#;
        let req: CreateWorkflowRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.name, "pipeline");
        assert_eq!(req.nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_list_workflows_mixed_statuses() {
        let state = test_state().await;

        // Create 4 workflows and manually change their statuses
        let mut ids = Vec::new();
        for name in ["draft-wf", "running-wf", "completed-wf", "failed-wf"] {
            let req = CreateWorkflowRequest {
                name: name.to_string(),
                description: String::new(),
                nodes: vec![],
            };
            let wf = create_workflow(State(state.clone()), Json(req)).await.unwrap();
            ids.push(wf.id.clone());
        }

        // Manually change statuses via workflows map
        {
            let mut wfs = state.workflows.write().await;
            if let Some(wf) = wfs.get_mut(&ids[1]) {
                wf.status = WorkflowStatus::Running;
            }
            if let Some(wf) = wfs.get_mut(&ids[2]) {
                wf.status = WorkflowStatus::Completed;
            }
            if let Some(wf) = wfs.get_mut(&ids[3]) {
                wf.status = WorkflowStatus::Failed;
            }
        }

        let summary = list_workflows(State(state)).await;
        assert_eq!(summary.total, 4);
        assert_eq!(summary.draft, 1);
        assert_eq!(summary.running, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.failed, 1);
    }

    #[tokio::test]
    async fn test_delete_same_workflow_twice_fails() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "once".to_string(),
            description: String::new(),
            nodes: vec![],
        };
        let wf = create_workflow(State(state.clone()), Json(req)).await.unwrap();
        let _ = delete_workflow(State(state.clone()), Path(wf.id.clone())).await.unwrap();
        let result = delete_workflow(State(state.clone()), Path(wf.id.clone())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_workflow_name_too_long() {
        let state = test_state().await;
        let long_name = "x".repeat(300);
        let req = CreateWorkflowRequest {
            name: long_name,
            description: String::new(),
            nodes: vec![],
        };
        let result = create_workflow(State(state), Json(req)).await;
        // Should succeed — no length limit enforced on workflow names
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_workflow_special_characters_name() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "workflow-测试_2026.special@v2".to_string(),
            description: "special chars".to_string(),
            nodes: vec![],
        };
        let wf = create_workflow(State(state.clone()), Json(req)).await.unwrap();
        assert_eq!(wf.name, "workflow-测试_2026.special@v2");
    }

    #[tokio::test]
    async fn test_list_workflows_order_preserved() {
        let state = test_state().await;
        for name in ["alpha", "beta", "gamma"] {
            let req = CreateWorkflowRequest {
                name: name.to_string(),
                description: String::new(),
                nodes: vec![],
            };
            let _ = create_workflow(State(state.clone()), Json(req)).await;
        }
        let summary = list_workflows(State(state)).await;
        assert_eq!(summary.workflows.len(), 3);
        // All created workflows should be present
        let names: Vec<&str> = summary.workflows.iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
        assert!(names.contains(&"gamma"));
    }

    #[tokio::test]
    async fn test_create_workflow_with_subworkflow_node_type() {
        let state = test_state().await;
        let req = CreateWorkflowRequest {
            name: "composite".to_string(),
            description: "nested workflow".to_string(),
            nodes: vec![WorkflowNode {
                id: "sw1".to_string(),
                name: "sub".to_string(),
                node_type: "subworkflow".to_string(),
                config: serde_json::json!({"workflow_id": "other-wf-id"}),
                dependencies: vec![],
            }],
        };
        let wf = create_workflow(State(state.clone()), Json(req)).await.unwrap();
        assert_eq!(wf.nodes[0].node_type, "subworkflow");
    }

    #[test]
    fn test_workflow_summary_serialize() {
        let summary = WorkflowSummary {
            total: 5,
            running: 2,
            completed: 1,
            failed: 1,
            draft: 1,
            workflows: vec![],
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"total\":5"));
        assert!(json.contains("\"running\":2"));
    }

    #[test]
    fn test_workflow_execution_serialize() {
        let exec = WorkflowExecution {
            workflow_id: "wf-1".to_string(),
            workflow_name: "test".to_string(),
            status: WorkflowStatus::Completed,
            nodes_total: 3,
            nodes_completed: 3,
            nodes_failed: 0,
            duration_ms: Some(1500),
        };
        let json = serde_json::to_string(&exec).unwrap();
        assert!(json.contains("\"nodes_total\":3"));
        assert!(json.contains("\"duration_ms\":1500"));
    }
}
