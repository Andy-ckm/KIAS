//! # Kanban Board
//!
//! Six-column task board for Agent cluster orchestration.
//! Columns: Triage → Todo → Ready → InProgress → Blocked → Done
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

/// Kanban column names — 6-column state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KanbanColumn {
    /// 待梳理 — new tasks land here for triage
    Triage,
    /// 待办 — triaged and ready for scheduling
    Todo,
    /// 就绪 — all dependencies met, waiting for agent pickup
    Ready,
    /// 进行中 — agent is actively working
    InProgress,
    /// 阻塞 — waiting on dependency or human intervention
    Blocked,
    /// 完成 — task successfully completed
    Done,
}

impl KanbanColumn {
    /// Next column in the natural flow
    pub fn next(&self) -> Option<KanbanColumn> {
        match self {
            Self::Triage => Some(Self::Todo),
            Self::Todo => Some(Self::Ready),
            Self::Ready => Some(Self::InProgress),
            Self::InProgress => Some(Self::Done),
            Self::Blocked => Some(Self::Ready), // unblock → back to Ready
            Self::Done => None,
        }
    }

    /// Previous column (for rollback/rejection)
    pub fn prev(&self) -> Option<KanbanColumn> {
        match self {
            Self::Triage => None,
            Self::Todo => Some(Self::Triage),
            Self::Ready => Some(Self::Todo),
            Self::InProgress => Some(Self::Ready),
            Self::Blocked => None, // blocked is a special state
            Self::Done => Some(Self::InProgress),
        }
    }

    /// All columns in order
    pub fn all() -> &'static [KanbanColumn] {
        &[
            Self::Triage,
            Self::Todo,
            Self::Ready,
            Self::InProgress,
            Self::Blocked,
            Self::Done,
        ]
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<KanbanColumn> {
        match s.to_lowercase().as_str() {
            "triage" => Some(Self::Triage),
            "todo" => Some(Self::Todo),
            "ready" => Some(Self::Ready),
            "inprogress" | "in_progress" | "in-progress" => Some(Self::InProgress),
            "blocked" => Some(Self::Blocked),
            "done" => Some(Self::Done),
            _ => None,
        }
    }
}

impl std::fmt::Display for KanbanColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Triage => write!(f, "Triage"),
            Self::Todo => write!(f, "Todo"),
            Self::Ready => write!(f, "Ready"),
            Self::InProgress => write!(f, "InProgress"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Trivial = 4,
}

impl Priority {
    pub fn from_int(v: i32) -> Self {
        match v {
            0 => Self::Critical,
            1 => Self::High,
            2 => Self::Medium,
            3 => Self::Low,
            _ => Self::Trivial,
        }
    }
    pub fn to_int(&self) -> i32 {
        *self as i32
    }
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

/// Record of a column transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnTransition {
    pub from: KanbanColumn,
    pub to: KanbanColumn,
    pub at: SystemTime,
    pub by: String,
    pub reason: Option<String>,
}

/// A task on the Kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanTask {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub description: String,
    pub column: KanbanColumn,
    pub priority: Priority,
    pub required_capabilities: Vec<Capability>,
    pub assigned_to: Option<String>,
    pub created_at: SystemTime,
    pub column_entered_at: SystemTime,
    pub updated_at: SystemTime,
    pub history: Vec<ColumnTransition>,
    pub tags: Vec<String>,
    pub estimated_minutes: Option<u64>,
    /// Blocking dependencies (task IDs that must be Done first)
    pub blocked_by: Vec<String>,
    /// Parent task IDs (for decomposition)
    pub parents: Vec<String>,
    /// Child task IDs (sub-tasks)
    pub children: Vec<String>,
    /// Workspace directory for file handoff
    pub workspace: Option<String>,
    /// Failure count for circuit breaker
    pub failure_count: u32,
    /// Block reason (when column == Blocked)
    pub block_reason: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl KanbanTask {
    /// Check if all blocking dependencies are resolved
    pub fn all_blockers_resolved(&self, board: &KanbanBoard) -> bool {
        self.blocked_by.iter().all(|dep_id| {
            board
                .tasks
                .iter()
                .find(|d| &d.id == dep_id)
                .map(|d| d.column == KanbanColumn::Done)
                .unwrap_or(true)
        })
    }

    /// Check if all parent tasks are done
    pub fn all_parents_done(&self, board: &KanbanBoard) -> bool {
        if self.parents.is_empty() {
            return true;
        }
        self.parents.iter().all(|pid| {
            board
                .tasks
                .iter()
                .find(|t| &t.id == pid)
                .map(|t| t.column == KanbanColumn::Done)
                .unwrap_or(true)
        })
    }

    /// Get workspace path
    pub fn workspace_path(&self) -> String {
        self.workspace
            .clone()
            .unwrap_or_else(|| format!("~/.kias/kanban/workspaces/{}", self.id))
    }
}

