//! YAML 工作流加载器
//!
//! 将 YAML 配置文件解析为 WorkflowGraph。
//! 参考 rp-engine 的 YAML-native 设计，让 KIAS 工作流支持声明式定义。
//!
//! ## YAML 格式
//!
//! ```yaml
//! name: code-review
//! description: Automated code review workflow
//! entry: fetch-pr
//!
//! nodes:
//!   - id: fetch-pr
//!     name: Fetch Pull Request
//!     type: tool
//!     config:
//!       tool: github.get_pr
//!     executor:
//!       type: http
//!       endpoint: "${GITHUB_API}/repos/${REPO}/pulls/${PR_NUM}"
//!     error_handler:
//!       action: retry
//!       max_retries: 3
//!
//!   - id: analyze
//!     name: Analyze Code
//!     type: llm
//!     config:
//!       model: gpt-5.5
//!       prompt: "Review this code for issues..."
//!
//!   - id: approve
//!     name: Human Approval
//!     type: human_review
//!     approval:
//!       type: Always
//!
//!   - id: merge
//!     name: Auto Merge
//!     type: tool
//!     config:
//!       tool: github.merge
//!     compensating_action:
//!       tool: github.unmerge
//!
//! edges:
//!   - from: fetch-pr
//!     to: analyze
//!   - from: analyze
//!     to: approve
//!     condition:
//!       expression: "issues_count == 0"
//!       description: "No issues found"
//!   - from: analyze
//!     to: fetch-pr
//!     condition:
//!       expression: "issues_count > 0"
//!       description: "Issues found, re-fetch"
//!   - from: approve
//!     to: merge
//!
//! exits:
//!   - merge
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::edge::{Condition, Edge};
use crate::graph::WorkflowGraph;
use crate::node::{CompensatingAction, ExecutorConfig, Node, NodeType};
use crate::approval::ApprovalPolicy;
use crate::error_handler::ErrorHandlerConfig;

/// YAML 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlWorkflowDef {
    /// 工作流名称
    pub name: String,
    /// 描述
    #[serde(default)]
    pub description: String,
    /// 入口节点 ID
    pub entry: String,
    /// 节点列表
    pub nodes: Vec<YamlNodeDef>,
    /// 边列表
    #[serde(default)]
    pub edges: Vec<YamlEdgeDef>,
    /// 退出节点 ID 列表
    #[serde(default, alias = "exit_nodes")]
    pub exits: Vec<String>,
    /// 全局变量
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
}

/// YAML 节点定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlNodeDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub node_type: YamlNodeType,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub executor: Option<ExecutorConfig>,
    #[serde(default)]
    pub compensating_action: Option<CompensatingAction>,
    #[serde(default)]
    pub error_handler: Option<ErrorHandlerConfig>,
    #[serde(default)]
    pub approval: Option<ApprovalPolicy>,
    /// 超时秒数
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// 重试次数
    #[serde(default)]
    pub retry_count: Option<u32>,
}

/// YAML 节点类型（字符串枚举）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlNodeType {
    /// 处理节点（执行任务）
    #[default]
    Process,
    /// 条件分支
    Condition,
    /// 并行分叉
    Fork,
    /// 并行合并
    Join,
    /// 子工作流
    SubWorkflow,
    /// 人工审核
    HumanReview,
    /// 工具调用（别名，映射为 Process）
    Tool,
    /// LLM 推理（别名，映射为 Process）
    Llm,
    /// HTTP 请求（别名，映射为 Process）
    Http,
    /// 脚本执行（别名，映射为 Process）
    Script,
}

/// YAML 边定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlEdgeDef {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub condition: Option<Condition>,
}

/// YAML 加载错误
#[derive(Debug, thiserror::Error)]
pub enum YamlLoadError {
    #[error("YAML parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Unknown node type: {0}")]
    UnknownNodeType(String),
}

/// 从 YAML 字符串解析工作流
pub fn parse_workflow_yaml(yaml: &str) -> Result<YamlWorkflowDef, YamlLoadError> {
    let def: YamlWorkflowDef = serde_yaml::from_str(yaml)?;
    Ok(def)
}

/// 从 YAML 文件加载工作流
pub fn load_workflow_from_file(path: &Path) -> Result<YamlWorkflowDef, YamlLoadError> {
    let content = std::fs::read_to_string(path)?;
    parse_workflow_yaml(&content)
}

