//! 可视化编排器数据模型
//!
//! 提供可视化节点/边/布局算法，以及代码↔图形双向同步接口。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 节点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualNodeType {
    Start,
    End,
    Task,
    Decision,
    Parallel,
    Merge,
    LLM,
    Tool,
    SubWorkflow,
}

/// 可视化节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: String,
    pub name: String,
    pub node_type: VisualNodeType,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub config: HashMap<String, String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

impl VisualNode {
    pub fn new(id: String, name: String, node_type: VisualNodeType) -> Self {
        Self {
            id,
            name,
            node_type,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 60.0,
            config: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn with_position(mut self, x: f64, y: f64) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// 可视化边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub condition: Option<String>,
}

impl VisualEdge {
    pub fn new(id: String, source: String, target: String) -> Self {
        Self {
            id,
            source,
            target,
            label: None,
            condition: None,
        }
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn with_condition(mut self, condition: &str) -> Self {
        self.condition = Some(condition.to_string());
        self
    }
}

/// 自动布局算法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAlgorithm {
    Hierarchical,
    ForceDirected,
    Grid,
    Dagre,
}

/// 编排布局
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationLayout {
    pub nodes: Vec<VisualNode>,
    pub edges: Vec<VisualEdge>,
    pub algorithm: LayoutAlgorithm,
    pub metadata: HashMap<String, String>,
}

impl OrchestrationLayout {
    pub fn new(algorithm: LayoutAlgorithm) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            algorithm,
            metadata: HashMap::new(),
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, node: VisualNode) {
        self.nodes.push(node);
    }

    /// 添加边
    pub fn add_edge(&mut self, edge: VisualEdge) {
        self.edges.push(edge);
    }

    /// 执行层级布局
    pub fn apply_hierarchical_layout(&mut self) {
        // 简单的层级布局：从左到右，按拓扑排序
        let mut levels: HashMap<String, usize> = HashMap::new();
        let mut visited: HashSet<String> = HashSet::new();

        // 找出入口节点（没有入边的节点）
        let has_incoming: HashSet<String> = self.edges.iter().map(|e| e.target.clone()).collect();

        for node in &self.nodes {
            if !has_incoming.contains(&node.id) {
                self.assign_level(&node.id, 0, &mut levels, &mut visited);
            }
        }

        // 处理未访问的节点
        for node in &self.nodes {
            if !visited.contains(&node.id) {
                self.assign_level(&node.id, 0, &mut levels, &mut visited);
            }
        }

        // 根据层级设置位置
        let mut level_counts: HashMap<usize, usize> = HashMap::new();
        for level in levels.values() {
            *level_counts.entry(*level).or_default() += 1;
        }

        let node_spacing_x = 200.0;
        let node_spacing_y = 120.0;
        let mut level_positions: HashMap<usize, Vec<usize>> = HashMap::new();

        for (node_id, level) in &levels {
            let level_nodes = level_positions.entry(*level).or_default();
            if let Some(pos) = self.nodes.iter().position(|n| &n.id == node_id) {
                level_nodes.push(pos);
            }
        }

        for (level, node_indices) in &mut level_positions {
            for (i, &node_idx) in node_indices.iter().enumerate() {
                let x = (*level as f64) * node_spacing_x;
                let y = (i as f64) * node_spacing_y;
                self.nodes[node_idx].x = x;
                self.nodes[node_idx].y = y;
            }
        }
    }

    fn assign_level(
        &self,
        node_id: &str,
        level: usize,
        levels: &mut HashMap<String, usize>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(node_id) {
            return;
        }
        visited.insert(node_id.to_string());
        levels.insert(node_id.to_string(), level);

        // 找到该节点的所有出边
        for edge in &self.edges {
            if edge.source == node_id {
                self.assign_level(&edge.target, level + 1, levels, visited);
            }
        }
    }

    /// 执行力导向布局
    pub fn apply_force_directed_layout(&mut self) {
        let iterations = 50;
        let repulsion = 5000.0;
        let attraction = 0.01;

        // 初始化位置
        for (i, node) in self.nodes.iter_mut().enumerate() {
            node.x = (i as f64) * 150.0;
            node.y = (i as f64) * 100.0;
        }

        for _ in 0..iterations {
            // 计算斥力
            let mut forces: HashMap<String, (f64, f64)> = HashMap::new();
            for node in &self.nodes {
                forces.insert(node.id.clone(), (0.0, 0.0));
            }

            // 节点间斥力
            for i in 0..self.nodes.len() {
                for j in (i + 1)..self.nodes.len() {
                    let dx = self.nodes[j].x - self.nodes[i].x;
                    let dy = self.nodes[j].y - self.nodes[i].y;
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let force = repulsion / (dist * dist);
                    let fx = force * dx / dist;
                    let fy = force * dy / dist;

                    if let Some((x, y)) = forces.get_mut(&self.nodes[i].id) {
                        *x -= fx;
                        *y -= fy;
                    }
                    if let Some((x, y)) = forces.get_mut(&self.nodes[j].id) {
                        *x += fx;
                        *y += fy;
                    }
                }
            }

            // 边引力
            for edge in &self.edges {
                let source_idx = self.nodes.iter().position(|n| n.id == edge.source);
                let target_idx = self.nodes.iter().position(|n| n.id == edge.target);
                if let (Some(si), Some(ti)) = (source_idx, target_idx) {
                    let dx = self.nodes[ti].x - self.nodes[si].x;
                    let dy = self.nodes[ti].y - self.nodes[si].y;
                    if let Some((x, y)) = forces.get_mut(&self.nodes[si].id) {
                        *x += dx * attraction;
                        *y += dy * attraction;
                    }
                    if let Some((x, y)) = forces.get_mut(&self.nodes[ti].id) {
                        *x -= dx * attraction;
                        *y -= dy * attraction;
                    }
                }
            }

            // 应用力
            for node in &mut self.nodes {
                if let Some((fx, fy)) = forces.get(&node.id) {
                    node.x = (node.x + fx).max(0.0);
                    node.y = (node.y + fy).max(0.0);
                }
            }
        }
    }

    /// 应用网格布局
    pub fn apply_grid_layout(&mut self) {
        let cols = ((self.nodes.len() as f64).sqrt().ceil() as usize).max(1);
        let spacing_x = 180.0;
        let spacing_y = 120.0;

        for (i, node) in self.nodes.iter_mut().enumerate() {
            node.x = (i % cols) as f64 * spacing_x;
            node.y = (i / cols) as f64 * spacing_y;
        }
    }

    /// 应用布局
    pub fn apply_layout(&mut self) {
        match self.algorithm {
            LayoutAlgorithm::Hierarchical => self.apply_hierarchical_layout(),
            LayoutAlgorithm::ForceDirected => self.apply_force_directed_layout(),
            LayoutAlgorithm::Grid => self.apply_grid_layout(),
            LayoutAlgorithm::Dagre => self.apply_hierarchical_layout(), // 简化实现
        }
    }
}

use std::collections::HashSet;

/// 同步状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_synced: chrono::DateTime<chrono::Utc>,
    pub source: SyncSource,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncSource {
    Code,
    Graphical,
}

/// 代码↔图形双向同步器
pub struct VisualOrchestrator {
    layout: OrchestrationLayout,
    code_snapshot: Option<String>,
    graph_snapshot: Option<String>,
    sync_status: Option<SyncStatus>,
}

impl Default for VisualOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualOrchestrator {
    pub fn new() -> Self {
        Self {
            layout: OrchestrationLayout::new(LayoutAlgorithm::Hierarchical),
            code_snapshot: None,
            graph_snapshot: None,
            sync_status: None,
        }
    }

