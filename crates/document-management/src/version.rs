//! 版本控制

use crate::document::DocumentVersion;
use crate::error::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct VersionControl {
    conn: Mutex<Connection>,
}

impl VersionControl {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS document_versions (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                content TEXT NOT NULL,
                checksum TEXT NOT NULL,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                comment TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_versions_document ON document_versions(document_id);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_version(
        &self,
        doc_id: &str,
        content: &str,
        comment: &str,
    ) -> Result<DocumentVersion> {
        self.create_version_with_author(doc_id, content, comment, "system")
    }

    /// 创建版本，指定作者
    pub fn create_version_with_author(
        &self,
        doc_id: &str,
        content: &str,
        comment: &str,
        created_by: &str,
    ) -> Result<DocumentVersion> {
        let conn = self.conn.lock().unwrap();

        // 获取当前最大版本号
        let max_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM document_versions WHERE document_id = ?1",
                params![doc_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let new_version = max_version + 1;
        let checksum = self.calculate_checksum(content);

        let version = DocumentVersion {
            id: Uuid::new_v4().to_string(),
            document_id: doc_id.to_string(),
            version: new_version,
            content: content.to_string(),
            checksum,
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            comment: comment.to_string(),
        };

        conn.execute(
            "INSERT INTO document_versions (id, document_id, version, content, checksum, created_at, created_by, comment)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                version.id,
                version.document_id,
                version.version,
                version.content,
                version.checksum,
                version.created_at.to_rfc3339(),
                version.created_by,
                version.comment,
            ],
        )?;

        Ok(version)
    }

    pub fn get_history(&self, doc_id: &str) -> Result<Vec<DocumentVersion>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, document_id, version, content, checksum, created_at, created_by, comment
             FROM document_versions WHERE document_id = ?1 ORDER BY version DESC",
        )?;

        let versions = stmt
            .query_map(params![doc_id], |row| {
                Ok(DocumentVersion {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    version: row.get(2)?,
                    content: row.get(3)?,
                    checksum: row.get(4)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    created_by: row.get(6)?,
                    comment: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(versions)
    }

    fn calculate_checksum(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_version() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let version = vc.create_version("doc1", "content v1", "初始版本").unwrap();
        assert_eq!(version.version, 1);
        assert!(!version.checksum.is_empty());
    }

    #[test]
    fn test_version_history() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        vc.create_version("doc1", "content v1", "版本1").unwrap();
        vc.create_version("doc1", "content v2", "版本2").unwrap();

        let history = vc.get_history("doc1").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, 2); // 最新版本在前
    }

    #[test]
    fn test_version_numbering() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let v1 = vc.create_version("doc1", "v1", "版本1").unwrap();
        let v2 = vc.create_version("doc1", "v2", "版本2").unwrap();
        let v3 = vc.create_version("doc1", "v3", "版本3").unwrap();

        assert_eq!(v1.version, 1);
        assert_eq!(v2.version, 2);
        assert_eq!(v3.version, 3);
    }

    #[test]
    fn test_empty_history() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let history = vc.get_history("nonexistent").unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_version_checksum_changes() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let v1 = vc.create_version("doc1", "content A", "v1").unwrap();
        let v2 = vc.create_version("doc1", "content B", "v2").unwrap();

        assert_ne!(v1.checksum, v2.checksum);
    }

    #[test]
    fn test_create_version_with_author() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let version = vc.create_version_with_author("doc1", "content", "测试", "test-user").unwrap();
        assert_eq!(version.created_by, "test-user");
        assert_eq!(version.version, 1);
    }

    #[test]
    fn test_default_author_is_system() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let version = vc.create_version("doc1", "content", "默认作者").unwrap();
        assert_eq!(version.created_by, "system");
    }

    #[test]
    fn test_version_content_preserved() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let vc = VersionControl::new(&db_path).unwrap();

        let content = "这是版本内容，包含中文";
        let version = vc.create_version("doc1", content, "保存内容").unwrap();
        assert_eq!(version.content, content);
    }
}
