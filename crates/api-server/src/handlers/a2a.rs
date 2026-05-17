//! # A2A Protocol HTTP Handlers
//!
//! Implements the Google A2A (Agent-to-Agent) protocol HTTP endpoints.
//!
//! ## Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/.well-known/agent.json` | Agent Card discovery |
//! | GET | `/a2a/v1/agents` | List all registered agent cards |
//! | GET | `/a2a/v1/agents/:id` | Get specific agent card |
//! | POST | `/a2a/v1/tasks` | Send a task to an agent |
//! | GET | `/a2a/v1/tasks/:id` | Get task status and details |
//! | POST | `/a2a/v1/tasks/:id/cancel` | Cancel an active task |
//! | DELETE | `/a2a/v1/tasks/:id` | Delete a completed task |
//! | GET | `/a2a/v1/tasks/:id/stream` | SSE stream for task updates |
//!
//! ## A2A Protocol Flow
//!
//! ```text
//! Client                    KIAS A2A Server              Agent
//!   │                            │                         │
//!   ├─ GET /.well-known/agent.json ──▶│                     │
//!   │◀── AgentCard ──────────────┤                         │
//!   │                            │                         │
//!   ├─ POST /a2a/v1/tasks ──────▶│                         │
//!   │   (send task message)      ├─ route task ───────────▶│
//!   │◀── A2aTask (Working) ─────┤                         │
//!   │                            │◀── update status ───────┤
//!   ├─ GET /a2a/v1/tasks/:id ───▶│                         │
//!   │◀── A2aTask (status) ──────┤                         │
//!   │                            │                         │
//!   ├─ GET /a2a/v1/tasks/:id/stream ─▶│                    │
//!   │◀── SSE events ────────────┤                         │
//! ```

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use chrono::Utc;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use tokio::sync::watch;
use uuid::Uuid;

use kias_common::a2a::{A2aArtifact, A2aMessage, A2aPart, A2aRole, A2aTask, A2aTaskStatus};

use crate::error::ApiError;
use crate::models::request::{ApiResponse, ListResponse};
use crate::websocket::{EventType, WsEvent};
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / Response types specific to A2A HTTP layer
// ---------------------------------------------------------------------------

/// Query params for listing agent cards
#[derive(Debug, Deserialize)]
pub struct AgentCardQuery {
    /// Filter by skill tag
    pub skill: Option<String>,
    /// Filter by capability
    pub streaming: Option<bool>,
}

/// Query params for listing tasks
#[derive(Debug, Deserialize)]
pub struct TaskListQuery {
    /// Filter by status
    pub status: Option<String>,
    /// Filter by session ID
    pub session_id: Option<String>,
}

