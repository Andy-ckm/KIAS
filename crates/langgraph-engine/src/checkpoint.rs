//! Checkpoint persistence for interrupt/resume support.
//!
//! Provides a `CheckpointStore` trait for pluggable storage backends,
//! plus an `InMemoryCheckpointStore` implementation using `RwLock`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::state::GraphState;

/// A checkpoint capturing graph state at a specific node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint identifier.
    pub id: String,
    /// The run this checkpoint belongs to.
    pub run_id: String,
    /// The node that was executing when checkpoint was created.
    pub node: String,
    /// Snapshot of the graph state.
    pub state: GraphState,
    /// When this checkpoint was created.
    pub timestamp: DateTime<Utc>,
    /// Version counter for ordering checkpoints within a run.
    pub version: u64,
}

/// Trait for checkpoint storage backends.
///
/// Implementors must be `Send + Sync` to support concurrent graph execution.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Save a new checkpoint. Returns the checkpoint ID.
    async fn save(&self, checkpoint: Checkpoint) -> Result<String, kias_common::KiasError>;

    /// Load the latest checkpoint for a given run.
    async fn load_latest(&self, run_id: &str)
        -> Result<Option<Checkpoint>, kias_common::KiasError>;

    /// Load a specific checkpoint by ID.
    async fn load_by_id(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<Checkpoint>, kias_common::KiasError>;

    /// Load all checkpoints for a given run, ordered by version.
    async fn load_history(&self, run_id: &str) -> Result<Vec<Checkpoint>, kias_common::KiasError>;

    /// Delete all checkpoints for a given run.
    async fn delete_run(&self, run_id: &str) -> Result<(), kias_common::KiasError>;
}

