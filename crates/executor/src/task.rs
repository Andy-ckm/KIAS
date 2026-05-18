use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    #[test]
    fn test_task_serialization_roundtrip() {
        let task = Task {
            id: "serial-1".to_string(),
            name: "serial-task".to_string(),
            agent_id: "agent-42".to_string(),
            payload: serde_json::json!({"command": "ls", "args": ["-la"]}),
            created_at: Utc::now(),
            timeout: Some(std::time::Duration::from_secs(60)),
        };
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "serial-1");
        assert_eq!(deserialized.name, "serial-task");
        assert_eq!(deserialized.agent_id, "agent-42");
        assert!(deserialized.timeout.is_some());
    }

    #[test]
    fn test_task_result_serialization_roundtrip() {
        let result = TaskResult {
            task_id: "result-1".to_string(),
            status: TaskStatus::Completed,
            output: Some(serde_json::json!({"stdout": "ok"})),
            error: None,
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.task_id, "result-1");
        assert_eq!(deserialized.status, TaskStatus::Completed);
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_task_status_all_variants() {
        // All 5 variants should be distinct
        let variants = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn test_task_no_timeout() {
        let task = Task {
            id: "no-timeout".to_string(),
            name: "test".to_string(),
            agent_id: "a1".to_string(),
            payload: serde_json::json!({}),
            created_at: Utc::now(),
            timeout: None,
        };
        assert!(task.timeout.is_none());
    }

    #[test]
    fn test_task_empty_payload() {
        let task = Task {
            id: "empty".to_string(),
            name: "test".to_string(),
            agent_id: "a1".to_string(),
            payload: serde_json::json!({}),
            created_at: Utc::now(),
            timeout: None,
        };
        assert_eq!(task.payload, serde_json::json!({}));
    }

    #[test]
    fn test_task_result_with_both_output_and_error() {
        let result = TaskResult {
            task_id: "partial".to_string(),
            status: TaskStatus::Failed,
            output: Some(serde_json::json!({"partial_output": "data"})),
            error: Some("connection reset".to_string()),
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };
        assert!(result.output.is_some());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_task_clone() {
        let task = Task {
            id: "clone-me".to_string(),
            name: "test".to_string(),
            agent_id: "a1".to_string(),
            payload: serde_json::json!({"key": "value"}),
            created_at: Utc::now(),
            timeout: Some(std::time::Duration::from_secs(10)),
        };
        let cloned = task.clone();
        assert_eq!(cloned.id, task.id);
        assert_eq!(cloned.name, task.name);
        assert_eq!(cloned.payload, task.payload);
    }

    #[test]
    fn test_task_status_serialization() {
        let json = serde_json::to_string(&TaskStatus::Cancelled).unwrap();
        assert_eq!(json, "\"Cancelled\"");
        let deserialized: TaskStatus = serde_json::from_str("\"Running\"").unwrap();
        assert_eq!(deserialized, TaskStatus::Running);
    }
}
