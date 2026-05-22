//! 质量评分器
//!
//! 对模块/workflow/插件自动打分，从测试覆盖/文档完整/错误处理/性能/安全维度评估。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 评分维度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScoreDimension {
    TestCoverage,  // 测试覆盖
    Documentation, // 文档完整
    ErrorHandling, // 错误处理
    Performance,   // 性能
    Security,      // 安全
}

impl std::fmt::Display for ScoreDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScoreDimension::TestCoverage => write!(f, "TestCoverage"),
            ScoreDimension::Documentation => write!(f, "Documentation"),
            ScoreDimension::ErrorHandling => write!(f, "ErrorHandling"),
            ScoreDimension::Performance => write!(f, "Performance"),
            ScoreDimension::Security => write!(f, "Security"),
        }
    }
}

/// 单项评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub dimension: ScoreDimension,
    pub score: f64, // 0-100
    pub details: String,
}

/// 评分报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub target_name: String,
    pub target_type: String,
    pub overall_score: f64,
    pub dimension_scores: Vec<DimensionScore>,
    pub suggestions: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl QualityReport {
    pub fn average_score(&self) -> f64 {
        if self.dimension_scores.is_empty() {
            return 0.0;
        }
        self.dimension_scores.iter().map(|d| d.score).sum::<f64>()
            / self.dimension_scores.len() as f64
    }
}

/// 质量评分器
pub struct QualityScorer {
    benchmarks: HashMap<String, HashMap<ScoreDimension, f64>>,
}

impl Default for QualityScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityScorer {
    pub fn new() -> Self {
        Self {
            benchmarks: HashMap::new(),
        }
    }

    /// 记录基准分数
    pub fn set_benchmark(&mut self, target: &str, dimension: ScoreDimension, score: f64) {
        self.benchmarks
            .entry(target.to_string())
            .or_default()
            .insert(dimension, score);
    }

