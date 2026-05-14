use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工作流状态（在图中流转）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub workflow_id: String,
    pub current_node: String,
    pub status: WorkflowStatus,
    pub data: HashMap<String, serde_json::Value>,
    pub history: Vec<StateTransition>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 工作流状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    NotStarted,
    Running,
    WaitingForHuman,
    Completed,
    Failed,
    Cancelled,
}

/// 状态转换记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_node: String,
    pub to_node: String,
    pub timestamp: DateTime<Utc>,
    pub data_changes: HashMap<String, serde_json::Value>,
}

impl WorkflowState {
    pub fn new(workflow_id: &str, entry_node: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            current_node: entry_node.to_string(),
            status: WorkflowStatus::NotStarted,
            data: HashMap::new(),
            history: Vec::new(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Set a value in the state data.
    pub fn set(&mut self, key: impl Into<String>, value: impl Serialize) {
        self.data
            .insert(key.into(), serde_json::to_value(value).unwrap_or_default());
        self.updated_at = Utc::now();
    }

    /// 获取数据
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// 状态转换
    pub fn transition(&mut self, to_node: &str, data_changes: HashMap<String, serde_json::Value>) {
        let transition = StateTransition {
            from_node: self.current_node.clone(),
            to_node: to_node.to_string(),
            timestamp: Utc::now(),
            data_changes: data_changes.clone(),
        };
        self.history.push(transition);
        self.current_node = to_node.to_string();
        for (k, v) in data_changes {
            self.data.insert(k, v);
        }
        self.updated_at = Utc::now();
    }
}