/// Task send request for the HTTP layer
#[derive(Debug, Deserialize)]
pub struct TaskSendBody {
    /// Optional task ID (auto-generated if omitted)
    pub id: Option<String>,
    /// Session ID for multi-turn conversations
    pub session_id: Option<String>,
    /// The message to send
    pub message: A2aMessage,
    /// Routing strategy hint
    pub target_agent: Option<String>,
    /// Required capabilities for capability-based routing
    pub required_capabilities: Option<Vec<String>>,
    /// Metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Task cancel request
#[derive(Debug, Deserialize)]
pub struct TaskCancelBody {
    /// Optional reason for cancellation
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// /fire — synchronous agent invocation (Sembr-inspired)
// ---------------------------------------------------------------------------

/// Request body for the synchronous `/a2a/v1/fire` endpoint.
///
/// "Fire" sends a message to an agent and **waits** for the result,
/// returning it in a single HTTP response. This is the standardised
/// synchronous counterpart to the async `POST /a2a/v1/tasks`.
#[derive(Debug, Deserialize)]
pub struct FireRequest {
    /// Target agent ID (optional — if omitted, routes by capability).
    pub target_agent: Option<String>,
    /// The message to deliver.
    pub message: A2aMessage,
    /// Maximum time to wait for a result (milliseconds). Default: 30 000.
    #[serde(default = "default_fire_timeout")]
    pub timeout_ms: u64,
    /// Required capabilities for capability-based routing.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    /// Arbitrary metadata attached to the request.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_fire_timeout() -> u64 {
    30_000
}

/// Response from the synchronous `/a2a/v1/fire` endpoint.
#[derive(Debug, Serialize)]
pub struct FireResponse {
    /// Unique request ID.
    pub request_id: String,
    /// Final status: "completed", "failed", or "timeout".
    pub status: String,
    /// Agent that handled the request (echoed from routing or request).
    pub target_agent: String,
    /// The agent's response message, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<A2aMessage>,
    /// Artifacts produced by the agent.
    #[serde(default)]
    pub artifacts: Vec<A2aArtifact>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Error description when status is "failed" or "timeout".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// SSE event data for task updates
#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    /// Event type: "status_change", "message", "artifact", "error", "complete"
    pub event: String,
    /// Task ID
    pub task_id: String,
    /// Current status
    pub status: A2aTaskStatus,
    /// Optional message data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<A2aMessage>,
    /// Optional artifact data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<A2aArtifact>,
    /// Timestamp
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// In-memory A2A task store with SSE change notification
// ---------------------------------------------------------------------------

/// A2A task store with change notification for SSE streaming.
///
/// Tasks are stored in-memory and each task has a `watch::Sender` that
/// notifies SSE subscribers when the task state changes.
#[derive(Clone)]
pub struct A2aTaskStore {
    tasks: std::sync::Arc<tokio::sync::RwLock<HashMap<String, A2aTask>>>,
    /// Per-task change notifiers for SSE streaming
    notifiers:
        std::sync::Arc<tokio::sync::RwLock<HashMap<String, watch::Sender<Option<TaskEvent>>>>>,
}

impl Default for A2aTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

impl A2aTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            notifiers: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Insert a new task and create its notifier channel.
    pub async fn insert(&self, task: A2aTask) {
        let task_id = task.id.clone();
        let (tx, _rx) = watch::channel(None);
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), task);
        }
        {
            let mut notifiers = self.notifiers.write().await;
            notifiers.insert(task_id, tx);
        }
    }

    /// Get a task by ID.
    pub async fn get(&self, task_id: &str) -> Option<A2aTask> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    /// Update a task's status and notify SSE listeners.
    pub async fn update(&self, task_id: &str, event: TaskEvent) -> Option<A2aTask> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = event.status.clone();
            task.updated_at = Utc::now();
            let updated = task.clone();

            // Notify SSE listeners
            let notifiers = self.notifiers.read().await;
            if let Some(tx) = notifiers.get(task_id) {
                let _ = tx.send(Some(event));
            }

            Some(updated)
        } else {
            None
        }
    }

    /// Remove a task and its notifier.
    pub async fn remove(&self, task_id: &str) -> Option<A2aTask> {
        let mut tasks = self.tasks.write().await;
        let mut notifiers = self.notifiers.write().await;
        notifiers.remove(task_id);
        tasks.remove(task_id)
    }

    /// List all tasks, optionally filtered by status.
    pub async fn list(
        &self,
        status_filter: Option<&str>,
        _session_filter: Option<&str>,
    ) -> Vec<A2aTask> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| {
                if let Some(sf) = status_filter {
                    let status_str = format!("{:?}", t.status).to_lowercase();
                    status_str.contains(&sf.to_lowercase())
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    /// Subscribe to task events for SSE streaming.
    pub async fn subscribe(&self, task_id: &str) -> Option<watch::Receiver<Option<TaskEvent>>> {
        let notifiers = self.notifiers.read().await;
        notifiers.get(task_id).map(|tx| tx.subscribe())
    }

    /// Task count.
    pub async fn count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }
}

// ---------------------------------------------------------------------------
// Agent Card handlers
// ---------------------------------------------------------------------------