/// 将 YAML 定义转换为 WorkflowGraph
pub fn yaml_to_graph(def: &YamlWorkflowDef) -> Result<WorkflowGraph, YamlLoadError> {
    let mut graph = WorkflowGraph::new(&def.name);

    // 添加节点
    for yaml_node in &def.nodes {
        let node_type = match yaml_node.node_type {
            YamlNodeType::Process | YamlNodeType::Tool | YamlNodeType::Llm | YamlNodeType::Http | YamlNodeType::Script => NodeType::Process,
            YamlNodeType::Condition => NodeType::Condition,
            YamlNodeType::Fork => NodeType::Fork,
            YamlNodeType::Join => NodeType::Join,
            YamlNodeType::HumanReview => NodeType::HumanReview,
            YamlNodeType::SubWorkflow => NodeType::SubWorkflow,
        };

        let mut node = Node::new(&yaml_node.id, &yaml_node.name, node_type);
        node.config = yaml_node.config.clone();
        node.executor = yaml_node.executor.clone();
        node.compensating_action = yaml_node.compensating_action.clone();
        node.error_handler = yaml_node.error_handler.clone();
        node.approval_policy = yaml_node.approval.clone();
        graph.add_node(node);
    }

    // 设置入口
    graph.set_entry(&def.entry);

    // 添加退出节点
    for exit in &def.exits {
        graph.add_exit_node(exit);
    }

    // 添加边
    for yaml_edge in &def.edges {
        let edge = Edge {
            from: yaml_edge.from.clone(),
            to: yaml_edge.to.clone(),
            condition: yaml_edge.condition.clone(),
        };
        graph.add_edge(edge);
    }

    // 验证
    graph
        .validate()
        .map_err(YamlLoadError::ValidationError)?;

    Ok(graph)
}

/// 从 YAML 文件直接构建 WorkflowGraph
pub fn load_graph_from_file(path: &Path) -> Result<WorkflowGraph, YamlLoadError> {
    let def = load_workflow_from_file(path)?;
    yaml_to_graph(&def)
}

/// 从 YAML 字符串直接构建 WorkflowGraph
pub fn load_graph_from_yaml(yaml: &str) -> Result<WorkflowGraph, YamlLoadError> {
    let def = parse_workflow_yaml(yaml)?;
    yaml_to_graph(&def)
}

/// 将 WorkflowGraph 序列化为 YAML
pub fn graph_to_yaml(graph: &WorkflowGraph) -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(graph)
}

/// 变量替换 — 将 ${VAR} 替换为 variables 中的值
pub fn expand_variables(yaml: &str, variables: &HashMap<String, serde_json::Value>) -> String {
    let mut result = yaml.to_string();
    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        let replacement = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&placeholder, &replacement);
    }
    result
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_WORKFLOW: &str = r#"
name: test-workflow
description: A simple test workflow
entry: step1

nodes:
  - id: step1
    name: First Step
    type: tool
    config:
      tool: echo
      args:
        message: "hello"
  - id: step2
    name: Second Step
    type: llm
    config:
      model: gpt-5.5
      prompt: "Analyze this"
  - id: step3
    name: Approval Gate
    type: human_review
    approval:
      type: Always

edges:
  - from: step1
    to: step2
  - from: step2
    to: step3
    condition:
      expression: "score > 0.8"
      description: "High confidence"

exits:
  - step3
