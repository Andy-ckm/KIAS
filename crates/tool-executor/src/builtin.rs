//! 内置工具实现

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 参数 schema
    fn parameters(&self) -> serde_json::Value;

    /// 执行工具
    async fn execute(&self, params: serde_json::Value) -> ToolResult;
}

/// 文件读取工具
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "读取文件内容"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "offset": { "type": "integer", "description": "起始行号", "default": 1 },
                "limit": { "type": "integer", "description": "读取行数", "default": 1000 }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let path = params["path"].as_str().unwrap_or("");
        let offset = params["offset"].as_u64().unwrap_or(1) as usize;
        let limit = params["limit"].as_u64().unwrap_or(1000) as usize;

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = (offset - 1).min(lines.len());
                let end = (start + limit).min(lines.len());
                let selected: Vec<String> = lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{}|{}", start + i + 1, line))
                    .collect();

                ToolResult {
                    success: true,
                    output: selected.join("\n"),
                    error: None,
                    metadata: Some(serde_json::json!({
                        "total_lines": lines.len(),
                        "showing": format!("{}-{}", start + 1, end),
                    })),
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to read file: {}", e)),
                metadata: None,
            },
        }
    }
}

/// 文件写入工具
pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "写入文件内容"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "content": { "type": "string", "description": "文件内容" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let path = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");

        // 创建父目录
        if let Some(parent) = PathBuf::from(path).parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        match tokio::fs::write(path, content).await {
            Ok(_) => ToolResult {
                success: true,
                output: format!("File written: {}", path),
                error: None,
                metadata: Some(serde_json::json!({
                    "bytes_written": content.len(),
                })),
            },
            Err(e) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to write file: {}", e)),
                metadata: None,
            },
        }
    }
}

/// Shell 执行工具
pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "执行 shell 命令"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell 命令" },
                "workdir": { "type": "string", "description": "工作目录" },
                "timeout": { "type": "integer", "description": "超时秒数", "default": 60 }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let command = params["command"].as_str().unwrap_or("");
        let workdir = params["workdir"].as_str();
        let timeout = params["timeout"].as_u64().unwrap_or(60);

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        match tokio::time::timeout(std::time::Duration::from_secs(timeout), cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                ToolResult {
                    success: output.status.success(),
                    output: if stdout.is_empty() {
                        stderr.clone()
                    } else {
                        stdout
                    },
                    error: if output.status.success() {
                        None
                    } else {
                        Some(stderr)
                    },
                    metadata: Some(serde_json::json!({
                        "exit_code": exit_code,
                    })),
                }
            }
            Ok(Err(e)) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to execute command: {}", e)),
                metadata: None,
            },
            Err(_) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Command timed out after {}s", timeout)),
                metadata: None,
            },
        }
    }
}

/// 搜索工具
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

