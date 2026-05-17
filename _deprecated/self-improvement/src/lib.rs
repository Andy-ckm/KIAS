//! 自改进循环 — KIAS的核心差异化功能
//!
//! 用KIAS开发KIAS，发现问题→注入KIAS→用KIAS修复→形成闭环
//!
//! 参考：
//! - OpenHuman的自学习机制
//! - Codex的自我改进循环
//! - AutoGPT的自我反思

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 问题来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProblemSource {
    /// 用户反馈
    UserFeedback,
    /// 系统监控
    SystemMonitoring,
    /// 测试失败
    TestFailure,
    /// 性能瓶颈
    PerformanceBottleneck,
    /// 安全漏洞
    SecurityVulnerability,
    /// 代码质量
    CodeQuality,
    /// 文档缺失
    DocumentationGap,
}

/// 问题严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProblemSeverity {
    /// 低：不影响功能
    Low,
    /// 中：影响部分功能
    Medium,
    /// 高：影响核心功能
    High,
    /// 紧急：系统不可用
    Critical,
}

/// 问题描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    /// 问题ID
    pub id: String,
    /// 问题标题
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 问题来源
    pub source: ProblemSource,
    /// 严重程度
    pub severity: ProblemSeverity,
    /// 影响范围
    pub impact: String,
    /// 复现步骤
    pub reproduction_steps: Vec<String>,
    /// 期望行为
    pub expected_behavior: String,
    /// 实际行为
    pub actual_behavior: String,
    /// 发现时间
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    /// 相关代码位置
    pub code_locations: Vec<CodeLocation>,
    /// 相关日志
    pub logs: Vec<String>,
}

/// 代码位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    /// 文件路径
    pub file: String,
    /// 行号
    pub line: Option<u32>,
    /// 函数名
    pub function: Option<String>,
    /// 相关代码片段
    pub snippet: Option<String>,
}

/// 解决方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    /// 方案ID
    pub id: String,
    /// 关联问题ID
    pub problem_id: String,
    /// 方案标题
    pub title: String,
    /// 方案描述
    pub description: String,
    /// 实现步骤
    pub implementation_steps: Vec<ImplementationStep>,
    /// 预期效果
    pub expected_outcome: String,
    /// 风险评估
    pub risks: Vec<Risk>,
    /// 工作量估算（小时）
    pub effort_hours: f64,
    /// 优先级
    pub priority: u32,
    /// 状态
    pub status: SolutionStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 实现步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationStep {
    /// 步骤序号
    pub order: u32,
    /// 步骤描述
    pub description: String,
    /// 涉及文件
    pub files: Vec<String>,
    /// 预期变更
    pub expected_changes: String,
    /// 验证方法
    pub verification: String,
}

/// 风险
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    /// 风险描述
    pub description: String,
    /// 可能性 (0.0-1.0)
    pub likelihood: f64,
    /// 影响程度
    pub impact: String,
    /// 缓解措施
    pub mitigation: String,
}

/// 方案状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SolutionStatus {
    /// 待评审
    Pending,
    /// 已批准
    Approved,
    /// 实施中
    InProgress,
    /// 已完成
    Completed,
    /// 已拒绝
    Rejected,
}

/// 改进记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRecord {
    /// 记录ID
    pub id: String,
    /// 关联问题ID
    pub problem_id: String,
    /// 关联方案ID
    pub solution_id: String,
    /// 实施结果
    pub result: ImplementationResult,
    /// 验证结果
    pub verification: VerificationResult,
    /// 经验教训
    pub lessons_learned: Vec<String>,
    /// 完成时间
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// 实施结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationResult {
    /// 是否成功
    pub success: bool,
    /// 变更文件列表
    pub changed_files: Vec<String>,
    /// 新增测试
    pub new_tests: Vec<String>,
    /// 代码行数变化
    pub lines_changed: i32,
    /// 实施耗时（小时）
    pub actual_hours: f64,
    /// 遇到的问题
    pub issues_encountered: Vec<String>,
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// 测试是否通过
    pub tests_passed: bool,
    /// 性能是否改善
    pub performance_improved: bool,
    /// 问题是否解决
    pub problem_resolved: bool,
    /// 是否引入新问题
    pub new_issues_introduced: bool,
    /// 验证详情
    pub details: String,
}

