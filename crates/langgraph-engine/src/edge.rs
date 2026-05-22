//! Enhanced edge types for GraphFlow, including evaluator-backed conditional edges,
//! DAG topology analysis, and parallel/serial execution scheduling.
//!
//! This module extends the basic edge model with:
//! - `EvaluableEdge`: conditional edges backed by `Box<dyn ConditionEvaluator>`
//! - `DagTopology`: DAG analysis for topological ordering and parallel stage detection
//! - `ExecutionSchedule`: serial and parallel execution stages derived from a DAG

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use crate::condition::ConditionEvaluator;
use crate::state::GraphState;

// ─── Evaluable conditional edge ──────────────────────────────────────

/// An edge that uses a `ConditionEvaluator` to decide routing.
///
/// Unlike the closure-based `EdgeCondition` in the core graph, this edge
/// carries a named, inspectable, composable evaluator.
pub struct EvaluableEdge {
    pub from: String,
    pub to: String,
    pub evaluator: Box<dyn ConditionEvaluator>,
}

impl EvaluableEdge {
    pub fn new(from: &str, to: &str, evaluator: Box<dyn ConditionEvaluator>) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            evaluator,
        }
    }

    /// Evaluate the condition against the given state.
    pub fn should_take(&self, state: &GraphState) -> bool {
        self.evaluator.evaluate(state)
    }

    /// Human-readable description.
    pub fn description(&self) -> String {
        format!("{} → [{}] → {}", self.from, self.evaluator.name(), self.to)
    }
}

impl fmt::Debug for EvaluableEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EvaluableEdge({})", self.description())
    }
}

// ─── DAG Topology ────────────────────────────────────────────────────

/// Represents a directed acyclic graph (DAG) for topological analysis.
///
/// Used to compute execution schedules: topological ordering for serial
/// execution, and level-based parallel stages for concurrent execution.
#[derive(Debug, Clone)]
pub struct DagTopology {
    /// Node names.
    pub nodes: HashSet<String>,
    /// Adjacency list: from → set of to.
    pub adjacency: HashMap<String, HashSet<String>>,
    /// Reverse adjacency: to → set of from (in-edges).
    pub reverse_adj: HashMap<String, HashSet<String>>,
}

impl DagTopology {
    /// Build a DAG from a list of (from, to) edge pairs.
    pub fn new<I>(nodes: I, edges: &[(String, String)]) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let node_set: HashSet<String> = nodes.into_iter().collect();
        let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
        let mut reverse_adj: HashMap<String, HashSet<String>> = HashMap::new();

        for node in &node_set {
            adjacency.entry(node.clone()).or_default();
            reverse_adj.entry(node.clone()).or_default();
        }

        for (from, to) in edges {
            if node_set.contains(from) && node_set.contains(to) {
                adjacency
                    .entry(from.clone())
                    .or_default()
                    .insert(to.clone());
                reverse_adj
                    .entry(to.clone())
                    .or_default()
                    .insert(from.clone());
            }
        }

        Self {
            nodes: node_set,
            adjacency,
            reverse_adj,
        }
    }

    /// Detect whether the graph contains a cycle using Kahn's algorithm.
    pub fn has_cycle(&self) -> bool {
        let sorted = self.kahn_topo_sort();
        sorted.len() < self.nodes.len()
    }

    /// Compute a topological ordering using Kahn's algorithm.
    /// Returns an empty vec if the graph contains a cycle.
    pub fn kahn_topo_sort(&self) -> Vec<String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node.clone(), 0);
        }
        for targets in self.adjacency.values() {
            for target in targets {
                *in_degree.entry(target.clone()).or_insert(0) += 1;
            }
        }

        let queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(n, _)| n.clone())
            .collect();

        // Sort for deterministic ordering
        let mut sorted_queue: Vec<String> = queue.into();
        sorted_queue.sort();

        let mut result = Vec::new();

        while let Some(node) = sorted_queue.pop() {
            result.push(node.clone());
            if let Some(targets) = self.adjacency.get(&node) {
                let mut sorted_targets: Vec<String> = targets.iter().cloned().collect();
                sorted_targets.sort();
                for target in sorted_targets {
                    let deg = in_degree
                        .get_mut(&target)
                        .expect("internal invariant violated: target not found in in_degree map");
                    *deg -= 1;
                    if *deg == 0 {
                        sorted_queue.push(target);
                    }
                }
            }
        }

        result
    }

    /// Compute parallel execution stages (levels).
    ///
    /// Nodes in the same stage have no dependencies on each other
    /// and can be executed concurrently. Stage N must complete
    /// before stage N+1 begins.
    ///
    /// Returns `None` if the graph contains a cycle.
    pub fn parallel_stages(&self) -> Option<Vec<Vec<String>>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node.clone(), 0);
        }
        for targets in self.adjacency.values() {
            for target in targets {
                *in_degree.entry(target.clone()).or_insert(0) += 1;
            }
        }

        let mut current_level: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(n, _)| n.clone())
            .collect();
        current_level.sort();

        let mut stages = Vec::new();
        let mut processed = 0;

        while !current_level.is_empty() {
            processed += current_level.len();

            let mut next_level = Vec::new();
            for node in &current_level {
                if let Some(targets) = self.adjacency.get(node) {
                    let mut sorted_targets: Vec<String> = targets.iter().cloned().collect();
                    sorted_targets.sort();
                    for target in sorted_targets {
                        let deg = in_degree.get_mut(&target).expect(
                            "internal invariant violated: target not found in in_degree map",
                        );
                        *deg -= 1;
                        if *deg == 0 {
                            next_level.push(target);
                        }
                    }
                }
            }

            stages.push(current_level);
            next_level.sort();
            current_level = next_level;
        }

        if processed < self.nodes.len() {
            None // cycle detected
        } else {
            Some(stages)
        }
    }

    /// Get nodes with no incoming edges (roots).
    pub fn roots(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| {
                self.reverse_adj
                    .get(*n)
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    /// Get nodes with no outgoing edges (leaves).
    pub fn leaves(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| self.adjacency.get(*n).map(|s| s.is_empty()).unwrap_or(true))
            .cloned()
            .collect()
    }
}

