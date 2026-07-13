#!/usr/bin/env python3
"""Apply integration-contract fixes using structural function replacement."""

from pathlib import Path
import re


def replace_exact(path_name: str, old: str, new: str, label: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_test_function(text: str, name: str, replacement: str) -> str:
    pattern = re.compile(
        rf"#\[tokio::test\]\nasync fn {re.escape(name)}\(\) \{{.*?\n\}}\n",
        re.DOTALL,
    )
    updated, count = pattern.subn(replacement.rstrip() + "\n", text, count=1)
    if count != 1:
        raise SystemExit(f"integration test {name}: expected one function, found {count}")
    return updated


def main() -> None:
    replace_exact(
        "crates/api-server/src/middleware/auth.rs",
        "    // Skip auth if not enabled\n"
        "    if !state.config.api_server.auth_enabled {\n"
        "        return Ok(next.run(request).await);\n"
        "    }\n",
        "    // Explicitly disabling authentication is a compatibility mode for\n"
        "    // loopback development and tests. Attach Admin claims so the\n"
        "    // downstream RBAC layer does not contradict that operator choice.\n"
        "    if !state.config.api_server.auth_enabled {\n"
        "        request.extensions_mut().insert(Claims {\n"
        "            sub: \"auth-disabled-local\".to_string(),\n"
        "            role: Role::Admin,\n"
        "            iat: 0,\n"
        "            exp: u64::MAX,\n"
        "            iss: \"kias-auth-disabled\".to_string(),\n"
        "        });\n"
        "        return Ok(next.run(request).await);\n"
        "    }\n",
        "auth-disabled RBAC bridge",
    )

    integration = Path("crates/api-server/tests/integration.rs")
    text = integration.read_text(encoding="utf-8")
    helper_pattern = re.compile(
        r"use kias_api_server::routes::create_router;\n"
        r"use kias_api_server::AppState;\n"
        r"use kias_common::config::KiasConfig;\n"
        r"\n"
        r"// ── Helpers .*?\n"
        r"\n"
        r"/// Build an AppState with default config .*?\n"
        r"async fn default_state\(\) -> AppState \{.*?\n\}\n"
        r"\n"
        r"/// Build an AppState with auth enabled .*?\n"
        r"async fn state_with_auth\(auth_tokens: Vec<&str>\) -> AppState \{.*?\n\}\n",
        re.DOTALL,
    )
    helper_replacement = '''use kias_api_server::models::node::{Node, NodeStatus, ResourceCapacity};
use kias_api_server::routes::create_router_with_surfaces;
use kias_api_server::surfaces::SurfaceConfig;
use kias_api_server::AppState;
use kias_common::config::KiasConfig;

// ── Helpers ───────────────────────────────────────────────────────────────

fn legacy_test_surfaces() -> SurfaceConfig {
    SurfaceConfig {
        knowledge: true,
        context: true,
        a2a: true,
        tier_routing: true,
        realtime: true,
        direct_execution: true,
        nl_commands: true,
        im: true,
        visualization: true,
        dev_fixtures: false,
    }
}

fn create_router(state: AppState) -> axum::Router {
    create_router_with_surfaces(state, legacy_test_surfaces())
}

async fn state_from_config(config: KiasConfig) -> AppState {
    let state = AppState::new_async(config).await;
    let now = chrono::Utc::now().to_rfc3339();
    let mut nodes = state.nodes.write().await;
    for (id, cpu, memory, gpu) in [
        ("node-1", "8", "16Gi", "1"),
        ("node-2", "4", "8Gi", "0"),
    ] {
        nodes.insert(
            id.to_string(),
            Node {
                id: id.to_string(),
                name: id.to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: cpu.to_string(),
                    memory: memory.to_string(),
                    gpu: gpu.to_string(),
                },
                allocatable: ResourceCapacity {
                    cpu: cpu.to_string(),
                    memory: memory.to_string(),
                    gpu: gpu.to_string(),
                },
                labels: Default::default(),
                created_at: now.clone(),
                last_heartbeat: now.clone(),
            },
        );
    }
    drop(nodes);
    state
}

/// Explicit legacy fixture: auth disabled, all optional surfaces, two nodes.
async fn default_state() -> AppState {
    let mut config = KiasConfig::default();
    config.api_server.auth_enabled = false;
    state_from_config(config).await
}

/// Auth-enabled legacy fixture with the same full route surface.
async fn state_with_auth(auth_tokens: Vec<&str>) -> AppState {
    let mut config = KiasConfig::default();
    config.api_server.auth_enabled = true;
    config.api_server.auth_tokens = auth_tokens.into_iter().map(String::from).collect();
    state_from_config(config).await
}
'''
    text, count = helper_pattern.subn(helper_replacement, text, count=1)
    if count != 1:
        raise SystemExit(f"integration helpers: expected one block, found {count}")

    text = replace_test_function(
        text,
        "test_update_config_with_valid_key_succeeds",
        '''#[tokio::test]
async fn test_runtime_config_mutation_is_not_exposed_even_to_admin() {
    let app = create_router(state_with_auth(vec!["admin-key"]).await);
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer admin-key")
        .body(Body::from(r#"{"logging_level": "debug"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}''',
    )
    text = replace_test_function(
        text,
        "test_update_config_invalid_level_returns_400",
        '''#[tokio::test]
async fn test_invalid_runtime_config_mutation_is_not_routed() {
    let app = create_router(default_state().await);
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"logging_level": "invalid_level"}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}''',
    )
    text = replace_test_function(
        text,
        "test_update_config_no_changes_returns_400",
        '''#[tokio::test]
async fn test_empty_runtime_config_mutation_is_not_routed() {
    let app = create_router(default_state().await);
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/config")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{}"#))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}''',
    )
    text = replace_test_function(
        text,
        "test_config_audit_log_returns_entries",
        '''#[tokio::test]
async fn test_config_audit_log_endpoint_remains_read_only() {
    let app = create_router(default_state().await);
    let (status, body) = get_json(&app, "/api/v1/config/audit-log").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
}''',
    )

    integration.write_text(text, encoding="utf-8")
    Path("scripts/export_ci_integration_contract_fixes.py").unlink(missing_ok=True)
    Path("scripts/export_ci_integration_contract_fixes_v2.py").unlink()


if __name__ == "__main__":
    main()