/// 自改进循环管理器
pub struct SelfImprovementManager {
    /// 问题库
    problems: HashMap<String, Problem>,
    /// 方案库
    solutions: HashMap<String, Solution>,
    /// 改进记录
    records: Vec<ImprovementRecord>,
    /// 知识库
    knowledge_base: Vec<KnowledgeEntry>,
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
    /// 来源问题ID
    pub source_problem_id: Option<String>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SelfImprovementManager {
    /// 创建新的自改进管理器
    pub fn new() -> Self {
        Self {
            problems: HashMap::new(),
            solutions: HashMap::new(),
            records: Vec::new(),
            knowledge_base: Vec::new(),
        }
    }
}

impl Default for SelfImprovementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfImprovementManager {
    /// 注册问题
    pub fn register_problem(&mut self, problem: Problem) {
        self.problems.insert(problem.id.clone(), problem);
    }

    /// 提交解决方案
    pub fn submit_solution(&mut self, solution: Solution) {
        self.solutions.insert(solution.id.clone(), solution);
    }

    /// 记录改进
    pub fn record_improvement(&mut self, record: ImprovementRecord) {
        // 提取经验教训
        for lesson in &record.lessons_learned {
            self.knowledge_base.push(KnowledgeEntry {
                id: uuid::Uuid::new_v4().to_string(),
                title: format!("Lesson from {}", record.problem_id),
                content: lesson.clone(),
                tags: vec!["lesson".to_string()],
                source_problem_id: Some(record.problem_id.clone()),
                created_at: chrono::Utc::now(),
            });
        }
        self.records.push(record);
    }

    /// 获取待处理问题
    pub fn pending_problems(&self) -> Vec<&Problem> {
        self.problems
            .values()
            .filter(|p| {
                p.severity == ProblemSeverity::High || p.severity == ProblemSeverity::Critical
            })
            .collect()
    }

    /// 获取待实施方案
    pub fn pending_solutions(&self) -> Vec<&Solution> {
        self.solutions
            .values()
            .filter(|s| s.status == SolutionStatus::Approved)
            .collect()
    }

