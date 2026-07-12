#!/usr/bin/env python3
"""Apply exact, bounded fixes exposed by strict Core/workspace CI."""

from pathlib import Path


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} matches, found {actual}: {old[:100]!r}")
    target.write_text(text.replace(old, new, count), encoding="utf-8")


def find_matching_brace(text: str, opening: int) -> int:
    depth = 0
    i = opening
    block_comment_depth = 0
    state = "normal"
    raw_hashes = 0

    while i < len(text):
        if state == "line_comment":
            if text[i] == "\n":
                state = "normal"
            i += 1
            continue

        if state == "block_comment":
            if text.startswith("/*", i):
                block_comment_depth += 1
                i += 2
            elif text.startswith("*/", i):
                block_comment_depth -= 1
                i += 2
                if block_comment_depth == 0:
                    state = "normal"
            else:
                i += 1
            continue

        if state == "string":
            if text[i] == "\\":
                i += 2
            elif text[i] == '"':
                state = "normal"
                i += 1
            else:
                i += 1
            continue

        if state == "raw_string":
            closing = '"' + ('#' * raw_hashes)
            if text.startswith(closing, i):
                i += len(closing)
                state = "normal"
            else:
                i += 1
            continue

        if text.startswith("//", i):
            state = "line_comment"
            i += 2
            continue
        if text.startswith("/*", i):
            state = "block_comment"
            block_comment_depth = 1
            i += 2
            continue
        if text[i] == '"':
            state = "string"
            i += 1
            continue
        if text[i] == "r":
            j = i + 1
            while j < len(text) and text[j] == "#":
                j += 1
            if j < len(text) and text[j] == '"':
                raw_hashes = j - i - 1
                state = "raw_string"
                i = j + 1
                continue
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1

    raise SystemExit(f"unmatched AppState brace at byte {opening}")


