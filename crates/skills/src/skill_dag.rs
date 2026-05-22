//! # Skill DAG — YAML 声明式多技能编排
//!
//! 在 pipeline.rs（顺序执行）之上，提供 DAG 级编排能力：
//! - 拓扑排序 + 并行级别
//! - 变量引用解析
//! - 循环检测
//! - 执行计划生成
//!
//! ## YAML 格式
//!
//! ```yaml
//! name: code-review
//! inputs:
//!   repo: string
//!
//! nodes:
//!   - id: fetch
//!     skill: github.get_pr
//!     inputs:
//!       url: "${inputs.repo}"
//!
//!   - id: lint
//!     skill: code.lint
//!     depends_on: [fetch]
//!     inputs:
//!       files: "${fetch.files}"
//!
//!   - id: security
//!     skill: code.security_scan
//!     depends_on: [fetch]
//!     inputs:
//!       files: "${fetch.files}"
//!
//!   - id: report
//!     skill: llm.summarize
//!     depends_on: [lint, security]
//!
//! outputs:
//!   result: "${report.output}"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// A skill DAG definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDag {
    /// Pipeline name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Input parameters with types
    pub inputs: Option<HashMap<String, String>>,
    /// DAG nodes
    pub nodes: Vec<DagNode>,
    /// Output mappings
    pub outputs: Option<HashMap<String, String>>,
}

/// A single node in the DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    /// Unique node ID
    pub id: String,
    /// Skill to invoke
    pub skill: String,
    /// Input variable mappings
    pub inputs: Option<HashMap<String, String>>,
    /// Dependencies (must complete first)
    pub depends_on: Option<Vec<String>>,
    /// Conditional execution expression
    pub condition: Option<String>,
    /// Retry policy
    pub retry: Option<DagRetryPolicy>,
    /// Fallback skill
    pub fallback: Option<String>,
    /// On-error behavior
    pub on_error: Option<DagOnError>,
    /// Timeout in seconds
    pub timeout_secs: Option<u64>,
}

/// Retry policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagRetryPolicy {
    /// Max attempts
    pub max_attempts: u32,
    /// Backoff in milliseconds
    pub backoff_ms: Option<u64>,
}

/// Error handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DagOnError {
    Fail,
    Skip,
    Fallback,
    Retry,
}

/// Execution plan with parallel levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagPlan {
    /// Pipeline name
    pub name: String,
    /// Execution levels (each level runs in parallel)
    pub levels: Vec<DagLevel>,
    /// Total nodes
    pub total_nodes: usize,
    /// Critical path depth
    pub depth: usize,
}

/// A parallel execution level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagLevel {
    /// Level index
    pub level: usize,
    /// Node IDs in this level
    pub nodes: Vec<String>,
}

/// DAG validation error
#[derive(Debug, Clone)]
pub enum DagError {
    DuplicateNode(String),
    MissingDependency { node: String, missing: String },
    CircularDependency(Vec<String>),
    InvalidReference(String),
    ParseError(String),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(f, "Duplicate node: {}", id),
            Self::MissingDependency { node, missing } => {
                write!(f, "Node '{}' depends on missing '{}'", node, missing)
            }
            Self::CircularDependency(cycle) => {
                write!(f, "Circular dependency: {}", cycle.join(" → "))
            }
            Self::InvalidReference(r) => write!(f, "Invalid reference: {}", r),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for DagError {}

impl SkillDag {
    /// Parse from YAML
    pub fn from_yaml(yaml: &str) -> Result<Self, DagError> {
        serde_yaml::from_str(yaml).map_err(|e| DagError::ParseError(e.to_string()))
    }

    /// Validate the DAG structure
    pub fn validate(&self) -> Result<(), DagError> {
        // 1. Duplicate IDs
        let mut seen = HashSet::new();
        for node in &self.nodes {
            if !seen.insert(&node.id) {
                return Err(DagError::DuplicateNode(node.id.clone()));
            }
        }

        // 2. Missing dependencies
        let node_ids: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &self.nodes {
            if let Some(deps) = &node.depends_on {
                for dep in deps {
                    if !node_ids.contains(dep.as_str()) {
                        return Err(DagError::MissingDependency {
                            node: node.id.clone(),
                            missing: dep.clone(),
                        });
                    }
                }
            }
        }

        // 3. Circular dependencies (Kahn's algorithm)
        self.detect_cycles()?;

        // 4. Variable references
        self.validate_refs()?;

        Ok(())
    }

