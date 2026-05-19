//! 审计日志

use crate::error::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub document_id: String,
    pub action: String,
    pub performed_by: String,
    pub performed_at: DateTime<Utc>,
    pub details: Option<String>,
    pub ip_address: Option<String>,
}

pub fn get_audit_log(db_path: &Path, doc_id: &str) -> Result<Vec<AuditEntry>> {
    let conn = Connection::open(db_path)?;

    // 确保表存在
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS document_audit_log (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL,
            action TEXT NOT NULL,
            performed_by TEXT NOT NULL,
            performed_at TEXT NOT NULL,
            details TEXT,
            ip_address TEXT
        );",
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, document_id, action, performed_by, performed_at, details, ip_address
         FROM document_audit_log WHERE document_id = ?1 ORDER BY performed_at DESC LIMIT 100",
    )?;

    let entries = stmt
        .query_map(params![doc_id], |row| {
            Ok(AuditEntry {
                id: row.get(0)?,
                document_id: row.get(1)?,
                action: row.get(2)?,
                performed_by: row.get(3)?,
                performed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                details: row.get(5)?,
                ip_address: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(entries)
}

pub fn record_audit(
    db_path: &Path,
    doc_id: &str,
    action: &str,
    performed_by: &str,
    details: Option<String>,
) -> Result<()> {
    let conn = Connection::open(db_path)?;

    // 确保表存在
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS document_audit_log (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL,
            action TEXT NOT NULL,
            performed_by TEXT NOT NULL,
            performed_at TEXT NOT NULL,
            details TEXT,
            ip_address TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_audit_document ON document_audit_log(document_id);
        ",
    )?;

    conn.execute(
        "INSERT INTO document_audit_log (id, document_id, action, performed_by, performed_at, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            doc_id,
            action,
            performed_by,
            Utc::now().to_rfc3339(),
            details,
        ],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_record_and_get_audit() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(
            &db_path,
            "doc1",
            "created",
            "user1",
            Some("创建文档".to_string()),
        )
        .unwrap();

        let entries = get_audit_log(&db_path, "doc1").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "created");
        assert_eq!(entries[0].performed_by, "user1");
        assert_eq!(entries[0].document_id, "doc1");
    }

    #[test]
    fn test_multiple_audit_entries() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(&db_path, "doc1", "created", "user1", None).unwrap();
        record_audit(&db_path, "doc1", "updated", "user1", None).unwrap();
        record_audit(&db_path, "doc1", "approved", "user2", None).unwrap();

        let entries = get_audit_log(&db_path, "doc1").unwrap();
        assert_eq!(entries.len(), 3);
        // 按时间倒序，最新的在前
        assert_eq!(entries[0].action, "approved");
        assert_eq!(entries[1].action, "updated");
        assert_eq!(entries[2].action, "created");
    }

    #[test]
    fn test_audit_empty_log() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        let entries = get_audit_log(&db_path, "nonexistent").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_audit_different_documents() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(&db_path, "doc1", "created", "user1", None).unwrap();
        record_audit(&db_path, "doc2", "created", "user2", None).unwrap();

        let entries1 = get_audit_log(&db_path, "doc1").unwrap();
        let entries2 = get_audit_log(&db_path, "doc2").unwrap();

        assert_eq!(entries1.len(), 1);
        assert_eq!(entries2.len(), 1);
        assert_eq!(entries1[0].performed_by, "user1");
        assert_eq!(entries2[0].performed_by, "user2");
    }

    #[test]
    fn test_audit_with_details() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(
            &db_path,
            "doc1",
            "approved",
            "approver1",
            Some("审批通过，含电子签名".to_string()),
        )
        .unwrap();

        let entries = get_audit_log(&db_path, "doc1").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].details.as_ref().unwrap(), "审批通过，含电子签名");
    }

    #[test]
    fn test_audit_without_details() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(&db_path, "doc1", "created", "user1", None).unwrap();

        let entries = get_audit_log(&db_path, "doc1").unwrap();
        assert!(entries[0].details.is_none());
    }

    #[test]
    fn test_audit_timestamp_ordering() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");

        record_audit(&db_path, "doc1", "created", "user1", None).unwrap();
        record_audit(&db_path, "doc1", "updated", "user1", None).unwrap();

        let entries = get_audit_log(&db_path, "doc1").unwrap();
        assert_eq!(entries.len(), 2);
        // 最新的在前
        assert!(entries[0].performed_at >= entries[1].performed_at);
    }
}
