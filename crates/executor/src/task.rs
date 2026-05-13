use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub timeout: Option<std::time::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_task_creation() {
        let task = Task {
            id: "task-1".to_string(),
            name: "test-task".to_string(),
            agent_id: "agent-1".to_string(),
            payload: serde_json::json!({"command": "echo hello"}),
            created_at: Utc::now(),
            timeout: Some(std::time::Duration::from_secs(30)),
        };
        assert_eq!(task.id, "task-1");
        assert_eq!(task.name, "test-task");
        assert!(task.timeout.is_some());
    }

    #[test]
    fn test_task_status_variants() {
        assert_ne!(TaskStatus::Pending, TaskStatus::Running);
        assert_ne!(TaskStatus::Completed, TaskStatus::Failed);
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
    }

    #[test]
    fn test_task_result() {
        let result = TaskResult {
            task_id: "task-1".to_string(),
            status: TaskStatus::Completed,
            output: Some(serde_json::json!({"stdout": "hello"})),
            error: None,
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };
        assert_eq!(result.task_id, "task-1");
        assert!(result.output.is_some());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_task_result_with_error() {
        let result = TaskResult {
            task_id: "task-2".to_string(),
            status: TaskStatus::Failed,
            output: None,
            error: Some("timeout exceeded".to_string()),
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };
        assert!(result.error.is_some());
        assert!(result.output.is_none());
    }
}
