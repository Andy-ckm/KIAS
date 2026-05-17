//! 方案自动生成 — KIAS自循环的核心
//!
//! 自动生成修复方案，包括：
//! - 代码修改方案
//! - 配置修改方案
//! - 依赖更新方案
//! - 测试添加方案

use serde::{Deserialize, Serialize};

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

impl Default for PersistencePlanGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistencePlanGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl PlanGenerator for PersistencePlanGenerator {
    fn generate(&self, problem_description: &str, _root_cause: &str) -> Option<GeneratedPlan> {
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
                    expected_changes: "添加AgentRepository字段，初始化时从SQLite加载数据"
                        .to_string(),
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

impl Default for ConfigFixPlanGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigFixPlanGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl PlanGenerator for ConfigFixPlanGenerator {
    fn generate(&self, problem_description: &str, _root_cause: &str) -> Option<GeneratedPlan> {
        if !problem_description.contains("配置") && !problem_description.contains("placeholder") {
            return None;
        }

        Some(GeneratedPlan {
            id: uuid::Uuid::new_v4().to_string(),
            plan_type: PlanType::ConfigChange,
            title: "修复配置文件".to_string(),
            description: "将placeholder配置替换为真实配置".to_string(),
            steps: vec![PlanStep {
                order: 1,
                step_type: StepType::ConfigChange,
                description: "更新config/kias.toml，使用真实API key".to_string(),
                files: vec!["config/kias.toml".to_string()],
                expected_changes: "API key从placeholder替换为真实值".to_string(),
                verification: "配置文件格式正确".to_string(),
            }],
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

impl Default for PlanGeneratorManager {
    fn default() -> Self {
        Self::new()
    }
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
    pub fn generate_plans(
        &mut self,
        problem_description: &str,
        root_cause: &str,
    ) -> Vec<GeneratedPlan> {
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

/// 错误驱动方案生成器 — 根据真实分析结果生成修复方案
///
/// 不再硬编码，而是根据 CargoOutputAnalyzer 的输出动态生成修复步骤。
/// 这是控制论闭环的关键：决策器基于真实信号生成行动方案。
pub struct ErrorDrivenPlanner;

impl Default for ErrorDrivenPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorDrivenPlanner {
    pub fn new() -> Self {
        Self
    }

    /// 根据分析结果生成修复方案
    pub fn generate_from_analysis(
        &self,
        analysis: &crate::analyzer::AnalysisResult,
    ) -> Option<GeneratedPlan> {
        if !analysis.found_root_cause {
            return None;
        }

        let error_category = analysis.error_category.as_ref()?;
        let related_files = &analysis.related_files;

        let (title, description, steps, risks) = match error_category {
            crate::analyzer::ErrorCategory::Unused => {
                let files = related_files
                    .iter()
                    .map(|f| PlanStep {
                        order: 1,
                        step_type: StepType::CodeChange,
                        description: format!("移除 {} 中的未使用代码", f),
                        files: vec![f.clone()],
                        expected_changes: "删除未使用的变量/导入".to_string(),
                        verification: "cargo clippy -- -D warnings 通过".to_string(),
                    })
                    .collect::<Vec<_>>();

                (
                    "修复未使用代码警告".to_string(),
                    "移除未使用的变量、导入和死代码".to_string(),
                    if steps_empty(&files) {
                        vec![generic_step("运行 cargo fix 自动修复")]
                    } else {
                        files
                    },
                    vec!["可能删除了将来需要的代码".to_string()],
                )
            }
            crate::analyzer::ErrorCategory::ClippyWarning => (
                "修复 Clippy 警告".to_string(),
                "按 Clippy 建议修改代码".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "运行 cargo clippy --fix 自动修复".to_string(),
                    files: related_files.clone(),
                    expected_changes: "按 Clippy 建议修改".to_string(),
                    verification: "cargo clippy -- -D warnings 零警告".to_string(),
                }],
                vec![],
            ),
            crate::analyzer::ErrorCategory::TypeError => (
                "修复类型错误".to_string(),
                format!("修复 {} 中的类型不匹配", related_files.join(", ")),
                vec![
                    PlanStep {
                        order: 1,
                        step_type: StepType::CodeChange,
                        description: "检查类型标注和实际值是否匹配".to_string(),
                        files: related_files.clone(),
                        expected_changes: "修正类型不匹配".to_string(),
                        verification: "cargo check 通过".to_string(),
                    },
                    PlanStep {
                        order: 2,
                        step_type: StepType::TestAddition,
                        description: "添加类型正确性测试".to_string(),
                        files: related_files.clone(),
                        expected_changes: "添加测试覆盖类型边界".to_string(),
                        verification: "cargo test 通过".to_string(),
                    },
                ],
                vec!["可能需要修改接口定义".to_string()],
            ),
            crate::analyzer::ErrorCategory::BorrowError => (
                "修复借用错误".to_string(),
                "修复所有权和借用问题".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "重构代码解决借用冲突（clone/restructure/ref cell）".to_string(),
                    files: related_files.clone(),
                    expected_changes: "修复所有权转移".to_string(),
                    verification: "cargo check 通过".to_string(),
                }],
                vec!["可能需要重构数据结构".to_string()],
            ),
            crate::analyzer::ErrorCategory::NotFound => (
                "修复未找到错误".to_string(),
                "添加缺失的定义或修正拼写".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "添加缺失的函数/类型/模块定义".to_string(),
                    files: related_files.clone(),
                    expected_changes: "添加缺失定义".to_string(),
                    verification: "cargo check 通过".to_string(),
                }],
                vec!["可能需要更新依赖".to_string()],
            ),
            crate::analyzer::ErrorCategory::TestFailure => (
                "修复测试失败".to_string(),
                "修复失败的测试用例".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "分析测试失败原因并修复".to_string(),
                    files: related_files.clone(),
                    expected_changes: "修复测试逻辑或被测代码".to_string(),
                    verification: "cargo test 通过".to_string(),
                }],
                vec!["可能需要更新测试期望".to_string()],
            ),
            crate::analyzer::ErrorCategory::CompilationError => (
                "修复编译错误".to_string(),
                format!(
                    "修复编译错误: {}",
                    analysis.root_cause.as_deref().unwrap_or("")
                ),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "根据编译错误信息修复代码".to_string(),
                    files: related_files.clone(),
                    expected_changes: "修复编译错误".to_string(),
                    verification: "cargo check --workspace 通过".to_string(),
                }],
                vec!["可能影响其他模块".to_string()],
            ),
            crate::analyzer::ErrorCategory::LifetimeError => (
                "修复生命周期错误".to_string(),
                "修复生命周期标注".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "添加或修正生命周期标注".to_string(),
                    files: related_files.clone(),
                    expected_changes: "修正生命周期".to_string(),
                    verification: "cargo check 通过".to_string(),
                }],
                vec!["可能需要重构 API".to_string()],
            ),
            crate::analyzer::ErrorCategory::Unknown => (
                "修复未知错误".to_string(),
                "需要人工分析".to_string(),
                vec![PlanStep {
                    order: 1,
                    step_type: StepType::CodeChange,
                    description: "人工分析并修复".to_string(),
                    files: related_files.clone(),
                    expected_changes: "待定".to_string(),
                    verification: "cargo check 通过".to_string(),
                }],
                vec!["需要人工介入".to_string()],
            ),
        };

        let difficulty = analysis.difficulty.unwrap_or(5);
        Some(GeneratedPlan {
            id: uuid::Uuid::new_v4().to_string(),
            plan_type: PlanType::CodeChange,
            title,
            description,
            steps,
            expected_outcome: "编译/测试/clippy 全部通过".to_string(),
            risks,
            requires_human: difficulty > 6,
            generated_at: chrono::Utc::now(),
        })
    }
}

