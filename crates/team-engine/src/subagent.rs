//! # SubAgent Orchestration
//!
//! Declarative sub-agent specs with sync/async delegation modes.
//!
//! Sub-agents are defined via YAML spec files in `workspace/subagents/`.
//! A parent agent can delegate tasks to a sub-agent either synchronously
//! (blocking) or asynchronously (poll-based).
//!
//! ## Spec File Format
//!
//! ```yaml
//! ---
//! name: code-reviewer
//! description: Reviews code for quality and correctness
//! tools:
//!   - read_file
//!   - search_code
//! max_depth: 2
//! ---
//! You are a senior code reviewer. Review code for bugs, style issues,
//! and architectural concerns. Be thorough but constructive.
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Errors specific to sub-agent operations
#[derive(Debug, thiserror::Error)]
pub enum SubAgentError {
    #[error("Sub-agent not found: {0}")]
    NotFound(String),

    #[error("Max delegation depth ({max}) exceeded for agent {agent}")]
    DepthExceeded { agent: String, max: u32 },

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Spec parse error: {0}")]
    SpecParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Task failed: {0}")]
    TaskFailed(String),
}

/// Declarative specification for a sub-agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentSpec {
    /// Unique name used as the lookup key
    pub name: String,
    /// Human-readable description of the agent's purpose
    pub description: String,
    /// System prompt (from YAML body)
    #[serde(default)]
    pub system_prompt: String,
    /// Tools this sub-agent is allowed to use
    #[serde(default)]
    pub tools: Vec<String>,
    /// Maximum nesting depth (default 1 = no sub-sub-agents)
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
}

fn default_max_depth() -> u32 {
    1
}

/// Delegation mode: synchronous or asynchronous
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DelegationMode {
    /// Block until the sub-agent completes
    Sync,
    /// Return a task handle immediately, poll for result later
    Async,
}

/// Status of an async delegated task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
}

/// Handle for an asynchronously delegated task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHandle {
    /// Unique task identifier
    pub task_id: String,
    /// Which sub-agent is handling it
    pub agent_name: String,
    /// Current status
    pub status: TaskStatus,
    /// Result payload (populated when Done or Failed)
    pub result: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task completed (if it has)
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskHandle {
    /// Check if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, TaskStatus::Done | TaskStatus::Failed)
    }
}

/// The result of a delegation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationOutcome {
    /// The sub-agent that handled it
    pub agent_name: String,
    /// Whether it succeeded
    pub success: bool,
    /// Output payload
    pub output: serde_json::Value,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Trait for executing a sub-agent task. Implementors define the actual
/// LLM invocation or tool-based execution.
#[async_trait::async_trait]
pub trait SubAgentExecutor: Send + Sync {
    /// Execute a task for the given sub-agent spec.
    /// Returns the output as a JSON value.
    async fn execute(
        &self,
        spec: &SubAgentSpec,
        task: &str,
        context: &serde_json::Value,
    ) -> Result<serde_json::Value, SubAgentError>;
}

/// A no-op executor for testing that echoes back the task
pub struct EchoExecutor;

#[async_trait::async_trait]
impl SubAgentExecutor for EchoExecutor {
    async fn execute(
        &self,
        spec: &SubAgentSpec,
        task: &str,
        _context: &serde_json::Value,
    ) -> Result<serde_json::Value, SubAgentError> {
        Ok(serde_json::json!({
            "agent": spec.name,
            "task": task,
            "echo": true,
        }))
    }
}

/// A failing executor for testing error paths
pub struct FailingExecutor;

#[async_trait::async_trait]
impl SubAgentExecutor for FailingExecutor {
    async fn execute(
        &self,
        _spec: &SubAgentSpec,
        _task: &str,
        _context: &serde_json::Value,
    ) -> Result<serde_json::Value, SubAgentError> {
        Err(SubAgentError::TaskFailed("intentional failure".to_string()))
    }
}

/// Registry of sub-agent specs loaded from YAML files
#[derive(Debug)]
pub struct SubAgentRegistry {
    specs: HashMap<String, SubAgentSpec>,
}

