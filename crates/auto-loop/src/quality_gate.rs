//! Quality Gate — automated quality checks for code changes.
//!
//! Provides a pipeline of quality checks that must pass before code is accepted:
//! - Compilation check
//! - Test execution
//! - Lint (clippy)
//! - Format (rustfmt)
//! - Production unwrap detection
//! - Dead code detection
//! - Documentation coverage
//!
//! Inspired by:
//! - GitHub Actions CI/CD pipeline
//! - SonarQube quality gates
//! - Google's Trunk-Based Development

use serde::{Deserialize, Serialize};

/// Quality check type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckType {
    Compile,
    Test,
    Clippy,
    Rustfmt,
    NoProductionUnwrap,
    NoDeadCode,
    DocCoverage,
    SecurityAudit,
    LicenseCheck,
}

/// Result of a single quality check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub check_type: CheckType,
    pub passed: bool,
    pub message: String,
    pub details: Vec<String>,
    pub duration_ms: u64,
}

/// Quality gate configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateConfig {
    /// Checks to run.
    pub checks: Vec<CheckType>,
    /// Fail the gate if any check fails (strict mode).
    pub fail_fast: bool,
    /// Minimum test pass rate (0.0 - 1.0).
    pub min_test_pass_rate: f64,
    /// Maximum allowed production unwraps.
    pub max_unwraps: usize,
    /// Minimum documentation coverage percentage.
    pub min_doc_coverage: f64,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            checks: vec![
                CheckType::Compile,
                CheckType::Test,
                CheckType::Clippy,
                CheckType::NoProductionUnwrap,
            ],
            fail_fast: true,
            min_test_pass_rate: 1.0,
            max_unwraps: 0,
            min_doc_coverage: 80.0,
        }
    }
}

/// Overall quality gate result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResult {
    pub passed: bool,
    pub checks: Vec<CheckResult>,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub total_duration_ms: u64,
    pub score: f64,
}

/// Quality gate executor.
pub struct QualityGate {
    config: QualityGateConfig,
}

impl QualityGate {
    pub fn new(config: QualityGateConfig) -> Self {
        Self { config }
    }

    /// Evaluate compilation result.
    pub fn check_compile(&self, errors: usize, warnings: usize) -> CheckResult {
        CheckResult {
            check_type: CheckType::Compile,
            passed: errors == 0,
            message: if errors == 0 {
                format!("Compilation passed ({} warnings)", warnings)
            } else {
                format!("Compilation failed with {} errors", errors)
            },
            details: Vec::new(),
            duration_ms: 0,
        }
    }

    /// Evaluate test results.
    pub fn check_tests(&self, passed: usize, failed: usize, ignored: usize) -> CheckResult {
        let total = passed + failed + ignored;
        let pass_rate = if total > 0 {
            passed as f64 / total as f64
        } else {
            1.0
        };
        CheckResult {
            check_type: CheckType::Test,
            passed: failed == 0 && pass_rate >= self.config.min_test_pass_rate,
            message: format!(
                "Tests: {} passed, {} failed, {} ignored ({:.1}% pass rate)",
                passed,
                failed,
                ignored,
                pass_rate * 100.0
            ),
            details: Vec::new(),
            duration_ms: 0,
        }
    }

    /// Evaluate clippy results.
    pub fn check_clippy(&self, errors: usize, warnings: usize) -> CheckResult {
        CheckResult {
            check_type: CheckType::Clippy,
            passed: errors == 0,
            message: if errors == 0 {
                format!("Clippy passed ({} warnings)", warnings)
            } else {
                format!("Clippy found {} errors", errors)
            },
            details: Vec::new(),
            duration_ms: 0,
        }
    }

    /// Check for production unwraps.
    pub fn check_unwraps(&self, unwrap_count: usize) -> CheckResult {
        CheckResult {
            check_type: CheckType::NoProductionUnwrap,
            passed: unwrap_count <= self.config.max_unwraps,
            message: format!(
                "Production unwraps: {} (max allowed: {})",
                unwrap_count, self.config.max_unwraps
            ),
            details: Vec::new(),
            duration_ms: 0,
        }
    }

    /// Evaluate rustfmt results.
    pub fn check_rustfmt(&self, unformatted_files: usize) -> CheckResult {
        CheckResult {
            check_type: CheckType::Rustfmt,
            passed: unformatted_files == 0,
            message: if unformatted_files == 0 {
                "All files formatted".to_string()
            } else {
                format!("{} files need formatting", unformatted_files)
            },
            details: Vec::new(),
            duration_ms: 0,
        }
    }

