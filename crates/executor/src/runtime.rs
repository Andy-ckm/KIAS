use async_trait::async_trait;
use kias_common::KiasResult;
use super::task::{Task, TaskResult, TaskStatus};
use chrono::Utc;
use std::time::Instant;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trait that all task executors must implement
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult>;
}

/// Runtime wrapper that adds timing, error handling, and retry logic
pub struct TaskRuntime {
    executor: Box<dyn TaskExecutor>,
    /// Maximum retry attempts on failure
    max_retries: u32,
    /// Global timeout for any single task execution
    global_timeout: Option<std::time::Duration>,
}

impl TaskRuntime {
    pub fn new(executor: Box<dyn TaskExecutor>) -> Self {
        Self {
            executor,
            max_retries: 0,
            global_timeout: None,
        }
    }

    pub fn with_retries(executor: Box<dyn TaskExecutor>, max_retries: u32) -> Self {
        Self {
            executor,
            max_retries,
            global_timeout: None,
        }
    }

    pub fn with_global_timeout(executor: Box<dyn TaskExecutor>, timeout: std::time::Duration) -> Self {
        Self {
            executor,
            max_retries: 0,
            global_timeout: Some(timeout),
        }
    }

    pub fn with_retries_and_timeout(
        executor: Box<dyn TaskExecutor>,
        max_retries: u32,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            executor,
            max_retries,
            global_timeout: Some(timeout),
        }
    }

    /// Run a task with optional retry logic and timeout enforcement
    pub async fn run_task(&self, task: &Task) -> KiasResult<TaskResult> {
        tracing::info!(task_id = %task.id, retries = self.max_retries, "Running task");

        let mut last_result = None;
        let total_start = Instant::now();

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tracing::info!(task_id = %task.id, attempt = attempt, "Retrying task");
            }

            let start_time = Utc::now();

            // Apply timeout: task-level > runtime-level
            let timeout = task.timeout.or(self.global_timeout);

            let result = if let Some(timeout_dur) = timeout {
                match tokio::time::timeout(timeout_dur, self.executor.execute(task)).await {
                    Ok(result) => result,
                    Err(_) => {
                        let end_time = Utc::now();
                        tracing::warn!(task_id = %task.id, timeout_ms = timeout_dur.as_millis() as u64, "Task timed out");
                        if attempt < self.max_retries {
                            last_result = Some(Ok(TaskResult {
                                task_id: task.id.clone(),
                                status: TaskStatus::Failed,
                                output: None,
                                error: Some(format!("Task timed out after {}ms", timeout_dur.as_millis())),
                                started_at: start_time,
                                completed_at: end_time,
                            }));
                            continue;
                        }
                        return Ok(TaskResult {
                            task_id: task.id.clone(),
                            status: TaskStatus::Failed,
                            output: None,
                            error: Some(format!("Task timed out after {}ms", timeout_dur.as_millis())),
                            started_at: start_time,
                            completed_at: end_time,
                        });
                    }
                }
            } else {
                self.executor.execute(task).await
            };

            let end_time = Utc::now();

            match result {
                Ok(output) => {
                    let final_status = output.status.clone();
                    if final_status == TaskStatus::Failed && attempt < self.max_retries {
                        last_result = Some(Ok(TaskResult {
                            task_id: task.id.clone(),
                            status: TaskStatus::Failed,
                            output: output.output,
                            error: output.error,
                            started_at: start_time,
                            completed_at: end_time,
                        }));
                        continue;
                    }
                    return Ok(TaskResult {
                        task_id: task.id.clone(),
                        status: final_status,
                        output: output.output,
                        error: output.error,
                        started_at: start_time,
                        completed_at: end_time,
                    });
                }
                Err(e) => {
                    if attempt < self.max_retries {
                        last_result = Some(Ok(TaskResult {
                            task_id: task.id.clone(),
                            status: TaskStatus::Failed,
                            output: None,
                            error: Some(e.to_string()),
                            started_at: start_time,
                            completed_at: end_time,
                        }));
                        continue;
                    }
                    return Ok(TaskResult {
                        task_id: task.id.clone(),
                        status: TaskStatus::Failed,
                        output: None,
                        error: Some(e.to_string()),
                        started_at: start_time,
                        completed_at: end_time,
                    });
                }
            }
        }

        tracing::warn!(
            task_id = %task.id,
            elapsed_ms = total_start.elapsed().as_millis() as u64,
            "Task exhausted all retries"
        );

        last_result.unwrap_or_else(|| {
            Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Failed,
                output: None,
                error: Some("All retries exhausted".to_string()),
                started_at: Utc::now(),
                completed_at: Utc::now(),
            })
        })
    }

    /// Run multiple tasks concurrently with bounded parallelism
    pub async fn run_tasks(&self, tasks: &[Task], max_concurrent: usize) -> Vec<KiasResult<TaskResult>> {
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let mut results = Vec::with_capacity(tasks.len());

        for task in tasks {
            let _permit = semaphore.clone().acquire_owned().await.unwrap();
            let result = self.run_task(task).await;
            results.push(result);
        }

        results
    }

    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    pub fn global_timeout(&self) -> Option<std::time::Duration> {
        self.global_timeout
    }
}

