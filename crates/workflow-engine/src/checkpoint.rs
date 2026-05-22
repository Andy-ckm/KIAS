use super::state::WorkflowState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

// ══════════════════════════════════════════════════════════════════════════
//  WAL (Write-Ahead Log) — delta-based checkpoint recovery
// ══════════════════════════════════════════════════════════════════════════
//
// Instead of persisting the entire WorkflowState as a JSON blob at every
// checkpoint, the WAL approach writes a compact *delta record* after each
// node execution:
//
//   WalRecord { workflow_id, seq, node_id, delta, status, ... }
//
// Recovery replays all WAL records for a workflow in sequence-order,
// reconstructing the state incrementally.  This is far cheaper in I/O
// for large states and naturally supports crash recovery: only the last
// partial record needs to be discarded.

/// A single WAL delta record — captures what changed after a node executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalRecord {
    /// Monotonically increasing sequence number (per workflow).
    pub seq: u64,
    /// Workflow this record belongs to.
    pub workflow_id: String,
    /// Node that just executed.
    pub node_id: String,
    /// Key-value changes applied to the state data.
    pub delta: HashMap<String, serde_json::Value>,
    /// Workflow status after this node executed.
    pub status: WalStatus,
    /// Whether this workflow's execution has completed (terminal state).
    pub terminal: bool,
    /// Timestamp.
    pub created_at: DateTime<Utc>,
}

/// Simplified workflow status for WAL records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalStatus {
    Running,
    WaitingForHuman,
    Completed,
    Failed,
}

/// Metadata for listing incomplete WAL-based workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalWorkflowInfo {
    pub workflow_id: String,
    pub last_node_id: String,
    pub last_seq: u64,
    pub status: WalStatus,
    pub created_at: DateTime<Utc>,
}

// ─── WalStore trait ───────────────────────────────────────────────────────

/// Trait for WAL persistence.
#[async_trait]
pub trait WalStore: Send + Sync {
    /// Append a WAL record.
    async fn append(&self, record: WalRecord) -> anyhow::Result<()>;

    /// Read all WAL records for a workflow, ordered by seq.
    async fn read_records(&self, workflow_id: &str) -> anyhow::Result<Vec<WalRecord>>;

    /// List all workflows that have an incomplete (non-terminal) WAL.
    async fn list_incomplete(&self) -> anyhow::Result<Vec<WalWorkflowInfo>>;

    /// Get the next sequence number for a workflow.
    async fn next_seq(&self, workflow_id: &str) -> anyhow::Result<u64>;
}

// ─── InMemoryWalStore ─────────────────────────────────────────────────────

/// In-memory WAL store (for testing / ephemeral use).
pub struct InMemoryWalStore {
    records: dashmap::DashMap<String, Vec<WalRecord>>,
}