// ─── Execution schedule ──────────────────────────────────────────────

/// Describes how to execute a DAG — either serial or parallel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionSchedule {
    /// Execute nodes one at a time in topological order.
    Serial(Vec<String>),
    /// Execute in parallel stages. Each stage is a group of nodes
    /// that can run concurrently. Stages execute sequentially.
    Parallel(Vec<Vec<String>>),
}

impl ExecutionSchedule {
    /// Derive a schedule from a DAG.
    ///
    /// Returns `Parallel` with stages if the DAG supports it,
    /// or `Serial` with topological order as fallback.
    pub fn from_dag(dag: &DagTopology) -> Option<Self> {
        match dag.parallel_stages() {
            Some(stages) => {
                if stages.iter().any(|s| s.len() > 1) {
                    Some(ExecutionSchedule::Parallel(stages))
                } else {
                    let flat: Vec<String> = stages.into_iter().flatten().collect();
                    Some(ExecutionSchedule::Serial(flat))
                }
            }
            None => None, // cycle
        }
    }

    /// Get total number of execution steps.
    pub fn total_steps(&self) -> usize {
        match self {
            ExecutionSchedule::Serial(nodes) => nodes.len(),
            ExecutionSchedule::Parallel(stages) => stages.len(),
        }
    }

    /// Whether this schedule involves any parallelism.
    pub fn is_parallel(&self) -> bool {
        matches!(self, ExecutionSchedule::Parallel(stages) if stages.iter().any(|s| s.len() > 1))
    }
}

impl fmt::Display for ExecutionSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionSchedule::Serial(nodes) => {
                write!(f, "Serial: {}", nodes.join(" → "))
            }
            ExecutionSchedule::Parallel(stages) => {
                let stage_strs: Vec<String> = stages
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("Stage {}: [{}]", i, s.join(", ")))
                    .collect();
                write!(f, "Parallel: {}", stage_strs.join(" | "))
            }
        }
    }
}

// ─── Multi-agent execution plan ──────────────────────────────────────

/// A plan for executing multiple agents in a DAG.
///
/// Supports both serial and parallel agent execution modes.
#[derive(Debug, Clone)]
pub struct AgentExecutionPlan {
    pub schedule: ExecutionSchedule,
    pub agents: HashMap<String, String>, // node_name → agent_id
}

impl AgentExecutionPlan {
    /// Create a new execution plan from a DAG and agent mapping.
    pub fn new(dag: &DagTopology, agents: HashMap<String, String>) -> Option<Self> {
        ExecutionSchedule::from_dag(dag).map(|schedule| Self { schedule, agents })
    }

    /// Get agents for a specific parallel stage.
    pub fn stage_agents(&self, stage_index: usize) -> Option<Vec<&str>> {
        match &self.schedule {
            ExecutionSchedule::Parallel(stages) => stages.get(stage_index).map(|nodes| {
                nodes
                    .iter()
                    .filter_map(|n| self.agents.get(n).map(|a| a.as_str()))
                    .collect()
            }),
            ExecutionSchedule::Serial(_) => {
                if stage_index == 0 {
                    Some(self.agents.values().map(|a| a.as_str()).collect())
                } else {
                    None
                }
            }
        }
    }

