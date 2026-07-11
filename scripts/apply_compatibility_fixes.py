#!/usr/bin/env python3
"""Apply the reviewed compatibility fixes for PR #2.

This script is intentionally exact: every replacement must match the expected
number of times or the process exits before committing any generated change.
It is removed by the one-shot workflow after successful validation.
"""

from __future__ import annotations

import re
from pathlib import Path


def replace_literal(path: str, old: str, new: str, expected: int = 1) -> None:
    target = Path(path)
    text = target.read_text("utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} literal matches, found {count}: {old!r}"
        )
    target.write_text(text.replace(old, new), "utf-8")
    print(f"{path}: replaced {count} literal match(es)")


def replace_regex(path: str, pattern: str, replacement: str, expected: int = 1) -> None:
    target = Path(path)
    text = target.read_text("utf-8")
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE | re.DOTALL)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} regex matches, found {count}: {pattern!r}"
        )
    target.write_text(updated, "utf-8")
    print(f"{path}: replaced {count} regex match(es)")


def main() -> None:
    replace_literal(
        "crates/cache/src/strategy.rs",
        "for (key, _) in entries.iter() {",
        "for key in entries.keys() {",
    )
    replace_literal(
        "crates/cache/src/layered_cache.rs",
        "use tracing::warn;\n",
        "",
    )
    replace_literal(
        "crates/cache/src/layered_cache.rs",
        """    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {""",
        """    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {""",
        expected=3,
    )
    replace_literal(
        "crates/cache/src/layered_cache.rs",
        'cache.entries.get(&"a".to_string()).unwrap()',
        'cache.entries.get("a").unwrap()',
    )

    replace_regex(
        "crates/mcp-protocol/src/server.rs",
        r"use crate::capabilities::\{\s*ClientCapabilities,\s*PromptsCapability,\s*ResourcesCapability,\s*ToolsCapability,\s*VersionNegotiation,\s*\};",
        "use crate::capabilities::{PromptsCapability, ResourcesCapability, ToolsCapability};",
    )
    replace_regex(
        "crates/mcp-protocol/src/sandbox.rs",
        r"struct DockerContainerInfo \{\s*container_id: String,\s*start_time: std::time::Instant,\s*\}",
        "struct DockerContainerInfo {\n    start_time: std::time::Instant,\n}",
    )
    replace_regex(
        "crates/mcp-protocol/src/sandbox.rs",
        r"let info = DockerContainerInfo \{\s*container_id: cid\.clone\(\),\s*start_time: std::time::Instant::now\(\),\s*\};",
        "let info = DockerContainerInfo {\n            start_time: std::time::Instant::now(),\n        };",
    )
    replace_regex(
        "crates/mcp-protocol/src/sandbox.rs",
        r"struct GVisorContainerInfo \{\s*container_id: String,\s*start_time: std::time::Instant,\s*\}",
        "struct GVisorContainerInfo {\n    start_time: std::time::Instant,\n}",
    )
    replace_regex(
        "crates/mcp-protocol/src/sandbox.rs",
        r"let info = GVisorContainerInfo \{\s*container_id: cid\.clone\(\),\s*start_time: std::time::Instant::now\(\),\s*\};",
        "let info = GVisorContainerInfo {\n            start_time: std::time::Instant::now(),\n        };",
    )
    replace_literal(
        "crates/mcp-protocol/src/sandbox.rs",
        "    runsc_bin: String,",
        "    _runsc_bin: String,",
    )
    replace_literal(
        "crates/mcp-protocol/src/sandbox.rs",
        '            runsc_bin: "runsc".to_string(),',
        '            _runsc_bin: "runsc".to_string(),',
    )
    replace_literal(
        "crates/mcp-protocol/src/sandbox.rs",
        "            runsc_bin: bin.into(),",
        "            _runsc_bin: bin.into(),",
    )

    replace_literal(
        "dashboard/src/pages/AgentDetail.tsx",
        "formatter={(value: number, name: string) => [",
        "formatter={(value, name) => [",
    )
    replace_literal(
        "dashboard/src/pages/AgentDetail.tsx",
        "name === 'cpu_percent' ? `${value.toFixed(1)}%` : `${value.toFixed(0)} MB`,",
        "String(name) === 'cpu_percent' ? `${Number(value ?? 0).toFixed(1)}%` : `${Number(value ?? 0).toFixed(0)} MB`,",
    )
    replace_literal(
        "dashboard/src/pages/AgentDetail.tsx",
        "name === 'cpu_percent' ? 'CPU' : 'Memory',",
        "String(name) === 'cpu_percent' ? 'CPU' : 'Memory',",
    )
    replace_literal(
        "dashboard/src/pages/AgentDetail.tsx",
        "formatter={(value: number) => [`${value.toLocaleString()} tokens`, 'Tokens']}",
        "formatter={value => [`${Number(value ?? 0).toLocaleString()} tokens`, 'Tokens']}",
    )

    replace_literal(
        "dashboard/src/pages/Tokens.tsx",
        "formatter={(value: number, name: string) => [formatTokens(value), name === 'input_tokens' ? 'Input' : 'Output']}",
        "formatter={(value, name) => [formatTokens(Number(value ?? 0)), String(name) === 'input_tokens' ? 'Input' : 'Output']}",
    )
    replace_literal(
        "dashboard/src/pages/Tokens.tsx",
        "formatter={(value: number) => formatTokens(value)}",
        "formatter={value => formatTokens(Number(value ?? 0))}",
    )
    replace_literal(
        "dashboard/src/pages/Tokens.tsx",
        "formatter={(value: number, name: string) => [formatTokens(value), name === 'input' ? 'Input' : 'Output']}",
        "formatter={(value, name) => [formatTokens(Number(value ?? 0)), String(name) === 'input' ? 'Input' : 'Output']}",
    )


if __name__ == "__main__":
    main()
