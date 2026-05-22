//! # Sprint Progress Auto-Tracker
//!
//! Tracks sprint completion rate, test coverage, and quality metrics.
//!
//! ## Core types
//!
//! - [`SprintTracker`] — tracks task completion and quality metrics
//! - [`SprintProgress`] — progress report for a sprint
//! - [`BurndownData`] — data points for burndown chart generation
//! - [`TaskStatus`] — enum for individual task states

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use time::{Date, OffsetDateTime};

/// Status of an individual sprint task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskStatus {
    /// Task has not been started yet.
    #[default]
    Todo,
    /// Task is currently being worked on.
    InProgress,
    /// Task is complete.
    Done,
    /// Task was blocked by an external dependency.
    Blocked,
    /// Task was removed from the sprint.
    Removed,
}

/// A single task within a sprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintTask {
    pub id: String,
    pub title: String,
    pub story_points: u32,
    pub status: TaskStatus,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub assignee: Option<String>,
}

impl SprintTask {
    pub fn new(id: &str, title: &str, story_points: u32) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            story_points,
            status: TaskStatus::Todo,
            created_at: OffsetDateTime::now_utc(),
            completed_at: None,
            assignee: None,
        }
    }

    pub fn mark_done(&mut self) {
        self.status = TaskStatus::Done;
        self.completed_at = Some(OffsetDateTime::now_utc());
    }

    pub fn is_complete(&self) -> bool {
        self.status == TaskStatus::Done
    }
}

/// A sprint with a start and end date.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    pub id: String,
    pub name: String,
    pub start_date: Date,
    pub end_date: Date,
    pub tasks: Vec<SprintTask>,
}

impl Sprint {
    pub fn new(id: &str, name: &str, start_date: Date, end_date: Date) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            start_date,
            end_date,
            tasks: Vec::new(),
        }
    }

    pub fn total_story_points(&self) -> u32 {
        self.tasks.iter().map(|t| t.story_points).sum()
    }

    pub fn completed_story_points(&self) -> u32 {
        self.tasks
            .iter()
            .filter(|t| t.is_complete())
            .map(|t| t.story_points)
            .sum()
    }

    pub fn completion_rate(&self) -> f64 {
        let total = self.total_story_points();
        if total == 0 {
            return 0.0;
        }
        self.completed_story_points() as f64 / total as f64
    }

    pub fn days_remaining(&self) -> i64 {
        let today = OffsetDateTime::now_utc().date();
        (self.end_date - today).whole_days()
    }

    pub fn add_task(&mut self, task: SprintTask) {
        self.tasks.push(task);
    }
}

/// Data point for burndown chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurndownPoint {
    /// Day offset from sprint start (0 = first day).
    pub day: i64,
    /// Total remaining story points at this point.
    pub remaining_points: u32,
    /// Ideal remaining points for this day.
    pub ideal_points: u32,
    /// Whether this is an actual data point or projected.
    pub is_actual: bool,
}

/// Data needed to render a burndown chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurndownData {
    pub sprint_id: String,
    pub points: Vec<BurndownPoint>,
    pub total_points: u32,
    pub days: i64,
}

impl BurndownData {
    /// Compute ideal burndown line.
    pub fn ideal_at_day(&self, day: i64) -> u32 {
        let total = self.total_points as f64;
        let days = self.days.max(1) as f64;
        let daily_burn = total / days;
        (total - (daily_burn * day as f64)).max(0.0) as u32
    }

    /// Actual remaining points at a given day.
    pub fn actual_at_day(&self, day: i64) -> Option<u32> {
        self.points
            .iter()
            .find(|p| p.day == day && p.is_actual)
            .map(|p| p.remaining_points)
    }
}

/// Test coverage data for a crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageData {
    pub crate_name: String,
    pub lines_covered: u32,
    pub lines_total: u32,
    pub functions_covered: u32,
    pub functions_total: u32,
    pub branch_coverage_pct: f64,
}

impl CoverageData {
    pub fn line_coverage_pct(&self) -> f64 {
        if self.lines_total == 0 {
            return 0.0;
        }
        self.lines_covered as f64 / self.lines_total as f64 * 100.0
    }

    pub fn function_coverage_pct(&self) -> f64 {
        if self.functions_total == 0 {
            return 0.0;
        }
        self.functions_covered as f64 / self.functions_total as f64 * 100.0
    }
}

/// Quality gate result for a single check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGate {
    pub name: String,
    pub passed: bool,
    pub threshold: f64,
    pub actual_value: f64,
    pub message: String,
}

/// Summary of all quality gates for a sprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub clippy_warnings: u32,
    pub fmt_violations: u32,
    pub test_pass_rate: f64,
    pub test_count: u32,
    pub gates: Vec<QualityGate>,
}

