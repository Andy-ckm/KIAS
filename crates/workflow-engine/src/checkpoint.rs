use super::state::WorkflowState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// Checkpoint (inspired by LangGraph Checkpointing)
///
/// Core design:
/// 1. Each node execution generates a checkpoint
/// 2. Supports restoring from any checkpoint
/// 3. Supports time-travel debugging
/// 4. Supports human-in-the-loop correction and resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub workflow_id: String,
    pub node_id: String,
    pub state: WorkflowState,
    pub created_at: DateTime<Utc>,
}

/// Lightweight checkpoint metadata for listing (avoids loading full state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub workflow_id: String,
    pub node_id: String,
    pub created_at: DateTime<Utc>,
}

impl From<&Checkpoint> for CheckpointInfo {
    fn from(cp: &Checkpoint) -> Self {
        Self {
            id: cp.id.clone(),
            workflow_id: cp.workflow_id.clone(),
            node_id: cp.node_id.clone(),
            created_at: cp.created_at,
        }
    }
}

// ───────────────────────── CheckpointStore trait ──────────────────────────

/// Trait abstracting checkpoint persistence.
///
/// Implementations must be `Send + Sync` so the store can be shared across
/// async tasks via `Arc<dyn CheckpointStore>`.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Persist a checkpoint.
    async fn save_checkpoint(&self, checkpoint: Checkpoint) -> anyhow::Result<()>;

    /// Load a checkpoint. If `checkpoint_id` is `None`, return the latest one.
    async fn load_checkpoint(
        &self,
        workflow_id: &str,
        checkpoint_id: Option<&str>,
    ) -> anyhow::Result<Option<Checkpoint>>;

    /// List all checkpoint metadata for a workflow (ordered by creation time).
    async fn list_checkpoints(&self, workflow_id: &str) -> anyhow::Result<Vec<CheckpointInfo>>;
}

// ───────────────────── InMemoryCheckpointStore ────────────────────────────