impl Default for InMemoryWalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryWalStore {
    pub fn new() -> Self {
        Self {
            records: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl WalStore for InMemoryWalStore {
    async fn append(&self, record: WalRecord) -> anyhow::Result<()> {
        let mut entry = self.records.entry(record.workflow_id.clone()).or_default();
        entry.push(record);
        Ok(())
    }

    async fn read_records(&self, workflow_id: &str) -> anyhow::Result<Vec<WalRecord>> {
        let records = self
            .records
            .get(workflow_id)
            .map(|r| r.clone())
            .unwrap_or_default();
        Ok(records)
    }

    async fn list_incomplete(&self) -> anyhow::Result<Vec<WalWorkflowInfo>> {
        let mut result = Vec::new();
        for entry in self.records.iter() {
            if let Some(last) = entry.value().last() {
                if !last.terminal {
                    result.push(WalWorkflowInfo {
                        workflow_id: last.workflow_id.clone(),
                        last_node_id: last.node_id.clone(),
                        last_seq: last.seq,
                        status: last.status.clone(),
                        created_at: last.created_at,
                    });
                }
            }
        }
        Ok(result)
    }

    async fn next_seq(&self, workflow_id: &str) -> anyhow::Result<u64> {
        let seq = self
            .records
            .get(workflow_id)
            .map(|r| r.last().map(|rec| rec.seq + 1).unwrap_or(1))
            .unwrap_or(1);
        Ok(seq)
    }
}

// ─── SqliteWalStore ───────────────────────────────────────────────────────

/// SQLite-backed WAL store for durable persistence.
pub struct SqliteWalStore {
    conn: Mutex<Connection>,
}

impl SqliteWalStore {
    /// Open (or create) a SQLite WAL database.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS wal_records (
                seq         INTEGER NOT NULL,
                workflow_id TEXT    NOT NULL,
                node_id     TEXT    NOT NULL,
                delta_json  TEXT    NOT NULL,
                status      TEXT    NOT NULL,
                terminal    INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT    NOT NULL,
                PRIMARY KEY (workflow_id, seq)
            );
            CREATE INDEX IF NOT EXISTS idx_wal_workflow
                ON wal_records(workflow_id, seq);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite WAL database (for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::open(":memory:")
    }
}

#[async_trait]
impl WalStore for SqliteWalStore {
    async fn append(&self, record: WalRecord) -> anyhow::Result<()> {
        let delta_json = serde_json::to_string(&record.delta)?;
        let status_str = serde_json::to_string(&record.status)?;
        let created_at = record.created_at.to_rfc3339();
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO wal_records (seq, workflow_id, node_id, delta_json, status, terminal, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.seq,
                record.workflow_id,
                record.node_id,
                delta_json,
                status_str,
                record.terminal as i32,
                created_at,
            ],
        )?;
        Ok(())
    }

    async fn read_records(&self, workflow_id: &str) -> anyhow::Result<Vec<WalRecord>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT seq, workflow_id, node_id, delta_json, status, terminal, created_at
             FROM wal_records WHERE workflow_id = ?1 ORDER BY seq ASC",
        )?;

