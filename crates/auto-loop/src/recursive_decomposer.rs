//! Recursive Task Decomposer — 递归任务分解引擎
//!
//! 将复杂任务递归分解为可执行的原子任务。
//!
//! # 论文支撑
//! - DeepResearchAgent (SkyworkAI, 2025): 分层多Agent任务规划
//! - Graph of Thoughts (Besta et al., 2024): 图状任务分解
//! - HuggingGPT (Shen et al., 2023): LLM控制器→任务拆解→模型分配

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::intent_recognizer::{Complexity, IntentType, RecognizedIntent};
use crate::task_decomposer::{TaskGraph, TaskNode, TaskStatus};

/// 递归分解配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionConfig {
    /// 最大递归深度
    pub max_depth: usize,
    /// 最大任务数
    pub max_tasks: usize,
    /// 最小任务粒度（描述长度）
    pub min_granularity: usize,
}

impl Default for DecompositionConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_tasks: 20,
            min_granularity: 10,
        }
    }
}

/// 递归分解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecursiveDecompositionResult {
    /// 原始意图
    pub intent: RecognizedIntent,
    /// 任务图
    pub task_graph: TaskGraph,
    /// 分解层次
    pub depth: usize,
    /// 总任务数
    pub total_tasks: usize,
    /// 是否需要多Agent协作
    pub requires_multi_agent: bool,
    /// 分解路径（调试用）
    pub decomposition_path: Vec<String>,
}

/// 递归任务分解器
pub struct RecursiveDecomposer {
    /// 配置
    #[allow(dead_code)]
    config: DecompositionConfig,
    /// 任务模板库
    templates: HashMap<IntentType, Vec<RecursiveTaskTemplate>>,
}

/// 递归任务模板
#[derive(Debug, Clone)]
struct RecursiveTaskTemplate {
    name: String,
    description: String,
    skills: Vec<String>,
    base_duration: u64,
    /// 子任务模板（递归）
    sub_templates: Vec<RecursiveTaskTemplate>,
    /// 分解条件
    decompose_when: DecomposeCondition,
}

/// 分解条件
#[derive(Debug, Clone)]
enum DecomposeCondition {
    /// 总是分解
    #[allow(dead_code)]
    Always,
    /// 按复杂度分解
    ByComplexity(Complexity),
    /// 按描述长度分解
    #[allow(dead_code)]
    ByDescriptionLength(usize),
    /// 不分解（原子任务）
    Never,
}

