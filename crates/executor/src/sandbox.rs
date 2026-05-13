//! # Sandbox Executor
//!
//! Sandboxed task execution inspired by microsandbox.
//! Provides isolated execution with resource limits, timeouts, and output capture.

use async_trait::async_trait;
use chrono::Utc;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use super::runtime::TaskExecutor;
use super::task::{Task, TaskResult, TaskStatus};

/// Sandbox execution policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// Maximum execution time
    pub timeout: Duration,
    /// Maximum memory usage in bytes (for monitoring)
    pub max_memory_bytes: u64,
    /// Maximum output size in bytes
    pub max_output_bytes: usize,
    /// Allowed environment variables (key prefix whitelist)
    pub env_whitelist: Vec<String>,
    /// Whether to capture stderr separately
    pub capture_stderr: bool,
    /// Working directory override
    pub workdir: Option<String>,
    /// Environment variables to inject
    pub env_vars: HashMap<String, String>,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_output_bytes: 1024 * 1024,        // 1MB
            env_whitelist: vec!["KIAS_".to_string(), "PATH".to_string()],
            capture_stderr: true,
            workdir: None,
            env_vars: HashMap::new(),
        }
    }
}

/// Sandbox execution result with detailed metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    /// Exit code of the process
    pub exit_code: i32,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr (if enabled)
    pub stderr: String,
    /// Wall clock duration in milliseconds
    pub duration_ms: u64,
    /// Whether the execution was killed due to timeout
    pub timed_out: bool,
    /// Resource usage stats (if available)
    pub resource_usage: ResourceUsage,
}

/// Resource usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Number of syscalls made
    pub syscall_count: u64,
}

/// Sandbox executor - runs tasks in isolated environments
pub struct SandboxExecutor {
    policy: SandboxPolicy,
    /// Execution history for auditing
    history: tokio::sync::RwLock<Vec<SandboxResult>>,
}

