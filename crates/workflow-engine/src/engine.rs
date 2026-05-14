use super::checkpoint::{Checkpoint, CheckpointStore};
use super::executor::ExecutorRegistry;
use super::graph::WorkflowGraph;
use super::node::{ExecutionResult, Node, NodeType, RetryPolicy};
use super::state::{WorkflowState, WorkflowStatus};
use super::subgraph::SubGraph;
use super::typed_state::EventSink;
use super::typed_state::StreamingEvent;
use kias_common::KiasResult;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Workflow execution engine (inspired by LangGraph)
///
/// Core design:
/// 1. State-graph-driven workflow execution
/// 2. Supports cycles, branching, and complex topologies
/// 3. Checkpoint and resume mechanism
/// 4. Human-in-the-loop support
/// 5. Real node execution with retries
/// 6. Streaming event emission for real-time observability
/// 7. Subgraph composition for hierarchical workflows
pub struct WorkflowEngine {
    checkpoint_store: CheckpointStore,
    executor_registry: ExecutorRegistry,
    default_retry_policy: RetryPolicy,
    event_sink: EventSink,
    subgraphs: HashMap<String, SubGraph>,
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
            event_sink: EventSink::new(),
            subgraphs: HashMap::new(),
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

    /// Set a shared event sink for streaming execution events.
    pub fn with_event_sink(mut self, sink: EventSink) -> Self {
        self.event_sink = sink;
        self
    }

    /// Register a named subgraph for composition.
    pub fn with_subgraph(mut self, name: impl Into<String>, subgraph: SubGraph) -> Self {
        self.subgraphs.insert(name.into(), subgraph);
        self
    }

