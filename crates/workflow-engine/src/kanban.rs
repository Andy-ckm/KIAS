//! # Kanban Board
//!
//! Six-column task board for Agent cluster orchestration.
//! Inspired by Hermes Kanban: Backlog → Todo → InProgress → Review → Done → Archived.
//!
//! ## Design Principles (钱学森系统工程)
//!
//! 1. **声明式**：定义任务期望状态，系统自动推进
//! 2. **可观测**：每列任务数、滞留时间、吞吐量
//! 3. **自驱动**：任务自动流转（条件满足 → 自动推进）
//! 4. **竞争认领**：Agent 按能力竞争认领任务

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Kanban column names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KanbanColumn {
    /// Not yet scheduled
    Backlog,
    /// Ready for execution
    Todo,
    /// Currently being worked on
    InProgress,
    /// Awaiting review/verification
    Review,
    /// Completed successfully
    Done,
    /// Archived (no longer active)
    Archived,
}

impl KanbanColumn {
    /// Next column in the natural flow
    pub fn next(&self) -> Option<KanbanColumn> {
        match self {
            Self::Backlog => Some(Self::Todo),
            Self::Todo => Some(Self::InProgress),
            Self::InProgress => Some(Self::Review),
            Self::Review => Some(Self::Done),
            Self::Done => Some(Self::Archived),
            Self::Archived => None,
        }
    }

    /// Previous column (for rollback/rejection)
    pub fn prev(&self) -> Option<KanbanColumn> {
        match self {
            Self::Backlog => None,
            Self::Todo => Some(Self::Backlog),
            Self::InProgress => Some(Self::Todo),
            Self::Review => Some(Self::InProgress),
            Self::Done => Some(Self::Review),
            Self::Archived => Some(Self::Done),
        }
    }

    /// All columns in order
    pub fn all() -> &'static [KanbanColumn] {
        &[
            Self::Backlog,
            Self::Todo,
            Self::InProgress,
            Self::Review,
            Self::Done,
            Self::Archived,
        ]
    }
}

impl std::fmt::Display for KanbanColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backlog => write!(f, "Backlog"),
            Self::Todo => write!(f, "Todo"),
            Self::InProgress => write!(f, "InProgress"),
            Self::Review => write!(f, "Review"),
            Self::Done => write!(f, "Done"),
            Self::Archived => write!(f, "Archived"),
        }
    }
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    /// Drop everything
    Critical = 0,
    /// Must do next
    High = 1,
    /// Normal priority
    Medium = 2,
    /// Nice to have
    Low = 3,
    /// Backlog filler
    Trivial = 4,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "P0-Critical"),
            Self::High => write!(f, "P1-High"),
            Self::Medium => write!(f, "P2-Medium"),
            Self::Low => write!(f, "P3-Low"),
            Self::Trivial => write!(f, "P4-Trivial"),
        }
    }
}

/// Agent capability tag
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability(pub String);

/// A task on the Kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanTask {
    /// Unique task ID
    pub id: String,
    /// Task title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Current column
    pub column: KanbanColumn,
    /// Priority level
    pub priority: Priority,
    /// Required agent capabilities
    pub required_capabilities: Vec<Capability>,
    /// Assigned agent ID (None = unassigned)
    pub assigned_to: Option<String>,
    /// When the task was created
    pub created_at: SystemTime,
    /// When the task entered the current column
    pub column_entered_at: SystemTime,
    /// When the task was last updated
    pub updated_at: SystemTime,
    /// Column transition history
    pub history: Vec<ColumnTransition>,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Estimated effort (in minutes)
    pub estimated_minutes: Option<u64>,
    /// Blocking dependencies (task IDs)
    pub blocked_by: Vec<String>,
    /// Task metadata (arbitrary key-value)
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Record of a column transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnTransition {
    /// From column
    pub from: KanbanColumn,
    /// To column
    pub to: KanbanColumn,
    /// When the transition happened
    pub at: SystemTime,
    /// Who/what triggered it
    pub by: String,
    /// Optional reason
    pub reason: Option<String>,
}

/// Board statistics for a single column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    /// Column name
    pub column: KanbanColumn,
    /// Number of tasks
    pub task_count: usize,
    /// Average time tasks spend in this column
    pub avg_duration: Duration,
    /// Longest any task has been in this column
    pub max_duration: Duration,
    /// Tasks by priority
    pub by_priority: HashMap<String, usize>,
}