    /// Detect cycles via topological sort
    fn detect_cycles(&self) -> Result<(), DagError> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in &self.nodes {
            in_degree.entry(&node.id).or_insert(0);
            adj.entry(&node.id).or_default();
            if let Some(deps) = &node.depends_on {
                for dep in deps {
                    adj.entry(dep).or_default().push(&node.id);
                    *in_degree.entry(&node.id).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut visited = 0;
        while let Some(cur) = queue.pop_front() {
            visited += 1;
            for &next in adj.get(cur).unwrap_or(&vec![]) {
                let deg = in_degree
                    .get_mut(next)
                    .expect("in_degree populated for all nodes in setup loop");
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(next);
                }
            }
        }

        if visited < self.nodes.len() {
            let cycle: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| in_degree.get(n.id.as_str()).copied().unwrap_or(0) > 0)
                .map(|n| n.id.clone())
                .collect();
            Err(DagError::CircularDependency(cycle))
        } else {
            Ok(())
        }
    }

    /// Validate ${ref} references
    fn validate_refs(&self) -> Result<(), DagError> {
        let node_ids: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();

        for node in &self.nodes {
            if let Some(inputs) = &node.inputs {
                for val in inputs.values() {
                    let mut rem = val.as_str();
                    while let Some(start) = rem.find("${") {
                        if let Some(end) = rem[start..].find('}') {
                            let ref_ = &rem[start + 2..start + end];
                            rem = &rem[start + end + 1..];

                            if ref_.starts_with("inputs.") {
                                continue;
                            }

                            let node_name = ref_.split('.').next().unwrap_or("");
                            if !node_ids.contains(node_name) {
                                return Err(DagError::InvalidReference(format!(
                                    "${{{}}} — node '{}' not found",
                                    ref_, node_name
                                )));
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Build execution plan (topological sort with parallel levels)
    pub fn build_plan(&self) -> Result<DagPlan, DagError> {
        self.validate()?;

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in &self.nodes {
            in_degree.entry(&node.id).or_insert(0);
            adj.entry(&node.id).or_default();
            if let Some(deps) = &node.depends_on {
                for dep in deps {
                    adj.entry(dep).or_default().push(&node.id);
                    *in_degree.entry(&node.id).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<(&str, usize)> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| (id, 0))
            .collect();

        let mut level_map: HashMap<usize, Vec<String>> = HashMap::new();

        while let Some((cur, level)) = queue.pop_front() {
            level_map.entry(level).or_default().push(cur.to_string());
            for &next in adj.get(cur).unwrap_or(&vec![]) {
                let deg = in_degree
                    .get_mut(next)
                    .expect("in_degree populated for all nodes in setup loop");
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back((next, level + 1));
                }
            }
        }

        let mut levels: Vec<DagLevel> = level_map
            .into_iter()
            .map(|(level, nodes)| DagLevel { level, nodes })
            .collect();
        levels.sort_by_key(|l| l.level);

        let depth = levels.len();

        Ok(DagPlan {
            name: self.name.clone(),
            levels,
            total_nodes: self.nodes.len(),
            depth,
        })
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<&DagNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Transitive dependencies
    pub fn all_deps(&self, node_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(node) = self.get_node(node_id) {
            if let Some(deps) = &node.depends_on {
                for dep in deps {
                    queue.push_back(dep.clone());
                }
            }
        }

        while let Some(cur) = queue.pop_front() {
            if !visited.insert(cur.clone()) {
                continue;
            }
            result.push(cur.clone());
            if let Some(node) = self.get_node(&cur) {
                if let Some(deps) = &node.depends_on {
                    for dep in deps {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        result
    }

    /// Transitive downstream nodes
    pub fn all_downstream(&self, node_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(node_id.to_string());
        let mut queue = VecDeque::new();
        queue.push_back(node_id.to_string());

        while let Some(cur) = queue.pop_front() {
            for node in &self.nodes {
                if let Some(deps) = &node.depends_on {
                    if deps.contains(&cur) && visited.insert(node.id.clone()) {
                        result.push(node.id.clone());
                        queue.push_back(node.id.clone());
                    }
                }
            }
        }

        result
    }
}

impl DagPlan {
    /// Human-readable plan display
    pub fn display(&self) -> String {
        let mut out = format!(
            "DAG: {} | Nodes: {} | Depth: {}\n",
            self.name, self.total_nodes, self.depth
        );
        for level in &self.levels {
            out.push_str(&format!(
                "  L{}: [{}]\n",
                level.level,
                level.nodes.join(", ")
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_dag() -> &'static str {
        r#"
name: test-dag
inputs:
  file: string

nodes:
  - id: read
    skill: file.read
    inputs:
      path: "${inputs.file}"

  - id: lint
    skill: code.lint
    depends_on: [read]
    inputs:
      files: "${read.content}"

  - id: security
    skill: code.security_scan
    depends_on: [read]
    inputs:
      files: "${read.content}"

  - id: report
    skill: llm.summarize
    depends_on: [lint, security]

outputs:
  result: "${report.output}"
"#
    }

    #[test]
    fn test_parse() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        assert_eq!(dag.name, "test-dag");
        assert_eq!(dag.nodes.len(), 4);
    }

    #[test]
    fn test_validate_ok() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_duplicate_node() {
        let yaml = r#"
name: bad
nodes:
  - id: a
    skill: x
  - id: a
    skill: y
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        assert!(matches!(dag.validate(), Err(DagError::DuplicateNode(_))));
    }

    #[test]
    fn test_missing_dep() {
        let yaml = r#"
name: bad
nodes:
  - id: a
    skill: x
    depends_on: [ghost]
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        assert!(matches!(
            dag.validate(),
            Err(DagError::MissingDependency { .. })
        ));
    }

    #[test]
    fn test_circular() {
        let yaml = r#"
name: cycle
nodes:
  - id: a
    skill: x
    depends_on: [c]
  - id: b
    skill: y
    depends_on: [a]
  - id: c
    skill: z
    depends_on: [b]
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        assert!(matches!(
            dag.validate(),
            Err(DagError::CircularDependency(_))
        ));
    }

    #[test]
    fn test_plan_levels() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        let plan = dag.build_plan().unwrap();

        assert_eq!(plan.total_nodes, 4);
        assert_eq!(plan.levels.len(), 3); // read → (lint, security) → report
        assert_eq!(plan.levels[0].nodes, vec!["read"]);
        assert_eq!(plan.levels[1].nodes.len(), 2);
        assert_eq!(plan.levels[2].nodes, vec!["report"]);
    }

    #[test]
    fn test_plan_diamond() {
        let yaml = r#"
name: diamond
nodes:
  - id: split
    skill: split
  - id: a
    skill: a
    depends_on: [split]
  - id: b
    skill: b
    depends_on: [split]
  - id: merge
    skill: merge
    depends_on: [a, b]
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        let plan = dag.build_plan().unwrap();
        assert_eq!(plan.depth, 3);
    }

    #[test]
    fn test_all_deps() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        let deps = dag.all_deps("report");
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn test_all_downstream() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        let ds = dag.all_downstream("read");
        assert_eq!(ds.len(), 3);
    }

    #[test]
    fn test_invalid_ref() {
        let yaml = r#"
name: bad-ref
nodes:
  - id: a
    skill: x
    inputs:
      data: "${ghost.output}"
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        assert!(matches!(dag.validate(), Err(DagError::InvalidReference(_))));
    }

    #[test]
    fn test_input_ref_ok() {
        let yaml = r#"
name: ok
inputs:
  p: string
nodes:
  - id: a
    skill: x
    inputs:
      data: "${inputs.p}"
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_display() {
        let dag = SkillDag::from_yaml(simple_dag()).unwrap();
        let plan = dag.build_plan().unwrap();
        let d = plan.display();
        assert!(d.contains("test-dag"));
        assert!(d.contains("Nodes: 4"));
    }

    #[test]
    fn test_node_with_retry() {
        let yaml = r#"
name: retry-test
nodes:
  - id: a
    skill: api.call
    retry:
      max_attempts: 3
      backoff_ms: 100
    fallback: api.cached
    on_error: Fallback
    timeout_secs: 30
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        let node = dag.get_node("a").unwrap();
        assert_eq!(node.retry.as_ref().unwrap().max_attempts, 3);
        assert_eq!(node.fallback, Some("api.cached".to_string()));
    }

    #[test]
    fn test_chain() {
        let yaml = r#"
name: chain
nodes:
  - id: a
    skill: s1
  - id: b
    skill: s2
    depends_on: [a]
  - id: c
    skill: s3
    depends_on: [b]
  - id: d
    skill: s4
    depends_on: [c]
"#;
        let dag = SkillDag::from_yaml(yaml).unwrap();
        let plan = dag.build_plan().unwrap();
        assert_eq!(plan.depth, 4);
        assert_eq!(plan.levels.len(), 4);
    }
}
