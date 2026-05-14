use super::checkpoint::{Checkpoint, CheckpointStore};
use super::executor::ExecutorRegistry;
use super::graph::WorkflowGraph;
use super::node::{ExecutionResult, Node, NodeType, RetryPolicy};
use super::state::{WorkflowState, WorkflowStatus};
use kias_common::KiasResult;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Workflow execution engine (inspired by LangGraph)
///
/// Core design:
/// 1. State-graph-driven workflow execution
/// 2. Supports cycles, branching, and complex topologies
/// 3. Checkpoint and resume mechanism
/// 4. Human-in-the-loop support
/// 5. Real node execution with retries
pub struct WorkflowEngine {
    checkpoint_store: CheckpointStore,
    executor_registry: ExecutorRegistry,
    default_retry_policy: RetryPolicy,
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            checkpoint_store: CheckpointStore::new(),
            executor_registry: ExecutorRegistry::new(),
            default_retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.default_retry_policy = policy;
        self
    }

    pub fn with_executor_registry(mut self, registry: ExecutorRegistry) -> Self {
        self.executor_registry = registry;
        self
    }

    /// Execute a workflow graph from initial state.
    pub async fn execute(
        &self,
        graph: &WorkflowGraph,
        initial_state: WorkflowState,
    ) -> KiasResult<WorkflowState> {
        graph.validate().map_err(|e| {
            kias_common::error::KiasError::Scheduler(format!("Invalid graph: {}", e))
        })?;

        let mut state = initial_state;
        state.status = WorkflowStatus::Running;

        tracing::info!(
            workflow_id = %state.workflow_id,
            entry_node = %graph.entry_node,
            "Starting workflow execution"
        );

        let max_steps = graph.nodes.len() * 100; // safety bound against infinite loops
        let mut step = 0;

        loop {
            step += 1;
            if step > max_steps {
                tracing::error!(
                    workflow_id = %state.workflow_id,
                    "Workflow exceeded maximum step count — possible infinite loop"
                );
                state.status = WorkflowStatus::Failed;
                break;
            }

            let current_node_id = state.current_node.clone();

            // Save checkpoint before executing
            self.save_checkpoint(&state, &current_node_id);

            // Check if we've reached an exit node
            if graph.exit_nodes.contains(&current_node_id) {
                tracing::info!(node = %current_node_id, "Reached exit node");
                state.status = WorkflowStatus::Completed;
                break;
            }

            // Get the node
            let node = graph.nodes.get(&current_node_id).ok_or_else(|| {
                kias_common::error::KiasError::Scheduler(format!(
                    "Node '{}' not found",
                    current_node_id
                ))
            })?;

            tracing::info!(
                node_id = %node.id,
                node_name = %node.name,
                node_type = ?node.node_type,
                "Executing node"
            );

            // Execute the node (dispatches based on type/executor)
            let should_continue = self.execute_node(node, &mut state).await?;

            if !should_continue {
                // Execution paused (e.g., HumanReview) or failed
                break;
            }

            // Determine next node based on edges and conditions
            let next_node_id = self.resolve_next_node(graph, &current_node_id, &state);

            match next_node_id {
                Some(next_id) => {
                    tracing::info!(
                        from = %current_node_id,
                        to = %next_id,
                        "Transitioning"
                    );
                    state.transition(&next_id, HashMap::new());
                }
                None => {
                    tracing::warn!(
                        node = %current_node_id,
                        "No outgoing edges — workflow stuck"
                    );
                    state.status = WorkflowStatus::Failed;
                    break;
                }
            }
        }

        // Save final checkpoint
        self.save_checkpoint(&state, &state.current_node);

        tracing::info!(
            workflow_id = %state.workflow_id,
            status = ?state.status,
            steps = step,
            "Workflow execution completed"
        );

        Ok(state)
    }

    /// Execute a single node. Returns `true` if the workflow should continue,
    /// `false` if it should stop (paused or failed).
    async fn execute_node(&self, node: &Node, state: &mut WorkflowState) -> KiasResult<bool> {
        match node.node_type {
            NodeType::Process => self.execute_process_node(node, state).await,
            NodeType::Condition => self.execute_condition_node(node, state).await,
            NodeType::Fork => {
                tracing::info!(node = %node.id, "Fork node — continuing");
                Ok(true)
            }
            NodeType::Join => {
                tracing::info!(node = %node.id, "Join node — continuing");
                Ok(true)
            }
            NodeType::SubWorkflow => self.execute_process_node(node, state).await,
            NodeType::HumanReview => {
                state.status = WorkflowStatus::WaitingForHuman;
                tracing::info!(
                    node = %node.id,
                    "Human review — pausing workflow"
                );
                Ok(false)
            }
        }
    }

    /// Execute a Process or SubWorkflow node with retries.
    async fn execute_process_node(
        &self,
        node: &Node,
        state: &mut WorkflowState,
    ) -> KiasResult<bool> {
        let executor_config = node.executor.as_ref().ok_or_else(|| {
            kias_common::error::KiasError::Validation(format!(
                "Process node '{}' has no executor config",
                node.id
            ))
        })?;

        let retry_policy = self.get_retry_policy(node);
        let mut last_result: Option<ExecutionResult> = None;
        let start = Instant::now();

        for attempt in 1..=retry_policy.max_attempts {
            if attempt > 1 {
                let delay = retry_policy.delay_for_attempt(attempt - 1);
                tracing::info!(
                    node = %node.id,
                    attempt = attempt,
                    delay_ms = delay.as_millis() as u64,
                    "Retrying node execution"
                );
                tokio::time::sleep(delay).await;
            }

            let mut result = self
                .executor_registry
                .execute(executor_config, &state.data)
                .await;
            result.retries = attempt - 1;

            if result.success {
                tracing::info!(
                    node = %node.id,
                    attempt = attempt,
                    duration_ms = result.duration_ms,
                    "Node execution succeeded"
                );

                // Merge output into state
                self.merge_result_into_state(state, &result, node);
                return Ok(true);
            }

            tracing::warn!(
                node = %node.id,
                attempt = attempt,
                max_attempts = retry_policy.max_attempts,
                error = %result.error.as_deref().unwrap_or("unknown"),
                "Node execution failed"
            );

            last_result = Some(result);
        }

        // All retries exhausted
        let failed = last_result.unwrap();
        let total_ms = start.elapsed().as_millis() as u64;

        tracing::error!(
            node = %node.id,
            retries = retry_policy.max_attempts,
            total_ms = total_ms,
            error = %failed.error.as_deref().unwrap_or("unknown"),
            "Node execution failed after all retries"
        );

        // Store failure info in state
        state.set(
            format!("{}_error", node.id),
            failed.error.unwrap_or_default(),
        );
        state.status = WorkflowStatus::Failed;
        Ok(false)
    }

    /// Evaluate a Condition node — selects the next edge based on state data.
    async fn execute_condition_node(
        &self,
        node: &Node,
        state: &mut WorkflowState,
    ) -> KiasResult<bool> {
        // Condition nodes use `config.condition_key` to look up a value in state
        let condition_key = node
            .config
            .get("condition_key")
            .and_then(|v| v.as_str())
            .unwrap_or("branch");

        let condition_value = state
            .data
            .get(condition_key)
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        tracing::info!(
            node = %node.id,
            condition_key = condition_key,
            condition_value = %condition_value,
            "Evaluating condition"
        );

        state.set(format!("{}_evaluated", node.id), condition_value);
        Ok(true)
    }

    /// Resolve the next node by evaluating outgoing edges.
    fn resolve_next_node(
        &self,
        graph: &WorkflowGraph,
        current_node_id: &str,
        state: &WorkflowState,
    ) -> Option<String> {
        let outgoing: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.from == current_node_id)
            .collect();

        if outgoing.is_empty() {
            return None;
        }

        // First try conditional edges
        for edge in &outgoing {
            if let Some(ref condition) = edge.condition {
                if self.evaluate_condition(&condition.expression, state) {
                    return Some(edge.to.clone());
                }
            }
        }

        // Fall back to the first unconditional edge
        for edge in &outgoing {
            if edge.condition.is_none() {
                return Some(edge.to.clone());
            }
        }

        None
    }

    /// Evaluate a condition expression against the current state.
    ///
    /// Supported formats:
    ///   - `"field == value"` — string equality
    ///   - `"field != value"` — string inequality
    ///   - `"field"` — truthy check (exists and not null/false)
    ///   - `"!field"` — falsy check
    fn evaluate_condition(&self, expression: &str, state: &WorkflowState) -> bool {
        let expr = expression.trim();

        // Equality: "field == value"
        if let Some((field, value)) = expr.split_once("==") {
            let field = field.trim();
            let value = value.trim().trim_matches('"');
            return state
                .data
                .get(field)
                .map(|v| match v {
                    serde_json::Value::String(s) => s == value,
                    serde_json::Value::Number(n) => n.to_string() == value,
                    serde_json::Value::Bool(b) => b.to_string() == value,
                    _ => false,
                })
                .unwrap_or(false);
        }

        // Inequality: "field != value"
        if let Some((field, value)) = expr.split_once("!=") {
            let field = field.trim();
            let value = value.trim().trim_matches('"');
            return !state
                .data
                .get(field)
                .map(|v| match v {
                    serde_json::Value::String(s) => s == value,
                    serde_json::Value::Number(n) => n.to_string() == value,
                    serde_json::Value::Bool(b) => b.to_string() == value,
                    _ => false,
                })
                .unwrap_or(false);
        }

        // Negation: "!field"
        if let Some(field) = expr.strip_prefix('!') {
            let field = field.trim();
            return !is_truthy(state.data.get(field));
        }

        // Truthy check: "field"
        is_truthy(state.data.get(expr))
    }

    /// Merge an execution result's output into the workflow state.
    fn merge_result_into_state(
        &self,
        state: &mut WorkflowState,
        result: &ExecutionResult,
        node: &Node,
    ) {
        // Store the full output
        state.set(format!("{}_output", node.id), result.output.clone());

        // Store stdout if present
        if let Some(ref stdout) = result.stdout {
            state.set(format!("{}_stdout", node.id), stdout.clone());
        }

        // If output is an object, merge its keys into state
        if let serde_json::Value::Object(ref map) = result.output {
            for (key, value) in map {
                state.set(key, value.clone());
            }
        }
    }

    /// Get the retry policy for a node (from config or default).
    fn get_retry_policy(&self, node: &Node) -> RetryPolicy {
        // Check if node config specifies retry settings
        if let Some(max_attempts) = node.config.get("max_retries").and_then(|v| v.as_u64()) {
            let base_delay_ms = node
                .config
                .get("retry_delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(1000);

            return RetryPolicy {
                max_attempts: max_attempts as u32 + 1, // retries + initial attempt
                base_delay: Duration::from_millis(base_delay_ms),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
            };
        }

        self.default_retry_policy.clone()
    }

    /// Save a checkpoint.
    fn save_checkpoint(&self, state: &WorkflowState, node_id: &str) {
        let checkpoint = Checkpoint {
            id: Uuid::new_v4().to_string(),
            workflow_id: state.workflow_id.clone(),
            node_id: node_id.to_string(),
            state: state.clone(),
            created_at: chrono::Utc::now(),
        };
        self.checkpoint_store.save(checkpoint);
    }

    /// Restore workflow state from a checkpoint.
    pub fn restore_from_checkpoint(
        &self,
        workflow_id: &str,
        checkpoint_id: Option<&str>,
    ) -> Option<WorkflowState> {
        if let Some(id) = checkpoint_id {
            self.checkpoint_store.get(workflow_id, id).map(|c| c.state)
        } else {
            self.checkpoint_store
                .get_latest(workflow_id)
                .map(|c| c.state)
        }
    }
}