fn steps_empty(steps: &[PlanStep]) -> bool {
    steps.is_empty()
}

fn generic_step(description: &str) -> PlanStep {
    PlanStep {
        order: 1,
        step_type: StepType::CodeChange,
        description: description.to_string(),
        files: vec![],
        expected_changes: "自动修复".to_string(),
        verification: "cargo check 通过".to_string(),
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
        let plan = generator.generate(
            "Workflow执行需要LLM API但配置是placeholder",
            "placeholder配置",
        );

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

    #[test]
    fn test_persistence_generator_no_match() {
        let generator = PersistencePlanGenerator::new();
        let plan = generator.generate("网络连接超时", "DNS解析失败");
        assert!(plan.is_none());
    }

    #[test]
    fn test_persistence_generator_match_丢失() {
        let generator = PersistencePlanGenerator::new();
        let plan = generator.generate("数据丢失问题", "内存存储");
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(matches!(plan.plan_type, PlanType::CodeChange));
        assert!(!plan.requires_human);
    }

    #[test]
    fn test_persistence_plan_structure() {
        let generator = PersistencePlanGenerator::new();
        let plan = generator
            .generate("Agent数据持久化缺失", "HashMap存储")
            .unwrap();

        assert!(!plan.id.is_empty());
        assert!(plan.title.contains("持久化"));
        assert!(!plan.description.is_empty());
        assert_eq!(plan.steps.len(), 3);
        assert!(!plan.expected_outcome.is_empty());
        assert!(!plan.risks.is_empty());

        // Verify step ordering
        for (i, step) in plan.steps.iter().enumerate() {
            assert_eq!(step.order, (i + 1) as u8);
            assert!(!step.description.is_empty());
            assert!(!step.files.is_empty());
            assert!(!step.verification.is_empty());
        }
    }

    #[test]
    fn test_config_fix_generator_no_match() {
        let generator = ConfigFixPlanGenerator::new();
        let plan = generator.generate("数据库连接失败", "网络问题");
        assert!(plan.is_none());
    }

    #[test]
    fn test_config_fix_generator_requires_human() {
        let generator = ConfigFixPlanGenerator::new();
        let plan = generator.generate("配置是placeholder", "默认配置").unwrap();
        assert!(plan.requires_human);
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_config_fix_plan_structure() {
        let generator = ConfigFixPlanGenerator::new();
        let plan = generator.generate("配置错误", "placeholder").unwrap();

        assert!(!plan.id.is_empty());
        assert!(plan.title.contains("配置"));
        assert!(matches!(plan.steps[0].step_type, StepType::ConfigChange));
    }

    #[test]
    fn test_manager_empty() {
        let mut manager = PlanGeneratorManager::new();
        let plans = manager.generate_plans("任何问题", "任何原因");
        assert!(plans.is_empty());
        assert!(manager.history().is_empty());
    }

    #[test]
    fn test_manager_history_tracking() {
        let mut manager = PlanGeneratorManager::new();
        manager.register_generator(Box::new(PersistencePlanGenerator::new()));

        manager.generate_plans("Agent数据持久化缺失", "HashMap");
        assert_eq!(manager.history().len(), 1);

        manager.generate_plans("数据丢失", "内存");
        assert_eq!(manager.history().len(), 2);
    }

    #[test]
    fn test_manager_multiple_generators_different_problems() {
        let mut manager = PlanGeneratorManager::new();
        manager.register_generator(Box::new(PersistencePlanGenerator::new()));
        manager.register_generator(Box::new(ConfigFixPlanGenerator::new()));

        // Only persistence generator matches
        let plans = manager.generate_plans("数据持久化缺失", "HashMap");
        assert_eq!(plans.len(), 1);

        // Only config generator matches
        let plans = manager.generate_plans("配置是placeholder", "默认值");
        assert_eq!(plans.len(), 1);

        // Neither matches
        let plans = manager.generate_plans("网络超时", "DNS");
        assert!(plans.is_empty());
    }

    #[test]
    fn test_generated_plan_serialization() {
        let generator = PersistencePlanGenerator::new();
        let plan = generator
            .generate("Agent数据持久化缺失", "HashMap")
            .unwrap();

        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: GeneratedPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, plan.id);
        assert_eq!(deserialized.title, plan.title);
        assert_eq!(deserialized.steps.len(), plan.steps.len());
    }

    #[test]
    fn test_plan_type_serialization() {
        let types = vec![
            PlanType::CodeChange,
            PlanType::ConfigChange,
            PlanType::DependencyUpdate,
            PlanType::TestAddition,
            PlanType::DocumentationUpdate,
        ];
        for pt in types {
            let json = serde_json::to_string(&pt).unwrap();
            let _: PlanType = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_step_type_serialization() {
        let types = vec![
            StepType::CodeChange,
            StepType::ConfigChange,
            StepType::TestAddition,
            StepType::DocumentationUpdate,
            StepType::DependencyUpdate,
        ];
        for st in types {
            let json = serde_json::to_string(&st).unwrap();
            let _: StepType = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_default_trait_implementations() {
        let _ = PersistencePlanGenerator::default();
        let _ = ConfigFixPlanGenerator::default();
        let _ = PlanGeneratorManager::default();
    }

    #[test]
    fn test_generator_names() {
        assert_eq!(
            PersistencePlanGenerator::new().name(),
            "PersistencePlanGenerator"
        );
        assert_eq!(
            ConfigFixPlanGenerator::new().name(),
            "ConfigFixPlanGenerator"
        );
    }

    #[test]
    fn test_error_driven_planner_type_error() {
        let planner = ErrorDrivenPlanner::new();
        let analysis = crate::analyzer::AnalysisResult {
            found_root_cause: true,
            root_cause: Some("mismatched types".to_string()),
            error_category: Some(crate::analyzer::ErrorCategory::TypeError),
            impact: None,
            difficulty: Some(4),
            estimated_hours: Some(2.0),
            related_files: vec!["src/main.rs".to_string()],
            details: Default::default(),
            analyzed_at: chrono::Utc::now(),
        };

        let plan = planner.generate_from_analysis(&analysis);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(plan.title.contains("类型"));
        assert_eq!(plan.steps.len(), 2); // code fix + test
        assert!(!plan.requires_human); // difficulty 4 < 7
    }

    #[test]
    fn test_error_driven_planner_borrow_error() {
        let planner = ErrorDrivenPlanner::new();
        let analysis = crate::analyzer::AnalysisResult {
            found_root_cause: true,
            root_cause: Some("use of moved value".to_string()),
            error_category: Some(crate::analyzer::ErrorCategory::BorrowError),
            impact: None,
            difficulty: Some(7),
            estimated_hours: Some(3.5),
            related_files: vec!["src/lib.rs".to_string()],
            details: Default::default(),
            analyzed_at: chrono::Utc::now(),
        };

        let plan = planner.generate_from_analysis(&analysis);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(plan.title.contains("借用"));
        assert!(plan.requires_human); // difficulty 7 > 6
    }

    #[test]
    fn test_error_driven_planner_no_root_cause() {
        let planner = ErrorDrivenPlanner::new();
        let analysis = crate::analyzer::AnalysisResult {
            found_root_cause: false,
            root_cause: None,
            error_category: None,
            impact: None,
            difficulty: None,
            estimated_hours: None,
            related_files: vec![],
            details: Default::default(),
            analyzed_at: chrono::Utc::now(),
        };

        let plan = planner.generate_from_analysis(&analysis);
        assert!(plan.is_none());
    }

    #[test]
    fn test_error_driven_planner_clippy_warning() {
        let planner = ErrorDrivenPlanner::new();
        let analysis = crate::analyzer::AnalysisResult {
            found_root_cause: true,
            root_cause: Some("unused variable".to_string()),
            error_category: Some(crate::analyzer::ErrorCategory::ClippyWarning),
            impact: None,
            difficulty: Some(2),
            estimated_hours: Some(0.5),
            related_files: vec!["src/main.rs".to_string()],
            details: Default::default(),
            analyzed_at: chrono::Utc::now(),
        };

        let plan = planner.generate_from_analysis(&analysis);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(plan.title.contains("Clippy"));
        assert!(!plan.requires_human);
    }

    #[test]
    fn test_error_driven_planner_test_failure() {
        let planner = ErrorDrivenPlanner::new();
        let analysis = crate::analyzer::AnalysisResult {
            found_root_cause: true,
            root_cause: Some("assertion failed".to_string()),
            error_category: Some(crate::analyzer::ErrorCategory::TestFailure),
            impact: None,
            difficulty: Some(5),
            estimated_hours: Some(2.5),
            related_files: vec!["tests/integration.rs".to_string()],
            details: Default::default(),
            analyzed_at: chrono::Utc::now(),
        };

        let plan = planner.generate_from_analysis(&analysis);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(plan.title.contains("测试"));
        assert_eq!(plan.steps.len(), 1);
    }
}
