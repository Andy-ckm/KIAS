use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Workflow node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub config: serde_json::Value,
    /// Optional executor configuration for this node.
    /// When set, the engine dispatches to the appropriate executor.
    pub executor: Option<ExecutorConfig>,
    /// Optional compensating action for saga rollback.
    /// When set, the engine executes this action in reverse order if a
    /// downstream node fails (saga pattern).
    pub compensating_action: Option<CompensatingAction>,
}

/// A compensating action for saga-pattern rollback.
///
/// When a workflow fails mid-execution, the engine walks the list of
/// successfully-completed nodes in reverse and executes each one's
/// `CompensatingAction` to undo its side-effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensatingAction {
    /// Human-readable description (for logging / debugging).
    pub description: String,
    /// The executor to run for compensation.
    pub executor: ExecutorConfig,
}

/// Node types (inspired by LangGraph)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeType {
    /// Processing node (executes a task)
    Process,
    /// Conditional branching node
    Condition,
    /// Parallel fork node
    Fork,
    /// Parallel join node
    Join,
    /// Sub-workflow node
    SubWorkflow,
    /// Human-in-the-loop node
    HumanReview,
}

/// Executor configuration — determines what a Process node actually does.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecutorConfig {
    /// Execute a shell command
    Shell {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        working_dir: Option<String>,
        #[serde(default)]
        timeout_secs: Option<u64>,
    },
    /// Make an HTTP request
    Http {
        method: String,
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        body: Option<serde_json::Value>,
        #[serde(default)]
        timeout_secs: Option<u64>,
    },
    /// Invoke an LLM (mock / placeholder)
    Llm {
        model: String,
        prompt: String,
        #[serde(default)]
        temperature: Option<f64>,
        #[serde(default)]
        max_tokens: Option<u32>,
    },
    /// Execute a sub-workflow by graph ID
    SubWorkflow { workflow_id: String },
}

/// Retry policy for node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of attempts (1 = no retry).
    pub max_attempts: u32,
    /// Base delay between retries.
    pub base_delay: Duration,
    /// Maximum delay (for exponential back-off cap).
    pub max_delay: Duration,
    /// Back-off multiplier.
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Compute the delay before the *next* attempt (exponential back-off).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let base_ms = self.base_delay.as_millis() as f64;
        let delay_ms = base_ms * self.backoff_multiplier.powi(attempt as i32 - 1);
        let delay = Duration::from_millis(delay_ms as u64);
        std::cmp::min(delay, self.max_delay)
    }
}

/// Result of executing a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded.
    pub success: bool,
    /// Captured stdout (for shell nodes).
    pub stdout: Option<String>,
    /// Captured stderr (for shell nodes).
    pub stderr: Option<String>,
    /// Structured output data.
    pub output: serde_json::Value,
    /// Error message when `success` is false.
    pub error: Option<String>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// How many retry attempts were needed.
    pub retries: u32,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            success: true,
            stdout: None,
            stderr: None,
            output: serde_json::json!({}),
            error: None,
            duration_ms: 0,
            retries: 0,
        }
    }
}

impl ExecutionResult {
    pub fn success(output: serde_json::Value) -> Self {
        Self {
            success: true,
            output,
            ..Default::default()
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            ..Default::default()
        }
    }
}

impl Node {
    pub fn new(id: &str, name: &str, node_type: NodeType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            node_type,
            config: serde_json::json!({}),
            executor: None,
            compensating_action: None,
        }
    }

    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }

    pub fn with_executor(mut self, executor: ExecutorConfig) -> Self {
        self.executor = Some(executor);
        self
    }

    pub fn with_compensating_action(mut self, action: CompensatingAction) -> Self {
        self.compensating_action = Some(action);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::new("n1", "Test", NodeType::Process);
        assert_eq!(node.id, "n1");
        assert_eq!(node.name, "Test");
        assert_eq!(node.node_type, NodeType::Process);
        assert!(node.executor.is_none());
    }

    #[test]
    fn test_node_with_executor() {
        let exec = ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["hello".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: Some(10),
        };
        let node = Node::new("n1", "Shell", NodeType::Process).with_executor(exec);
        assert!(node.executor.is_some());
    }

    #[test]
    fn test_retry_policy_delay() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        };
        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_max_delay_cap() {
        let policy = RetryPolicy {
            max_attempts: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 3.0,
        };
        // Attempt 4 would be 1 * 3^3 = 27s, but capped at 5s
        assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(5));
    }

    #[test]
    fn test_execution_result_success() {
        let r = ExecutionResult::success(serde_json::json!({"key": "value"}));
        assert!(r.success);
        assert!(r.error.is_none());
        assert_eq!(r.output["key"], "value");
    }

    #[test]
    fn test_execution_result_failure() {
        let r = ExecutionResult::failure("something broke");
        assert!(!r.success);
        assert_eq!(r.error.as_deref(), Some("something broke"));
    }

    #[test]
    fn test_node_builder_chain() {
        let node = Node::new("n1", "test", NodeType::Process)
            .with_config(serde_json::json!({"key": "value"}))
            .with_executor(ExecutorConfig::Llm {
                model: "gpt-4".into(),
                prompt: "hello".into(),
                temperature: Some(0.7),
                max_tokens: Some(100),
            });
        assert_eq!(node.config["key"], "value");
        assert!(node.executor.is_some());
    }

    #[test]
    fn test_node_with_compensating_action() {
        let comp = CompensatingAction {
            description: "Delete the created resource".into(),
            executor: ExecutorConfig::Shell {
                command: "rm".into(),
                args: vec!["-rf".into(), "/tmp/resource".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: Some(10),
            },
        };
        let node = Node::new("n1", "Create", NodeType::Process).with_compensating_action(comp);
        assert!(node.compensating_action.is_some());
        assert_eq!(
            node.compensating_action.as_ref().unwrap().description,
            "Delete the created resource"
        );
    }
}
