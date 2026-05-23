//! API Smoke Tests - 快速验证核心功能
//!
//! 这些测试用于快速验证API的基本功能是否正常，
//! 适合在部署前或CI/CD流水线中运行。

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use kias_api_server::routes::create_router;
use kias_api_server::AppState;
use kias_common::config::KiasConfig;

/// 快速构建AppState
async fn smoke_state() -> AppState {
    AppState::new_async(KiasConfig::default()).await
}

/// Smoke Test 1: 健康检查
#[tokio::test]
async fn smoke_health_check() {
    let app = create_router(smoke_state().await);
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "健康检查端点应返回200");
}

/// Smoke Test 2: 就绪检查
#[tokio::test]
async fn smoke_readiness_check() {
    let app = create_router(smoke_state().await);
    let req = Request::builder()
        .uri("/readyz")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "就绪检查端点应返回200");

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["status"].is_string(), "应返回status字段");
}

/// Smoke Test 3: 创建Agent
#[tokio::test]
async fn smoke_create_agent() {
    let app = create_router(smoke_state().await);
    let body = serde_json::json!({
        "name": "smoke-test-agent",
        "image": "python:3.11",
        "command": ["echo", "hello"],
        "priority": "medium"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "创建Agent应返回201");

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["data"]["id"].is_string(), "应返回Agent ID");
    assert_eq!(
        json["data"]["spec"]["name"], "smoke-test-agent",
        "名称应匹配"
    );
}

/// Smoke Test 4: 获取Agent列表
#[tokio::test]
async fn smoke_list_agents() {
    let state = smoke_state().await;
    let app = create_router(state.clone());

    // 先创建一个Agent
    let body = serde_json::json!({
        "name": "list-test-agent",
        "image": "node:18",
        "command": ["node", "server.js"],
        "priority": "low"
    });

    let create_req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // 获取列表
    let list_req = Request::builder()
        .uri("/api/v1/agents")
        .body(Body::empty())
        .unwrap();

    let list_resp = app.oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK, "获取Agent列表应返回200");

    let bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["items"].is_array(), "应返回items数组");
    assert!(json["total"].is_u64(), "应返回total字段");
}

/// Smoke Test 5: 错误处理 - 无效JSON
#[tokio::test]
async fn smoke_invalid_json_returns_400() {
    let app = create_router(smoke_state().await);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agents")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "无效JSON应返回400");
}

/// Smoke Test 6: 不存在的端点返回404
#[tokio::test]
async fn smoke_unknown_endpoint_returns_404() {
    let app = create_router(smoke_state().await);
    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "未知端点应返回404");
}

/// Smoke Test 7: 认证保护
#[tokio::test]
async fn smoke_auth_required_for_config() {
    let mut config = KiasConfig::default();
    config.api_server.auth_enabled = true;
    config.api_server.auth_tokens = vec!["valid-k".to_string()];

    let app = create_router(AppState::new_async(config).await);

    // 无认证请求
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"log_level": "debug"}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "无认证应返回401");
}