impl RecursiveDecomposer {
    /// 创建新的递归分解器
    pub fn new() -> Self {
        Self {
            config: DecompositionConfig::default(),
            templates: Self::default_templates(),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(config: DecompositionConfig) -> Self {
        Self {
            config,
            templates: Self::default_templates(),
        }
    }

    /// 默认递归模板
    fn default_templates() -> HashMap<IntentType, Vec<RecursiveTaskTemplate>> {
        let mut templates = HashMap::new();

        // 代码生成模板（递归）
        templates.insert(
            IntentType::CodeGeneration,
            vec![
                RecursiveTaskTemplate {
                    name: "需求分析".into(),
                    description: "分析代码需求，确定接口和数据结构".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                    sub_templates: vec![
                        RecursiveTaskTemplate {
                            name: "接口设计".into(),
                            description: "定义函数签名和数据结构".into(),
                            skills: vec!["design".into()],
                            base_duration: 30,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                        RecursiveTaskTemplate {
                            name: "数据模型".into(),
                            description: "设计数据模型和关系".into(),
                            skills: vec!["design".into()],
                            base_duration: 30,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                    ],
                    decompose_when: DecomposeCondition::ByComplexity(Complexity::Complex),
                },
                RecursiveTaskTemplate {
                    name: "代码实现".into(),
                    description: "编写核心代码实现".into(),
                    skills: vec!["coding".into()],
                    base_duration: 180,
                    sub_templates: vec![
                        RecursiveTaskTemplate {
                            name: "核心逻辑".into(),
                            description: "实现核心业务逻辑".into(),
                            skills: vec!["coding".into()],
                            base_duration: 120,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                        RecursiveTaskTemplate {
                            name: "错误处理".into(),
                            description: "实现错误处理和边界条件".into(),
                            skills: vec!["coding".into()],
                            base_duration: 60,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                    ],
                    decompose_when: DecomposeCondition::ByComplexity(Complexity::Medium),
                },
                RecursiveTaskTemplate {
                    name: "单元测试".into(),
                    description: "编写单元测试验证功能".into(),
                    skills: vec!["testing".into()],
                    base_duration: 120,
                    sub_templates: vec![],
                    decompose_when: DecomposeCondition::Never,
                },
                RecursiveTaskTemplate {
                    name: "代码审查".into(),
                    description: "审查代码质量和规范".into(),
                    skills: vec!["review".into()],
                    base_duration: 60,
                    sub_templates: vec![],
                    decompose_when: DecomposeCondition::Never,
                },
            ],
        );

        // Bug修复模板（递归）
        templates.insert(
            IntentType::BugFix,
            vec![
                RecursiveTaskTemplate {
                    name: "问题定位".into(),
                    description: "分析错误日志，定位问题根因".into(),
                    skills: vec!["debugging".into()],
                    base_duration: 90,
                    sub_templates: vec![
                        RecursiveTaskTemplate {
                            name: "日志分析".into(),
                            description: "分析错误日志和堆栈跟踪".into(),
                            skills: vec!["debugging".into()],
                            base_duration: 30,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                        RecursiveTaskTemplate {
                            name: "根因分析".into(),
                            description: "确定问题根本原因".into(),
                            skills: vec!["analysis".into()],
                            base_duration: 60,
                            sub_templates: vec![],
                            decompose_when: DecomposeCondition::Never,
                        },
                    ],
                    decompose_when: DecomposeCondition::ByComplexity(Complexity::Medium),
                },
                RecursiveTaskTemplate {
                    name: "修复方案".into(),
                    description: "设计修复方案".into(),
                    skills: vec!["analysis".into()],
                    base_duration: 60,
                    sub_templates: vec![],
                    decompose_when: DecomposeCondition::Never,
                },
                RecursiveTaskTemplate {
                    name: "代码修复".into(),
                    description: "实现修复代码".into(),
                    skills: vec!["coding".into()],
                    base_duration: 120,
                    sub_templates: vec![],
                    decompose_when: DecomposeCondition::Never,
                },
                RecursiveTaskTemplate {
                    name: "回归测试".into(),
                    description: "验证修复并运行回归测试".into(),
                    skills: vec!["testing".into()],
                    base_duration: 90,
                    sub_templates: vec![],
                    decompose_when: DecomposeCondition::Never,
                },
            ],
        );

        templates
    }

    /// 递归分解意图
    pub fn decompose(&self, intent: &RecognizedIntent) -> RecursiveDecompositionResult {
        let mut task_graph = TaskGraph {
            nodes: HashMap::new(),
            edges: vec![],
            roots: vec![],
            leaves: vec![],
        };
        let mut decomposition_path = vec![];
        let mut task_counter = 0;

        if let Some(templates) = self.templates.get(&intent.intent_type) {
            let mut prev_id: Option<String> = None;

            for template in templates {
                let task_id = format!("task_{}", task_counter);
                task_counter += 1;

                // 检查是否需要递归分解
                let should_decompose = self.should_decompose(template, intent);

                if should_decompose && !template.sub_templates.is_empty() {
                    // 递归分解
                    decomposition_path.push(format!("{}: decompose", template.name));

                    let mut sub_prev_id: Option<String> = None;
                    for sub_template in &template.sub_templates {
                        let sub_id = format!("task_{}", task_counter);
                        task_counter += 1;

                        let sub_deps = if let Some(ref prev) = sub_prev_id {
                            vec![prev.clone()]
                        } else if let Some(ref prev) = prev_id {
                            vec![prev.clone()]
                        } else {
                            vec![]
                        };

                        let sub_node = TaskNode {
                            id: sub_id.clone(),
                            name: sub_template.name.clone(),
                            description: sub_template.description.clone(),
                            dependencies: sub_deps,
                            estimated_duration: sub_template.base_duration,
                            status: TaskStatus::Pending,
                            required_skills: sub_template.skills.clone(),
                        };

                        if let Some(ref prev) = sub_prev_id {
                            task_graph.edges.push((prev.clone(), sub_id.clone()));
                        } else if let Some(ref prev) = prev_id {
                            task_graph.edges.push((prev.clone(), sub_id.clone()));
                        }

                        task_graph.nodes.insert(sub_id.clone(), sub_node);
                        sub_prev_id = Some(sub_id);
                    }

                    if let Some(last_sub) = sub_prev_id {
                        prev_id = Some(last_sub);
                    }
                } else {
                    // 原子任务
                    decomposition_path.push(format!("{}: atomic", template.name));

                    let deps = if let Some(ref prev) = prev_id {
                        vec![prev.clone()]
                    } else {
                        vec![]
                    };

                    let node = TaskNode {
                        id: task_id.clone(),
                        name: template.name.clone(),
                        description: template.description.clone(),
                        dependencies: deps,
                        estimated_duration: template.base_duration,
                        status: TaskStatus::Pending,
                        required_skills: template.skills.clone(),
                    };

                    if let Some(ref prev) = prev_id {
                        task_graph.edges.push((prev.clone(), task_id.clone()));
                    }

                    task_graph.nodes.insert(task_id.clone(), node);
                    prev_id = Some(task_id);
                }
            }

            // 设置 roots 和 leaves
            task_graph.roots = task_graph
                .nodes
                .values()
                .filter(|n| n.dependencies.is_empty())
                .map(|n| n.id.clone())
                .collect();

            task_graph.leaves = task_graph
                .nodes
                .keys()
                .filter(|id| !task_graph.edges.iter().any(|(from, _)| from == *id))
                .cloned()
                .collect();
        }

        let total_tasks = task_graph.nodes.len();
        let depth = decomposition_path.len();

        RecursiveDecompositionResult {
            intent: intent.clone(),
            task_graph,
            depth,
            total_tasks,
            requires_multi_agent: total_tasks > 2,
            decomposition_path,
        }
    }

    /// 检查是否应该分解
    fn should_decompose(
        &self,
        template: &RecursiveTaskTemplate,
        intent: &RecognizedIntent,
    ) -> bool {
        match &template.decompose_when {
            DecomposeCondition::Always => true,
            DecomposeCondition::ByComplexity(required) => {
                matches!(
                    (required, &intent.complexity),
                    (Complexity::Simple, _)
                        | (Complexity::Medium, Complexity::Medium)
                        | (Complexity::Medium, Complexity::Complex)
                        | (Complexity::Complex, Complexity::Complex)
                )
            }
            DecomposeCondition::ByDescriptionLength(min_len) => intent.raw_input.len() >= *min_len,
            DecomposeCondition::Never => false,
        }
    }
}

impl Default for RecursiveDecomposer {
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
            raw_input: "test input for decomposition".into(),
            confidence: 0.8,
        }
    }

    #[test]
    fn test_recursive_decompose_simple() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert!(result.total_tasks > 0);
        assert!(result.depth > 0);
    }

    #[test]
    fn test_recursive_decompose_complex() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        // Complex should trigger recursive decomposition
        assert!(result.total_tasks > 4);
        assert!(result
            .decomposition_path
            .iter()
            .any(|p| p.contains("decompose")));
    }

    #[test]
    fn test_recursive_decompose_bugfix() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(result.total_tasks > 0);
    }

    #[test]
    fn test_decomposition_path() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        assert!(!result.decomposition_path.is_empty());
    }

    #[test]
    fn test_multi_agent_detection() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        assert!(result.requires_multi_agent);
    }

