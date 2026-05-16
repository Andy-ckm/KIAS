//! 方案自动生成 — KIAS自循环的核心
//!
//! 自动生成修复方案，包括：
//! - 代码修改方案
//! - 配置修改方案
//! - 依赖更新方案
//! - 测试添加方案

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 方案类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanType {
    /// 代码修改
    CodeChange,
    /// 配置修改
    ConfigChange,
    /// 依赖更新
    DependencyUpdate,
    /// 测试添加
    TestAddition,
    /// 文档更新
    DocumentationUpdate,
}

/// 生成的方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPlan {
    /// 方案ID
    pub id: String,
    /// 方案类型
    pub plan_type: PlanType,
    /// 方案标题
    pub title: String,
    /// 方案描述
    pub description: String,
    /// 实现步骤
    pub steps: Vec<PlanStep>,
    /// 预期效果
    pub expected_outcome: String,
    /// 风险评估
    pub risks: Vec<String>,
    /// 是否需要人类介入
    pub requires_human: bool,
    /// 生成时间
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// 方案步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// 步骤序号
    pub order: u8,
    /// 步骤类型
    pub step_type: StepType,
    /// 步骤描述
    pub description: String,
    /// 涉及文件
    pub files: Vec<String>,
    /// 预期变更
    pub expected_changes: String,
    /// 验证方法
    pub verification: String,
}

/// 步骤类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    /// 代码修改
    CodeChange,
    /// 配置修改
    ConfigChange,
    /// 测试添加
    TestAddition,
    /// 文档更新
    DocumentationUpdate,
    /// 依赖更新
    DependencyUpdate,
}

/// 方案生成器 trait
pub trait PlanGenerator: Send + Sync {
    /// 生成方案
    fn generate(&self, problem_description: &str, root_cause: &str) -> Option<GeneratedPlan>;
    
    /// 获取生成器名称
    fn name(&self) -> &str;
}

/// 数据持久化方案生成器
pub struct PersistencePlanGenerator;

impl PersistencePlanGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl PlanGenerator for PersistencePlanGenerator {
    fn generate(&self, problem_description: &str, root_cause: &str) -> Option<GeneratedPlan> {
        if !problem_description.contains("持久化") && !problem_description.contains("丢失") {
            return None;
        }
        
        Some(GeneratedPlan {
            id: uuid::Uuid::new_v4().to_string(),
            plan_type: PlanType::CodeChange,
            title: "实现Agent数据持久化".to_string(),
            description: "将Agent数据从HashMap存储改为SQLite持久化存储".to_string(),
            steps: vec![
                PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "修改AppState，添加agent_repository字段".to_string(),
                    files: vec!["crates/api-server/src/lib.rs".to_string()],
                    expected_changes: "添加AgentRepository字段，初始化时从SQLite加载数据".to_string(),
                    verification: "编译通过，测试通过".to_string(),
                },
                PlanStep {
                    order: 2,
                    step_type: StepType::CodeChange,
                    description: "修改Agent handler，使用repository而不是HashMap".to_string(),
                    files: vec!["crates/api-server/src/handlers/agents.rs".to_string()],
                    expected_changes: "所有Agent操作都通过repository进行".to_string(),
                    verification: "Agent CRUD操作正常".to_string(),
                },
                PlanStep {
                    order: 3,
                    step_type: StepType::TestAddition,
                    description: "添加持久化测试".to_string(),
                    files: vec!["crates/api-server/src/handlers/agents.rs".to_string()],
                    expected_changes: "添加测试验证数据持久化".to_string(),
                    verification: "测试通过，重启后数据不丢失".to_string(),
                },
            ],
            expected_outcome: "服务器重启后Agent数据不丢失".to_string(),
            risks: vec!["需要修改数据库schema".to_string()],
            requires_human: false,
            generated_at: chrono::Utc::now(),
        })
    }
    
    fn name(&self) -> &str {
        "PersistencePlanGenerator"
    }
}

/// 配置修复方案生成器
pub struct ConfigFixPlanGenerator;

impl ConfigFixPlanGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl PlanGenerator for ConfigFixPlanGenerator {
    fn generate(&self, problem_description: &str, root_cause: &str) -> Option<GeneratedPlan> {
        if !problem_description.contains("配置") && !problem_description.contains("placeholder") {
            return None;
        }
        
        Some(GeneratedPlan {
            id: uuid::Uuid::new_v4().to_string(),
            plan_type: PlanType::ConfigChange,
            title: "修复配置文件".to_string(),
            description: "将placeholder配置替换为真实配置".to_string(),
            steps: vec![
                PlanStep {
                    order: 1,
                    step_type: StepType::ConfigChange,
                    description: "更新config/kias.toml，使用真实API key".to_string(),
                    files: vec!["config/kias.toml".to_string()],
                    expected_changes: "API key从placeholder替换为真实值".to_string(),
                    verification: "配置文件格式正确".to_string(),
                },
            ],
            expected_outcome: "API调用正常工作".to_string(),
            risks: vec!["需要真实API key".to_string()],
            requires_human: true,
            generated_at: chrono::Utc::now(),
        })
    }
    
    fn name(&self) -> &str {
        "ConfigFixPlanGenerator"
    }
}

/// 方案生成器管理器
pub struct PlanGeneratorManager {
    /// 生成器列表
    generators: Vec<Box<dyn PlanGenerator>>,
    /// 生成历史
    history: Vec<GeneratedPlan>,
}

impl PlanGeneratorManager {
    pub fn new() -> Self {
        Self {
            generators: Vec::new(),
            history: Vec::new(),
        }
    }
    
    /// 注册生成器
    pub fn register_generator(&mut self, generator: Box<dyn PlanGenerator>) {
        self.generators.push(generator);
    }
    
    /// 生成方案
    pub fn generate_plans(&mut self, problem_description: &str, root_cause: &str) -> Vec<GeneratedPlan> {
        let mut plans = Vec::new();
        
        for generator in &self.generators {
            if let Some(plan) = generator.generate(problem_description, root_cause) {
                plans.push(plan.clone());
                self.history.push(plan);
            }
        }
        
        plans
    }
    
    /// 获取生成历史
    pub fn history(&self) -> &[GeneratedPlan] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistence_plan_generator() {
        let generator = PersistencePlanGenerator::new();
        let plan = generator.generate("Agent数据持久化缺失", "HashMap存储");
        
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_config_fix_plan_generator() {
        let generator = ConfigFixPlanGenerator::new();
        let plan = generator.generate("Workflow执行需要LLM API但配置是placeholder", "placeholder配置");
        
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_plan_generator_manager() {
        let mut manager = PlanGeneratorManager::new();
        
        manager.register_generator(Box::new(PersistencePlanGenerator::new()));
        manager.register_generator(Box::new(ConfigFixPlanGenerator::new()));
        
        let plans = manager.generate_plans("Agent数据持久化缺失", "HashMap存储");
        assert!(!plans.is_empty());
    }
}
