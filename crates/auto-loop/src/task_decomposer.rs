//! Task Decomposer — 任务拆解引擎
//!
//! 将识别后的意图拆解为可执行的任务图（DAG）。
//!
//! # 参考来源
//! - DeepResearchAgent: 分层任务规划
//! - Dify Agent 工作流: DAG 执行引擎
//! - K8S: Pod 调度与依赖管理

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::intent_recognizer::{Complexity, IntentType, RecognizedIntent};

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// 任务节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// 任务 ID
    pub id: String,
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 依赖的任务 ID
    pub dependencies: Vec<String>,
    /// 预估耗时（秒）
    pub estimated_duration: u64,
    /// 任务状态
    pub status: TaskStatus,
    /// 所需技能
    pub required_skills: Vec<String>,
}

/// 任务图（DAG）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    /// 所有任务节点
    pub nodes: HashMap<String, TaskNode>,
    /// 边列表 (from, to)
    pub edges: Vec<(String, String)>,
    /// 图的根节点（无依赖）
    pub roots: Vec<String>,
    /// 图的叶子节点（无后续）
    pub leaves: Vec<String>,
}

/// 任务拆解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionResult {
    /// 原始意图
    pub intent: RecognizedIntent,
    /// 任务图
    pub task_graph: TaskGraph,
    /// 总预估耗时（秒）
    pub total_estimated_duration: u64,
    /// 任务数量
    pub task_count: usize,
    /// 是否需要多 Agent 协作
    pub requires_multi_agent: bool,
}

/// 任务拆解器
pub struct TaskDecomposer {
    /// 任务模板
    templates: HashMap<IntentType, Vec<TaskTemplate>>,
}

/// 任务模板
struct TaskTemplate {
    name: String,
    description: String,
    skills: Vec<String>,
    base_duration: u64,
}

impl TaskDecomposer {
    /// 创建默认拆解器
    pub fn new() -> Self {
        Self {
            templates: Self::default_templates(),
        }
    }