/// Board statistics for a single column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub column: KanbanColumn,
    pub task_count: usize,
    pub avg_duration: Duration,
    pub max_duration: Duration,
    pub by_priority: HashMap<String, usize>,
}

/// Overall board statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardStats {
    pub total_tasks: usize,
    pub columns: Vec<ColumnStats>,
    pub throughput_per_hour: f64,
    pub wip_violations: Vec<String>,
}

/// WIP (Work-In-Progress) limit per column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipLimit {
    pub column: KanbanColumn,
    pub max_tasks: usize,
}

/// The Kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanBoard {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tasks: Vec<KanbanTask>,
    pub wip_limits: Vec<WipLimit>,
    pub created_at: SystemTime,
}

impl KanbanBoard {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            tasks: Vec::new(),
            wip_limits: vec![
                WipLimit {
                    column: KanbanColumn::InProgress,
                    max_tasks: 5,
                },
                WipLimit {
                    column: KanbanColumn::Ready,
                    max_tasks: 10,
                },
            ],
            created_at: SystemTime::now(),
        }
    }

    pub fn add_task(&mut self, task: KanbanTask) -> Result<(), KanbanError> {
        if self.tasks.iter().any(|t| t.id == task.id) {
            return Err(KanbanError::DuplicateTaskId(task.id));
        }
        self.tasks.push(task);
        Ok(())
    }

    /// Move a task to the next column in the natural flow
    pub fn advance(&mut self, task_id: &str, by: &str) -> Result<(), KanbanError> {
        let current = self
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?
            .column;

        let next = current.next().ok_or(KanbanError::AlreadyAtEnd)?;

        // Blocked must use unblock() explicitly
        if current == KanbanColumn::Blocked {
            return Err(KanbanError::InvalidTransition {
                from: current,
                to: next,
            });
        }

        self.move_to(task_id, next, by, None)
    }

    /// Move a task to a specific column
    pub fn move_to(
        &mut self,
        task_id: &str,
        target: KanbanColumn,
        by: &str,
        reason: Option<String>,
    ) -> Result<(), KanbanError> {
        // Validate transition
        {
            let task = self
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

            if !is_valid_transition(task.column, target) {
                return Err(KanbanError::InvalidTransition {
                    from: task.column,
                    to: target,
                });
            }

            if let Some(limit) = self.wip_limits.iter().find(|l| l.column == target) {
                let count = self.tasks.iter().filter(|t| t.column == target).count();
                if count >= limit.max_tasks {
                    return Err(KanbanError::WipLimitExceeded {
                        column: target,
                        current: count,
                        limit: limit.max_tasks,
                    });
                }
            }

            // Todo→Ready: check dependencies
            if task.column == KanbanColumn::Todo
                && target == KanbanColumn::Ready
                && !task.all_blockers_resolved(self)
            {
                return Err(KanbanError::BlockedByDependency(task_id.to_string()));
            }
        }

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
        if target == KanbanColumn::Blocked {
            task.block_reason = reason.clone();
        } else if from == KanbanColumn::Blocked {
            task.block_reason = None;
        }
        task.history.push(ColumnTransition {
            from,
            to: target,
            at: now,
            by: by.to_string(),
            reason,
        });

        Ok(())
    }

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

    pub fn tasks_in_column(&self, column: KanbanColumn) -> Vec<&KanbanTask> {
        self.tasks.iter().filter(|t| t.column == column).collect()
    }

    /// Unassigned tasks in Ready, sorted by priority
    pub fn claimable_tasks(&self) -> Vec<&KanbanTask> {
        let mut tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t.column == KanbanColumn::Ready && t.assigned_to.is_none())
            .collect();
        tasks.sort_by_key(|t| t.priority);
        tasks
    }

    /// Agent claims a task (Ready → InProgress)
    pub fn claim(&mut self, task_id: &str, agent_id: &str) -> Result<(), KanbanError> {
        {
            let task = self
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

            if task.column != KanbanColumn::Ready {
                return Err(KanbanError::InvalidTransition {
                    from: task.column,
                    to: KanbanColumn::InProgress,
                });
            }
            if task.assigned_to.is_some() {
                return Err(KanbanError::AlreadyAssigned(task_id.to_string()));
            }
        }

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

    /// Block a task with reason
    pub fn block(&mut self, task_id: &str, by: &str, reason: &str) -> Result<(), KanbanError> {
        self.move_to(task_id, KanbanColumn::Blocked, by, Some(reason.to_string()))
    }

    /// Unblock a task (Blocked → Ready)
    pub fn unblock(&mut self, task_id: &str, by: &str) -> Result<(), KanbanError> {
        {
            let task = self
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;
            if task.column != KanbanColumn::Blocked {
                return Err(KanbanError::InvalidTransition {
                    from: task.column,
                    to: KanbanColumn::Ready,
                });
            }
        }

        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;

        let now = SystemTime::now();
        task.column = KanbanColumn::Ready;
        task.block_reason = None;
        task.updated_at = now;
        task.column_entered_at = now;
        task.history.push(ColumnTransition {
            from: KanbanColumn::Blocked,
            to: KanbanColumn::Ready,
            at: now,
            by: by.to_string(),
            reason: Some("Unblocked".to_string()),
        });
        Ok(())
    }

    /// Reject from Done → InProgress
    pub fn reject(&mut self, task_id: &str, by: &str, reason: &str) -> Result<(), KanbanError> {
        self.move_to(
            task_id,
            KanbanColumn::InProgress,
            by,
            Some(format!("Rejected: {}", reason)),
        )
    }

    /// Add parent-child dependency
    pub fn add_dependency(&mut self, child_id: &str, parent_id: &str) -> Result<(), KanbanError> {
        if child_id == parent_id {
            return Err(KanbanError::CircularDependency(child_id.to_string()));
        }
        // Verify both exist
        let _ = self
            .tasks
            .iter()
            .find(|t| t.id == child_id)
            .ok_or_else(|| KanbanError::TaskNotFound(child_id.to_string()))?;
        let _ = self
            .tasks
            .iter()
            .find(|t| t.id == parent_id)
            .ok_or_else(|| KanbanError::TaskNotFound(parent_id.to_string()))?;
        // Mutate child
        let child = self.tasks.iter_mut().find(|t| t.id == child_id).expect("Child task verified to exist");
        if !child.parents.contains(&parent_id.to_string()) {
            child.parents.push(parent_id.to_string());
        }
        if !child.blocked_by.contains(&parent_id.to_string()) {
            child.blocked_by.push(parent_id.to_string());
        }
        // Mutate parent
        let parent = self.tasks.iter_mut().find(|t| t.id == parent_id).expect("Parent task verified to exist");
        if !parent.children.contains(&child_id.to_string()) {
            parent.children.push(child_id.to_string());
        }
        Ok(())
    }

    /// Get dependency tree
    pub fn task_tree(&self, task_id: &str) -> Result<String, KanbanError> {
        let _ = self
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| KanbanError::TaskNotFound(task_id.to_string()))?;
        let mut output = String::new();
        self.fmt_tree(task_id, &mut output, 0);
        Ok(output)
    }

    fn fmt_tree(&self, task_id: &str, output: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        if let Some(task) = self.tasks.iter().find(|t| t.id == task_id) {
            output.push_str(&format!(
                "{}[{}] {} ({}) — {}\n",
                indent, task.id, task.title, task.column, task.priority
            ));
            for child_id in &task.children {
                self.fmt_tree(child_id, output, depth + 1);
            }
        }
    }

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
                *by_priority.entry(task.priority.to_string()).or_insert(0) += 1;
            }
            columns.push(ColumnStats {
                column: col,
                task_count: count,
                avg_duration,
                max_duration,
                by_priority,
            });
        }

        let done_tasks = self
            .tasks
            .iter()
            .filter(|t| t.column == KanbanColumn::Done)
            .filter(|t| {
                t.history.iter().any(|h| {
                    h.to == KanbanColumn::Done
                        && now.duration_since(h.at).unwrap_or(Duration::ZERO)
                            < Duration::from_secs(86400)
                })
            })
            .count();
        let throughput_per_hour = done_tasks as f64 / 24.0;

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

    /// Tasks that can be auto-advanced to Ready
    pub fn auto_advanceable(&self) -> Vec<&KanbanTask> {
        self.tasks
            .iter()
            .filter(|t| {
                t.column == KanbanColumn::Todo
                    && t.all_blockers_resolved(self)
                    && t.all_parents_done(self)
            })
            .collect()
    }

    /// Tasks stuck in the same column for too long
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

