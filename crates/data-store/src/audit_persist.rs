//! Persistent audit log backed by SQLite.
//!
//! Provides [`SqliteAuditLog`] which implements the [`AuditLogger`] trait from
//! `kias-common::audit`, persisting every audit event to a SQLite table so that
//! the audit trail survives server restarts and can be queried for compliance.

use async_trait::async_trait;
use kias_common::audit::{AuditAction, AuditEvent, AuditLogger, AuditOutcome};
use kias_common::KiasResult;
use sqlx::SqlitePool;
use tracing::{debug, warn};

/// SQLite-backed audit log with query capabilities.
///
/// Implements [`AuditLogger`] so it can be used as a drop-in replacement for
/// [`MemoryAuditLog`](kias_common::audit::MemoryAuditLog).
#[derive(Debug, Clone)]
pub struct SqliteAuditLog {
    pool: SqlitePool,
}

impl SqliteAuditLog {
    /// Create a new `SqliteAuditLog` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a single audit event into the database.
    async fn insert_event(&self, event: &AuditEvent) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO audit_log (id, timestamp, actor, action, resource_type, resource_id, details, ip_address, user_agent, outcome) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&event.id)
        .bind(event.timestamp.to_rfc3339())
        .bind(&event.actor)
        .bind(action_to_string(event.action))
        .bind(&event.resource_type)
        .bind(&event.resource_id)
        .bind(&event.details)
        .bind(&event.ip_address)
        .bind(&event.user_agent)
        .bind(outcome_to_string(event.outcome))
        .execute(&self.pool)
        .await
        .map_err(|e| kias_common::KiasError::Config(format!("audit insert failed: {e}")))?;

        debug!(
            audit_id = %event.id,
            actor = %event.actor,
            action = %event.action,
            resource = %event.resource_type,
            "audit event persisted"
        );
        Ok(())
    }

    /// Query audit events with optional filters.
    pub async fn query(
        &self,
        actor: Option<&str>,
        action: Option<&str>,
        resource_type: Option<&str>,
        limit: i64,
    ) -> KiasResult<Vec<AuditEvent>> {
        let mut sql = String::from(
            "SELECT id, timestamp, actor, action, resource_type, resource_id, details, ip_address, user_agent, outcome FROM audit_log WHERE 1=1"
        );
        let mut binds: Vec<String> = Vec::new();

        if let Some(a) = actor {
            sql.push_str(" AND actor = ?");
            binds.push(a.to_string());
        }
        if let Some(a) = action {
            sql.push_str(" AND action = ?");
            binds.push(a.to_string());
        }
        if let Some(rt) = resource_type {
            sql.push_str(" AND resource_type = ?");
            binds.push(rt.to_string());
        }
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

        let mut query = sqlx::query_as::<_, AuditEventRow>(&sql);
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(limit);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| kias_common::KiasError::Config(format!("audit query failed: {e}")))?;

        Ok(rows.into_iter().map(AuditEventRow::into_event).collect())
    }

    /// Get the total count of audit events.
    pub async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| kias_common::KiasError::Config(format!("audit count failed: {e}")))?;
        Ok(row.0)
    }

    /// Purge audit events older than the given number of days.
    pub async fn purge_older_than(&self, days: i64) -> KiasResult<u64> {
        let result =
            sqlx::query("DELETE FROM audit_log WHERE timestamp < datetime('now', ? || ' days')")
                .bind(format!("-{days}"))
                .execute(&self.pool)
                .await
                .map_err(|e| kias_common::KiasError::Config(format!("audit purge failed: {e}")))?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl AuditLogger for SqliteAuditLog {
    async fn log_event(&self, event: AuditEvent) {
        if let Err(e) = self.insert_event(&event).await {
            warn!(error = %e, "failed to persist audit event");
        }
    }
}

// ── Row mapping ───────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct AuditEventRow {
    id: String,
    timestamp: String,
    actor: String,
    action: String,
    resource_type: String,
    resource_id: String,
    details: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
    outcome: String,
}

impl AuditEventRow {
    fn into_event(self) -> AuditEvent {
        let timestamp = chrono::DateTime::parse_from_rfc3339(&self.timestamp)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        AuditEvent {
            id: self.id,
            timestamp,
            actor: self.actor,
            action: string_to_action(&self.action),
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            details: self.details,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
            outcome: string_to_outcome(&self.outcome),
        }
    }
}

fn action_to_string(a: AuditAction) -> &'static str {
    match a {
        AuditAction::Create => "Create",
        AuditAction::Read => "Read",
        AuditAction::Update => "Update",
        AuditAction::Delete => "Delete",
        AuditAction::Login => "Login",
        AuditAction::Logout => "Logout",
        AuditAction::Schedule => "Schedule",
        AuditAction::Execute => "Execute",
        AuditAction::ConfigChange => "ConfigChange",
    }
}

