//! State graph — builder, execution, and node/edge management.

use chrono::Utc;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::checkpoint::{Checkpoint, CheckpointStore, InMemoryCheckpointStore};
use crate::state::GraphState;
use crate::stream::{ExecutionEvent, ExecutionStream};
use crate::validation::{self, GraphTopology, ValidationError};

/// Async node handler: takes state, returns modified state.
///
/// Uses `Arc` internally so handlers can be shared across parallel branches.
pub type NodeHandler = Arc<
    dyn Fn(GraphState) -> Pin<Box<dyn Future<Output = kias_common::KiasResult<GraphState>> + Send>>
        + Send
        + Sync,
>;

/// Edge condition: returns true if this edge should be taken.
pub type EdgeCondition = Box<dyn Fn(&GraphState) -> bool + Send + Sync>;

/// Router function: returns the name of the next node based on state.
pub type RouterFn = Box<dyn Fn(&GraphState) -> String + Send + Sync>;

/// A node in the graph with its handler.
pub struct GraphNode {
    pub name: String,
    pub handler: NodeHandler,
}

/// Edge types in the graph.
pub enum EdgeType {
    /// Unconditional edge: always taken.
    Direct { from: String, to: String },
    /// Conditional edge: taken only if condition returns true.
    Conditional {
        from: String,
        to: String,
        condition: EdgeCondition,
    },
    /// Router edge: the router function returns the name of the next node.
    Router { from: String, router: RouterFn },
    /// Fan-out edge: executes multiple branches in parallel, then merges.
    FanOut {
        from: String,
        targets: Vec<String>,
        join_node: String,
    },
}

/// Builder for constructing a `StateGraph`.
pub struct StateGraphBuilder {
    nodes: HashMap<String, GraphNode>,
    edges: Vec<EdgeType>,
    entry: String,
    checkpoint_store: Option<Arc<dyn CheckpointStore>>,
    stream: Option<Arc<ExecutionStream>>,
    max_steps: usize,
}

impl StateGraphBuilder {
    pub fn new(entry: &str) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            entry: entry.to_string(),
            checkpoint_store: None,
            stream: None,
            max_steps: 1000,
        }
    }

    /// Register a node with an async handler.
    pub fn add_node<F, Fut>(mut self, name: &str, handler: F) -> Self
    where
        F: Fn(GraphState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = kias_common::KiasResult<GraphState>> + Send + 'static,
    {
        let name_owned = name.to_string();
        self.nodes.insert(
            name_owned.clone(),
            GraphNode {
                name: name_owned,
                handler: Arc::new(move |state| Box::pin(handler(state))),
            },
        );
        self
    }

    /// Add an unconditional edge from one node to another.
    pub fn add_edge(mut self, from: &str, to: &str) -> Self {
        self.edges.push(EdgeType::Direct {
            from: from.to_string(),
            to: to.to_string(),
        });
        self
    }

    /// Add a conditional edge: taken only if the condition returns true.
    pub fn add_conditional_edge<F>(mut self, from: &str, to: &str, condition: F) -> Self
    where
        F: Fn(&GraphState) -> bool + Send + Sync + 'static,
    {
        self.edges.push(EdgeType::Conditional {
            from: from.to_string(),
            to: to.to_string(),
            condition: Box::new(condition),
        });
        self
    }

    /// Add a router edge: the router function returns the name of the next node.
    ///
    /// This enables dynamic multi-target branching based on state.
    pub fn add_router<F>(mut self, from: &str, router: F) -> Self
    where
        F: Fn(&GraphState) -> String + Send + Sync + 'static,
    {
        self.edges.push(EdgeType::Router {
            from: from.to_string(),
            router: Box::new(router),
        });
        self
    }

    /// Add a fan-out edge: execute multiple branches in parallel, then join.
    ///
    /// All branch nodes execute concurrently with the same initial state.
    /// When all complete, their state changes are merged (last-write-wins)
    /// and execution continues at `join_node`.
    pub fn add_fan_out(mut self, from: &str, targets: Vec<&str>, join_node: &str) -> Self {
        self.edges.push(EdgeType::FanOut {
            from: from.to_string(),
            targets: targets.into_iter().map(|s| s.to_string()).collect(),
            join_node: join_node.to_string(),
        });
        self
    }

    /// Set a checkpoint store for persistent interrupt/resume.
    pub fn with_checkpoint_store(mut self, store: Arc<dyn CheckpointStore>) -> Self {
        self.checkpoint_store = Some(store);
        self
    }

    /// Set an execution stream for real-time event monitoring.
    pub fn with_stream(mut self, stream: Arc<ExecutionStream>) -> Self {
        self.stream = Some(stream);
        self
    }

    /// Set the maximum number of execution steps (default: 1000).
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Build the state graph. Returns `Err` if validation fails.
    pub fn build(self) -> Result<StateGraph, Vec<ValidationError>> {
        let edge_tuples: Vec<(String, String, bool)> = self
            .edges
            .iter()
            .filter_map(|e| match e {
                EdgeType::Direct { from, to } => Some((from.clone(), to.clone(), false)),
                EdgeType::Conditional { from, to, .. } => Some((from.clone(), to.clone(), true)),
                _ => None,
            })
            .collect();

        // Collect reachability hints from Router and FanOut edges
        let mut reachability_hints = Vec::new();
        let node_names: Vec<String> = self.nodes.keys().cloned().collect();
        for edge in &self.edges {
            match edge {
                EdgeType::Router { from, .. } => {
                    // Router can dynamically reach any node
                    for name in &node_names {
                        reachability_hints.push((from.clone(), name.clone()));
                    }
                }
                EdgeType::FanOut {
                    from,
                    targets,
                    join_node,
                    ..
                } => {
                    for target in targets {
                        reachability_hints.push((from.clone(), target.clone()));
                        reachability_hints.push((target.clone(), join_node.clone()));
                    }
                    reachability_hints.push((from.clone(), join_node.clone()));
                }
                _ => {}
            }
        }

        let topology = GraphTopology {
            entry: self.entry.clone(),
            nodes: self.nodes.keys().cloned().collect(),
            edges: edge_tuples,
            reachability_hints,
        };

        let errors = validation::validate(&topology);
        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(StateGraph {
            nodes: self.nodes,
            edges: self.edges,
            entry: self.entry,
            checkpoint_store: self
                .checkpoint_store
                .unwrap_or_else(|| Arc::new(InMemoryCheckpointStore::new())),
            stream: self.stream,
            max_steps: self.max_steps,
        })
    }

    /// Build the state graph without validation (for testing).
    pub fn build_unchecked(self) -> StateGraph {
        StateGraph {
            nodes: self.nodes,
            edges: self.edges,
            entry: self.entry,
            checkpoint_store: self
                .checkpoint_store
                .unwrap_or_else(|| Arc::new(InMemoryCheckpointStore::new())),
            stream: self.stream,
            max_steps: self.max_steps,
        }
    }
}

