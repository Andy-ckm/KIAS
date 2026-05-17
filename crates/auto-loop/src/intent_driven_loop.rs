//! Intent-Driven Loop — 意图驱动的自动循环
//!
//! 将 IntentRecognizer + TaskDecomposer 集成到 auto-loop 执行链路。
//!
//! # 核心流程
//! 用户输入 → 意图识别 → 任务拆解 → 选择编排模式 → 执行任务图
//!
//! # 参考来源
//! - Claude Multi-Agent Design: 五种编排模式
//! - DeepResearchAgent: 分层任务规划
//! - KIAS auto-loop: detector → analyzer → planner → codegen → verifier

use serde::{Deserialize, Serialize};

use crate::context_aware_decomposer::{
    ContextAwareDecomposer, ContextAwareDecomposition, OrchestrationPattern,
};
use crate::intent_recognizer::{Complexity, IntentRecognizer, IntentType, RecognizedIntent};
use crate::task_decomposer::{DecompositionResult, TaskDecomposer};

/// 意图驱动循环配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentDrivenConfig {
    /// 是否启用 LLM 意图识别
    pub enable_llm_intent: bool,
    /// 是否启用上下文感知分解
    pub enable_context_aware: bool,
    /// 最大并行任务数
    pub max_parallel_tasks: usize,
    /// 任务超时（秒）
    pub task_timeout: u64,
}

impl Default for IntentDrivenConfig {
    fn default() -> Self {
        Self {
            enable_llm_intent: false,
            enable_context_aware: true,
            max_parallel_tasks: 5,
            task_timeout: 300,
        }
    }
}

/// 意图驱动循环
pub struct IntentDrivenLoop {
    /// 配置
    config: IntentDrivenConfig,
    /// 意图识别器
    intent_recognizer: IntentRecognizer,
    /// 任务分解器
    task_decomposer: TaskDecomposer,
    /// 上下文感知分解器
    context_decomposer: ContextAwareDecomposer,
}

/// 循环执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopResult {
    /// 识别的意图
    pub intent: RecognizedIntent,
    /// 任务分解结果
    pub decomposition: DecompositionResult,
    /// 上下文感知分解结果（如果启用）
    pub context_decomposition: Option<ContextAwareDecomposition>,
    /// 执行状态
    pub status: LoopStatus,
    /// 执行日志
    pub logs: Vec<String>,
}

/// 循环状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopStatus {
    /// 已创建
    Created,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 等待人工确认
    WaitingForConfirmation,
}