impl SubAgentRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            specs: HashMap::new(),
        }
    }

    /// Load all sub-agent specs from a directory of YAML files.
    /// Each file has YAML front matter (between `---` delimiters) for the spec
    /// fields, and the body becomes the system prompt.
    pub fn load_from_dir(dir: &Path) -> Result<Self, SubAgentError> {
        let mut registry = Self::new();
        if !dir.exists() {
            return Ok(registry);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")) {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let spec = Self::parse_spec_file(&content)?;
            registry.specs.insert(spec.name.clone(), spec);
        }
        Ok(registry)
    }

    /// Parse a spec file with YAML front matter and markdown body
    fn parse_spec_file(content: &str) -> Result<SubAgentSpec, SubAgentError> {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return Err(SubAgentError::SpecParseError(
                "File must start with YAML front matter (---)".to_string(),
            ));
        }

        // Find the closing ---
        let rest = &trimmed[3..];
        let end = rest
            .find("\n---")
            .or_else(|| rest.find("\r\n---"))
            .ok_or_else(|| {
                SubAgentError::SpecParseError("Missing closing --- for front matter".to_string())
            })?;

        let front_matter = &rest[..end];
        let body_start = rest[end + 4..].trim();

        let mut spec: SubAgentSpec = serde_yaml::from_str(front_matter)?;
        spec.system_prompt = body_start.to_string();
        Ok(spec)
    }

    /// Register a spec manually
    pub fn register(&mut self, spec: SubAgentSpec) {
        self.specs.insert(spec.name.clone(), spec);
    }

    /// Look up a sub-agent by name
    pub fn get(&self, name: &str) -> Option<&SubAgentSpec> {
        self.specs.get(name)
    }

    /// List all registered sub-agent names
    pub fn list_names(&self) -> Vec<&str> {
        self.specs.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered sub-agents
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

impl Default for SubAgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Runner that manages delegation to sub-agents with depth tracking
pub struct SubAgentRunner {
    registry: Arc<SubAgentRegistry>,
    executor: Arc<dyn SubAgentExecutor>,
    /// Active async tasks: task_id -> TaskHandle
    tasks: Arc<Mutex<HashMap<String, TaskHandle>>>,
    /// Current delegation depth (per top-level call chain)
    current_depth: Arc<Mutex<u32>>,
}

impl SubAgentRunner {
    /// Create a new runner with the given registry and executor
    pub fn new(registry: Arc<SubAgentRegistry>, executor: Arc<dyn SubAgentExecutor>) -> Self {
        Self {
            registry,
            executor,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            current_depth: Arc::new(Mutex::new(0)),
        }
    }

    /// Delegate a task to a sub-agent synchronously (blocking).
    ///
    /// Returns the outcome directly. Fails if max depth is exceeded.
    pub async fn delegate_sync(
        &self,
        agent_name: &str,
        task: &str,
        context: &serde_json::Value,
    ) -> Result<DelegationOutcome, SubAgentError> {
        let spec = self
            .registry
            .get(agent_name)
            .ok_or_else(|| SubAgentError::NotFound(agent_name.to_string()))?;

        // Check depth
        {
            let depth = self.current_depth.lock().await;
            if *depth >= spec.max_depth {
                return Err(SubAgentError::DepthExceeded {
                    agent: agent_name.to_string(),
                    max: spec.max_depth,
                });
            }
        }

        // Increment depth
        {
            let mut depth = self.current_depth.lock().await;
            *depth += 1;
        }

        let start = std::time::Instant::now();
        let result = self.executor.execute(spec, task, context).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Decrement depth
        {
            let mut depth = self.current_depth.lock().await;
            *depth = depth.saturating_sub(1);
        }

        match result {
            Ok(output) => Ok(DelegationOutcome {
                agent_name: agent_name.to_string(),
                success: true,
                output,
                error: None,
                duration_ms,
            }),
            Err(e) => Ok(DelegationOutcome {
                agent_name: agent_name.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms,
            }),
        }
    }

    /// Delegate a task to a sub-agent asynchronously.
    ///
    /// Returns a `TaskHandle` immediately. The task runs in the background.
    /// Use `poll_result()` to check on it.
    pub async fn delegate_async(
        &self,
        agent_name: &str,
        task: &str,
        context: &serde_json::Value,
    ) -> Result<TaskHandle, SubAgentError> {
        let spec = self
            .registry
            .get(agent_name)
            .ok_or_else(|| SubAgentError::NotFound(agent_name.to_string()))?
            .clone();

        // Check depth
        {
            let depth = self.current_depth.lock().await;
            if *depth >= spec.max_depth {
                return Err(SubAgentError::DepthExceeded {
                    agent: agent_name.to_string(),
                    max: spec.max_depth,
                });
            }
        }

        let task_id = Uuid::new_v4().to_string();
        let handle = TaskHandle {
            task_id: task_id.clone(),
            agent_name: agent_name.to_string(),
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        // Insert into tasks map
        self.tasks.lock().await.insert(task_id.clone(), handle.clone());

        // Spawn the task
        let executor = Arc::clone(&self.executor);
        let tasks_map = Arc::clone(&self.tasks);
        let task_str = task.to_string();
        let ctx = context.clone();
        let depth_mutex = Arc::clone(&self.current_depth);

        tokio::spawn(async move {
            // Mark running
            {
                let mut map = tasks_map.lock().await;
                if let Some(h) = map.get_mut(&task_id) {
                    h.status = TaskStatus::Running;
                }
            }

            // Increment depth
            {
                let mut depth = depth_mutex.lock().await;
                *depth += 1;
            }

            let start = std::time::Instant::now();
            let result = executor.execute(&spec, &task_str, &ctx).await;
            let _duration_ms = start.elapsed().as_millis() as u64;

            // Decrement depth
            {
                let mut depth = depth_mutex.lock().await;
                *depth = depth.saturating_sub(1);
            }

            // Mark done/failed
            let mut map = tasks_map.lock().await;
            if let Some(h) = map.get_mut(&task_id) {
                h.completed_at = Some(Utc::now());
                match result {
                    Ok(output) => {
                        h.status = TaskStatus::Done;
                        h.result = Some(output);
                    }
                    Err(e) => {
                        h.status = TaskStatus::Failed;
                        h.error = Some(e.to_string());
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Poll the result of an async task.
    ///
    /// Returns the current `TaskHandle` snapshot. Check `.status` to see
    /// if it's `Done` or `Failed`, then read `.result` or `.error`.
    pub async fn poll_result(&self, task_id: &str) -> Result<TaskHandle, SubAgentError> {
        let map = self.tasks.lock().await;
        map.get(task_id)
            .cloned()
            .ok_or_else(|| SubAgentError::TaskNotFound(task_id.to_string()))
    }

    /// Get the number of registered sub-agents
    pub fn agent_count(&self) -> usize {
        self.registry.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec(name: &str) -> SubAgentSpec {
        SubAgentSpec {
            name: name.to_string(),
            description: format!("Test agent: {name}"),
            system_prompt: "You are a test agent.".to_string(),
            tools: vec!["read_file".to_string(), "write_file".to_string()],
            max_depth: 2,
        }
    }

    fn make_registry_with_agents() -> SubAgentRegistry {
        let mut reg = SubAgentRegistry::new();
        reg.register(make_spec("reviewer"));
        reg.register(make_spec("writer"));
        reg
    }

    // ── SubAgentSpec tests ──────────────────────────────────────────

    #[test]
    fn test_spec_defaults() {
        let yaml = r#"
name: helper
description: A helpful agent
"#;
        let spec: SubAgentSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.name, "helper");
        assert_eq!(spec.description, "A helpful agent");
        assert!(spec.tools.is_empty());
        assert_eq!(spec.max_depth, 1);
    }

    #[test]
    fn test_spec_with_all_fields() {
        let yaml = r#"
name: coder
description: Writes code
tools:
  - read_file
  - write_file
  - terminal
max_depth: 3
"#;
        let spec: SubAgentSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.name, "coder");
        assert_eq!(spec.tools.len(), 3);
        assert_eq!(spec.max_depth, 3);
    }

    // ── SubAgentRegistry tests ──────────────────────────────────────

    #[test]
    fn test_registry_register_and_get() {
        let reg = make_registry_with_agents();
        assert_eq!(reg.len(), 2);
        assert!(reg.get("reviewer").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list_names() {
        let reg = make_registry_with_agents();
        let mut names = reg.list_names();
        names.sort();
        assert_eq!(names, vec!["reviewer", "writer"]);
    }

    #[test]
    fn test_parse_spec_file() {
        let content = r#"---
name: code-reviewer
description: Reviews code
tools:
  - read_file
max_depth: 2
---
You are a senior code reviewer. Be thorough."#;

        let spec = SubAgentRegistry::parse_spec_file(content).unwrap();
        assert_eq!(spec.name, "code-reviewer");
        assert_eq!(spec.max_depth, 2);
        assert_eq!(spec.tools, vec!["read_file"]);
        assert!(spec.system_prompt.contains("senior code reviewer"));
    }

    #[test]
    fn test_parse_spec_file_no_front_matter() {
        let content = "just some text without front matter";
        let result = SubAgentRegistry::parse_spec_file(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_is_empty_default() {
        let reg = SubAgentRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    // ── SubAgentRunner tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_sync_delegation_success() {
        let mut reg = SubAgentRegistry::new();
        reg.register(make_spec("reviewer"));

        let runner = SubAgentRunner::new(
            Arc::new(reg),
            Arc::new(EchoExecutor),
        );

        let outcome = runner
            .delegate_sync("reviewer", "Review this code", &serde_json::json!({}))
            .await
            .unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.agent_name, "reviewer");
        assert_eq!(outcome.output["echo"], true);
    }

    #[tokio::test]
    async fn test_sync_delegation_not_found() {
        let reg = SubAgentRegistry::new();
        let runner = SubAgentRunner::new(Arc::new(reg), Arc::new(EchoExecutor));

        let result = runner
            .delegate_sync("ghost", "do something", &serde_json::json!({}))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubAgentError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_sync_delegation_failure() {
        let mut reg = SubAgentRegistry::new();
        reg.register(make_spec("failer"));

        let runner = SubAgentRunner::new(Arc::new(reg), Arc::new(FailingExecutor));

        let outcome = runner
            .delegate_sync("failer", "fail please", &serde_json::json!({}))
            .await
            .unwrap();

        assert!(!outcome.success);
        assert!(outcome.error.is_some());
        assert!(outcome.error.unwrap().contains("intentional failure"));
    }

    #[tokio::test]
    async fn test_async_delegation_poll_result() {
        let mut reg = SubAgentRegistry::new();
        reg.register(make_spec("async-reviewer"));

        let runner = SubAgentRunner::new(
            Arc::new(reg),
            Arc::new(EchoExecutor),
        );

        let handle = runner
            .delegate_async("async-reviewer", "Review PR", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(handle.status, TaskStatus::Pending);
        assert!(!handle.is_terminal());

        // Wait a bit for the background task to finish
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let updated = runner.poll_result(&handle.task_id).await.unwrap();
        assert_eq!(updated.status, TaskStatus::Done);
        assert!(updated.is_terminal());
        assert!(updated.result.is_some());
        assert_eq!(updated.result.unwrap()["echo"], true);
    }

    #[tokio::test]
    async fn test_poll_nonexistent_task() {
        let reg = SubAgentRegistry::new();
        let runner = SubAgentRunner::new(Arc::new(reg), Arc::new(EchoExecutor));

        let result = runner.poll_result("nonexistent-id").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubAgentError::TaskNotFound(_)));
    }

    #[tokio::test]
    async fn test_depth_limit_enforced() {
        let mut reg = SubAgentRegistry::new();
        let mut spec = make_spec("shallow");
        spec.max_depth = 0; // no nesting allowed
        reg.register(spec);

        let runner = SubAgentRunner::new(
            Arc::new(reg),
            Arc::new(EchoExecutor),
        );

        let result = runner
            .delegate_sync("shallow", "nested task", &serde_json::json!({}))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SubAgentError::DepthExceeded { .. }));
    }

    #[tokio::test]
    async fn test_async_task_handle_fields() {
        let mut reg = SubAgentRegistry::new();
        reg.register(make_spec("agent-x"));

        let runner = SubAgentRunner::new(
            Arc::new(reg),
            Arc::new(EchoExecutor),
        );

        let handle = runner
            .delegate_async("agent-x", "some task", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(handle.agent_name, "agent-x");
        assert!(!handle.task_id.is_empty());
        assert!(handle.result.is_none());
        assert!(handle.error.is_none());
        assert!(handle.completed_at.is_none());
    }

    #[test]
    fn test_task_handle_terminal_states() {
        let done = TaskHandle {
            task_id: "t1".into(),
            agent_name: "a".into(),
            status: TaskStatus::Done,
            result: Some(serde_json::json!({})),
            error: None,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        assert!(done.is_terminal());

        let failed = TaskHandle {
            task_id: "t2".into(),
            agent_name: "a".into(),
            status: TaskStatus::Failed,
            result: None,
            error: Some("boom".into()),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        assert!(failed.is_terminal());

        let pending = TaskHandle {
            task_id: "t3".into(),
            agent_name: "a".into(),
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
        };
        assert!(!pending.is_terminal());
    }

    #[test]
    fn test_delegation_mode_equality() {
        assert_eq!(DelegationMode::Sync, DelegationMode::Sync);
        assert_eq!(DelegationMode::Async, DelegationMode::Async);
        assert_ne!(DelegationMode::Sync, DelegationMode::Async);
    }
}