/// Overall board statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardStats {
    /// Total tasks on board
    pub total_tasks: usize,
    /// Per-column stats
    pub columns: Vec<ColumnStats>,
    /// Throughput (tasks completed per hour, rolling 24h)
    pub throughput_per_hour: f64,
    /// WIP limit violations
    pub wip_violations: Vec<String>,
}

/// WIP (Work-In-Progress) limit per column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipLimit {
    /// Column
    pub column: KanbanColumn,
    /// Maximum tasks allowed
    pub max_tasks: usize,
}

/// The Kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanBoard {
    /// Board name
    pub name: String,
    /// All tasks
    pub tasks: Vec<KanbanTask>,
    /// WIP limits
    pub wip_limits: Vec<WipLimit>,
}

impl KanbanBoard {
    /// Create a new empty board
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tasks: Vec::new(),
            wip_limits: vec![
                WipLimit {
                    column: KanbanColumn::InProgress,
                    max_tasks: 5,
                },
                WipLimit {
                    column: KanbanColumn::Review,
                    max_tasks: 3,
                },
            ],
        }
    }

    /// Add a task to the board
    pub fn add_task(&mut self, task: KanbanTask) -> Result<(), KanbanError> {
        if self.tasks.iter().any(|t| t.id == task.id) {
            return Err(KanbanError::DuplicateTaskId(task.id));
        }
        self.tasks.push(task);
        Ok(())
    }

    /// Move a task to the next column
    pub fn advance(&mut self, task_id: &str, by: &str) -> Result<KanbanColumn, KanbanError> {
        // First, find current column (immutable borrow)
        let current = self
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?
            .column;

        let next = current.next().ok_or(KanbanError::AlreadyAtEnd)?;

        // Check WIP limit
        if let Some(limit) = self.wip_limits.iter().find(|l| l.column == next) {
            let count = self.tasks.iter().filter(|t| t.column == next).count();
            if count >= limit.max_tasks {
                return Err(KanbanError::WipLimitExceeded {
                    column: next,
                    current: count,
                    limit: limit.max_tasks,
                });
            }
        }

        // Now mutate
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        let now = SystemTime::now();
        let from = task.column;
        task.column = next;
        task.updated_at = now;
        task.column_entered_at = now;
        task.history.push(ColumnTransition {
            from,
            to: next,
            at: now,
            by: by.to_string(),
            reason: None,
        });

        Ok(next)
    }

    /// Move a task to a specific column
    pub fn move_to(
        &mut self,
        task_id: &str,
        target: KanbanColumn,
        by: &str,
        reason: Option<String>,
    ) -> Result<(), KanbanError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        let now = SystemTime::now();
        let from = task.column;
        task.column = target;
        task.updated_at = now;
        task.column_entered_at = now;
        task.history.push(ColumnTransition {
            from,
            to: target,
            at: now,
            by: by.to_string(),
            reason,
        });

        Ok(())
    }

    /// Assign a task to an agent
    pub fn assign(&mut self, task_id: &str, agent_id: &str) -> Result<(), KanbanError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        task.assigned_to = Some(agent_id.to_string());
        task.updated_at = SystemTime::now();
        Ok(())
    }

    /// Unassign a task
    pub fn unassign(&mut self, task_id: &str) -> Result<(), KanbanError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        task.assigned_to = None;
        task.updated_at = SystemTime::now();
        Ok(())
    }

    /// Get tasks in a specific column
    pub fn tasks_in_column(&self, column: KanbanColumn) -> Vec<&KanbanTask> {
        self.tasks.iter().filter(|t| t.column == column).collect()
    }

    /// Get unassigned tasks in Todo column, sorted by priority
    pub fn claimable_tasks(&self) -> Vec<&KanbanTask> {
        let mut tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t.column == KanbanColumn::Todo && t.assigned_to.is_none())
            .collect();
        tasks.sort_by_key(|t| t.priority);
        tasks
    }

    /// Agent claims a task (moves from Todo to InProgress)
    pub fn claim(&mut self, task_id: &str, agent_id: &str) -> Result<(), KanbanError> {
        // Validate first (immutable borrow)
        {
            let task = self
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

            if task.column != KanbanColumn::Todo {
                return Err(KanbanError::InvalidTransition {
                    from: task.column,
                    to: KanbanColumn::InProgress,
                });
            }

            if task.assigned_to.is_some() {
                return Err(KanbanError::AlreadyAssigned(task_id.to_string()));
            }
        }

        // Check WIP limit
        if let Some(limit) = self
            .wip_limits
            .iter()
            .find(|l| l.column == KanbanColumn::InProgress)
        {
            let count = self
                .tasks
                .iter()
                .filter(|t| t.column == KanbanColumn::InProgress)
                .count();
            if count >= limit.max_tasks {
                return Err(KanbanError::WipLimitExceeded {
                    column: KanbanColumn::InProgress,
                    current: count,
                    limit: limit.max_tasks,
                });
            }
        }

        // Now mutate
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        let now = SystemTime::now();
        let from = task.column;
        task.column = KanbanColumn::InProgress;
        task.assigned_to = Some(agent_id.to_string());
        task.updated_at = now;
        task.column_entered_at = now;
        task.history.push(ColumnTransition {
            from,
            to: KanbanColumn::InProgress,
            at: now,
            by: agent_id.to_string(),
            reason: Some("Agent claimed task".to_string()),
        });

        Ok(())
    }

    /// Reject a task from Review back to InProgress
    pub fn reject(&mut self, task_id: &str, by: &str, reason: &str) -> Result<(), KanbanError> {
        self.move_to(
            task_id,
            KanbanColumn::InProgress,
            by,
            Some(format!("Rejected: {}", reason)),
        )
    }

    /// Get board statistics
    pub fn stats(&self) -> BoardStats {
        let now = SystemTime::now();
        let mut columns = Vec::new();

        for &col in KanbanColumn::all() {
            let tasks_in_col: Vec<_> = self.tasks.iter().filter(|t| t.column == col).collect();
            let count = tasks_in_col.len();

            let durations: Vec<Duration> = tasks_in_col
                .iter()
                .filter_map(|t| now.duration_since(t.column_entered_at).ok())
                .collect();

            let avg_duration = if durations.is_empty() {
                Duration::ZERO
            } else {
                durations.iter().sum::<Duration>() / durations.len() as u32
            };

            let max_duration = durations.iter().max().copied().unwrap_or(Duration::ZERO);

            let mut by_priority = HashMap::new();
            for task in &tasks_in_col {
                let key = task.priority.to_string();
                *by_priority.entry(key).or_insert(0) += 1;
            }

            columns.push(ColumnStats {
                column: col,
                task_count: count,
                avg_duration,
                max_duration,
                by_priority,
            });
        }

        // Throughput: tasks completed in last 24h
        let done_tasks = self
            .tasks
            .iter()
            .filter(|t| t.column == KanbanColumn::Done || t.column == KanbanColumn::Archived)
            .filter(|t| {
                t.history.iter().any(|h| {
                    h.to == KanbanColumn::Done
                        && now.duration_since(h.at).unwrap_or(Duration::ZERO)
                            < Duration::from_secs(86400)
                })
            })
            .count();
        let throughput_per_hour = done_tasks as f64 / 24.0;

        // WIP violations
        let mut wip_violations = Vec::new();
        for limit in &self.wip_limits {
            let count = self
                .tasks
                .iter()
                .filter(|t| t.column == limit.column)
                .count();
            if count > limit.max_tasks {
                wip_violations.push(format!(
                    "{}: {}/{} tasks",
                    limit.column, count, limit.max_tasks
                ));
            }
        }

        BoardStats {
            total_tasks: self.tasks.len(),
            columns,
            throughput_per_hour,
            wip_violations,
        }
    }

    /// Find tasks that can be auto-advanced
    /// (e.g., all blockers resolved, review passed)
    pub fn auto_advanceable(&self) -> Vec<&KanbanTask> {
        self.tasks
            .iter()
            .filter(|t| {
                // Tasks in Todo with no blockers
                t.column == KanbanColumn::Todo
                    && t.blocked_by.iter().all(|dep_id| {
                        self.tasks
                            .iter()
                            .find(|d| &d.id == dep_id)
                            .map(|d| {
                                d.column == KanbanColumn::Done || d.column == KanbanColumn::Archived
                            })
                            .unwrap_or(true)
                    })
            })
            .collect()
    }

    /// Find stale tasks (in same column for too long)
    pub fn stale_tasks(&self, threshold: Duration) -> Vec<&KanbanTask> {
        let now = SystemTime::now();
        self.tasks
            .iter()
            .filter(|t| {
                now.duration_since(t.column_entered_at)
                    .map(|d| d > threshold)
                    .unwrap_or(false)
            })
            .collect()
    }
}

