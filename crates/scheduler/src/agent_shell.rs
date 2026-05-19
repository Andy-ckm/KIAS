//! Agent Shell — 模板+参数调度
//!
//! 核心概念：
//! - Shell：模板，定义 Agent 的能力和约束
//! - Params：参数，填充 Shell 的具体值
//! - Intent：意图，用户的需求
//! - Scheduler：调度器，根据 Intent 选择 Shell + Params
//!
//! 参考：K8S Pod 调度 + Dify Agent 工作流

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent Shell 模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentShell {
    /// Shell ID
    pub id: String,
    /// Shell 名称
    pub name: String,
    /// Shell 描述
    pub description: String,
    /// 能力列表
    pub capabilities: Vec<String>,
    /// 约束列表
    pub constraints: Vec<Constraint>,
    /// 参数模板
    pub param_templates: Vec<ParamTemplate>,
    /// 调度策略
    pub scheduling_strategy: SchedulingStrategy,
}

/// 约束
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// 约束名称
    pub name: String,
    /// 约束类型
    pub constraint_type: ConstraintType,
    /// 约束值
    pub value: String,
}

/// 约束类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConstraintType {
    /// 资源约束（CPU、内存、GPU）
    Resource,
    /// 时间约束（超时、截止时间）
    Time,
    /// 权限约束（文件访问、网络访问）
    Permission,
    /// 依赖约束（需要其他 Agent）
    Dependency,
}

/// 参数模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamTemplate {
    /// 参数名称
    pub name: String,
    /// 参数类型
    pub param_type: ParamType,
    /// 是否必填
    pub required: bool,
    /// 默认值
    pub default_value: Option<String>,
    /// 描述
    pub description: String,
}

/// 参数类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    /// 字符串
    String,
    /// 数字
    Number,
    /// 布尔
    Boolean,
    /// 列表
    List,
    /// 对象
    Object,
}

/// 调度策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SchedulingStrategy {
    /// 轮询
    RoundRobin,
    /// 最少负载
    LeastLoaded,
    /// 亲和性
    Affinity,
    /// 缓存感知
    CacheAware,
    /// GPU 感知
    GpuAware,
    /// 优先级
    Priority,
    /// 资源感知
    ResourceAware,
}

/// Agent 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentParams {
    /// 参数值
    pub values: HashMap<String, String>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 用户意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// 意图 ID
    pub id: String,
    /// 意图描述
    pub description: String,
    /// 意图类型
    pub intent_type: IntentType,
    /// 需求列表
    pub requirements: Vec<String>,
    /// 优先级
    pub priority: Priority,
}

/// 意图类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntentType {
    /// 代码生成
    CodeGeneration,
    /// 代码审查
    CodeReview,
    /// 测试生成
    TestGeneration,
    /// 文档生成
    Documentation,
    /// 调试
    Debugging,
    /// 重构
    Refactoring,
    /// 其他
    Other,
}

/// 优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
    /// 紧急
    Critical,
}

/// 调度结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    /// 选择的 Shell
    pub shell: AgentShell,
    /// 填充的参数
    pub params: AgentParams,
    /// 调度理由
    pub reason: String,
    /// 置信度
    pub confidence: f64,
}

/// Agent Shell 调度器
pub struct AgentShellScheduler {
    /// 可用的 Shell 列表
    shells: Vec<AgentShell>,
    /// 调度策略
    strategy: SchedulingStrategy,
}

impl AgentShellScheduler {
    /// 创建新的调度器
    pub fn new() -> Self {
        Self {
            shells: Vec::new(),
            strategy: SchedulingStrategy::LeastLoaded,
        }
    }

    /// 添加 Shell
    pub fn add_shell(&mut self, shell: AgentShell) {
        self.shells.push(shell);
    }

