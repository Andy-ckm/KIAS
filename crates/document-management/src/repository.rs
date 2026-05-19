//! 文档仓库 - SQLite 存储

use crate::document::*;
use crate::error::{DocumentError, Result};
use crate::DocumentStatistics;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct DocumentRepository {
    conn: Mutex<Connection>,
    pub(crate) db_path: std::path::PathBuf,
}

impl DocumentRepository {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                doc_type TEXT NOT NULL,
                category TEXT NOT NULL,
                status TEXT NOT NULL,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                updated_by TEXT,
                tags TEXT,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);
            CREATE INDEX IF NOT EXISTS idx_documents_category ON documents(category);
            CREATE INDEX IF NOT EXISTS idx_documents_created_at ON documents(created_at);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            db_path: db_path.to_path_buf(),
        })
    }

    fn row_to_document(row: &rusqlite::Row) -> rusqlite::Result<Document> {
        let tags_str: String = row.get(11)?;
        let metadata_str: String = row.get(12)?;

        Ok(Document {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            doc_type: serde_json::from_str(&row.get::<_, String>(3)?)
                .unwrap_or(DocumentType::Other),
            category: row.get(4)?,
            status: serde_json::from_str(&row.get::<_, String>(5)?)
                .unwrap_or(DocumentStatus::Draft),
            version: row.get(6)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            created_by: row.get(8)?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_by: row.get(10)?,
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
        })
    }

    fn get_with_conn(conn: &Connection, id: &str) -> Result<Document> {
        let mut stmt = conn.prepare(
            "SELECT id, title, content, doc_type, category, status, version, created_at, created_by, updated_at, updated_by, tags, metadata
             FROM documents WHERE id = ?1",
        )?;

        stmt.query_row(params![id], Self::row_to_document)
            .map_err(|_| DocumentError::NotFound(id.to_string()))
    }

    pub fn get(&self, id: &str) -> Result<Document> {
        let conn = self.conn.lock().unwrap();
        Self::get_with_conn(&conn, id)
    }

    pub fn create(&self, request: CreateDocumentRequest) -> Result<Document> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let tags_json = serde_json::to_string(&request.tags).unwrap_or_default();
        let metadata_json = serde_json::to_string(&DocumentMetadata::default()).unwrap_or_default();

        conn.execute(
            "INSERT INTO documents (id, title, content, doc_type, category, status, version, created_at, created_by, updated_at, tags, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                request.title,
                request.content,
                serde_json::to_string(&request.doc_type)?,
                request.category,
                serde_json::to_string(&DocumentStatus::Draft)?,
                1,
                now.to_rfc3339(),
                request.created_by,
                now.to_rfc3339(),
                tags_json,
                metadata_json,
            ],
        )?;

        Ok(Document {
            id,
            title: request.title,
            content: request.content,
            doc_type: request.doc_type,
            category: request.category,
            status: DocumentStatus::Draft,
            version: 1,
            created_at: now,
            created_by: request.created_by.clone(),
            updated_at: now,
            updated_by: None,
            tags: request.tags,
            metadata: DocumentMetadata::default(),
        })
    }

    pub fn update(&self, id: &str, request: UpdateDocumentRequest) -> Result<Document> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();

        let current = Self::get_with_conn(&conn, id)?;

        let new_title = request.title.unwrap_or(current.title);
        let new_content = request.content.unwrap_or(current.content);
        let new_tags = request.tags.unwrap_or(current.tags);
        let tags_json = serde_json::to_string(&new_tags)?;

        conn.execute(
            "UPDATE documents SET title = ?1, content = ?2, tags = ?3, updated_at = ?4, updated_by = ?5, version = version + 1 WHERE id = ?6",
            params![new_title, new_content, tags_json, now.to_rfc3339(), request.updated_by, id],
        )?;

        Self::get_with_conn(&conn, id)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: DocumentStatus,
        updated_by: &str,
    ) -> Result<Document> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();

        conn.execute(
            "UPDATE documents SET status = ?1, updated_at = ?2, updated_by = ?3 WHERE id = ?4",
            params![
                serde_json::to_string(&status)?,
                now.to_rfc3339(),
                updated_by,
                id
            ],
        )?;

        Self::get_with_conn(&conn, id)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id FROM documents WHERE title LIKE ?1 OR content LIKE ?1 OR category LIKE ?1 LIMIT 50",
        )?;

        let ids: Vec<String> = stmt
            .query_map(params![search_pattern], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut documents = Vec::new();
        for id in ids {
            if let Ok(doc) = Self::get_with_conn(&conn, &id) {
                documents.push(doc);
            }
        }

        Ok(documents)
    }

    pub fn get_statistics(&self) -> Result<DocumentStatistics> {
        let conn = self.conn.lock().unwrap();

        let total: usize =
            conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;

        let draft: usize = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = '\"Draft\"'",
            [],
            |row| row.get(0),
        )?;

        let under_review: usize = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = '\"UnderReview\"'",
            [],
            |row| row.get(0),
        )?;

        let approved: usize = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = '\"Approved\"'",
            [],
            |row| row.get(0),
        )?;

        let published: usize = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = '\"Published\"'",
            [],
            |row| row.get(0),
        )?;

        let archived: usize = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = '\"Archived\"'",
            [],
            |row| row.get(0),
        )?;

        Ok(DocumentStatistics {
            total_documents: total,
            draft_count: draft,
            under_review_count: under_review,
            approved_count: approved,
            published_count: published,
            archived_count: archived,
        })
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DocumentError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn list_by_status(&self, status: &DocumentStatus) -> Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let status_json = serde_json::to_string(status)?;
        let mut stmt = conn.prepare(
            "SELECT id FROM documents WHERE status = ?1 ORDER BY updated_at DESC LIMIT 100",
        )?;

        let ids: Vec<String> = stmt
            .query_map(params![status_json], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut documents = Vec::new();
        for id in ids {
            if let Ok(doc) = Self::get_with_conn(&conn, &id) {
                documents.push(doc);
            }
        }

        Ok(documents)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: usize =
            conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::*;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, DocumentRepository) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let repo = DocumentRepository::new(&db_path).unwrap();
        (tmp, repo)
    }

    fn make_request(title: &str) -> CreateDocumentRequest {
        CreateDocumentRequest {
            title: title.to_string(),
            content: format!("Content of {}", title),
            doc_type: DocumentType::Policy,
            category: "General".to_string(),
            created_by: "admin".to_string(),
            tags: vec!["test".to_string()],
        }
    }

    #[test]
    fn test_create_and_get() {
        let (_tmp, repo) = setup_repo();
        let doc = repo.create(make_request("Doc1")).unwrap();
        assert_eq!(doc.title, "Doc1");
        assert_eq!(doc.status, DocumentStatus::Draft);
        assert_eq!(doc.version, 1);

        let fetched = repo.get(&doc.id).unwrap();
        assert_eq!(fetched.id, doc.id);
        assert_eq!(fetched.title, "Doc1");
    }

    #[test]
    fn test_get_not_found() {
        let (_tmp, repo) = setup_repo();
        let result = repo.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_document() {
        let (_tmp, repo) = setup_repo();
        let doc = repo.create(make_request("Original")).unwrap();

        let updated = repo
            .update(
                &doc.id,
                UpdateDocumentRequest {
                    title: Some("Updated Title".to_string()),
                    content: Some("New content".to_string()),
                    tags: None,
                    updated_by: "editor".to_string(),
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.content, "New content");
        assert_eq!(updated.version, 2);
        assert_eq!(updated.tags, vec!["test".to_string()]);
    }

    #[test]
    fn test_update_status() {
        let (_tmp, repo) = setup_repo();
        let doc = repo.create(make_request("StatusDoc")).unwrap();
        assert_eq!(doc.status, DocumentStatus::Draft);

        let reviewed = repo
            .update_status(&doc.id, DocumentStatus::UnderReview, "reviewer")
            .unwrap();
        assert_eq!(reviewed.status, DocumentStatus::UnderReview);

        let approved = repo
            .update_status(&doc.id, DocumentStatus::Approved, "approver")
            .unwrap();
        assert_eq!(approved.status, DocumentStatus::Approved);
    }

    #[test]
    fn test_search_by_title() {
        let (_tmp, repo) = setup_repo();
        repo.create(make_request("Alpha Policy")).unwrap();
        repo.create(make_request("Beta Procedure")).unwrap();
        repo.create(make_request("Alpha Standard")).unwrap();

        let results = repo.search("Alpha").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_content() {
        let (_tmp, repo) = setup_repo();
        repo.create(make_request("Doc A")).unwrap();
        repo.create(make_request("Doc B")).unwrap();

        let results = repo.search("Content of Doc A").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_no_results() {
        let (_tmp, repo) = setup_repo();
        repo.create(make_request("Doc")).unwrap();

        let results = repo.search("nonexistent_query_xyz").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_statistics_empty() {
        let (_tmp, repo) = setup_repo();
        let stats = repo.get_statistics().unwrap();
        assert_eq!(stats.total_documents, 0);
        assert_eq!(stats.draft_count, 0);
    }

    #[test]
    fn test_get_statistics_with_documents() {
        let (_tmp, repo) = setup_repo();
        let doc1 = repo.create(make_request("Doc1")).unwrap();
        let doc2 = repo.create(make_request("Doc2")).unwrap();
        repo.create(make_request("Doc3")).unwrap();

        repo.update_status(&doc1.id, DocumentStatus::UnderReview, "user")
            .unwrap();
        repo.update_status(&doc2.id, DocumentStatus::Approved, "user")
            .unwrap();

        let stats = repo.get_statistics().unwrap();
        assert_eq!(stats.total_documents, 3);
        assert_eq!(stats.draft_count, 1);
        assert_eq!(stats.under_review_count, 1);
        assert_eq!(stats.approved_count, 1);
    }

    #[test]
    fn test_delete_document() {
        let (_tmp, repo) = setup_repo();
        let doc = repo.create(make_request("ToDelete")).unwrap();
        assert_eq!(repo.count().unwrap(), 1);

        repo.delete(&doc.id).unwrap();
        assert_eq!(repo.count().unwrap(), 0);
        assert!(repo.get(&doc.id).is_err());
    }

    #[test]
    fn test_delete_not_found() {
        let (_tmp, repo) = setup_repo();
        let result = repo.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_by_status() {
        let (_tmp, repo) = setup_repo();
        repo.create(make_request("Draft1")).unwrap();
        repo.create(make_request("Draft2")).unwrap();
        let doc3 = repo.create(make_request("Review1")).unwrap();
        repo.update_status(&doc3.id, DocumentStatus::UnderReview, "user")
            .unwrap();

        let drafts = repo.list_by_status(&DocumentStatus::Draft).unwrap();
        assert_eq!(drafts.len(), 2);

        let reviewing = repo.list_by_status(&DocumentStatus::UnderReview).unwrap();
        assert_eq!(reviewing.len(), 1);
    }

    #[test]
    fn test_count() {
        let (_tmp, repo) = setup_repo();
        assert_eq!(repo.count().unwrap(), 0);

        repo.create(make_request("Doc1")).unwrap();
        assert_eq!(repo.count().unwrap(), 1);

        repo.create(make_request("Doc2")).unwrap();
        assert_eq!(repo.count().unwrap(), 2);
    }

    #[test]
    fn test_create_preserves_tags() {
        let (_tmp, repo) = setup_repo();
        let req = CreateDocumentRequest {
            title: "Tagged".to_string(),
            content: "content".to_string(),
            doc_type: DocumentType::Procedure,
            category: "Ops".to_string(),
            created_by: "admin".to_string(),
            tags: vec!["important".to_string(), "review".to_string()],
        };
        let doc = repo.create(req).unwrap();
        assert_eq!(doc.tags.len(), 2);
        assert!(doc.tags.contains(&"important".to_string()));
    }
}
