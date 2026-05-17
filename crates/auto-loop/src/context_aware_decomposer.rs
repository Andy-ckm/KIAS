//! Context-Aware Decomposer — 上下文感知的任务分解
//!
//! 基于 Claude 多智能体设计文章的核心洞察：
//! "围绕上下文边界设计，而不是围绕角色分工设计"
//!
//! # 核心原则
//! 1. 按上下文拆分，不按角色拆分
//! 2. 如果两个子任务依赖的信息高度重叠，它们应该由同一个 agent 完成
//! 3. 只有在上下文确实可以隔离时，才值得拆分
//!
//! # 参考来源
//! - Claude Multi-Agent Design (2026): 子代理 vs 团队型智能体
//! - DeepResearchAgent: 分层任务规划
//! - Graph of Thoughts: 图状任务分解

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::intent_recognizer::{Complexity, IntentType, RecognizedIntent};
use crate::task_decomposer::{TaskGraph, TaskNode, TaskStatus};

/// 上下文边界
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBoundary {
    /// 上下文 ID
    pub id: String,
    /// 上下文描述
    pub description: String,
    /// 所需知识
    pub required_knowledge: Vec<String>,
    /// 所需工具
    pub required_tools: Vec<String>,
    /// 与其他上下文的重叠度 (0.0-1.0)
    pub overlap_scores: HashMap<String, f64>,
}

/// 上下文感知的任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAwareTask {
    /// 任务 ID
    pub id: String,
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 所属上下文边界
    pub context_id: String,
    /// 所需知识
    pub required_knowledge: Vec<String>,
    /// 所需工具
    pub required_tools: Vec<String>,
    /// 预估耗时（秒）
    pub estimated_duration: u64,
    /// 依赖的任务
    pub dependencies: Vec<String>,
    /// 是否可以并行
    pub parallelizable: bool,
}

/// 上下文感知的分解结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAwareDecomposition {
    /// 原始意图
    pub intent: RecognizedIntent,
    /// 上下文边界
    pub contexts: Vec<ContextBoundary>,
    /// 任务列表
    pub tasks: Vec<ContextAwareTask>,
    /// 任务图
    pub task_graph: TaskGraph,
    /// 编排模式
    pub orchestration_pattern: OrchestrationPattern,
    /// 是否需要多 Agent
    pub requires_multi_agent: bool,
    /// 分解理由
    pub decomposition_rationale: String,
}

/// 编排模式（来自文章的五种模式）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrchestrationPattern {
    /// 提示链：按顺序执行，后一步处理前一步的输出
    PromptChaining,
    /// 路由：先分类，再决定交给哪个处理器
    Routing,
    /// 并行：独立子任务同时运行
    Parallelization,
    /// 编排者-工人：中心 agent 负责拆任务、分发、整合
    OrchestratorWorker,
    /// 评估者-优化者：一个生成，一个评估，循环迭代
    EvaluatorOptimizer,
}

/// 上下文感知的分解器
pub struct ContextAwareDecomposer {
    /// 上下文重叠阈值（超过此值则合并）
    #[allow(dead_code)]
    overlap_threshold: f64,
}

impl ContextAwareDecomposer {
    /// 创建新的分解器
    pub fn new() -> Self {
        Self {
            overlap_threshold: 0.7, // 70% 重叠则合并
        }
    }

