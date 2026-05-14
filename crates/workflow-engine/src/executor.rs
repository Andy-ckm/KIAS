use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;

use crate::node::{ExecutionResult, ExecutorConfig};

/// Trait for node executors. Each executor handles one kind of work
/// (shell, HTTP, LLM, etc.).
#[async_trait]
pub trait NodeExecutor: Send + Sync + std::fmt::Debug {
    async fn execute(
        &self,
        config: &ExecutorConfig,
        state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult;
}

// ─── Shell Executor ──────────────────────────────────────────────────────────

/// Runs shell commands via `tokio::process::Command`.
#[derive(Debug, Default)]
pub struct ShellExecutor;

impl ShellExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NodeExecutor for ShellExecutor {
    async fn execute(
        &self,
        config: &ExecutorConfig,
        _state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult {
        let (command, args, env, working_dir, timeout_secs) = match config {
            ExecutorConfig::Shell {
                command,
                args,
                env,
                working_dir,
                timeout_secs,
            } => (command, args, env, working_dir, timeout_secs),
            _ => return ExecutionResult::failure("ShellExecutor received non-Shell config"),
        };

        let start = Instant::now();

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c");

        // Build the full command string: command + args
        let full_cmd = if args.is_empty() {
            command.clone()
        } else {
            format!("{} {}", command, args.join(" "))
        };
        cmd.arg(&full_cmd);

        // Environment variables
        for (k, v) in env {
            cmd.env(k, v);
        }

        // Working directory
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Execute with optional timeout
        let output = if let Some(secs) = timeout_secs {
            match tokio::time::timeout(std::time::Duration::from_secs(*secs), cmd.output()).await {
                Ok(result) => result,
                Err(_) => {
                    return ExecutionResult {
                        success: false,
                        error: Some(format!("Command timed out after {}s", secs)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        ..Default::default()
                    };
                }
            }
        } else {
            cmd.output().await
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                let success = output.status.success();

                let mut result = if success {
                    ExecutionResult::success(serde_json::json!({
                        "exit_code": exit_code,
                        "stdout": stdout,
                    }))
                } else {
                    ExecutionResult::failure(format!(
                        "Command exited with code {}: {}",
                        exit_code, stderr
                    ))
                };
                result.stdout = Some(stdout);
                result.stderr = Some(stderr);
                result.duration_ms = duration_ms;
                result
            }
            Err(e) => ExecutionResult {
                success: false,
                error: Some(format!("Failed to execute command: {}", e)),
                duration_ms,
                ..Default::default()
            },
        }
    }
}

// ─── HTTP Executor ───────────────────────────────────────────────────────────

/// Makes HTTP requests via `reqwest`.
#[derive(Debug)]
pub struct HttpExecutor {
    client: reqwest::Client,
}

impl HttpExecutor {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NodeExecutor for HttpExecutor {
    async fn execute(
        &self,
        config: &ExecutorConfig,
        _state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult {
        let (method, url, headers, body, timeout_secs) = match config {
            ExecutorConfig::Http {
                method,
                url,
                headers,
                body,
                timeout_secs,
            } => (method, url, headers, body, timeout_secs),
            _ => return ExecutionResult::failure("HttpExecutor received non-Http config"),
        };

        let start = Instant::now();

        let method_upper = method.to_uppercase();
        let mut request = match method_upper.as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            "HEAD" => self.client.head(url),
            other => {
                return ExecutionResult::failure(format!("Unsupported HTTP method: {}", other))
            }
        };

        // Set headers
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        // Set timeout
        if let Some(secs) = timeout_secs {
            request = request.timeout(std::time::Duration::from_secs(*secs));
        }

        // Set body
        if let Some(body_value) = body {
            request = request.json(body_value);
        }

        // Execute
        let result = request.send().await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let success = response.status().is_success();

                // Collect response headers
                let resp_headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();

                match response.text().await {
                    Ok(body_text) => {
                        // Try to parse as JSON
                        let json_body: serde_json::Value =
                            serde_json::from_str(&body_text).unwrap_or(serde_json::json!(null));

                        let output = serde_json::json!({
                            "status": status,
                            "headers": resp_headers,
                            "body": json_body,
                        });

                        let mut result = if success {
                            ExecutionResult::success(output)
                        } else {
                            ExecutionResult::failure(format!(
                                "HTTP {} error: {}",
                                status, body_text
                            ))
                        };
                        result.duration_ms = duration_ms;
                        result
                    }
                    Err(e) => ExecutionResult {
                        success: false,
                        error: Some(format!("Failed to read response body: {}", e)),
                        duration_ms,
                        ..Default::default()
                    },
                }
            }
            Err(e) => ExecutionResult {
                success: false,
                error: Some(format!("HTTP request failed: {}", e)),
                duration_ms,
                ..Default::default()
            },
        }
    }
}

// ─── LLM Executor ────────────────────────────────────────────────────────────

/// Simulated LLM executor (mock — returns a deterministic response).
/// In production this would call an actual LLM API.
#[derive(Debug, Default)]
pub struct LlmExecutor;

impl LlmExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NodeExecutor for LlmExecutor {
    async fn execute(
        &self,
        config: &ExecutorConfig,
        state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult {
        let (model, prompt, temperature, max_tokens) = match config {
            ExecutorConfig::Llm {
                model,
                prompt,
                temperature,
                max_tokens,
            } => (model, prompt, temperature, max_tokens),
            _ => return ExecutionResult::failure("LlmExecutor received non-Llm config"),
        };

        let start = Instant::now();

        // Simulate processing delay
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Build a mock response that incorporates the prompt and state context
        let state_summary: serde_json::Value =
            serde_json::to_value(state_data).unwrap_or(serde_json::json!({}));

        let response_text = format!(
            "[LLM Mock Response] model={}, prompt=\"{}\", state_keys={:?}",
            model,
            prompt.chars().take(100).collect::<String>(),
            state_data.keys().collect::<Vec<_>>()
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        ExecutionResult {
            success: true,
            output: serde_json::json!({
                "model": model,
                "prompt": prompt,
                "response": response_text,
                "temperature": temperature,
                "max_tokens": max_tokens,
                "usage": {
                    "prompt_tokens": prompt.len() / 4,
                    "completion_tokens": response_text.len() / 4,
                    "total_tokens": (prompt.len() + response_text.len()) / 4,
                },
                "context_state": state_summary,
            }),
            duration_ms,
            ..Default::default()
        }
    }
}

// ─── SubWorkflow Executor ────────────────────────────────────────────────────

/// Sub-workflow executor.
///
/// Note: SubWorkflow nodes are primarily executed by `WorkflowEngine::execute_subworkflow_node`
/// (which creates a child engine with isolated state). This executor exists for cases where
/// a sub-workflow needs to be invoked via the generic executor registry rather than through
/// direct engine composition. It is a thin shim that validates the config and records the intent.
///
/// For most hierarchical workflow patterns, prefer direct engine composition (as implemented in
/// `execute_subworkflow_node`) over using this executor, because the engine has full access to
/// the graph topology, checkpoint store, and event sink needed for correct subgraph execution.
#[derive(Debug, Default)]
pub struct SubWorkflowExecutor;

impl SubWorkflowExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NodeExecutor for SubWorkflowExecutor {
    async fn execute(
        &self,
        config: &ExecutorConfig,
        state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult {
        let workflow_id = match config {
            ExecutorConfig::SubWorkflow { workflow_id } => workflow_id,
            _ => {
                return ExecutionResult::failure(
                    "SubWorkflowExecutor received non-SubWorkflow config",
                )
            }
        };

        tracing::info!(
            workflow_id = %workflow_id,
            state_keys = state_data.len(),
            "SubWorkflow executor called — subgraph execution is handled by WorkflowEngine::execute_subworkflow_node"
        );

        // Return structured output documenting that this executor is a shim.
        // The actual subgraph execution happens in the engine's `execute_subworkflow_node`,
        // which has access to the full graph, checkpoint store, and event sink.
        ExecutionResult::success(serde_json::json!({
            "sub_workflow_id": workflow_id,
            "status": "deferred",
            "message": "SubWorkflow execution is handled by WorkflowEngine::execute_subworkflow_node"
        }))
    }
}

// ─── Executor Registry ───────────────────────────────────────────────────────

/// Registry that maps `ExecutorConfig` variants to their implementations.
pub struct ExecutorRegistry {
    shell: Box<dyn NodeExecutor>,
    http: Box<dyn NodeExecutor>,
    llm: Box<dyn NodeExecutor>,
    sub_workflow: Box<dyn NodeExecutor>,
}

impl std::fmt::Debug for ExecutorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorRegistry").finish()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self {
            shell: Box::new(ShellExecutor::new()),
            http: Box::new(HttpExecutor::new()),
            llm: Box::new(LlmExecutor::new()),
            sub_workflow: Box::new(SubWorkflowExecutor::new()),
        }
    }

