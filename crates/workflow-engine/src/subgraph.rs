//! # SubGraph Composition — 嵌套工作流图
//!
//! This module implements subgraph composition, allowing one `WorkflowGraph`
//! to be embedded as a node inside another. This mirrors LangGraph's ability
//! to compose graphs hierarchically.
//!
//! ## Design
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │  Parent Graph                        │
//! │                                      │
//! │  [A] ──→ [SubGraph: analyze] ──→ [C] │
//! │              │                        │
//! │         ┌────┴────┐                   │
//! │         │ analyze  │                   │
//! │  entry →│  graph   │→ exit            │
//! │         │ [X]→[Y]  │                   │
//! │         └──────────┘                   │
//! └─────────────────────────────────────┘
//! ```
//!
//! ## State Mappings
//!
//! SubGraphs support state mapping between parent and child:
//! - `input_mapping`: extract fields from parent state → child initial state
//! - `output_mapping`: merge child final state → parent state

use super::graph::WorkflowGraph;
use super::state::WorkflowState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A wrapper that makes a `WorkflowGraph` composable as a sub-workflow.
///
/// The subgraph has its own isolated state scope but communicates with
/// the parent through explicit input/output mappings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraph {
    /// The inner workflow graph to execute.
    pub graph: WorkflowGraph,

    /// How to map parent state fields → child state fields before execution.
    ///
    /// Key = parent field name, Value = child field name.
    /// If empty, child starts with empty state.
    pub input_mapping: HashMap<String, String>,

    /// How to map child state fields → parent state fields after execution.
    ///
    /// Key = child field name, Value = parent field name.
    /// If empty, no output is mapped back.
    pub output_mapping: HashMap<String, String>,

    /// If true, subgraph failure causes parent to fail.
    /// If false, parent can continue (error is stored in state).
    pub propagate_failure: bool,

    /// Optional timeout for the entire subgraph execution (seconds).
    pub timeout_secs: Option<u64>,
}

impl SubGraph {
    /// Create a subgraph from an existing workflow graph.
    pub fn new(graph: WorkflowGraph) -> Self {
        Self {
            graph,
            input_mapping: HashMap::new(),
            output_mapping: HashMap::new(),
            propagate_failure: true,
            timeout_secs: None,
        }
    }

    /// Add an input mapping: `parent_field` → `child_field`.
    pub fn with_input_mapping(
        mut self,
        parent_field: impl Into<String>,
        child_field: impl Into<String>,
    ) -> Self {
        self.input_mapping
            .insert(parent_field.into(), child_field.into());
        self
    }

    /// Add an output mapping: `child_field` → `parent_field`.
    pub fn with_output_mapping(
        mut self,
        child_field: impl Into<String>,
        parent_field: impl Into<String>,
    ) -> Self {
        self.output_mapping
            .insert(child_field.into(), parent_field.into());
        self
    }

    /// Whether subgraph failure propagates to the parent.
    pub fn with_propagate_failure(mut self, propagate: bool) -> Self {
        self.propagate_failure = propagate;
        self
    }

    /// Set a timeout for subgraph execution.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Extract initial child state from parent state using input_mapping.
    ///
    /// For each (parent_key, child_key) in input_mapping, copies
    /// `parent_state[parent_key]` → `child_state[child_key]`.
    pub fn extract_input(&self, parent_state: &WorkflowState) -> WorkflowState {
        let mut child_state = WorkflowState::new(
            &format!("{}:sub", parent_state.workflow_id),
            &self.graph.entry_node,
        );

        for (parent_key, child_key) in &self.input_mapping {
            if let Some(value) = parent_state.get(parent_key) {
                child_state.set(child_key.clone(), value.clone());
            }
        }

        child_state
    }

    /// Merge child output back into parent state using output_mapping.
    ///
    /// For each (child_key, parent_key) in output_mapping, copies
    /// `child_state[child_key]` → `parent_state[parent_key]`.
    pub fn merge_output(&self, child_state: &WorkflowState, parent_state: &mut WorkflowState) {
        for (child_key, parent_key) in &self.output_mapping {
            if let Some(value) = child_state.get(child_key) {
                parent_state.set(parent_key.clone(), value.clone());
            }
        }
    }
}