    /// 使用自定义阈值创建
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            overlap_threshold: threshold,
        }
    }

    /// 分解意图
    pub fn decompose(&self, intent: &RecognizedIntent) -> ContextAwareDecomposition {
        // 1. 识别上下文边界
        let contexts = self.identify_contexts(intent);

        // 2. 生成任务
        let raw_tasks = self.generate_tasks(intent, &contexts);

        // 3. 合并高重叠任务
        let merged_tasks = self.merge_overlapping_tasks(raw_tasks);

        // 4. 选择编排模式
        let pattern = self.select_orchestration_pattern(intent, &merged_tasks);

        // 5. 构建任务图
        let task_graph = self.build_task_graph(&merged_tasks);

        // 6. 判断是否需要多 Agent
        let requires_multi_agent = self.should_use_multi_agent(intent, &merged_tasks);

        // 7. 生成分解理由
        let rationale = self.generate_rationale(intent, &merged_tasks, &pattern);

        ContextAwareDecomposition {
            intent: intent.clone(),
            contexts,
            tasks: merged_tasks,
            task_graph,
            orchestration_pattern: pattern,
            requires_multi_agent,
            decomposition_rationale: rationale,
        }
    }

    /// 识别上下文边界
    fn identify_contexts(&self, intent: &RecognizedIntent) -> Vec<ContextBoundary> {
        let mut contexts = Vec::new();

        match intent.intent_type {
            IntentType::CodeGeneration => {
                contexts.push(ContextBoundary {
                    id: "code_context".into(),
                    description: "代码实现上下文".into(),
                    required_knowledge: vec![
                        "编程语言".into(),
                        "设计模式".into(),
                        "API设计".into(),
                    ],
                    required_tools: vec!["codegen".into(), "reviewer".into()],
                    overlap_scores: HashMap::new(),
                });
                contexts.push(ContextBoundary {
                    id: "test_context".into(),
                    description: "测试上下文".into(),
                    required_knowledge: vec!["测试框架".into(), "边界条件".into()],
                    required_tools: vec!["testgen".into()],
                    overlap_scores: HashMap::new(),
                });
            }
            IntentType::BugFix => {
                contexts.push(ContextBoundary {
                    id: "debug_context".into(),
                    description: "调试上下文".into(),
                    required_knowledge: vec!["错误日志".into(), "代码结构".into()],
                    required_tools: vec!["debugger".into()],
                    overlap_scores: HashMap::new(),
                });
            }
            IntentType::SecurityAudit => {
                contexts.push(ContextBoundary {
                    id: "security_context".into(),
                    description: "安全审计上下文".into(),
                    required_knowledge: vec!["漏洞模式".into(), "安全最佳实践".into()],
                    required_tools: vec!["security_scanner".into()],
                    overlap_scores: HashMap::new(),
                });
            }
            _ => {
                contexts.push(ContextBoundary {
                    id: "general_context".into(),
                    description: "通用上下文".into(),
                    required_knowledge: vec![],
                    required_tools: vec![],
                    overlap_scores: HashMap::new(),
                });
            }
        }

        contexts
    }

    /// 生成任务
    fn generate_tasks(
        &self,
        intent: &RecognizedIntent,
        _contexts: &[ContextBoundary],
    ) -> Vec<ContextAwareTask> {
        let mut tasks = Vec::new();
        let mut task_counter = 0;

        match intent.intent_type {
            IntentType::CodeGeneration => {
                // 需求分析
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "需求分析".into(),
                    description: "分析代码需求，确定接口和数据结构".into(),
                    context_id: "code_context".into(),
                    required_knowledge: vec!["编程语言".into()],
                    required_tools: vec![],
                    estimated_duration: 60,
                    dependencies: vec![],
                    parallelizable: false,
                });
                task_counter += 1;

                // 代码实现
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "代码实现".into(),
                    description: "编写核心代码实现".into(),
                    context_id: "code_context".into(),
                    required_knowledge: vec!["编程语言".into(), "设计模式".into()],
                    required_tools: vec!["codegen".into()],
                    estimated_duration: 180,
                    dependencies: vec![format!("task_{}", task_counter - 1)],
                    parallelizable: false,
                });
                task_counter += 1;

                // 代码实现 + 测试（合并，因为上下文高度重叠）
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "代码实现+测试".into(),
                    description: "编写代码和对应测试（上下文重叠，合并执行）".into(),
                    context_id: "code_context".into(),
                    required_knowledge: vec!["编程语言".into(), "测试框架".into()],
                    required_tools: vec!["codegen".into(), "testgen".into()],
                    estimated_duration: 240,
                    dependencies: vec![format!("task_{}", task_counter - 1)],
                    parallelizable: false,
                });
                task_counter += 1;

                // 代码审查
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "代码审查".into(),
                    description: "审查代码质量和规范".into(),
                    context_id: "code_context".into(),
                    required_knowledge: vec!["代码规范".into()],
                    required_tools: vec!["reviewer".into()],
                    estimated_duration: 60,
                    dependencies: vec![format!("task_{}", task_counter - 1)],
                    parallelizable: false,
                });
            }
            IntentType::BugFix => {
                // 问题定位
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "问题定位".into(),
                    description: "分析错误日志，定位问题根因".into(),
                    context_id: "debug_context".into(),
                    required_knowledge: vec!["错误日志".into()],
                    required_tools: vec!["debugger".into()],
                    estimated_duration: 90,
                    dependencies: vec![],
                    parallelizable: false,
                });
                task_counter += 1;

                // 修复+测试（合并）
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "修复+测试".into(),
                    description: "实现修复并验证（上下文重叠，合并执行）".into(),
                    context_id: "debug_context".into(),
                    required_knowledge: vec!["代码结构".into(), "测试框架".into()],
                    required_tools: vec!["codegen".into(), "testgen".into()],
                    estimated_duration: 180,
                    dependencies: vec![format!("task_{}", task_counter - 1)],
                    parallelizable: false,
                });
            }
            _ => {
                // 通用任务
                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "分析".into(),
                    description: "分析任务需求".into(),
                    context_id: "general_context".into(),
                    required_knowledge: vec![],
                    required_tools: vec![],
                    estimated_duration: 60,
                    dependencies: vec![],
                    parallelizable: false,
                });
                task_counter += 1;

                tasks.push(ContextAwareTask {
                    id: format!("task_{}", task_counter),
                    name: "执行".into(),
                    description: "执行任务".into(),
                    context_id: "general_context".into(),
                    required_knowledge: vec![],
                    required_tools: vec![],
                    estimated_duration: 120,
                    dependencies: vec![format!("task_{}", task_counter - 1)],
                    parallelizable: false,
                });
            }
        }

        tasks
    }

    /// 合并高重叠任务
    fn merge_overlapping_tasks(&self, tasks: Vec<ContextAwareTask>) -> Vec<ContextAwareTask> {
        // 简化实现：直接返回，实际应该根据 overlap_scores 合并
        tasks
    }

    /// 选择编排模式
    fn select_orchestration_pattern(
        &self,
        intent: &RecognizedIntent,
        tasks: &[ContextAwareTask],
    ) -> OrchestrationPattern {
        // 根据文章的选型标准
        let has_dependencies = tasks.iter().any(|t| !t.dependencies.is_empty());
        let has_parallel = tasks.iter().any(|t| t.parallelizable);
        let is_complex = matches!(intent.complexity, Complexity::Complex);

        if has_parallel && !has_dependencies {
            // 独立子任务，并行执行
            OrchestrationPattern::Parallelization
        } else if has_dependencies && is_complex {
            // 复杂依赖，需要编排者
            OrchestrationPattern::OrchestratorWorker
        } else if has_dependencies {
            // 线性依赖，提示链
            OrchestrationPattern::PromptChaining
        } else {
            // 简单任务，路由
            OrchestrationPattern::Routing
        }
    }

    /// 构建任务图
    fn build_task_graph(&self, tasks: &[ContextAwareTask]) -> TaskGraph {
        let mut nodes = HashMap::new();
        let mut edges = Vec::new();

        for task in tasks {
            let node = TaskNode {
                id: task.id.clone(),
                name: task.name.clone(),
                description: task.description.clone(),
                dependencies: task.dependencies.clone(),
                estimated_duration: task.estimated_duration,
                status: TaskStatus::Pending,
                required_skills: task.required_tools.clone(),
            };

            for dep in &task.dependencies {
                edges.push((dep.clone(), task.id.clone()));
            }

            nodes.insert(task.id.clone(), node);
        }

        let roots = nodes
            .values()
            .filter(|n| n.dependencies.is_empty())
            .map(|n| n.id.clone())
            .collect();

        let leaves = nodes
            .keys()
            .filter(|id| !edges.iter().any(|(from, _)| from == *id))
            .cloned()
            .collect();

        TaskGraph {
            nodes,
            edges,
            roots,
            leaves,
        }
    }

    /// 判断是否需要多 Agent
    fn should_use_multi_agent(
        &self,
        intent: &RecognizedIntent,
        tasks: &[ContextAwareTask],
    ) -> bool {
        // 文章的核心洞察：
        // 1. 任务属于"尴尬式并行"→ 用子代理
        // 2. 任务需要"持续协商"→ 用团队
        // 3. 简单任务 → 不用多 Agent

        let has_parallel = tasks.iter().any(|t| t.parallelizable);
        let is_complex = matches!(intent.complexity, Complexity::Complex);
        let task_count = tasks.len();

        // 简单任务不需要多 Agent
        if task_count <= 1 {
            return false;
        }

        // 复杂任务需要多 Agent
        if is_complex {
            return true;
        }

        // 有并行任务需要多 Agent
        if has_parallel {
            return true;
        }

        // 默认不需要
        false
    }

    /// 生成分解理由
    fn generate_rationale(
        &self,
        intent: &RecognizedIntent,
        tasks: &[ContextAwareTask],
        pattern: &OrchestrationPattern,
    ) -> String {
        let mut reasons = Vec::new();

        reasons.push(format!("意图类型: {:?}", intent.intent_type));
        reasons.push(format!("复杂度: {:?}", intent.complexity));
        reasons.push(format!("任务数量: {}", tasks.len()));
        reasons.push(format!("编排模式: {:?}", pattern));

        if tasks.iter().any(|t| t.parallelizable) {
            reasons.push("存在可并行任务，适合多 Agent".into());
        }

        if tasks.iter().any(|t| !t.dependencies.is_empty()) {
            reasons.push("存在任务依赖，需要编排".into());
        }

        reasons.join("; ")
    }
}