/// Task cancellation token for graceful shutdown
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Cancellable task runtime wrapper
pub struct CancellableRuntime {
    runtime: TaskRuntime,
    cancel_token: CancellationToken,
}

impl CancellableRuntime {
    pub fn new(runtime: TaskRuntime) -> Self {
        Self {
            runtime,
            cancel_token: CancellationToken::new(),
        }
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    pub async fn run_task(&self, task: &Task) -> KiasResult<TaskResult> {
        if self.cancel_token.is_cancelled() {
            return Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Cancelled,
                output: None,
                error: Some("Task cancelled before execution".to_string()),
                started_at: Utc::now(),
                completed_at: Utc::now(),
            });
        }
        self.runtime.run_task(task).await
    }
}

/// HTTP executor - makes HTTP calls to remote endpoints
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
impl TaskExecutor for HttpExecutor {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
        let start_time = Utc::now();

        let url = task
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| kias_common::error::KiasError::Config("Missing 'url' in task payload".to_string()))?;

        let method = task
            .payload
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let body = task.payload.get("body");
        let headers: Option<&serde_json::Map<String, serde_json::Value>> =
            task.payload.get("headers").and_then(|v| v.as_object());

        tracing::info!(task_id = %task.id, url = %url, method = %method, "Executing HTTP task");

        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            _ => return Err(kias_common::error::KiasError::Config(format!("Unsupported HTTP method: {}", method))),
        };

        if let Some(headers_map) = headers {
            for (key, value) in headers_map {
                if let Some(val) = value.as_str() {
                    request = request.header(key.as_str(), val);
                }
            }
        }

        if let Some(body_val) = body {
            request = request.json(body_val);
        }

        let response = request.send().await;
        let end_time = Utc::now();

        match response {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let body_text = resp.text().await.unwrap_or_default();
                let success = status_code >= 200 && status_code < 300;

                Ok(TaskResult {
                    task_id: task.id.clone(),
                    status: if success { TaskStatus::Completed } else { TaskStatus::Failed },
                    output: Some(serde_json::json!({
                        "status_code": status_code,
                        "body": body_text,
                    })),
                    error: if !success {
                        Some(format!("HTTP {} returned status {}", method, status_code))
                    } else {
                        None
                    },
                    started_at: start_time,
                    completed_at: end_time,
                })
            }
            Err(e) => Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Failed,
                output: None,
                error: Some(format!("HTTP request failed: {}", e)),
                started_at: start_time,
                completed_at: end_time,
            }),
        }
    }
}

/// LLM executor - calls LLM APIs for text generation/analysis tasks
pub struct LlmExecutor {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl LlmExecutor {
    pub fn new(api_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl TaskExecutor for LlmExecutor {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
        let start_time = Utc::now();

        let prompt = task
            .payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| kias_common::error::KiasError::Config("Missing 'prompt' in task payload".to_string()))?;

        let max_tokens = task
            .payload
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(1024);

        tracing::info!(task_id = %task.id, model = %self.model, "Executing LLM task");

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "max_tokens": max_tokens,
        });

        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        let end_time = Utc::now();

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();

                if status >= 200 && status < 300 {
                    let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&body);
                    let content = parsed
                        .ok()
                        .and_then(|v| v["choices"].as_array()?.first()?.get("message")?.get("content")?.as_str().map(|s| s.to_string()));

