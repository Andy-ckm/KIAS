//! 任务队列 - SQLite 持久化

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

/// 任务队列
pub struct TaskQueue {
    conn: Mutex<Connection>,
}

/// 队列统计
pub struct QueueStatistics {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub pending: usize,
}

impl TaskQueue {
    /// 创建新的任务队列
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // 创建表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                task_type TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                priority TEXT NOT NULL,
                result TEXT,
                updated_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 入队任务
    pub fn enqueue(&self, task: &AutomationTask) -> Result<Uuid> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;
        let _task_json = serde_json::to_string(task)?;

        conn.execute(
            "INSERT INTO tasks (id, task_type, status, created_at, created_by, priority, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task.id.to_string(),
                serde_json::to_string(&task.task_type)?,
                "Pending",
                task.created_at.to_rfc3339(),
                task.created_by,
                serde_json::to_string(&task.priority)?,
                Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(task.id)
    }

    /// 更新任务状态
    pub fn update_status(&self, task_id: Uuid, status: &TaskStatus) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;
        let status_str = match status {
            TaskStatus::Pending => "Pending",
            TaskStatus::Running => "Running",
            TaskStatus::Success => "Success",
            TaskStatus::Failed => "Failed",
            TaskStatus::PartialSuccess => "PartialSuccess",
            TaskStatus::Cancelled => "Cancelled",
        };

        conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status_str, Utc::now().to_rfc3339(), task_id.to_string()],
        )?;

        Ok(())
    }

    /// 获取任务历史
    pub fn get_history(&self, limit: Option<usize>) -> Result<Vec<AutomationResult>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;
        let limit = limit.unwrap_or(100);

        let mut stmt = conn.prepare(
            "SELECT id, task_type, status, created_at, created_by, priority, result, updated_at
             FROM tasks ORDER BY created_at DESC LIMIT ?1",
        )?;

        let tasks = stmt
            .query_map(params![limit], |row| {
                let id_str: String = row.get(0)?;
                let task_type_str: String = row.get(1)?;
                let status_str: String = row.get(2)?;
                let created_at_str: String = row.get(3)?;

                let status = match status_str.as_str() {
                    "Pending" => TaskStatus::Pending,
                    "Running" => TaskStatus::Running,
                    "Success" => TaskStatus::Success,
                    "Failed" => TaskStatus::Failed,
                    "PartialSuccess" => TaskStatus::PartialSuccess,
                    "Cancelled" => TaskStatus::Cancelled,
                    _ => TaskStatus::Pending,
                };

                Ok(AutomationResult {
                    task_id: Uuid::parse_str(&id_str).unwrap_or_default(),
                    task_type: task_type_str,
                    status,
                    started_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    completed_at: None,
                    host_results: vec![],
                    summary: String::new(),
                    audit_trail: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> Result<QueueStatistics> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;

        let total: usize = conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;

        let successful: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'Success'",
            [],
            |row| row.get(0),
        )?;

        let failed: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'Failed'",
            [],
            |row| row.get(0),
        )?;

        let pending: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'Pending'",
            [],
            |row| row.get(0),
        )?;

        Ok(QueueStatistics {
            total,
            successful,
            failed,
            pending,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_queue() -> (TaskQueue, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let queue = TaskQueue::new(&db_path).unwrap();
        (queue, tmp)
    }

    #[test]
    fn test_create_queue() {
        let (queue, _tmp) = create_test_queue();
        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_enqueue_task() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls -la".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::Normal,
        };

        let task_id = queue.enqueue(&task).unwrap();
        assert_eq!(task_id, task.id);

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 1);
    }

    #[test]
    fn test_update_status() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls -la".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::Normal,
        };

        queue.enqueue(&task).unwrap();
        queue.update_status(task.id, &TaskStatus::Success).unwrap();

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.successful, 1);
    }

    #[test]
    fn test_get_history() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls -la".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::Normal,
        };

        queue.enqueue(&task).unwrap();

        let history = queue.get_history(Some(10)).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_enqueue_multiple_tasks() {
        let (queue, _tmp) = create_test_queue();

        for i in 0..3 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: "test-user".to_string(),
                priority: TaskPriority::Normal,
            };
            queue.enqueue(&task).unwrap();
        }

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.pending, 3);
    }

    #[test]
    fn test_update_status_to_failed() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls -la".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::Normal,
        };

        queue.enqueue(&task).unwrap();
        queue.update_status(task.id, &TaskStatus::Failed).unwrap();

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.failed, 1);
    }

    #[test]
    fn test_update_status_pending_to_running() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "test".to_string(),
                hosts: vec!["host1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::High,
        };

        queue.enqueue(&task).unwrap();
        queue.update_status(task.id, &TaskStatus::Running).unwrap();

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 0);
    }

    #[test]
    fn test_get_history_with_limit() {
        let (queue, _tmp) = create_test_queue();

        for i in 0..5 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: "test-user".to_string(),
                priority: TaskPriority::Normal,
            };
            queue.enqueue(&task).unwrap();
        }

        let history = queue.get_history(Some(3)).unwrap();
        assert_eq!(history.len(), 3);

        let all_history = queue.get_history(None).unwrap();
        assert_eq!(all_history.len(), 5);
    }

    #[test]
    fn test_get_statistics_mixed_statuses() {
        let (queue, _tmp) = create_test_queue();

        let task1 = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "cmd1".to_string(),
                hosts: vec!["h1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::Normal,
        };
        let task2 = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "cmd2".to_string(),
                hosts: vec!["h2".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::High,
        };
        let task3 = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "cmd3".to_string(),
                hosts: vec!["h3".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::Low,
        };

        queue.enqueue(&task1).unwrap();
        queue.enqueue(&task2).unwrap();
        queue.enqueue(&task3).unwrap();

        queue.update_status(task1.id, &TaskStatus::Success).unwrap();
        queue.update_status(task2.id, &TaskStatus::Failed).unwrap();
        // task3 stays Pending

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.successful, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.pending, 1);
    }

    #[test]
    fn test_update_status_cancelled() {
        let (queue, _tmp) = create_test_queue();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "test".to_string(),
                hosts: vec!["host".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::Normal,
        };

        queue.enqueue(&task).unwrap();
        queue
            .update_status(task.id, &TaskStatus::Cancelled)
            .unwrap();

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_history_empty_queue() {
        let (queue, _tmp) = create_test_queue();
        let history = queue.get_history(Some(10)).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_enqueue_returns_task_id() {
        let (queue, _tmp) = create_test_queue();
        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "test".to_string(),
                hosts: vec!["h1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::Normal,
        };
        let task_id = queue.enqueue(&task).unwrap();
        assert_ne!(task_id, Uuid::nil());
    }

    #[test]
    fn test_get_history_none_limit() {
        let (queue, _tmp) = create_test_queue();
        for i in 0..3 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["h1".to_string()],
                },
                created_at: Utc::now(),
                created_by: "user".to_string(),
                priority: TaskPriority::Normal,
            };
            queue.enqueue(&task).unwrap();
        }
        let history = queue.get_history(None).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_statistics_empty_queue() {
        let (queue, _tmp) = create_test_queue();
        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_update_status_success() {
        let (queue, _tmp) = create_test_queue();
        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "test".to_string(),
                hosts: vec!["h1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::High,
        };
        queue.enqueue(&task).unwrap();
        queue.update_status(task.id, &TaskStatus::Success).unwrap();

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.successful, 1);
        assert_eq!(stats.pending, 0);
    }

    #[test]
    fn test_task_priority_high() {
        let (queue, _tmp) = create_test_queue();
        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "urgent".to_string(),
                hosts: vec!["h1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "user".to_string(),
            priority: TaskPriority::Critical,
        };
        let task_id = queue.enqueue(&task).unwrap();
        assert_ne!(task_id, Uuid::nil());
    }

    #[test]
    fn test_enqueue_different_task_types() {
        let (queue, _tmp) = create_test_queue();
        let task_types = vec![
            TaskType::CustomCommand {
                command: "ls".to_string(),
                hosts: vec!["h1".to_string()],
            },
            TaskType::ComplianceScan {
                profile: "cis".to_string(),
                hosts: vec!["h1".to_string()],
            },
            TaskType::PatchInstall {
                packages: vec!["vim".to_string()],
                hosts: vec!["h1".to_string()],
            },
            TaskType::SecurityUpdate {
                hosts: vec!["h1".to_string()],
            },
        ];

        for tt in task_types {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: tt,
                created_at: Utc::now(),
                created_by: "user".to_string(),
                priority: TaskPriority::Normal,
            };
            queue.enqueue(&task).unwrap();
        }

        let stats = queue.get_statistics().unwrap();
        assert_eq!(stats.total, 4);
    }
}