"#;

    #[test]
    fn test_parse_simple_workflow() {
        let def = parse_workflow_yaml(SIMPLE_WORKFLOW).unwrap();
        assert_eq!(def.name, "test-workflow");
        assert_eq!(def.nodes.len(), 3);
        assert_eq!(def.edges.len(), 2);
        assert_eq!(def.exits.len(), 1);
        assert_eq!(def.entry, "step1");
    }

    #[test]
    fn test_yaml_to_graph() {
        let def = parse_workflow_yaml(SIMPLE_WORKFLOW).unwrap();
        let graph = yaml_to_graph(&def).unwrap();
        assert_eq!(graph.name, "test-workflow");
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.entry_node, "step1");
        assert_eq!(graph.exit_nodes, vec!["step3"]);
    }

    #[test]
    fn test_load_graph_from_yaml() {
        let graph = load_graph_from_yaml(SIMPLE_WORKFLOW).unwrap();
        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn test_node_types() {
        let def = parse_workflow_yaml(SIMPLE_WORKFLOW).unwrap();
        let step1 = &def.nodes[0];
        assert!(matches!(step1.node_type, YamlNodeType::Tool));
        let step2 = &def.nodes[1];
        assert!(matches!(step2.node_type, YamlNodeType::Llm));
        let step3 = &def.nodes[2];
        assert!(matches!(step3.node_type, YamlNodeType::HumanReview));
    }

    #[test]
    fn test_edge_conditions() {
        let def = parse_workflow_yaml(SIMPLE_WORKFLOW).unwrap();
        let edge_with_cond = def.edges.iter().find(|e| e.condition.is_some()).unwrap();
        let cond = edge_with_cond.condition.as_ref().unwrap();
        assert_eq!(cond.expression, "score > 0.8");
    }

    #[test]
    fn test_validation_error_no_entry() {
        let yaml = r#"
name: bad-workflow
entry: nonexistent
nodes:
  - id: step1
    name: Step 1
    type: tool
edges: []
"#;
        let def = parse_workflow_yaml(yaml).unwrap();
        let result = yaml_to_graph(&def);
        assert!(result.is_err());
    }

    #[test]
    fn test_variable_expansion() {
        let yaml = "endpoint: ${API_URL}/v1/chat";
        let mut vars = HashMap::new();
        vars.insert(
            "API_URL".to_string(),
            serde_json::Value::String("https://api.openai.com".to_string()),
        );
        let expanded = expand_variables(yaml, &vars);
        assert!(expanded.contains("https://api.openai.com/v1/chat"));
    }

    #[test]
    fn test_graph_to_yaml_roundtrip() {
        let graph = load_graph_from_yaml(SIMPLE_WORKFLOW).unwrap();
        let yaml_str = graph_to_yaml(&graph).unwrap();
        assert!(yaml_str.contains("test-workflow"));
        assert!(yaml_str.contains("step1"));
    }

    #[test]
    fn test_empty_workflow() {
        let yaml = r#"
name: empty
entry: ""
nodes: []
edges: []
"#;
        let def = parse_workflow_yaml(yaml).unwrap();
        let result = yaml_to_graph(&def);
        // 应该失败，因为没有入口节点
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_node_type() {
        let yaml = r#"
name: bad-type
entry: step1
nodes:
  - id: step1
    name: Step 1
    type: unknown_type
"#;
        let result = parse_workflow_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_conditional_branching() {
        let yaml = r#"
name: branching
entry: decide

nodes:
  - id: decide
    name: Decision
    type: condition
  - id: path_a
    name: Path A
    type: tool
  - id: path_b
    name: Path B
    type: tool

edges:
  - from: decide
    to: path_a
    condition:
      expression: "value > 10"
      description: "High value"
  - from: decide
    to: path_b
    condition:
      expression: "value <= 10"
      description: "Low value"

exits:
  - path_a
  - path_b
"#;
        let graph = load_graph_from_yaml(yaml).unwrap();
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.exit_nodes.len(), 2);
        let next = graph.get_next_nodes("decide");
        assert_eq!(next.len(), 2);
    }

    #[test]
    fn test_default_node_type() {
        let yaml = r#"
name: defaults
entry: step1
nodes:
  - id: step1
    name: Step 1
"#;
        let def = parse_workflow_yaml(yaml).unwrap();
        assert!(matches!(def.nodes[0].node_type, YamlNodeType::Process));
    }

    #[test]
    fn test_executor_config() {
        let yaml = r#"
name: with-executor
entry: step1
nodes:
  - id: step1
    name: HTTP Call
    type: process
    config:
      url: "https://api.example.com"
    executor:
      type: Http
      method: GET
      url: "https://api.example.com"
"#;
        let def = parse_workflow_yaml(yaml).unwrap();
        let node = &def.nodes[0];
        assert!(node.executor.is_some());
    }

    #[test]
    fn test_compensating_action() {
        let yaml = r#"
name: saga
entry: step1
nodes:
  - id: step1
    name: Create Resource
    type: tool
    config:
      tool: aws.create_instance
    compensating_action:
      description: "Rollback instance creation"
      executor:
        type: Shell
        command: "aws delete-instance"
"#;
        let def = parse_workflow_yaml(yaml).unwrap();
        assert!(def.nodes[0].compensating_action.is_some());
    }

    #[test]
    fn test_multi_variable_expansion() {
        let yaml = "url: ${PROTO}://${HOST}:${PORT}/api";
        let mut vars = HashMap::new();
        vars.insert("PROTO".to_string(), serde_json::json!("https"));
        vars.insert("HOST".to_string(), serde_json::json!("api.example.com"));
        vars.insert("PORT".to_string(), serde_json::json!("443"));
        let expanded = expand_variables(yaml, &vars);
        assert_eq!(expanded, "url: https://api.example.com:443/api");
    }

    #[test]
    fn test_missing_exit_nodes() {
        let yaml = r#"
name: no-exits
entry: step1
nodes:
  - id: step1
    name: Only Step
    type: tool
"#;
        let graph = load_graph_from_yaml(yaml).unwrap();
        assert!(graph.exit_nodes.is_empty());
    }
}