    /// 评估模块质量
    pub fn score_module(&self, name: &str, metrics: &ModuleMetrics) -> QualityReport {
        let mut dimension_scores = Vec::new();
        let mut suggestions = Vec::new();

        // 测试覆盖评分
        let test_score =
            (metrics.test_count as f64 / metrics.code_lines.max(1) as f64 * 100.0).min(100.0);
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::TestCoverage,
            score: test_score,
            details: format!(
                "{}/{} lines covered by {} tests",
                metrics.test_count, metrics.code_lines, metrics.test_count
            ),
        });
        if test_score < 70.0 {
            suggestions.push("Increase test coverage to improve reliability".to_string());
        }

        // 文档评分
        let doc_score = if metrics.has_docs { 100.0 } else { 40.0 };
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Documentation,
            score: doc_score,
            details: if metrics.has_docs {
                "Documentation complete".to_string()
            } else {
                "Missing documentation".to_string()
            },
        });
        if !metrics.has_docs {
            suggestions.push("Add documentation to improve maintainability".to_string());
        }

        // 错误处理评分
        let error_score = (metrics.error_handling_score as f64).min(100.0);
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::ErrorHandling,
            score: error_score,
            details: format!("Error handling score: {}", metrics.error_handling_score),
        });
        if error_score < 60.0 {
            suggestions.push("Improve error handling with proper error types".to_string());
        }

        // 性能评分
        let perf_score = (100.0 - metrics.avg_latency_ms.min(1000) as f64 / 10.0).max(0.0);
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Performance,
            score: perf_score,
            details: format!("Avg latency: {}ms", metrics.avg_latency_ms),
        });
        if metrics.avg_latency_ms > 500 {
            suggestions.push("Optimize performance to reduce latency".to_string());
        }

        // 安全评分
        let sec_score = if metrics.has_security_review {
            100.0
        } else {
            50.0
        };
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Security,
            score: sec_score,
            details: if metrics.has_security_review {
                "Security review completed".to_string()
            } else {
                "Security review needed".to_string()
            },
        });
        if !metrics.has_security_review {
            suggestions.push("Conduct security review before production".to_string());
        }

        let overall =
            dimension_scores.iter().map(|d| d.score).sum::<f64>() / dimension_scores.len() as f64;

        QualityReport {
            target_name: name.to_string(),
            target_type: "module".to_string(),
            overall_score: overall,
            dimension_scores,
            suggestions,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 评估 Workflow 质量
    pub fn score_workflow(&self, name: &str, metrics: &WorkflowMetrics) -> QualityReport {
        let mut dimension_scores = Vec::new();
        let mut suggestions = Vec::new();

        // 测试覆盖
        let test_score = metrics.test_nodes as f64 / metrics.total_nodes.max(1) as f64 * 100.0;
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::TestCoverage,
            score: test_score,
            details: format!(
                "{}/{} nodes tested",
                metrics.test_nodes, metrics.total_nodes
            ),
        });

        // 文档
        let doc_score = if metrics.has_description { 100.0 } else { 30.0 };
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Documentation,
            score: doc_score,
            details: if metrics.has_description {
                "Description provided".to_string()
            } else {
                "Missing workflow description".to_string()
            },
        });

        // 错误处理
        let error_score = if metrics.has_error_handling {
            90.0
        } else {
            40.0
        };
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::ErrorHandling,
            score: error_score,
            details: if metrics.has_error_handling {
                "Error handling configured".to_string()
            } else {
                "No error handling".to_string()
            },
        });

        // 性能
        let perf_score = (100.0 - metrics.estimated_duration_ms.min(10000) as f64 / 100.0).max(0.0);
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Performance,
            score: perf_score,
            details: format!("Est. duration: {}ms", metrics.estimated_duration_ms),
        });

        // 安全
        let sec_score = if metrics.has_input_validation {
            85.0
        } else {
            50.0
        };
        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Security,
            score: sec_score,
            details: if metrics.has_input_validation {
                "Input validation present".to_string()
            } else {
                "Input validation missing".to_string()
            },
        });

        let overall =
            dimension_scores.iter().map(|d| d.score).sum::<f64>() / dimension_scores.len() as f64;

        QualityReport {
            target_name: name.to_string(),
            target_type: "workflow".to_string(),
            overall_score: overall,
            dimension_scores,
            suggestions,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 评估插件质量
    pub fn score_plugin(&self, name: &str, metrics: &PluginMetrics) -> QualityReport {
        let mut dimension_scores = Vec::new();
        let mut suggestions = Vec::new();

        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::TestCoverage,
            score: metrics.test_coverage,
            details: format!("Test coverage: {}%", metrics.test_coverage),
        });

        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Documentation,
            score: metrics.doc_completeness,
            details: format!("Documentation: {}%", metrics.doc_completeness),
        });

        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::ErrorHandling,
            score: metrics.error_handling_score,
            details: format!("Error handling: {}", metrics.error_handling_score),
        });

        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Performance,
            score: metrics.performance_score,
            details: format!("Performance score: {}", metrics.performance_score),
        });

        dimension_scores.push(DimensionScore {
            dimension: ScoreDimension::Security,
            score: metrics.security_score,
            details: format!("Security score: {}", metrics.security_score),
        });

        let overall =
            dimension_scores.iter().map(|d| d.score).sum::<f64>() / dimension_scores.len() as f64;

        QualityReport {
            target_name: name.to_string(),
            target_type: "plugin".to_string(),
            overall_score: overall,
            dimension_scores,
            suggestions,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// 模块指标
#[derive(Debug, Default)]
pub struct ModuleMetrics {
    pub test_count: usize,
    pub code_lines: usize,
    pub has_docs: bool,
    pub error_handling_score: u8,
    pub avg_latency_ms: u64,
    pub has_security_review: bool,
}

/// Workflow 指标
#[derive(Debug, Default)]
pub struct WorkflowMetrics {
    pub total_nodes: usize,
    pub test_nodes: usize,
    pub has_description: bool,
    pub has_error_handling: bool,
    pub estimated_duration_ms: u64,
    pub has_input_validation: bool,
}

/// Plugin 指标
#[derive(Debug, Default)]
pub struct PluginMetrics {
    pub test_coverage: f64,
    pub doc_completeness: f64,
    pub error_handling_score: f64,
    pub performance_score: f64,
    pub security_score: f64,
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_scorer_score_module() {
        let scorer = QualityScorer::new();
        let metrics = ModuleMetrics {
            test_count: 50,
            code_lines: 100,
            has_docs: true,
            error_handling_score: 80,
            avg_latency_ms: 100,
            has_security_review: true,
        };
        let report = scorer.score_module("test-module", &metrics);
        assert_eq!(report.target_name, "test-module");
        assert_eq!(report.target_type, "module");
        assert_eq!(report.dimension_scores.len(), 5);
    }

    #[test]
    fn test_quality_scorer_score_workflow() {
        let scorer = QualityScorer::new();
        let metrics = WorkflowMetrics {
            total_nodes: 10,
            test_nodes: 8,
            has_description: true,
            has_error_handling: true,
            estimated_duration_ms: 500,
            has_input_validation: true,
        };
        let report = scorer.score_workflow("test-workflow", &metrics);
        assert_eq!(report.target_type, "workflow");
        assert!(!report.dimension_scores.is_empty());
    }

    #[test]
    fn test_quality_scorer_score_plugin() {
        let scorer = QualityScorer::new();
        let metrics = PluginMetrics {
            test_coverage: 85.0,
            doc_completeness: 90.0,
            error_handling_score: 80.0,
            performance_score: 75.0,
            security_score: 95.0,
        };
        let report = scorer.score_plugin("test-plugin", &metrics);
        assert_eq!(report.target_type, "plugin");
        assert_eq!(report.overall_score, 85.0);
    }

    #[test]
    fn test_quality_report_average_score() {
        let report = QualityReport {
            target_name: "test".to_string(),
            target_type: "module".to_string(),
            overall_score: 80.0,
            dimension_scores: vec![
                DimensionScore {
                    dimension: ScoreDimension::TestCoverage,
                    score: 90.0,
                    details: "".to_string(),
                },
                DimensionScore {
                    dimension: ScoreDimension::Documentation,
                    score: 70.0,
                    details: "".to_string(),
                },
            ],
            suggestions: vec![],
            timestamp: chrono::Utc::now(),
        };
        assert_eq!(report.average_score(), 80.0);
    }

    #[test]
    fn test_score_dimension_display() {
        assert_eq!(ScoreDimension::TestCoverage.to_string(), "TestCoverage");
        assert_eq!(ScoreDimension::Documentation.to_string(), "Documentation");
        assert_eq!(ScoreDimension::ErrorHandling.to_string(), "ErrorHandling");
        assert_eq!(ScoreDimension::Performance.to_string(), "Performance");
        assert_eq!(ScoreDimension::Security.to_string(), "Security");
    }

    #[test]
    fn test_quality_scorer_set_benchmark() {
        let mut scorer = QualityScorer::new();
        scorer.set_benchmark("test-target", ScoreDimension::TestCoverage, 95.0);
        assert!(scorer.benchmarks.contains_key("test-target"));
    }

    #[test]
    fn test_quality_scorer_suggestions_for_low_coverage() {
        let scorer = QualityScorer::new();
        let metrics = ModuleMetrics {
            test_count: 10,
            code_lines: 100,
            has_docs: false,
            error_handling_score: 30,
            avg_latency_ms: 1000,
            has_security_review: false,
        };
        let report = scorer.score_module("low-quality-module", &metrics);
        assert!(!report.suggestions.is_empty());
    }
}
