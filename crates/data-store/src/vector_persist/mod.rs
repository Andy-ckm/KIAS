//! # Persistent Vector Store
//!
//! Bridges the in-memory HNSW vector index (from `kias-knowledge`) with SQLite
//! persistence. Vectors are stored as binary blobs in SQLite and loaded into
//! memory on startup for fast similarity search.
//!
//! ## Design
//!
//! - **Write-through**: Every insert/update writes to both SQLite and in-memory index
//! - **Read-through**: On startup, loads all vectors from SQLite into memory
//! - **Crash recovery**: SQLite provides durability; in-memory index provides speed

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{debug, info};

/// A single vector entry stored in the persistent vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Unique ID for this entry.
    pub id: String,
    /// The name/index this vector belongs to.
    pub index_name: String,
    /// External identifier (e.g., knowledge node ID).
    pub external_id: String,
    /// The embedding vector (f32 values serialized as bytes).
    pub embedding: Vec<f32>,
    /// Optional metadata.
    pub metadata: serde_json::Value,
}

/// Persistent vector store backed by SQLite.
pub struct PersistentVectorStore {
    pool: SqlitePool,
    /// In-memory index: index_name -> Vec<(external_id, embedding, metadata)>
    indices: dashmap::DashMap<String, Vec<IndexEntry>>,
}

#[derive(Debug, Clone)]
struct IndexEntry {
    external_id: String,
    embedding: Vec<f32>,
    metadata: serde_json::Value,
}