    /// 从代码同步到图形
    pub fn sync_code_to_graph(&mut self, code: &str) -> Result<&OrchestrationLayout, SyncError> {
        // 简单的解析逻辑：提取节点和边信息
        self.code_snapshot = Some(code.to_string());
        // 在实际实现中，这里会解析代码生成节点和边
        self.sync_status = Some(SyncStatus {
            last_synced: chrono::Utc::now(),
            source: SyncSource::Code,
            version: Uuid::new_v4().to_string(),
        });
        Ok(&self.layout)
    }

    /// 从图形同步到代码
    pub fn sync_graph_to_code(&mut self) -> Result<String, SyncError> {
        let mut code = String::from("// Generated workflow code\n");
        code.push_str("workflow {\n");
        for node in &self.layout.nodes {
            code.push_str(&format!(
                "  node(\"{}\", {}: {}, x: {}, y: {})\n",
                node.name,
                format!("{:?}", node.node_type).to_lowercase(),
                node.config.len(),
                node.x,
                node.y
            ));
        }
        for edge in &self.layout.edges {
            code.push_str(&format!(
                "  edge(\"{}\" -> \"{}\")\n",
                edge.source, edge.target
            ));
        }
        code.push_str("}\n");
        self.graph_snapshot = Some(code.clone());
        Ok(code)
    }

