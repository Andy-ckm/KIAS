//! Context Manager API handlers
//!
//! Endpoints for managing session context and triggering compression.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use kias_knowledge::context_manager::{ContextMessage, MessageRole};

use crate::AppState;

/// Request to add a message to a session context
#[derive(Debug, Deserialize)]
pub struct AddMessageRequest {
    pub role: String,
    pub content: String,
}

/// Response for context operations
#[derive(Debug, Serialize)]
pub struct ContextResponse {
    pub session_id: String,
    pub message_count: usize,
    pub total_tokens: usize,
    pub compression_level: String,
}

/// Response for context stats
#[derive(Debug, Serialize)]
pub struct ContextStatsResponse {
    pub session_id: String,
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_messages: usize,
    pub summary_messages: usize,
    pub total_tokens: usize,
    pub max_tokens: usize,
    pub utilization: f64,
    pub compression_count: usize,
}

/// POST /api/v1/context/{session_id}/messages
/// Add a message to a session context
pub async fn add_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<AddMessageRequest>,
) -> Json<ContextResponse> {
    let role = match req.role.as_str() {
        "system" => MessageRole::System,
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::User,
    };

    let message = ContextMessage::new(role, &req.content);

    if let Some(ref ctx_manager) = state.context_manager {
        ctx_manager.push(&session_id, message).await;
    }

    Json(ContextResponse {
        session_id,
        message_count: 0,
        total_tokens: 0,
        compression_level: "none".to_string(),
    })
}

/// POST /api/v1/context/{session_id}/compress
/// Trigger compression for a session
pub async fn compress_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<ContextResponse> {
    let mut level = "none".to_string();
    let mut message_count = 0;
    let mut total_tokens = 0;

    if let Some(ref ctx_manager) = state.context_manager {
        if let Some(result) = ctx_manager.compress(&session_id).await {
            level = format!("{:?}", result.level);
            message_count = result.messages_after;
            total_tokens = result.tokens_after;
        }
    }

    Json(ContextResponse {
        session_id,
        message_count,
        total_tokens,
        compression_level: level,
    })
}

/// GET /api/v1/context/{session_id}/stats
/// Get context stats for a session
pub async fn get_context_stats(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<ContextStatsResponse> {
    if let Some(ref ctx_manager) = state.context_manager {
        if let Some(stats) = ctx_manager.session_stats(&session_id).await {
            return Json(ContextStatsResponse {
                session_id,
                total_messages: stats.total_messages,
                user_messages: stats.user_messages,
                assistant_messages: stats.assistant_messages,
                tool_messages: stats.tool_messages,
                summary_messages: stats.summary_messages,
                total_tokens: stats.total_tokens,
                max_tokens: stats.max_tokens,
                utilization: stats.utilization,
                compression_count: stats.compression_count,
            });
        }
    }

    Json(ContextStatsResponse {
        session_id,
        total_messages: 0,
        user_messages: 0,
        assistant_messages: 0,
        tool_messages: 0,
        summary_messages: 0,
        total_tokens: 0,
        max_tokens: 0,
        utilization: 0.0,
        compression_count: 0,
    })
}
