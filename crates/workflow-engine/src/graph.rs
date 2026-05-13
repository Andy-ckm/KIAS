use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::node::Node;
use super::edge::Edge;

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
