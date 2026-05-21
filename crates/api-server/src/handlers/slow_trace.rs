//! Slow Action Tracing handler.
//!
//! Exposes endpoints for querying and managing slow action traces.
//!
//! # Endpoints
//!
//! - `GET /api/v1/observability/slow-traces` — list recent slow traces
//! - `GET /api/v1/observability/slow-traces/summary` — aggregated statistics
//! - `GET /api/v1/observability/slow-traces/agent/:id` — traces for specific agent
//! - `PUT /api/v1/observability/slow-traces/config` — update threshold/config
//! - `DELETE /api/v1/observability/slow-traces` — clear all traces

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use kias_monitor::slow_trace::{
    SlowTrace, SlowTraceConfig, SlowTraceSummary,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

// ─── Query Parameters ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SlowTraceQuery {
    /// Maximum number of traces to return (default: 50).
    pub limit: Option<usize>,
}

// ─── Response Types ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SlowTraceListResponse {
    pub traces: Vec<SlowTrace>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct SlowTraceConfigResponse {
    pub threshold_ms: u64,
    pub max_traces: usize,
    pub enabled: bool,
}

// ─── Handlers ────────────────────────────────────────────────────────

/// GET /api/v1/observability/slow-traces
///
/// Returns recent slow traces, newest first.
pub async fn list_slow_traces(
    State(state): State<AppState>,
    Query(query): Query<SlowTraceQuery>,
) -> Json<SlowTraceListResponse> {
    let limit = query.limit.unwrap_or(50);
    let traces = state.slow_trace_collector.recent(limit).await;
    let total = state.slow_trace_collector.count().await;
    Json(SlowTraceListResponse { traces, total })
}

/// GET /api/v1/observability/slow-traces/summary
///
/// Returns aggregated slow trace statistics.
pub async fn slow_trace_summary(
    State(state): State<AppState>,
) -> Json<SlowTraceSummary> {
    Json(state.slow_trace_collector.summary().await)
}

/// GET /api/v1/observability/slow-traces/agent/:id
///
/// Returns slow traces for a specific agent.
pub async fn agent_slow_traces(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Json<SlowTraceListResponse> {
    let traces = state.slow_trace_collector.by_agent(&agent_id).await;
    let total = traces.len();
    Json(SlowTraceListResponse { traces, total })
}

/// PUT /api/v1/observability/slow-traces/config
///
/// Update slow trace configuration (threshold, enabled, etc.).
pub async fn update_slow_trace_config(
    State(state): State<AppState>,
    Json(config): Json<SlowTraceConfig>,
) -> Json<SlowTraceConfigResponse> {
    state.slow_trace_collector.update_config(config.clone()).await;
    Json(SlowTraceConfigResponse {
        threshold_ms: config.threshold_ms,
        max_traces: config.max_traces,
        enabled: config.enabled,
    })
}

/// DELETE /api/v1/observability/slow-traces
///
/// Clear all stored slow traces.
pub async fn clear_slow_traces(State(state): State<AppState>) -> StatusCode {
    state.slow_trace_collector.clear().await;
    StatusCode::NO_CONTENT
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::time::Duration;

    async fn test_state() -> AppState {
        AppState::new_async(kias_common::config::KiasConfig::default()).await
    }

    #[tokio::test]
    async fn test_list_slow_traces_empty() {
        let state = test_state().await;
        let result = list_slow_traces(
            State(state),
            Query(SlowTraceQuery { limit: None }),
        )
        .await;
        assert!(result.traces.is_empty());
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn test_list_slow_traces_with_data() {
        let state = test_state().await;

        // Record a slow action
        state
            .slow_trace_collector
            .record(
                "a1",
                "test-agent",
                kias_monitor::ActionCategory::LlmInference,
                "slow call",
                Duration::from_millis(2000),
                None,
            )
            .await;

        let result = list_slow_traces(
            State(state),
            Query(SlowTraceQuery { limit: None }),
        )
        .await;
        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.total, 1);
        assert_eq!(result.traces[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn test_list_slow_traces_with_limit() {
        let state = test_state().await;

        for i in 0..5 {
            state
                .slow_trace_collector
                .record(
                    &format!("a{}", i),
                    &format!("agent-{}", i),
                    kias_monitor::ActionCategory::Other,
                    &format!("action-{}", i),
                    Duration::from_millis(2000),
                    None,
                )
                .await;
        }

        let result = list_slow_traces(
            State(state),
            Query(SlowTraceQuery { limit: Some(3) }),
        )
        .await;
        assert_eq!(result.traces.len(), 3);
        assert_eq!(result.total, 5);
    }

    #[tokio::test]
    async fn test_slow_trace_summary() {
        let state = test_state().await;

        state
            .slow_trace_collector
            .record(
                "a1",
                "agent-1",
                kias_monitor::ActionCategory::LlmInference,
                "slow-1",
                Duration::from_millis(2000),
                None,
            )
            .await;

        let summary = slow_trace_summary(State(state)).await;
        assert_eq!(summary.total_count, 1);
        assert!(summary.by_category.contains_key("llm_inference"));
        assert!(summary.enabled);
    }

    #[tokio::test]
    async fn test_agent_slow_traces() {
        let state = test_state().await;

        state
            .slow_trace_collector
            .record(
                "a1",
                "agent-1",
                kias_monitor::ActionCategory::LlmInference,
                "action-1",
                Duration::from_millis(2000),
                None,
            )
            .await;
        state
            .slow_trace_collector
            .record(
                "a2",
                "agent-2",
                kias_monitor::ActionCategory::ToolExecution,
                "action-2",
                Duration::from_millis(3000),
                None,
            )
            .await;

        let result = agent_slow_traces(State(state), Path("a1".to_string())).await;
        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.total, 1);
        assert_eq!(result.traces[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn test_update_slow_trace_config() {
        let state = test_state().await;

        let new_config = SlowTraceConfig {
            threshold_ms: 500,
            max_traces: 500,
            enabled: true,
        };

        let result = update_slow_trace_config(State(state), Json(new_config)).await;
        assert_eq!(result.threshold_ms, 500);
        assert_eq!(result.max_traces, 500);
        assert!(result.enabled);
    }

    #[tokio::test]
    async fn test_clear_slow_traces() {
        let state = test_state().await;

        state
            .slow_trace_collector
            .record(
                "a1",
                "agent",
                kias_monitor::ActionCategory::Other,
                "action",
                Duration::from_millis(2000),
                None,
            )
            .await;
        assert_eq!(state.slow_trace_collector.count().await, 1);

        let status = clear_slow_traces(State(state)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_slow_trace_list_response_serialization() {
        let resp = SlowTraceListResponse {
            traces: vec![],
            total: 0,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["traces"].is_array());
    }

    #[tokio::test]
    async fn test_slow_trace_config_response_serialization() {
        let resp = SlowTraceConfigResponse {
            threshold_ms: 1000,
            max_traces: 1000,
            enabled: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["threshold_ms"], 1000);
        assert_eq!(json["enabled"], true);
    }
}