/// In-memory checkpoint store using `RwLock<HashMap>`.
///
/// Suitable for development and single-process deployments.
/// For production, implement `CheckpointStore` with etcd or SQLite.
pub struct InMemoryCheckpointStore {
    /// Map from checkpoint_id → checkpoint.
    by_id: RwLock<HashMap<String, Checkpoint>>,
    /// Map from run_id → list of checkpoint_ids (ordered by version).
    by_run: RwLock<HashMap<String, Vec<String>>>,
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryCheckpointStore {
    pub fn new() -> Self {
        Self {
            by_id: RwLock::new(HashMap::new()),
            by_run: RwLock::new(HashMap::new()),
        }
    }

    /// Get the total number of stored checkpoints.
    pub fn count(&self) -> usize {
        self.by_id.read().map(|m| m.len()).unwrap_or(0)
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn save(&self, checkpoint: Checkpoint) -> Result<String, kias_common::KiasError> {
        let id = checkpoint.id.clone();
        let run_id = checkpoint.run_id.clone();

        {
            let mut by_id = self
                .by_id
                .write()
                .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;
            by_id.insert(id.clone(), checkpoint);
        }

        {
            let mut by_run = self
                .by_run
                .write()
                .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;
            by_run.entry(run_id).or_default().push(id.clone());
        }

        Ok(id)
    }

    async fn load_latest(
        &self,
        run_id: &str,
    ) -> Result<Option<Checkpoint>, kias_common::KiasError> {
        let by_run = self
            .by_run
            .read()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;

        let ids = match by_run.get(run_id) {
            Some(ids) => ids,
            None => return Ok(None),
        };

        let latest_id = match ids.last() {
            Some(id) => id,
            None => return Ok(None),
        };

        let by_id = self
            .by_id
            .read()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;

        Ok(by_id.get(latest_id).cloned())
    }

    async fn load_by_id(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<Checkpoint>, kias_common::KiasError> {
        let by_id = self
            .by_id
            .read()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;
        Ok(by_id.get(checkpoint_id).cloned())
    }

    async fn load_history(&self, run_id: &str) -> Result<Vec<Checkpoint>, kias_common::KiasError> {
        let by_run = self
            .by_run
            .read()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;

        let ids = match by_run.get(run_id) {
            Some(ids) => ids,
            None => return Ok(Vec::new()),
        };

        let by_id = self
            .by_id
            .read()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;

        let mut checkpoints: Vec<Checkpoint> =
            ids.iter().filter_map(|id| by_id.get(id).cloned()).collect();

        checkpoints.sort_by_key(|c| c.version);
        Ok(checkpoints)
    }

    async fn delete_run(&self, run_id: &str) -> Result<(), kias_common::KiasError> {
        let ids = {
            let mut by_run = self
                .by_run
                .write()
                .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;
            by_run.remove(run_id).unwrap_or_default()
        };

        let mut by_id = self
            .by_id
            .write()
            .map_err(|e| kias_common::KiasError::Storage(format!("Lock poisoned: {}", e)))?;
        for id in ids {
            by_id.remove(&id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GraphState;

    fn make_checkpoint(id: &str, run_id: &str, node: &str, version: u64) -> Checkpoint {
        let mut state = GraphState::new();
        state.set("step", version as i32);
        Checkpoint {
            id: id.to_string(),
            run_id: run_id.to_string(),
            node: node.to_string(),
            state,
            timestamp: Utc::now(),
            version,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_by_id() {
        let store = InMemoryCheckpointStore::new();
        let cp = make_checkpoint("cp1", "run1", "start", 0);
        let id = store.save(cp).await.unwrap();
        assert_eq!(id, "cp1");

        let loaded = store.load_by_id("cp1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().node, "start");
    }

    #[tokio::test]
    async fn test_load_by_id_missing() {
        let store = InMemoryCheckpointStore::new();
        let loaded = store.load_by_id("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_load_latest() {
        let store = InMemoryCheckpointStore::new();
        store
            .save(make_checkpoint("cp1", "run1", "node_a", 0))
            .await
            .unwrap();
        store
            .save(make_checkpoint("cp2", "run1", "node_b", 1))
            .await
            .unwrap();

        let latest = store.load_latest("run1").await.unwrap().unwrap();
        assert_eq!(latest.id, "cp2");
    }

    #[tokio::test]
    async fn test_load_latest_missing_run() {
        let store = InMemoryCheckpointStore::new();
        let latest = store.load_latest("no_such_run").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_load_history_ordered() {
        let store = InMemoryCheckpointStore::new();
        store
            .save(make_checkpoint("cp1", "run1", "a", 2))
            .await
            .unwrap();
        store
            .save(make_checkpoint("cp2", "run1", "b", 0))
            .await
            .unwrap();
        store
            .save(make_checkpoint("cp3", "run1", "c", 1))
            .await
            .unwrap();

        let history = store.load_history("run1").await.unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].version, 0);
        assert_eq!(history[1].version, 1);
        assert_eq!(history[2].version, 2);
    }

    #[tokio::test]
    async fn test_load_history_empty_run() {
        let store = InMemoryCheckpointStore::new();
        let history = store.load_history("no_run").await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_delete_run() {
        let store = InMemoryCheckpointStore::new();
        store
            .save(make_checkpoint("cp1", "run1", "a", 0))
            .await
            .unwrap();
        store
            .save(make_checkpoint("cp2", "run1", "b", 1))
            .await
            .unwrap();
        assert_eq!(store.count(), 2);

        store.delete_run("run1").await.unwrap();
        assert_eq!(store.count(), 0);
        assert!(store.load_latest("run1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_run() {
        let store = InMemoryCheckpointStore::new();
        store.delete_run("no_run").await.unwrap(); // should not error
    }

    #[tokio::test]
    async fn test_count() {
        let store = InMemoryCheckpointStore::new();
        assert_eq!(store.count(), 0);
        store
            .save(make_checkpoint("cp1", "run1", "a", 0))
            .await
            .unwrap();
        assert_eq!(store.count(), 1);
        store
            .save(make_checkpoint("cp2", "run1", "b", 1))
            .await
            .unwrap();
        assert_eq!(store.count(), 2);
    }

    #[tokio::test]
    async fn test_multiple_runs_isolated() {
        let store = InMemoryCheckpointStore::new();
        store
            .save(make_checkpoint("cp1", "run1", "a", 0))
            .await
            .unwrap();
        store
            .save(make_checkpoint("cp2", "run2", "b", 0))
            .await
            .unwrap();

        store.delete_run("run1").await.unwrap();
        assert_eq!(store.count(), 1);
        assert!(store.load_latest("run2").await.unwrap().is_some());
    }

    #[test]
    fn test_default_trait() {
        let store = InMemoryCheckpointStore::default();
        assert_eq!(store.count(), 0);
    }
}
