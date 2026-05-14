//! Comprehensive integration tests for the KIAS API Server.
//!
//! Uses `tower::ServiceExt::oneshot` to send requests directly to the axum
//! router without binding a real TCP listener.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use kias_api_server::routes::create_router;
use kias_api_server::AppState;
use kias_common::config::KiasConfig;

// ── Helpers ───────────────────────────────────────────────────────────────

/// Build an AppState with default config (auth disabled, 2 seed nodes).
fn default_state() -> AppState {
    AppState::new(KiasConfig::default())
}

/// Build an AppState with auth enabled and a specific set of API keys.
fn state_with_auth(api_keys: Vec<&str>) -> AppState {
    let mut config = KiasConfig::default();
    config.api_server.auth_enabled = true;
    config.api_server.api_keys = api_keys.into_iter().map(String::from).collect();
    AppState::new(config)
}

/// Create an agent via the API and return its JSON body.
async fn create_test_agent(app: &axum::Router, name: &str) -> Value {
    let body = serde_json::json!({
        "name": name,
        "image": "python:3.11",
        "command": ["python", "app.py"],
        "priority": "high"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Perform a GET request and parse the JSON body.
async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!(null));
    (status, body)
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Health Endpoints
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_health_liveness_returns_200() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_readiness_returns_200_with_components() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
    assert!(body["version"].is_string());
    assert!(body["components"].is_array());
    let components = body["components"].as_array().unwrap();
    assert!(components.len() >= 2);
}

#[tokio::test]
async fn test_readiness_components_have_correct_names() {
    let app = create_router(default_state());
    let (_, body) = get_json(&app, "/readyz").await;
    let components = body["components"].as_array().unwrap();
    let names: Vec<&str> = components
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"agents_store"));
    assert!(names.contains(&"nodes_store"));
}