/// Kanban board errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KanbanError {
    /// Task ID already exists
    DuplicateTaskId(String),
    /// Task not found
    TaskNotFound(String),
    /// Invalid column transition
    InvalidTransition {
        from: KanbanColumn,
        to: KanbanColumn,
    },
    /// Task already at the last column
    AlreadyAtEnd,
    /// WIP limit exceeded
    WipLimitExceeded {
        column: KanbanColumn,
        current: usize,
        limit: usize,
    },
    /// Task already assigned
    AlreadyAssigned(String),
}

impl std::fmt::Display for KanbanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTaskId(id) => write!(f, "Duplicate task ID: {}", id),
            Self::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            Self::InvalidTransition { from, to } => {
                write!(f, "Invalid transition: {} → {}", from, to)
            }
            Self::AlreadyAtEnd => write!(f, "Task already at last column"),
            Self::WipLimitExceeded {
                column,
                current,
                limit,
            } => write!(f, "WIP limit exceeded on {}: {}/{}", column, current, limit),
            Self::AlreadyAssigned(id) => write!(f, "Task already assigned: {}", id),
        }
    }
}

impl std::error::Error for KanbanError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, title: &str, priority: Priority) -> KanbanTask {
        KanbanTask {
            id: id.to_string(),
            title: title.to_string(),
            description: String::new(),
            column: KanbanColumn::Backlog,
            priority,
            required_capabilities: vec![],
            assigned_to: None,
            created_at: SystemTime::now(),
            column_entered_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            history: vec![],
            tags: vec![],
            estimated_minutes: None,
            blocked_by: vec![],
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_column_flow() {
        assert_eq!(KanbanColumn::Backlog.next(), Some(KanbanColumn::Todo));
        assert_eq!(KanbanColumn::Todo.next(), Some(KanbanColumn::InProgress));
        assert_eq!(KanbanColumn::InProgress.next(), Some(KanbanColumn::Review));
        assert_eq!(KanbanColumn::Review.next(), Some(KanbanColumn::Done));
        assert_eq!(KanbanColumn::Done.next(), Some(KanbanColumn::Archived));
        assert_eq!(KanbanColumn::Archived.next(), None);
    }

    #[test]
    fn test_column_rollback() {
        assert_eq!(KanbanColumn::Review.prev(), Some(KanbanColumn::InProgress));
        assert_eq!(KanbanColumn::InProgress.prev(), Some(KanbanColumn::Todo));
        assert_eq!(KanbanColumn::Backlog.prev(), None);
    }

    #[test]
    fn test_add_task() {
        let mut board = KanbanBoard::new("test");
        let task = make_task("t1", "Test task", Priority::Medium);
        board.add_task(task).unwrap();
        assert_eq!(board.tasks.len(), 1);
    }

    #[test]
    fn test_duplicate_task_id() {
        let mut board = KanbanBoard::new("test");
        board
            .add_task(make_task("t1", "Task 1", Priority::Medium))
            .unwrap();
        let result = board.add_task(make_task("t1", "Task 2", Priority::High));
        assert!(matches!(result, Err(KanbanError::DuplicateTaskId(_))));
    }

    #[test]
    fn test_advance_task() {
        let mut board = KanbanBoard::new("test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();
        board.advance("t1", "system").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Todo);
        assert_eq!(board.tasks[0].history.len(), 1);
    }

    #[test]
    fn test_claim_task() {
        let mut board = KanbanBoard::new("test");
        let mut task = make_task("t1", "Test", Priority::Medium);
        task.column = KanbanColumn::Todo;
        task.column_entered_at = SystemTime::now();
        board.add_task(task).unwrap();
        board.claim("t1", "agent-1").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);
        assert_eq!(board.tasks[0].assigned_to, Some("agent-1".to_string()));
    }