    /// 生成改进报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# KIAS 自改进循环报告\n\n");
        report.push_str(&format!(
            "生成时间: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        // 问题统计
        report.push_str("## 问题统计\n\n");
        report.push_str(&format!("- 总问题数: {}\n", self.problems.len()));
        report.push_str(&format!(
            "- 待处理高优问题: {}\n",
            self.pending_problems().len()
        ));

        // 方案统计
        report.push_str("\n## 方案统计\n\n");
        report.push_str(&format!("- 总方案数: {}\n", self.solutions.len()));
        report.push_str(&format!(
            "- 待实施方案: {}\n",
            self.pending_solutions().len()
        ));

        // 改进记录
        report.push_str("\n## 改进记录\n\n");
        report.push_str(&format!("- 已完成改进: {}\n", self.records.len()));
        let successful = self.records.iter().filter(|r| r.result.success).count();
        report.push_str(&format!("- 成功改进: {}\n", successful));

        // 知识库
        report.push_str("\n## 知识库\n\n");
        report.push_str(&format!("- 经验教训条目: {}\n", self.knowledge_base.len()));

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_problem() {
        let mut manager = SelfImprovementManager::new();
        let problem = Problem {
            id: "p1".to_string(),
            title: "Test Problem".to_string(),
            description: "Test".to_string(),
            source: ProblemSource::UserFeedback,
            severity: ProblemSeverity::High,
            impact: "Test".to_string(),
            reproduction_steps: vec![],
            expected_behavior: "Test".to_string(),
            actual_behavior: "Test".to_string(),
            discovered_at: chrono::Utc::now(),
            code_locations: vec![],
            logs: vec![],
        };
        manager.register_problem(problem);
        assert_eq!(manager.problems.len(), 1);
    }

    #[test]
    fn test_submit_solution() {
        let mut manager = SelfImprovementManager::new();
        let solution = Solution {
            id: "s1".to_string(),
            problem_id: "p1".to_string(),
            title: "Test Solution".to_string(),
            description: "Test".to_string(),
            implementation_steps: vec![],
            expected_outcome: "Test".to_string(),
            risks: vec![],
            effort_hours: 1.0,
            priority: 1,
            status: SolutionStatus::Approved,
            created_at: chrono::Utc::now(),
        };
        manager.submit_solution(solution);
        assert_eq!(manager.solutions.len(), 1);
    }

    #[test]
    fn test_record_improvement() {
        let mut manager = SelfImprovementManager::new();
        let record = ImprovementRecord {
            id: "r1".to_string(),
            problem_id: "p1".to_string(),
            solution_id: "s1".to_string(),
            result: ImplementationResult {
                success: true,
                changed_files: vec![],
                new_tests: vec![],
                lines_changed: 10,
                actual_hours: 0.5,
                issues_encountered: vec![],
            },
            verification: VerificationResult {
                tests_passed: true,
                performance_improved: true,
                problem_resolved: true,
                new_issues_introduced: false,
                details: "Test".to_string(),
            },
            lessons_learned: vec!["Lesson 1".to_string()],
            completed_at: chrono::Utc::now(),
        };
        manager.record_improvement(record);
        assert_eq!(manager.records.len(), 1);
        assert_eq!(manager.knowledge_base.len(), 1);
    }

    #[test]
    fn test_generate_report() {
        let manager = SelfImprovementManager::new();
        let report = manager.generate_report();
        assert!(report.contains("KIAS 自改进循环报告"));
    }

    #[test]
    fn test_pending_problems_filters_by_severity() {
        let mut manager = SelfImprovementManager::new();

        // Add problems with different severities
        for (id, severity) in [
            ("p1", ProblemSeverity::Low),
            ("p2", ProblemSeverity::Medium),
            ("p3", ProblemSeverity::High),
            ("p4", ProblemSeverity::Critical),
        ] {
            manager.register_problem(Problem {
                id: id.to_string(),
                title: format!("Problem {id}"),
                description: "desc".to_string(),
                source: ProblemSource::TestFailure,
                severity,
                impact: "some".to_string(),
                reproduction_steps: vec![],
                expected_behavior: "ok".to_string(),
                actual_behavior: "fail".to_string(),
                discovered_at: chrono::Utc::now(),
                code_locations: vec![],
                logs: vec![],
            });
        }

        let pending = manager.pending_problems();
        assert_eq!(pending.len(), 2);
        let ids: Vec<&str> = pending.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"p3"));
        assert!(ids.contains(&"p4"));
    }

