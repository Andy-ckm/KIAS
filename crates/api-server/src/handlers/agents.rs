use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::Utc;
use std::time::Instant;
use uuid::Uuid;
use validator::Validate;

use kias_executor::{ShellExecutor, Task, TaskRuntime, TaskStatus as ExecutorTaskStatus};

use crate::error::ApiError;
use crate::models::agent::{Agent, AgentSpec, AgentStatus, AgentSummary};
use crate::models::request::{ActionResponse, ApiResponse, ListResponse, PaginationParams};
use crate::AppState;

/// Request body for agent invocation
#[derive(Debug, serde::Deserialize)]
pub struct InvokeRequest {
    /// The prompt to send to the agent
    pub prompt: String,
    /// Optional timeout override in seconds (default: 300)
    pub timeout_secs: Option<u64>,
}

/// Response body for agent invocation
#[derive(Debug, serde::Serialize)]
pub struct InvokeResponse {
    pub run_id: String,
    pub agent_id: String,
    pub output: String,
    pub tokens_used: Option<u64>,
    pub cost: Option<f64>,
    pub duration_ms: u64,
}

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

/// POST /api/v1/agents/:id/invoke
/// Invoke an agent with a prompt (CI-friendly, non-interactive execution).
///
/// Returns the agent's output along with metadata about the run.
pub async fn invoke_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<InvokeRequest>,
) -> Result<Json<ApiResponse<InvokeResponse>>, ApiError> {
    // Verify agent exists
    let agent = {
        let agents = state.agents.read().await;
        agents
            .get(&id)
            .ok_or_else(|| ApiError::not_found(format!("Agent '{id}' not found")))?
            .clone()
    };

    let run_id = Uuid::new_v4().to_string();
    let timeout = req.timeout_secs.unwrap_or(300);
    let timeout_dur = std::time::Duration::from_secs(timeout);
    let start = Instant::now();

    tracing::info!(
        agent_id = %id,
        run_id = %run_id,
        prompt_len = req.prompt.len(),
        timeout = timeout,
        "Invoking agent"
    );

    // Build and execute task using the shell executor (CI mode: runs agent command)
    let task = Task {
        id: run_id.clone(),
        name: format!("invoke-{}", agent.spec.name),
        agent_id: id.clone(),
        payload: serde_json::json!({
            "prompt": req.prompt,
            "image": agent.spec.image,
            "command": agent.spec.command,
        }),
        created_at: Utc::now(),
        timeout: Some(timeout_dur),
    };

    let shell_executor = ShellExecutor::new(timeout_dur);
    let runtime = TaskRuntime::new(Box::new(shell_executor));
    let result = runtime.run_task(&task).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    let output = match result {
        Ok(task_result) => {
            let status = if task_result.status == ExecutorTaskStatus::Completed {
                "success"
            } else {
                "failed"
            };

            let output_text = task_result
                .output
                .and_then(|v| {
                    v.get("stdout")
                        .and_then(|s| s.as_str().map(|ss| ss.to_string()))
                })
                .unwrap_or_default();

            let error_text = task_result.error.unwrap_or_default();

            let final_output =
                if task_result.status == ExecutorTaskStatus::Failed && !error_text.is_empty() {
                    format!("{}: {}", status, error_text)
                } else {
                    output_text
                };

            tracing::info!(
                run_id = %run_id,
                status = %status,
                duration_ms = duration_ms,
                "Agent invocation complete"
            );

            InvokeResponse {
                run_id,
                agent_id: id,
                output: final_output,
                tokens_used: None,
                cost: None,
                duration_ms,
            }
        }
        Err(e) => {
            tracing::error!(run_id = %run_id, error = %e, "Agent invocation failed");
            return Err(ApiError::internal(format!("Execution error: {e}")));
        }
    };

    Ok(Json(ApiResponse { data: output }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoke_request_deserialize() {
        let json = r#"{"prompt": "Hello world", "timeout_secs": 60}"#;
        let req: InvokeRequest = serde_json::from_str(json).expect("should parse");
        assert_eq!(req.prompt, "Hello world");
        assert_eq!(req.timeout_secs, Some(60));
    }

    #[test]
    fn test_invoke_request_deserialize_without_timeout() {
        let json = r#"{"prompt": "Hello world"}"#;
        let req: InvokeRequest = serde_json::from_str(json).expect("should parse");
        assert_eq!(req.prompt, "Hello world");
        assert!(req.timeout_secs.is_none());
    }

    #[test]
    fn test_invoke_response_serialize() {
        let resp = InvokeResponse {
            run_id: "run-1".to_string(),
            agent_id: "agent-1".to_string(),
            output: "Hello!".to_string(),
            tokens_used: Some(150),
            cost: Some(0.003),
            duration_ms: 1200,
        };
        let json = serde_json::to_string(&resp).expect("should serialize");
        assert!(json.contains("run-1"));
        assert!(json.contains("Hello!"));
    }
}

#[cfg(test)]
mod handler_tests {
    use super::*;
    use crate::models::agent::ResourceRequest;
    use crate::AppState;
    use axum::extract::{Path, Query, State};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("knowledge retriever init with local embedding engine");

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

    fn test_spec(name: &str) -> AgentSpec {
        AgentSpec {
            name: name.to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_agent() {
        let state = test_state().await;
        let spec = test_spec("test-agent");
        let result = create_agent(State(state.clone()), Json(spec)).await;
        assert!(result.is_ok());
        let (status, json) = result.unwrap();
        assert_eq!(status, axum::http::StatusCode::CREATED);
        let agent = &json.data;
        assert_eq!(agent.spec.name, "test-agent");

        // Get by ID
        let result = get_agent(State(state.clone()), Path(agent.id.clone())).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data.spec.name, "test-agent");
    }

    #[tokio::test]
    async fn test_create_duplicate_agent_fails() {
        let state = test_state().await;
        let _ = create_agent(State(state.clone()), Json(test_spec("dup")))
            .await
            .unwrap();
        let result = create_agent(State(state.clone()), Json(test_spec("dup"))).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_nonexistent_agent_fails() {
        let state = test_state().await;
        let result = get_agent(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_agents_empty() {
        let state = test_state().await;
        let pagination = PaginationParams {
            page: Some(1),
            per_page: Some(10),
        };
        let result = list_agents(State(state), Query(pagination)).await;
        assert_eq!(result.total, 0);
        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_list_agents_with_items() {
        let state = test_state().await;
        let _ = create_agent(State(state.clone()), Json(test_spec("a1")))
            .await
            .unwrap();
        let _ = create_agent(State(state.clone()), Json(test_spec("a2")))
            .await
            .unwrap();
        let pagination = PaginationParams {
            page: Some(1),
            per_page: Some(10),
        };
        let result = list_agents(State(state), Query(pagination)).await;
        assert_eq!(result.total, 2);
        assert_eq!(result.items.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_agent() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("to-delete")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        let result = delete_agent(State(state.clone()), Path(id.clone())).await;
        assert!(result.is_ok());
        // Verify deleted
        let result = get_agent(State(state), Path(id)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_agent_fails() {
        let state = test_state().await;
        let result = delete_agent(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_agent_status() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("status-test")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        let result =
            update_agent_status(State(state.clone()), Path(id), Json(AgentStatus::Running)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data.status, AgentStatus::Running);
    }

    #[tokio::test]
    async fn test_update_status_nonexistent_agent_fails() {
        let state = test_state().await;
        let result = update_agent_status(
            State(state),
            Path("nonexistent".to_string()),
            Json(AgentStatus::Running),
        )
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_agents_pagination() {
        let state = test_state().await;
        // Create 5 agents
        for i in 0..5 {
            let _ = create_agent(
                State(state.clone()),
                Json(test_spec(&format!("p-agent-{i}"))),
            )
            .await
            .unwrap();
        }
        // Page 1: 2 items
        let pagination = PaginationParams {
            page: Some(1),
            per_page: Some(2),
        };
        let result = list_agents(State(state.clone()), Query(pagination)).await;
        assert_eq!(result.total, 5);
        assert_eq!(result.items.len(), 2);

        // Page 2: 2 items
        let pagination = PaginationParams {
            page: Some(2),
            per_page: Some(2),
        };
        let result = list_agents(State(state.clone()), Query(pagination)).await;
        assert_eq!(result.total, 5);
        assert_eq!(result.items.len(), 2);

        // Page 3: 1 item (remaining)
        let pagination = PaginationParams {
            page: Some(3),
            per_page: Some(2),
        };
        let result = list_agents(State(state), Query(pagination)).await;
        assert_eq!(result.total, 5);
        assert_eq!(result.items.len(), 1);
    }

    #[tokio::test]
    async fn test_create_agent_empty_name_fails() {
        let state = test_state().await;
        let spec = test_spec("");
        let result = create_agent(State(state), Json(spec)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_agent_summary_fields_in_list() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("summary-test")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        // Update status to Running so we can verify it in the summary
        let _ = update_agent_status(
            State(state.clone()),
            Path(id.clone()),
            Json(AgentStatus::Running),
        )
        .await;

        let pagination = PaginationParams {
            page: Some(1),
            per_page: Some(10),
        };
        let result = list_agents(State(state), Query(pagination)).await;
        assert_eq!(result.items.len(), 1);
        let summary = &result.items[0];
        assert_eq!(summary.id, id);
        assert_eq!(summary.name, "summary-test");
        assert_eq!(summary.status, AgentStatus::Running);
    }

    #[tokio::test]
    async fn test_create_agent_returns_201() {
        let state = test_state().await;
        let (status, _) = create_agent(State(state), Json(test_spec("created")))
            .await
            .unwrap();
        assert_eq!(status, axum::http::StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_delete_agent_returns_success_message() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("msg-test")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        let result = delete_agent(State(state), Path(id)).await.unwrap();
        assert!(result.message.contains("msg-test"));
        assert!(result.message.contains("deleted successfully"));
    }

    #[tokio::test]
    async fn test_invoke_nonexistent_agent_fails() {
        let state = test_state().await;
        let req = InvokeRequest {
            prompt: "hello".to_string(),
            timeout_secs: Some(10),
        };
        let result = invoke_agent(State(state), Path("nonexistent".to_string()), Json(req)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_agent_preserves_labels_and_env() {
        let state = test_state().await;
        let mut spec = test_spec("label-env-test");
        spec.labels.insert("team".to_string(), "infra".to_string());
        spec.labels.insert("env".to_string(), "prod".to_string());
        spec.env
            .insert("API_KEY".to_string(), "secret123".to_string());
        let (_, json) = create_agent(State(state.clone()), Json(spec))
            .await
            .unwrap();
        let agent = &json.data;
        assert_eq!(agent.spec.labels.get("team").unwrap(), "infra");
        assert_eq!(agent.spec.labels.get("env").unwrap(), "prod");
        assert_eq!(agent.spec.env.get("API_KEY").unwrap(), "secret123");
    }

    #[tokio::test]
    async fn test_create_agent_with_resource_request() {
        let state = test_state().await;
        let mut spec = test_spec("res-test");
        spec.resource_request = Some(ResourceRequest {
            cpu: Some("2".to_string()),
            memory: Some("4Gi".to_string()),
            gpu: None,
        });
        let (_, json) = create_agent(State(state), Json(spec)).await.unwrap();
        let agent = json.data.clone(); // clone to move out of Json wrapper
        let rr = agent.spec.resource_request.unwrap();
        assert_eq!(rr.cpu.unwrap(), "2");
        assert_eq!(rr.memory.unwrap(), "4Gi");
        assert!(rr.gpu.is_none());
    }

    #[tokio::test]
    async fn test_list_agents_page_beyond_total_returns_empty() {
        let state = test_state().await;
        let _ = create_agent(State(state.clone()), Json(test_spec("only-one")))
            .await
            .unwrap();
        let pagination = PaginationParams {
            page: Some(100),
            per_page: Some(10),
        };
        let result = list_agents(State(state), Query(pagination)).await;
        assert_eq!(result.total, 1);
        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_update_agent_status_transitions() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("transitions")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        // Pending → Running
        let result = update_agent_status(
            State(state.clone()),
            Path(id.clone()),
            Json(AgentStatus::Running),
        )
        .await
        .unwrap();
        assert_eq!(result.data.status, AgentStatus::Running);
        // Running → Succeeded
        let result = update_agent_status(
            State(state.clone()),
            Path(id.clone()),
            Json(AgentStatus::Succeeded),
        )
        .await
        .unwrap();
        assert_eq!(result.data.status, AgentStatus::Succeeded);
        // Succeeded → Failed
        let result = update_agent_status(
            State(state.clone()),
            Path(id.clone()),
            Json(AgentStatus::Failed),
        )
        .await
        .unwrap();
        assert_eq!(result.data.status, AgentStatus::Failed);
    }

    #[tokio::test]
    async fn test_create_agent_default_status_is_pending() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state), Json(test_spec("pending-check")))
            .await
            .unwrap();
        assert_eq!(json.data.status, AgentStatus::Pending);
        assert!(json.data.node_id.is_none());
        assert_eq!(json.data.restart_count, 0);
    }

    #[tokio::test]
    async fn test_create_agent_name_too_long_fails() {
        let state = test_state().await;
        let long_name = "a".repeat(129);
        let spec = test_spec(&long_name);
        let result = create_agent(State(state), Json(spec)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_then_update_status_fails() {
        let state = test_state().await;
        let (_, json) = create_agent(State(state.clone()), Json(test_spec("del-then-update")))
            .await
            .unwrap();
        let id = json.data.id.clone();
        // Delete
        let _ = delete_agent(State(state.clone()), Path(id.clone())).await;
        // Try to update status on deleted agent
        let result = update_agent_status(State(state), Path(id), Json(AgentStatus::Running)).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            axum::http::StatusCode::NOT_FOUND
        );
    }
}