impl QualityMetrics {
    pub fn all_gates_passed(&self) -> bool {
        self.gates.iter().all(|g| g.passed)
    }
}

/// Full sprint progress report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintProgress {
    pub sprint_id: String,
    pub sprint_name: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub total_story_points: u32,
    pub completed_story_points: u32,
    pub completion_rate_pct: f64,
    pub days_remaining: i64,
    pub days_total: i64,
    pub quality_metrics: QualityMetrics,
    pub burndown: BurndownData,
    pub coverage_data: Vec<CoverageData>,
    pub generated_at: OffsetDateTime,
}

impl SprintProgress {
    pub fn is_on_track(&self) -> bool {
        let expected_rate = 1.0 - (self.days_remaining as f64 / self.days_total.max(1) as f64);
        self.completion_rate_pct >= expected_rate * 0.8
    }

    pub fn summary(&self) -> String {
        format!(
            "Sprint '{}': {}/{} tasks, {:.1}% complete, {} days left, {}",
            self.sprint_name,
            self.completed_tasks,
            self.total_tasks,
            self.completion_rate_pct * 100.0,
            self.days_remaining,
            if self.is_on_track() {
                "ON TRACK"
            } else {
                "AT RISK"
            }
        )
    }
}

/// Tracks sprint progress over time.
pub struct SprintTracker {
    current_sprint: Option<Sprint>,
    historical_sprints: Vec<Sprint>,
    burndown_history: VecDeque<BurndownPoint>,
    coverage_history: Vec<CoverageData>,
    quality_history: Vec<QualityMetrics>,
}

impl Default for SprintTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SprintTracker {
    pub fn new() -> Self {
        Self {
            current_sprint: None,
            historical_sprints: Vec::new(),
            burndown_history: VecDeque::new(),
            coverage_history: Vec::new(),
            quality_history: Vec::new(),
        }
    }

    /// Start a new sprint.
    pub fn start_sprint(&mut self, id: &str, name: &str, start_date: Date, end_date: Date) {
        if let Some(cur) = self.current_sprint.take() {
            self.historical_sprints.push(cur);
        }
        self.current_sprint = Some(Sprint::new(id, name, start_date, end_date));
        self.burndown_history.clear();
    }

    /// Add a task to the current sprint.
    pub fn add_task(&mut self, task: SprintTask) -> KiasResult<()> {
        let sprint = self
            .current_sprint
            .as_mut()
            .ok_or_else(|| KiasError::Config("No active sprint to add task to".to_string()))?;
        sprint.add_task(task);
        Ok(())
    }

