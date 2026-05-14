use super::state::WorkflowState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 检查点（借鉴 LangGraph Checkpointing）
///
/// 核心设计：
/// 1. 每个节点执行后生成检查点
/// 2. 支持从任意检查点恢复
/// 3. 支持时间旅行调试
/// 4. 支持人类介入修正后继续执行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub workflow_id: String,
    pub node_id: String,
    pub state: WorkflowState,
    pub created_at: DateTime<Utc>,
}

/// 检查点存储
pub struct CheckpointStore {
    checkpoints: dashmap::DashMap<String, Vec<Checkpoint>>,
}

impl Default for CheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointStore {
    pub fn new() -> Self {
        Self {
            checkpoints: dashmap::DashMap::new(),
        }
    }

    /// 保存检查点
    pub fn save(&self, checkpoint: Checkpoint) {
        let mut entry = self
            .checkpoints
            .entry(checkpoint.workflow_id.clone())
            .or_default();
        entry.push(checkpoint);
    }

    /// 获取最新检查点
    pub fn get_latest(&self, workflow_id: &str) -> Option<Checkpoint> {
        self.checkpoints
            .get(workflow_id)
            .and_then(|checkpoints| checkpoints.last().cloned())
    }

    /// 获取指定检查点
    pub fn get(&self, workflow_id: &str, checkpoint_id: &str) -> Option<Checkpoint> {
        self.checkpoints
            .get(workflow_id)
            .and_then(|checkpoints| checkpoints.iter().find(|c| c.id == checkpoint_id).cloned())
    }

    /// 获取所有检查点
    pub fn get_all(&self, workflow_id: &str) -> Vec<Checkpoint> {
        self.checkpoints
            .get(workflow_id)
            .map(|c| c.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WorkflowState;

    #[test]
    fn test_checkpoint_store() {
        let store = CheckpointStore::new();

        let cp = Checkpoint {
            id: "cp-1".to_string(),
            workflow_id: "wf-1".to_string(),
            node_id: "node-1".to_string(),
            state: WorkflowState::new("wf-1", "node-1"),
            created_at: chrono::Utc::now(),
        };

        store.save(cp);

        let latest = store.get_latest("wf-1");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().id, "cp-1");

        let by_id = store.get("wf-1", "cp-1");
        assert!(by_id.is_some());

        let all = store.get_all("wf-1");
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_checkpoint_store_empty() {
        let store = CheckpointStore::new();
        assert!(store.get_latest("nope").is_none());
        assert!(store.get("nope", "nope").is_none());
        assert!(store.get_all("nope").is_empty());
    }

    #[test]
    fn test_multiple_checkpoints() {
        let store = CheckpointStore::new();

        for i in 0..3 {
            store.save(Checkpoint {
                id: format!("cp-{}", i),
                workflow_id: "wf-1".to_string(),
                node_id: format!("node-{}", i),
                state: WorkflowState::new("wf-1", &format!("node-{}", i)),
                created_at: chrono::Utc::now(),
            });
        }

        let all = store.get_all("wf-1");
        assert_eq!(all.len(), 3);

        let latest = store.get_latest("wf-1").unwrap();
        assert_eq!(latest.id, "cp-2");
    }
}
