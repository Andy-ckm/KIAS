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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let s = WorkflowState::new("wf-1", "start");
        assert_eq!(s.workflow_id, "wf-1");
        assert_eq!(s.current_node, "start");
        assert_eq!(s.status, WorkflowStatus::NotStarted);
        assert!(s.data.is_empty());
        assert!(s.history.is_empty());
    }

    #[test]
    fn test_set_and_get() {
        let mut s = WorkflowState::new("wf-1", "start");
        s.set("count", 42);
        assert_eq!(s.get("count"), Some(&serde_json::json!(42)));
        assert_eq!(s.get("missing"), None);
    }

    #[test]
    fn test_set_string_value() {
        let mut s = WorkflowState::new("wf-1", "start");
        s.set("name", "hello");
        assert_eq!(s.get("name"), Some(&serde_json::json!("hello")));
    }

    #[test]
    fn test_transition_records_history() {
        let mut s = WorkflowState::new("wf-1", "start");
        let mut changes = HashMap::new();
        changes.insert("progress".to_string(), serde_json::json!(50));
        s.transition("step2", changes);
        assert_eq!(s.current_node, "step2");
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].from_node, "start");
        assert_eq!(s.history[0].to_node, "step2");
    }

    #[test]
    fn test_transition_merges_data() {
        let mut s = WorkflowState::new("wf-1", "start");
        s.set("keep", "yes");
        let mut changes = HashMap::new();
        changes.insert("new_key".to_string(), serde_json::json!("new_val"));
        s.transition("step2", changes);
        assert_eq!(s.get("keep"), Some(&serde_json::json!("yes")));
        assert_eq!(s.get("new_key"), Some(&serde_json::json!("new_val")));
    }

    #[test]
    fn test_transition_updates_timestamp() {
        let mut s = WorkflowState::new("wf-1", "start");
        let before = s.updated_at;
        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));
        s.transition("step2", HashMap::new());
        assert!(s.updated_at >= before);
    }

    #[test]
    fn test_multiple_transitions() {
        let mut s = WorkflowState::new("wf-1", "a");
        s.transition("b", HashMap::new());
        s.transition("c", HashMap::new());
        assert_eq!(s.current_node, "c");
        assert_eq!(s.history.len(), 2);
        assert_eq!(s.history[0].to_node, "b");
        assert_eq!(s.history[1].to_node, "c");
    }

    #[test]
    fn test_status_variants() {
        let mut s = WorkflowState::new("wf-1", "start");
        s.status = WorkflowStatus::Running;
        assert_eq!(s.status, WorkflowStatus::Running);
        s.status = WorkflowStatus::Completed;
        assert_eq!(s.status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        let mut s = WorkflowState::new("wf-1", "start");
        s.set("key", "value");
        s.transition("step2", HashMap::new());
        let json = serde_json::to_string(&s).unwrap();
        let restored: WorkflowState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.workflow_id, "wf-1");
        assert_eq!(restored.current_node, "step2");
        assert_eq!(restored.get("key"), Some(&serde_json::json!("value")));
        assert_eq!(restored.history.len(), 1);
    }
}
