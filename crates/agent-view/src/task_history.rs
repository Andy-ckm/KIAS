use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

/// Task execution outcome
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskOutcome {
    Success,
    Failure,
    Timeout,
    Cancelled,
    Retry,
}

/// A single task execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub agent_id: String,
    pub task_type: String,
    pub description: String,
    pub outcome: TaskOutcome,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub tokens_consumed: u64,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
}

impl TaskRecord {
    pub fn new(
        task_id: &str,
        agent_id: &str,
        task_type: &str,
        description: &str,
        outcome: TaskOutcome,
        duration_ms: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            task_type: task_type.to_string(),
            description: description.to_string(),
            outcome,
            started_at: now - Duration::milliseconds(duration_ms as i64),
            completed_at: now,
            duration_ms,
            tokens_consumed: 0,
            retry_count: 0,
            error_message: None,
            metadata: serde_json::json!({}),
        }
    }

    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.tokens_consumed = tokens;
        self
    }

    pub fn with_error(mut self, error: &str) -> Self {
        self.error_message = Some(error.to_string());
        self
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retry_count = retries;
        self
    }

    pub fn is_success(&self) -> bool {
        self.outcome == TaskOutcome::Success
    }
}

/// Summary statistics for a set of tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    pub total_tasks: u64,
    pub successful: u64,
    pub failed: u64,
    pub timed_out: u64,
    pub cancelled: u64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: u64,
    pub p95_duration_ms: u64,
    pub p99_duration_ms: u64,
    pub total_tokens: u64,
    pub avg_tokens_per_task: f64,
    pub total_retries: u64,
}

/// Query filter for task records
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub agent_id: Option<String>,
    pub task_type: Option<String>,
    pub outcome: Option<TaskOutcome>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub min_duration_ms: Option<u64>,
    pub max_duration_ms: Option<u64>,
}

impl TaskFilter {
    pub fn matches(&self, record: &TaskRecord) -> bool {
        if let Some(ref agent_id) = self.agent_id {
            if &record.agent_id != agent_id { return false; }
        }
        if let Some(ref task_type) = self.task_type {
            if &record.task_type != task_type { return false; }
        }
        if let Some(ref outcome) = self.outcome {
            if &record.outcome != outcome { return false; }
        }
        if let Some(since) = self.since {
            if record.completed_at < since { return false; }
        }
        if let Some(until) = self.until {
            if record.completed_at > until { return false; }
        }
        if let Some(min_dur) = self.min_duration_ms {
            if record.duration_ms < min_dur { return false; }
        }
        if let Some(max_dur) = self.max_duration_ms {
            if record.duration_ms > max_dur { return false; }
        }
        true
    }
}

/// Task history storage and query engine
pub struct TaskHistory {
    records: Vec<TaskRecord>,
    max_records: usize,
}

