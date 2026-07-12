#!/usr/bin/env python3
"""Apply the reviewed Core quality and SQLite dependency fixes.

Every replacement has an exact expected match count. The script exits before
formatting, testing, or committing if repository contents differ from the
reviewed input.
"""

from pathlib import Path


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} matches, found {actual}: {old[:80]!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


def main() -> None:
    replace_exact(
        "crates/tool-executor/src/builtin.rs",
        '''        let result = tool.execute(serde_json::json!({"command": ""})).await;
        // Empty command should not panic - just return a result
        assert!(true); // Always true - just verifying no panic
''',
        '''        let result = tool.execute(serde_json::json!({"command": ""})).await;
        assert!(result.success);
        assert_eq!(
            result
                .metadata
                .as_ref()
                .and_then(|value| value["exit_code"].as_i64()),
            Some(0)
        );
''',
    )
    replace_exact(
        "crates/tool-executor/src/builtin.rs",
        '''        // Test file write to a path that cannot be created (read-only location)
        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": "/root/impossible_file_12345.txt",
                "content": "test content"
            }))
            .await;
        // Should fail gracefully, not panic
        assert!(true); // Always true - just verifying no panic
''',
        '''        // /proc does not allow creating arbitrary regular files, including for root.
        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": "/proc/1/kias-impossible-file.txt",
                "content": "test content"
            }))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
''',
    )
    replace_exact(
        "crates/tool-executor/src/vulnerability_scan.rs",
        '''        let result = scanner.scan("shell", &params, &default_ctx());
        // This might not be caught since pattern is "chmod +s" not numeric
        // Just verify no panic
        // u64 is always >= 0, this assertion is always true
''',
        '''        let result = scanner.scan("shell", &params, &default_ctx());
        // Numeric setuid detection is not guaranteed yet, but the aggregate score
        // must remain internally consistent for every scan result.
        assert_eq!(result.overall_risk, result.risk.overall());
''',
    )
    replace_exact(
        "crates/controller/src/runtime_loop.rs",
        '''    // ── Helper ──────────────────────────────────────────────────────────────

    fn fast_config() -> RuntimeLoopConfig {
        RuntimeLoopConfig {
            max_rounds: 5,
            loop_timeout: Duration::from_secs(10),
            round_timeout: Duration::from_secs(5),
            quality_threshold: 0.8,
            stop_on_achieve: true,
            cooldown: Duration::ZERO,
        }
    }

''',
        "",
    )
    replace_exact(
        "crates/common/src/metrics.rs",
        '.find(|mf| mf.get_name() == "kias_scheduler_latency_seconds")',
        '.find(|mf| mf.name() == "kias_scheduler_latency_seconds")',
    )
    replace_exact(
        "crates/common/src/tls.rs",
        '''        let key = b"not a pem file";
        let err = validate_pem_files(cert, key.as_bytes()).unwrap_err();
''',
        '''        let key = b"not a pem file";
        let err = validate_pem_files(cert, key).unwrap_err();
''',
    )

    replace_exact(
        "crates/api-server/src/handlers/config.rs",
        '        assert_eq!(result.scheduler.algorithm, "cache_aware");',
        '        assert_eq!(result.scheduler.algorithm, "resource_aware");',
    )
    replace_exact(
        "crates/api-server/src/handlers/dashboard.rs",
        '    use crate::models::agent::{Agent, AgentSpec};',
        '''    use crate::models::agent::{Agent, AgentSpec};
    use crate::models::node::{Node, NodeStatus, ResourceCapacity};''',
    )
    replace_exact(
        "crates/api-server/src/handlers/dashboard.rs",
        '''    #[tokio::test]
    async fn test_dashboard_empty_state() {
        let state = test_state().await;
        let result = realtime_dashboard(State(state)).await;

        assert_eq!(result.agents.total, 0);
        assert_eq!(result.agents.running, 0);
        assert_eq!(result.nodes.total, 2); // seeded nodes
        assert_eq!(result.nodes.ready, 2);
        assert_eq!(result.tokens.total_tokens, 0);
        assert!(result.recent_events.is_empty());
    }
''',
        '''    #[tokio::test]
    async fn test_dashboard_empty_state() {
        let state = test_state().await;
        let result = realtime_dashboard(State(state)).await;

        assert_eq!(result.agents.total, 0);
        assert_eq!(result.agents.running, 0);
        assert_eq!(result.nodes.total, 0);
        assert_eq!(result.nodes.ready, 0);
        assert_eq!(result.tokens.total_tokens, 0);
        assert!(result.recent_events.is_empty());
    }
''',
    )
    replace_exact(
        "crates/api-server/src/handlers/dashboard.rs",
        '''    #[tokio::test]
    async fn test_dashboard_node_summary() {
        let state = test_state().await;
        let result = realtime_dashboard(State(state)).await;

        assert_eq!(result.nodes.total, 2);
        assert_eq!(result.nodes.ready, 2);
        assert_eq!(result.nodes.not_ready, 0);
        let node1 = result
            .nodes
            .nodes
            .iter()
            .find(|n| n.id == "node-1")
            .unwrap();
        let node2 = result
            .nodes
            .nodes
            .iter()
            .find(|n| n.id == "node-2")
            .unwrap();
        assert_eq!(node1.cpu, "8");
        assert_eq!(node2.cpu, "4");
    }
''',
        '''    #[tokio::test]
    async fn test_dashboard_node_summary() {
        let state = test_state().await;
        let now = chrono::Utc::now().to_rfc3339();
        {
            let mut nodes = state.nodes.write().await;
            for (id, cpu) in [("node-1", "8"), ("node-2", "4")] {
                nodes.insert(
                    id.to_string(),
                    Node {
                        id: id.to_string(),
                        name: id.to_string(),
                        status: NodeStatus::Ready,
                        resources: ResourceCapacity {
                            cpu: cpu.to_string(),
                            memory: "8Gi".to_string(),
                            gpu: "0".to_string(),
                        },
                        allocatable: ResourceCapacity {
                            cpu: cpu.to_string(),
                            memory: "8Gi".to_string(),
                            gpu: "0".to_string(),
                        },
                        labels: HashMap::new(),
                        created_at: now.clone(),
                        last_heartbeat: now.clone(),
                    },
                );
            }
        }

        let result = realtime_dashboard(State(state)).await;
        assert_eq!(result.nodes.total, 2);
        assert_eq!(result.nodes.ready, 2);
        assert_eq!(result.nodes.not_ready, 0);
        let node1 = result.nodes.nodes.iter().find(|n| n.id == "node-1").unwrap();
        let node2 = result.nodes.nodes.iter().find(|n| n.id == "node-2").unwrap();
        assert_eq!(node1.cpu, "8");
        assert_eq!(node2.cpu, "4");
    }
''',
    )

    replace_exact(
        "crates/api-server/src/routes/product.rs",
        '            "kias-test",',
        '            "kias",',
    )
    replace_exact(
        "crates/api-server/src/routes/product.rs",
        '''.uri("/api/v1/system/capabilities")
                    .body(Body::empty())''',
        '''.uri("/api/v1/system/capabilities")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", bearer(crate::auth::Role::Viewer)),
                    )
                    .body(Body::empty())''',
    )
    replace_exact(
        "crates/api-server/src/routes/product.rs",
        '''.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())''',
        '''.oneshot(
                    Request::builder()
                        .uri(path)
                        .header(
                            AUTHORIZATION,
                            format!("Bearer {}", bearer(crate::auth::Role::Viewer)),
                        )
                        .body(Body::empty())
                        .unwrap(),
                )''',
    )
    replace_exact(
        "crates/api-server/src/routes/product.rs",
        '''.method("PATCH")
                    .uri("/api/v1/config")
                    .body(Body::empty())''',
        '''.method("PATCH")
                    .uri("/api/v1/config")
                    .header(
                        AUTHORIZATION,
                        format!("Bearer {}", bearer(crate::auth::Role::Admin)),
                    )
                    .body(Body::empty())''',
    )

    replace_exact(
        "crates/controller/src/state.rs",
        '''    mod tests {
        use super::*;

        #[test]
        fn test_delivery_log_basic_read_write() {
''',
        '''    mod delivery_log_tests {
        use super::*;

        #[test]
        fn test_delivery_log_basic_read_write() {
''',
    )
    replace_exact(
        "crates/auto-loop/src/deployer.rs",
        '''    #[test]
    fn test_git_snapshot_deployer_health_check() {
        let deployer = GitSnapshotDeployer::new("/workspace/kias");
        // /workspace/kias 应该是 git 仓库
        assert!(deployer.health_check());
    }
''',
        '''    #[test]
    fn test_git_snapshot_deployer_health_check() {
        let workspace =
            std::env::temp_dir().join(format!("kias-git-fixture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(workspace.join(".git")).unwrap();
        let deployer = GitSnapshotDeployer::new(&workspace);
        assert!(deployer.health_check());
        std::fs::remove_dir_all(workspace).unwrap();
    }
''',
    )

    replace_exact(
        "Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }',
        'sqlx = { version = "0.8.1", default-features = false, features = ["runtime-tokio", "sqlite", "chrono", "uuid", "macros"] }',
    )
    replace_exact(
        "crates/data-store/Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }',
        'sqlx.workspace = true',
    )
    replace_exact(
        "crates/data-governance/Cargo.toml",
        'sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }',
        'sqlx.workspace = true',
    )
    replace_exact(
        "crates/api-server/Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }',
        'sqlx.workspace = true',
    )


if __name__ == "__main__":
    main()