    #[test]
    fn test_pending_solutions_filters_by_status() {
        let mut manager = SelfImprovementManager::new();

        for (id, status) in [
            ("s1", SolutionStatus::Pending),
            ("s2", SolutionStatus::Approved),
            ("s3", SolutionStatus::InProgress),
            ("s4", SolutionStatus::Completed),
            ("s5", SolutionStatus::Approved),
        ] {
            manager.submit_solution(Solution {
                id: id.to_string(),
                problem_id: "p1".to_string(),
                title: format!("Solution {id}"),
                description: "desc".to_string(),
                implementation_steps: vec![],
                expected_outcome: "fix".to_string(),
                risks: vec![],
                effort_hours: 1.0,
                priority: 1,
                status,
                created_at: chrono::Utc::now(),
            });
        }

        let pending = manager.pending_solutions();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_record_improvement_multiple_lessons() {
        let mut manager = SelfImprovementManager::new();
        manager.register_problem(Problem {
            id: "p1".to_string(),
            title: "P".to_string(),
            description: "D".to_string(),
            source: ProblemSource::CodeQuality,
            severity: ProblemSeverity::Medium,
            impact: "I".to_string(),
            reproduction_steps: vec![],
            expected_behavior: "E".to_string(),
            actual_behavior: "A".to_string(),
            discovered_at: chrono::Utc::now(),
            code_locations: vec![],
            logs: vec![],
        });

        manager.record_improvement(ImprovementRecord {
            id: "r1".to_string(),
            problem_id: "p1".to_string(),
            solution_id: "s1".to_string(),
            result: ImplementationResult {
                success: true,
                changed_files: vec!["file.rs".to_string()],
                new_tests: vec!["test1".to_string()],
                lines_changed: 50,
                actual_hours: 2.0,
                issues_encountered: vec![],
            },
            verification: VerificationResult {
                tests_passed: true,
                performance_improved: false,
                problem_resolved: true,
                new_issues_introduced: false,
                details: "OK".to_string(),
            },
            lessons_learned: vec![
                "Lesson A".to_string(),
                "Lesson B".to_string(),
                "Lesson C".to_string(),
            ],
            completed_at: chrono::Utc::now(),
        });

        assert_eq!(manager.records.len(), 1);
        assert_eq!(manager.knowledge_base.len(), 3);
        assert!(manager
            .knowledge_base
            .iter()
            .all(|k| k.tags.contains(&"lesson".to_string())));
    }

    #[test]
    fn test_generate_report_with_data() {
        let mut manager = SelfImprovementManager::new();

        // Add a high-severity problem
        manager.register_problem(Problem {
            id: "p1".to_string(),
            title: "Critical Bug".to_string(),
            description: "desc".to_string(),
            source: ProblemSource::SystemMonitoring,
            severity: ProblemSeverity::Critical,
            impact: "major".to_string(),
            reproduction_steps: vec!["step1".to_string()],
            expected_behavior: "works".to_string(),
            actual_behavior: "crashes".to_string(),
            discovered_at: chrono::Utc::now(),
            code_locations: vec![],
            logs: vec![],
        });

        // Add approved solution
        manager.submit_solution(Solution {
            id: "s1".to_string(),
            problem_id: "p1".to_string(),
            title: "Fix".to_string(),
            description: "Fix it".to_string(),
            implementation_steps: vec![],
            expected_outcome: "works".to_string(),
            risks: vec![],
            effort_hours: 1.0,
            priority: 1,
            status: SolutionStatus::Approved,
            created_at: chrono::Utc::now(),
        });

        // Add a completed improvement
        manager.record_improvement(ImprovementRecord {
            id: "r1".to_string(),
            problem_id: "p1".to_string(),
            solution_id: "s1".to_string(),
            result: ImplementationResult {
                success: true,
                changed_files: vec![],
                new_tests: vec![],
                lines_changed: 0,
                actual_hours: 1.0,
                issues_encountered: vec![],
            },
            verification: VerificationResult {
                tests_passed: true,
                performance_improved: true,
                problem_resolved: true,
                new_issues_introduced: false,
                details: "Fixed".to_string(),
            },
            lessons_learned: vec!["Always test first".to_string()],
            completed_at: chrono::Utc::now(),
        });

        let report = manager.generate_report();
        assert!(report.contains("总问题数: 1"));
        assert!(report.contains("待处理高优问题: 1"));
        assert!(report.contains("总方案数: 1"));
        assert!(report.contains("待实施方案: 1"));
        assert!(report.contains("已完成改进: 1"));
        assert!(report.contains("成功改进: 1"));
        assert!(report.contains("经验教训条目: 1"));
    }

    #[test]
    fn test_problem_serialization_roundtrip() {
        let problem = Problem {
            id: "p1".to_string(),
            title: "Test".to_string(),
            description: "desc".to_string(),
            source: ProblemSource::SecurityVulnerability,
            severity: ProblemSeverity::Critical,
            impact: "high".to_string(),
            reproduction_steps: vec!["step1".to_string()],
            expected_behavior: "secure".to_string(),
            actual_behavior: "vulnerable".to_string(),
            discovered_at: chrono::Utc::now(),
            code_locations: vec![CodeLocation {
                file: "auth.rs".to_string(),
                line: Some(42),
                function: Some("verify".to_string()),
                snippet: Some("if token.is_empty()".to_string()),
            }],
            logs: vec!["error at auth".to_string()],
        };

        let json = serde_json::to_string(&problem).unwrap();
        let deserialized: Problem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "p1");
        assert_eq!(deserialized.severity, ProblemSeverity::Critical);
        assert_eq!(deserialized.code_locations.len(), 1);
        assert_eq!(deserialized.code_locations[0].line, Some(42));
    }

