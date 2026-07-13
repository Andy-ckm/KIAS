#!/usr/bin/env python3
"""Replace the ripgrep-only SearchTool with a deterministic built-in engine."""

from pathlib import Path


def replace_once(path_name: str, old: str, new: str, label: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    replace_once(
        "crates/tool-executor/Cargo.toml",
        'tracing = { workspace = true }\n'
        'uuid = { workspace = true }\n',
        'tracing = { workspace = true }\n'
        'uuid = { workspace = true }\n'
        'regex = { workspace = true }\n',
        "tool-executor regex dependency",
    )

    replace_once(
        "crates/tool-executor/src/builtin.rs",
        'use async_trait::async_trait;\n'
        'use serde::{Deserialize, Serialize};\n'
        'use std::path::PathBuf;\n',
        'use async_trait::async_trait;\n'
        'use regex::Regex;\n'
        'use serde::{Deserialize, Serialize};\n'
        'use std::fs::File;\n'
        'use std::io::{BufRead, BufReader};\n'
        'use std::path::{Path, PathBuf};\n',
        "SearchTool imports",
    )

    old = '''/// 搜索工具
pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "搜索文件内容"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "搜索模式 (regex)" },
                "path": { "type": "string", "description": "搜索路径", "default": "." },
                "file_glob": { "type": "string", "description": "文件过滤" },
                "limit": { "type": "integer", "description": "最大结果数", "default": 50 }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let pattern = params["pattern"].as_str().unwrap_or("");
        let path = params["path"].as_str().unwrap_or(".");
        let limit = params["limit"].as_u64().unwrap_or(50);

        // 使用 rg (ripgrep) 进行搜索
        let mut cmd = tokio::process::Command::new("rg");
        cmd.arg("--json")
            .arg("--max-count")
            .arg(limit.to_string())
            .arg(pattern)
            .arg(path);

        match cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let lines: Vec<&str> = stdout.lines().take(limit as usize).collect();

                ToolResult {
                    success: true,
                    output: lines.join("\n"),
                    error: None,
                    metadata: Some(serde_json::json!({
                        "matches": lines.len(),
                    })),
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Search failed: {}", e)),
                metadata: None,
            },
        }
    }
}
'''

    new = '''/// 搜索工具
pub struct SearchTool;

#[derive(Debug)]
struct SearchOutcome {
    matches: Vec<String>,
    truncated: bool,
    files_scanned: usize,
    files_skipped: usize,
}

fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.as_bytes();
    let candidate = candidate.as_bytes();
    let (mut p, mut c) = (0usize, 0usize);
    let mut star = None;
    let mut retry = 0usize;

    while c < candidate.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == candidate[c]) {
            p += 1;
            c += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = c;
        } else if let Some(star_index) = star {
            p = star_index + 1;
            retry += 1;
            c = retry;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

fn path_matches_glob(path: &Path, glob: Option<&str>) -> bool {
    let Some(glob) = glob.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let normalized_path = path.to_string_lossy().replace('\\\\', "/");
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();

    if glob.contains('/') {
        wildcard_matches(glob, &normalized_path)
    } else {
        wildcard_matches(glob, &file_name)
    }
}

fn search_file(
    path: &Path,
    regex: &Regex,
    limit: usize,
    outcome: &mut SearchOutcome,
) -> std::io::Result<()> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let mut line_number = 0usize;
    outcome.files_scanned += 1;

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\\n', &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        line_number += 1;

        // NUL bytes are a reliable low-cost signal that this is not a text file.
        if buffer.contains(&0) {
            outcome.files_scanned = outcome.files_scanned.saturating_sub(1);
            outcome.files_skipped += 1;
            return Ok(());
        }

        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end_matches(['\\r', '\\n']);
        if regex.is_match(line) {
            outcome
                .matches
                .push(format!("{}:{}:{}", path.display(), line_number, line));
            if outcome.matches.len() >= limit {
                outcome.truncated = true;
                return Ok(());
            }
        }
    }

    Ok(())
}

fn search_path(
    path: &Path,
    regex: &Regex,
    glob: Option<&str>,
    limit: usize,
    outcome: &mut SearchOutcome,
) -> std::io::Result<()> {
    if outcome.matches.len() >= limit {
        outcome.truncated = true;
        return Ok(());
    }

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        outcome.files_skipped += 1;
        return Ok(());
    }

    if metadata.is_file() {
        if path_matches_glob(path, glob) {
            if search_file(path, regex, limit, outcome).is_err() {
                outcome.files_skipped += 1;
            }
        }
        return Ok(());
    }

    if !metadata.is_dir() {
        outcome.files_skipped += 1;
        return Ok(());
    }

    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if outcome.matches.len() >= limit {
            outcome.truncated = true;
            break;
        }
        let entry_path = entry.path();
        if entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".git")
        {
            outcome.files_skipped += 1;
            continue;
        }
        if search_path(&entry_path, regex, glob, limit, outcome).is_err() {
            outcome.files_skipped += 1;
        }
    }
    Ok(())
}

fn run_builtin_search(
    root: PathBuf,
    pattern: String,
    glob: Option<String>,
    limit: usize,
) -> Result<SearchOutcome, String> {
    if pattern.is_empty() {
        return Err("Search pattern must not be empty".to_string());
    }
    let regex = Regex::new(&pattern).map_err(|error| format!("Invalid search regex: {error}"))?;
    if !root.exists() {
        return Err(format!("Search path does not exist: {}", root.display()));
    }

    let mut outcome = SearchOutcome {
        matches: Vec::new(),
        truncated: false,
        files_scanned: 0,
        files_skipped: 0,
    };
    search_path(&root, &regex, glob.as_deref(), limit, &mut outcome)
        .map_err(|error| format!("Search failed: {error}"))?;
    Ok(outcome)
}

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "搜索文件内容"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "搜索模式 (regex)" },
                "path": { "type": "string", "description": "搜索路径", "default": "." },
                "file_glob": { "type": "string", "description": "文件过滤" },
                "limit": { "type": "integer", "description": "最大结果数", "default": 50 }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let pattern = params["pattern"].as_str().unwrap_or("").to_string();
        let path = PathBuf::from(params["path"].as_str().unwrap_or("."));
        let file_glob = params["file_glob"].as_str().map(str::to_string);
        let limit = params["limit"].as_u64().unwrap_or(50).clamp(1, 10_000) as usize;

        match tokio::task::spawn_blocking(move || {
            run_builtin_search(path, pattern, file_glob, limit)
        })
        .await
        {
            Ok(Ok(outcome)) => ToolResult {
                success: true,
                output: outcome.matches.join("\n"),
                error: None,
                metadata: Some(serde_json::json!({
                    "matches": outcome.matches.len(),
                    "truncated": outcome.truncated,
                    "files_scanned": outcome.files_scanned,
                    "files_skipped": outcome.files_skipped,
                    "engine": "builtin-regex"
                })),
            },
            Ok(Err(error)) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
                metadata: Some(serde_json::json!({
                    "matches": 0,
                    "engine": "builtin-regex"
                })),
            },
            Err(error) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Search worker failed: {error}")),
                metadata: Some(serde_json::json!({
                    "matches": 0,
                    "engine": "builtin-regex"
                })),
            },
        }
    }
}
'''

    replace_once(
        "crates/tool-executor/src/builtin.rs",
        old,
        new,
        "SearchTool implementation",
    )

    replace_once(
        "crates/tool-executor/src/builtin.rs",
        "    async fn test_search_tool_executes_rg() {\n",
        "    async fn test_search_tool_uses_builtin_engine() {\n",
        "SearchTool implementation test name",
    )

    Path("scripts/export_ci_search_fallback_fix.py").unlink()


if __name__ == "__main__":
    main()