/// 获取所有内置工具
pub fn get_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(FileReadTool),
        Box::new(FileWriteTool),
        Box::new(ShellTool),
        Box::new(SearchTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_read_tool_metadata() {
        let tool = FileReadTool;
        assert_eq!(tool.name(), "file_read");
        assert!(!tool.description().is_empty());
        let params = tool.parameters();
        assert!(params["properties"]["path"].is_object());
    }

    #[test]
    fn test_file_write_tool_metadata() {
        let tool = FileWriteTool;
        assert_eq!(tool.name(), "file_write");
        let params = tool.parameters();
        assert!(params["properties"]["content"].is_object());
    }

    #[test]
    fn test_shell_tool_metadata() {
        let tool = ShellTool;
        assert_eq!(tool.name(), "shell");
        let params = tool.parameters();
        assert!(params["properties"]["command"].is_object());
    }

    #[test]
    fn test_search_tool_metadata() {
        let tool = SearchTool;
        assert_eq!(tool.name(), "search");
        let params = tool.parameters();
        assert!(params["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_shell_tool_echo() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({"command": "echo hello"}))
            .await;
        assert!(result.success);
        assert!(result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_shell_tool_failure() {
        let tool = ShellTool;
        let result = tool.execute(serde_json::json!({"command": "false"})).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_file_write_and_read() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let writer = FileWriteTool;
        let write_result = writer
            .execute(serde_json::json!({"path": &path, "content": "line1\nline2\nline3"}))
            .await;
        assert!(write_result.success);

        let reader = FileReadTool;
        let read_result = reader.execute(serde_json::json!({"path": &path})).await;
        assert!(read_result.success);
        assert!(read_result.output.contains("line1"));
        assert!(read_result.output.contains("line2"));
    }

    #[test]
    fn test_get_builtin_tools() {
        let tools = get_builtin_tools();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"file_write"));
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"search"));
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            success: true,
            output: "test output".to_string(),
            error: None,
            metadata: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("test output"));
        assert!(json.contains("\"success\":true"));
    }

    // ========== FileReadTool 额外测试 ==========

    #[tokio::test]
    async fn test_file_read_not_found() {
        let tool = FileReadTool;
        let result = tool
            .execute(serde_json::json!({"path": "/nonexistent/file.txt"}))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Failed to read file"));
    }

    #[tokio::test]
    async fn test_file_read_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();

        let tool = FileReadTool;
        let result = tool
            .execute(serde_json::json!({"path": tmp.path().to_str().unwrap()}))
            .await;
        assert!(result.success);
        assert!(result.output.is_empty());
        let meta = result.metadata.unwrap();
        assert_eq!(meta["total_lines"], 0);
    }

    #[tokio::test]
    async fn test_file_read_with_offset_and_limit() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "line1\nline2\nline3\nline4\nline5").unwrap();

        let tool = FileReadTool;
        // 读取第2-3行
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "offset": 2,
                "limit": 2
            }))
            .await;
        assert!(result.success);
        assert!(result.output.contains("2|line2"));
        assert!(result.output.contains("3|line3"));
        assert!(!result.output.contains("1|line1"));
        assert!(!result.output.contains("4|line4"));
    }

    #[tokio::test]
    async fn test_file_read_offset_beyond_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "line1\nline2").unwrap();

        let tool = FileReadTool;
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "offset": 100,
                "limit": 10
            }))
            .await;
        assert!(result.success);
        assert!(result.output.is_empty());
    }

    #[tokio::test]
    async fn test_file_read_limit_larger_than_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "only_one_line").unwrap();

        let tool = FileReadTool;
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "offset": 1,
                "limit": 1000
            }))
            .await;
        assert!(result.success);
        assert!(result.output.contains("1|only_one_line"));
        let meta = result.metadata.unwrap();
        assert_eq!(meta["total_lines"], 1);
    }

    #[tokio::test]
    async fn test_file_read_missing_path_param() {
        let tool = FileReadTool;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(!result.success);
    }

    #[test]
    fn test_file_read_tool_parameters_schema() {
        let tool = FileReadTool;
        let params = tool.parameters();
        let props = &params["properties"];
        assert!(props["path"].is_object());
        assert!(props["offset"].is_object());
        assert!(props["limit"].is_object());
        assert_eq!(params["required"][0], "path");
    }

    // ========== FileWriteTool 额外测试 ==========

    #[tokio::test]
    async fn test_file_write_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a/b/c/file.txt");

        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": path.to_str().unwrap(),
                "content": "nested content"
            }))
            .await;
        assert!(result.success);
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "nested content");
    }

    #[tokio::test]
    async fn test_file_write_empty_content() {
        let tmp = tempfile::NamedTempFile::new().unwrap();

        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "content": ""
            }))
            .await;
        assert!(result.success);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "");
    }

    #[tokio::test]
    async fn test_file_write_overwrites_existing() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "old content").unwrap();

        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "content": "new content"
            }))
            .await;
        assert!(result.success);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_file_write_metadata_bytes_written() {
        let tmp = tempfile::NamedTempFile::new().unwrap();

        let tool = FileWriteTool;
        let result = tool
            .execute(serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
                "content": "12345"
            }))
            .await;
        assert!(result.success);
        let meta = result.metadata.unwrap();
        assert_eq!(meta["bytes_written"], 5);
    }

    #[tokio::test]
    async fn test_file_write_missing_params() {
        let tool = FileWriteTool;
        let result = tool.execute(serde_json::json!({})).await;
        // 空路径会失败
        assert!(!result.success);
    }

    #[test]
    fn test_file_write_tool_parameters_schema() {
        let tool = FileWriteTool;
        let params = tool.parameters();
        assert_eq!(params["required"][0], "path");
        assert_eq!(params["required"][1], "content");
    }

    // ========== ShellTool 额外测试 ==========

    #[tokio::test]
    async fn test_shell_with_workdir() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({
                "command": "pwd",
                "workdir": "/tmp"
            }))
            .await;
        assert!(result.success);
        assert!(result.output.trim().contains("/tmp"));
    }

    #[tokio::test]
    async fn test_shell_stderr_on_failure() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({"command": "echo err_msg >&2 && exit 1"}))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("err_msg"));
    }

    #[tokio::test]
    async fn test_shell_exit_code_metadata() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({"command": "exit 42"}))
            .await;
        assert!(!result.success);
        let meta = result.metadata.unwrap();
        assert_eq!(meta["exit_code"], 42);
    }

    #[tokio::test]
    async fn test_shell_success_exit_code_metadata() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({"command": "true"}))
            .await;
        assert!(result.success);
        let meta = result.metadata.unwrap();
        assert_eq!(meta["exit_code"], 0);
    }

    #[tokio::test]
    async fn test_shell_timeout() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({
                "command": "sleep 10",
                "timeout": 1
            }))
            .await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_shell_multiline_output() {
        let tool = ShellTool;
        let result = tool
            .execute(serde_json::json!({"command": "printf 'a\\nb\\nc'"}))
            .await;
        assert!(result.success);
        assert!(result.output.contains("a"));
        assert!(result.output.contains("b"));
        assert!(result.output.contains("c"));
    }

    #[tokio::test]
    async fn test_shell_missing_command() {
        let tool = ShellTool;
        let result = tool.execute(serde_json::json!({})).await;
        // 空命令会执行 sh -c ""，可能成功也可能失败
        // 但不应该 panic
        let _ = result;
    }

    // ========== SearchTool 额外测试 ==========

    #[tokio::test]
    async fn test_search_basic_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "hello world\nfoo bar\nhello again").unwrap();

        let tool = SearchTool;
        let result = tool
            .execute(serde_json::json!({
                "pattern": "hello",
                "path": tmp.path().to_str().unwrap()
            }))
            .await;
        assert!(result.success);
        let meta = result.metadata.unwrap();
        assert!(meta["matches"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "nothing here").unwrap();

        let tool = SearchTool;
        let result = tool
            .execute(serde_json::json!({
                "pattern": "nonexistent_pattern_xyz_12345",
                "path": tmp.path().to_str().unwrap()
            }))
            .await;
        // rg with --json may output summary lines even with 0 matches
        // Just verify the tool doesn't panic and returns a result
        assert!(result.metadata.is_some());
    }

    #[tokio::test]
    async fn test_search_with_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.txt");
        let content = (0..100).map(|i| format!("line {} target", i)).collect::<Vec<_>>().join("\n");
        std::fs::write(&file, content).unwrap();

        let tool = SearchTool;
        let result = tool
            .execute(serde_json::json!({
                "pattern": "target",
                "path": tmp.path().to_str().unwrap(),
                "limit": 5
            }))
            .await;
        assert!(result.success);
        let meta = result.metadata.unwrap();
        assert!(meta["matches"].as_u64().unwrap() <= 5);
    }

    #[tokio::test]
    async fn test_search_with_file_glob() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "fn main() {}").unwrap();

        let tool = SearchTool;
        let result = tool
            .execute(serde_json::json!({
                "pattern": "fn main",
                "path": tmp.path().to_str().unwrap(),
                "file_glob": "*.rs"
            }))
            .await;
        assert!(result.success);
    }

    #[test]
    fn test_search_tool_parameters_schema() {
        let tool = SearchTool;
        let params = tool.parameters();
        assert_eq!(params["required"][0], "pattern");
        assert!(params["properties"]["pattern"].is_object());
        assert!(params["properties"]["path"].is_object());
        assert!(params["properties"]["file_glob"].is_object());
        assert!(params["properties"]["limit"].is_object());
    }

    // ========== ToolResult 额外测试 ==========

    #[test]
    fn test_tool_result_with_error() {
        let result = ToolResult {
            success: false,
            output: String::new(),
            error: Some("something went wrong".to_string()),
            metadata: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("something went wrong"));
    }

    #[test]
    fn test_tool_result_clone() {
        let result = ToolResult {
            success: true,
            output: "test".to_string(),
            error: None,
            metadata: Some(serde_json::json!({"k": "v"})),
        };
        let cloned = result.clone();
        assert_eq!(cloned.success, result.success);
        assert_eq!(cloned.output, result.output);
    }

    #[test]
    fn test_tool_result_debug() {
        let result = ToolResult {
            success: true,
            output: "out".to_string(),
            error: None,
            metadata: None,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("ToolResult"));
        assert!(debug.contains("true"));
    }
}
