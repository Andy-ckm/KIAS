//! 自动迭代循环 — KIAS的核心运行时闭环
//!
//! 自动发现问题 → 自动分析 → 自动提出方案 → 自动实施 → 自动验证 → 自动积累经验
//! 人类适当时候介入，默认自动迭代

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 循环状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopStatus {
    /// 空闲
    Idle,
    /// 发现问题中
    Discovering,
    /// 分析问题中
    Analyzing,
    /// 制定方案中
    Planning,
    /// 实施修复中
    Implementing,
    /// 验证修复中
    Verifying,
    /// 等待人类介入
    WaitingForHuman,
    /// 完成
    Completed,
    /// 失败
    Failed,
}

/// 问题发现方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// 系统监控
    SystemMonitoring,
    /// 测试失败
    TestFailure,
    /// 用户反馈
    UserFeedback,
    /// 自检
    SelfCheck,
    /// 性能分析
    PerformanceAnalysis,
}

/// 发现的问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProblem {
    /// 问题ID
    pub id: String,
    /// 问题标题
    pub title: String,
    /// 问题描述
    pub description: String,
    /// 发现方式
    pub discovery_method: DiscoveryMethod,
    /// 严重程度 (1-10)
    pub severity: u8,
    /// 影响范围
    pub impact: String,
    /// 相关代码位置
    pub code_locations: Vec<String>,
    /// 相关日志
    pub logs: Vec<String>,
    /// 发现时间
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

/// 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// 问题ID
    pub problem_id: String,
    /// 根因分析
    pub root_cause: String,
    /// 影响分析
    pub impact_analysis: String,
    /// 修复难度 (1-10)
    pub difficulty: u8,
    /// 预计工作量（小时）
    pub estimated_hours: f64,
    /// 相关模块
    pub affected_modules: Vec<String>,
    /// 分析时间
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
}

/// 修复方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPlan {
    /// 方案ID
    pub id: String,
    /// 关联问题ID
    pub problem_id: String,
    /// 方案标题
    pub title: String,
    /// 方案描述
    pub description: String,
    /// 实现步骤
    pub steps: Vec<FixStep>,
    /// 预期效果
    pub expected_outcome: String,
    /// 风险评估
    pub risks: Vec<String>,
    /// 是否需要人类介入
    pub requires_human: bool,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 修复步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixStep {
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

/// 实施结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationResult {
    /// 方案ID
    pub plan_id: String,
    /// 是否成功
    pub success: bool,
    /// 变更文件列表
    pub changed_files: Vec<String>,
    /// 新增测试
    pub new_tests: Vec<String>,
    /// 代码行数变化
    pub lines_changed: i32,
    /// 实施耗时（秒）
    pub duration_seconds: u64,
    /// 遇到的问题
    pub issues: Vec<String>,
    /// 实施时间
    pub implemented_at: chrono::DateTime<chrono::Utc>,
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// 实施结果ID
    pub implementation_id: String,
    /// 测试是否通过
    pub tests_passed: bool,
    /// 问题是否解决
    pub problem_resolved: bool,
    /// 是否引入新问题
    pub new_issues_introduced: bool,
    /// 性能是否改善
    pub performance_improved: bool,
    /// 验证详情
    pub details: String,
    /// 验证时间
    pub verified_at: chrono::DateTime<chrono::Utc>,
}

/// 循环记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopRecord {
    /// 记录ID
    pub id: String,
    /// 问题
    pub problem: DiscoveredProblem,
    /// 分析结果
    pub analysis: Option<AnalysisResult>,
    /// 修复方案
    pub plan: Option<FixPlan>,
    /// 实施结果
    pub implementation: Option<ImplementationResult>,
    /// 验证结果
    pub verification: Option<VerificationResult>,
    /// 循环状态
    pub status: LoopStatus,
    /// 开始时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 完成时间
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 经验教训
    pub lessons: Vec<String>,
}

/// 自动迭代循环管理器
pub struct AutoLoopManager {
    /// 循环记录
    records: Vec<LoopRecord>,
    /// 当前状态
    current_status: LoopStatus,
    /// 配置
    config: AutoLoopConfig,
    /// 知识库
    knowledge_base: Vec<KnowledgeEntry>,
}