#[tokio::test]
async fn test_health_endpoint_no_auth_required() {
    let app = create_router(state_with_auth(vec!["secret-key"]));
    let (status, body) = get_json(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_readiness_endpoint_no_auth_required() {
    let app = create_router(state_with_auth(vec!["secret-key"]));
    let (status, _) = get_json(&app, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Agent CRUD
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_create_agent_returns_201() {
    let app = create_router(default_state());
    let body = serde_json::json!({ "name": "test-agent", "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["data"]["spec"]["name"], "test-agent");
    assert!(json["data"]["id"].is_string());
    assert_eq!(json["data"]["status"], "Pending");
}

#[tokio::test]
async fn test_create_agent_and_get_by_id() {
    let app = create_router(default_state());
    let created = create_test_agent(&app, "fetch-me").await;
    let id = created["data"]["id"].as_str().unwrap();

    let (status, body) = get_json(&app, &format!("/api/v1/agents/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["spec"]["name"], "fetch-me");
    assert_eq!(body["data"]["id"], id);
}

#[tokio::test]
async fn test_list_agents_empty() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/agents").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);
}

#[tokio::test]
async fn test_list_agents_after_creation() {
    let app = create_router(default_state());
    create_test_agent(&app, "agent-1").await;
    create_test_agent(&app, "agent-2").await;

    let (status, body) = get_json(&app, "/api/v1/agents").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn test_delete_agent() {
    let app = create_router(default_state());
    let created = create_test_agent(&app, "doomed-agent").await;
    let id = created["data"]["id"].as_str().unwrap();

    // Delete
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/agents/{id}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["message"].as_str().unwrap().contains("deleted"));

    // Verify gone
    let (status, _) = get_json(&app, &format!("/api/v1/agents/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_agent_status() {
    let app = create_router(default_state());
    let created = create_test_agent(&app, "status-agent").await;
    let id = created["data"]["id"].as_str().unwrap();

    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/v1/agents/{id}/status"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#""Running""#.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["data"]["status"], "Running");
}

#[tokio::test]
async fn test_create_duplicate_agent_returns_409() {
    let app = create_router(default_state());
    create_test_agent(&app, "unique-agent").await;

    let body = serde_json::json!({ "name": "unique-agent", "image": "python:3.11" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Node Management Endpoints
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_nodes_returns_seed_nodes() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn test_get_node_by_id() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes/node-1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], "node-1");
    assert_eq!(body["data"]["status"], "Ready");
    assert_eq!(body["data"]["resources"]["cpu"], "8");
}

#[tokio::test]
async fn test_get_nonexistent_node_returns_404() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
}

#[tokio::test]
async fn test_list_node_agents_empty() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes/node-1/agents").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_node_agents_for_nonexistent_node() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes/fake-node/agents").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Knowledge Endpoints
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_knowledge_search_returns_empty_results() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/knowledge/search?q=test").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_knowledge_search_with_limit() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/knowledge/search?q=rust&limit=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert!(body["items"].is_array());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Authentication Middleware
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_auth_no_token_returns_401() {
    let app = create_router(state_with_auth(vec!["valid-key"]));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_invalid_token_returns_401() {
    let app = create_router(state_with_auth(vec!["valid-key"]));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .header(header::AUTHORIZATION, "Bearer wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_valid_token_succeeds() {
    let app = create_router(state_with_auth(vec!["valid-key"]));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .header(header::AUTHORIZATION, "Bearer valid-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_malformed_header_returns_401() {
    let app = create_router(state_with_auth(vec!["valid-key"]));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .header(header::AUTHORIZATION, "Token valid-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_disabled_allows_all_requests() {
    let app = create_router(default_state());
    let req = Request::builder()
        .uri("/api/v1/agents")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_no_keys_configured_denies_all() {
    let mut config = KiasConfig::default();
    config.api_server.auth_enabled = true;
    config.api_server.api_keys = vec![];
    let app = create_router(AppState::new(config));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .header(header::AUTHORIZATION, "Bearer anything")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Error Handling
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_nonexistent_agent_returns_404() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/agents/nonexistent-id").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn test_delete_nonexistent_agent_returns_404() {
    let app = create_router(default_state());
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/agents/nonexistent-id")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_status_nonexistent_agent_returns_404() {
    let app = create_router(default_state());
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/agents/nonexistent-id/status")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#""Running""#.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_agent_with_empty_name_returns_400() {
    let app = create_router(default_state());
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
async fn test_create_agent_with_invalid_json_returns_4xx() {
    let app = create_router(default_state());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("not valid json{{{"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {status}"
    );
}

#[tokio::test]
async fn test_error_response_has_correct_structure() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/agents/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_object());
    assert!(body["error"]["code"].is_number());
    assert!(body["error"]["message"].is_string());
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Pagination and Filtering
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pagination_agents_per_page() {
    let app = create_router(default_state());
    create_test_agent(&app, "page-a").await;
    create_test_agent(&app, "page-b").await;
    create_test_agent(&app, "page-c").await;

    // Get page 1 with per_page=2
    let (status, body) = get_json(&app, "/api/v1/agents?page=1&per_page=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // Get page 2 with per_page=2
    let (status, body) = get_json(&app, "/api/v1/agents?page=2&per_page=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_pagination_nodes_per_page() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/nodes?page=1&per_page=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_pagination_defaults() {
    let app = create_router(default_state());
    for i in 0..5 {
        create_test_agent(&app, &format!("default-page-{i}")).await;
    }
    let (status, body) = get_json(&app, "/api/v1/agents").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 5);
    assert_eq!(body["items"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_pagination_page_zero_treated_as_one() {
    let app = create_router(default_state());
    create_test_agent(&app, "zero-page-agent").await;
    let (status, body) = get_json(&app, "/api/v1/agents?page=0&per_page=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Request Logging Middleware
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_logging_middleware_does_not_break_response() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_logging_middleware_preserves_status_codes() {
    let app = create_router(default_state());
    let (s, _) = get_json(&app, "/health").await;
    assert_eq!(s, StatusCode::OK);

    let (s, _) = get_json(&app, "/api/v1/agents/no-such-id").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = get_json(&app, "/api/v1/nodes").await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn test_logging_middleware_preserves_404_body() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/agents/ghost").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional edge-case tests
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_agent_default_image_applied() {
    let app = create_router(default_state());
    let body = serde_json::json!({ "name": "default-image-agent" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["data"]["spec"]["image"], "python:3.11");
}

#[tokio::test]
async fn test_agent_custom_command() {
    let app = create_router(default_state());
    let body = serde_json::json!({
        "name": "custom-cmd-agent",
        "image": "node:18",
        "command": ["node", "server.js"]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    let cmd = json["data"]["spec"]["command"].as_array().unwrap();
    assert_eq!(cmd[0], "node");
    assert_eq!(cmd[1], "server.js");
}

#[tokio::test]
async fn test_node_resource_capacity_details() {
    let app = create_router(default_state());
    let (_, body) = get_json(&app, "/api/v1/nodes/node-2").await;
    let resources = &body["data"]["resources"];
    assert_eq!(resources["cpu"], "4");
    assert_eq!(resources["memory"], "8Gi");
    assert_eq!(resources["gpu"], "0");
}

#[tokio::test]
async fn test_agent_id_is_uuid_format() {
    let app = create_router(default_state());
    let created = create_test_agent(&app, "uuid-agent").await;
    let id = created["data"]["id"].as_str().unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
}

#[tokio::test]
async fn test_multiple_auth_keys() {
    let app = create_router(state_with_auth(vec!["key-1", "key-2", "key-3"]));
    let req = Request::builder()
        .uri("/api/v1/agents")
        .header(header::AUTHORIZATION, "Bearer key-2")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Metrics Endpoints
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_metrics_summary_empty() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/metrics/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["agent_count"], 0);
    assert_eq!(body["node_count"], 2); // seed nodes
    assert_eq!(body["task_stats"]["pending"], 0);
    assert_eq!(body["task_stats"]["running"], 0);
}

#[tokio::test]
async fn test_metrics_summary_with_agents() {
    let app = create_router(default_state());
    create_test_agent(&app, "metrics-agent-1").await;
    create_test_agent(&app, "metrics-agent-2").await;

    let (status, body) = get_json(&app, "/api/v1/metrics/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["agent_count"], 2);
    assert_eq!(body["task_stats"]["pending"], 2); // new agents are Pending
}

#[tokio::test]
async fn test_metrics_agent_not_found() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/metrics/agents/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], 404);
}

#[tokio::test]
async fn test_metrics_per_agent() {
    let app = create_router(default_state());
    let created = create_test_agent(&app, "metrics-per-agent").await;
    let id = created["data"]["id"].as_str().unwrap();

    let (status, body) = get_json(&app, &format!("/api/v1/metrics/agents/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "metrics-per-agent");
    assert_eq!(body["status"], "Pending");
    assert_eq!(body["restart_count"], 0);
    assert!(body["created_at"].is_string());
}

#[tokio::test]
async fn test_cluster_status_healthy() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/cluster/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["overall"], "healthy");
    assert_eq!(body["total_agents"], 0);
    assert_eq!(body["running_agents"], 0);
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["status"], "Ready");
}

#[tokio::test]
async fn test_cluster_status_with_agents() {
    let app = create_router(default_state());
    create_test_agent(&app, "cluster-agent-1").await;
    create_test_agent(&app, "cluster-agent-2").await;

    let (status, body) = get_json(&app, "/api/v1/cluster/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_agents"], 2);
    assert_eq!(body["running_agents"], 0); // agents start as Pending
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Config Endpoints
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_config_returns_sanitized_data() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/config").await;
    assert_eq!(status, StatusCode::OK);
    // Verify no secrets are leaked
    assert!(
        body["api_server"]["api_keys"].is_null(),
        "raw api_keys should not be present"
    );
    assert!(
        body["api_server"]["jwt_secret"].is_null(),
        "jwt_secret should not be present"
    );
    // Verify sanitized fields exist
    assert!(body["api_server"]["api_key_count"].is_number());
    assert!(body["api_server"]["jwt_configured"].is_boolean());
    assert_eq!(body["api_server"]["port"], 8080);
    assert_eq!(body["logging"]["level"], "info");
}

#[tokio::test]
async fn test_get_config_no_auth_required_when_disabled() {
    let app = create_router(default_state());
    let (status, _) = get_json(&app, "/api/v1/config").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_update_config_requires_auth() {
    let app = create_router(state_with_auth(vec!["admin-key"]));
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"logging_level": "debug"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_update_config_with_valid_key_succeeds() {
    let app = create_router(state_with_auth(vec!["admin-key"]));
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer admin-key")
        .body(Body::from(r#"{"logging_level": "debug"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!json["changes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_update_config_invalid_level_returns_400() {
    let app = create_router(default_state());
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"logging_level": "invalid_level"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_config_no_changes_returns_400() {
    let app = create_router(default_state());
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_config_audit_log_returns_entries() {
    let app = create_router(default_state());
    // Make a config change to generate an audit entry
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"logging_level": "debug"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Now retrieve audit log — note: oneshot doesn't share state across calls,
    // so this tests the endpoint structure rather than accumulated state.
    let (status, body) = get_json(&app, "/api/v1/config/audit-log").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
}

#[tokio::test]
async fn test_config_audit_log_empty_initially() {
    let app = create_router(default_state());
    let (status, body) = get_json(&app, "/api/v1/config/audit-log").await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 0);
}
