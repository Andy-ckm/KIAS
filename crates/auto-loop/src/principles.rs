//! 开发原则定义 — 钱学森系统工程 + 第一性原则 + 四步开发法
//!
//! 将顶层方法论编码为可执行的验证门禁，而非仅文档。
//! 每个原则对应一个可验证的断言。

use serde::{Deserialize, Serialize};

/// 钱学森系统工程七大原则
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum XuesenPrinciple {
    /// 整体性 — 系统行为由整体决定，非局部最优
    Holism,
    /// 综合集成 — 多学科知识融合
    MetaSynthesis,
    /// 反馈控制 — 感知→决策→执行→反馈 闭环
    FeedbackControl,
    /// 层次分解 — 复杂系统分解为可管理层级
    HierarchyDecomposition,
    /// 鲁棒性 — 系统在扰动下保持功能
    Robustness,
    /// 可观测性 — 系统状态可被外部观测
    Observability,
    /// 工程化 — 可测量、可验证、可重复
    Engineering,
}

/// 第一性原则
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FirstPrinciple {
    /// 回归本质 — 从物理定律出发，不从类比出发
    BackToBasics,
    /// 质疑假设 — 挑战"行业惯例"和"一直以来的做法"
    QuestionAssumptions,
    /// 论文/源码支撑 — 每个功能必须有学术或开源依据
    EvidenceBased,
    /// 做减法 — 去掉不必要的复杂性
    SubtractSimplify,
}

/// 四步开发法阶段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DevStep {
    /// Step 1: 评估 — 这个功能真的需要吗？
    Evaluate,
    /// Step 2: 审视 — 现有系统能否覆盖？
    Inspect,
    /// Step 3: 方案 — 怎么做、做到什么程度
    Plan,
    /// Step 4: 开发 — 按方案执行，不超范围
    Implement,
}

/// 原则验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipleCheckResult {
    /// 原则名称
    pub principle: String,
    /// 是否通过
    pub passed: bool,
    /// 检查详情
    pub details: String,
    /// 建议的修复措施
    pub remediation: Option<String>,
}

/// 四步开发法验证器
///
/// 确保每个功能经历完整的评估→审视→方案→开发流程
pub struct FourStepValidator;

impl FourStepValidator {
    /// 验证一个功能是否完成了四步开发法
    pub fn validate(
        has_evaluation: bool,
        has_inspection: bool,
        has_plan: bool,
        has_implementation: bool,
    ) -> Vec<PrincipleCheckResult> {
        vec![
            PrincipleCheckResult {
                principle: "Step 1: 评估".to_string(),
                passed: has_evaluation,
                details: if has_evaluation {
                    "已完成需求评估".to_string()
                } else {
                    "未评估即开始开发 — 违反四步开发法铁律".to_string()
                },
                remediation: if !has_evaluation {
                    Some("先回答：这个功能解决什么真实问题？不做会怎样？".to_string())
                } else {
                    None
                },
            },
            PrincipleCheckResult {
                principle: "Step 2: 审视".to_string(),
                passed: has_inspection,
                details: if has_inspection {
                    "已审视现有系统".to_string()
                } else {
                    "未审视即动手 — 可能重复造轮子".to_string()
                },
                remediation: if !has_inspection {
                    Some("扫描 codebase，量化已有能力，找出差距".to_string())
                } else {
                    None
                },
            },
            PrincipleCheckResult {
                principle: "Step 3: 方案".to_string(),
                passed: has_plan,
                details: if has_plan {
                    "已制定实施方案".to_string()
                } else {
                    "无方案直接编码 — 必然返工".to_string()
                },
                remediation: if !has_plan {
                    Some("写方案文档：目标、范围、接口、测试策略".to_string())
                } else {
                    None
                },
            },
            PrincipleCheckResult {
                principle: "Step 4: 开发".to_string(),
                passed: has_implementation,
                details: if has_implementation {
                    "已按方案执行".to_string()
                } else {
                    "尚未实施".to_string()
                },
                remediation: None,
            },
        ]
    }

    /// 快速检查：是否所有步骤都通过
    pub fn all_passed(
        has_evaluation: bool,
        has_inspection: bool,
        has_plan: bool,
        has_implementation: bool,
    ) -> bool {
        has_evaluation && has_inspection && has_plan && has_implementation
    }
}

/// 质量门禁
///
/// 集成验证器结果和原则检查，形成统一的质量关口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGate {
    /// 编译是否通过
    pub compilation_passed: bool,
    /// 测试是否通过
    pub tests_passed: bool,
    /// Clippy 是否零警告
    pub clippy_clean: bool,
    /// 格式是否规范
    pub format_clean: bool,
    /// 原则检查结果
    pub principle_checks: Vec<PrincipleCheckResult>,
    /// 总体是否通过
    pub overall_passed: bool,
}