impl IntentDrivenLoop {
    /// 创建新的意图驱动循环
    pub fn new() -> Self {
        Self {
            config: IntentDrivenConfig::default(),
            intent_recognizer: IntentRecognizer::new(),
            task_decomposer: TaskDecomposer::new(),
            context_decomposer: ContextAwareDecomposer::new(),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(config: IntentDrivenConfig) -> Self {
        Self {
            config,
            intent_recognizer: IntentRecognizer::new(),
            task_decomposer: TaskDecomposer::new(),
            context_decomposer: ContextAwareDecomposer::new(),
        }
    }

    /// 执行意图驱动循环
    pub fn execute(&self, input: &str) -> LoopResult {
        let mut logs = Vec::new();
        logs.push(format!("开始处理: {}", input));

        // Step 1: 意图识别
        let intent = self.intent_recognognize(input);
        logs.push(format!(
            "识别意图: {:?} (置信度: {:.2})",
            intent.intent_type, intent.confidence
        ));

        // Step 2: 任务拆解
        let decomposition = self.task_decomposer.decompose(&intent);
        logs.push(format!(
            "任务拆解: {} 个任务, 预估 {} 秒",
            decomposition.task_count, decomposition.total_estimated_duration
        ));

        // Step 3: 上下文感知分解（如果启用）
        let context_decomposition = if self.config.enable_context_aware {
            let ctx_result = self.context_decomposer.decompose(&intent);
            logs.push(format!(
                "上下文分解: {:?} 编排模式",
                ctx_result.orchestration_pattern
            ));
            Some(ctx_result)
        } else {
            None
        };

        // Step 4: 选择执行策略
        let status =
            self.select_execution_strategy(&intent, &decomposition, &context_decomposition);
        logs.push(format!("执行策略: {:?}", status));

        LoopResult {
            intent,
            decomposition,
            context_decomposition,
            status,
            logs,
        }
    }

    /// 意图识别
    fn intent_recognognize(&self, input: &str) -> RecognizedIntent {
        self.intent_recognizer.recognize(input)
    }

    /// 选择执行策略
    fn select_execution_strategy(
        &self,
        intent: &RecognizedIntent,
        decomposition: &DecompositionResult,
        _context_decomposition: &Option<ContextAwareDecomposition>,
    ) -> LoopStatus {
        // 根据 Claude 文章的选型标准
        let is_complex = matches!(intent.complexity, Complexity::Complex);
        let has_many_tasks = decomposition.task_count > 3;
        let needs_confirmation = matches!(
            intent.intent_type,
            IntentType::SecurityAudit | IntentType::ArchitectureDesign
        );

        if needs_confirmation {
            // 安全审计和架构设计需要人工确认
            LoopStatus::WaitingForConfirmation
        } else if is_complex || has_many_tasks {
            // 复杂任务需要多 Agent 协作
            LoopStatus::Running
        } else {
            // 简单任务直接执行
            LoopStatus::Running
        }
    }

    /// 获取编排模式
    pub fn get_orchestration_pattern(&self, input: &str) -> OrchestrationPattern {
        let intent = self.intent_recognizer.recognize(input);
        let ctx_result = self.context_decomposer.decompose(&intent);
        ctx_result.orchestration_pattern
    }

    /// 判断是否需要多 Agent
    pub fn needs_multi_agent(&self, input: &str) -> bool {
        let intent = self.intent_recognizer.recognize(input);
        let decomposition = self.task_decomposer.decompose(&intent);
        decomposition.requires_multi_agent
    }
}

impl Default for IntentDrivenLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_driven_loop_new() {
        let loop_inst = IntentDrivenLoop::new();
        assert!(!loop_inst.config.enable_llm_intent);
        assert!(loop_inst.config.enable_context_aware);
    }

    #[test]
    fn test_execute_code_generation() {
        let loop_inst = IntentDrivenLoop::new();
        let result = loop_inst.execute("请帮我实现一个 Rust 的 HTTP 服务器");
        assert_eq!(result.intent.intent_type, IntentType::CodeGeneration);
        assert!(!result.decomposition.task_graph.nodes.is_empty());
        assert!(result.context_decomposition.is_some());
    }

    #[test]
    fn test_execute_bug_fix() {
        let loop_inst = IntentDrivenLoop::new();
        let result = loop_inst.execute("修复这个空指针异常");
        assert_eq!(result.intent.intent_type, IntentType::BugFix);
        assert!(result.status == LoopStatus::Running);
    }

    #[test]
    fn test_execute_security_audit() {
        let loop_inst = IntentDrivenLoop::new();
        let result = loop_inst.execute("对系统进行安全审计");
        assert_eq!(result.intent.intent_type, IntentType::SecurityAudit);
        assert_eq!(result.status, LoopStatus::WaitingForConfirmation);
    }

    #[test]
    fn test_orchestration_pattern() {
        let loop_inst = IntentDrivenLoop::new();
        let pattern = loop_inst.get_orchestration_pattern("实现一个复杂系统");
        // Should be OrchestratorWorker for complex tasks
        assert!(
            pattern == OrchestrationPattern::OrchestratorWorker
                || pattern == OrchestrationPattern::PromptChaining
        );
    }

    #[test]
    fn test_multi_agent_detection() {
        let loop_inst = IntentDrivenLoop::new();
        // 需要包含"然后"等关键词才能触发复杂度评估
        let needs =
            loop_inst.needs_multi_agent("先实现用户认证，然后添加权限管理，之后集成到主系统");
        assert!(needs);
    }

    #[test]
    fn test_simple_task_no_multi_agent() {
        let loop_inst = IntentDrivenLoop::new();
        let needs = loop_inst.needs_multi_agent("写一个函数");
        assert!(!needs);
    }

    #[test]
    fn test_custom_config() {
        let config = IntentDrivenConfig {
            enable_llm_intent: true,
            enable_context_aware: false,
            max_parallel_tasks: 10,
            task_timeout: 600,
        };
        let loop_inst = IntentDrivenLoop::with_config(config);
        assert!(loop_inst.config.enable_llm_intent);
        assert!(!loop_inst.config.enable_context_aware);
    }

    #[test]
    fn test_execution_logs() {
        let loop_inst = IntentDrivenLoop::new();
        let result = loop_inst.execute("写代码");
        assert!(!result.logs.is_empty());
        assert!(result.logs.iter().any(|l| l.contains("开始处理")));
    }

    #[test]
    fn test_loop_status() {
        let loop_inst = IntentDrivenLoop::new();
        let result = loop_inst.execute("写代码");
        assert!(
            result.status == LoopStatus::Running
                || result.status == LoopStatus::WaitingForConfirmation
        );
    }
}
