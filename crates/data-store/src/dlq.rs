//! Dead Letter Queue (DLQ) for permanently failed tasks.
//!
//! When a task exhausts all retries, it is moved to the DLQ instead of being
//! silently dropped. Operators can inspect, retry, or discard DLQ entries.

use chrono::{DateTime, Utc};
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Reasons a task enters the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeadLetterReason {
    /// Task exceeded max retries.
    MaxRetriesExceeded,
    /// Task timed out and cannot be retried.
    Timeout,
    /// Task was explicitly cancelled by operator.
    Cancelled,
    /// Dependency (agent, workflow) no longer exists.
    DependencyMissing,
    /// Unknown/unclassified failure.
    Unknown,
}

impl std::fmt::Display for DeadLetterReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxRetriesExceeded => write!(f, "max_retries_exceeded"),
            Self::Timeout => write!(f, "timeout"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::DependencyMissing => write!(f, "dependency_missing"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

fn reason_from_str(s: &str) -> DeadLetterReason {
    match s {
        "max_retries_exceeded" => DeadLetterReason::MaxRetriesExceeded,
        "timeout" => DeadLetterReason::Timeout,
        "cancelled" => DeadLetterReason::Cancelled,
        "dependency_missing" => DeadLetterReason::DependencyMissing,
        _ => DeadLetterReason::Unknown,
    }
}

/// A single entry in the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub task_name: String,
    pub task_type: String,
    pub input: Option<String>,
    pub last_error: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub failed_at: DateTime<Utc>,
    pub original_created_at: Option<String>,
    pub reason: DeadLetterReason,
    pub can_retry: bool,
    pub metadata: String,
}

/// SQLite-backed dead letter queue.
#[derive(Debug, Clone)]
pub struct DeadLetterQueue {
    pool: SqlitePool,
}

impl DeadLetterQueue {
    /// Create a new DLQ backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Enqueue a permanently failed task into the DLQ.
    #[allow(clippy::too_many_arguments)]
    pub async fn enqueue(
        &self,
        task_id: &str,
        agent_id: &str,
        workflow_id: Option<&str>,
        task_name: &str,
        task_type: &str,
        input: Option<&str>,
        last_error: &str,
        retry_count: i32,
        max_retries: i32,
        reason: DeadLetterReason,
    ) -> KiasResult<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO dead_letter_queue (id, task_id, agent_id, workflow_id, task_name, task_type, input, last_error, retry_count, max_retries, failed_at, original_created_at, dead_letter_reason, can_retry, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(task_id)
        .bind(agent_id)
        .bind(workflow_id)
        .bind(task_name)
        .bind(task_type)
        .bind(input)
        .bind(last_error)
        .bind(retry_count)
        .bind(max_retries)
        .bind(&now)
        .bind(&now) // original_created_at = now for new entries
        .bind(reason.to_string())
        .bind(1i32) // can_retry = true by default
        .bind("{}")
        .execute(&self.pool)
        .await
        .map_err(|e| kias_common::KiasError::Config(format!("dlq enqueue failed: {e}")))?;

        info!(
            dlq_id = %id,
            task_id = %task_id,
            agent_id = %agent_id,
            reason = %reason,
            retry_count = retry_count,
            "task moved to dead letter queue"
        );

        Ok(id)
    }

    /// List all DLQ entries, optionally filtered by agent.
    pub async fn list(
        &self,
        agent_id: Option<&str>,
        can_retry_only: bool,
        limit: i64,
    ) -> KiasResult<Vec<DeadLetterEntry>> {
        let mut sql = String::from(
            "SELECT id, task_id, agent_id, workflow_id, task_name, task_type, input, last_error, retry_count, max_retries, failed_at, original_created_at, dead_letter_reason, can_retry, metadata FROM dead_letter_queue WHERE 1=1"
        );
        let mut binds: Vec<String> = Vec::new();

        if let Some(aid) = agent_id {
            sql.push_str(" AND agent_id = ?");
            binds.push(aid.to_string());
        }
        if can_retry_only {
            sql.push_str(" AND can_retry = 1");
        }
        sql.push_str(" ORDER BY failed_at DESC LIMIT ?");

        let mut query = sqlx::query_as::<_, DlqRow>(&sql);
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(limit);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| kias_common::KiasError::Config(format!("dlq list failed: {e}")))?;

        Ok(rows.into_iter().map(DlqRow::into_entry).collect())
    }

    /// Get a specific DLQ entry by its ID.
    pub async fn get(&self, id: &str) -> KiasResult<Option<DeadLetterEntry>> {
        let row = sqlx::query_as::<_, DlqRow>(
            "SELECT id, task_id, agent_id, workflow_id, task_name, task_type, input, last_error, retry_count, max_retries, failed_at, original_created_at, dead_letter_reason, can_retry, metadata FROM dead_letter_queue WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| kias_common::KiasError::Config(format!("dlq get failed: {e}")))?;

        Ok(row.map(DlqRow::into_entry))
    }

    /// Get a DLQ entry by task_id.
    pub async fn get_by_task(&self, task_id: &str) -> KiasResult<Option<DeadLetterEntry>> {
        let row = sqlx::query_as::<_, DlqRow>(
            "SELECT id, task_id, agent_id, workflow_id, task_name, task_type, input, last_error, retry_count, max_retries, failed_at, original_created_at, dead_letter_reason, can_retry, metadata FROM dead_letter_queue WHERE task_id = ?"
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| kias_common::KiasError::Config(format!("dlq get_by_task failed: {e}")))?;

        Ok(row.map(DlqRow::into_entry))
    }

    /// Retry a DLQ entry: mark it as retried and return the entry data so the
    /// caller can re-submit the task.
    pub async fn retry(&self, id: &str) -> KiasResult<Option<DeadLetterEntry>> {
        let entry = self.get(id).await?;
        if let Some(ref e) = entry {
            if !e.can_retry {
                warn!(dlq_id = %id, "attempted to retry non-retryable DLQ entry");
                return Ok(None);
            }
            // Mark as no longer retryable (the caller will re-create the task)
            sqlx::query("UPDATE dead_letter_queue SET can_retry = 0 WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    kias_common::KiasError::Config(format!("dlq retry update failed: {e}"))
                })?;
            debug!(dlq_id = %id, task_id = %e.task_id, "DLQ entry marked for retry");
        }
        Ok(entry)
    }

    /// Remove a DLQ entry (discard permanently).
    pub async fn discard(&self, id: &str) -> KiasResult<bool> {
        let result = sqlx::query("DELETE FROM dead_letter_queue WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| kias_common::KiasError::Config(format!("dlq discard failed: {e}")))?;
        Ok(result.rows_affected() > 0)
    }

    /// Get DLQ statistics.
    pub async fn stats(&self) -> KiasResult<DlqStats> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM dead_letter_queue")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| kias_common::KiasError::Config(format!("dlq stats failed: {e}")))?;

        let retryable: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM dead_letter_queue WHERE can_retry = 1")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| kias_common::KiasError::Config(format!("dlq stats failed: {e}")))?;

        Ok(DlqStats {
            total: total.0,
            retryable: retryable.0,
            discarded: total.0 - retryable.0,
        })
    }

    /// Purge entries older than the given number of days.
    pub async fn purge_older_than(&self, days: i64) -> KiasResult<u64> {
        let result = sqlx::query(
            "DELETE FROM dead_letter_queue WHERE failed_at < datetime('now', ? || ' days')",
        )
        .bind(format!("-{days}"))
        .execute(&self.pool)
        .await
        .map_err(|e| kias_common::KiasError::Config(format!("dlq purge failed: {e}")))?;
        Ok(result.rows_affected())
    }
}

