//! 工作流管理模块

use serde::{Deserialize, Serialize};

/// 工作流运行实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub run_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
}
/// 工作流状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_run_serialize() {
        let run = WorkflowRun {
            run_id: "run-001".to_string(),
            workflow_name: "data-pipeline".to_string(),
            status: WorkflowStatus::Running,
            started_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: None,
            input: serde_json::json!({"key": "value"}),
            output: None,
        };
        let json = serde_json::to_string(&run).expect("should serialize");
        assert!(json.contains("run-001"));
        assert!(json.contains("Running"));
    }

    #[test]
    fn test_workflow_run_deserialize() {
        let json = r#"{
            "run_id": "run-002",
            "workflow_name": "test-wf",
            "status": "Completed",
            "started_at": "2024-01-01T00:00:00Z",
            "completed_at": "2024-01-01T00:05:00Z",
            "input": {},
            "output": {"result": "success"}
        }"#;
        let run: WorkflowRun = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(run.run_id, "run-002");
        assert!(matches!(run.status, WorkflowStatus::Completed));
        assert!(run.output.is_some());
    }

    #[test]
    fn test_workflow_status_variants() {
        let statuses = vec![
            ("Pending", WorkflowStatus::Pending),
            ("Running", WorkflowStatus::Running),
            ("Completed", WorkflowStatus::Completed),
            ("Failed", WorkflowStatus::Failed),
            ("Cancelled", WorkflowStatus::Cancelled),
        ];
        for (name, status) in statuses {
            let json = serde_json::to_string(&status).expect("should serialize");
            assert!(json.contains(name), "Expected {} in {}", name, json);
        }
    }

    #[test]
    fn test_workflow_run_clone_debug() {
        let run = WorkflowRun {
            run_id: "run-clone".to_string(),
            workflow_name: "wf-test".to_string(),
            status: WorkflowStatus::Failed,
            started_at: "2024-01-01".to_string(),
            completed_at: Some("2024-01-02".to_string()),
            input: serde_json::json!({"key": "value"}),
            output: Some(serde_json::json!({"error": "timeout"})),
        };
        let cloned = run.clone();
        assert_eq!(cloned.run_id, "run-clone");
        assert!(matches!(cloned.status, WorkflowStatus::Failed));
        assert!(cloned.output.is_some());
        let _debug = format!("{:?}", cloned);
    }

    #[test]
    fn test_workflow_run_with_complex_input() {
        let run = WorkflowRun {
            run_id: "run-complex".to_string(),
            workflow_name: "data-pipeline".to_string(),
            status: WorkflowStatus::Running,
            started_at: "2024-01-01".to_string(),
            completed_at: None,
            input: serde_json::json!({
                "nested": {"key": "value"},
                "array": [1, 2, 3],
                "flag": true
            }),
            output: None,
        };
        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("data-pipeline"));
        let deserialized: WorkflowRun = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.input["nested"]["key"], "value");
    }

    #[test]
    fn test_workflow_status_deserialize() {
        let statuses = vec!["Pending", "Running", "Completed", "Failed", "Cancelled"];
        for name in statuses {
            let json = String::from("\"") + name + "\"";
            let status: WorkflowStatus = serde_json::from_str(&json).unwrap();
            let serialized = serde_json::to_string(&status).unwrap();
            assert!(serialized.contains(name));
        }
    }
}