        let records = stmt
            .query_map(params![workflow_id], |row| {
                let delta_json: String = row.get(3)?;
                let status_str: String = row.get(4)?;
                let terminal: i32 = row.get(5)?;
                let created_at_str: String = row.get(6)?;

                let delta: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&delta_json).unwrap_or_default();
                let status: WalStatus =
                    serde_json::from_str(&status_str).unwrap_or(WalStatus::Running);
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(WalRecord {
                    seq: row.get(0)?,
                    workflow_id: row.get(1)?,
                    node_id: row.get(2)?,
                    delta,
                    status,
                    terminal: terminal != 0,
                    created_at,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    async fn list_incomplete(&self) -> anyhow::Result<Vec<WalWorkflowInfo>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT w.workflow_id, w.node_id, w.seq, w.status, w.created_at
             FROM wal_records w
             INNER JOIN (
                 SELECT workflow_id, MAX(seq) as max_seq
                 FROM wal_records
                 GROUP BY workflow_id
             ) m ON w.workflow_id = m.workflow_id AND w.seq = m.max_seq
             WHERE w.terminal = 0",
        )?;

        let infos = stmt
            .query_map([], |row| {
                let status_str: String = row.get(3)?;
                let status: WalStatus =
                    serde_json::from_str(&status_str).unwrap_or(WalStatus::Running);
                let created_at_str: String = row.get(4)?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(WalWorkflowInfo {
                    workflow_id: row.get(0)?,
                    last_node_id: row.get(1)?,
                    last_seq: row.get(2)?,
                    status,
                    created_at,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(infos)
    }

    async fn next_seq(&self, workflow_id: &str) -> anyhow::Result<u64> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT COALESCE(MAX(seq), 0) + 1 FROM wal_records WHERE workflow_id = ?1")?;
        let seq: u64 = stmt.query_row(params![workflow_id], |row| row.get(0))?;
        Ok(seq)
    }
}

// ─── WAL replay helper ────────────────────────────────────────────────────

/// Reconstruct a [`WorkflowState`] by replaying WAL records.
///
/// Applies each record's delta in sequence order.  Returns `None` if
/// there are no records for the workflow.
pub fn replay_wal(
    workflow_id: &str,
    records: &[WalRecord],
    entry_node: &str,
) -> Option<WorkflowState> {
    if records.is_empty() {
        return None;
    }

    let mut state = WorkflowState::new(workflow_id, entry_node);

    for record in records {
        // Apply delta
        for (key, value) in &record.delta {
            state.data.insert(key.clone(), value.clone());
        }

        // Update current node
        state.current_node = record.node_id.clone();

        // Update status
        state.status = match record.status {
            WalStatus::Running => crate::state::WorkflowStatus::Running,
            WalStatus::WaitingForHuman => crate::state::WorkflowStatus::WaitingForHuman,
            WalStatus::Completed => crate::state::WorkflowStatus::Completed,
            WalStatus::Failed => crate::state::WorkflowStatus::Failed,
        };

        state.updated_at = record.created_at;
    }

    Some(state)
}

/// Scan for incomplete workflows and return their IDs + reconstructed state.
///
/// This is the crash-recovery entry point: call at startup to find
/// workflows that were mid-execution when the host crashed.
pub async fn scan_incomplete_workflows(
    wal_store: &dyn WalStore,
) -> anyhow::Result<Vec<(WalWorkflowInfo, Option<WorkflowState>)>> {
    let incomplete = wal_store.list_incomplete().await?;
    let mut result = Vec::new();

    for info in incomplete {
        let records = wal_store.read_records(&info.workflow_id).await?;
        let state = replay_wal(&info.workflow_id, &records, &info.last_node_id);
        result.push((info, state));
    }

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════════════
//  DeltaRecord — detailed per-node checkpoint delta (enhanced WAL record)
// ══════════════════════════════════════════════════════════════════════════
//
// DeltaRecord captures richer information than the basic WalRecord:
// it records both the key-value writes applied and a full state snapshot
// after the node executed, making point-in-time recovery trivial.

/// A detailed delta record written after each node execution.
///
/// Unlike [`WalRecord`] which only carries key-value deltas, `DeltaRecord`
/// also snapshots the entire state after the node, enabling:
/// - Fast full-state restore without replaying all deltas
/// - Detailed audit trail (writes vs. full state)
/// - Debugging with point-in-time state inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaRecord {
    /// The node that just executed.
    pub node_id: String,
    /// Timestamp of the execution.
    pub timestamp: DateTime<Utc>,
    /// Full state snapshot after this node executed.
    pub state_snapshot: WorkflowState,
    /// Key-value pairs written/changed by this node.
    pub writes: HashMap<String, serde_json::Value>,
    /// Workflow ID this record belongs to.
    pub workflow_id: String,
    /// Monotonically increasing sequence number.
    pub seq: u64,
}

impl DeltaRecord {
    /// Create a new DeltaRecord from the current state after a node execution.
    pub fn new(
        node_id: impl Into<String>,
        workflow_id: impl Into<String>,
        seq: u64,
        state: &WorkflowState,
        writes: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            timestamp: Utc::now(),
            state_snapshot: state.clone(),
            writes,
            workflow_id: workflow_id.into(),
            seq,
        }
    }

    /// Convert this DeltaRecord to a WalRecord for WAL-based storage.
    pub fn to_wal_record(&self, status: WalStatus, terminal: bool) -> WalRecord {
        WalRecord {
            seq: self.seq,
            workflow_id: self.workflow_id.clone(),
            node_id: self.node_id.clone(),
            delta: self.writes.clone(),
            status,
            terminal,
            created_at: self.timestamp,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  CheckpointRecovery — startup crash recovery
// ══════════════════════════════════════════════════════════════════════════

/// Recovery result for a single workflow.
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    /// The workflow info from the WAL.
    pub info: WalWorkflowInfo,
    /// Reconstructed state (None if no WAL records exist).
    pub state: Option<WorkflowState>,
    /// Whether recovery was successful (true) or the workflow had errors (false).
    pub success: bool,
}

/// Handles startup crash recovery by scanning WAL stores for incomplete
/// workflows and reconstructing their state.
///
/// # Usage
///
/// ```ignore
/// let recovery = CheckpointRecovery::new(wal_store);
/// let results = recovery.recover_all().await?;
/// for result in results {
///     if result.success {
///         resume_workflow(result.info.workflow_id, result.state.unwrap()).await;
///     }
/// }
/// ```
pub struct CheckpointRecovery<'a> {
    wal_store: &'a dyn WalStore,
}

impl<'a> CheckpointRecovery<'a> {
    pub fn new(wal_store: &'a dyn WalStore) -> Self {
        Self { wal_store }
    }

    /// Scan for all incomplete workflows and reconstruct their state.
    pub async fn recover_all(&self) -> anyhow::Result<Vec<RecoveryResult>> {
        let incomplete = self.wal_store.list_incomplete().await?;
        let mut results = Vec::new();

        for info in incomplete {
            let records = self.wal_store.read_records(&info.workflow_id).await?;
            let state = replay_wal(&info.workflow_id, &records, &info.last_node_id);
            let success = state.is_some() || records.is_empty();
            results.push(RecoveryResult {
                info,
                state,
                success,
            });
        }

        Ok(results)
    }

    /// Recover a specific workflow by ID.
    pub async fn recover_workflow(
        &self,
        workflow_id: &str,
    ) -> anyhow::Result<Option<RecoveryResult>> {
        let records = self.wal_store.read_records(workflow_id).await?;
        if records.is_empty() {
            return Ok(None);
        }

        let last = records.last().unwrap();
        let info = WalWorkflowInfo {
            workflow_id: workflow_id.to_string(),
            last_node_id: last.node_id.clone(),
            last_seq: last.seq,
            status: last.status.clone(),
            created_at: last.created_at,
        };
        let state = replay_wal(workflow_id, &records, &last.node_id);
        let success = state.is_some();

        Ok(Some(RecoveryResult {
            info,
            state,
            success,
        }))
    }

    /// Get the count of incomplete workflows.
    pub async fn incomplete_count(&self) -> anyhow::Result<usize> {
        let incomplete = self.wal_store.list_incomplete().await?;
        Ok(incomplete.len())
    }

    /// Append a DeltaRecord to the WAL store.
    pub async fn write_delta(
        &self,
        delta: &DeltaRecord,
        status: WalStatus,
        terminal: bool,
    ) -> anyhow::Result<()> {
        let record = delta.to_wal_record(status, terminal);
        self.wal_store.append(record).await
    }
}

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

    // ════════════════════════════════════════════════════════════════════
    //  WAL tests
    // ════════════════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_in_memory_wal_append_and_replay() {
        let store = InMemoryWalStore::new();

        // Simulate two node executions
        let mut delta1 = HashMap::new();
        delta1.insert("step1_out".into(), serde_json::json!("hello"));

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-wal-1".into(),
                node_id: "node-1".into(),
                delta: delta1,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let mut delta2 = HashMap::new();
        delta2.insert("step2_out".into(), serde_json::json!(42));

        store
            .append(WalRecord {
                seq: 2,
                workflow_id: "wf-wal-1".into(),
                node_id: "node-2".into(),
                delta: delta2,
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Replay
        let records = store.read_records("wf-wal-1").await.unwrap();
        assert_eq!(records.len(), 2);

        let state = replay_wal("wf-wal-1", &records, "node-1").unwrap();
        assert_eq!(state.current_node, "node-2");
        assert_eq!(state.get("step1_out").unwrap().as_str().unwrap(), "hello");
        assert_eq!(state.get("step2_out").unwrap().as_i64().unwrap(), 42);
        assert_eq!(state.status, crate::state::WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_in_memory_wal_empty_replay() {
        let records: Vec<WalRecord> = vec![];
        let state = replay_wal("wf-empty", &records, "start");
        assert!(state.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_wal_next_seq() {
        let store = InMemoryWalStore::new();

        // First seq for new workflow
        assert_eq!(store.next_seq("wf-new").await.unwrap(), 1);

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-new".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        assert_eq!(store.next_seq("wf-new").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_wal_list_incomplete() {
        let store = InMemoryWalStore::new();

        // Add a running workflow (incomplete)
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-run".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Add a completed workflow (terminal)
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-done".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let incomplete = store.list_incomplete().await.unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].workflow_id, "wf-run");
    }

    #[tokio::test]
    async fn test_sqlite_wal_append_and_replay() {
        let store = SqliteWalStore::open_in_memory().unwrap();

        let mut delta1 = HashMap::new();
        delta1.insert("output_a".into(), serde_json::json!("value_a"));

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-sql-wal".into(),
                node_id: "start".into(),
                delta: delta1,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let mut delta2 = HashMap::new();
        delta2.insert("output_b".into(), serde_json::json!(99));
        delta2.insert("branch".into(), serde_json::json!("left"));

        store
            .append(WalRecord {
                seq: 2,
                workflow_id: "wf-sql-wal".into(),
                node_id: "middle".into(),
                delta: delta2,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let records = store.read_records("wf-sql-wal").await.unwrap();
        assert_eq!(records.len(), 2);

        let state = replay_wal("wf-sql-wal", &records, "start").unwrap();
        assert_eq!(state.current_node, "middle");
        assert_eq!(state.get("output_a").unwrap().as_str().unwrap(), "value_a");
        assert_eq!(state.get("output_b").unwrap().as_i64().unwrap(), 99);
        assert_eq!(state.get("branch").unwrap().as_str().unwrap(), "left");
    }

    #[tokio::test]
    async fn test_sqlite_wal_list_incomplete() {
        let store = SqliteWalStore::open_in_memory().unwrap();

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-inc".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::WaitingForHuman,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let incomplete = store.list_incomplete().await.unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].status, WalStatus::WaitingForHuman);
    }

    #[tokio::test]
    async fn test_sqlite_wal_next_seq() {
        let store = SqliteWalStore::open_in_memory().unwrap();

        assert_eq!(store.next_seq("wf-s").await.unwrap(), 1);

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-s".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        assert_eq!(store.next_seq("wf-s").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_scan_incomplete_workflows() {
        let store = InMemoryWalStore::new();

        // Incomplete workflow
        let mut delta = HashMap::new();
        delta.insert("progress".into(), serde_json::json!("50%"));

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-crash".into(),
                node_id: "step2".into(),
                delta,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let incomplete = scan_incomplete_workflows(&store).await.unwrap();
        assert_eq!(incomplete.len(), 1);
        let (info, state) = &incomplete[0];
        assert_eq!(info.workflow_id, "wf-crash");
        assert_eq!(info.last_node_id, "step2");
        let state = state.as_ref().unwrap();
        assert_eq!(state.get("progress").unwrap().as_str().unwrap(), "50%");
    }

    #[tokio::test]
    async fn test_wal_record_serialization_roundtrip() {
        let mut delta = HashMap::new();
        delta.insert("key".into(), serde_json::json!({"nested": true}));

        let record = WalRecord {
            seq: 7,
            workflow_id: "wf-ser".into(),
            node_id: "n-ser".into(),
            delta,
            status: WalStatus::Failed,
            terminal: true,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&record).unwrap();
        let restored: WalRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.seq, 7);
        assert_eq!(restored.workflow_id, "wf-ser");
        assert_eq!(restored.status, WalStatus::Failed);
        assert!(restored.terminal);
        assert_eq!(
            restored.delta.get("key").unwrap()["nested"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn test_wal_replay_preserves_overwritten_keys() {
        let store = InMemoryWalStore::new();

        // First record sets key
        let mut delta1 = HashMap::new();
        delta1.insert("counter".into(), serde_json::json!(1));

        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-ow".into(),
                node_id: "n1".into(),
                delta: delta1,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Second record overwrites same key
        let mut delta2 = HashMap::new();
        delta2.insert("counter".into(), serde_json::json!(2));

        store
            .append(WalRecord {
                seq: 2,
                workflow_id: "wf-ow".into(),
                node_id: "n2".into(),
                delta: delta2,
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let records = store.read_records("wf-ow").await.unwrap();
        let state = replay_wal("wf-ow", &records, "n1").unwrap();
        // Last write wins
        assert_eq!(state.get("counter").unwrap().as_i64().unwrap(), 2);
    }

    // ── DeltaRecord tests ─────────────────────────────────────────────

    #[test]
    fn test_delta_record_new() {
        let state = WorkflowState::new("wf-dr", "node-1");
        let mut writes = HashMap::new();
        writes.insert("output".into(), serde_json::json!("done"));

        let dr = DeltaRecord::new("node-1", "wf-dr", 1, &state, writes.clone());
        assert_eq!(dr.node_id, "node-1");
        assert_eq!(dr.workflow_id, "wf-dr");
        assert_eq!(dr.seq, 1);
        assert_eq!(dr.writes.len(), 1);
        assert_eq!(dr.state_snapshot.workflow_id, "wf-dr");
    }

    #[test]
    fn test_delta_record_to_wal_record() {
        let state = WorkflowState::new("wf-dr2", "n1");
        let mut writes = HashMap::new();
        writes.insert("key".into(), serde_json::json!(42));

        let dr = DeltaRecord::new("n1", "wf-dr2", 5, &state, writes);
        let wal = dr.to_wal_record(WalStatus::Running, false);

        assert_eq!(wal.seq, 5);
        assert_eq!(wal.workflow_id, "wf-dr2");
        assert_eq!(wal.node_id, "n1");
        assert_eq!(wal.status, WalStatus::Running);
        assert!(!wal.terminal);
        assert_eq!(wal.delta.get("key").unwrap().as_i64().unwrap(), 42);
    }

    #[test]
    fn test_delta_record_serialization_roundtrip() {
        let mut state = WorkflowState::new("wf-ser", "n1");
        state.set("k", "v");
        let mut writes = HashMap::new();
        writes.insert("k".into(), serde_json::json!("v"));

        let dr = DeltaRecord::new("n1", "wf-ser", 3, &state, writes);
        let json = serde_json::to_string(&dr).unwrap();
        let restored: DeltaRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.node_id, "n1");
        assert_eq!(restored.seq, 3);
        assert_eq!(restored.workflow_id, "wf-ser");
        assert_eq!(
            restored.state_snapshot.get("k").unwrap().as_str().unwrap(),
            "v"
        );
    }

    // ── CheckpointRecovery tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_checkpoint_recovery_recover_all_empty() {
        let store = InMemoryWalStore::new();
        let recovery = CheckpointRecovery::new(&store);
        let results = recovery.recover_all().await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_recover_all_incomplete() {
        let store = InMemoryWalStore::new();

        let mut delta = HashMap::new();
        delta.insert("step".into(), serde_json::json!("middle"));
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-rec".into(),
                node_id: "n1".into(),
                delta,
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let recovery = CheckpointRecovery::new(&store);
        let results = recovery.recover_all().await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        let state = results[0].state.as_ref().unwrap();
        assert_eq!(state.get("step").unwrap().as_str().unwrap(), "middle");
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_recover_workflow_not_found() {
        let store = InMemoryWalStore::new();
        let recovery = CheckpointRecovery::new(&store);
        let result = recovery.recover_workflow("nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_recover_specific_workflow() {
        let store = InMemoryWalStore::new();

        let mut delta = HashMap::new();
        delta.insert("result".into(), serde_json::json!(99));
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-spec".into(),
                node_id: "n1".into(),
                delta,
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let recovery = CheckpointRecovery::new(&store);
        let result = recovery.recover_workflow("wf-spec").await.unwrap();
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.success);
        assert_eq!(r.info.workflow_id, "wf-spec");
        assert_eq!(r.info.status, WalStatus::Completed);
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_incomplete_count() {
        let store = InMemoryWalStore::new();

        // Add 2 incomplete workflows
        for wf in &["wf-a", "wf-b"] {
            store
                .append(WalRecord {
                    seq: 1,
                    workflow_id: wf.to_string(),
                    node_id: "n1".into(),
                    delta: HashMap::new(),
                    status: WalStatus::Running,
                    terminal: false,
                    created_at: Utc::now(),
                })
                .await
                .unwrap();
        }

        // Add 1 completed workflow
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-done".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let recovery = CheckpointRecovery::new(&store);
        assert_eq!(recovery.incomplete_count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_write_delta() {
        let store = InMemoryWalStore::new();
        let recovery = CheckpointRecovery::new(&store);

        let state = WorkflowState::new("wf-wd", "n1");
        let mut writes = HashMap::new();
        writes.insert("key".into(), serde_json::json!("val"));
        let dr = DeltaRecord::new("n1", "wf-wd", 1, &state, writes);

        recovery
            .write_delta(&dr, WalStatus::Running, false)
            .await
            .unwrap();

        let records = store.read_records("wf-wd").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].node_id, "n1");
        assert_eq!(
            records[0].delta.get("key").unwrap().as_str().unwrap(),
            "val"
        );
    }

    #[tokio::test]
    async fn test_checkpoint_recovery_mixed_complete_incomplete() {
        let store = InMemoryWalStore::new();

        // Complete workflow
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-complete".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Completed,
                terminal: true,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Incomplete workflow
        store
            .append(WalRecord {
                seq: 1,
                workflow_id: "wf-incomplete".into(),
                node_id: "n1".into(),
                delta: HashMap::new(),
                status: WalStatus::Running,
                terminal: false,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let recovery = CheckpointRecovery::new(&store);
        let results = recovery.recover_all().await.unwrap();
        // Only incomplete workflows should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].info.workflow_id, "wf-incomplete");
    }
}