    #[test]
    fn test_claim_already_assigned() {
        let mut board = KanbanBoard::new("test");
        let mut task = make_task("t1", "Test", Priority::Medium);
        task.column = KanbanColumn::Todo;
        task.assigned_to = Some("agent-1".to_string());
        board.add_task(task).unwrap();
        let result = board.claim("t1", "agent-2");
        assert!(matches!(result, Err(KanbanError::AlreadyAssigned(_))));
    }

    #[test]
    fn test_claimable_tasks() {
        let mut board = KanbanBoard::new("test");
        let mut t1 = make_task("t1", "High", Priority::High);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "Low", Priority::Low);
        t2.column = KanbanColumn::Todo;
        let mut t3 = make_task("t3", "Assigned", Priority::Medium);
        t3.column = KanbanColumn::Todo;
        t3.assigned_to = Some("agent-1".to_string());
        let mut t4 = make_task("t4", "Backlog", Priority::Critical);
        t4.column = KanbanColumn::Backlog;

        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();
        board.add_task(t4).unwrap();

        let claimable = board.claimable_tasks();
        assert_eq!(claimable.len(), 2);
        // Sorted by priority
        assert_eq!(claimable[0].id, "t1");
        assert_eq!(claimable[1].id, "t2");
    }

    #[test]
    fn test_wip_limit() {
        let mut board = KanbanBoard::new("test");
        board.wip_limits = vec![WipLimit {
            column: KanbanColumn::InProgress,
            max_tasks: 2,
        }];

        // Move 3 tasks to Todo first
        for i in 0..3 {
            let mut task = make_task(&format!("t{}", i), "Task", Priority::Medium);
            task.column = KanbanColumn::Todo;
            board.add_task(task).unwrap();
        }

        board.claim("t0", "a1").unwrap();
        board.claim("t1", "a2").unwrap();
        let result = board.claim("t2", "a3");
        assert!(matches!(result, Err(KanbanError::WipLimitExceeded { .. })));
    }

    #[test]
    fn test_reject_task() {
        let mut board = KanbanBoard::new("test");
        let mut task = make_task("t1", "Test", Priority::Medium);
        task.column = KanbanColumn::Review;
        board.add_task(task).unwrap();
        board.reject("t1", "reviewer", "Needs more tests").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);
    }

    #[test]
    fn test_move_to_specific_column() {
        let mut board = KanbanBoard::new("test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();
        board
            .move_to("t1", KanbanColumn::Done, "admin", Some("Skip".to_string()))
            .unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Done);
    }

    #[test]
    fn test_tasks_in_column() {
        let mut board = KanbanBoard::new("test");
        let mut t1 = make_task("t1", "A", Priority::Medium);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "B", Priority::Medium);
        t2.column = KanbanColumn::InProgress;
        let mut t3 = make_task("t3", "C", Priority::Medium);
        t3.column = KanbanColumn::Todo;
        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();

        let todo_tasks = board.tasks_in_column(KanbanColumn::Todo);
        assert_eq!(todo_tasks.len(), 2);
        let in_progress = board.tasks_in_column(KanbanColumn::InProgress);
        assert_eq!(in_progress.len(), 1);
    }

    #[test]
    fn test_auto_advanceable() {
        let mut board = KanbanBoard::new("test");
        let mut t1 = make_task("t1", "Unblocked", Priority::Medium);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "Blocked", Priority::Medium);
        t2.column = KanbanColumn::Todo;
        t2.blocked_by = vec!["t3".to_string()];
        let mut t3 = make_task("t3", "Blocker", Priority::Medium);
        t3.column = KanbanColumn::InProgress;

        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();

        let advanceable = board.auto_advanceable();
        assert_eq!(advanceable.len(), 1);
        assert_eq!(advanceable[0].id, "t1");
    }

    #[test]
    fn test_board_stats() {
        let mut board = KanbanBoard::new("test");
        let mut t1 = make_task("t1", "A", Priority::High);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "B", Priority::Medium);
        t2.column = KanbanColumn::InProgress;
        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();

        let stats = board.stats();
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.columns.len(), 6);
    }

    #[test]
    fn test_task_not_found() {
        let mut board = KanbanBoard::new("test");
        let result = board.advance("nonexistent", "system");
        assert!(matches!(result, Err(KanbanError::TaskNotFound(_))));
    }

    #[test]
    fn test_assign_and_unassign() {
        let mut board = KanbanBoard::new("test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();
        board.assign("t1", "agent-1").unwrap();
        assert_eq!(board.tasks[0].assigned_to, Some("agent-1".to_string()));
        board.unassign("t1").unwrap();
        assert_eq!(board.tasks[0].assigned_to, None);
    }
}
