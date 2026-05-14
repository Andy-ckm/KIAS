use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: String,
    pub content: String,
    pub node_type: NodeType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Document,
    Concept,
    Entity,
    Relation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub relationship: String,
    pub weight: f64,
}

pub struct KnowledgeGraph {
    nodes: HashMap<String, KnowledgeNode>,
    edges: Vec<Edge>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: KnowledgeNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn get_node(&self, id: &str) -> Option<&KnowledgeNode> {
        self.nodes.get(id)
    }

    pub fn get_neighbors(&self, node_id: &str) -> Vec<&KnowledgeNode> {
        self.edges
            .iter()
            .filter(|e| e.from == node_id)
            .filter_map(|e| self.nodes.get(&e.to))
            .collect()
    }

    /// Get all nodes in the graph
    pub fn get_all_nodes(&self) -> Vec<&KnowledgeNode> {
        self.nodes.values().collect()
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Search nodes by content (case-insensitive substring match)
    pub fn search_by_content(&self, query: &str) -> Vec<&KnowledgeNode> {
        let query_lower = query.to_lowercase();
        self.nodes
            .values()
            .filter(|n| n.content.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Search nodes by node type
    pub fn search_by_type(&self, node_type: &NodeType) -> Vec<&KnowledgeNode> {
        self.nodes
            .values()
            .filter(|n| &n.node_type == node_type)
            .collect()
    }

    /// Get edges from a specific node
    pub fn get_outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.from == node_id).collect()
    }

    /// Get edges to a specific node
    pub fn get_incoming_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.to == node_id).collect()
    }

    /// Remove a node and all its associated edges
    pub fn remove_node(&mut self, id: &str) -> Option<KnowledgeNode> {
        self.edges.retain(|e| e.from != id && e.to != id);
        self.nodes.remove(id)
    }

    /// Get weighted shortest path between two nodes (BFS with weight)
    pub fn shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        use std::collections::VecDeque;

        if from == to {
            return Some(vec![from.to_string()]);
        }

        let mut visited = HashMap::new();
        let mut queue = VecDeque::new();
        queue.push_back((from.to_string(), vec![from.to_string()]));
        visited.insert(from.to_string(), 0.0f64);

        while let Some((current, path)) = queue.pop_front() {
            for edge in self.edges.iter().filter(|e| e.from == current) {
                if edge.to == to {
                    let mut result = path.clone();
                    result.push(to.to_string());
                    return Some(result);
                }
                let new_weight = visited[&current] + edge.weight;
                if !visited.contains_key(&edge.to) || new_weight < visited[&edge.to] {
                    visited.insert(edge.to.clone(), new_weight);
                    let mut new_path = path.clone();
                    new_path.push(edge.to.clone());
                    queue.push_back((edge.to.clone(), new_path));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, content: &str) -> KnowledgeNode {
        KnowledgeNode {
            id: id.to_string(),
            content: content.to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::new(),
        }
    }

    fn make_typed_node(id: &str, content: &str, node_type: NodeType) -> KnowledgeNode {
        KnowledgeNode {
            id: id.to_string(),
            content: content.to_string(),
            node_type,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_graph_creation() {
        let graph = KnowledgeGraph::new();
        assert!(graph.get_node("any").is_none());
        assert_eq!(graph.node_count(), 0);
    }

    #[test]
    fn test_add_and_get_node() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "hello"));
        let node = graph.get_node("n1").unwrap();
        assert_eq!(node.content, "hello");
    }

    #[test]
    fn test_add_edge_and_neighbors() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "A"));
        graph.add_node(make_node("n2", "B"));
        graph.add_node(make_node("n3", "C"));

        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n2".to_string(),
            relationship: "related_to".to_string(),
            weight: 0.8,
        });
        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n3".to_string(),
            relationship: "related_to".to_string(),
            weight: 0.5,
        });

        let neighbors = graph.get_neighbors("n1");
        assert_eq!(neighbors.len(), 2);

        let no_neighbors = graph.get_neighbors("n2");
        assert_eq!(no_neighbors.len(), 0);
    }

    #[test]
    fn test_node_types() {
        let mut graph = KnowledgeGraph::new();
        let node = make_typed_node("c1", "concept", NodeType::Concept);
        graph.add_node(node);
        assert!(matches!(
            graph.get_node("c1").unwrap().node_type,
            NodeType::Concept
        ));
    }

    #[test]
    fn test_get_all_nodes() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "A"));
        graph.add_node(make_node("n2", "B"));
        let all = graph.get_all_nodes();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_search_by_content() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "Rust is a systems programming language"));
        graph.add_node(make_node("n2", "Python is interpreted"));
        graph.add_node(make_node("n3", "Rust has a borrow checker"));

        let results = graph.search_by_content("rust");
        assert_eq!(results.len(), 2);

        let results = graph.search_by_content("python");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_type() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_typed_node("n1", "doc", NodeType::Document));
        graph.add_node(make_typed_node("n2", "concept", NodeType::Concept));
        graph.add_node(make_typed_node("n3", "entity", NodeType::Entity));

        let docs = graph.search_by_type(&NodeType::Document);
        assert_eq!(docs.len(), 1);

        let concepts = graph.search_by_type(&NodeType::Concept);
        assert_eq!(concepts.len(), 1);
    }

    #[test]
    fn test_edge_queries() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "A"));
        graph.add_node(make_node("n2", "B"));
        graph.add_node(make_node("n3", "C"));
        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n2".to_string(),
            relationship: "rel".to_string(),
            weight: 1.0,
        });
        graph.add_edge(Edge {
            from: "n2".to_string(),
            to: "n3".to_string(),
            relationship: "rel".to_string(),
            weight: 1.0,
        });

        let outgoing = graph.get_outgoing_edges("n1");
        assert_eq!(outgoing.len(), 1);

        let incoming = graph.get_incoming_edges("n2");
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn test_remove_node() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("n1", "A"));
        graph.add_node(make_node("n2", "B"));
        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n2".to_string(),
            relationship: "rel".to_string(),
            weight: 1.0,
        });

        let removed = graph.remove_node("n1");
        assert!(removed.is_some());
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_shortest_path() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("a", "A"));
        graph.add_node(make_node("b", "B"));
        graph.add_node(make_node("c", "C"));
        graph.add_node(make_node("d", "D"));
        graph.add_edge(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            relationship: "r".to_string(),
            weight: 1.0,
        });
        graph.add_edge(Edge {
            from: "b".to_string(),
            to: "c".to_string(),
            relationship: "r".to_string(),
            weight: 1.0,
        });
        graph.add_edge(Edge {
            from: "c".to_string(),
            to: "d".to_string(),
            relationship: "r".to_string(),
            weight: 1.0,
        });

        let path = graph.shortest_path("a", "d");
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn test_shortest_path_same_node() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("a", "A"));
        let path = graph.shortest_path("a", "a").unwrap();
        assert_eq!(path, vec!["a"]);
    }

    #[test]
    fn test_shortest_path_no_route() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("a", "A"));
        graph.add_node(make_node("b", "B"));
        assert!(graph.shortest_path("a", "b").is_none());
    }
}
