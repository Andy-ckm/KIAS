#!/usr/bin/env python3
"""Align stale tests with the hardened product contract, then remove this helper."""

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
        "crates/api-server/src/handlers/config.rs",
        '        assert_eq!(result.scheduler.algorithm, "cache_aware");\n',
        '        assert_eq!(result.scheduler.algorithm, "resource_aware");\n',
    )

    replace_once(
        "crates/api-server/src/handlers/dashboard.rs",
        '        assert_eq!(result.nodes.total, 2); // seeded nodes\n'
        '        assert_eq!(result.nodes.ready, 2);\n',
        '        assert_eq!(result.nodes.total, 0); // production defaults do not fabricate nodes\n'
        '        assert_eq!(result.nodes.ready, 0);\n',
    )
    replace_once(
        "crates/api-server/src/handlers/dashboard.rs",
        '    async fn test_dashboard_node_summary() {\n'
        '        let state = test_state().await;\n'
        '        let result = realtime_dashboard(State(state)).await;\n',
        '    async fn test_dashboard_node_summary() {\n'
        '        let state = test_state().await;\n'
        '        *state.nodes.write().await = crate::synthetic_nodes();\n'
        '        let result = realtime_dashboard(State(state)).await;\n',
    )

    replace_once(
        "crates/api-server/src/routes/product.rs",
        '    async fn test_state() -> AppState {\n'
        '        AppState::new(kias_common::config::KiasConfig::default()).await\n'
        '    }\n',
        '    async fn test_state() -> AppState {\n'
        '        let mut config = kias_common::config::KiasConfig::default();\n'
        '        config.api_server.auth_enabled = false;\n'
        '        AppState::new(config).await\n'
        '    }\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '    async fn core_profile_exposes_basic_probe_and_capability_contract() {\n'
        '        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());\n',
        '    async fn core_profile_exposes_basic_probe_and_capability_contract() {\n'
        '        let app =\n'
        '            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '                    .uri("/api/v1/system/capabilities")\n'
        '                    .body(Body::empty())\n',
        '                    .uri("/api/v1/system/capabilities")\n'
        '                    .header(\n'
        '                        AUTHORIZATION,\n'
        '                        format!("Bearer {}", bearer(crate::auth::Role::Viewer)),\n'
        '                    )\n'
        '                    .body(Body::empty())\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '    async fn optional_and_labs_routes_are_absent_by_default() {\n'
        '        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());\n',
        '    async fn optional_and_labs_routes_are_absent_by_default() {\n'
        '        let app =\n'
        '            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())\n',
        '                .oneshot(\n'
        '                    Request::builder()\n'
        '                        .uri(path)\n'
        '                        .header(\n'
        '                            AUTHORIZATION,\n'
        '                            format!("Bearer {}", bearer(crate::auth::Role::Viewer)),\n'
        '                        )\n'
        '                        .body(Body::empty())\n'
        '                        .unwrap(),\n'
        '                )\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '    async fn fake_runtime_config_mutation_is_not_advertised() {\n'
        '        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());\n',
        '    async fn fake_runtime_config_mutation_is_not_advertised() {\n'
        '        let app =\n'
        '            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());\n',
    )
    replace_once(
        "crates/api-server/src/routes/product.rs",
        '                    .method("PATCH")\n'
        '                    .uri("/api/v1/config")\n'
        '                    .body(Body::empty())\n',
        '                    .method("PATCH")\n'
        '                    .uri("/api/v1/config")\n'
        '                    .header(\n'
        '                        AUTHORIZATION,\n'
        '                        format!("Bearer {}", bearer(crate::auth::Role::Admin)),\n'
        '                    )\n'
        '                    .body(Body::empty())\n',
    )

    Path("scripts/export_ci_test_contract_fixes.py").unlink()


if __name__ == "__main__":
    main()
