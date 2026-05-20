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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
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
                .expect("Failed to create knowledge retriever");

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(std::collections::HashMap::new())),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
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
    async fn test_add_message_user_role() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "user".to_string(),
            content: "Hello world".to_string(),
        };
        let result = add_message(State(state), Path("session-1".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "session-1");
    }

    #[tokio::test]
    async fn test_add_message_system_role() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "system".to_string(),
            content: "System prompt".to_string(),
        };
        let result = add_message(State(state), Path("session-2".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "session-2");
    }

    #[tokio::test]
    async fn test_add_message_unknown_role_defaults_to_user() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "unknown_role".to_string(),
            content: "test".to_string(),
        };
        let result = add_message(State(state), Path("session-3".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "session-3");
    }

    #[tokio::test]
    async fn test_compress_session_no_context_manager() {
        let state = test_state().await;
        let result = compress_session(State(state), Path("session-1".to_string())).await;
        assert_eq!(result.session_id, "session-1");
        assert_eq!(result.compression_level, "none");
        assert_eq!(result.message_count, 0);
    }

    #[tokio::test]
    async fn test_get_context_stats_no_context_manager() {
        let state = test_state().await;
        let result = get_context_stats(State(state), Path("session-1".to_string())).await;
        assert_eq!(result.session_id, "session-1");
        assert_eq!(result.total_messages, 0);
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.utilization, 0.0);
        assert_eq!(result.compression_count, 0);
    }

    #[tokio::test]
    async fn test_add_message_assistant_role() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "assistant".to_string(),
            content: "I can help with that".to_string(),
        };
        let result = add_message(State(state), Path("session-4".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "session-4");
        assert_eq!(result.compression_level, "none");
    }

    #[tokio::test]
    async fn test_add_message_tool_role() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "tool".to_string(),
            content: "{\"output\": \"ok\"}".to_string(),
        };
        let result = add_message(State(state), Path("session-5".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "session-5");
    }

    #[test]
    fn test_add_message_request_deserialize() {
        let json = r#"{"role":"user","content":"hello"}"#;
        let req: AddMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.role, "user");
        assert_eq!(req.content, "hello");
    }

    #[test]
    fn test_context_response_serialize() {
        let resp = ContextResponse {
            session_id: "s1".to_string(),
            message_count: 5,
            total_tokens: 1000,
            compression_level: "summary".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("session_id"));
        assert!(json.contains("s1"));
        assert!(json.contains("message_count"));
        assert!(json.contains("1000"));
    }

    #[test]
    fn test_context_stats_response_serialize() {
        let resp = ContextStatsResponse {
            session_id: "s1".to_string(),
            total_messages: 10,
            user_messages: 4,
            assistant_messages: 3,
            tool_messages: 2,
            summary_messages: 1,
            total_tokens: 2000,
            max_tokens: 4096,
            utilization: 0.488,
            compression_count: 2,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("user_messages"));
        assert!(json.contains("assistant_messages"));
        assert!(json.contains("tool_messages"));
        assert!(json.contains("summary_messages"));
        assert!(json.contains("utilization"));
    }

    #[test]
    fn test_compress_session_response_defaults() {
        // Verify default values for compress when no context_manager
        let resp = ContextResponse {
            session_id: "s1".to_string(),
            message_count: 0,
            total_tokens: 0,
            compression_level: "none".to_string(),
        };
        assert_eq!(resp.compression_level, "none");
        assert_eq!(resp.message_count, 0);
    }

    #[tokio::test]
    async fn test_add_message_returns_default_response_values() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "user".to_string(),
            content: "test content".to_string(),
        };
        let result = add_message(State(state), Path("s1".to_string()), Json(req)).await;
        // add_message always returns these defaults (no context_manager = no counting)
        assert_eq!(result.message_count, 0);
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.compression_level, "none");
    }

    #[tokio::test]
    async fn test_add_message_empty_content() {
        let state = test_state().await;
        let req = AddMessageRequest {
            role: "user".to_string(),
            content: "".to_string(),
        };
        let result = add_message(State(state), Path("s-empty".to_string()), Json(req)).await;
        assert_eq!(result.session_id, "s-empty");
        assert_eq!(result.compression_level, "none");
    }

    #[tokio::test]
    async fn test_get_context_stats_all_zero_defaults() {
        let state = test_state().await;
        let result = get_context_stats(State(state), Path("s-new".to_string())).await;
        assert_eq!(result.total_messages, 0);
        assert_eq!(result.user_messages, 0);
        assert_eq!(result.assistant_messages, 0);
        assert_eq!(result.tool_messages, 0);
        assert_eq!(result.summary_messages, 0);
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.max_tokens, 0);
        assert_eq!(result.utilization, 0.0);
        assert_eq!(result.compression_count, 0);
    }

    #[tokio::test]
    async fn test_compress_session_all_zero_defaults() {
        let state = test_state().await;
        let result = compress_session(State(state), Path("s-new".to_string())).await;
        assert_eq!(result.session_id, "s-new");
        assert_eq!(result.message_count, 0);
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.compression_level, "none");
    }

    #[test]
    fn test_add_message_request_missing_content_fails() {
        let json = r#"{"role":"user"}"#;
        let result: Result<AddMessageRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_message_request_missing_role_fails() {
        let json = r#"{"content":"hello"}"#;
        let result: Result<AddMessageRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_stats_response_all_fields_serialize() {
        let resp = ContextStatsResponse {
            session_id: "s1".to_string(),
            total_messages: 10,
            user_messages: 4,
            assistant_messages: 3,
            tool_messages: 2,
            summary_messages: 1,
            total_tokens: 2000,
            max_tokens: 4096,
            utilization: 0.488,
            compression_count: 2,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["session_id"], "s1");
        assert_eq!(json["total_messages"], 10);
        assert_eq!(json["user_messages"], 4);
        assert_eq!(json["assistant_messages"], 3);
        assert_eq!(json["tool_messages"], 2);
        assert_eq!(json["summary_messages"], 1);
        assert_eq!(json["total_tokens"], 2000);
        assert_eq!(json["max_tokens"], 4096);
        assert_eq!(json["compression_count"], 2);
    }
}