    /// 默认任务模板
    fn default_templates() -> HashMap<IntentType, Vec<TaskTemplate>> {
        let mut templates = HashMap::new();

        // 代码生成模板
        templates.insert(
            IntentType::CodeGeneration,
            vec![
                TaskTemplate {
                    name: "需求分析".into(),
                    description: "分析代码需求，确定接口和数据结构".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                },
                TaskTemplate {
                    name: "代码实现".into(),
                    description: "编写核心代码实现".into(),
                    skills: vec!["coding".into()],
                    base_duration: 180,
                },
                TaskTemplate {
                    name: "单元测试".into(),
                    description: "编写单元测试验证功能".into(),
                    skills: vec!["testing".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "代码审查".into(),
                    description: "审查代码质量和规范".into(),
                    skills: vec!["review".into()],
                    base_duration: 60,
                },
            ],
        );

        // Bug 修复模板
        templates.insert(
            IntentType::BugFix,
            vec![
                TaskTemplate {
                    name: "问题定位".into(),
                    description: "分析错误日志，定位问题根因".into(),
                    skills: vec!["debugging".into()],
                    base_duration: 90,
                },
                TaskTemplate {
                    name: "修复方案".into(),
                    description: "设计修复方案".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                },
                TaskTemplate {
                    name: "代码修复".into(),
                    description: "实现修复代码".into(),
                    skills: vec!["coding".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "回归测试".into(),
                    description: "验证修复并运行回归测试".into(),
                    skills: vec!["testing".into()],
                    base_duration: 90,
                },
            ],
        );

        // 代码审查模板
        templates.insert(
            IntentType::CodeReview,
            vec![
                TaskTemplate {
                    name: "静态分析".into(),
                    description: "运行静态分析工具".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 30,
                },
                TaskTemplate {
                    name: "代码审查".into(),
                    description: "人工审查代码逻辑和风格".into(),
                    skills: vec!["review".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "问题报告".into(),
                    description: "生成审查报告和改进建议".into(),
                    skills: vec!["documentation".into()],
                    base_duration: 60,
                },
            ],
        );

        // 测试生成模板
        templates.insert(
            IntentType::TestGeneration,
            vec![
                TaskTemplate {
                    name: "测试分析".into(),
                    description: "分析代码结构，确定测试覆盖点".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                },
                TaskTemplate {
                    name: "测试用例设计".into(),
                    description: "设计测试用例和边界条件".into(),
                    skills: vec!["testing".into()],
                    base_duration: 90,
                },
                TaskTemplate {
                    name: "测试代码编写".into(),
                    description: "编写测试代码".into(),
                    skills: vec!["coding".into(), "testing".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "测试执行".into(),
                    description: "运行测试并验证结果".into(),
                    skills: vec!["testing".into()],
                    base_duration: 60,
                },
            ],
        );

        // 架构设计模板
        templates.insert(
            IntentType::ArchitectureDesign,
            vec![
                TaskTemplate {
                    name: "需求分析".into(),
                    description: "分析系统需求和约束".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "架构设计".into(),
                    description: "设计系统架构和组件".into(),
                    skills: vec!["architecture".into()],
                    base_duration: 180,
                },
                TaskTemplate {
                    name: "接口定义".into(),
                    description: "定义组件接口和协议".into(),
                    skills: vec!["design".into()],
                    base_duration: 90,
                },
                TaskTemplate {
                    name: "文档编写".into(),
                    description: "编写架构文档".into(),
                    skills: vec!["documentation".into()],
                    base_duration: 120,
                },
            ],
        );

        // 性能优化模板
        templates.insert(
            IntentType::PerformanceOptimization,
            vec![
                TaskTemplate {
                    name: "性能分析".into(),
                    description: "运行性能分析工具，定位瓶颈".into(),
                    skills: vec!["profiling".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "优化方案".into(),
                    description: "设计性能优化方案".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 90,
                },
                TaskTemplate {
                    name: "代码优化".into(),
                    description: "实现性能优化".into(),
                    skills: vec!["coding".into()],
                    base_duration: 180,
                },
                TaskTemplate {
                    name: "基准测试".into(),
                    description: "运行基准测试验证优化效果".into(),
                    skills: vec!["testing".into()],
                    base_duration: 90,
                },
            ],
        );

        // 安全审计模板
        templates.insert(
            IntentType::SecurityAudit,
            vec![
                TaskTemplate {
                    name: "漏洞扫描".into(),
                    description: "运行安全扫描工具".into(),
                    skills: vec!["security".into()],
                    base_duration: 90,
                },
                TaskTemplate {
                    name: "代码审计".into(),
                    description: "人工审查安全敏感代码".into(),
                    skills: vec!["security".into(), "review".into()],
                    base_duration: 180,
                },
                TaskTemplate {
                    name: "渗透测试".into(),
                    description: "进行渗透测试".into(),
                    skills: vec!["security".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "安全报告".into(),
                    description: "生成安全审计报告".into(),
                    skills: vec!["documentation".into()],
                    base_duration: 60,
                },
            ],
        );

        // 文档生成模板
        templates.insert(
            IntentType::Documentation,
            vec![
                TaskTemplate {
                    name: "内容分析".into(),
                    description: "分析代码和功能，提取关键信息".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                },
                TaskTemplate {
                    name: "文档编写".into(),
                    description: "编写文档内容".into(),
                    skills: vec!["documentation".into()],
                    base_duration: 120,
                },
                TaskTemplate {
                    name: "示例编写".into(),
                    description: "编写使用示例".into(),
                    skills: vec!["coding".into()],
                    base_duration: 60,
                },
            ],
        );

        // 知识查询模板
        templates.insert(
            IntentType::KnowledgeQuery,
            vec![
                TaskTemplate {
                    name: "知识检索".into(),
                    description: "从知识库检索相关信息".into(),
                    skills: vec!["search".into()],
                    base_duration: 30,
                },
                TaskTemplate {
                    name: "知识整合".into(),
                    description: "整合检索结果，生成答案".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                },
            ],
        );

        // 系统管理模板
        templates.insert(
            IntentType::SystemAdmin,
            vec![
                TaskTemplate {
                    name: "状态检查".into(),
                    description: "检查系统状态和配置".into(),
                    skills: vec!["monitoring".into()],
                    base_duration: 30,
                },
                TaskTemplate {
                    name: "配置变更".into(),
                    description: "执行配置变更".into(),
                    skills: vec!["configuration".into()],
                    base_duration: 60,
                },
                TaskTemplate {
                    name: "验证确认".into(),
                    description: "验证变更生效".into(),
                    skills: vec!["testing".into()],
                    base_duration: 30,
                },
            ],
        );

        templates
    }

    /// 拆解意图为任务图
    pub fn decompose(&self, intent: &RecognizedIntent) -> DecompositionResult {
        let templates = self.templates.get(&intent.intent_type);
        let task_count = match intent.complexity {
            Complexity::Simple => 1,
            Complexity::Medium => 3,
            Complexity::Complex => 5,
        };

        let (nodes, edges, roots, leaves) = if let Some(templates) = templates {
            let selected: Vec<&TaskTemplate> = templates.iter().take(task_count).collect();
            let mut nodes = HashMap::new();
            let mut edges = Vec::new();
            let mut prev_id: Option<String> = None;

            for (i, template) in selected.iter().enumerate() {
                let id = format!("task_{}", i);
                let deps = if let Some(ref prev) = prev_id {
                    vec![prev.clone()]
                } else {
                    vec![]
                };

                let node = TaskNode {
                    id: id.clone(),
                    name: template.name.clone(),
                    description: template.description.clone(),
                    dependencies: deps,
                    estimated_duration: template.base_duration,
                    status: TaskStatus::Pending,
                    required_skills: template.skills.clone(),
                };

                if let Some(ref prev) = prev_id {
                    edges.push((prev.clone(), id.clone()));
                }

                nodes.insert(id.clone(), node);
                prev_id = Some(id);
            }

            let roots = if selected.is_empty() {
                vec![]
            } else {
                vec!["task_0".to_string()]
            };

            let leaves = if let Some(ref prev) = prev_id {
                vec![prev.clone()]
            } else {
                vec![]
            };

            (nodes, edges, roots, leaves)
        } else {
            // Unknown intent - create a single analysis task
            let node = TaskNode {
                id: "task_0".into(),
                name: "意图分析".into(),
                description: "分析用户意图，确定后续步骤".into(),
                dependencies: vec![],
                estimated_duration: 60,
                status: TaskStatus::Pending,
                required_skills: vec!["analysis".into()],
            };
            let mut nodes = HashMap::new();
            nodes.insert("task_0".into(), node);
            (nodes, vec![], vec!["task_0".into()], vec!["task_0".into()])
        };

        let total_duration: u64 = nodes.values().map(|n| n.estimated_duration).sum();
        let task_count = nodes.len();
        let requires_multi_agent = task_count > 2;

        let task_graph = TaskGraph {
            nodes,
            edges,
            roots,
            leaves,
        };

        DecompositionResult {
            intent: intent.clone(),
            task_graph,
            total_estimated_duration: total_duration,
            task_count,
            requires_multi_agent,
        }
    }

    /// 拓扑排序获取执行顺序
    pub fn topological_sort(graph: &TaskGraph) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node_id in graph.nodes.keys() {
            in_degree.entry(node_id.clone()).or_insert(0);
        }
        for (_, to) in &graph.edges {
            if let Some(deg) = in_degree.get_mut(to) {
                *deg += 1;
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut sorted = Vec::new();
        while let Some(current) = queue.pop_front() {
            sorted.push(current.clone());
            for (from, to) in &graph.edges {
                if *from == current {
                    if let Some(deg) = in_degree.get_mut(to) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(to.clone());
                        }
                    }
                }
            }
        }

        if sorted.len() == graph.nodes.len() {
            Ok(sorted)
        } else {
            Err("Cycle detected in task graph".into())
        }
    }

    /// 获取就绪任务（依赖已完成）
    pub fn get_ready_tasks(graph: &TaskGraph) -> Vec<String> {
        graph
            .nodes
            .values()
            .filter(|node| {
                node.status == TaskStatus::Pending
                    && node.dependencies.iter().all(|dep| {
                        graph
                            .nodes
                            .get(dep)
                            .map(|n| n.status == TaskStatus::Completed)
                            .unwrap_or(false)
                    })
            })
            .map(|node| node.id.clone())
            .collect()
    }
}

impl Default for TaskDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_recognizer::Priority;

    fn create_test_intent(intent_type: IntentType, complexity: Complexity) -> RecognizedIntent {
        RecognizedIntent {
            intent_type,
            complexity,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "test input".into(),
            confidence: 0.8,
        }
    }

    #[test]
    fn test_decompose_code_generation() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(result.task_count > 0);
        assert!(result.total_estimated_duration > 0);
    }

    #[test]
    fn test_decompose_simple() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert_eq!(result.task_count, 1);
    }

    #[test]
    fn test_decompose_complex() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        assert!(result.task_count >= 3);
        assert!(result.requires_multi_agent);
    }

    #[test]
    fn test_topological_sort() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        let sorted = TaskDecomposer::topological_sort(&result.task_graph);
        assert!(sorted.is_ok());
        let sorted = sorted.unwrap();
        assert_eq!(sorted.len(), result.task_count);
    }