/// 自动循环配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLoopConfig {
    /// 是否启用自动循环
    pub enabled: bool,
    /// 最大并发问题数
    pub max_concurrent_problems: usize,
    /// 自动修复阈值（严重程度低于此值自动修复）
    pub auto_fix_threshold: u8,
    /// 是否需要人类确认
    pub require_human_confirmation: bool,
    /// 循环间隔（秒）
    pub loop_interval_seconds: u64,
    /// 最大重试次数
    pub max_retries: u8,
}

/// 知识条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// 条目ID
    pub id: String,
    /// 标题
    pub title: String,
    /// 内容
    pub content: String,
    /// 标签
    pub tags: Vec<String>,
    /// 来源循环ID
    pub source_loop_id: Option<String>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Default for AutoLoopConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_problems: 5,
            auto_fix_threshold: 7,
            require_human_confirmation: false,
            loop_interval_seconds: 300,
            max_retries: 3,
        }
    }
}

impl AutoLoopManager {
    /// 创建新的自动循环管理器
    pub fn new(config: AutoLoopConfig) -> Self {
        Self {
            records: Vec::new(),
            current_status: LoopStatus::Idle,
            config,
            knowledge_base: Vec::new(),
        }
    }

    /// 启动循环
    pub fn start_loop(&mut self, problem: DiscoveredProblem) -> String {
        let loop_id = uuid::Uuid::new_v4().to_string();
        let record = LoopRecord {
            id: loop_id.clone(),
            problem,
            analysis: None,
            plan: None,
            implementation: None,
            verification: None,
            status: LoopStatus::Discovering,
            started_at: chrono::Utc::now(),
            completed_at: None,
            lessons: Vec::new(),
        };
        self.records.push(record);
        self.current_status = LoopStatus::Discovering;
        loop_id
    }