    /// Run all configured checks and aggregate results.
    pub fn evaluate(&self, results: Vec<CheckResult>) -> QualityGateResult {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let score = if total > 0 {
            passed as f64 / total as f64 * 100.0
        } else {
            100.0
        };
        let gate_passed = if self.config.fail_fast {
            results.iter().all(|r| r.passed)
        } else {
            passed > failed
        };

        QualityGateResult {
            passed: gate_passed,
            checks: results,
            total_checks: total,
            passed_checks: passed,
            failed_checks: failed,
            total_duration_ms: total_duration,
            score,
        }
    }
}

impl Default for QualityGate {
    fn default() -> Self {
        Self::new(QualityGateConfig::default())
    }
}

/// Sprint tracking — lightweight sprint progress management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintTracker {
    pub sprint_name: String,
    pub goals: Vec<SprintGoal>,
    pub started_at_ms: u64,
    pub target_end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintGoal {
    pub goal_id: String,
    pub description: String,
    pub status: GoalStatus,
    pub progress_percent: f64,
    pub tasks_total: usize,
    pub tasks_done: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GoalStatus {
    NotStarted,
    InProgress,
    Completed,
    Blocked,
    Cancelled,
}

impl SprintTracker {
    pub fn new(name: impl Into<String>, duration_days: u32) -> Self {
        let now = now_ms();
        Self {
            sprint_name: name.into(),
            goals: Vec::new(),
            started_at_ms: now,
            target_end_ms: now + (duration_days as u64 * 86400 * 1000),
        }
    }

    pub fn add_goal(&mut self, goal: SprintGoal) {
        self.goals.push(goal);
    }

    pub fn update_progress(&mut self, goal_id: &str, tasks_done: usize) {
        if let Some(goal) = self.goals.iter_mut().find(|g| g.goal_id == goal_id) {
            goal.tasks_done = tasks_done;
            goal.progress_percent = if goal.tasks_total > 0 {
                tasks_done as f64 / goal.tasks_total as f64 * 100.0
            } else {
                0.0
            };
            goal.status = if tasks_done >= goal.tasks_total {
                GoalStatus::Completed
            } else if tasks_done > 0 {
                GoalStatus::InProgress
            } else {
                GoalStatus::NotStarted
            };
        }
    }

    pub fn overall_progress(&self) -> f64 {
        if self.goals.is_empty() {
            return 0.0;
        }
        let total: f64 = self.goals.iter().map(|g| g.progress_percent).sum();
        total / self.goals.len() as f64
    }

    pub fn completed_goals(&self) -> usize {
        self.goals
            .iter()
            .filter(|g| g.status == GoalStatus::Completed)
            .count()
    }

    pub fn is_complete(&self) -> bool {
        self.goals
            .iter()
            .all(|g| g.status == GoalStatus::Completed || g.status == GoalStatus::Cancelled)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_gate_all_pass() {
        let gate = QualityGate::default();
        let results = vec![
            gate.check_compile(0, 0),
            gate.check_tests(100, 0, 0),
            gate.check_clippy(0, 0),
            gate.check_unwraps(0),
        ];
        let result = gate.evaluate(results);
        assert!(result.passed);
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_quality_gate_compile_fail() {
        let gate = QualityGate::default();
        let results = vec![gate.check_compile(3, 0), gate.check_tests(100, 0, 0)];
        let result = gate.evaluate(results);
        assert!(!result.passed);
    }

    #[test]
    fn test_quality_gate_unwrap_exceeded() {
        let gate = QualityGate::default();
        let results = vec![gate.check_unwraps(5)];
        let result = gate.evaluate(results);
        assert!(!result.passed);
    }

    #[test]
    fn test_sprint_tracker() {
        let mut sprint = SprintTracker::new("Sprint 1", 14);
        sprint.add_goal(SprintGoal {
            goal_id: "g1".into(),
            description: "Complete feature X".into(),
            status: GoalStatus::NotStarted,
            progress_percent: 0.0,
            tasks_total: 10,
            tasks_done: 0,
        });
        sprint.update_progress("g1", 5);
        assert_eq!(sprint.overall_progress(), 50.0);
        assert!(!sprint.is_complete());

        sprint.update_progress("g1", 10);
        assert!(sprint.is_complete());
    }

    #[test]
    fn test_test_pass_rate() {
        let gate = QualityGate::new(QualityGateConfig {
            min_test_pass_rate: 0.95,
            ..Default::default()
        });
        let result = gate.check_tests(95, 5, 0);
        assert!(!result.passed); // 95% < 95% threshold? Actually 95/100 = 0.95 >= 0.95

        let result2 = gate.check_tests(90, 10, 0);
        assert!(!result2.passed); // 90% < 95%
    }
}