/// Check if a column transition is valid
fn is_valid_transition(from: KanbanColumn, to: KanbanColumn) -> bool {
    matches!(
        (from, to),
        (KanbanColumn::Triage, KanbanColumn::Todo)
            | (KanbanColumn::Todo, KanbanColumn::Ready)
            | (KanbanColumn::Ready, KanbanColumn::InProgress)
            | (KanbanColumn::InProgress, KanbanColumn::Done)
            | (KanbanColumn::Ready, KanbanColumn::Blocked)
            | (KanbanColumn::InProgress, KanbanColumn::Blocked)
            | (KanbanColumn::Blocked, KanbanColumn::Ready)
            | (KanbanColumn::Done, KanbanColumn::InProgress)
            | (KanbanColumn::Todo, KanbanColumn::Triage)
            | (KanbanColumn::Ready, KanbanColumn::Todo)
            | (KanbanColumn::InProgress, KanbanColumn::Ready)
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KanbanError {
    DuplicateTaskId(String),
    TaskNotFound(String),
    InvalidTransition {
        from: KanbanColumn,
        to: KanbanColumn,
    },
    AlreadyAtEnd,
    WipLimitExceeded {
        column: KanbanColumn,
        current: usize,
        limit: usize,
    },
    AlreadyAssigned(String),
    BlockedByDependency(String),
    CircularDependency(String),
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
            Self::BlockedByDependency(id) => {
                write!(f, "Task {} blocked by unresolved dependency", id)
            }
            Self::CircularDependency(id) => {
                write!(f, "Circular dependency detected involving task {}", id)
            }
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
            board_id: "test-board".to_string(),
            title: title.to_string(),
            description: String::new(),
            column: KanbanColumn::Triage,
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
            parents: vec![],
            children: vec![],
            workspace: None,
            failure_count: 0,
            block_reason: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_column_flow() {
        assert_eq!(KanbanColumn::Triage.next(), Some(KanbanColumn::Todo));
        assert_eq!(KanbanColumn::Todo.next(), Some(KanbanColumn::Ready));
        assert_eq!(KanbanColumn::Ready.next(), Some(KanbanColumn::InProgress));
        assert_eq!(KanbanColumn::InProgress.next(), Some(KanbanColumn::Done));
        assert_eq!(KanbanColumn::Blocked.next(), Some(KanbanColumn::Ready));
        assert_eq!(KanbanColumn::Done.next(), None);
    }

    #[test]
    fn test_column_rollback() {
        assert_eq!(KanbanColumn::Todo.prev(), Some(KanbanColumn::Triage));
        assert_eq!(KanbanColumn::Ready.prev(), Some(KanbanColumn::Todo));
        assert_eq!(KanbanColumn::InProgress.prev(), Some(KanbanColumn::Ready));
        assert_eq!(KanbanColumn::Triage.prev(), None);
        assert_eq!(KanbanColumn::Blocked.prev(), None);
    }

    #[test]
    fn test_column_display() {
        assert_eq!(KanbanColumn::Triage.to_string(), "Triage");
        assert_eq!(KanbanColumn::Done.to_string(), "Done");
    }

    #[test]
    fn test_column_from_str() {
        assert_eq!(KanbanColumn::parse("triage"), Some(KanbanColumn::Triage));
        assert_eq!(
            KanbanColumn::parse("InProgress"),
            Some(KanbanColumn::InProgress)
        );
        assert_eq!(
            KanbanColumn::parse("in_progress"),
            Some(KanbanColumn::InProgress)
        );
        assert_eq!(KanbanColumn::parse("unknown"), None);
    }

    #[test]
    fn test_add_task() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Task", Priority::Medium))
            .unwrap();
        assert_eq!(board.tasks.len(), 1);
    }

    #[test]
    fn test_duplicate_task_id() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "A", Priority::Medium))
            .unwrap();
        assert!(matches!(
            board.add_task(make_task("t1", "B", Priority::High)),
            Err(KanbanError::DuplicateTaskId(_))
        ));
    }

    #[test]
    fn test_full_flow() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();

        board.advance("t1", "sys").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Todo);

        board.advance("t1", "sys").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);

        board.claim("t1", "agent-1").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);

        board.advance("t1", "agent-1").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Done);
    }

    #[test]
    fn test_block_and_unblock() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::InProgress;
        board.add_task(t).unwrap();

        board.block("t1", "sys", "Waiting for API key").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Blocked);
        assert_eq!(
            board.tasks[0].block_reason,
            Some("Waiting for API key".to_string())
        );

        board.unblock("t1", "admin").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);
        assert!(board.tasks[0].block_reason.is_none());
    }

    #[test]
    fn test_claim_task() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::Ready;
        board.add_task(t).unwrap();

        board.claim("t1", "agent-1").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);
        assert_eq!(board.tasks[0].assigned_to, Some("agent-1".to_string()));
    }

    #[test]
    fn test_claim_wrong_column() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::Todo;
        board.add_task(t).unwrap();
        assert!(matches!(
            board.claim("t1", "a1"),
            Err(KanbanError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn test_claimable_tasks() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t1 = make_task("t1", "High", Priority::High);
        t1.column = KanbanColumn::Ready;
        let mut t2 = make_task("t2", "Low", Priority::Low);
        t2.column = KanbanColumn::Ready;
        let mut t3 = make_task("t3", "Assigned", Priority::Medium);
        t3.column = KanbanColumn::Ready;
        t3.assigned_to = Some("a1".to_string());

        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();

        let claimable = board.claimable_tasks();
        assert_eq!(claimable.len(), 2);
        assert_eq!(claimable[0].id, "t1");
        assert_eq!(claimable[1].id, "t2");
    }

    #[test]
    fn test_wip_limit() {
        let mut board = KanbanBoard::new("b1", "test");
        board.wip_limits = vec![WipLimit {
            column: KanbanColumn::InProgress,
            max_tasks: 2,
        }];

        for i in 0..3 {
            let mut t = make_task(&format!("t{}", i), "T", Priority::Medium);
            t.column = KanbanColumn::Ready;
            board.add_task(t).unwrap();
        }

        board.claim("t0", "a1").unwrap();
        board.claim("t1", "a2").unwrap();
        assert!(matches!(
            board.claim("t2", "a3"),
            Err(KanbanError::WipLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_reject_task() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::Done;
        board.add_task(t).unwrap();
        board.reject("t1", "reviewer", "Needs tests").unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);
    }

    #[test]
    fn test_parent_child_dependency() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Parent", Priority::Medium))
            .unwrap();
        board
            .add_task(make_task("t2", "Child", Priority::Medium))
            .unwrap();

        board.add_dependency("t2", "t1").unwrap();

        let child = board.tasks.iter().find(|t| t.id == "t2").unwrap();
        assert!(child.parents.contains(&"t1".to_string()));
        assert!(child.blocked_by.contains(&"t1".to_string()));

        let parent = board.tasks.iter().find(|t| t.id == "t1").unwrap();
        assert!(parent.children.contains(&"t2".to_string()));
    }

    #[test]
    fn test_circular_dependency() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "A", Priority::Medium))
            .unwrap();
        assert!(matches!(
            board.add_dependency("t1", "t1"),
            Err(KanbanError::CircularDependency(_))
        ));
    }

    #[test]
    fn test_auto_advanceable() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t1 = make_task("t1", "Free", Priority::Medium);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "Blocked", Priority::Medium);
        t2.column = KanbanColumn::Todo;
        t2.blocked_by = vec!["t3".to_string()];
        let mut t3 = make_task("t3", "Blocker", Priority::Medium);
        t3.column = KanbanColumn::InProgress;

        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();

        let adv = board.auto_advanceable();
        assert_eq!(adv.len(), 1);
        assert_eq!(adv[0].id, "t1");
    }

    #[test]
    fn test_board_stats() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t1 = make_task("t1", "A", Priority::High);
        t1.column = KanbanColumn::Todo;
        board.add_task(t1).unwrap();

        let stats = board.stats();
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.columns.len(), 6);
    }

    #[test]
    fn test_task_not_found() {
        let mut board = KanbanBoard::new("b1", "test");
        assert!(matches!(
            board.advance("nope", "sys"),
            Err(KanbanError::TaskNotFound(_))
        ));
    }

    #[test]
    fn test_assign_and_unassign() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();
        board.assign("t1", "a1").unwrap();
        assert_eq!(board.tasks[0].assigned_to, Some("a1".to_string()));
        board.unassign("t1").unwrap();
        assert_eq!(board.tasks[0].assigned_to, None);
    }

    #[test]
    fn test_workspace_path() {
        let t = make_task("t_abc", "Test", Priority::Medium);
        assert_eq!(t.workspace_path(), "~/.kias/kanban/workspaces/t_abc");

        let mut t2 = make_task("t_xyz", "Test", Priority::Medium);
        t2.workspace = Some("/custom/path".to_string());
        assert_eq!(t2.workspace_path(), "/custom/path");
    }

    #[test]
    fn test_priority_from_int() {
        assert_eq!(Priority::from_int(0), Priority::Critical);
        assert_eq!(Priority::from_int(2), Priority::Medium);
        assert_eq!(Priority::from_int(99), Priority::Trivial);
    }

    #[test]
    fn test_done_cannot_advance() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::Done;
        board.add_task(t).unwrap();
        assert!(matches!(
            board.advance("t1", "sys"),
            Err(KanbanError::AlreadyAtEnd)
        ));
    }

    #[test]
    fn test_blocked_must_unblock() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Test", Priority::Medium);
        t.column = KanbanColumn::Blocked;
        board.add_task(t).unwrap();
        assert!(matches!(
            board.advance("t1", "sys"),
            Err(KanbanError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn test_move_to_specific() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Test", Priority::Medium))
            .unwrap();
        board
            .move_to("t1", KanbanColumn::Todo, "admin", None)
            .unwrap();
        assert_eq!(board.tasks[0].column, KanbanColumn::Todo);
    }

    #[test]
    fn test_tasks_in_column() {
        let mut board = KanbanBoard::new("b1", "test");
        let mut t1 = make_task("t1", "A", Priority::Medium);
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "B", Priority::Medium);
        t2.column = KanbanColumn::InProgress;
        let mut t3 = make_task("t3", "C", Priority::Medium);
        t3.column = KanbanColumn::Todo;
        board.add_task(t1).unwrap();
        board.add_task(t2).unwrap();
        board.add_task(t3).unwrap();

        assert_eq!(board.tasks_in_column(KanbanColumn::Todo).len(), 2);
        assert_eq!(board.tasks_in_column(KanbanColumn::InProgress).len(), 1);
    }

    #[test]
    fn test_task_tree() {
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Root", Priority::High))
            .unwrap();
        board
            .add_task(make_task("t2", "Child1", Priority::Medium))
            .unwrap();
        board
            .add_task(make_task("t3", "Child2", Priority::Medium))
            .unwrap();
        board.add_dependency("t2", "t1").unwrap();
        board.add_dependency("t3", "t1").unwrap();

        let tree = board.task_tree("t1").unwrap();
        assert!(tree.contains("Root"));
        assert!(tree.contains("Child1"));
        assert!(tree.contains("Child2"));
    }
}