    /// 分析问题
    pub fn analyze_problem(&mut self, loop_id: &str, analysis: AnalysisResult) {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == loop_id) {
            record.analysis = Some(analysis);
            record.status = LoopStatus::Planning;
            self.current_status = LoopStatus::Planning;
        }
    }

    /// 制定方案
    pub fn create_plan(&mut self, loop_id: &str, plan: FixPlan) {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == loop_id) {
            let requires_human = plan.requires_human;
            record.plan = Some(plan);
            if requires_human && self.config.require_human_confirmation {
                record.status = LoopStatus::WaitingForHuman;
                self.current_status = LoopStatus::WaitingForHuman;
            } else {
                record.status = LoopStatus::Implementing;
                self.current_status = LoopStatus::Implementing;
            }
        }
    }

    /// 实施修复
    pub fn implement_fix(&mut self, loop_id: &str, result: ImplementationResult) {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == loop_id) {
            record.implementation = Some(result);
            record.status = LoopStatus::Verifying;
            self.current_status = LoopStatus::Verifying;
        }
    }

    /// 验证修复
    pub fn verify_fix(&mut self, loop_id: &str, result: VerificationResult) {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == loop_id) {
            let success = result.problem_resolved && !result.new_issues_introduced;
            record.verification = Some(result);
            record.status = if success {
                LoopStatus::Completed
            } else {
                LoopStatus::Failed
            };
            record.completed_at = Some(chrono::Utc::now());
            self.current_status = LoopStatus::Idle;

            // 提取经验教训
            if success {
                record.lessons.push(format!(
                    "问题 '{}' 已成功修复",
                    record.problem.title
                ));
            } else {
                record.lessons.push(format!(
                    "问题 '{}' 修复失败，需要进一步分析",
                    record.problem.title
                ));
            }

            // 积累知识
            for lesson in &record.lessons {
                self.knowledge_base.push(KnowledgeEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: format!("Lesson from {}", record.problem.title),
                    content: lesson.clone(),
                    tags: vec!["auto-loop".to_string()],
                    source_loop_id: Some(loop_id.to_string()),
                    created_at: chrono::Utc::now(),
                });
            }
        }
    }

    /// 获取当前状态
    pub fn current_status(&self) -> &LoopStatus {
        &self.current_status
    }

    /// 获取循环记录
    pub fn records(&self) -> &[LoopRecord] {
        &self.records
    }

    /// 获取知识库
    pub fn knowledge_base(&self) -> &[KnowledgeEntry] {
        &self.knowledge_base
    }

    /// 生成循环报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# KIAS 自动迭代循环报告\n\n");
        report.push_str(&format!(
            "生成时间: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        report.push_str("## 循环统计\n\n");
        report.push_str(&format!("- 总循环数: {}\n", self.records.len()));
        let completed = self.records.iter().filter(|r| r.status == LoopStatus::Completed).count();
        let failed = self.records.iter().filter(|r| r.status == LoopStatus::Failed).count();
        report.push_str(&format!("- 成功完成: {}\n", completed));
        report.push_str(&format!("- 失败: {}\n", failed));
        report.push_str(&format!("- 当前状态: {:?}\n", self.current_status));

        report.push_str("\n## 知识库\n\n");
        report.push_str(&format!("- 经验教训条目: {}\n", self.knowledge_base.len()));

        report.push_str("\n## 最近循环\n\n");
        for record in self.records.iter().rev().take(5) {
            report.push_str(&format!("### {}\n", record.problem.title));
            report.push_str(&format!("- 状态: {:?}\n", record.status));
            report.push_str(&format!("- 严重程度: {}\n", record.problem.severity));
            if let Some(analysis) = &record.analysis {
                report.push_str(&format!("- 根因: {}\n", analysis.root_cause));
            }
            report.push_str("\n");
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_loop_manager_creation() {
        let config = AutoLoopConfig::default();
        let manager = AutoLoopManager::new(config);
        assert_eq!(manager.current_status(), &LoopStatus::Idle);
        assert_eq!(manager.records().len(), 0);
    }

    #[test]
    fn test_start_loop() {
        let config = AutoLoopConfig::default();
        let mut manager = AutoLoopManager::new(config);
        let problem = DiscoveredProblem {
            id: "p1".to_string(),
            title: "Test Problem".to_string(),
            description: "Test".to_string(),
            discovery_method: DiscoveryMethod::SelfCheck,
            severity: 5,
            impact: "Test".to_string(),
            code_locations: vec![],
            logs: vec![],
            discovered_at: chrono::Utc::now(),
        };
        let loop_id = manager.start_loop(problem);
        assert!(!loop_id.is_empty());
        assert_eq!(manager.records().len(), 1);
    }

    #[test]
    fn test_full_loop() {
        let config = AutoLoopConfig::default();
        let mut manager = AutoLoopManager::new(config);
        let problem = DiscoveredProblem {
            id: "p1".to_string(),
            title: "Test Problem".to_string(),
            description: "Test".to_string(),
            discovery_method: DiscoveryMethod::SelfCheck,
            severity: 5,
            impact: "Test".to_string(),
            code_locations: vec![],
            logs: vec![],
            discovered_at: chrono::Utc::now(),
        };
        let loop_id = manager.start_loop(problem);

        // 分析
        let analysis = AnalysisResult {
            problem_id: "p1".to_string(),
            root_cause: "Test root cause".to_string(),
            impact_analysis: "Test impact".to_string(),
            difficulty: 3,
            estimated_hours: 1.0,
            affected_modules: vec!["test".to_string()],
            analyzed_at: chrono::Utc::now(),
        };
        manager.analyze_problem(&loop_id, analysis);

        // 制定方案
        let plan = FixPlan {
            id: "plan1".to_string(),
            problem_id: "p1".to_string(),
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            steps: vec![],
            expected_outcome: "Test".to_string(),
            risks: vec![],
            requires_human: false,
            created_at: chrono::Utc::now(),
        };
        manager.create_plan(&loop_id, plan);

        // 实施
        let implementation = ImplementationResult {
            plan_id: "plan1".to_string(),
            success: true,
            changed_files: vec!["test.rs".to_string()],
            new_tests: vec![],
            lines_changed: 10,
            duration_seconds: 60,
            issues: vec![],
            implemented_at: chrono::Utc::now(),
        };
        manager.implement_fix(&loop_id, implementation);

        // 验证
        let verification = VerificationResult {
            implementation_id: "impl1".to_string(),
            tests_passed: true,
            problem_resolved: true,
            new_issues_introduced: false,
            performance_improved: false,
            details: "Test".to_string(),
            verified_at: chrono::Utc::now(),
        };
        manager.verify_fix(&loop_id, verification);

        assert_eq!(manager.current_status(), &LoopStatus::Idle);
        assert_eq!(manager.records()[0].status, LoopStatus::Completed);
        assert_eq!(manager.knowledge_base().len(), 1);
    }

    #[test]
    fn test_generate_report() {
        let config = AutoLoopConfig::default();
        let manager = AutoLoopManager::new(config);
        let report = manager.generate_report();
        assert!(report.contains("KIAS 自动迭代循环报告"));
    }
}
