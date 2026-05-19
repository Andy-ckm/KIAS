//! 电子签名服务

use crate::error::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct SignatureService {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Signature {
    pub id: String,
    pub document_id: String,
    pub signed_by: String,
    pub signature_data: String,
    pub signed_at: chrono::DateTime<Utc>,
    pub ip_address: Option<String>,
    pub meaning: String,
}

impl SignatureService {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS signatures (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                signed_by TEXT NOT NULL,
                signature_data TEXT NOT NULL,
                signed_at TEXT NOT NULL,
                ip_address TEXT,
                meaning TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_signatures_document ON signatures(document_id);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn sign(&self, doc_id: &str, signed_by: &str, signature_data: &str) -> Result<Signature> {
        let conn = self.conn.lock().unwrap();

        let signature = Signature {
            id: Uuid::new_v4().to_string(),
            document_id: doc_id.to_string(),
            signed_by: signed_by.to_string(),
            signature_data: signature_data.to_string(),
            signed_at: Utc::now(),
            ip_address: None,
            meaning: "审批签名".to_string(),
        };

        conn.execute(
            "INSERT INTO signatures (id, document_id, signed_by, signature_data, signed_at, ip_address, meaning)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                signature.id,
                signature.document_id,
                signature.signed_by,
                signature.signature_data,
                signature.signed_at.to_rfc3339(),
                signature.ip_address,
                signature.meaning,
            ],
        )?;

        Ok(signature)
    }

    pub fn verify(&self, doc_id: &str, signed_by: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM signatures WHERE document_id = ?1 AND signed_by = ?2",
            params![doc_id, signed_by],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    pub fn get_signatures(&self, doc_id: &str) -> Result<Vec<Signature>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, document_id, signed_by, signature_data, signed_at, ip_address, meaning
             FROM signatures WHERE document_id = ?1 ORDER BY signed_at DESC",
        )?;

        let signatures = stmt
            .query_map(params![doc_id], |row| {
                Ok(Signature {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    signed_by: row.get(2)?,
                    signature_data: row.get(3)?,
                    signed_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    ip_address: row.get(5)?,
                    meaning: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(signatures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sign_document() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let service = SignatureService::new(&db_path).unwrap();

        let sig = service.sign("doc1", "approver1", "signature_data").unwrap();
        assert_eq!(sig.document_id, "doc1");
        assert_eq!(sig.signed_by, "approver1");
    }

    #[test]
    fn test_verify_signature() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let service = SignatureService::new(&db_path).unwrap();

        service.sign("doc1", "approver1", "sig_data").unwrap();

        assert!(service.verify("doc1", "approver1").unwrap());
        assert!(!service.verify("doc1", "approver2").unwrap());
    }
}
