//! Graph validation — structural integrity checks before execution.

use std::collections::{HashMap, HashSet, VecDeque};

/// Validation error with details.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub kind: ValidationErrorKind,
    pub message: String,
}

/// Categories of validation errors.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationErrorKind {
    /// Entry node not found in graph.
    MissingEntryNode,
    /// Node referenced by an edge doesn't exist.
    DanglingEdge,
    /// No outgoing edges from a non-terminal node.
    DeadEnd,
    /// Graph has a cycle with no exit condition.
    UnboundedCycle,
    /// No edges at all — graph is trivial.
    NoEdges,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

/// Graph topology information extracted for validation.
pub struct GraphTopology {
    pub entry: String,
    pub nodes: HashSet<String>,
    pub edges: Vec<(String, String, bool)>, // (from, to, is_conditional)
    /// Extra reachability hints: pairs of (from, to) from routers and fan-outs.
    /// These are used for reachability checking only.
    pub reachability_hints: Vec<(String, String)>,
}

/// Validate a graph topology.
///
/// Returns an empty vector if valid, otherwise returns all validation errors found.
pub fn validate(topology: &GraphTopology) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // 1. Entry node must exist
    if !topology.nodes.contains(&topology.entry) {
        errors.push(ValidationError {
            kind: ValidationErrorKind::MissingEntryNode,
            message: format!("Entry node '{}' not found in graph", topology.entry),
        });
    }

    // 2. All edge endpoints must exist
    for (from, to, _) in &topology.edges {
        if !topology.nodes.contains(from) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::DanglingEdge,
                message: format!("Edge source '{}' not found", from),
            });
        }
        if !topology.nodes.contains(to) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::DanglingEdge,
                message: format!("Edge target '{}' not found", to),
            });
        }
    }

    if topology.edges.is_empty() && topology.reachability_hints.is_empty() && topology.nodes.len() > 1 {
        errors.push(ValidationError {
            kind: ValidationErrorKind::NoEdges,
            message: "Graph has multiple nodes but no edges".to_string(),
        });
    }

    // 3. Check for unreachable nodes (BFS from entry)
    // Include reachability hints from routers and fan-outs
    let mut all_edges = topology.edges.clone();
    for (from, to) in &topology.reachability_hints {
        all_edges.push((from.clone(), to.clone(), false));
    }
    let reachable = reachable_from(&topology.entry, &all_edges);
    for node in &topology.nodes {
        if node != &topology.entry && !reachable.contains(node.as_str()) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::DeadEnd,
                message: format!("Node '{}' is unreachable from entry", node),
            });
        }
    }

    // 4. Check for nodes with no outgoing edges that aren't leaf nodes
    // (A node with conditional edges should have at least one non-conditional fallback
    //  or all conditional edges must cover all cases — we check the simpler case)
    let outgoing: HashMap<&str, usize> = {
        let mut map: HashMap<&str, usize> = HashMap::new();
        for (from, _, _) in &topology.edges {
            *map.entry(from.as_str()).or_insert(0) += 1;
        }
        // Also count reachability_hints as outgoing edges
        for (from, _) in &topology.reachability_hints {
            *map.entry(from.as_str()).or_insert(0) += 1;
        }
        map
    };

    for node in &topology.nodes {
        let count = outgoing.get(node.as_str()).copied().unwrap_or(0);
        if count == 0 && topology.nodes.len() > 1 {
            // This is fine for terminal nodes, but let's warn if it's not the entry
            // (entry with no outgoing edges = no execution)
            if node == &topology.entry {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::DeadEnd,
                    message: format!("Entry node '{}' has no outgoing edges", node),
                });
            }
        }
    }

    errors
}

/// Compute the set of nodes reachable from a starting node via BFS.
fn reachable_from<'a>(start: &'a str, edges: &'a [(String, String, bool)]) -> HashSet<&'a str> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        for (from, to, _) in edges {
            if from == current && !visited.contains(to.as_str()) {
                visited.insert(to.as_str());
                queue.push_back(to.as_str());
            }
        }
    }

    visited
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_linear_graph() {
        let topo = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into(), "b".into(), "c".into()].into_iter().collect(),
            edges: vec![
                ("a".into(), "b".into(), false),
                ("b".into(), "c".into(), false),
            ],
            reachability_hints: vec![],
        };
        let errors = validate(&topo);
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_missing_entry() {
        let topo = GraphTopology {
            entry: "missing".to_string(),
            nodes: vec!["a".into()].into_iter().collect(),
            edges: vec![],
            reachability_hints: vec![],
        };
        let errors = validate(&topo);
        assert!(errors.iter().any(|e| e.kind == ValidationErrorKind::MissingEntryNode));
    }

    #[test]
    fn test_dangling_edge() {
        let topo = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into()].into_iter().collect(),
            edges: vec![("a".into(), "b".into(), false)],
            reachability_hints: vec![],
        };
        let errors = validate(&topo);
        assert!(errors.iter().any(|e| e.kind == ValidationErrorKind::DanglingEdge));
    }

    #[test]
    fn test_unreachable_node() {
        let topo = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into(), "b".into(), "c".into()].into_iter().collect(),
            edges: vec![("a".into(), "b".into(), false)],
            reachability_hints: vec![],
            // c is unreachable
        };
        let errors = validate(&topo);
        assert!(errors.iter().any(|e| e.kind == ValidationErrorKind::DeadEnd
            && e.message.contains("unreachable")));
    }

    #[test]
    fn test_entry_no_outgoing() {
        let _topo = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into()].into_iter().collect(),
            edges: vec![],
            reachability_hints: vec![],
        };
        // Single-node graph is fine (entry is also terminal)
        // But with multiple nodes and no edges:
        let topo2 = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into(), "b".into()].into_iter().collect(),
            edges: vec![],
            reachability_hints: vec![],
        };
        let errors = validate(&topo2);
        assert!(errors.iter().any(|e| e.kind == ValidationErrorKind::NoEdges));
    }

    #[test]
    fn test_cycle_with_exit() {
        let topo = GraphTopology {
            entry: "a".to_string(),
            nodes: vec!["a".into(), "loop".into(), "end".into()]
                .into_iter()
                .collect(),
            edges: vec![
                ("a".into(), "loop".into(), false),
                ("loop".into(), "loop".into(), true), // conditional self-loop
                ("loop".into(), "end".into(), true),  // conditional exit
            ],
            reachability_hints: vec![],
        };
        let errors = validate(&topo);
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
    }
}