    /// Execute a node's executor config against the current state data.
    pub async fn execute(
        &self,
        config: &ExecutorConfig,
        state_data: &HashMap<String, serde_json::Value>,
    ) -> ExecutionResult {
        match config {
            ExecutorConfig::Shell { .. } => self.shell.execute(config, state_data).await,
            ExecutorConfig::Http { .. } => self.http.execute(config, state_data).await,
            ExecutorConfig::Llm { .. } => self.llm.execute(config, state_data).await,
            ExecutorConfig::SubWorkflow { .. } => {
                self.sub_workflow.execute(config, state_data).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_executor_echo() {
        let exec = ShellExecutor::new();
        let config = ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["hello world".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: None,
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(result.success);
        assert!(result.stdout.as_ref().unwrap().contains("hello world"));
    }

    #[tokio::test]
    async fn test_shell_executor_failure() {
        let exec = ShellExecutor::new();
        let config = ExecutorConfig::Shell {
            command: "false".into(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: None,
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_shell_executor_timeout() {
        let exec = ShellExecutor::new();
        let config = ExecutorConfig::Shell {
            command: "sleep".into(),
            args: vec!["60".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: Some(1),
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_shell_executor_env() {
        let exec = ShellExecutor::new();
        let mut env = HashMap::new();
        env.insert("TEST_VAR".into(), "hello".into());
        let config = ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["$TEST_VAR".into()],
            env,
            working_dir: None,
            timeout_secs: None,
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(result.success);
        assert!(result.stdout.as_ref().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_shell_executor_non_shell_config() {
        let exec = ShellExecutor::new();
        let config = ExecutorConfig::Llm {
            model: "test".into(),
            prompt: "test".into(),
            temperature: None,
            max_tokens: None,
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_http_executor_invalid_method() {
        let exec = HttpExecutor::new();
        let config = ExecutorConfig::Http {
            method: "INVALID".into(),
            url: "http://localhost".into(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(!result.success);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("Unsupported HTTP method"));
    }

    #[tokio::test]
    async fn test_http_executor_unreachable() {
        let exec = HttpExecutor::new();
        let config = ExecutorConfig::Http {
            method: "GET".into(),
            url: "http://127.0.0.1:1".into(), // unlikely to be listening
            headers: HashMap::new(),
            body: None,
            timeout_secs: Some(2),
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_llm_executor() {
        let exec = LlmExecutor::new();
        let config = ExecutorConfig::Llm {
            model: "gpt-4".into(),
            prompt: "What is Rust?".into(),
            temperature: Some(0.7),
            max_tokens: Some(100),
        };
        let mut state = HashMap::new();
        state.insert("context".into(), serde_json::json!("some context"));
        let result = exec.execute(&config, &state).await;
        assert!(result.success);
        assert_eq!(result.output["model"], "gpt-4");
        assert!(result.output["response"]
            .as_str()
            .unwrap()
            .contains("What is Rust?"));
    }

    #[tokio::test]
    async fn test_subworkflow_executor() {
        let exec = SubWorkflowExecutor::new();
        let config = ExecutorConfig::SubWorkflow {
            workflow_id: "sub-wf-1".into(),
        };
        let result = exec.execute(&config, &HashMap::new()).await;
        assert!(result.success);
        assert_eq!(result.output["sub_workflow_id"], "sub-wf-1");
    }

    #[tokio::test]
    async fn test_registry_shell() {
        let registry = ExecutorRegistry::new();
        let config = ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["registry test".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: None,
        };
        let result = registry.execute(&config, &HashMap::new()).await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_registry_llm() {
        let registry = ExecutorRegistry::new();
        let config = ExecutorConfig::Llm {
            model: "test".into(),
            prompt: "hello".into(),
            temperature: None,
            max_tokens: None,
        };
        let result = registry.execute(&config, &HashMap::new()).await;
        assert!(result.success);
    }
}
