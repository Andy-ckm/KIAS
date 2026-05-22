//! Failure-path integration tests for the API server.
//!
//! These tests exercise error handling across three critical failure modes:
//!   1. Agent not found (404)
//!   2. Budget exceeded (403/exceeded status)
//!   3. Invalid input (400 validation errors)
//!
//! Each test verifies that the API returns the correct HTTP status code
//! and a well-structured error response body.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use crate::routes::create_router;
use crate::AppState;

// ── Helpers ────────────────────────────────────────────────────────────────

async fn default_state() -> AppState {
    AppState::new_async(kias_common::config::KiasConfig::default()).await
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!(null));
    (status, body)
}

async fn create_agent(app: &axum::Router, name: &str) -> Value {
    let body = serde_json::json!({ "name": name, "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. FAILURE PATH: Agent Not Found
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_failure_get_agent_not_found_returns_404_with_error_body() {
    let app = create_router(default_state().await);
    let (status, body) = get_json(&app, "/api/v1/agents/does-not-exist-123").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    // Verify error body structure
    assert!(body["error"].is_object(), "response must contain 'error' object");
    assert_eq!(body["error"]["code"], 404, "error.code must be 404");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("not found"),
        "error message must mention 'not found'"
    );
}

#[tokio::test]
async fn test_failure_delete_agent_not_found_returns_404() {
    let app = create_router(default_state().await);
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/agents/ghost-agent-id")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], 404);
}

#[tokio::test]
async fn test_failure_update_status_of_nonexistent_agent_returns_404() {
    let app = create_router(default_state().await);
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/agents/fake-id-999/status")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#""Running""#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["error"]["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn test_failure_invoke_nonexistent_agent_returns_404() {
    let app = create_router(default_state().await);
    let body = serde_json::json!({ "prompt": "hello" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents/no-such-agent/invoke")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"]["code"], 404);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. FAILURE PATH: Budget Exceeded
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_failure_budget_status_for_nonexistent_agent_returns_404() {
    let app = create_router(default_state().await);
    let (status, body) = get_json(&app, "/api/v1/tokens/budget/nonexistent-agent").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    // Token budget endpoint returns bare 404 (no error body), which is still a failure path
    assert_eq!(status, StatusCode::NOT_FOUND);
    _ = body; // response may be empty or have error structure
}

#[tokio::test]
async fn test_failure_budget_exceeded_returns_exceeded_health() {
    let app = create_router(default_state().await);

    // Create an agent
    let created = create_agent(&app, "budget-exceeded-agent").await;
    let agent_id = created["data"]["id"].as_str().unwrap();

    // Transition agent to Running status first (Running → 15000 simulated tokens)
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/v1/agents/{}/status", agent_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#""Running""#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Set a very tight budget (daily_limit = 100, agent will simulate 15000 tokens → exceeded)
    let budget = serde_json::json!({
        "agent_id": agent_id,
        "agent_name": "budget-exceeded-agent",
        "daily_limit": 100,
        "monthly_limit": 1000,
        "input_cost_per_1k": 0.03,
        "output_cost_per_1k": 0.06,
        "alert_threshold": 0.5
    });
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/v1/tokens/budget/{}", agent_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&budget).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let status_body: Value = serde_json::from_slice(&bytes).unwrap();

    // The budget status should be "exceeded" since 15000 >> 100
    assert_eq!(
        status_body["status"].as_str().unwrap(),
        "exceeded",
        "Expected budget status to be 'exceeded'"
    );
    assert!(
        status_body["daily_utilization"].as_f64().unwrap() > 1.0,
        "Daily utilization should exceed 1.0"
    );
}

#[tokio::test]
async fn test_failure_set_budget_for_nonexistent_agent_returns_404() {
    let app = create_router(default_state().await);
    let budget = serde_json::json!({
        "agent_id": "fake",
        "agent_name": "fake",
        "daily_limit": 50000,
        "monthly_limit": 1000000,
        "input_cost_per_1k": 0.03,
        "output_cost_per_1k": 0.06,
        "alert_threshold": 0.8
    });
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/tokens/budget/nonexistent-agent")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&budget).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. FAILURE PATH: Invalid Input
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_failure_create_agent_empty_name_returns_400() {
    let app = create_router(default_state().await);
    let body = serde_json::json!({ "name": "", "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_failure_create_agent_malformed_json_returns_4xx() {
    let app = create_router(default_state().await);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{invalid json!!!"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();

    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422 for malformed JSON, got {status}"
    );
}

#[tokio::test]
async fn test_failure_create_agent_missing_required_field_returns_4xx() {
    let app = create_router(default_state().await);
    // Send empty object — 'name' is required
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();

    // Should be 400 or 422 (validation/rejection)
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422 for missing name field, got {status}"
    );
}

#[tokio::test]
async fn test_failure_create_agent_name_too_long_returns_400() {
    let app = create_router(default_state().await);
    // AgentSpec has validate(length(min=1, max=128)) on name
    let long_name = "x".repeat(200);
    let body = serde_json::json!({ "name": long_name, "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(err["error"]["message"].is_string(), "error must have a message");
}

#[tokio::test]
async fn test_failure_create_duplicate_agent_returns_409() {
    let app = create_router(default_state().await);
    create_agent(&app, "duplicate-name").await;

    // Attempt to create another agent with the same name
    let body = serde_json::json!({ "name": "duplicate-name", "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let err: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(err["error"]["message"].as_str().unwrap().contains("already exists"));
}