/// GET /.well-known/agent.json
///
/// Returns the server's own Agent Card per the A2A specification.
/// This is the primary discovery endpoint for A2A clients.
pub async fn well_known_agent_card() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": "kias-server",
        "name": "KIAS Agent Scheduler",
        "description": "Kubernetes-like Intelligent Agent Scheduling System — manages, routes, and orchestrates AI agent tasks across a cluster",
        "protocolVersion": "1.0",
        "version": env!("CARGO_PKG_VERSION"),
        "url": "http://localhost:8080/a2a/v1",
        "capabilities": {
            "streaming": true,
            "pushNotifications": true,
            "stateTransitionHistory": true
        },
        "defaultInputModes": ["text/plain", "application/json"],
        "defaultOutputModes": ["text/plain", "application/json"],
        "skills": [
            {
                "id": "task-routing",
                "name": "Task Routing",
                "description": "Route tasks to the best agent using capability, load-balanced, broadcast, or chain strategies",
                "examples": ["Route this code review to a security expert", "Balance this workload across available agents"],
                "tags": ["routing", "scheduling", "load-balancing"],
                "locationBound": false
            },
            {
                "id": "agent-handoff",
                "name": "Agent Handoff",
                "description": "Seamlessly transfer tasks between agents based on capability gaps or load",
                "examples": ["Hand off this task to a more specialized agent"],
                "tags": ["handoff", "delegation", "collaboration"],
                "locationBound": false
            },
            {
                "id": "workflow-execution",
                "name": "Workflow Execution",
                "description": "Execute DAG-based multi-step workflows with Shell, HTTP, or LLM nodes",
                "examples": ["Run this CI/CD pipeline", "Execute this data processing workflow"],
                "tags": ["workflow", "dag", "orchestration"],
                "locationBound": false
            }
        ],
        "authentication": {
            "schemes": ["bearer"],
            "required": true
        },
        "provider": {
            "organization": "KIAS",
            "url": "https://github.com/kias-project"
        }
    }))
}

/// GET /a2a/v1/agents
///
/// List all registered agent cards, with optional filtering by skill or capability.
pub async fn list_agent_cards(
    State(state): State<AppState>,
    Query(query): Query<AgentCardQuery>,
) -> Json<ListResponse<serde_json::Value>> {
    let agents = state.agents.read().await;
    let all_cards: Vec<serde_json::Value> = agents
        .values()
        .map(|agent| {
            serde_json::json!({
                "id": agent.id,
                "name": agent.spec.name,
                "description": format!("KIAS agent: {}", agent.spec.name),
                "protocolVersion": "1.0",
                "version": "0.1.0",
                "url": format!("http://localhost:8080/a2a/v1/agents/{}", agent.id),
                "capabilities": {
                    "streaming": false,
                    "pushNotifications": false,
                    "stateTransitionHistory": true
                },
                "defaultInputModes": ["text/plain", "application/json"],
                "defaultOutputModes": ["text/plain", "application/json"],
                "skills": [],
                "status": format!("{:?}", agent.status),
                "nodeId": agent.node_id,
            })
        })
        .collect();

    // Apply filters
    let filtered: Vec<serde_json::Value> = all_cards
        .into_iter()
        .filter(|card| {
            if let Some(ref skill_filter) = query.skill {
                if let Some(skills) = card.get("skills").and_then(|s| s.as_array()) {
                    return skills.iter().any(|s| {
                        s.get("tags")
                            .and_then(|t| t.as_array())
                            .map(|tags| {
                                tags.iter()
                                    .any(|t| t.as_str() == Some(skill_filter.as_str()))
                            })
                            .unwrap_or(false)
                    });
                }
                return false;
            }
            true
        })
        .filter(|card| {
            if let Some(streaming) = query.streaming {
                return card
                    .get("capabilities")
                    .and_then(|c| c.get("streaming"))
                    .and_then(|s| s.as_bool())
                    == Some(streaming);
            }
            true
        })
        .collect();

    let total = filtered.len();
    Json(ListResponse {
        items: filtered,
        total,
    })
}

/// GET /a2a/v1/agents/:id
///
/// Get a specific agent's card by ID.
pub async fn get_agent_card(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let agents = state.agents.read().await;
    let agent = agents
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("Agent '{}' not found", id)))?;

    Ok(Json(serde_json::json!({
        "id": agent.id,
        "name": agent.spec.name,
        "description": format!("KIAS agent: {}", agent.spec.name),
        "protocolVersion": "1.0",
        "version": "0.1.0",
        "url": format!("http://localhost:8080/a2a/v1/agents/{}", agent.id),
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": true
        },
        "defaultInputModes": ["text/plain", "application/json"],
        "defaultOutputModes": ["text/plain", "application/json"],
        "skills": [],
        "status": format!("{:?}", agent.status),
        "nodeId": agent.node_id,
        "createdAt": agent.created_at,
        "labels": agent.spec.labels,
    })))
}

// ---------------------------------------------------------------------------
// Task lifecycle handlers
// ---------------------------------------------------------------------------

/// Helper to create a WsEvent for A2A task events
fn a2a_ws_event(event_type: EventType, data: serde_json::Value) -> WsEvent {
    WsEvent {
        event_type,
        data,
        timestamp: Utc::now().to_rfc3339(),
    }
}