/// DLQ statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqStats {
    pub total: i64,
    pub retryable: i64,
    pub discarded: i64,
}

// ── Row mapping ───────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct DlqRow {
    id: String,
    task_id: String,
    agent_id: String,
    workflow_id: Option<String>,
    task_name: String,
    task_type: String,
    input: Option<String>,
    last_error: String,
    retry_count: i32,
    max_retries: i32,
    failed_at: String,
    original_created_at: Option<String>,
    dead_letter_reason: String,
    can_retry: i32,
    metadata: String,
}

impl DlqRow {
    fn into_entry(self) -> DeadLetterEntry {
        let failed_at = chrono::DateTime::parse_from_rfc3339(&self.failed_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        DeadLetterEntry {
            id: self.id,
            task_id: self.task_id,
            agent_id: self.agent_id,
            workflow_id: self.workflow_id,
            task_name: self.task_name,
            task_type: self.task_type,
            input: self.input,
            last_error: self.last_error,
            retry_count: self.retry_count,
            max_retries: self.max_retries,
            failed_at,
            original_created_at: self.original_created_at,
            reason: reason_from_str(&self.dead_letter_reason),
            can_retry: self.can_retry != 0,
            metadata: self.metadata,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE dead_letter_queue (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                workflow_id TEXT,
                task_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                input TEXT,
                last_error TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 3,
                failed_at TEXT NOT NULL DEFAULT (datetime('now')),
                original_created_at TEXT,
                dead_letter_reason TEXT NOT NULL DEFAULT 'max_retries_exceeded',
                can_retry INTEGER NOT NULL DEFAULT 1,
                metadata TEXT NOT NULL DEFAULT '{}'
            )",
        )
        .execute(&pool)
        .await
        .expect("create dlq table");
        pool
    }

    #[tokio::test]
    async fn test_enqueue_and_list() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "task-1",
                "agent-1",
                Some("wf-1"),
                "process_data",
                "compute",
                Some("{\"input\": 42}"),
                "connection timeout",
                3,
                3,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();

        assert!(!id.is_empty());

        let entries = dlq.list(None, false, 100).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].task_id, "task-1");
        assert_eq!(entries[0].agent_id, "agent-1");
        assert_eq!(entries[0].reason, DeadLetterReason::MaxRetriesExceeded);
        assert!(entries[0].can_retry);
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "t1",
                "a1",
                None,
                "task",
                "type",
                None,
                "err",
                1,
                3,
                DeadLetterReason::Timeout,
            )
            .await
            .unwrap();

        let entry = dlq.get(&id).await.unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().reason, DeadLetterReason::Timeout);
    }

    #[tokio::test]
    async fn test_get_by_task_id() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        dlq.enqueue(
            "task-42",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Unknown,
        )
        .await
        .unwrap();

        let entry = dlq.get_by_task("task-42").await.unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().task_id, "task-42");
    }

    #[tokio::test]
    async fn test_retry_marks_non_retryable() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "t1",
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                3,
                3,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();

        // First retry — should succeed
        let entry = dlq.retry(&id).await.unwrap();
        assert!(entry.is_some());

        // Now can_retry should be false
        let entry = dlq.get(&id).await.unwrap().unwrap();
        assert!(!entry.can_retry);

        // Second retry — should return None (not retryable)
        let result = dlq.retry(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_discard() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "t1",
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                1,
                3,
                DeadLetterReason::Cancelled,
            )
            .await
            .unwrap();

        assert!(dlq.discard(&id).await.unwrap());
        assert!(dlq.get(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_stats() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        dlq.enqueue(
            "t1",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            3,
            3,
            DeadLetterReason::MaxRetriesExceeded,
        )
        .await
        .unwrap();
        dlq.enqueue(
            "t2",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            3,
            3,
            DeadLetterReason::Timeout,
        )
        .await
        .unwrap();

        let stats = dlq.stats().await.unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.retryable, 2);
        assert_eq!(stats.discarded, 0);
    }

    #[tokio::test]
    async fn test_list_by_agent() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        dlq.enqueue(
            "t1",
            "agent-A",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Unknown,
        )
        .await
        .unwrap();
        dlq.enqueue(
            "t2",
            "agent-B",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Unknown,
        )
        .await
        .unwrap();
        dlq.enqueue(
            "t3",
            "agent-A",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Unknown,
        )
        .await
        .unwrap();

        let a_entries = dlq.list(Some("agent-A"), false, 100).await.unwrap();
        assert_eq!(a_entries.len(), 2);

        let b_entries = dlq.list(Some("agent-B"), false, 100).await.unwrap();
        assert_eq!(b_entries.len(), 1);
    }

    #[tokio::test]
    async fn test_list_can_retry_only() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id1 = dlq
            .enqueue(
                "t1",
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                3,
                3,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();
        dlq.enqueue(
            "t2",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Timeout,
        )
        .await
        .unwrap();

        // Mark first as non-retryable
        dlq.retry(&id1).await.unwrap();

        // List retryable only
        let retryable = dlq.list(None, true, 100).await.unwrap();
        assert_eq!(retryable.len(), 1);
        assert_eq!(retryable[0].task_id, "t2");

        // List all
        let all = dlq.list(None, false, 100).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_with_limit() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        for i in 0..5 {
            dlq.enqueue(
                &format!("t{i}"),
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                1,
                3,
                DeadLetterReason::Unknown,
            )
            .await
            .unwrap();
        }

        let limited = dlq.list(None, false, 3).await.unwrap();
        assert_eq!(limited.len(), 3);

        let all = dlq.list(None, false, 100).await.unwrap();
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn test_discard_nonexistent() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let removed = dlq.discard("nonexistent-id").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let entry = dlq.get("nonexistent-id").await.unwrap();
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_get_by_task_nonexistent() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let entry = dlq.get_by_task("nonexistent-task").await.unwrap();
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_stats_after_discard() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id1 = dlq
            .enqueue(
                "t1",
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                3,
                3,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();
        dlq.enqueue(
            "t2",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Timeout,
        )
        .await
        .unwrap();

        dlq.discard(&id1).await.unwrap();

        let stats = dlq.stats().await.unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.retryable, 1);
        assert_eq!(stats.discarded, 0);
    }

    #[tokio::test]
    async fn test_all_reasons() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let reasons = [DeadLetterReason::MaxRetriesExceeded,
            DeadLetterReason::Timeout,
            DeadLetterReason::Cancelled,
            DeadLetterReason::DependencyMissing,
            DeadLetterReason::Unknown];

        for (i, reason) in reasons.iter().enumerate() {
            dlq.enqueue(
                &format!("t{i}"),
                "a1",
                None,
                "n",
                "t",
                None,
                "e",
                1,
                3,
                reason.clone(),
            )
            .await
            .unwrap();
        }

        let entries = dlq.list(None, false, 100).await.unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[tokio::test]
    async fn test_reason_display_and_parse() {
        assert_eq!(
            DeadLetterReason::MaxRetriesExceeded.to_string(),
            "max_retries_exceeded"
        );
        assert_eq!(DeadLetterReason::Timeout.to_string(), "timeout");
        assert_eq!(DeadLetterReason::Cancelled.to_string(), "cancelled");
        assert_eq!(
            DeadLetterReason::DependencyMissing.to_string(),
            "dependency_missing"
        );
        assert_eq!(DeadLetterReason::Unknown.to_string(), "unknown");

        assert!(matches!(
            reason_from_str("max_retries_exceeded"),
            DeadLetterReason::MaxRetriesExceeded
        ));
        assert!(matches!(
            reason_from_str("timeout"),
            DeadLetterReason::Timeout
        ));
        assert!(matches!(
            reason_from_str("cancelled"),
            DeadLetterReason::Cancelled
        ));
        assert!(matches!(
            reason_from_str("dependency_missing"),
            DeadLetterReason::DependencyMissing
        ));
        assert!(matches!(
            reason_from_str("unknown"),
            DeadLetterReason::Unknown
        ));
        assert!(matches!(
            reason_from_str("invalid"),
            DeadLetterReason::Unknown
        ));
    }

    #[tokio::test]
    async fn test_enqueue_with_workflow_id() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "t1",
                "a1",
                Some("wf-123"),
                "process",
                "compute",
                Some("{\"key\": \"value\"}"),
                "timeout error",
                5,
                5,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();

        let entry = dlq.get(&id).await.unwrap().unwrap();
        assert_eq!(entry.workflow_id, Some("wf-123".to_string()));
        assert_eq!(entry.input, Some("{\"key\": \"value\"}".to_string()));
        assert_eq!(entry.retry_count, 5);
        assert_eq!(entry.max_retries, 5);
        assert_eq!(entry.task_name, "process");
        assert_eq!(entry.task_type, "compute");
        assert_eq!(entry.last_error, "timeout error");
    }

    #[tokio::test]
    async fn test_purge_older_than() {
        let pool = setup_db().await;
        let pool_clone = pool.clone();
        let dlq = DeadLetterQueue::new(pool);

        // Insert an entry with a very old failed_at date
        sqlx::query(
            "INSERT INTO dead_letter_queue (id, task_id, agent_id, workflow_id, task_name, task_type, input, last_error, retry_count, max_retries, failed_at, original_created_at, dead_letter_reason, can_retry, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("old-entry")
        .bind("t-old")
        .bind("a1")
        .bind(None::<&str>)
        .bind("n")
        .bind("t")
        .bind(None::<&str>)
        .bind("e")
        .bind(1i32)
        .bind(3i32)
        .bind("2020-01-01T00:00:00Z")
        .bind("2020-01-01T00:00:00Z")
        .bind("unknown")
        .bind(1i32)
        .bind("{}")
        .execute(&pool_clone)
        .await
        .unwrap();

        // Insert a recent entry
        dlq.enqueue(
            "t-new",
            "a1",
            None,
            "n",
            "t",
            None,
            "e",
            1,
            3,
            DeadLetterReason::Unknown,
        )
        .await
        .unwrap();

        // Purge entries older than 30 days
        let purged = dlq.purge_older_than(30).await.unwrap();
        assert_eq!(purged, 1);

        // Only recent entry should remain
        let remaining = dlq.list(None, false, 100).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].task_id, "t-new");
    }

    #[tokio::test]
    async fn test_entry_fields_complete() {
        let pool = setup_db().await;
        let dlq = DeadLetterQueue::new(pool);

        let id = dlq
            .enqueue(
                "task-abc",
                "agent-xyz",
                Some("wf-99"),
                "my_task",
                "llm_call",
                Some("prompt text"),
                "rate limited",
                3,
                5,
                DeadLetterReason::MaxRetriesExceeded,
            )
            .await
            .unwrap();

        let entry = dlq.get(&id).await.unwrap().unwrap();
        assert_eq!(entry.task_id, "task-abc");
        assert_eq!(entry.agent_id, "agent-xyz");
        assert_eq!(entry.workflow_id, Some("wf-99".to_string()));
        assert_eq!(entry.task_name, "my_task");
        assert_eq!(entry.task_type, "llm_call");
        assert_eq!(entry.input, Some("prompt text".to_string()));
        assert_eq!(entry.last_error, "rate limited");
        assert_eq!(entry.retry_count, 3);
        assert_eq!(entry.max_retries, 5);
        assert!(entry.can_retry);
        assert_eq!(entry.reason, DeadLetterReason::MaxRetriesExceeded);
        assert!(!entry.id.is_empty());
    }
}