                    Ok(TaskResult {
                        task_id: task.id.clone(),
                        status: TaskStatus::Completed,
                        output: Some(serde_json::json!({
                            "response": content.unwrap_or_else(|| body.clone()),
                            "raw": body,
                        })),
                        error: None,
                        started_at: start_time,
                        completed_at: end_time,
                    })
                } else {
                    Ok(TaskResult {
                        task_id: task.id.clone(),
                        status: TaskStatus::Failed,
                        output: Some(serde_json::json!({"raw": body})),
                        error: Some(format!("LLM API returned status {}: {}", status, &body[..body.len().min(200)])),
                        started_at: start_time,
                        completed_at: end_time,
                    })
                }
            }
            Err(e) => Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Failed,
                output: None,
                error: Some(format!("LLM request failed: {}", e)),
                started_at: start_time,
                completed_at: end_time,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockExecutor {
        fail_count: std::sync::atomic::AtomicU32,
        fail_until: u32,
        delay: Option<std::time::Duration>,
    }

    impl MockExecutor {
        fn succeed() -> Self {
            Self {
                fail_count: std::sync::atomic::AtomicU32::new(0),
                fail_until: 0,
                delay: None,
            }
        }

        fn fail_always() -> Self {
            Self {
                fail_count: std::sync::atomic::AtomicU32::new(0),
                fail_until: u32::MAX,
                delay: None,
            }
        }

        fn fail_n_times(n: u32) -> Self {
            Self {
                fail_count: std::sync::atomic::AtomicU32::new(0),
                fail_until: n,
                delay: None,
            }
        }

        fn with_delay(delay: std::time::Duration) -> Self {
            Self {
                fail_count: std::sync::atomic::AtomicU32::new(0),
                fail_until: 0,
                delay: Some(delay),
            }
        }
    }

    #[async_trait]
    impl TaskExecutor for MockExecutor {
        async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }

            let count = self.fail_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < self.fail_until {
                return Ok(TaskResult {
                    task_id: task.id.clone(),
                    status: TaskStatus::Failed,
                    output: None,
                    error: Some(format!("fail attempt {}", count)),
                    started_at: Utc::now(),
                    completed_at: Utc::now(),
                });
            }
            Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Completed,
                output: Some(json!({"result": "success"})),
                error: None,
                started_at: Utc::now(),
                completed_at: Utc::now(),
            })
        }
    }

    fn make_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            name: "test".to_string(),
            agent_id: "agent-1".to_string(),
            payload: json!({"command": "echo test"}),
            created_at: Utc::now(),
            timeout: None,
        }
    }

    fn make_task_with_timeout(id: &str, timeout_ms: u64) -> Task {
        Task {
            id: id.to_string(),
            name: "test".to_string(),
            agent_id: "agent-1".to_string(),
            payload: json!({"command": "echo test"}),
            created_at: Utc::now(),
            timeout: Some(std::time::Duration::from_millis(timeout_ms)),
        }
    }

    #[tokio::test]
    async fn test_runtime_success() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::succeed()));
        let result = runtime.run_task(&make_task("t1")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_runtime_failure() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::fail_always()));
        let result = runtime.run_task(&make_task("t2")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_runtime_retry_success() {
        let runtime = TaskRuntime::with_retries(Box::new(MockExecutor::fail_n_times(2)), 3);
        let result = runtime.run_task(&make_task("t3")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_runtime_retry_exhausted() {
        let runtime = TaskRuntime::with_retries(Box::new(MockExecutor::fail_n_times(5)), 2);
        let result = runtime.run_task(&make_task("t4")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_runtime_max_retries() {
        let runtime = TaskRuntime::with_retries(Box::new(MockExecutor::succeed()), 5);
        assert_eq!(runtime.max_retries(), 5);
    }

    #[tokio::test]
    async fn test_runtime_preserves_task_id() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::succeed()));
        let result = runtime.run_task(&make_task("my-task")).await.unwrap();
        assert_eq!(result.task_id, "my-task");
    }

    #[tokio::test]
    async fn test_task_level_timeout_enforcement() {
        // Task with 100ms timeout, executor takes 500ms
        let runtime = TaskRuntime::new(Box::new(MockExecutor::with_delay(std::time::Duration::from_millis(500))));
        let task = make_task_with_timeout("timeout-task", 100);
        let result = runtime.run_task(&task).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_global_timeout_enforcement() {
        let runtime = TaskRuntime::with_global_timeout(
            Box::new(MockExecutor::with_delay(std::time::Duration::from_millis(500))),
            std::time::Duration::from_millis(100),
        );
        let result = runtime.run_task(&make_task("t")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_task_timeout_overrides_global() {
        // Global timeout 500ms, task timeout 100ms -> task timeout wins
        let runtime = TaskRuntime::with_retries_and_timeout(
            Box::new(MockExecutor::with_delay(std::time::Duration::from_millis(300))),
            0,
            std::time::Duration::from_millis(500),
        );
        let task = make_task_with_timeout("t", 100);
        let result = runtime.run_task(&task).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellable_runtime_cancelled() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::succeed()));
        let cancellable = CancellableRuntime::new(runtime);
        cancellable.cancel();

        let result = cancellable.run_task(&make_task("t")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancellable_runtime_not_cancelled() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::succeed()));
        let cancellable = CancellableRuntime::new(runtime);

        let result = cancellable.run_task(&make_task("t")).await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_run_tasks_concurrent() {
        let runtime = TaskRuntime::new(Box::new(MockExecutor::succeed()));
        let tasks = vec![
            make_task("t1"),
            make_task("t2"),
            make_task("t3"),
        ];
        let results = runtime.run_tasks(&tasks, 2).await;
        assert_eq!(results.len(), 3);
        for result in results {
            assert_eq!(result.unwrap().status, TaskStatus::Completed);
        }
    }

    #[tokio::test]
    async fn test_global_timeout_accessor() {
        let runtime = TaskRuntime::with_global_timeout(
            Box::new(MockExecutor::succeed()),
            std::time::Duration::from_secs(10),
        );
        assert_eq!(runtime.global_timeout(), Some(std::time::Duration::from_secs(10)));
    }
}
