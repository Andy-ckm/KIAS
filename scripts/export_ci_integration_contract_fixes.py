#!/usr/bin/env python3
"""Repair auth-disabled compatibility and modernize the legacy full-surface suite."""

from pathlib import Path


def replace_once(path_name: str, old: str, new: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path_name}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    replace_once(
        "crates/api-server/src/middleware/auth.rs",
        '    // Skip auth if not enabled\n'
        '    if !state.config.api_server.auth_enabled {\n'
        '        return Ok(next.run(request).await);\n'
        '    }\n',
        '    // Explicitly disabling authentication is a compatibility mode for\n'
        '    // loopback development and tests. Attach Admin claims so the\n'
        '    // downstream RBAC layer does not contradict that operator choice.\n'
        '    if !state.config.api_server.auth_enabled {\n'
        '        request.extensions_mut().insert(Claims {\n'
        '            sub: "auth-disabled-local".to_string(),\n'
        '            role: Role::Admin,\n'
        '            iat: 0,\n'
        '            exp: u64::MAX,\n'
        '            iss: "kias-auth-disabled".to_string(),\n'
        '        });\n'
        '        return Ok(next.run(request).await);\n'
        '    }\n',
    )

    replace_once(
        "crates/api-server/tests/integration.rs",
        'use kias_api_server::routes::create_router;\n'
        'use kias_api_server::AppState;\n'
        'use kias_common::config::KiasConfig;\n'
        '\n'
        '// ── Helpers ───────────────────────────────────────────────────────────────\n'
        '\n'
        '/// Build an AppState with default config (auth disabled, 2 seed nodes).\n'
        'async fn default_state() -> AppState {\n'
        '    AppState::new_async(KiasConfig::default()).await\n'
        '}\n'
        '\n'
        '/// Build an AppState with auth enabled and a specific set of API keys.\n'
        'async fn state_with_auth(auth_tokens: Vec<&str>) -> AppState {\n'
        '    let mut config = KiasConfig::default();\n'
        '    config.api_server.auth_enabled = true;\n'
        '    config.api_server.auth_tokens = auth_tokens.into_iter().map(String::from).collect();\n'
        '    AppState::new_async(config).await\n'
        '}\n',
        'use kias_api_server::models::node::{Node, NodeStatus, ResourceCapacity};\n'
        'use kias_api_server::routes::create_router_with_surfaces;\n'
        'use kias_api_server::surfaces::SurfaceConfig;\n'
        'use kias_api_server::AppState;\n'
        'use kias_common::config::KiasConfig;\n'
        '\n'
        '// ── Helpers ───────────────────────────────────────────────────────────────\n'
        '\n'
        'fn legacy_test_surfaces() -> SurfaceConfig {\n'
        '    SurfaceConfig {\n'
        '        knowledge: true,\n'
        '        context: true,\n'
        '        a2a: true,\n'
        '        tier_routing: true,\n'
        '        realtime: true,\n'
        '        direct_execution: true,\n'
        '        nl_commands: true,\n'
        '        im: true,\n'
        '        visualization: true,\n'
        '        dev_fixtures: false,\n'
        '    }\n'
        '}\n'
        '\n'
        'fn create_router(state: AppState) -> axum::Router {\n'
        '    create_router_with_surfaces(state, legacy_test_surfaces())\n'
        '}\n'
        '\n'
        'async fn state_from_config(config: KiasConfig) -> AppState {\n'
        '    let state = AppState::new_async(config).await;\n'
        '    let now = chrono::Utc::now().to_rfc3339();\n'
        '    let mut nodes = state.nodes.write().await;\n'
        '    for (id, cpu, memory, gpu) in [("node-1", "8", "16Gi", "1"), ("node-2", "4", "8Gi", "0")] {\n'
        '        nodes.insert(\n'
        '            id.to_string(),\n'
        '            Node {\n'
        '                id: id.to_string(),\n'
        '                name: id.to_string(),\n'
        '                status: NodeStatus::Ready,\n'
        '                resources: ResourceCapacity {\n'
        '                    cpu: cpu.to_string(),\n'
        '                    memory: memory.to_string(),\n'
        '                    gpu: gpu.to_string(),\n'
        '                },\n'
        '                allocatable: ResourceCapacity {\n'
        '                    cpu: cpu.to_string(),\n'
        '                    memory: memory.to_string(),\n'
        '                    gpu: gpu.to_string(),\n'
        '                },\n'
        '                labels: Default::default(),\n'
        '                created_at: now.clone(),\n'
        '                last_heartbeat: now.clone(),\n'
        '            },\n'
        '        );\n'
        '    }\n'
        '    drop(nodes);\n'
        '    state\n'
        '}\n'
        '\n'
        '/// Build an explicit legacy fixture: auth disabled, all surfaces, two nodes.\n'
        'async fn default_state() -> AppState {\n'
        '    let mut config = KiasConfig::default();\n'
        '    config.api_server.auth_enabled = false;\n'
        '    state_from_config(config).await\n'
        '}\n'
        '\n'
        '/// Build an AppState with auth enabled and a specific set of API keys.\n'
        'async fn state_with_auth(auth_tokens: Vec<&str>) -> AppState {\n'
        '    let mut config = KiasConfig::default();\n'
        '    config.api_server.auth_enabled = true;\n'
        '    config.api_server.auth_tokens = auth_tokens.into_iter().map(String::from).collect();\n'
        '    state_from_config(config).await\n'
        '}\n',
    )

    replace_once(
        "crates/api-server/tests/integration.rs",
        '#[tokio::test]\n'
        'async fn test_update_config_with_valid_key_succeeds() {\n'
        '    let app = create_router(state_with_auth(vec!["admin-key"]).await);\n'
        '    let req = Request::builder()\n'
        '        .method("PATCH")\n'
        '        .uri("/api/v1/config")\n'
        '        .header(header::CONTENT_TYPE, "application/json")\n'
        '        .header(header::AUTHORIZATION, "Bearer admin-key")\n'
        '        .body(Body::from(r#"{"logging_level": "debug"}"#))\n'
        '        .unwrap();\n'
        '    let resp = app.clone().oneshot(req).await.unwrap();\n'
        '    assert_eq!(resp.status(), StatusCode::OK);\n'
        '    let bytes = resp.into_body().collect().await.unwrap().to_bytes();\n'
        '    let json: Value = serde_json::from_slice(&bytes).unwrap();\n'
        '    assert!(!json["changes"].as_array().unwrap().is_empty());\n'
        '}\n',
        '#[tokio::test]\n'
        'async fn test_runtime_config_mutation_is_not_exposed_even_to_admin() {\n'
        '    let app = create_router(state_with_auth(vec!["admin-key"]).await);\n'
        '    let req = Request::builder()\n'
        '        .method("PATCH")\n'
        '        .uri("/api/v1/config")\n'
        '        .header(header::CONTENT_TYPE, "application/json")\n'
        '        .header(header::AUTHORIZATION, "Bearer admin-key")\n'
        '        .body(Body::from(r#"{"logging_level": "debug"}"#))\n'
        '        .unwrap();\n'
        '    let resp = app.clone().oneshot(req).await.unwrap();\n'
        '    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);\n'
        '}\n',
    )

    for old_name, body in (
        (
            "test_update_config_invalid_level_returns_400",
            r#"{"logging_level": "invalid_level"}"#,
        ),
        ("test_update_config_no_changes_returns_400", r#"{}"#),
    ):
        old = (
            '#[tokio::test]\n'
            f'async fn {old_name}() {{\n'
            '    let app = create_router(default_state().await);\n'
            '    let req = Request::builder()\n'
            '        .method("PATCH")\n'
            '        .uri("/api/v1/config")\n'
            '        .header(header::CONTENT_TYPE, "application/json")\n'
            f'        .body(Body::from({body}))\n'
            '        .unwrap();\n'
            '    let resp = app.clone().oneshot(req).await.unwrap();\n'
            '    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);\n'
            '}\n'
        )
        new = (
            '#[tokio::test]\n'
            f'async fn {old_name.replace("returns_400", "is_not_routed")}() {{\n'
            '    let app = create_router(default_state().await);\n'
            '    let req = Request::builder()\n'
            '        .method("PATCH")\n'
            '        .uri("/api/v1/config")\n'
            '        .header(header::CONTENT_TYPE, "application/json")\n'
            f'        .body(Body::from({body}))\n'
            '        .unwrap();\n'
            '    let resp = app.clone().oneshot(req).await.unwrap();\n'
            '    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);\n'
            '}\n'
        )
        replace_once("crates/api-server/tests/integration.rs", old, new)

    replace_once(
        "crates/api-server/tests/integration.rs",
        '#[tokio::test]\n'
        'async fn test_config_audit_log_returns_entries() {\n'
        '    let app = create_router(default_state().await);\n'
        '    // Make a config change to generate an audit entry\n'
        '    let req = Request::builder()\n'
        '        .method("PATCH")\n'
        '        .uri("/api/v1/config")\n'
        '        .header(header::CONTENT_TYPE, "application/json")\n'
        '        .body(Body::from(r#"{"logging_level": "debug"}"#))\n'
        '        .unwrap();\n'
        '    let resp = app.clone().oneshot(req).await.unwrap();\n'
        '    assert_eq!(resp.status(), StatusCode::OK);\n'
        '\n'
        '    // Now retrieve audit log — note: oneshot doesn\'t share state across calls,\n'
        '    // so this tests the endpoint structure rather than accumulated state.\n'
        '    let (status, body) = get_json(&app, "/api/v1/config/audit-log").await;\n'
        '    assert_eq!(status, StatusCode::OK);\n'
        '    assert!(body.is_array());\n'
        '}\n',
        '#[tokio::test]\n'
        'async fn test_config_audit_log_endpoint_remains_read_only() {\n'
        '    let app = create_router(default_state().await);\n'
        '    let (status, body) = get_json(&app, "/api/v1/config/audit-log").await;\n'
        '    assert_eq!(status, StatusCode::OK);\n'
        '    assert!(body.is_array());\n'
        '}\n',
    )

    Path("scripts/export_ci_integration_contract_fixes.py").unlink()


if __name__ == "__main__":
    main()