impl QualityGate {
    /// 基于验证器结果构建质量门禁
    pub fn from_verifier_results(
        compilation_passed: bool,
        tests_passed: bool,
        clippy_clean: bool,
        format_clean: bool,
        principle_checks: Vec<PrincipleCheckResult>,
    ) -> Self {
        let all_principles_passed = principle_checks.iter().all(|p| p.passed);
        let overall_passed = compilation_passed
            && tests_passed
            && clippy_clean
            && format_clean
            && all_principles_passed;

        Self {
            compilation_passed,
            tests_passed,
            clippy_clean,
            format_clean,
            principle_checks,
            overall_passed,
        }
    }

    /// 生成质量报告
    pub fn report(&self) -> String {
        let mut r = String::from("# 质量门禁报告\n\n");
        r.push_str(&format!(
            "- 编译: {}\n",
            if self.compilation_passed {
                "✅"
            } else {
                "❌"
            }
        ));
        r.push_str(&format!(
            "- 测试: {}\n",
            if self.tests_passed { "✅" } else { "❌" }
        ));
        r.push_str(&format!(
            "- Clippy: {}\n",
            if self.clippy_clean { "✅" } else { "❌" }
        ));
        r.push_str(&format!(
            "- 格式: {}\n",
            if self.format_clean { "✅" } else { "❌" }
        ));
        r.push_str("\n## 原则检查\n\n");
        for check in &self.principle_checks {
            r.push_str(&format!(
                "- {} {}: {}\n",
                if check.passed { "✅" } else { "❌" },
                check.principle,
                check.details
            ));
            if let Some(ref rem) = check.remediation {
                r.push_str(&format!("  → 建议: {}\n", rem));
            }
        }
        r.push_str(&format!(
            "\n## 总体: {}\n",
            if self.overall_passed {
                "✅ 通过"
            } else {
                "❌ 未通过"
            }
        ));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xuesen_principles_all_defined() {
        let principles = [
            XuesenPrinciple::Holism,
            XuesenPrinciple::MetaSynthesis,
            XuesenPrinciple::FeedbackControl,
            XuesenPrinciple::HierarchyDecomposition,
            XuesenPrinciple::Robustness,
            XuesenPrinciple::Observability,
            XuesenPrinciple::Engineering,
        ];
        assert_eq!(principles.len(), 7);
    }

    #[test]
    fn test_first_principles_all_defined() {
        let principles = [
            FirstPrinciple::BackToBasics,
            FirstPrinciple::QuestionAssumptions,
            FirstPrinciple::EvidenceBased,
            FirstPrinciple::SubtractSimplify,
        ];
        assert_eq!(principles.len(), 4);
    }

    #[test]
    fn test_four_step_all_pass() {
        let results = FourStepValidator::validate(true, true, true, true);
        assert!(results.iter().all(|r| r.passed));
        assert!(FourStepValidator::all_passed(true, true, true, true));
    }

    #[test]
    fn test_four_step_skip_evaluate() {
        let results = FourStepValidator::validate(false, true, true, true);
        assert!(!results[0].passed);
        assert!(results[0].remediation.is_some());
        assert!(!FourStepValidator::all_passed(false, true, true, true));
    }

    #[test]
    fn test_four_step_skip_plan() {
        let results = FourStepValidator::validate(true, true, false, true);
        assert!(!results[2].passed);
        assert!(results[2].remediation.as_ref().unwrap().contains("方案"));
    }

    #[test]
    fn test_quality_gate_all_pass() {
        let gate = QualityGate::from_verifier_results(
            true,
            true,
            true,
            true,
            FourStepValidator::validate(true, true, true, true),
        );
        assert!(gate.overall_passed);
    }

    #[test]
    fn test_quality_gate_clippy_fail() {
        let gate = QualityGate::from_verifier_results(
            true,
            true,
            false,
            true,
            FourStepValidator::validate(true, true, true, true),
        );
        assert!(!gate.overall_passed);
    }

    #[test]
    fn test_quality_gate_report() {
        let gate = QualityGate::from_verifier_results(
            true,
            true,
            true,
            true,
            FourStepValidator::validate(true, true, true, true),
        );
        let report = gate.report();
        assert!(report.contains("总体: ✅ 通过"));
    }

    #[test]
    fn test_dev_step_variants() {
        assert_eq!(DevStep::Evaluate, DevStep::Evaluate);
        assert_ne!(DevStep::Evaluate, DevStep::Implement);
    }
}