    /// Get a reference to the event sink.
    pub fn event_sink(&self) -> &EventSink {
        &self.event_sink
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
        let workflow_start = Instant::now();

        tracing::info!(
            workflow_id = %state.workflow_id,
            entry_node = %graph.entry_node,
            "Starting workflow execution"
        );

        self.event_sink
            .emit(StreamingEvent::WorkflowStarted {
                workflow_id: state.workflow_id.clone(),
                entry_node: graph.entry_node.clone(),
                timestamp: chrono::Utc::now(),
            })
            .await;

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

                self.event_sink
                    .emit(StreamingEvent::WorkflowFailed {
                        workflow_id: state.workflow_id.clone(),
                        error: "Exceeded maximum step count".into(),
                        failed_node: Some(state.current_node.clone()),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
                break;
            }

            let current_node_id = state.current_node.clone();

            // Save checkpoint before executing
            self.save_checkpoint(&state, &current_node_id);

            // Check if we've reached an exit node
            if graph.exit_nodes.contains(&current_node_id) {
                tracing::info!(node = %current_node_id, "Reached exit node");
                state.status = WorkflowStatus::Completed;

                let total_ms = workflow_start.elapsed().as_millis() as u64;
                self.event_sink
                    .emit(StreamingEvent::WorkflowComplete {
                        workflow_id: state.workflow_id.clone(),
                        status: "completed".into(),
                        total_steps: step as u64,
                        total_duration_ms: total_ms,
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
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

            self.event_sink
                .emit(StreamingEvent::NodeStart {
                    workflow_id: state.workflow_id.clone(),
                    node_id: node.id.clone(),
                    node_type: format!("{:?}", node.node_type),
                    revision: state.history.len() as u64,
                    timestamp: chrono::Utc::now(),
                })
                .await;

            let node_start = Instant::now();

            // Execute the node (dispatches based on type/executor)
            let should_continue = self.execute_node(node, &mut state).await?;

            let node_duration = node_start.elapsed().as_millis() as u64;
            self.event_sink
                .emit(StreamingEvent::NodeComplete {
                    workflow_id: state.workflow_id.clone(),
                    node_id: node.id.clone(),
                    success: should_continue,
                    duration_ms: node_duration,
                    revision: state.history.len() as u64,
                    timestamp: chrono::Utc::now(),
                })
                .await;

            if !should_continue {
                // Execution paused (e.g., HumanReview) or failed
                if state.status == WorkflowStatus::WaitingForHuman {
                    self.event_sink
                        .emit(StreamingEvent::HumanInterrupt {
                            workflow_id: state.workflow_id.clone(),
                            node_id: node.id.clone(),
                            reason: "HumanReview node reached".into(),
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
                } else if state.status == WorkflowStatus::Failed {
                    let error_msg = state
                        .data
                        .get(&format!("{}_error", node.id))
                        .and_then(|v| v.as_str())
                        .unwrap_or("node execution failed")
                        .to_string();
                    self.event_sink
                        .emit(StreamingEvent::WorkflowFailed {
                            workflow_id: state.workflow_id.clone(),
                            error: error_msg,
                            failed_node: Some(node.id.clone()),
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
                }
                break;
            }

            // Determine next node based on edges and conditions
            let next_node_id = self.resolve_next_node(graph, &current_node_id, &state);

            match next_node_id {
                Some(ref next_id) => {
                    tracing::info!(
                        from = %current_node_id,
                        to = %next_id,
                        "Transitioning"
                    );

                    self.event_sink
                        .emit(StreamingEvent::EdgeTraversed {
                            workflow_id: state.workflow_id.clone(),
                            from: current_node_id.clone(),
                            to: next_id.clone(),
                            condition: None,
                            timestamp: chrono::Utc::now(),
                        })
                        .await;

                    state.transition(next_id, HashMap::new());
                }
                None => {
                    tracing::warn!(
                        node = %current_node_id,
                        "No outgoing edges — workflow stuck"
                    );
                    state.status = WorkflowStatus::Failed;

                    self.event_sink
                        .emit(StreamingEvent::WorkflowFailed {
                            workflow_id: state.workflow_id.clone(),
                            error: "No outgoing edges from node".into(),
                            failed_node: Some(current_node_id),
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
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
            NodeType::SubWorkflow => self.execute_subworkflow_node(node, state).await,
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

    /// Execute a Process node with retries.
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

    /// Execute a SubWorkflow node using registered subgraphs.
    ///
    /// This enables hierarchical graph composition — a parent graph can
    /// delegate to a child graph with isolated state and explicit
    /// input/output mappings.
    async fn execute_subworkflow_node(
        &self,
        node: &Node,
        state: &mut WorkflowState,
    ) -> KiasResult<bool> {
        // Look up the subgraph name from node config
        let subgraph_name = node
            .config
            .get("subgraph")
            .and_then(|v| v.as_str())
            .unwrap_or(&node.id);

        let subgraph = self.subgraphs.get(subgraph_name).ok_or_else(|| {
            kias_common::error::KiasError::Validation(format!(
                "SubGraph '{}' not registered for node '{}'",
                subgraph_name, node.id
            ))
        })?;

        tracing::info!(
            node = %node.id,
            subgraph = subgraph_name,
            "Executing subgraph"
        );

        // Extract input state from parent
        let child_initial = subgraph.extract_input(state);

        // Create a child engine (no subgraph nesting to prevent infinite recursion)
        let child_engine = WorkflowEngine {
            checkpoint_store: CheckpointStore::new(),
            executor_registry: ExecutorRegistry::new(),
            default_retry_policy: self.default_retry_policy.clone(),
            event_sink: self.event_sink.clone(),
            subgraphs: HashMap::new(),
        };

        // Execute with optional timeout (Box::pin to avoid recursive async)
        let execute_fut = Box::pin(child_engine.execute(&subgraph.graph, child_initial));
        let result = if let Some(timeout) = subgraph.timeout_secs {
            match tokio::time::timeout(Duration::from_secs(timeout), execute_fut).await {
                Ok(r) => r,
                Err(_) => {
                    state.set(format!("{}_error", node.id), "Subgraph timed out");
                    state.status = WorkflowStatus::Failed;
                    return Ok(false);
                }
            }
        } else {
            execute_fut.await
        };

        match result {
            Ok(child_state) => {
                if child_state.status == WorkflowStatus::Completed {
                    // Merge output back into parent state
                    subgraph.merge_output(&child_state, state);

                    tracing::info!(
                        node = %node.id,
                        subgraph = subgraph_name,
                        "Subgraph completed successfully"
                    );
                    Ok(true)
                } else {
                    let error = format!(
                        "Subgraph '{}' failed with status {:?}",
                        subgraph_name, child_state.status
                    );
                    tracing::warn!(node = %node.id, error = %error, "Subgraph failed");

                    state.set(format!("{}_error", node.id), error);

                    if subgraph.propagate_failure {
                        state.status = WorkflowStatus::Failed;
                        Ok(false)
                    } else {
                        // Continue despite subgraph failure
                        state.set(
                            format!("{}_subgraph_status", node.id),
                            format!("{:?}", child_state.status),
                        );
                        Ok(true)
                    }
                }
            }
            Err(e) => {
                let error = format!("Subgraph '{}' execution error: {}", subgraph_name, e);
                tracing::error!(node = %node.id, error = %error);

                state.set(format!("{}_error", node.id), &error);

                if subgraph.propagate_failure {
                    state.status = WorkflowStatus::Failed;
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
        }
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
    pub fn evaluate_condition(&self, expression: &str, state: &WorkflowState) -> bool {
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
            id: uuid::Uuid::new_v4().to_string(),
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
        // Build a simple child graph
        let mut child = WorkflowGraph::new("inner-wf");
        child.add_node(
            Node::new("inner", "Inner", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["inner".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        child.add_node(Node::new("inner_end", "InnerEnd", NodeType::Process));
        child.add_edge(Edge::new("inner", "inner_end"));
        child.set_entry("inner");
        child.add_exit_node("inner_end");

        let subgraph = SubGraph::new(child);

        let mut graph = WorkflowGraph::new("test-subwf");
        graph.add_node(
            Node::new("sub", "Sub", NodeType::SubWorkflow)
                .with_config(serde_json::json!({"subgraph": "inner-wf"})),
        );
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("sub", "end"));
        graph.set_entry("sub");
        graph.add_exit_node("end");

        let state = WorkflowState::new("wf-sub", &graph.entry_node);
        let engine = WorkflowEngine::new().with_subgraph("inner-wf", subgraph);
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

    // ─────────── Streaming event tests ───────────

    #[tokio::test]
    async fn test_streaming_events_emitted() {
        let graph = make_linear_graph();
        let state = WorkflowState::new("wf-stream", &graph.entry_node);
        let sink = EventSink::new();
        let engine = WorkflowEngine::new().with_event_sink(sink.clone());

        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Completed);

        let events = sink.take_events().await;
        // Should have: WorkflowStarted, (NodeStart, NodeComplete) x2, EdgeTraversed x2, WorkflowComplete
        assert!(events.len() >= 3, "Expected at least 3 events, got {}", events.len());

        // First event should be WorkflowStarted
        assert!(matches!(&events[0], StreamingEvent::WorkflowStarted { .. }));

        // Last event should be WorkflowComplete
        let last = events.last().unwrap();
        assert!(matches!(last, StreamingEvent::WorkflowComplete { .. }));
    }

    #[tokio::test]
    async fn test_streaming_events_on_failure() {
        let mut graph = WorkflowGraph::new("test-stream-fail");
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

        let sink = EventSink::new();
        let engine = WorkflowEngine::new().with_event_sink(sink.clone());

        let state = WorkflowState::new("wf-fail-stream", &graph.entry_node);
        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::Failed);

        let events = sink.take_events().await;
        let has_failure = events.iter().any(|e| matches!(e, StreamingEvent::WorkflowFailed { .. }));
        assert!(has_failure, "Expected a WorkflowFailed event");
    }

    #[tokio::test]
    async fn test_streaming_events_human_interrupt() {
        let mut graph = WorkflowGraph::new("test-stream-human");
        graph.add_node(Node::new("review", "Review", NodeType::HumanReview));
        graph.add_node(Node::new("end", "End", NodeType::Process));
        graph.add_edge(Edge::new("review", "end"));
        graph.set_entry("review");
        graph.add_exit_node("end");

        let sink = EventSink::new();
        let engine = WorkflowEngine::new().with_event_sink(sink.clone());

        let state = WorkflowState::new("wf-human-stream", &graph.entry_node);
        let result = engine.execute(&graph, state).await.unwrap();
        assert_eq!(result.status, WorkflowStatus::WaitingForHuman);

        let events = sink.take_events().await;
        let has_human = events.iter().any(|e| matches!(e, StreamingEvent::HumanInterrupt { .. }));
        assert!(has_human, "Expected a HumanInterrupt event");
    }

    // ─────────── Subgraph composition tests ───────────

    #[tokio::test]
    async fn test_subgraph_composition() {
        // Build child graph
        let mut child = WorkflowGraph::new("child");
        child.add_node(
            Node::new("child_start", "ChildStart", NodeType::Process).with_executor(
                ExecutorConfig::Shell {
                    command: "echo".into(),
                    args: vec!["child".into()],
                    env: HashMap::new(),
                    working_dir: None,
                    timeout_secs: None,
                },
            ),
        );
        child.add_node(Node::new("child_end", "ChildEnd", NodeType::Process));
        child.add_edge(Edge::new("child_start", "child_end"));
        child.set_entry("child_start");
        child.add_exit_node("child_end");

        let subgraph = SubGraph::new(child)
            .with_input_mapping("parent_data", "child_input")
            .with_output_mapping("child_output", "parent_result");

        // Build parent graph
        let mut parent = WorkflowGraph::new("parent");
        parent.add_node(
            Node::new("pre", "Pre", NodeType::Process).with_executor(ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["pre".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            }),
        );
        parent.add_node(
            Node::new("sub", "SubGraph", NodeType::SubWorkflow)
                .with_config(serde_json::json!({"subgraph": "child"})),
        );
        parent.add_node(Node::new("post", "Post", NodeType::Process));
        parent.add_edge(Edge::new("pre", "sub"));
        parent.add_edge(Edge::new("sub", "post"));
        parent.set_entry("pre");
        parent.add_exit_node("post");

        let mut state = WorkflowState::new("wf-parent", &parent.entry_node);
        state.set("parent_data", serde_json::json!({"key": "value"}));

        let engine = WorkflowEngine::new().with_subgraph("child", subgraph);
        let result = engine.execute(&parent, state).await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_subgraph_not_registered_fails() {
        let mut parent = WorkflowGraph::new("parent-no-sub");
        parent.add_node(
            Node::new("sub", "Sub", NodeType::SubWorkflow)
                .with_config(serde_json::json!({"subgraph": "missing"})),
        );
        parent.add_node(Node::new("end", "End", NodeType::Process));
        parent.add_edge(Edge::new("sub", "end"));
        parent.set_entry("sub");
        parent.add_exit_node("end");

        let state = WorkflowState::new("wf-no-sub", &parent.entry_node);
        let engine = WorkflowEngine::new();
        let result = engine.execute(&parent, state).await;

        // Should fail with validation error (subgraph not registered)
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("SubGraph 'missing' not registered"));
    }
}