    /// Number of execution stages.
    pub fn stage_count(&self) -> usize {
        self.schedule.total_steps()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::condition::{Always, CompareOp, NumericCompare};

    fn make_dag_topology() -> DagTopology {
        // a → b → d
        // a → c → d
        DagTopology::new(
            vec!["a", "b", "c", "d"].into_iter().map(String::from),
            &[
                ("a".into(), "b".into()),
                ("a".into(), "c".into()),
                ("b".into(), "d".into()),
                ("c".into(), "d".into()),
            ],
        )
    }

    #[test]
    fn test_dag_topo_sort() {
        let dag = make_dag_topology();
        let order = dag.kahn_topo_sort();
        assert_eq!(order.len(), 4);
        assert_eq!(order[0], "a"); // root first
        assert_eq!(order[3], "d"); // leaf last
                                   // b and c must appear before d
        let b_pos = order.iter().position(|x| x == "b").unwrap();
        let c_pos = order.iter().position(|x| x == "c").unwrap();
        let d_pos = order.iter().position(|x| x == "d").unwrap();
        assert!(b_pos < d_pos);
        assert!(c_pos < d_pos);
    }

    #[test]
    fn test_dag_cycle_detection() {
        let dag = DagTopology::new(
            vec!["a", "b", "c"].into_iter().map(String::from),
            &[
                ("a".into(), "b".into()),
                ("b".into(), "c".into()),
                ("c".into(), "a".into()), // cycle!
            ],
        );
        assert!(dag.has_cycle());
        assert!(dag.parallel_stages().is_none());
    }

    #[test]
    fn test_parallel_stages() {
        let dag = make_dag_topology();
        let stages = dag.parallel_stages().unwrap();
        // Stage 0: [a], Stage 1: [b, c], Stage 2: [d]
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0], vec!["a"]);
        assert_eq!(stages[1].len(), 2);
        assert!(stages[1].contains(&"b".to_string()));
        assert!(stages[1].contains(&"c".to_string()));
        assert_eq!(stages[2], vec!["d"]);
    }

    #[test]
    fn test_execution_schedule_parallel() {
        let dag = make_dag_topology();
        let schedule = ExecutionSchedule::from_dag(&dag).unwrap();
        assert!(schedule.is_parallel());
        assert_eq!(schedule.total_steps(), 3);
    }

    #[test]
    fn test_execution_schedule_serial_chain() {
        // Linear chain: a → b → c → d
        let dag = DagTopology::new(
            vec!["a", "b", "c", "d"].into_iter().map(String::from),
            &[
                ("a".into(), "b".into()),
                ("b".into(), "c".into()),
                ("c".into(), "d".into()),
            ],
        );
        let schedule = ExecutionSchedule::from_dag(&dag).unwrap();
        assert!(!schedule.is_parallel());
        assert!(matches!(schedule, ExecutionSchedule::Serial(_)));
    }

    #[test]
    fn test_roots_and_leaves() {
        let dag = make_dag_topology();
        let mut roots = dag.roots();
        roots.sort();
        assert_eq!(roots, vec!["a"]);

        let mut leaves = dag.leaves();
        leaves.sort();
        assert_eq!(leaves, vec!["d"]);
    }

    #[test]
    fn test_evaluable_edge_basic() {
        let edge = EvaluableEdge::new("start", "end", Always.into_boxed());
        assert!(edge.should_take(&GraphState::new()));
        assert!(edge.description().contains("Always"));
    }

    #[test]
    fn test_evaluable_edge_with_evaluator() {
        let edge = EvaluableEdge::new(
            "check",
            "proceed",
            NumericCompare::new("score", CompareOp::Ge, 90.0).into_boxed(),
        );
        let mut state = GraphState::new();
        state.set("score", 95i32);
        assert!(edge.should_take(&state));

        let mut state2 = GraphState::new();
        state2.set("score", 50i32);
        assert!(!edge.should_take(&state2));
    }

    #[test]
    fn test_agent_execution_plan() {
        let dag = make_dag_topology();
        let mut agents = HashMap::new();
        agents.insert("a".to_string(), "orchestrator".to_string());
        agents.insert("b".to_string(), "researcher".to_string());
        agents.insert("c".to_string(), "writer".to_string());
        agents.insert("d".to_string(), "reviewer".to_string());

        let plan = AgentExecutionPlan::new(&dag, agents).unwrap();
        assert_eq!(plan.stage_count(), 3);
        assert!(plan.stage_agents(10).is_none()); // out of range
    }

    #[test]
    fn test_single_node_dag() {
        let dag = DagTopology::new(vec!["only"].into_iter().map(String::from), &[]);
        let order = dag.kahn_topo_sort();
        assert_eq!(order, vec!["only"]);
        assert!(!dag.has_cycle());
        let stages = dag.parallel_stages().unwrap();
        assert_eq!(stages, vec![vec!["only"]]);
    }
}