    /// 设置调度策略
    pub fn with_strategy(mut self, strategy: SchedulingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// 根据意图调度 Shell
    pub fn schedule(&self, intent: &Intent) -> Option<ScheduleResult> {
        // 1. 过滤出符合条件的 Shell
        let candidates: Vec<&AgentShell> = self
            .shells
            .iter()
            .filter(|shell| self.matches_intent(shell, intent))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // 2. 根据调度策略选择 Shell
        let selected = match self.strategy {
            SchedulingStrategy::RoundRobin => self.select_round_robin(&candidates),
            SchedulingStrategy::LeastLoaded => self.select_least_loaded(&candidates),
            SchedulingStrategy::Affinity => self.select_affinity(&candidates, intent),
            SchedulingStrategy::CacheAware => self.select_cache_aware(&candidates),
            SchedulingStrategy::GpuAware => self.select_gpu_aware(&candidates),
            SchedulingStrategy::Priority => self.select_priority(&candidates, intent),
            SchedulingStrategy::ResourceAware => self.select_resource_aware(&candidates),
        };

        // 3. 填充参数
        let params = self.fill_params(selected, intent);

        Some(ScheduleResult {
            shell: selected.clone(),
            params,
            reason: format!(
                "Selected shell '{}' for intent '{}'",
                selected.name, intent.description
            ),
            confidence: 0.8,
        })
    }

    /// 检查 Shell 是否匹配意图
    fn matches_intent(&self, shell: &AgentShell, intent: &Intent) -> bool {
        // 检查能力匹配
        let has_required_capability = intent
            .requirements
            .iter()
            .all(|req| shell.capabilities.contains(req));

        // 检查约束满足
        let constraints_satisfied = shell
            .constraints
            .iter()
            .all(|constraint| self.check_constraint(constraint));

        has_required_capability && constraints_satisfied
    }

    /// 检查约束
    fn check_constraint(&self, _constraint: &Constraint) -> bool {
        // 简化实现：所有约束都满足
        true
    }

    /// 轮询选择
    fn select_round_robin<'a>(&self, candidates: &[&'a AgentShell]) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 最少负载选择
    fn select_least_loaded<'a>(&self, candidates: &[&'a AgentShell]) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 亲和性选择
    fn select_affinity<'a>(
        &self,
        candidates: &[&'a AgentShell],
        _intent: &Intent,
    ) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 缓存感知选择
    fn select_cache_aware<'a>(&self, candidates: &[&'a AgentShell]) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// GPU 感知选择
    fn select_gpu_aware<'a>(&self, candidates: &[&'a AgentShell]) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 优先级选择
    fn select_priority<'a>(
        &self,
        candidates: &[&'a AgentShell],
        _intent: &Intent,
    ) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 资源感知选择
    fn select_resource_aware<'a>(&self, candidates: &[&'a AgentShell]) -> &'a AgentShell {
        // 简化实现：返回第一个
        candidates[0]
    }

    /// 填充参数
    fn fill_params(&self, shell: &AgentShell, intent: &Intent) -> AgentParams {
        let mut values = HashMap::new();

        // 根据意图填充参数
        for template in &shell.param_templates {
            if template.required {
                let value = self.get_param_value(template, intent);
                values.insert(template.name.clone(), value);
            }
        }

        AgentParams {
            values,
            metadata: HashMap::new(),
        }
    }

    /// 获取参数值
    fn get_param_value(&self, template: &ParamTemplate, _intent: &Intent) -> String {
        // 简化实现：返回默认值或空字符串
        template.default_value.clone().unwrap_or_default()
    }
}