    /// 获取布局
    pub fn get_layout(&self) -> &OrchestrationLayout {
        &self.layout
    }

    /// 获取布局的可变引用
    pub fn get_layout_mut(&mut self) -> &mut OrchestrationLayout {
        &mut self.layout
    }

    /// 添加节点
    pub fn add_node(&mut self, node: VisualNode) {
        self.layout.add_node(node);
    }

    /// 添加边
    pub fn add_edge(&mut self, edge: VisualEdge) {
        self.layout.add_edge(edge);
    }

    /// 检查是否需要同步
    pub fn needs_sync(&self) -> bool {
        self.code_snapshot.is_some() || self.graph_snapshot.is_some()
    }

    /// 获取同步状态
    pub fn get_sync_status(&self) -> Option<&SyncStatus> {
        self.sync_status.as_ref()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Sync conflict")]
    SyncConflict,
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_node_creation() {
        let node = VisualNode::new(
            "n1".to_string(),
            "Test Node".to_string(),
            VisualNodeType::Task,
        );
        assert_eq!(node.id, "n1");
        assert_eq!(node.name, "Test Node");
        assert_eq!(node.node_type, VisualNodeType::Task);
    }

    #[test]
    fn test_visual_node_position() {
        let node = VisualNode::new("n1".to_string(), "Test".to_string(), VisualNodeType::Task)
            .with_position(100.0, 200.0)
            .with_size(150.0, 80.0);
        assert_eq!(node.x, 100.0);
        assert_eq!(node.y, 200.0);
        assert_eq!(node.width, 150.0);
        assert_eq!(node.height, 80.0);
    }

    #[test]
    fn test_visual_node_center() {
        let node = VisualNode::new("n1".to_string(), "Test".to_string(), VisualNodeType::Task)
            .with_position(100.0, 200.0)
            .with_size(100.0, 60.0);
        assert_eq!(node.center(), (150.0, 230.0));
    }

    #[test]
    fn test_visual_edge_creation() {
        let edge = VisualEdge::new("e1".to_string(), "n1".to_string(), "n2".to_string())
            .with_label("next")
            .with_condition("success");
        assert_eq!(edge.id, "e1");
        assert_eq!(edge.source, "n1");
        assert_eq!(edge.target, "n2");
        assert_eq!(edge.label, Some("next".to_string()));
        assert_eq!(edge.condition, Some("success".to_string()));
    }

    #[test]
    fn test_orchestration_layout_grid() {
        let mut layout = OrchestrationLayout::new(LayoutAlgorithm::Grid);
        layout.add_node(VisualNode::new(
            "n1".to_string(),
            "Node 1".to_string(),
            VisualNodeType::Task,
        ));
        layout.add_node(VisualNode::new(
            "n2".to_string(),
            "Node 2".to_string(),
            VisualNodeType::Task,
        ));
        layout.add_node(VisualNode::new(
            "n3".to_string(),
            "Node 3".to_string(),
            VisualNodeType::Task,
        ));
        layout.apply_layout();
        assert_eq!(layout.nodes[0].x, 0.0);
        assert_eq!(layout.nodes[0].y, 0.0);
        assert_eq!(layout.nodes[1].x, 180.0);
        assert_eq!(layout.nodes[1].y, 0.0);
    }

    #[test]
    fn test_visual_orchestrator_add_node() {
        let mut orchestrator = VisualOrchestrator::new();
        orchestrator.add_node(VisualNode::new(
            "n1".to_string(),
            "Node 1".to_string(),
            VisualNodeType::Start,
        ));
        assert_eq!(orchestrator.get_layout().nodes.len(), 1);
    }

    #[test]
    fn test_visual_orchestrator_sync_graph_to_code() {
        let mut orchestrator = VisualOrchestrator::new();
        orchestrator.add_node(VisualNode::new(
            "n1".to_string(),
            "Start".to_string(),
            VisualNodeType::Start,
        ));
        orchestrator.add_edge(VisualEdge::new(
            "e1".to_string(),
            "n1".to_string(),
            "n2".to_string(),
        ));
        let code = orchestrator.sync_graph_to_code().unwrap();
        assert!(code.contains("workflow"));
        assert!(code.contains("n1"));
    }

    #[test]
    fn test_sync_status() {
        let status = SyncStatus {
            last_synced: chrono::Utc::now(),
            source: SyncSource::Graphical,
            version: "v1".to_string(),
        };
        assert_eq!(status.source, SyncSource::Graphical);
    }
}