impl Default for TaskHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHistory {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            max_records: 50_000,
        }
    }

    pub fn with_max_records(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records,
        }
    }

    pub fn record(&mut self, task: TaskRecord) {
        self.records.push(task);
        if self.records.len() > self.max_records {
            let drain = self.records.len() - self.max_records;
            self.records.drain(0..drain);
        }
    }

    pub fn query(&self, filter: &TaskFilter) -> Vec<&TaskRecord> {
        self.records.iter().filter(|r| filter.matches(r)).collect()
    }

    pub fn for_agent(&self, agent_id: &str) -> Vec<&TaskRecord> {
        let filter = TaskFilter { agent_id: Some(agent_id.to_string()), ..Default::default() };
        self.query(&filter)
    }

    pub fn recent_failures(&self, limit: usize) -> Vec<&TaskRecord> {
        let filter = TaskFilter { outcome: Some(TaskOutcome::Failure), ..Default::default() };
        let mut results = self.query(&filter);
        results.sort_by_key(|r| std::cmp::Reverse(r.completed_at));
        results.into_iter().take(limit).collect()
    }

    pub fn stats(&self, filter: &TaskFilter) -> TaskStats {
        let filtered: Vec<&TaskRecord> = self.records.iter().filter(|r| filter.matches(r)).collect();
        let total = filtered.len() as u64;
        if total == 0 {
            return TaskStats {
                total_tasks: 0, successful: 0, failed: 0, timed_out: 0, cancelled: 0,
                success_rate: 0.0, avg_duration_ms: 0.0, p50_duration_ms: 0, p95_duration_ms: 0,
                p99_duration_ms: 0, total_tokens: 0, avg_tokens_per_task: 0.0, total_retries: 0,
            };
        }

        let mut successful = 0u64;
        let mut failed = 0u64;
        let mut timed_out = 0u64;
        let mut cancelled = 0u64;
        let mut total_duration = 0u64;
        let mut total_tokens = 0u64;
        let mut total_retries = 0u64;
        let mut durations: Vec<u64> = Vec::with_capacity(filtered.len());

        for r in &filtered {
            match r.outcome {
                TaskOutcome::Success => successful += 1,
                TaskOutcome::Failure => failed += 1,
                TaskOutcome::Timeout => timed_out += 1,
                TaskOutcome::Cancelled => cancelled += 1,
                TaskOutcome::Retry => { successful += 1; },
            }
            total_duration += r.duration_ms;
            total_tokens += r.tokens_consumed;
            total_retries += r.retry_count as u64;
            durations.push(r.duration_ms);
        }

        durations.sort_unstable();
        let p = |pct: f64| -> u64 {
            let idx = ((pct / 100.0) * durations.len() as f64) as usize;
            durations.get(idx.min(durations.len().saturating_sub(1))).copied().unwrap_or(0)
        };

        TaskStats {
            total_tasks: total,
            successful,
            failed,
            timed_out,
            cancelled,
            success_rate: successful as f64 / total as f64,
            avg_duration_ms: total_duration as f64 / total as f64,
            p50_duration_ms: p(50.0),
            p95_duration_ms: p(95.0),
            p99_duration_ms: p(99.0),
            total_tokens,
            avg_tokens_per_task: total_tokens as f64 / total as f64,
            total_retries,
        }
    }

    pub fn agent_stats(&self, agent_id: &str) -> TaskStats {
        let filter = TaskFilter { agent_id: Some(agent_id.to_string()), ..Default::default() };
        self.stats(&filter)
    }

    pub fn task_type_breakdown(&self) -> HashMap<String, u64> {
        let mut breakdown = HashMap::new();
        for r in &self.records {
            *breakdown.entry(r.task_type.clone()).or_insert(0) += 1;
        }
        breakdown
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(task_id: &str, agent_id: &str, outcome: TaskOutcome, dur: u64) -> TaskRecord {
        TaskRecord::new(task_id, agent_id, "code", "test task", outcome, dur)
    }

    #[test]
    fn test_task_record_creation() {
        let task = TaskRecord::new("t1", "a1", "code", "implement feature", TaskOutcome::Success, 500);
        assert_eq!(task.task_id, "t1");
        assert!(task.is_success());
        assert_eq!(task.duration_ms, 500);
    }

    #[test]
    fn test_task_record_builder() {
        let task = TaskRecord::new("t1", "a1", "llm", "generate", TaskOutcome::Success, 100)
            .with_tokens(5000)
            .with_retries(2);
        assert_eq!(task.tokens_consumed, 5000);
        assert_eq!(task.retry_count, 2);
    }

    #[test]
    fn test_task_record_with_error() {
        let task = TaskRecord::new("t1", "a1", "code", "test", TaskOutcome::Failure, 100)
            .with_error("connection timeout");
        assert!(task.error_message.is_some());
        assert!(!task.is_success());
    }

    #[test]
    fn test_task_history_record() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(make_task("t2", "a1", TaskOutcome::Failure, 200));
        assert_eq!(history.count(), 2);
    }

    #[test]
    fn test_task_history_max_records() {
        let mut history = TaskHistory::with_max_records(3);
        for i in 0..5 {
            history.record(make_task(&format!("t{}", i), "a1", TaskOutcome::Success, 100));
        }
        assert_eq!(history.count(), 3);
    }

    #[test]
    fn test_task_history_for_agent() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(make_task("t2", "a2", TaskOutcome::Success, 200));
        history.record(make_task("t3", "a1", TaskOutcome::Failure, 300));
        let a1_tasks = history.for_agent("a1");
        assert_eq!(a1_tasks.len(), 2);
    }

    #[test]
    fn test_task_history_recent_failures() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(make_task("t2", "a1", TaskOutcome::Failure, 200));
        history.record(make_task("t3", "a1", TaskOutcome::Failure, 300));
        let failures = history.recent_failures(10);
        assert_eq!(failures.len(), 2);
    }

    #[test]
    fn test_task_history_stats() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(make_task("t2", "a1", TaskOutcome::Success, 200));
        history.record(make_task("t3", "a1", TaskOutcome::Failure, 300));
        let stats = history.agent_stats("a1");
        assert_eq!(stats.total_tasks, 3);
        assert_eq!(stats.successful, 2);
        assert_eq!(stats.failed, 1);
        assert!((stats.success_rate - 2.0/3.0).abs() < 0.01);
        assert!((stats.avg_duration_ms - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_task_history_empty_stats() {
        let history = TaskHistory::new();
        let stats = history.stats(&TaskFilter::default());
        assert_eq!(stats.total_tasks, 0);
    }

    #[test]
    fn test_task_type_breakdown() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(TaskRecord::new("t2", "a1", "research", "search", TaskOutcome::Success, 200));
        history.record(TaskRecord::new("t3", "a1", "code", "fix", TaskOutcome::Success, 300));
        let breakdown = history.task_type_breakdown();
        assert_eq!(breakdown.get("code"), Some(&2));
        assert_eq!(breakdown.get("research"), Some(&1));
    }

    #[test]
    fn test_task_filter_outcome() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 100));
        history.record(make_task("t2", "a1", TaskOutcome::Failure, 200));
        let filter = TaskFilter { outcome: Some(TaskOutcome::Failure), ..Default::default() };
        assert_eq!(history.query(&filter).len(), 1);
    }

    #[test]
    fn test_task_filter_duration() {
        let mut history = TaskHistory::new();
        history.record(make_task("t1", "a1", TaskOutcome::Success, 50));
        history.record(make_task("t2", "a1", TaskOutcome::Success, 500));
        history.record(make_task("t3", "a1", TaskOutcome::Success, 5000));
        let filter = TaskFilter { min_duration_ms: Some(100), max_duration_ms: Some(1000), ..Default::default() };
        assert_eq!(history.query(&filter).len(), 1);
    }

    #[test]
    fn test_percentiles() {
        let mut history = TaskHistory::new();
        for i in 1..=100 {
            history.record(make_task(&format!("t{}", i), "a1", TaskOutcome::Success, i));
        }
        let stats = history.agent_stats("a1");
        assert!(stats.p50_duration_ms > 0);
        assert!(stats.p95_duration_ms >= stats.p50_duration_ms);
        assert!(stats.p99_duration_ms >= stats.p95_duration_ms);
    }
}