    #[test]
    fn test_get_ready_tasks() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        let ready = TaskDecomposer::get_ready_tasks(&result.task_graph);
        assert!(!ready.is_empty());
        // First task should be ready (no dependencies)
        assert!(ready.contains(&"task_0".to_string()));
    }

    #[test]
    fn test_task_dependencies() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        // Second task should depend on first
        let task_1 = result.task_graph.nodes.get("task_1").unwrap();
        assert!(task_1.dependencies.contains(&"task_0".to_string()));
    }

    #[test]
    fn test_unknown_intent() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::Unknown, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert_eq!(result.task_count, 1);
        assert_eq!(
            result.task_graph.nodes.get("task_0").unwrap().name,
            "意图分析"
        );
    }

    #[test]
    fn test_all_intent_types() {
        let decomposer = TaskDecomposer::new();
        let types = vec![
            IntentType::CodeGeneration,
            IntentType::BugFix,
            IntentType::CodeReview,
            IntentType::TestGeneration,
            IntentType::ArchitectureDesign,
            IntentType::PerformanceOptimization,
            IntentType::SecurityAudit,
            IntentType::Documentation,
            IntentType::KnowledgeQuery,
            IntentType::SystemAdmin,
        ];
        for intent_type in types {
            let intent = create_test_intent(intent_type.clone(), Complexity::Medium);
            let result = decomposer.decompose(&intent);
            assert!(result.task_count > 0, "Failed for {:?}", intent_type);
        }
    }

    #[test]
    fn test_graph_structure() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        // Roots should have no dependencies
        for root in &result.task_graph.roots {
            let node = result.task_graph.nodes.get(root).unwrap();
            assert!(node.dependencies.is_empty());
        }
        // Edges should reference existing nodes
        for (from, to) in &result.task_graph.edges {
            assert!(result.task_graph.nodes.contains_key(from));
            assert!(result.task_graph.nodes.contains_key(to));
        }
    }

    #[test]
    fn test_skill_assignment() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::SecurityAudit, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        // Security audit tasks should have security skill
        let has_security_skill = result
            .task_graph
            .nodes
            .values()
            .any(|n| n.required_skills.contains(&"security".to_string()));
        assert!(has_security_skill);
    }

    #[test]
    fn test_decomposition_result_fields() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        assert_eq!(result.intent.intent_type, IntentType::CodeGeneration);
        assert!(result.total_estimated_duration > 0);
        assert!(result.task_count > 0);
    }

    #[test]
    fn test_task_status_default_pending() {
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
        assert_ne!(TaskStatus::Pending, TaskStatus::Completed);
    }

    #[test]
    fn test_topological_sort_empty_graph() {
        let graph = TaskGraph {
            nodes: HashMap::new(),
            edges: vec![],
            roots: vec![],
            leaves: vec![],
        };
        let sorted = TaskDecomposer::topological_sort(&graph);
        assert!(sorted.is_ok());
        assert!(sorted.unwrap().is_empty());
    }

    #[test]
    fn test_topological_sort_single_node() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "task_0".to_string(),
            TaskNode {
                id: "task_0".into(),
                name: "solo".into(),
                description: "single task".into(),
                dependencies: vec![],
                required_skills: vec![],
                estimated_duration: 10,
                status: TaskStatus::Pending,
            },
        );
        let graph = TaskGraph {
            nodes,
            edges: vec![],
            roots: vec!["task_0".into()],
            leaves: vec!["task_0".into()],
        };
        let sorted = TaskDecomposer::topological_sort(&graph).unwrap();
        assert_eq!(sorted, vec!["task_0"]);
    }

    #[test]
    fn test_get_ready_tasks_all_completed() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Simple);
        let mut result = decomposer.decompose(&intent);
        // Mark all tasks as completed
        for node in result.task_graph.nodes.values_mut() {
            node.status = TaskStatus::Completed;
        }
        let ready = TaskDecomposer::get_ready_tasks(&result.task_graph);
        assert!(
            ready.is_empty(),
            "No tasks should be ready if all completed"
        );
    }

    #[test]
    fn test_complexity_affects_task_count() {
        let decomposer = TaskDecomposer::new();
        let simple = decomposer.decompose(&create_test_intent(
            IntentType::CodeGeneration,
            Complexity::Simple,
        ));
        let complex = decomposer.decompose(&create_test_intent(
            IntentType::CodeGeneration,
            Complexity::Complex,
        ));
        assert!(
            complex.task_count >= simple.task_count,
            "Complex should have >= tasks than simple"
        );
    }

    #[test]
    fn test_decompose_performance_optimization() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::PerformanceOptimization, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(result.task_count > 0);
    }

    #[test]
    fn test_decompose_test_generation() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::TestGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(result.task_count > 0);
    }

    #[test]
    fn test_decompose_documentation() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::Documentation, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert!(result.task_count > 0);
    }

    #[test]
    fn test_graph_edges_are_valid() {
        let decomposer = TaskDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        for (from, to) in &result.task_graph.edges {
            assert!(
                result.task_graph.nodes.contains_key(from),
                "Edge from {} missing",
                from
            );
            assert!(
                result.task_graph.nodes.contains_key(to),
                "Edge to {} missing",
                to
            );
        }
    }
}