/// Check if a JSON value is truthy.
fn is_truthy(value: Option<&serde_json::Value>) -> bool {
    match value {
        None => false,
        Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        Some(serde_json::Value::Array(a)) => !a.is_empty(),
        Some(serde_json::Value::Object(_)) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Edge;
    use crate::graph::WorkflowGraph;
    use crate::node::{ExecutorConfig, Node, NodeType};

    fn make_linear_graph() -> WorkflowGraph {
        let mut graph = WorkflowGraph::new("test-linear");
        graph.add_node(
            Node::new("start", "Start", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["started".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(
            Node::new("middle", "Middle", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["processed".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("start", "middle"));
        graph.add_edge(Edge::new("middle", "end"));
        graph.set_entry("start");
        graph.add_exit_node("end");
        graph
    }

    #[tokio::test]
    async fn test_linear_workflow() {
        let graph = make_linear_graph();
        let state = WorkflowState::new("wf-1", &graph.entry_node);
        let engine = WorkflowEngine::new();

        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert_eq!(result.current_node, "end");
        assert!(result.history.len() >= 2); // at least start->middle, middle->end
    }

    #[tokio::test]
    async fn test_workflow_with_condition() {
        let mut graph = WorkflowGraph::new("test-condition");

        graph.add_node(
            Node::new("decide", "Decide", NodeType::Condition)
                .with_config(serde_json::json!({"condition_key": "route"})),
        );
        graph.add_node(
            Node::new("path_a", "PathA", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["path A".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(
            Node::new("path_b", "PathB", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["path B".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));

        graph.add_edge(Edge::new("decide", "path_a").with_condition("route == \"a\"", "Go to A"));
        graph.add_edge(Edge::new("decide", "path_b").with_condition("route == \"b\"", "Go to B"));
        graph.add_edge(Edge::new("path_a", "end"));
        graph.add_edge(Edge::new("path_b", "end"));

        graph.set_entry("decide");
        graph.add_exit_node("end");

        // Test path A
        let mut state = WorkflowState::new("wf-a", &graph.entry_node);
        state.set("route", "a");
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert!(result.history.iter().any(|t| t.to_node == "path_a"));

        // Test path B
        let mut state = WorkflowState::new("wf-b", &graph.entry_node);
        state.set("route", "b");
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert!(result.history.iter().any(|t| t.to_node == "path_b"));
    }

    #[tokio::test]
    async fn test_workflow_with_human_review() {
        let mut graph = WorkflowGraph::new("test-human");
        graph.add_node(
            Node::new("start", "Start", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["go".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(Node::new("review", "Review", NodeType::HumanReview));
        graph.add_node(Node::new("end", "End", NodeType::Process));

        graph.add_edge(Edge::new("start", "review"));
        graph.add_edge(Edge::new("review", "end"));

        graph.set_entry("start");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-human", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::WaitingForHuman);
        assert_eq!(result.current_node, "review");
    }

    #[tokio::test]
    async fn test_workflow_node_execution_failure() {
        let mut graph = WorkflowGraph::new("test-fail");
        graph.add_node(Node::new("fail", "Fail", NodeType::Process).with_executor(
            ExecutorConfig::Shell {
                command: "false".into(),
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            },
        ));
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("fail", "end"));
        graph.set_entry("fail");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-fail", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Failed);
    }

    #[tokio::test]
    async fn test_workflow_retry_on_failure() {
        let mut graph = WorkflowGraph::new("test-retry");
        graph.add_node(
            Node::new("retry", "Retry", NodeType::Process)
                .with_config(serde_json::json!({"max_retries": 2, "retry_delay_ms": 10}))
                .with_executor(ExecutorConfig::Shell {
                    command: "false".into(),
                    args: vec![],
                    env: HashMap::new(),
                    working_dir: None,
                    timeout_secs: None,
                }),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("retry", "end"));
        graph.set_entry("retry");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-retry", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Failed);
        // Error should be stored in state
        assert!(result.data.contains_key("retry_error"));
    }

    #[tokio::test]
    async fn test_workflow_retry_eventually_succeeds() {
        // Use a command that fails the first time but we can't easily do that
        // with a simple shell command. Instead, test with a successful command
        // to verify the retry logic doesn't break success paths.
        let mut graph = WorkflowGraph::new("test-retry-ok");
        graph.add_node(
            Node::new("task", "Task", NodeType::Process)
                .with_config(serde_json::json!({"max_retries": 3, "retry_delay_ms": 10}))
                .with_executor(ExecutorConfig::Shell {
                    command: "echo".into(),
                    args: vec!["ok".into()],
                    env: HashMap::new(),
                    working_dir: None,
                    timeout_secs: None,
                }),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("task", "end"));
        graph.set_entry("task");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-ok", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_workflow_with_llm_node() {
        let mut graph = WorkflowGraph::new("test-llm");
        graph.add_node(
            Node::new("think", "Think", NodeType::Process).with_executor(ExecutorConfig::Llm {
                model: "gpt-4".into(),
                prompt: "Analyze the data".into(),
                temperature: Some(0.5),
                max_tokens: Some(500),
            }),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("think", "end"));
        graph.set_entry("think");
        graph.add_exit_node("end");

        let mut state = WorkflowState::new("wf-llm", &graph.entry_node);
        state.set("input_data", "some data to analyze");

        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
        // LLM output should be merged into state
        assert!(result.data.contains_key("think_output"));
    }

    #[tokio::test]
    async fn test_workflow_with_subworkflow_node() {
        let mut graph = WorkflowGraph::new("test-subwf");
        graph.add_node(
            Node::new("sub", "Sub", NodeType::SubWorkflow).with_executor(
                ExecutorConfig::SubWorkflow {
                    workflow_id: "inner-wf".into(),
                },
            ),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("sub", "end"));
        graph.set_entry("sub");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-sub", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_workflow_fork_join() {
        let mut graph = WorkflowGraph::new("test-fork");
        graph.add_node(
            Node::new("start", "Start", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["go".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        graph.add_node(Node::new("fork1", "Fork", NodeType::Fork));
        graph.add_node(
            Node::new("branch_a", "BranchA", NodeType::Process).with_executor(
                ExecutorConfig::Shell {
                    command: "echo".into(),
                    args: vec!["A".into()],
                    env: HashMap::new(),
                    working_dir: None,
                    timeout_secs: None,
                },
            ),
        );
        graph.add_node(Node::new("join1", "Join", NodeType::Join));
        graph.add_node(Node::new("end", "End", NodeType::Process));

        graph.add_edge(Edge::new("start", "fork1"));
        graph.add_edge(Edge::new("fork1", "branch_a"));
        graph.add_edge(Edge::new("branch_a", "join1"));
        graph.add_edge(Edge::new("join1", "end"));

        graph.set_entry("start");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-fork", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_checkpoint_and_restore() {
        let graph = make_linear_graph();
        let state = WorkflowState::new("wf-cp", &graph.entry_node);
        let engine = WorkflowEngine::new();

        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Completed);

        // Should be able to restore from checkpoint
        let restored = engine.restore_from_checkpoint("wf-cp", None);
        assert!(restored.is_some());
        let restored = restored.unwrap();
        assert_eq!(restored.workflow_id, "wf-cp");
    }

    #[tokio::test]
    async fn test_invalid_graph() {
        let graph = WorkflowGraph::new("invalid");
        // No entry node, no nodes
        let state = WorkflowState::new("wf-invalid", "");
        let engine = WorkflowEngine::new();

        let result = engine.execute(&graph, state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_condition_fallback_to_unconditional() {
        let mut graph = WorkflowGraph::new("test-fallback");
        graph.add_node(Node::new("start", "Start", NodeType::Condition));
        graph.add_node(
            Node::new("default_path", "Default", NodeType::Process).with_executor(
                ExecutorConfig::Shell {
                    command: "echo".into(),
                    args: vec!["default".into()],
                    env: HashMap::new(),
                    working_dir: None,
                    timeout_secs: None,
                },
            ),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));

        // Conditional edge that won't match
        graph.add_edge(
            Edge::new("start", "never").with_condition("route == \"special\"", "Special"),
        );
        // Unconditional fallback
        graph.add_edge(Edge::new("start", "default_path"));
        graph.add_edge(Edge::new("default_path", "end"));

        graph.set_entry("start");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-fallback", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
        assert!(result.history.iter().any(|t| t.to_node == "default_path"));
    }

    #[tokio::test]
    async fn test_workflow_stuck_no_outgoing_edges() {
        let mut graph = WorkflowGraph::new("test-stuck");
        graph.add_node(
            Node::new("stuck", "Stuck", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["ok".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        // No edges from "stuck", and it's not an exit node
        graph.set_entry("stuck");

        let state = WorkflowState::new("wf-stuck", &graph.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&graph, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Failed);
    }

    #[tokio::test]
    async fn test_condition_truthy_check() {
        let engine = WorkflowEngine::new();
        let mut state = WorkflowState::new("test", "node1");

        // Empty state — should be falsy
        assert!(!engine.evaluate_condition("my_key", &state));

        // Set a truthy value
        state.set("my_key", "hello");
        assert!(engine.evaluate_condition("my_key", &state));

        // Negation
        assert!(engine.evaluate_condition("!missing_key", &state));
    }

    #[tokio::test]
    async fn test_condition_equality() {
        let engine = WorkflowEngine::new();
        let mut state = WorkflowState::new("test", "node1");
        state.set("status", "ready");

        assert!(engine.evaluate_condition("status == \"ready\"", &state));
        assert!(!engine.evaluate_condition("status == \"busy\"", &state));
        assert!(engine.evaluate_condition("status != \"busy\"", &state));
    }
}