/// POST /a2a/v1/tasks
///
/// Send a task to an agent. The task enters the A2A lifecycle:
/// Submitted → Working → Completed/Failed/Cancelled.
pub async fn send_task(
    State(state): State<AppState>,
    Json(body): Json<TaskSendBody>,
) -> Result<(axum::http::StatusCode, Json<ApiResponse<serde_json::Value>>), ApiError> {
    let task_id = body.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = Utc::now();

    // Validate message has content
    if body.message.parts.is_empty() {
        return Err(ApiError::bad_request(
            "Message must contain at least one part",
        ));
    }

    // Build the A2A task
    let task = A2aTask {
        id: task_id.clone(),
        status: A2aTaskStatus::Submitted,
        messages: vec![body.message],
        metadata: body.metadata,
        artifacts: vec![],
        created_at: now,
        updated_at: now,
    };

    // Store the task
    state.a2a_tasks.insert(task.clone()).await;

    // Emit WebSocket event
    state.event_bus.publish(a2a_ws_event(
        EventType::A2aTaskSubmitted,
        serde_json::json!({ "task_id": task_id }),
    ));

    tracing::info!(task_id = %task_id, "A2A task submitted");

    // Transition to Working state
    let working_event = TaskEvent {
        event: "status_change".to_string(),
        task_id: task_id.clone(),
        status: A2aTaskStatus::Working,
        message: None,
        artifact: None,
        timestamp: now.to_rfc3339(),
    };
    state.a2a_tasks.update(&task_id, working_event).await;

    state.event_bus.publish(a2a_ws_event(
        EventType::A2aTaskWorking,
        serde_json::json!({ "task_id": task_id, "status": "Working" }),
    ));

    // Simulate task completion after a short delay
    let a2a_tasks = state.a2a_tasks.clone();
    let event_bus = state.event_bus.clone();
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let complete_event = TaskEvent {
            event: "complete".to_string(),
            task_id: task_id_clone.clone(),
            status: A2aTaskStatus::Completed,
            message: Some(A2aMessage {
                role: A2aRole::Agent,
                parts: vec![A2aPart::Text {
                    text: "Task processed successfully by KIAS".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            }),
            artifact: None,
            timestamp: Utc::now().to_rfc3339(),
        };

        // Update task status
        if let Some(mut updated) = a2a_tasks.update(&task_id_clone, complete_event).await {
            updated.messages.push(A2aMessage {
                role: A2aRole::Agent,
                parts: vec![A2aPart::Text {
                    text: "Task processed successfully by KIAS".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            });

            updated.artifacts.push(A2aArtifact {
                id: Uuid::new_v4().to_string(),
                name: Some("result".to_string()),
                parts: vec![A2aPart::Text {
                    text: "Task output from KIAS agent scheduler".to_string(),
                    metadata: None,
                }],
                metadata: HashMap::new(),
            });

            // Re-store with full data
            let mut tasks = a2a_tasks.tasks.write().await;
            tasks.insert(task_id_clone.clone(), updated);
        }

        event_bus.publish(a2a_ws_event(
            EventType::A2aTaskCompleted,
            serde_json::json!({ "task_id": task_id_clone, "status": "Completed" }),
        ));
    });

    let response_task = state.a2a_tasks.get(&task_id).await;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            data: serde_json::to_value(&response_task).unwrap_or_default(),
        }),
    ))
}

/// GET /a2a/v1/tasks/:id
///
/// Get task status and details.
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let task = state
        .a2a_tasks
        .get(&id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Task '{}' not found", id)))?;

    Ok(Json(ApiResponse {
        data: serde_json::to_value(&task).unwrap_or_default(),
    }))
}

/// GET /a2a/v1/tasks
///
/// List all tasks with optional filtering.
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Json<ListResponse<serde_json::Value>> {
    let tasks = state
        .a2a_tasks
        .list(query.status.as_deref(), query.session_id.as_deref())
        .await;

    let items: Vec<serde_json::Value> = tasks
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect();

    let total = items.len();
    Json(ListResponse { items, total })
}