impl Default for AgentShellScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_shell() -> AgentShell {
        AgentShell {
            id: "shell-1".to_string(),
            name: "Code Generator".to_string(),
            description: "Generates code based on requirements".to_string(),
            capabilities: vec!["code-generation".to_string()],
            constraints: vec![],
            param_templates: vec![ParamTemplate {
                name: "language".to_string(),
                param_type: ParamType::String,
                required: true,
                default_value: Some("rust".to_string()),
                description: "Programming language".to_string(),
            }],
            scheduling_strategy: SchedulingStrategy::LeastLoaded,
        }
    }

    fn create_test_intent() -> Intent {
        Intent {
            id: "intent-1".to_string(),
            description: "Generate Rust code".to_string(),
            intent_type: IntentType::CodeGeneration,
            requirements: vec!["code-generation".to_string()],
            priority: Priority::Medium,
        }
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = AgentShellScheduler::new();
        assert_eq!(scheduler.shells.len(), 0);
    }

    #[test]
    fn test_add_shell() {
        let mut scheduler = AgentShellScheduler::new();
        let shell = create_test_shell();
        scheduler.add_shell(shell);
        assert_eq!(scheduler.shells.len(), 1);
    }

    #[test]
    fn test_schedule_with_matching_intent() {
        let mut scheduler = AgentShellScheduler::new();
        let shell = create_test_shell();
        scheduler.add_shell(shell);

        let intent = create_test_intent();
        let result = scheduler.schedule(&intent);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.shell.name, "Code Generator");
        assert_eq!(result.params.values.get("language").unwrap(), "rust");
    }

    #[test]
    fn test_schedule_with_non_matching_intent() {
        let mut scheduler = AgentShellScheduler::new();
        let shell = create_test_shell();
        scheduler.add_shell(shell);

        let intent = Intent {
            id: "intent-2".to_string(),
            description: "Test code".to_string(),
            intent_type: IntentType::TestGeneration,
            requirements: vec!["test-generation".to_string()],
            priority: Priority::Medium,
        };

        let result = scheduler.schedule(&intent);
        assert!(result.is_none());
    }

    #[test]
    fn test_scheduling_strategy() {
        let scheduler = AgentShellScheduler::new().with_strategy(SchedulingStrategy::RoundRobin);
        assert_eq!(scheduler.strategy, SchedulingStrategy::RoundRobin);
    }

    #[test]
    fn test_schedule_empty_shells() {
        let scheduler = AgentShellScheduler::new();
        let intent = create_test_intent();
        let result = scheduler.schedule(&intent);
        assert!(result.is_none());
    }

    #[test]
    fn test_schedule_multiple_shells_picks_first_match() {
        let mut scheduler = AgentShellScheduler::new();
        let shell1 = create_test_shell();
        let shell2 = AgentShell {
            id: "shell-2".to_string(),
            name: "Test Generator".to_string(),
            description: "Generates tests".to_string(),
            capabilities: vec!["test-generation".to_string()],
            constraints: vec![],
            param_templates: vec![],
            scheduling_strategy: SchedulingStrategy::RoundRobin,
        };
        scheduler.add_shell(shell1);
        scheduler.add_shell(shell2);

        let intent = create_test_intent();
        let result = scheduler.schedule(&intent);
        assert!(result.is_some());
        assert_eq!(result.unwrap().shell.id, "shell-1");
    }

    #[test]
    fn test_fill_params_with_default_value() {
        let scheduler = AgentShellScheduler::new();
        let shell = create_test_shell();
        let intent = create_test_intent();

        let params = scheduler.fill_params(&shell, &intent);
        // ParamTemplate has default_value "rust" for "language"
        assert_eq!(params.values.get("language").unwrap(), "rust");
    }

    #[test]
    fn test_param_type_equality() {
        assert_eq!(ParamType::String, ParamType::String);
        assert_ne!(ParamType::String, ParamType::Number);
        assert_ne!(ParamType::Boolean, ParamType::List);
    }

    #[test]
    fn test_scheduling_strategy_all_variants() {
        let strategies = [SchedulingStrategy::RoundRobin,
            SchedulingStrategy::LeastLoaded,
            SchedulingStrategy::Affinity,
            SchedulingStrategy::CacheAware,
            SchedulingStrategy::GpuAware,
            SchedulingStrategy::Priority,
            SchedulingStrategy::ResourceAware];
        // All variants should be distinct
        for i in 0..strategies.len() {
            for j in (i + 1)..strategies.len() {
                assert_ne!(strategies[i], strategies[j]);
            }
        }
    }
}
