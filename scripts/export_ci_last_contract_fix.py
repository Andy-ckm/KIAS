#!/usr/bin/env python3
"""Align the final stale agent-status integration contract, then self-delete."""

from pathlib import Path
import re

path = Path("crates/api-server/tests/integration.rs")
text = path.read_text(encoding="utf-8")
pattern = re.compile(
    r"#\[tokio::test\]\nasync fn test_update_agent_status\(\) \{.*?\n\}\n",
    re.DOTALL,
)
replacement = '''#[tokio::test]
async fn test_direct_agent_status_mutation_route_is_not_exposed() {
    let app = create_router(default_state().await);
    let created = create_test_agent(&app, "status-agent").await;
    let id = created["data"]["id"].as_str().unwrap();

    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/api/v1/agents/{id}/status"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#""Running""#.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise SystemExit(f"agent status integration test: expected one function, found {count}")
path.write_text(text, encoding="utf-8")
Path("scripts/export_ci_last_contract_fix.py").unlink()