    #[test]
    fn test_solution_serialization_roundtrip() {
        let solution = Solution {
            id: "s1".to_string(),
            problem_id: "p1".to_string(),
            title: "Fix".to_string(),
            description: "Fix the bug".to_string(),
            implementation_steps: vec![ImplementationStep {
                order: 1,
                description: "Add validation".to_string(),
                files: vec!["auth.rs".to_string()],
                expected_changes: "Add input check".to_string(),
                verification: "Run tests".to_string(),
            }],
            expected_outcome: "Bug fixed".to_string(),
            risks: vec![Risk {
                description: "May break compat".to_string(),
                likelihood: 0.1,
                impact: "low".to_string(),
                mitigation: "Add feature flag".to_string(),
            }],
            effort_hours: 2.5,
            priority: 1,
            status: SolutionStatus::InProgress,
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&solution).unwrap();
        let deserialized: Solution = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.implementation_steps.len(), 1);
        assert_eq!(deserialized.risks[0].likelihood, 0.1);
    }

    #[test]
    fn test_empty_manager_report() {
        let manager = SelfImprovementManager::new();
        let report = manager.generate_report();
        assert!(report.contains("总问题数: 0"));
        assert!(report.contains("待处理高优问题: 0"));
        assert!(report.contains("总方案数: 0"));
        assert!(report.contains("待实施方案: 0"));
        assert!(report.contains("已完成改进: 0"));
        assert!(report.contains("成功改进: 0"));
        assert!(report.contains("经验教训条目: 0"));
    }

    #[test]
    fn test_default_trait() {
        let manager = SelfImprovementManager::default();
        assert!(manager.problems.is_empty());
        assert!(manager.solutions.is_empty());
        assert!(manager.records.is_empty());
        assert!(manager.knowledge_base.is_empty());
    }

    #[test]
    fn test_code_location_optional_fields() {
        let loc = CodeLocation {
            file: "test.rs".to_string(),
            line: None,
            function: None,
            snippet: None,
        };
        let json = serde_json::to_string(&loc).unwrap();
        let deserialized: CodeLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file, "test.rs");
        assert!(deserialized.line.is_none());
        assert!(deserialized.function.is_none());
        assert!(deserialized.snippet.is_none());
    }

    #[test]
    fn test_multiple_improvements_accumulate_knowledge() {
        let mut manager = SelfImprovementManager::new();

        for i in 0..5 {
            manager.record_improvement(ImprovementRecord {
                id: format!("r{i}"),
                problem_id: format!("p{i}"),
                solution_id: format!("s{i}"),
                result: ImplementationResult {
                    success: i % 2 == 0,
                    changed_files: vec![],
                    new_tests: vec![],
                    lines_changed: i * 10,
                    actual_hours: i as f64,
                    issues_encountered: vec![],
                },
                verification: VerificationResult {
                    tests_passed: true,
                    performance_improved: false,
                    problem_resolved: true,
                    new_issues_introduced: false,
                    details: "ok".to_string(),
                },
                lessons_learned: vec![format!("Lesson {i}a"), format!("Lesson {i}b")],
                completed_at: chrono::Utc::now(),
            });
        }

        assert_eq!(manager.records.len(), 5);
        assert_eq!(manager.knowledge_base.len(), 10);

        let report = manager.generate_report();
        assert!(report.contains("已完成改进: 5"));
        assert!(report.contains("成功改进: 3")); // 0, 2, 4
        assert!(report.contains("经验教训条目: 10"));
    }
}