/// In-memory checkpoint store backed by `DashMap` (for testing / ephemeral use).
pub struct InMemoryCheckpointStore {
    checkpoints: dashmap::DashMap<String, Vec<Checkpoint>>,
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryCheckpointStore {
    pub fn new() -> Self {
        Self {
            checkpoints: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn save_checkpoint(&self, checkpoint: Checkpoint) -> anyhow::Result<()> {
        let mut entry = self
            .checkpoints
            .entry(checkpoint.workflow_id.clone())
            .or_default();
        entry.push(checkpoint);
        Ok(())
    }

    async fn load_checkpoint(
        &self,
        workflow_id: &str,
        checkpoint_id: Option<&str>,
    ) -> anyhow::Result<Option<Checkpoint>> {
        let result = match checkpoint_id {
            Some(id) => self
                .checkpoints
                .get(workflow_id)
                .and_then(|cps| cps.iter().find(|c| c.id == id).cloned()),
            None => self
                .checkpoints
                .get(workflow_id)
                .and_then(|cps| cps.last().cloned()),
        };
        Ok(result)
    }

    async fn list_checkpoints(&self, workflow_id: &str) -> anyhow::Result<Vec<CheckpointInfo>> {
        let infos = self
            .checkpoints
            .get(workflow_id)
            .map(|cps| cps.iter().map(CheckpointInfo::from).collect())
            .unwrap_or_default();
        Ok(infos)
    }
}

// ───────────────────── SqliteCheckpointStore ──────────────────────────────

/// SQLite-backed checkpoint store for durable persistence.
///
/// Uses `rusqlite` (bundled SQLite) behind a `Mutex<Connection>`.  All
/// operations are short single-statement queries, so synchronous locking
/// inside the async trait methods is acceptable.
pub struct SqliteCheckpointStore {
    conn: Mutex<Connection>,
}

impl SqliteCheckpointStore {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                id          TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                node_id     TEXT NOT NULL,
                state_json  TEXT NOT NULL,
                created_at  TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_checkpoints_workflow
                ON checkpoints(workflow_id, created_at);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite database (for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::open(":memory:")
    }
}

#[async_trait]
impl CheckpointStore for SqliteCheckpointStore {
    async fn save_checkpoint(&self, checkpoint: Checkpoint) -> anyhow::Result<()> {
        let state_json = serde_json::to_string(&checkpoint.state)?;
        let created_at = checkpoint.created_at.to_rfc3339();
        // Lock the connection and execute the INSERT synchronously.
        // The critical section is very short (single INSERT), so blocking
        // the async task briefly is acceptable.
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO checkpoints (id, workflow_id, node_id, state_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                checkpoint.id,
                checkpoint.workflow_id,
                checkpoint.node_id,
                state_json,
                created_at
            ],
        )?;
        Ok(())
    }

    async fn load_checkpoint(
        &self,
        workflow_id: &str,
        checkpoint_id: Option<&str>,
    ) -> anyhow::Result<Option<Checkpoint>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;

        let (sql, param_values): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match checkpoint_id
        {
            Some(id) => (
                "SELECT id, workflow_id, node_id, state_json, created_at \
                 FROM checkpoints WHERE workflow_id = ?1 AND id = ?2 \
                 ORDER BY created_at DESC LIMIT 1",
                vec![Box::new(workflow_id.to_string()), Box::new(id.to_string())],
            ),
            None => (
                "SELECT id, workflow_id, node_id, state_json, created_at \
                 FROM checkpoints WHERE workflow_id = ?1 \
                 ORDER BY created_at DESC LIMIT 1",
                vec![Box::new(workflow_id.to_string())],
            ),
        };

        let mut stmt = conn.prepare(sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut row_iter = stmt.query_map(params_ref.as_slice(), row_to_checkpoint)?;

        if let Some(row) = row_iter.next() {
            return Ok(Some(row?));
        }
        Ok(None)
    }

    async fn list_checkpoints(&self, workflow_id: &str) -> anyhow::Result<Vec<CheckpointInfo>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, node_id, created_at
             FROM checkpoints
             WHERE workflow_id = ?1
             ORDER BY created_at ASC",
        )?;

        let infos = stmt
            .query_map(params![workflow_id], |row| {
                Ok(CheckpointInfo {
                    id: row.get(0)?,
                    workflow_id: row.get(1)?,
                    node_id: row.get(2)?,
                    created_at: {
                        let s: String = row.get(3)?;
                        chrono::DateTime::parse_from_rfc3339(&s)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now())
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(infos)
    }
}

/// Helper: map a SQLite row to a `Checkpoint`.
fn row_to_checkpoint(row: &rusqlite::Row<'_>) -> rusqlite::Result<Checkpoint> {
    let state_json: String = row.get(3)?;
    let created_at_str: String = row.get(4)?;

    let state: WorkflowState = serde_json::from_str(&state_json)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(Checkpoint {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        node_id: row.get(2)?,
        state,
        created_at,
    })
}

// ───────────────────── Backward-compatible alias ──────────────────────────

/// Legacy alias — prefer [`InMemoryCheckpointStore`].
pub type CheckpointStoreImpl = InMemoryCheckpointStore;

// ───────────────────────────── Tests ──────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WorkflowState;

    // ── InMemoryCheckpointStore ───────────────────────────────────────

    #[tokio::test]
    async fn test_in_memory_save_and_load() {
        let store = InMemoryCheckpointStore::new();

        let cp = Checkpoint {
            id: "cp-1".to_string(),
            workflow_id: "wf-1".to_string(),
            node_id: "node-1".to_string(),
            state: WorkflowState::new("wf-1", "node-1"),
            created_at: chrono::Utc::now(),
        };
        store.save_checkpoint(cp).await.unwrap();

        let loaded = store.load_checkpoint("wf-1", None).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "cp-1");

        let by_id = store.load_checkpoint("wf-1", Some("cp-1")).await.unwrap();
        assert!(by_id.is_some());

        let list = store.list_checkpoints("wf-1").await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_empty() {
        let store = InMemoryCheckpointStore::new();
        assert!(store.load_checkpoint("nope", None).await.unwrap().is_none());
        assert!(store.list_checkpoints("nope").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_multiple_checkpoints() {
        let store = InMemoryCheckpointStore::new();

        for i in 0..3 {
            store
                .save_checkpoint(Checkpoint {
                    id: format!("cp-{i}"),
                    workflow_id: "wf-1".to_string(),
                    node_id: format!("node-{i}"),
                    state: WorkflowState::new("wf-1", &format!("node-{i}")),
                    created_at: chrono::Utc::now(),
                })
                .await
                .unwrap();
        }

        let list = store.list_checkpoints("wf-1").await.unwrap();
        assert_eq!(list.len(), 3);

        let latest = store.load_checkpoint("wf-1", None).await.unwrap().unwrap();
        assert_eq!(latest.id, "cp-2");
    }

    // ── SqliteCheckpointStore ────────────────────────────────────────

    #[tokio::test]
    async fn test_sqlite_save_and_load() {
        let store = SqliteCheckpointStore::open_in_memory().unwrap();

        let cp = Checkpoint {
            id: "cp-s1".to_string(),
            workflow_id: "wf-s1".to_string(),
            node_id: "node-1".to_string(),
            state: WorkflowState::new("wf-s1", "node-1"),
            created_at: chrono::Utc::now(),
        };
        store.save_checkpoint(cp).await.unwrap();

        let loaded = store.load_checkpoint("wf-s1", None).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "cp-s1");
        assert_eq!(loaded.workflow_id, "wf-s1");
    }

    #[tokio::test]
    async fn test_sqlite_load_by_id() {
        let store = SqliteCheckpointStore::open_in_memory().unwrap();

        for i in 0..3 {
            store
                .save_checkpoint(Checkpoint {
                    id: format!("cp-{i}"),
                    workflow_id: "wf-1".to_string(),
                    node_id: format!("node-{i}"),
                    state: WorkflowState::new("wf-1", &format!("node-{i}")),
                    created_at: chrono::Utc::now(),
                })
                .await
                .unwrap();
        }

        let specific = store.load_checkpoint("wf-1", Some("cp-1")).await.unwrap();
        assert!(specific.is_some());
        assert_eq!(specific.unwrap().node_id, "node-1");

        let latest = store.load_checkpoint("wf-1", None).await.unwrap().unwrap();
        assert_eq!(latest.id, "cp-2");
    }

    #[tokio::test]
    async fn test_sqlite_list_checkpoints() {
        let store = SqliteCheckpointStore::open_in_memory().unwrap();

        for i in 0..3 {
            store
                .save_checkpoint(Checkpoint {
                    id: format!("cp-{i}"),
                    workflow_id: "wf-list".to_string(),
                    node_id: format!("node-{i}"),
                    state: WorkflowState::new("wf-list", &format!("node-{i}")),
                    created_at: chrono::Utc::now(),
                })
                .await
                .unwrap();
        }

        let list = store.list_checkpoints("wf-list").await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, "cp-0");
        assert_eq!(list[2].id, "cp-2");

        // Empty for unknown workflow
        let empty = store.list_checkpoints("unknown").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_sqlite_empty() {
        let store = SqliteCheckpointStore::open_in_memory().unwrap();
        assert!(store.load_checkpoint("nope", None).await.unwrap().is_none());
        assert!(store.list_checkpoints("nope").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sqlite_state_roundtrip() {
        let store = SqliteCheckpointStore::open_in_memory().unwrap();

        let mut state = WorkflowState::new("wf-rt", "node-1");
        state.set("key1", "value1");
        state.set("count", 42);

        store
            .save_checkpoint(Checkpoint {
                id: "cp-rt".to_string(),
                workflow_id: "wf-rt".to_string(),
                node_id: "node-1".to_string(),
                state,
                created_at: chrono::Utc::now(),
            })
            .await
            .unwrap();

        let loaded = store.load_checkpoint("wf-rt", None).await.unwrap().unwrap();
        assert_eq!(
            loaded.state.data.get("key1").unwrap().as_str().unwrap(),
            "value1"
        );
        assert_eq!(
            loaded.state.data.get("count").unwrap().as_i64().unwrap(),
            42
        );
    }
}