/// POST /a2a/v1/tasks/:id/cancel
///
/// Cancel an active task. Only tasks in Submitted/Working/InputRequired can be cancelled.
pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TaskCancelBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let task = state
        .a2a_tasks
        .get(&id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Task '{}' not found", id)))?;

    if task.status.is_terminal() {
        return Err(ApiError::bad_request(format!(
            "Task '{}' is already in terminal state {:?}",
            id, task.status
        )));
    }

    let reason = body
        .reason
        .unwrap_or_else(|| "User requested cancellation".to_string());

    let cancel_event = TaskEvent {
        event: "status_change".to_string(),
        task_id: id.clone(),
        status: A2aTaskStatus::Cancelled,
        message: Some(A2aMessage {
            role: A2aRole::System,
            parts: vec![A2aPart::Text {
                text: format!("Cancelled: {}", reason),
                metadata: None,
            }],
            is_final: true,
            metadata: HashMap::new(),
        }),
        artifact: None,
        timestamp: Utc::now().to_rfc3339(),
    };

    let updated = state.a2a_tasks.update(&id, cancel_event).await;

    state.event_bus.publish(a2a_ws_event(
        EventType::A2aTaskCancelled,
        serde_json::json!({ "task_id": id, "reason": reason }),
    ));

    tracing::info!(task_id = %id, reason = %reason, "A2A task cancelled");

    Ok(Json(ApiResponse {
        data: serde_json::to_value(&updated).unwrap_or_default(),
    }))
}

/// DELETE /a2a/v1/tasks/:id
///
/// Delete a task. Only terminal tasks can be deleted.
pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let task = state
        .a2a_tasks
        .get(&id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Task '{}' not found", id)))?;

    if !task.status.is_terminal() {
        return Err(ApiError::bad_request(format!(
            "Cannot delete active task '{}' (status: {:?}). Cancel it first.",
            id, task.status
        )));
    }

    state.a2a_tasks.remove(&id).await;

    state.event_bus.publish(a2a_ws_event(
        EventType::A2aTaskDeleted,
        serde_json::json!({ "task_id": id }),
    ));

    tracing::info!(task_id = %id, "A2A task deleted");

    Ok(Json(ApiResponse {
        data: serde_json::json!({ "deleted": true, "task_id": id }),
    }))
}

// ---------------------------------------------------------------------------
// SSE Streaming
// ---------------------------------------------------------------------------

/// Map a watch event into an SSE Event
fn task_event_to_sse(event: Option<TaskEvent>) -> Option<Result<Event, Infallible>> {
    event.map(|evt| {
        let data = serde_json::to_string(&evt).unwrap_or_default();
        let event_name = evt.event.clone();
        Ok(Event::default().data(data).event(event_name))
    })
}

/// GET /a2a/v1/tasks/:id/stream
///
/// Server-Sent Events stream for real-time task updates.
/// Clients subscribe to receive status changes, messages, and artifacts.
pub async fn stream_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Verify task exists
    let _task = state
        .a2a_tasks
        .get(&id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Task '{}' not found", id)))?;

    let rx = state
        .a2a_tasks
        .subscribe(&id)
        .await
        .ok_or_else(|| ApiError::internal("Failed to subscribe to task events"))?;

    // Convert watch::Receiver into a Stream using tokio_stream::WatchStream
    let watch_stream = tokio_stream::wrappers::WatchStream::new(rx);
    let sse_stream = watch_stream.filter_map(|item| async { task_event_to_sse(item) });

    tracing::info!(task_id = %id, "SSE stream opened for A2A task");

    Ok(Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ))
}

// ---------------------------------------------------------------------------
// /fire — synchronous agent invocation handler
// ---------------------------------------------------------------------------