/// Result of subgraph execution.
#[derive(Debug, Clone)]
pub struct SubGraphResult {
    /// The final state of the child workflow.
    pub child_state: WorkflowState,
    /// Whether the subgraph completed successfully.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Total execution time in milliseconds.
    pub duration_ms: u64,
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::Edge;
    use crate::node::{ExecutorConfig, Node, NodeType};

    fn make_simple_graph() -> WorkflowGraph {
        let mut graph = WorkflowGraph::new("analyze");
        graph.add_node(Node::new("x", "Step X", NodeType::Process).with_executor(
            ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["x".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            },
        ));
        graph.add_node(Node::new("y", "Step Y", NodeType::Process).with_executor(
            ExecutorConfig::Shell {
                command: "echo".into(),
                args: vec!["y".into()],
                env: HashMap::new(),
                working_dir: None,
                timeout_secs: None,
            },
        ));
        graph.add_edge(Edge::new("x", "y"));
        graph.set_entry("x");
        graph.add_exit_node("y");
        graph
    }

    #[test]
    fn test_subgraph_creation() {
        let graph = make_simple_graph();
        let sg = SubGraph::new(graph);
        assert!(sg.input_mapping.is_empty());
        assert!(sg.output_mapping.is_empty());
        assert!(sg.propagate_failure);
        assert!(sg.timeout_secs.is_none());
    }

    #[test]
    fn test_subgraph_with_mappings() {
        let graph = make_simple_graph();
        let sg = SubGraph::new(graph)
            .with_input_mapping("parent_input", "child_input")
            .with_output_mapping("child_result", "parent_result")
            .with_propagate_failure(false)
            .with_timeout(30);

        assert_eq!(sg.input_mapping.get("parent_input").unwrap(), "child_input");
        assert_eq!(
            sg.output_mapping.get("child_result").unwrap(),
            "parent_result"
        );
        assert!(!sg.propagate_failure);
        assert_eq!(sg.timeout_secs, Some(30));
    }

    #[test]
    fn test_extract_input() {
        let graph = make_simple_graph();
        let sg = SubGraph::new(graph)
            .with_input_mapping("data", "input_data")
            .with_input_mapping("config", "settings");

        let mut parent_state = WorkflowState::new("wf-parent", "a");
        parent_state.set("data", serde_json::json!({"text": "hello"}));
        parent_state.set("config", serde_json::json!({"verbose": true}));
        parent_state.set("unmapped", "should not appear");

        let child_state = sg.extract_input(&parent_state);

        // Mapped fields should exist
        assert_eq!(
            child_state.get("input_data").unwrap(),
            &serde_json::json!({"text": "hello"})
        );
        assert_eq!(
            child_state.get("settings").unwrap(),
            &serde_json::json!({"verbose": true})
        );

        // Unmapped field should not be in child
        assert!(child_state.get("unmapped").is_none());
    }

    #[test]
    fn test_merge_output() {
        let graph = make_simple_graph();
        let sg = SubGraph::new(graph)
            .with_output_mapping("result", "analysis")
            .with_output_mapping("score", "confidence");

        let mut child_state = WorkflowState::new("wf:sub", "x");
        child_state.set("result", serde_json::json!({"sentiment": "positive"}));
        child_state.set("score", 0.95);
        child_state.set("internal_debug", "should not propagate");

        let mut parent_state = WorkflowState::new("wf-parent", "a");

        sg.merge_output(&child_state, &mut parent_state);

        assert_eq!(
            parent_state.get("analysis").unwrap(),
            &serde_json::json!({"sentiment": "positive"})
        );
        assert_eq!(
            parent_state.get("confidence").unwrap(),
            &serde_json::json!(0.95)
        );
        // Unmapped field should not appear
        assert!(parent_state.get("internal_debug").is_none());
    }

    #[test]
    fn test_extract_input_missing_parent_field() {
        let graph = make_simple_graph();
        let sg = SubGraph::new(graph).with_input_mapping("nonexistent", "child_field");

        let parent_state = WorkflowState::new("wf-parent", "a");
        let child_state = sg.extract_input(&parent_state);

        // Missing parent field should not create child field
        assert!(child_state.get("child_field").is_none());
    }
}
