use super::edge::Edge;
use super::node::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工作流图（借鉴 LangGraph StateGraph）
///
/// 核心设计：
/// 1. 节点是处理步骤
/// 2. 边是条件分支
/// 3. 支持循环、分支、并行等复杂拓扑
/// 4. 状态对象在图中流转
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub id: String,
    pub name: String,
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
    pub entry_node: String,
    pub exit_nodes: Vec<String>,
}

impl WorkflowGraph {
    pub fn new(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            entry_node: String::new(),
            exit_nodes: Vec::new(),
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, node: Node) {
        if self.nodes.is_empty() {
            self.entry_node = node.id.clone();
        }
        self.nodes.insert(node.id.clone(), node);
    }

    /// 添加边
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// 设置入口节点
    pub fn set_entry(&mut self, node_id: &str) {
        self.entry_node = node_id.to_string();
    }

    /// 添加退出节点
    pub fn add_exit_node(&mut self, node_id: &str) {
        self.exit_nodes.push(node_id.to_string());
    }

    /// 获取节点的后续节点
    pub fn get_next_nodes(&self, node_id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.from == node_id)
            .map(|e| e.to.as_str())
            .collect()
    }

    /// 验证图结构
    pub fn validate(&self) -> Result<(), String> {
        if self.entry_node.is_empty() {
            return Err("No entry node defined".to_string());
        }
        if !self.nodes.contains_key(&self.entry_node) {
            return Err("Entry node not found".to_string());
        }
        for exit in &self.exit_nodes {
            if !self.nodes.contains_key(exit) {
                return Err(format!("Exit node '{}' not found", exit));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{Node, NodeType};

    fn make_node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            name: format!("Node {id}"),
            node_type: NodeType::Process,
            config: serde_json::json!({}),
            executor: None,
            compensating_action: None,
            error_handler: None,
            approval_policy: None,
        }
    }

    #[test]
    fn test_new_graph() {
        let g = WorkflowGraph::new("test");
        assert_eq!(g.name, "test");
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
        assert!(g.entry_node.is_empty());
        assert!(g.exit_nodes.is_empty());
    }

    #[test]
    fn test_add_node_sets_entry_on_first() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        assert_eq!(g.entry_node, "a");
        g.add_node(make_node("b"));
        assert_eq!(g.entry_node, "a"); // unchanged
    }

    #[test]
    fn test_add_edge_and_get_next() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        g.add_node(make_node("b"));
        g.add_node(make_node("c"));
        g.add_edge(Edge::new("a", "b"));
        g.add_edge(Edge::new("a", "c"));
        let next = g.get_next_nodes("a");
        assert_eq!(next.len(), 2);
        assert!(next.contains(&"b"));
        assert!(next.contains(&"c"));
        assert!(g.get_next_nodes("b").is_empty());
    }

    #[test]
    fn test_set_entry() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        g.add_node(make_node("b"));
        g.set_entry("b");
        assert_eq!(g.entry_node, "b");
    }

    #[test]
    fn test_add_exit_node() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        g.add_exit_node("a");
        assert_eq!(g.exit_nodes, vec!["a"]);
    }

    #[test]
    fn test_validate_ok() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        g.add_exit_node("a");
        assert!(g.validate().is_ok());
    }

    #[test]
    fn test_validate_no_entry() {
        let g = WorkflowGraph::new("t");
        assert_eq!(g.validate().unwrap_err(), "No entry node defined");
    }

    #[test]
    fn test_validate_entry_not_found() {
        let mut g = WorkflowGraph::new("t");
        g.entry_node = "missing".to_string();
        assert_eq!(g.validate().unwrap_err(), "Entry node not found");
    }

    #[test]
    fn test_validate_exit_not_found() {
        let mut g = WorkflowGraph::new("t");
        g.add_node(make_node("a"));
        g.add_exit_node("missing");
        assert!(g.validate().unwrap_err().contains("Exit node"));
    }

    #[test]
    fn test_conditional_edge() {
        let edge = Edge::new("a", "b").with_condition("x > 0", "positive");
        assert!(edge.condition.is_some());
        let cond = edge.condition.unwrap();
        assert_eq!(cond.expression, "x > 0");
    }
}