    #[test]
    fn test_custom_config() {
        let config = DecompositionConfig {
            max_depth: 5,
            max_tasks: 50,
            min_granularity: 5,
        };
        let decomposer = RecursiveDecomposer::with_config(config);
        assert_eq!(decomposer.config.max_depth, 5);
    }

    #[test]
    fn test_unknown_intent() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::Unknown, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert_eq!(result.total_tasks, 0);
    }

    #[test]
    fn test_graph_structure() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        // All edges should reference existing nodes
        for (from, to) in &result.task_graph.edges {
            assert!(result.task_graph.nodes.contains_key(from));
            assert!(result.task_graph.nodes.contains_key(to));
        }
    }

    #[test]
    fn test_skill_assignment() {
        let decomposer = RecursiveDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        // Should have coding skill
        let has_coding = result
            .task_graph
            .nodes
            .values()
            .any(|n| n.required_skills.contains(&"coding".to_string()));
        assert!(has_coding);
    }

    #[test]
    fn test_decompose_condition() {
        let decomposer = RecursiveDecomposer::new();
        let template = RecursiveTaskTemplate {
            name: "test".into(),
            description: "test".into(),
            skills: vec![],
            base_duration: 60,
            sub_templates: vec![],
            decompose_when: DecomposeCondition::ByComplexity(Complexity::Complex),
        };
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Simple);
        assert!(!decomposer.should_decompose(&template, &intent));
    }
}