fn string_to_action(s: &str) -> AuditAction {
    match s {
        "Create" => AuditAction::Create,
        "Read" => AuditAction::Read,
        "Update" => AuditAction::Update,
        "Delete" => AuditAction::Delete,
        "Login" => AuditAction::Login,
        "Logout" => AuditAction::Logout,
        "Schedule" => AuditAction::Schedule,
        "Execute" => AuditAction::Execute,
        "ConfigChange" => AuditAction::ConfigChange,
        _ => AuditAction::Read, // fallback
    }
}

fn outcome_to_string(o: AuditOutcome) -> &'static str {
    match o {
        AuditOutcome::Success => "Success",
        AuditOutcome::Failure => "Failure",
    }
}

fn string_to_outcome(s: &str) -> AuditOutcome {
    match s {
        "Success" => AuditOutcome::Success,
        "Failure" => AuditOutcome::Failure,
        _ => AuditOutcome::Failure,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::audit::{AuditAction, AuditEvent, AuditLogger, AuditOutcome};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                details TEXT NOT NULL DEFAULT '',
                ip_address TEXT,
                user_agent TEXT,
                outcome TEXT NOT NULL CHECK (outcome IN ('Success', 'Failure'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create audit_log table");
        pool
    }

    fn make_event(actor: &str, action: AuditAction, outcome: AuditOutcome) -> AuditEvent {
        AuditEvent::new(actor, action, "agent", "test-agent-1", outcome)
            .with_details("test event")
            .with_ip("127.0.0.1")
            .with_user_agent("test/1.0")
    }

    #[tokio::test]
    async fn test_insert_and_query() {
        let pool = setup_db().await;
        let log = SqliteAuditLog::new(pool);

        let event = make_event("admin", AuditAction::Create, AuditOutcome::Success);
        log.log_event(event).await;

        let count = log.count().await.unwrap();
        assert_eq!(count, 1);

        let events = log.query(Some("admin"), None, None, 100).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].actor, "admin");
        assert_eq!(events[0].action, AuditAction::Create);
        assert_eq!(events[0].outcome, AuditOutcome::Success);
        assert_eq!(events[0].ip_address.as_deref(), Some("127.0.0.1"));
        assert_eq!(events[0].user_agent.as_deref(), Some("test/1.0"));
    }

    #[tokio::test]
    async fn test_query_by_action() {
        let pool = setup_db().await;
        let log = SqliteAuditLog::new(pool);

        log.log_event(make_event("u1", AuditAction::Create, AuditOutcome::Success))
            .await;
        log.log_event(make_event("u1", AuditAction::Delete, AuditOutcome::Failure))
            .await;
        log.log_event(make_event("u2", AuditAction::Create, AuditOutcome::Success))
            .await;

        let creates = log.query(None, Some("Create"), None, 100).await.unwrap();
        assert_eq!(creates.len(), 2);

        let deletes = log.query(None, Some("Delete"), None, 100).await.unwrap();
        assert_eq!(deletes.len(), 1);
    }

    #[tokio::test]
    async fn test_query_by_resource_type() {
        let pool = setup_db().await;
        let log = SqliteAuditLog::new(pool);

        log.log_event(make_event(
            "admin",
            AuditAction::Create,
            AuditOutcome::Success,
        ))
        .await;

        let agents = log.query(None, None, Some("agent"), 100).await.unwrap();
        assert_eq!(agents.len(), 1);

        let nodes = log.query(None, None, Some("node"), 100).await.unwrap();
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_purge_old_events() {
        let pool = setup_db().await;
        let log = SqliteAuditLog::new(pool);

        // Insert event with old timestamp
        let mut event = make_event("admin", AuditAction::Create, AuditOutcome::Success);
        event.timestamp = chrono::Utc::now() - chrono::Duration::days(10);
        log.log_event(event).await;
        assert_eq!(log.count().await.unwrap(), 1);

        // Purge events older than 5 days (should delete the 10-day-old event)
        let purged = log.purge_older_than(5).await.unwrap();
        assert_eq!(purged, 1);
        assert_eq!(log.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_multiple_events_ordering() {
        let pool = setup_db().await;
        let log = SqliteAuditLog::new(pool);

        for i in 0..5 {
            let mut event = make_event(
                &format!("user-{i}"),
                AuditAction::Execute,
                AuditOutcome::Success,
            );
            event.details = format!("event-{i}");
            log.log_event(event).await;
        }

        let all = log.query(None, None, None, 10).await.unwrap();
        assert_eq!(all.len(), 5);
        // DESC order — most recent first
        assert!(all[0].timestamp >= all[4].timestamp);
    }
}