    /// Update task status by ID.
    pub fn update_task_status(&mut self, task_id: &str, status: TaskStatus) -> KiasResult<()> {
        let sprint = self
            .current_sprint
            .as_mut()
            .ok_or_else(|| KiasError::Config("No active sprint".to_string()))?;
        let task = sprint
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KiasError::Config(format!("Task '{task_id}' not found")))?;
        if status == TaskStatus::Done {
            task.mark_done();
        } else {
            task.status = status;
        }
        Ok(())
    }

    /// Record a burndown data point for today.
    pub fn record_burndown(&mut self, remaining_points: u32) -> KiasResult<()> {
        let sprint = self
            .current_sprint
            .as_ref()
            .ok_or_else(|| KiasError::Config("No active sprint".to_string()))?;
        let today = OffsetDateTime::now_utc().date();
        let day = (today - sprint.start_date).whole_days();
        let total = sprint.total_story_points();
        let days = (sprint.end_date - sprint.start_date).whole_days().max(1);
        let ideal = if days > 0 {
            let daily = total as f64 / days as f64;
            (total as f64 - (daily * day as f64)).max(0.0) as u32
        } else {
            0
        };
        self.burndown_history.push_back(BurndownPoint {
            day,
            remaining_points,
            ideal_points: ideal,
            is_actual: true,
        });
        Ok(())
    }

    /// Record coverage data for a crate.
    pub fn record_coverage(&mut self, coverage: CoverageData) {
        self.coverage_history.push(coverage);
    }

    /// Record quality metrics snapshot.
    pub fn record_quality(&mut self, metrics: QualityMetrics) {
        self.quality_history.push(metrics);
    }

    /// Generate a full progress report for the current sprint.
    pub fn generate_progress_report(&self) -> KiasResult<SprintProgress> {
        let sprint = self
            .current_sprint
            .as_ref()
            .ok_or_else(|| KiasError::Config("No active sprint".to_string()))?;

        let completed = sprint.tasks.iter().filter(|t| t.is_complete()).count();
        let total_sp = sprint.total_story_points();
        let completed_sp = sprint.completed_story_points();
        let days_total = (sprint.end_date - sprint.start_date).whole_days();

        // Build burndown data
        let burndown_points: Vec<BurndownPoint> = self.burndown_history.iter().cloned().collect();
        let burndown = BurndownData {
            sprint_id: sprint.id.clone(),
            points: burndown_points,
            total_points: total_sp,
            days: days_total,
        };

        let quality = self
            .quality_history
            .last()
            .cloned()
            .unwrap_or_else(|| QualityMetrics {
                clippy_warnings: 0,
                fmt_violations: 0,
                test_pass_rate: 1.0,
                test_count: 0,
                gates: Vec::new(),
            });

        Ok(SprintProgress {
            sprint_id: sprint.id.clone(),
            sprint_name: sprint.name.clone(),
            total_tasks: sprint.tasks.len(),
            completed_tasks: completed,
            total_story_points: total_sp,
            completed_story_points: completed_sp,
            completion_rate_pct: sprint.completion_rate() * 100.0,
            days_remaining: sprint.days_remaining(),
            days_total,
            quality_metrics: quality,
            burndown,
            coverage_data: self.coverage_history.clone(),
            generated_at: OffsetDateTime::now_utc(),
        })
    }

    /// Get current sprint reference.
    pub fn current_sprint(&self) -> Option<&Sprint> {
        self.current_sprint.as_ref()
    }

    /// Get historical sprints.
    pub fn historical_sprints(&self) -> &[Sprint] {
        &self.historical_sprints
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_date(days_from_now: i64) -> Date {
        let now = OffsetDateTime::now_utc().date();
        now - Duration::days(days_from_now)
    }

    #[test]
    fn test_sprint_task_new() {
        let task = SprintTask::new("T-1", "Implement feature X", 5);
        assert_eq!(task.id, "T-1");
        assert_eq!(task.story_points, 5);
        assert_eq!(task.status, TaskStatus::Todo);
        assert!(task.completed_at.is_none());
    }

    #[test]
    fn test_sprint_task_mark_done() {
        let mut task = SprintTask::new("T-1", "Implement feature X", 3);
        task.mark_done();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_is_complete() {
        let mut task = SprintTask::new("T-1", "Implement feature X", 3);
        assert!(!task.is_complete());
        task.mark_done();
        assert!(task.is_complete());
    }

    #[test]
    fn test_sprint_new() {
        let start = make_date(0);
        let end = make_date(-14);
        let sprint = Sprint::new("S-1", "Sprint 1", start, end);
        assert_eq!(sprint.id, "S-1");
        assert!(sprint.tasks.is_empty());
    }

    #[test]
    fn test_sprint_story_points() {
        let start = make_date(0);
        let end = make_date(-14);
        let mut sprint = Sprint::new("S-1", "Sprint 1", start, end);
        sprint.add_task(SprintTask::new("T-1", "Task 1", 5));
        sprint.add_task(SprintTask::new("T-2", "Task 2", 3));
        assert_eq!(sprint.total_story_points(), 8);
        assert_eq!(sprint.completed_story_points(), 0);
    }

    #[test]
    fn test_sprint_completion_rate() {
        let start = make_date(0);
        let end = make_date(-14);
        let mut sprint = Sprint::new("S-1", "Sprint 1", start, end);
        sprint.add_task(SprintTask::new("T-1", "Task 1", 5));
        let mut t2 = SprintTask::new("T-2", "Task 2", 5);
        t2.mark_done();
        sprint.add_task(t2);
        assert!((sprint.completion_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sprint_tracker_start_sprint() {
        let mut tracker = SprintTracker::new();
        tracker.start_sprint("S-1", "Sprint 1", make_date(0), make_date(-14));
        assert!(tracker.current_sprint().is_some());
        assert_eq!(tracker.current_sprint().unwrap().id, "S-1");
    }

    #[test]
    fn test_sprint_tracker_add_task() {
        let mut tracker = SprintTracker::new();
        tracker.start_sprint("S-1", "Sprint 1", make_date(0), make_date(-14));
        let task = SprintTask::new("T-1", "Task 1", 5);
        tracker.add_task(task).unwrap();
        assert_eq!(tracker.current_sprint().unwrap().tasks.len(), 1);
    }

    #[test]
    fn test_sprint_tracker_no_active_sprint_error() {
        let tracker = SprintTracker::new();
        let result = tracker.generate_progress_report();
        assert!(result.is_err());
    }

    #[test]
    fn test_sprint_progress_is_on_track() {
        let progress = SprintProgress {
            sprint_id: "S-1".to_string(),
            sprint_name: "Sprint 1".to_string(),
            total_tasks: 10,
            completed_tasks: 5,
            total_story_points: 20,
            completed_story_points: 10,
            completion_rate_pct: 50.0,
            days_remaining: 7,
            days_total: 14,
            quality_metrics: QualityMetrics {
                clippy_warnings: 0,
                fmt_violations: 0,
                test_pass_rate: 1.0,
                test_count: 100,
                gates: Vec::new(),
            },
            burndown: BurndownData {
                sprint_id: "S-1".to_string(),
                points: Vec::new(),
                total_points: 20,
                days: 14,
            },
            coverage_data: Vec::new(),
            generated_at: OffsetDateTime::now_utc(),
        };
        // At day 7 (50% through), should have ~50% done
        assert!(progress.is_on_track());
    }

    #[test]
    fn test_sprint_progress_summary() {
        let progress = SprintProgress {
            sprint_id: "S-1".to_string(),
            sprint_name: "Sprint 1".to_string(),
            total_tasks: 10,
            completed_tasks: 5,
            total_story_points: 20,
            completed_story_points: 10,
            completion_rate_pct: 50.0,
            days_remaining: 7,
            days_total: 14,
            quality_metrics: QualityMetrics {
                clippy_warnings: 0,
                fmt_violations: 0,
                test_pass_rate: 1.0,
                test_count: 100,
                gates: Vec::new(),
            },
            burndown: BurndownData {
                sprint_id: "S-1".to_string(),
                points: Vec::new(),
                total_points: 20,
                days: 14,
            },
            coverage_data: Vec::new(),
            generated_at: OffsetDateTime::now_utc(),
        };
        let summary = progress.summary();
        assert!(summary.contains("Sprint 1"));
        assert!(summary.contains("ON TRACK"));
    }

    #[test]
    fn test_coverage_data_line_coverage_pct() {
        let cov = CoverageData {
            crate_name: "test-crate".to_string(),
            lines_covered: 80,
            lines_total: 100,
            functions_covered: 10,
            functions_total: 10,
            branch_coverage_pct: 75.0,
        };
        assert!((cov.line_coverage_pct() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coverage_data_zero_total() {
        let cov = CoverageData {
            crate_name: "empty".to_string(),
            lines_covered: 0,
            lines_total: 0,
            functions_covered: 0,
            functions_total: 0,
            branch_coverage_pct: 0.0,
        };
        assert_eq!(cov.line_coverage_pct(), 0.0);
        assert_eq!(cov.function_coverage_pct(), 0.0);
    }

    #[test]
    fn test_burndown_data_ideal_at_day() {
        let data = BurndownData {
            sprint_id: "S-1".to_string(),
            points: Vec::new(),
            total_points: 20,
            days: 10,
        };
        assert_eq!(data.ideal_at_day(0), 20);
        assert_eq!(data.ideal_at_day(5), 10);
        assert_eq!(data.ideal_at_day(10), 0);
    }

    #[test]
    fn test_burndown_data_actual_at_day() {
        let point = BurndownPoint {
            day: 3,
            remaining_points: 15,
            ideal_points: 14,
            is_actual: true,
        };
        let data = BurndownData {
            sprint_id: "S-1".to_string(),
            points: vec![point],
            total_points: 20,
            days: 10,
        };
        assert_eq!(data.actual_at_day(3), Some(15));
        assert_eq!(data.actual_at_day(5), None);
    }

    #[test]
    fn test_quality_metrics_all_gates_passed() {
        let metrics = QualityMetrics {
            clippy_warnings: 2,
            fmt_violations: 0,
            test_pass_rate: 1.0,
            test_count: 100,
            gates: vec![
                QualityGate {
                    name: "Coverage".to_string(),
                    passed: true,
                    threshold: 80.0,
                    actual_value: 85.0,
                    message: "ok".to_string(),
                },
                QualityGate {
                    name: "Tests".to_string(),
                    passed: true,
                    threshold: 100.0,
                    actual_value: 100.0,
                    message: "ok".to_string(),
                },
            ],
        };
        assert!(metrics.all_gates_passed());
    }

    #[test]
    fn test_update_task_status() {
        let mut tracker = SprintTracker::new();
        tracker.start_sprint("S-1", "Sprint 1", make_date(0), make_date(-14));
        let task = SprintTask::new("T-1", "Task 1", 5);
        tracker.add_task(task).unwrap();
        tracker
            .update_task_status("T-1", TaskStatus::InProgress)
            .unwrap();
        assert_eq!(
            tracker.current_sprint().unwrap().tasks[0].status,
            TaskStatus::InProgress
        );
    }

    #[test]
    fn test_update_nonexistent_task() {
        let mut tracker = SprintTracker::new();
        tracker.start_sprint("S-1", "Sprint 1", make_date(0), make_date(-14));
        let result = tracker.update_task_status("NONEXISTENT", TaskStatus::Done);
        assert!(result.is_err());
    }
}