impl Default for ContextAwareDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_recognizer::{Complexity, IntentRecognizer, Priority};

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
    fn test_context_aware_decompose() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(!result.tasks.is_empty());
        assert!(!result.contexts.is_empty());
    }

    #[test]
    fn test_orchestration_pattern() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        // Complex code generation should use orchestrator-worker
        assert_eq!(
            result.orchestration_pattern,
            OrchestrationPattern::OrchestratorWorker
        );
    }

    #[test]
    fn test_multi_agent_detection() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Complex);
        let result = decomposer.decompose(&intent);
        assert!(result.requires_multi_agent);
    }

    #[test]
    fn test_simple_task_no_multi_agent() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::Unknown, Complexity::Simple);
        let result = decomposer.decompose(&intent);
        assert!(!result.requires_multi_agent);
    }

    #[test]
    fn test_context_boundaries() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(!result.contexts.is_empty());
    }

    #[test]
    fn test_task_graph_structure() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        // All edges should reference existing nodes
        for (from, to) in &result.task_graph.edges {
            assert!(result.task_graph.nodes.contains_key(from));
            assert!(result.task_graph.nodes.contains_key(to));
        }
    }

    #[test]
    fn test_decomposition_rationale() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::CodeGeneration, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(!result.decomposition_rationale.is_empty());
    }

    #[test]
    fn test_bugfix_pattern() {
        let decomposer = ContextAwareDecomposer::new();
        let intent = create_test_intent(IntentType::BugFix, Complexity::Medium);
        let result = decomposer.decompose(&intent);
        assert!(!result.tasks.is_empty());
    }

    #[test]
    fn test_custom_threshold() {
        let decomposer = ContextAwareDecomposer::with_threshold(0.5);
        assert_eq!(decomposer.overlap_threshold, 0.5);
    }
}