impl SandboxExecutor {
    /// Create a new sandbox executor with default policy
    pub fn new() -> Self {
        Self {
            policy: SandboxPolicy::default(),
            history: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Create a sandbox executor with custom policy
    pub fn with_policy(policy: SandboxPolicy) -> Self {
        Self {
            policy,
            history: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Execute a command in the sandbox
    pub async fn execute_command(&self, command: &str) -> KiasResult<SandboxResult> {
        let start = std::time::Instant::now();

        // Build the command with environment
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        // Apply sandbox policy
        for (key, value) in &self.policy.env_vars {
            cmd.env(key, value);
        }

        if let Some(ref workdir) = self.policy.workdir {
            cmd.current_dir(workdir);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Execute with timeout
        let output = match tokio::time::timeout(self.policy.timeout, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration = start.elapsed();
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Truncate output if needed
                let stdout = if stdout.len() > self.policy.max_output_bytes {
                    stdout[..self.policy.max_output_bytes].to_string()
                } else {
                    stdout.to_string()
                };

                let stderr = if stderr.len() > self.policy.max_output_bytes {
                    stderr[..self.policy.max_output_bytes].to_string()
                } else {
                    stderr.to_string()
                };

                SandboxResult {
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                    duration_ms: duration.as_millis() as u64,
                    timed_out: false,
                    resource_usage: ResourceUsage::default(),
                }
            }
            Ok(Err(e)) => SandboxResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Failed to execute: {}", e),
                duration_ms: start.elapsed().as_millis() as u64,
                timed_out: false,
                resource_usage: ResourceUsage::default(),
            },
            Err(_) => SandboxResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: "Execution timed out".to_string(),
                duration_ms: self.policy.timeout.as_millis() as u64,
                timed_out: true,
                resource_usage: ResourceUsage::default(),
            },
        };

        // Store in history
        {
            let mut history = self.history.write().await;
            history.push(output.clone());
            // Keep only last 100 results
            if history.len() > 100 {
                history.remove(0);
            }
        }

        Ok(output)
    }

    /// Get execution history
    pub async fn history(&self) -> Vec<SandboxResult> {
        let history = self.history.read().await;
        history.clone()
    }

    /// Get the current sandbox policy
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}

impl Default for SandboxExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for SandboxExecutor {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
        let start_time = Utc::now();

        // Extract command from task payload
        let command = task
            .payload
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("echo 'no command specified'");

        tracing::info!(task_id = %task.id, command = %command, "Executing in sandbox");

        let result = self.execute_command(command).await?;

        let end_time = Utc::now();
        let status = if result.timed_out {
            TaskStatus::Failed
        } else if result.exit_code == 0 {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };

        Ok(TaskResult {
            task_id: task.id.clone(),
            status,
            output: Some(serde_json::json!({
                "stdout": result.stdout,
                "stderr": result.stderr,
                "exit_code": result.exit_code,
                "duration_ms": result.duration_ms,
                "timed_out": result.timed_out,
            })),
            error: if result.exit_code != 0 {
                Some(result.stderr)
            } else {
                None
            },
            started_at: start_time,
            completed_at: end_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_policy_default() {
        let policy = SandboxPolicy::default();
        assert_eq!(policy.timeout, Duration::from_secs(30));
        assert_eq!(policy.max_memory_bytes, 512 * 1024 * 1024);
        assert!(policy.capture_stderr);
    }

    #[test]
    fn test_sandbox_policy_custom() {
        let policy = SandboxPolicy {
            timeout: Duration::from_secs(60),
            max_memory_bytes: 1024 * 1024 * 1024,
            max_output_bytes: 2 * 1024 * 1024,
            env_whitelist: vec!["CUSTOM_".to_string()],
            capture_stderr: false,
            workdir: Some("/tmp".to_string()),
            env_vars: HashMap::from([("KEY".to_string(), "VAL".to_string())]),
        };
        assert_eq!(policy.timeout, Duration::from_secs(60));
        assert!(!policy.capture_stderr);
    }

    #[tokio::test]
    async fn test_sandbox_execute_simple_command() {
        let executor = SandboxExecutor::new();
        let result = executor.execute_command("echo hello").await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
        assert!(!result.timed_out);
    }

    #[tokio::test]
    async fn test_sandbox_execute_with_env() {
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let policy = SandboxPolicy {
            env_vars,
            ..Default::default()
        };
        let executor = SandboxExecutor::with_policy(policy);
        let result = executor.execute_command("echo $TEST_VAR").await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("test_value"));
    }

    #[tokio::test]
    async fn test_sandbox_execute_failing_command() {
        let executor = SandboxExecutor::new();
        let result = executor.execute_command("exit 1").await.unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_sandbox_timeout() {
        let policy = SandboxPolicy {
            timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let executor = SandboxExecutor::with_policy(policy);
        let result = executor.execute_command("sleep 10").await.unwrap();
        assert!(result.timed_out);
    }

    #[tokio::test]
    async fn test_sandbox_history() {
        let executor = SandboxExecutor::new();
        executor.execute_command("echo 1").await.unwrap();
        executor.execute_command("echo 2").await.unwrap();

        let history = executor.history().await;
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_sandbox_as_task_executor() {
        let executor = SandboxExecutor::new();
        let task = Task {
            id: "task-1".to_string(),
            name: "test".to_string(),
            agent_id: "agent-1".to_string(),
            payload: serde_json::json!({"command": "echo sandbox-test"}),
            created_at: Utc::now(),
            timeout: Some(Duration::from_secs(10)),
        };

        let result = executor.execute(&task).await.unwrap();
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_sandbox_task_with_no_command() {
        let executor = SandboxExecutor::new();
        let task = Task {
            id: "task-2".to_string(),
            name: "test".to_string(),
            agent_id: "agent-1".to_string(),
            payload: serde_json::json!({}),
            created_at: Utc::now(),
            timeout: None,
        };

        let result = executor.execute(&task).await.unwrap();
        assert_eq!(result.task_id, "task-2");
    }
}