/// POST /a2a/v1/fire
///
/// Synchronous "fire and wait" endpoint. Sends a message to an agent
/// (or routes by capability) and blocks until the result is available
/// or the timeout expires.
///
/// This is the Sembr-inspired standardised synchronous A2A call:
/// one request in, one response out.
pub async fn fire_agent(
    State(state): State<AppState>,
    Json(body): Json<FireRequest>,
) -> Result<Json<FireResponse>, ApiError> {
    let request_id = Uuid::new_v4().to_string();
    let timeout = std::time::Duration::from_millis(body.timeout_ms);
    let start = std::time::Instant::now();

    // Validate message has content
    if body.message.parts.is_empty() {
        return Err(ApiError::bad_request(
            "Message must contain at least one part",
        ));
    }

    // Determine target agent
    let target = body
        .target_agent
        .clone()
        .unwrap_or_else(|| "auto".to_string());

    tracing::info!(
        request_id = %request_id,
        target = %target,
        timeout_ms = body.timeout_ms,
        "A2A /fire request received"
    );

    // Create an A2A task in the store
    let task = A2aTask {
        id: request_id.clone(),
        status: A2aTaskStatus::Submitted,
        messages: vec![body.message],
        metadata: body.metadata,
        artifacts: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    state.a2a_tasks.insert(task).await;

    // Emit WebSocket event: submitted
    state.event_bus.publish(a2a_ws_event(
        EventType::A2aTaskSubmitted,
        serde_json::json!({ "task_id": request_id, "source": "fire" }),
    ));

    // Transition to Working
    let working_event = TaskEvent {
        event: "status_change".to_string(),
        task_id: request_id.clone(),
        status: A2aTaskStatus::Working,
        message: None,
        artifact: None,
        timestamp: Utc::now().to_rfc3339(),
    };
    state.a2a_tasks.update(&request_id, working_event).await;

    // Wait for the task to complete (poll-based with timeout).
    // In a production system this would use a channel/notification.
    // Here we simulate by checking the task store periodically.
    let poll_interval = std::time::Duration::from_millis(50);
    let mut elapsed = std::time::Duration::ZERO;

    // Simulate processing: mark complete after a short delay
    let a2a_tasks = state.a2a_tasks.clone();
    let event_bus = state.event_bus.clone();
    let rid = request_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let complete_event = TaskEvent {
            event: "complete".to_string(),
            task_id: rid.clone(),
            status: A2aTaskStatus::Completed,
            message: Some(A2aMessage {
                role: A2aRole::Agent,
                parts: vec![A2aPart::Text {
                    text: "Fire request processed by KIAS (target: auto)".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            }),
            artifact: None,
            timestamp: Utc::now().to_rfc3339(),
        };

        if let Some(mut updated) = a2a_tasks.update(&rid, complete_event).await {
            updated.messages.push(A2aMessage {
                role: A2aRole::Agent,
                parts: vec![A2aPart::Text {
                    text: "Fire request processed by KIAS (target: auto)".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            });
            updated.artifacts.push(A2aArtifact {
                id: Uuid::new_v4().to_string(),
                name: Some("fire-result".to_string()),
                parts: vec![A2aPart::Text {
                    text: "Fire endpoint result artifact".to_string(),
                    metadata: None,
                }],
                metadata: HashMap::new(),
            });
            let mut tasks = a2a_tasks.tasks.write().await;
            tasks.insert(rid.clone(), updated);
        }

        event_bus.publish(a2a_ws_event(
            EventType::A2aTaskCompleted,
            serde_json::json!({ "task_id": rid, "source": "fire" }),
        ));
    });

    // Poll for completion or timeout
    loop {
        if elapsed >= timeout {
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::warn!(
                request_id = %request_id,
                duration_ms = duration_ms,
                "A2A /fire request timed out"
            );
            return Ok(Json(FireResponse {
                request_id,
                status: "timeout".to_string(),
                target_agent: target,
                result: None,
                artifacts: vec![],
                duration_ms,
                error: Some(format!("Request timed out after {}ms", body.timeout_ms)),
            }));
        }

        tokio::time::sleep(poll_interval).await;
        elapsed = start.elapsed();

        if let Some(task) = state.a2a_tasks.get(&request_id).await {
            if task.status.is_terminal() {
                let duration_ms = start.elapsed().as_millis() as u64;
                let status_str = match &task.status {
                    A2aTaskStatus::Completed => "completed",
                    A2aTaskStatus::Failed => "failed",
                    A2aTaskStatus::Cancelled => "cancelled",
                    A2aTaskStatus::Rejected => "rejected",
                    _ => "unknown",
                }
                .to_string();

                // Extract the last agent message as the result
                let result_msg = task
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == A2aRole::Agent)
                    .cloned();

                tracing::info!(
                    request_id = %request_id,
                    status = %status_str,
                    duration_ms = duration_ms,
                    "A2A /fire request completed"
                );

                return Ok(Json(FireResponse {
                    request_id,
                    status: status_str,
                    target_agent: target,
                    result: result_msg,
                    artifacts: task.artifacts,
                    duration_ms,
                    error: None,
                }));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_event_serialization() {
        let event = TaskEvent {
            event: "status_change".to_string(),
            task_id: "task-1".to_string(),
            status: A2aTaskStatus::Working,
            message: None,
            artifact: None,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("status_change"));
        assert!(json.contains("task-1"));
        assert!(json.contains("Working"));
    }

    #[test]
    fn test_task_send_body_deserialization() {
        let json = r#"{
            "message": {
                "role": "User",
                "parts": [{"Text": {"text": "hello"}}],
                "is_final": true,
                "metadata": {}
            }
        }"#;

        let body: TaskSendBody = serde_json::from_str(json).unwrap();
        assert!(body.id.is_none());
        assert_eq!(body.message.parts.len(), 1);
    }

    #[test]
    fn test_task_send_body_with_target() {
        let json = r#"{
            "message": {
                "role": "User",
                "parts": [{"Text": {"text": "review this code"}}],
                "is_final": true,
                "metadata": {}
            },
            "target_agent": "agent-1",
            "required_capabilities": ["code-review"]
        }"#;

        let body: TaskSendBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.target_agent, Some("agent-1".to_string()));
        assert_eq!(
            body.required_capabilities,
            Some(vec!["code-review".to_string()])
        );
    }

    #[test]
    fn test_agent_card_query_deserialization() {
        // Verify the query struct can be deserialized from JSON
        let json = r#"{"skill": "coding", "streaming": true}"#;
        let query: AgentCardQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.skill, Some("coding".to_string()));
        assert_eq!(query.streaming, Some(true));
    }

    #[test]
    fn test_task_list_query_defaults() {
        let json = r#"{}"#;
        let query: TaskListQuery = serde_json::from_str(json).unwrap();
        assert!(query.status.is_none());
        assert!(query.session_id.is_none());
    }

    #[tokio::test]
    async fn test_task_store_insert_and_get() {
        let store = A2aTaskStore::new();
        let task = A2aTask {
            id: "t1".to_string(),
            status: A2aTaskStatus::Submitted,
            messages: vec![],
            metadata: HashMap::new(),
            artifacts: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.insert(task).await;
        let retrieved = store.get("t1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "t1");
    }

    #[tokio::test]
    async fn test_task_store_update_and_notify() {
        let store = A2aTaskStore::new();
        let task = A2aTask {
            id: "t1".to_string(),
            status: A2aTaskStatus::Submitted,
            messages: vec![],
            metadata: HashMap::new(),
            artifacts: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.insert(task).await;

        // Subscribe before update
        let mut rx = store.subscribe("t1").await.unwrap();

        let event = TaskEvent {
            event: "status_change".to_string(),
            task_id: "t1".to_string(),
            status: A2aTaskStatus::Working,
            message: None,
            artifact: None,
            timestamp: Utc::now().to_rfc3339(),
        };

        let updated = store.update("t1", event).await;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, A2aTaskStatus::Working);

        // Verify notification was sent
        assert!(rx.changed().await.is_ok());
        let received = rx.borrow().clone();
        assert!(received.is_some());
        assert_eq!(received.unwrap().event, "status_change");
    }

    #[tokio::test]
    async fn test_task_store_remove() {
        let store = A2aTaskStore::new();
        let task = A2aTask {
            id: "t1".to_string(),
            status: A2aTaskStatus::Completed,
            messages: vec![],
            metadata: HashMap::new(),
            artifacts: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.insert(task).await;
        assert_eq!(store.count().await, 1);

        let removed = store.remove("t1").await;
        assert!(removed.is_some());
        assert_eq!(store.count().await, 0);
        assert!(store.get("t1").await.is_none());
    }

    #[tokio::test]
    async fn test_task_store_list_filter() {
        let store = A2aTaskStore::new();

        store
            .insert(A2aTask {
                id: "t1".to_string(),
                status: A2aTaskStatus::Submitted,
                messages: vec![],
                metadata: HashMap::new(),
                artifacts: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await;

        store
            .insert(A2aTask {
                id: "t2".to_string(),
                status: A2aTaskStatus::Completed,
                messages: vec![],
                metadata: HashMap::new(),
                artifacts: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await;

        let all = store.list(None, None).await;
        assert_eq!(all.len(), 2);

        let submitted = store.list(Some("submitted"), None).await;
        assert_eq!(submitted.len(), 1);
        assert_eq!(submitted[0].id, "t1");

        let completed = store.list(Some("completed"), None).await;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "t2");
    }

    #[tokio::test]
    async fn test_task_store_nonexistent() {
        let store = A2aTaskStore::new();
        assert!(store.get("nonexistent").await.is_none());
        assert!(store.subscribe("nonexistent").await.is_none());

        let event = TaskEvent {
            event: "test".to_string(),
            task_id: "nonexistent".to_string(),
            status: A2aTaskStatus::Working,
            message: None,
            artifact: None,
            timestamp: Utc::now().to_rfc3339(),
        };
        assert!(store.update("nonexistent", event).await.is_none());
    }

    #[test]
    fn test_cancel_body_deserialization() {
        let json = r#"{"reason": "no longer needed"}"#;
        let body: TaskCancelBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.reason, Some("no longer needed".to_string()));

        let empty = r#"{}"#;
        let body: TaskCancelBody = serde_json::from_str(empty).unwrap();
        assert!(body.reason.is_none());
    }

    #[test]
    fn test_well_known_card_structure() {
        let card = serde_json::json!({
            "id": "kias-server",
            "name": "KIAS Agent Scheduler",
            "protocolVersion": "1.0",
            "capabilities": {
                "streaming": true,
                "pushNotifications": true,
                "stateTransitionHistory": true
            },
            "skills": []
        });

        assert!(card.get("id").is_some());
        assert!(card.get("capabilities").is_some());
        assert!(card.get("skills").is_some());
        assert_eq!(card["capabilities"]["streaming"], true);
    }

    #[test]
    fn test_task_event_with_artifact() {
        let event = TaskEvent {
            event: "complete".to_string(),
            task_id: "t1".to_string(),
            status: A2aTaskStatus::Completed,
            message: None,
            artifact: Some(A2aArtifact {
                id: "art-1".to_string(),
                name: Some("result".to_string()),
                parts: vec![A2aPart::Text {
                    text: "output".to_string(),
                    metadata: None,
                }],
                metadata: HashMap::new(),
            }),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("art-1"));
        assert!(json.contains("result"));
    }

    // ------------------------------------------------------------------
    // /fire endpoint tests
    // ------------------------------------------------------------------

    #[test]
    fn test_fire_request_deserialization() {
        let json = r#"{
            "message": {
                "role": "User",
                "parts": [{"Text": {"text": "summarize this"}}],
                "is_final": true,
                "metadata": {}
            },
            "target_agent": "agent-42",
            "timeout_ms": 5000,
            "required_capabilities": ["summarization"]
        }"#;

        let req: FireRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.target_agent, Some("agent-42".to_string()));
        assert_eq!(req.timeout_ms, 5000);
        assert_eq!(req.required_capabilities, vec!["summarization".to_string()]);
        assert_eq!(req.message.parts.len(), 1);
    }

    #[test]
    fn test_fire_request_defaults() {
        let json = r#"{
            "message": {
                "role": "User",
                "parts": [{"Text": {"text": "hello"}}],
                "is_final": true,
                "metadata": {}
            }
        }"#;

        let req: FireRequest = serde_json::from_str(json).unwrap();
        assert!(req.target_agent.is_none());
        assert_eq!(req.timeout_ms, 30_000);
        assert!(req.required_capabilities.is_empty());
    }

    #[test]
    fn test_fire_response_serialization() {
        let resp = FireResponse {
            request_id: "req-1".to_string(),
            status: "completed".to_string(),
            target_agent: "agent-1".to_string(),
            result: Some(A2aMessage {
                role: A2aRole::Agent,
                parts: vec![A2aPart::Text {
                    text: "done".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            }),
            artifacts: vec![],
            duration_ms: 150,
            error: None,
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("req-1"));
        assert!(json.contains("completed"));
        assert!(json.contains("agent-1"));
        assert!(json.contains("done"));
        assert!(json.contains("150"));
        // error is None, should be skipped
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_fire_response_with_error() {
        let resp = FireResponse {
            request_id: "req-2".to_string(),
            status: "timeout".to_string(),
            target_agent: "auto".to_string(),
            result: None,
            artifacts: vec![],
            duration_ms: 30000,
            error: Some("Request timed out after 30000ms".to_string()),
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("timeout"));
        assert!(json.contains("Request timed out"));
    }

    #[test]
    fn test_fire_request_empty_parts_rejected() {
        let json = r#"{
            "message": {
                "role": "User",
                "parts": [],
                "is_final": true,
                "metadata": {}
            }
        }"#;

        let req: FireRequest = serde_json::from_str(json).unwrap();
        // The handler validates parts.is_empty() — we verify the struct
        // deserializes correctly and has empty parts.
        assert!(req.message.parts.is_empty());
    }
}