def add_missing_run_service_fields() -> None:
    root = Path("crates/api-server/src/handlers")
    expected_paths = {
        "agents.rs",
        "auth_gxp.rs",
        "config.rs",
        "context.rs",
        "health.rs",
        "im.rs",
        "metrics.rs",
        "nl_command.rs",
        "nodes.rs",
        "scheduler.rs",
        "tier_routing.rs",
        "tokens.rs",
    }
    changed_paths: set[str] = set()
    additions = 0

    for path in sorted(root.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        insertions: list[tuple[int, str]] = []
        cursor = 0
        while True:
            marker = text.find("AppState {", cursor)
            if marker < 0:
                break
            opening = text.find("{", marker)
            closing = find_matching_brace(text, opening)
            body = text[opening + 1 : closing]
            if "run_service:" not in body:
                line_start = text.rfind("\n", 0, marker) + 1
                indentation = text[line_start:marker]
                if indentation.strip():
                    raise SystemExit(f"{path}: unexpected AppState indentation at byte {marker}")
                insertions.append((opening + 1, f"\n{indentation}    run_service: None,"))
            cursor = closing + 1

        if insertions:
            for position, insertion in reversed(insertions):
                text = text[:position] + insertion + text[position:]
            path.write_text(text, encoding="utf-8")
            changed_paths.add(path.name)
            additions += len(insertions)

    if additions != 42:
        raise SystemExit(f"expected 42 AppState fixture updates, applied {additions}")
    if changed_paths != expected_paths:
        raise SystemExit(
            f"unexpected AppState fixture files: expected {sorted(expected_paths)}, got {sorted(changed_paths)}"
        )


def main() -> None:
    replace_exact(
        "crates/tool-executor/src/builtin.rs",
        '        let result = tool.execute(serde_json::json!({"command": ""})).await;\n'
        "        // Empty command should not panic - just return a result\n"
        "        assert!(true); // Always true - just verifying no panic\n",
        '        let _result = tool.execute(serde_json::json!({"command": ""})).await;\n'
        "        // Returning from execute is sufficient for this no-panic regression test.\n",
    )
    replace_exact(
        "crates/tool-executor/src/builtin.rs",
        "        let result = tool\n"
        "            .execute(serde_json::json!({\n"
        '                "path": "/root/impossible_file_12345.txt",',
        "        let _result = tool\n"
        "            .execute(serde_json::json!({\n"
        '                "path": "/root/impossible_file_12345.txt",',
    )
    replace_exact(
        "crates/tool-executor/src/builtin.rs",
        "        // Should fail gracefully, not panic\n"
        "        assert!(true); // Always true - just verifying no panic\n",
        "        // Returning from execute is sufficient for this no-panic regression test.\n",
    )
    replace_exact(
        "crates/tool-executor/src/vulnerability_scan.rs",
        '        let result = scanner.scan("shell", &params, &default_ctx());\n'
        '        // This might not be caught since pattern is "chmod +s" not numeric\n'
        "        // Just verify no panic\n"
        "        // u64 is always >= 0, this assertion is always true\n",
        '        let _result = scanner.scan("shell", &params, &default_ctx());\n'
        "        // Returning from scan is sufficient for this no-panic regression test.\n",
    )

    add_missing_run_service_fields()

    replace_exact(
        "crates/api-server/src/handlers/durable_agents.rs",
        "        let created_id = created.data.id;\n",
        "        let created_id = created.data.id.clone();\n",
    )

    replace_exact(
        "crates/controller/src/runtime_loop.rs",
        "    // ── Helper ──────────────────────────────────────────────────────────────\n\n"
        "    fn fast_config() -> RuntimeLoopConfig {\n"
        "        RuntimeLoopConfig {\n"
        "            max_rounds: 5,\n"
        "            loop_timeout: Duration::from_secs(10),\n"
        "            round_timeout: Duration::from_secs(5),\n"
        "            quality_threshold: 0.8,\n"
        "            stop_on_achieve: true,\n"
        "            cooldown: Duration::ZERO,\n"
        "        }\n"
        "    }\n\n",
        "",
    )
    replace_exact(
        "crates/controller/src/state.rs",
        "    mod tests {\n"
        "        use super::*;\n\n"
        "        #[test]\n"
        "        fn test_delivery_log_basic_read_write()",
        "    mod delivery_log_tests {\n"
        "        use super::*;\n\n"
        "        #[test]\n"
        "        fn test_delivery_log_basic_read_write()",
    )

    replace_exact(
        "crates/mcp-protocol/src/server.rs",
        "use crate::capabilities::{\n"
        "    ClientCapabilities, PromptsCapability, ResourcesCapability, ToolsCapability, VersionNegotiation,\n"
        "};\n",
        "use crate::capabilities::{PromptsCapability, ResourcesCapability, ToolsCapability};\n",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "struct DockerContainerInfo {\n"
        "    container_id: String,\n"
        "    start_time: std::time::Instant,\n"
        "}",
        "struct DockerContainerInfo {\n"
        "    _container_id: String,\n"
        "    start_time: std::time::Instant,\n"
        "}",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "        let info = DockerContainerInfo {\n"
        "            container_id: cid.clone(),\n",
        "        let info = DockerContainerInfo {\n"
        "            _container_id: cid.clone(),\n",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "struct GVisorContainerInfo {\n"
        "    container_id: String,\n"
        "    start_time: std::time::Instant,\n"
        "}",
        "struct GVisorContainerInfo {\n"
        "    _container_id: String,\n"
        "    start_time: std::time::Instant,\n"
        "}",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "        let info = GVisorContainerInfo {\n"
        "            container_id: cid.clone(),\n",
        "        let info = GVisorContainerInfo {\n"
        "            _container_id: cid.clone(),\n",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "    runsc_bin: String,\n",
        "    _runsc_bin: String,\n",
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        '            runsc_bin: "runsc".to_string(),\n',
        '            _runsc_bin: "runsc".to_string(),\n',
    )
    replace_exact(
        "crates/mcp-protocol/src/sandbox.rs",
        "            runsc_bin: bin.into(),\n",
        "            _runsc_bin: bin.into(),\n",
    )

    replace_exact(
        "crates/compliance-security/src/auth_providers.rs",
        "use zeroize::{Zeroize, ZeroizeOnDrop};\n",
        "use zeroize::Zeroize;\n",
    )
    replace_exact(
        "crates/a2a-registry/src/a2a_enhanced.rs",
        "    protocol_negotiator: ProtocolNegotiator,\n",
        "",
    )
    replace_exact(
        "crates/a2a-registry/src/a2a_enhanced.rs",
        "            protocol_negotiator: ProtocolNegotiator,\n",
        "",
    )
    replace_exact(
        "crates/gxp-compliance/src/audit_trail.rs",
        "        let mut current = proof.leaf_hash.clone();\n"
        "        let mut idx = proof.leaf_index;\n\n",
        "        let mut current = proof.leaf_hash.clone();\n\n",
    )
    replace_exact(
        "crates/gxp-compliance/src/audit_trail.rs",
        "            };\n"
        "            idx /= 2;\n"
        "        }\n\n"
        "        current == proof.merkle_root\n",
        "            };\n"
        "        }\n\n"
        "        current == proof.merkle_root\n",
    )
    replace_exact(
        "crates/gxp-compliance/src/risk_assessment.rs",
        "    historical: HashMap<String, Vec<u32>>,\n",
        "    _historical: HashMap<String, Vec<u32>>,\n",
    )
    replace_exact(
        "crates/gxp-compliance/src/risk_assessment.rs",
        "            historical: HashMap::new(),\n",
        "            _historical: HashMap::new(),\n",
    )

    replace_exact(
        "crates/llm-engine/src/cost.rs",
        "        let total = tracker.get_total_cost().await;\n"
        "        let daily = tracker\n",
        "        let total = tracker.get_total_cost().await;\n"
        "        assert!(total > 0.0);\n"
        "        let daily = tracker\n",
    )
    replace_exact(
        "crates/llm-engine/src/provider.rs",
        "        let mut chunks: Vec<StreamChunk> = Vec::new();\n",
        "        let chunks: Vec<StreamChunk> = Vec::new();\n",
    )
    replace_exact(
        "crates/llm-engine/src/provider.rs",
        "        let mut chunks: Vec<crate::types::StreamChunk> = Vec::new();\n",
        "        let chunks: Vec<crate::types::StreamChunk> = Vec::new();\n",
    )
    replace_exact(
        "crates/llm-engine/src/streaming.rs",
        "            StreamEvent::ToolCallStart { id, name } => {\n"
        '                assert_eq!(id, "call_abc-123_DEF");\n'
        "            }\n",
        "            StreamEvent::ToolCallStart { id, name } => {\n"
        '                assert_eq!(id, "call_abc-123_DEF");\n'
        '                assert_eq!(name, "test");\n'
        "            }\n",
    )


if __name__ == "__main__":
    main()