impl PersistentVectorStore {
    /// Create a new persistent vector store.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            indices: dashmap::DashMap::new(),
        }
    }

    /// Initialize a vector index if it doesn't exist.
    pub async fn create_index(
        &self,
        name: &str,
        dimension: usize,
        metric: &str,
    ) -> KiasResult<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO vector_indices (id, name, dimension, metric) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(dimension as i64)
        .bind(metric)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create vector index: {e}")))?;

        // Ensure in-memory map has an entry
        self.indices.entry(name.to_string()).or_default();

        info!("Created vector index: {name} (dim={dimension}, metric={metric})");
        Ok(())
    }

    /// Load all vectors from SQLite into memory.
    pub async fn load_from_db(&self) -> KiasResult<usize> {
        let indices: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM vector_indices")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to load vector indices: {e}")))?;

        let mut total_loaded = 0;

        for (index_id, index_name) in indices {
            let rows: Vec<(String, String, Vec<u8>, String)> = sqlx::query_as(
                "SELECT id, external_id, embedding, metadata FROM vector_entries WHERE index_id = ?",
            )
            .bind(&index_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to load vector entries: {e}")))?;

            let mut entries = Vec::with_capacity(rows.len());
            for (_entry_id, external_id, embedding_bytes, metadata_str) in rows {
                let embedding = bytes_to_embedding(&embedding_bytes);
                let metadata: serde_json::Value =
                    serde_json::from_str(&metadata_str).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                entries.push(IndexEntry {
                    external_id,
                    embedding,
                    metadata,
                });
            }

            let count = entries.len();
            total_loaded += count;
            self.indices.insert(index_name.clone(), entries);
            debug!("Loaded {count} vectors into index '{index_name}'");
        }

        info!("Loaded {total_loaded} total vectors from database");
        Ok(total_loaded)
    }

    /// Insert a vector into the store (write-through).
    pub async fn insert(
        &self,
        index_name: &str,
        external_id: &str,
        embedding: &[f32],
        metadata: serde_json::Value,
    ) -> KiasResult<String> {
        // Get index ID
        let index_id: (String,) =
            sqlx::query_as("SELECT id FROM vector_indices WHERE name = ?")
                .bind(index_name)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| KiasError::NotFound(format!("Vector index '{index_name}' not found")))?;

        let entry_id = uuid::Uuid::new_v4().to_string();
        let embedding_bytes = embedding_to_bytes(embedding);
        let metadata_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT OR REPLACE INTO vector_entries (id, index_id, external_id, embedding, metadata) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&entry_id)
        .bind(&index_id.0)
        .bind(external_id)
        .bind(&embedding_bytes)
        .bind(&metadata_str)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to insert vector: {e}")))?;

        // Update in-memory index
        let entry = IndexEntry {
            external_id: external_id.to_string(),
            embedding: embedding.to_vec(),
            metadata,
        };
        self.indices
            .entry(index_name.to_string())
            .or_default()
            .push(entry);

        // Update entry count
        sqlx::query(
            "UPDATE vector_indices SET entry_count = (SELECT COUNT(*) FROM vector_entries WHERE index_id = ?), updated_at = datetime('now') WHERE id = ?"
        )
        .bind(&index_id.0)
        .bind(&index_id.0)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update vector count: {e}")))?;

        debug!("Inserted vector for '{external_id}' into '{index_name}'");
        Ok(entry_id)
    }

    /// Search for the K nearest vectors using cosine similarity.
    pub fn search(
        &self,
        index_name: &str,
        query: &[f32],
        top_k: usize,
    ) -> KiasResult<Vec<VectorSearchResult>> {
        let entries = self.indices.get(index_name).ok_or_else(|| {
            KiasError::NotFound(format!("Vector index '{index_name}' not found"))
        })?;

        let mut results: Vec<VectorSearchResult> = entries
            .iter()
            .map(|entry| {
                let similarity = cosine_similarity(query, &entry.embedding);
                VectorSearchResult {
                    external_id: entry.external_id.clone(),
                    similarity,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        // Sort by similarity (highest first)
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);

        Ok(results)
    }

    /// Remove a vector by external ID.
    pub async fn remove(&self, index_name: &str, external_id: &str) -> KiasResult<bool> {
        let result = sqlx::query(
            "DELETE FROM vector_entries WHERE external_id = ? AND index_id = (SELECT id FROM vector_indices WHERE name = ?)"
        )
        .bind(external_id)
        .bind(index_name)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to remove vector: {e}")))?;

        // Remove from in-memory index
        if let Some(mut entries) = self.indices.get_mut(index_name) {
            entries.retain(|e| e.external_id != external_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Get the number of vectors in an index.
    pub fn count(&self, index_name: &str) -> usize {
        self.indices
            .get(index_name)
            .map(|e| e.len())
            .unwrap_or(0)
    }

    /// List all index names.
    pub fn list_indices(&self) -> Vec<String> {
        self.indices.iter().map(|entry| entry.key().clone()).collect()
    }
}

/// Result from a vector similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub external_id: String,
    pub similarity: f64,
    pub metadata: serde_json::Value,
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Serialize f32 slice to bytes (little-endian).
fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|v| v.to_le_bytes().to_vec())
        .collect()
}

/// Deserialize bytes to f32 vec (little-endian).
fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::MigrationRunner;

    async fn setup_store() -> PersistentVectorStore {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        MigrationRunner::new(pool.clone())
            .run_all()
            .await
            .expect("Failed to run migrations");
        PersistentVectorStore::new(pool)
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 1e-6);

        let d = vec![1.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &d);
        assert!((sim - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_serialization() {
        let original = vec![1.0f32, -0.5, std::f32::consts::PI, 0.0];
        let bytes = embedding_to_bytes(&original);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[tokio::test]
    async fn test_create_index() {
        let store = setup_store().await;
        store.create_index("test-idx", 128, "cosine").await.expect("Failed to create index");

        let indices = store.list_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], "test-idx");
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let store = setup_store().await;
        store.create_index("test", 3, "cosine").await.expect("Failed to create index");

        // Insert some vectors
        store.insert("test", "doc1", &[1.0, 0.0, 0.0], serde_json::json!({"type": "a"})).await.unwrap();
        store.insert("test", "doc2", &[0.0, 1.0, 0.0], serde_json::json!({"type": "b"})).await.unwrap();
        store.insert("test", "doc3", &[0.7, 0.7, 0.0], serde_json::json!({"type": "c"})).await.unwrap();

        assert_eq!(store.count("test"), 3);

        // Search for something close to [1, 0, 0]
        let results = store.search("test", &[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].external_id, "doc1");
        assert!(results[0].similarity > 0.99);
    }

    #[tokio::test]
    async fn test_persistence_reload() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        MigrationRunner::new(pool.clone())
            .run_all()
            .await
            .expect("Failed to run migrations");

        // Write some vectors
        {
            let store = PersistentVectorStore::new(pool.clone());
            store.create_index("persist", 3, "cosine").await.unwrap();
            store.insert("persist", "v1", &[1.0, 2.0, 3.0], serde_json::json!({})).await.unwrap();
            store.insert("persist", "v2", &[4.0, 5.0, 6.0], serde_json::json!({})).await.unwrap();
        }

        // Reload in a new store instance
        {
            let store = PersistentVectorStore::new(pool);
            let loaded = store.load_from_db().await.unwrap();
            assert_eq!(loaded, 2);
            assert_eq!(store.count("persist"), 2);

            let results = store.search("persist", &[1.0, 2.0, 3.0], 1).unwrap();
            assert_eq!(results[0].external_id, "v1");
        }
    }

    #[tokio::test]
    async fn test_remove() {
        let store = setup_store().await;
        store.create_index("rm-test", 3, "cosine").await.unwrap();
        store.insert("rm-test", "a", &[1.0, 0.0, 0.0], serde_json::json!({})).await.unwrap();
        store.insert("rm-test", "b", &[0.0, 1.0, 0.0], serde_json::json!({})).await.unwrap();

        assert_eq!(store.count("rm-test"), 2);

        let removed = store.remove("rm-test", "a").await.unwrap();
        assert!(removed);
        assert_eq!(store.count("rm-test"), 1);

        let removed = store.remove("rm-test", "nonexistent").await.unwrap();
        assert!(!removed);
    }
}