/// The compiled state graph ready for execution.
pub struct StateGraph {
    nodes: HashMap<String, GraphNode>,
    edges: Vec<EdgeType>,
    entry: String,
    checkpoint_store: Arc<dyn CheckpointStore>,
    stream: Option<Arc<ExecutionStream>>,
    max_steps: usize,
}

impl StateGraph {
    /// Create a new builder.
    pub fn builder(entry: &str) -> StateGraphBuilder {
        StateGraphBuilder::new(entry)
    }

    /// Get the entry node name.
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Get node names.
    pub fn node_names(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the checkpoint store.
    pub fn checkpoint_store(&self) -> &dyn CheckpointStore {
        self.checkpoint_store.as_ref()
    }

    /// Execute the graph from the given initial state.
    pub async fn execute(&self, initial_state: GraphState) -> kias_common::KiasResult<GraphState> {
        let start_time = Instant::now();
        let mut state = initial_state;
        let mut current = self.entry.clone();
        let mut step: usize = 0;

        self.emit_event(ExecutionEvent::NodeStart {
            node: current.clone(),
            step,
            timestamp_ms: Utc::now().timestamp_millis(),
        })
        .await;

        loop {
            if step >= self.max_steps {
                return Err(kias_common::KiasError::Validation(format!(
                    "Execution exceeded max steps ({})",
                    self.max_steps
                )));
            }

            // Check interruption
            if state.metadata.is_interrupted {
                self.emit_event(ExecutionEvent::Interrupted {
                    node: current.clone(),
                    reason: "State interrupted".to_string(),
                    checkpoint_id: state.metadata.checkpoint_id.clone(),
                })
                .await;
                return Ok(state);
            }

            // Get the node
            let node = self.nodes.get(&current).ok_or_else(|| {
                kias_common::KiasError::Validation(format!("Node '{}' not found", current))
            })?;

            // Execute
            let node_start = Instant::now();
            state.metadata.step = step;
            state.metadata.node_history.push(current.clone());

            let result = (node.handler)(state.clone()).await;
            let node_duration = node_start.elapsed().as_millis() as u64;

            match result {
                Ok(output) => {
                    state = output;
                    self.emit_event(ExecutionEvent::NodeComplete {
                        node: current.clone(),
                        step,
                        duration_ms: node_duration,
                        timestamp_ms: Utc::now().timestamp_millis(),
                    })
                    .await;
                }
                Err(e) => {
                    self.emit_event(ExecutionEvent::NodeError {
                        node: current.clone(),
                        step,
                        error: e.to_string(),
                        timestamp_ms: Utc::now().timestamp_millis(),
                    })
                    .await;
                    self.emit_event(ExecutionEvent::Failed {
                        node: current.clone(),
                        error: e.to_string(),
                    })
                    .await;
                    return Err(e);
                }
            }

            // Save checkpoint
            self.save_checkpoint(&current, &state).await?;

            // Find next
            let next = self.find_next(&current, &state).await?;

            match next {
                Some(NextNode::Single(next_node)) => {
                    self.emit_event(ExecutionEvent::EdgeTaken {
                        from: current.clone(),
                        to: next_node.clone(),
                        is_conditional: self.is_conditional_edge(&current),
                    })
                    .await;
                    current = next_node;
                    step += 1;
                }
                Some(NextNode::FanOut { targets, join_node }) => {
                    state = self
                        .execute_fan_out(&current, &targets, &join_node, state, &mut step)
                        .await?;
                    current = join_node;
                }
                None => {
                    let total_duration = start_time.elapsed().as_millis() as u64;
                    self.emit_event(ExecutionEvent::Completed {
                        total_steps: step + 1,
                        total_duration_ms: total_duration,
                    })
                    .await;
                    return Ok(state);
                }
            }
        }
    }

    /// Resume execution from a checkpoint.
    pub async fn resume_from_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> kias_common::KiasResult<GraphState> {
        let checkpoint = self
            .checkpoint_store
            .load_by_id(checkpoint_id)
            .await?
            .ok_or_else(|| {
                kias_common::KiasError::Validation(format!(
                    "Checkpoint '{}' not found",
                    checkpoint_id
                ))
            })?;

        let mut state = checkpoint.state.clone();
        state.metadata.is_interrupted = false;

        self.emit_event(ExecutionEvent::Resumed {
            checkpoint_id: checkpoint_id.to_string(),
            node: checkpoint.node.clone(),
        })
        .await;

        // Resume from the checkpoint node, not from entry
        self.execute_from(&checkpoint.node, state).await
    }

    /// Execute the graph starting from a specific node (for resume support).
    pub async fn execute_from(
        &self,
        start_node: &str,
        initial_state: GraphState,
    ) -> kias_common::KiasResult<GraphState> {
        let start_time = Instant::now();
        let mut state = initial_state;
        let mut current = start_node.to_string();
        let mut step: usize = state.metadata.step;

        self.emit_event(ExecutionEvent::NodeStart {
            node: current.clone(),
            step,
            timestamp_ms: Utc::now().timestamp_millis(),
        })
        .await;

        loop {
            if step >= self.max_steps {
                return Err(kias_common::KiasError::Validation(format!(
                    "Execution exceeded max steps ({})",
                    self.max_steps
                )));
            }

            if state.metadata.is_interrupted {
                self.emit_event(ExecutionEvent::Interrupted {
                    node: current.clone(),
                    reason: "State interrupted".to_string(),
                    checkpoint_id: state.metadata.checkpoint_id.clone(),
                })
                .await;
                return Ok(state);
            }

            let node = self.nodes.get(&current).ok_or_else(|| {
                kias_common::KiasError::Validation(format!("Node '{}' not found", current))
            })?;

            let node_start = Instant::now();
            state.metadata.step = step;
            state.metadata.node_history.push(current.clone());

            let result = (node.handler)(state.clone()).await;
            let node_duration = node_start.elapsed().as_millis() as u64;

            match result {
                Ok(output) => {
                    state = output;
                    self.emit_event(ExecutionEvent::NodeComplete {
                        node: current.clone(),
                        step,
                        duration_ms: node_duration,
                        timestamp_ms: Utc::now().timestamp_millis(),
                    })
                    .await;
                }
                Err(e) => {
                    self.emit_event(ExecutionEvent::NodeError {
                        node: current.clone(),
                        step,
                        error: e.to_string(),
                        timestamp_ms: Utc::now().timestamp_millis(),
                    })
                    .await;
                    self.emit_event(ExecutionEvent::Failed {
                        node: current.clone(),
                        error: e.to_string(),
                    })
                    .await;
                    return Err(e);
                }
            }

            self.save_checkpoint(&current, &state).await?;

            let next = self.find_next(&current, &state).await?;

            match next {
                Some(NextNode::Single(next_node)) => {
                    self.emit_event(ExecutionEvent::EdgeTaken {
                        from: current.clone(),
                        to: next_node.clone(),
                        is_conditional: self.is_conditional_edge(&current),
                    })
                    .await;
                    current = next_node;
                    step += 1;
                }
                Some(NextNode::FanOut { targets, join_node }) => {
                    state = self
                        .execute_fan_out(&current, &targets, &join_node, state, &mut step)
                        .await?;
                    current = join_node;
                }
                None => {
                    let total_duration = start_time.elapsed().as_millis() as u64;
                    self.emit_event(ExecutionEvent::Completed {
                        total_steps: step + 1,
                        total_duration_ms: total_duration,
                    })
                    .await;
                    return Ok(state);
                }
            }
        }
    }

    /// Resume execution from the latest checkpoint for a given run.
    pub async fn resume_latest(&self, run_id: &str) -> kias_common::KiasResult<GraphState> {
        let checkpoint = self
            .checkpoint_store
            .load_latest(run_id)
            .await?
            .ok_or_else(|| {
                kias_common::KiasError::Validation(format!(
                    "No checkpoint found for run '{}'",
                    run_id
                ))
            })?;

        self.resume_from_checkpoint(&checkpoint.id).await
    }

    // ─── Private helpers ───────────────────────────────────────────────

    async fn find_next(
        &self,
        current: &str,
        state: &GraphState,
    ) -> kias_common::KiasResult<Option<NextNode>> {
        // 1. Conditional edges (evaluated in order, first match wins)
        for edge in &self.edges {
            if let EdgeType::Conditional {
                from,
                to,
                condition,
            } = edge
            {
                if from == current && condition(state) {
                    return Ok(Some(NextNode::Single(to.clone())));
                }
            }
        }

        // 2. Router edges
        for edge in &self.edges {
            if let EdgeType::Router { from, router } = edge {
                if from == current {
                    let target = router(state);
                    return Ok(Some(NextNode::Single(target)));
                }
            }
        }

        // 3. Fan-out edges
        for edge in &self.edges {
            if let EdgeType::FanOut {
                from,
                targets,
                join_node,
            } = edge
            {
                if from == current {
                    return Ok(Some(NextNode::FanOut {
                        targets: targets.clone(),
                        join_node: join_node.clone(),
                    }));
                }
            }
        }

        // 4. Direct edges
        for edge in &self.edges {
            if let EdgeType::Direct { from, to } = edge {
                if from == current {
                    return Ok(Some(NextNode::Single(to.clone())));
                }
            }
        }

        Ok(None)
    }

    fn is_conditional_edge(&self, from: &str) -> bool {
        self.edges.iter().any(|e| match e {
            EdgeType::Conditional {
                from: f,
                condition: _,
                ..
            } => f == from,
            _ => false,
        })
    }

    /// Execute fan-out branches concurrently, merge results.
    async fn execute_fan_out(
        &self,
        source: &str,
        targets: &[String],
        _join_node: &str,
        state: GraphState,
        step: &mut usize,
    ) -> kias_common::KiasResult<GraphState> {
        self.emit_event(ExecutionEvent::BranchStart {
            source: source.to_string(),
            branches: targets.to_vec(),
        })
        .await;

        let mut handles = Vec::new();

        for target in targets.iter().cloned() {
            let node = self.nodes.get(&target).ok_or_else(|| {
                kias_common::KiasError::Validation(format!("Fan-out target '{}' not found", target))
            })?;
            let handler = node.handler.clone();
            let branch_state = state.clone();

            handles.push(tokio::spawn(async move {
                let result = (handler)(branch_state).await;
                (target, result)
            }));
        }

        // Await all branches
        let mut merged = state.clone();
        let mut any_error: Option<(String, kias_common::KiasError)> = None;

        for handle in handles {
            let (branch_name, result) = handle
                .await
                .map_err(|e| kias_common::KiasError::Storage(format!("Join error: {}", e)))?;

            match result {
                Ok(branch_state) => {
                    merged.merge(branch_state);
                    *step += 1;
                }
                Err(e) => {
                    if any_error.is_none() {
                        any_error = Some((branch_name, e));
                    }
                }
            }
        }

        if let Some((branch, err)) = any_error {
            return Err(kias_common::KiasError::Validation(format!(
                "Fan-out branch '{}' failed: {}",
                branch, err
            )));
        }

        self.emit_event(ExecutionEvent::BranchComplete {
            source: source.to_string(),
            branches: targets.to_vec(),
        })
        .await;

        Ok(merged)
    }

    async fn save_checkpoint(&self, node: &str, state: &GraphState) -> kias_common::KiasResult<()> {
        let checkpoint = Checkpoint {
            id: Uuid::new_v4().to_string(),
            run_id: state.metadata.run_id.clone(),
            node: node.to_string(),
            state: state.clone(),
            timestamp: Utc::now(),
            version: state.metadata.step as u64,
        };

        let checkpoint_id = self.checkpoint_store.save(checkpoint).await?;

        self.emit_event(ExecutionEvent::CheckpointSaved {
            checkpoint_id,
            node: node.to_string(),
        })
        .await;

        Ok(())
    }

    async fn emit_event(&self, event: ExecutionEvent) {
        if let Some(ref stream) = self.stream {
            stream.emit(event);
        }
    }
}

/// Internal enum for the type of next node(s).
enum NextNode {
    Single(String),
    FanOut {
        targets: Vec<String>,
        join_node: String,
    },
}
