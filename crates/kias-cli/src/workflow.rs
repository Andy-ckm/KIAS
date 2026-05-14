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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
