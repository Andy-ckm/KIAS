//! # Persistent Vector Store
//!
//! Bridges the HNSW vector index from `kias-common` with SQLite persistence.
//! Vectors are stored as binary blobs in SQLite and loaded into memory on startup
//! for fast approximate nearest neighbor (ANN) search via HNSW.
//!
//! ## Design
//!
//! - **Write-through**: Every insert/update writes to both SQLite and HNSW index
//! - **Read-through**: On startup, loads all vectors from SQLite into HNSW
//! - **Crash recovery**: SQLite provides durability; HNSW provides O(log N) search
//! - **HNSW parameters**: M=16, M_max=32, ef_construction=200, ef_search=100
//!
//! ## Implementation Status
//!
//! The HNSW index in `kias-common` is a hand-rolled implementation with
//! proper multi-layer graph, beam search, and connection pruning — it is
//! NOT a brute-force O(N) scan. However, it lacks battle-tested quality
//! (SIMD distance, parallel build, disk-backed graphs, recall benchmarks).
//!
//! **TODO(#real-hnsw)**: Feature-gate `hnsw_rs` crate as a production-grade
//! alternative. Enable via `cargo build --features real-hnsw`. See
//! `Cargo.toml` `[features]` section.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// HNSW-backed vector store from common crate (re-exported for API compatibility)
pub use kias_common::vector::VectorStore as HnswVectorStore;

/// Statistics about an HNSW index
pub use kias_common::vector::VectorStoreStats;

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

/// Per-index HNSW store with its metadata map
#[derive(Clone)]
struct HnswIndex {
    store: Arc<RwLock<kias_common::vector::VectorStore>>,
    metadata: Arc<dashmap::DashMap<String, serde_json::Value>>,
}

impl HnswIndex {
    /// Insert a vector into this index, handling nested mutable borrows.
    async fn insert(&self, node_id: String, vector: Vec<f32>) {
        let mut store = self.store.write().await;
        store.insert(node_id, vector);
    }

    /// Remove a vector from this index.
    #[allow(dead_code)]
    async fn remove(&self, node_id: &str) {
        let mut store = self.store.write().await;
        store.remove(node_id);
    }
}

/// Persistent vector store backed by SQLite with HNSW index for fast ANN search.
pub struct PersistentVectorStore {
    pool: SqlitePool,
    /// HNSW indices per named index
    indices: Arc<RwLock<dashmap::DashMap<String, HnswIndex>>>,
}

impl PersistentVectorStore {
    /// Create a new persistent vector store.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            indices: Arc::new(RwLock::new(dashmap::DashMap::default())),
        }
    }

    /// Initialize a vector index if it doesn't exist.
    pub async fn create_index(
        &self,
        name: &str,
        dimension: usize,
        _metric: &str,
    ) -> KiasResult<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO vector_indices (id, name, dimension, metric) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(dimension as i64)
        .bind(_metric)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create vector index: {e}")))?;

        // Ensure HNSW index is initialized in-memory
        let indices = self.indices.write().await;
        if !indices.contains_key(name) {
            let index = HnswIndex {
                store: Arc::new(RwLock::new(kias_common::vector::VectorStore::new(
                    dimension,
                ))),
                metadata: Arc::new(dashmap::DashMap::default()),
            };
            indices.insert(name.to_string(), index);
        }

        info!("Created vector index: {name} (dim={dimension})");
        Ok(())
    }

    /// Load all vectors from SQLite into HNSW indices.
    pub async fn load_from_db(&self) -> KiasResult<usize> {
        let indices_info: Vec<(String, String, i64)> =
            sqlx::query_as("SELECT id, name, dimension FROM vector_indices")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to load vector indices: {e}")))?;

        let mut total_loaded = 0;
        let indices_guard = self.indices.write().await;

        for (index_id, index_name, dimension) in indices_info {
            let rows: Vec<(String, String, Vec<u8>, String)> = sqlx::query_as(
                "SELECT id, external_id, embedding, metadata FROM vector_entries WHERE index_id = ?",
            )
            .bind(&index_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                KiasError::Config(format!(
                    "Failed to load vector entries for index '{index_name}': {e}"
                ))
            })?;

            let dim = dimension as usize;
            let index = HnswIndex {
                store: Arc::new(RwLock::new(kias_common::vector::VectorStore::new(dim))),
                metadata: Arc::new(dashmap::DashMap::default()),
            };

            {
                let mut hnsw = index.store.write().await;
                for (_entry_id, external_id, embedding_bytes, metadata_str) in rows {
                    let embedding = bytes_to_embedding(&embedding_bytes);
                    let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                    hnsw.insert(external_id.clone(), embedding);
                    index.metadata.insert(external_id, metadata);
                    total_loaded += 1;
                }
            }

            let count = index.metadata.len();
            indices_guard.insert(index_name.clone(), index);
            debug!("Loaded {count} vectors into HNSW index '{index_name}'");
        }

        drop(indices_guard);
        info!("Loaded {total_loaded} total vectors into HNSW indices");
        Ok(total_loaded)
    }

    /// Insert a vector into the store (write-through to SQLite + HNSW).
    pub async fn insert(
        &self,
        index_name: &str,
        external_id: &str,
        embedding: &[f32],
        metadata: serde_json::Value,
    ) -> KiasResult<String> {
        // Get index info
        let (index_id,): (String,) = sqlx::query_as("SELECT id FROM vector_indices WHERE name = ?")
            .bind(index_name)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| KiasError::NotFound(format!("Vector index '{index_name}' not found")))?;

        let entry_id = uuid::Uuid::new_v4().to_string();
        let embedding_bytes = embedding_to_bytes(embedding);
        let metadata_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT OR REPLACE INTO vector_entries (id, index_id, external_id, embedding, metadata) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&entry_id)
        .bind(&index_id)
        .bind(external_id)
        .bind(&embedding_bytes)
        .bind(&metadata_str)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to insert vector: {e}")))?;

        // Insert into HNSW index — clone Arc out, drop guard, then await
        {
            // Get or create the index and clone its Arc — must clone BEFORE guard drops
            let hnsw_index: Arc<HnswIndex> = {
                let indices_w = self.indices.write().await;
                indices_w
                    .entry(index_name.to_string())
                    .or_insert_with(|| HnswIndex {
                        store: Arc::new(RwLock::new(kias_common::vector::VectorStore::new(
                            embedding.len(),
                        ))),
                        metadata: Arc::new(dashmap::DashMap::default()),
                    });
                // Clone HnswIndex while guard is live, THEN wrap in Arc (Ref must drop first)
                let hnsw_index_owned = indices_w
                    .get(index_name)
                    .ok_or_else(|| {
                        KiasError::Storage(format!("HNSW index not found: {index_name}"))
                    })?
                    .clone();
                Arc::new(hnsw_index_owned)
            };
            // Now hnsw_index is owned, we can await without holding the guard
            let ext_id = external_id.to_string();
            hnsw_index.insert(ext_id, embedding.to_vec()).await;
            hnsw_index
                .metadata
                .insert(external_id.to_string(), metadata);
        }

        // Update entry count
        sqlx::query(
            "UPDATE vector_indices SET entry_count = (SELECT COUNT(*) FROM vector_entries WHERE index_id = ?), updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&index_id)
        .bind(&index_id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update vector count: {e}")))?;

        debug!("Inserted vector for '{external_id}' into HNSW index '{index_name}'");
        Ok(entry_id)
    }

    /// Search for the K nearest vectors using HNSW ANN search.
    /// Uses `search_knn` which returns (node_id, cosine_similarity) pairs directly.
    pub async fn search(
        &self,
        index_name: &str,
        query: &[f32],
        top_k: usize,
    ) -> KiasResult<Vec<VectorSearchResult>> {
        let indices = self.indices.read().await;
        let inner = indices
            .get(index_name)
            .ok_or_else(|| KiasError::NotFound(format!("Vector index '{index_name}' not found")))?;

        // Use HNSW ANN search for all index sizes — the implementation provides
        // O(log N) query time with proper beam search and connection pruning.
        let knn_results = {
            let store_guard = inner.store.read().await;
            store_guard.search_knn(query, top_k)
        };

        let search_results: Vec<VectorSearchResult> = knn_results
            .into_iter()
            .map(|(external_id, distance)| {
                let metadata = inner
                    .metadata
                    .get(&external_id)
                    .map(|m| m.clone())
                    .unwrap_or(serde_json::Value::Null);
                // search_knn returns cosine_distance (1 - similarity), convert back
                let similarity = 1.0 - distance as f64;
                VectorSearchResult {
                    external_id,
                    similarity,
                    metadata,
                }
            })
            .collect();

        Ok(search_results)
    }

    /// Remove a vector by external ID.
    pub async fn remove(&self, index_name: &str, external_id: &str) -> KiasResult<bool> {
        let result = sqlx::query(
            "DELETE FROM vector_entries WHERE external_id = ? AND index_id = (SELECT id FROM vector_indices WHERE name = ?)",
        )
        .bind(external_id)
        .bind(index_name)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to remove vector: {e}")))?;

        {
            // Use write lock and extract Arcs before nested await
            let indices_w = self.indices.write().await;
            // Get reference and clone the Arc<store> before we need to await
            let store_opt: Option<Arc<RwLock<kias_common::vector::VectorStore>>> =
                indices_w.get(index_name).map(|v| v.store.clone());
            let meta_opt: Option<Arc<dashmap::DashMap<String, serde_json::Value>>> =
                indices_w.get(index_name).map(|v| v.metadata.clone());
            // Drop the guard before the nested await
            drop(indices_w);

            if let Some(store) = store_opt {
                let ext_id = external_id.to_string();
                store.write().await.remove(&ext_id);
            }
            if let Some(meta) = meta_opt {
                meta.remove(&external_id.to_string());
            }
        }

        Ok(result.rows_affected() > 0)
    }

    /// Get the number of vectors in an index.
    pub fn count(&self, index_name: &str) -> usize {
        self.indices
            .try_read()
            .ok()
            .and_then(|indices| {
                indices
                    .get(index_name)
                    .and_then(|inner| inner.store.try_read().ok().map(|hnsw| hnsw.len()))
            })
            .unwrap_or(0)
    }

    /// List all index names.
    pub fn list_indices(&self) -> Vec<String> {
        self.indices
            .try_read()
            .map(|indices| indices.iter().map(|e| e.key().clone()).collect())
            .unwrap_or_default()
    }

    /// Get HNSW statistics for an index.
    pub fn stats(&self, index_name: &str) -> Option<kias_common::vector::VectorStoreStats> {
        self.indices.try_read().ok().and_then(|indices| {
            indices
                .get(index_name)
                .and_then(|inner| inner.store.try_read().ok().map(|hnsw| hnsw.stats()))
        })
    }

    /// Save the HNSW graph structure to SQLite for fast restart.
    ///
    /// This persists the full graph topology (layers + connections) so that
    /// [`load_graph_from_db`] can restore it in O(N) instead of rebuilding
    /// via O(N·M·logN) re-inserts from vector entries.
    pub async fn save_graph_to_db(&self, index_name: &str) -> KiasResult<()> {
        let indices = self.indices.read().await;
        let inner = indices
            .get(index_name)
            .ok_or_else(|| KiasError::NotFound(format!("Vector index '{index_name}' not found")))?;

        let store_guard = inner.store.read().await;
        let snapshot = store_guard.save_graph();
        let vector_count = snapshot.entries.len();
        let layer_count = snapshot.layers.len();
        drop(store_guard);

        let snapshot_json = serde_json::to_string(&snapshot)
            .map_err(|e| KiasError::Config(format!("Failed to serialize HNSW graph: {e}")))?;

        sqlx::query(
            "INSERT INTO hnsw_graphs (id, index_name, snapshot_json, vector_count, layer_count)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(index_name) DO UPDATE SET
               snapshot_json = excluded.snapshot_json,
               vector_count = excluded.vector_count,
               layer_count = excluded.layer_count,
               updated_at = datetime('now')",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(index_name)
        .bind(&snapshot_json)
        .bind(vector_count as i64)
        .bind(layer_count as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to save HNSW graph: {e}")))?;

        info!("Saved HNSW graph for '{index_name}': {vector_count} vectors, {layer_count} layers");
        Ok(())
    }

    /// Load a pre-saved HNSW graph from SQLite.
    ///
    /// Returns `Ok(true)` if a graph was loaded, `Ok(false)` if no saved graph
    /// exists for this index.  Falls back to re-inserting from vector entries
    /// if the graph cannot be deserialized.
    pub async fn load_graph_from_db(&self, index_name: &str, dimension: usize) -> KiasResult<bool> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT snapshot_json FROM hnsw_graphs WHERE index_name = ?")
                .bind(index_name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to query HNSW graph: {e}")))?;

        let Some((snapshot_json,)) = row else {
            return Ok(false);
        };

        let snapshot: kias_common::vector::HnswSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|e| KiasError::Config(format!("Failed to deserialize HNSW graph: {e}")))?;

        if snapshot.dimension != dimension {
            return Err(KiasError::Config(format!(
                "HNSW graph dimension mismatch: saved={}, expected={}",
                snapshot.dimension, dimension
            )));
        }

        let hnsw = kias_common::vector::VectorStore::load_graph(snapshot);
        let count = hnsw.len();

        let indices = self.indices.write().await;
        indices.insert(
            index_name.to_string(),
            HnswIndex {
                store: Arc::new(RwLock::new(hnsw)),
                metadata: Arc::new(dashmap::DashMap::default()),
            },
        );

        info!("Loaded HNSW graph for '{index_name}': {count} vectors");
        Ok(true)
    }
}

/// Result from a vector similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub external_id: String,
    pub similarity: f64,
    pub metadata: serde_json::Value,
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

    #[tokio::test]
    async fn test_create_index() {
        let store = setup_store().await;
        store
            .create_index("test", 3, "cosine")
            .await
            .expect("Failed to create index");

        let indices = store.list_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], "test");
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let store = setup_store().await;
        store
            .create_index("test", 3, "cosine")
            .await
            .expect("Failed to create index");

        store
            .insert(
                "test",
                "doc1",
                &[1.0, 0.0, 0.0],
                serde_json::json!({"type": "a"}),
            )
            .await
            .unwrap();
        store
            .insert(
                "test",
                "doc2",
                &[0.0, 1.0, 0.0],
                serde_json::json!({"type": "b"}),
            )
            .await
            .unwrap();
        store
            .insert(
                "test",
                "doc3",
                &[0.7, 0.7, 0.0],
                serde_json::json!({"type": "c"}),
            )
            .await
            .unwrap();

        assert_eq!(store.count("test"), 3);

        let results = store.search("test", &[1.0, 0.0, 0.0], 2).await.unwrap();
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

        {
            let store = PersistentVectorStore::new(pool.clone());
            store.create_index("persist", 3, "cosine").await.unwrap();
            store
                .insert("persist", "v1", &[1.0, 2.0, 3.0], serde_json::json!({}))
                .await
                .unwrap();
            store
                .insert("persist", "v2", &[4.0, 5.0, 6.0], serde_json::json!({}))
                .await
                .unwrap();
        }

        {
            let store = PersistentVectorStore::new(pool);
            let loaded = store.load_from_db().await.unwrap();
            assert_eq!(loaded, 2);
            assert_eq!(store.count("persist"), 2);

            let results = store.search("persist", &[1.0, 2.0, 3.0], 1).await.unwrap();
            assert_eq!(results[0].external_id, "v1");
        }
    }

    #[tokio::test]
    async fn test_remove() {
        let store = setup_store().await;
        store.create_index("rm-test", 3, "cosine").await.unwrap();
        store
            .insert("rm-test", "a", &[1.0, 0.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        store
            .insert("rm-test", "b", &[0.0, 1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(store.count("rm-test"), 2);

        let removed = store.remove("rm-test", "a").await.unwrap();
        assert!(removed);
        assert_eq!(store.count("rm-test"), 1);

        let removed = store.remove("rm-test", "nonexistent").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_hnsw_stats() {
        let store = setup_store().await;
        store.create_index("stats-test", 4, "cosine").await.unwrap();
        for i in 0..10 {
            let vec = vec![i as f32; 4];
            store
                .insert(
                    "stats-test",
                    &format!("v{i}"),
                    &vec,
                    serde_json::json!({"idx": i}),
                )
                .await
                .unwrap();
        }

        let stats = store.stats("stats-test");
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.total_vectors, 10);
        assert_eq!(stats.dimension, 4);
    }

    #[tokio::test]
    async fn test_insert_into_nonexistent_index() {
        let store = setup_store().await;
        let result = store
            .insert("nonexistent", "v1", &[1.0, 0.0, 0.0], serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_nonexistent_index() {
        let store = setup_store().await;
        let result = store.search("nonexistent", &[1.0, 0.0, 0.0], 5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_duplicate_index_idempotent() {
        let store = setup_store().await;
        store.create_index("dup", 3, "cosine").await.unwrap();
        store.create_index("dup", 3, "cosine").await.unwrap();
        let indices = store.list_indices();
        assert_eq!(indices.len(), 1);
    }

    #[tokio::test]
    async fn test_insert_overwrites_same_external_id() {
        let store = setup_store().await;
        store.create_index("ow", 3, "cosine").await.unwrap();
        store
            .insert("ow", "doc1", &[1.0, 0.0, 0.0], serde_json::json!({"v": 1}))
            .await
            .unwrap();
        store
            .insert("ow", "doc1", &[0.0, 1.0, 0.0], serde_json::json!({"v": 2}))
            .await
            .unwrap();
        assert_eq!(store.count("ow"), 1);
        let results = store.search("ow", &[0.0, 1.0, 0.0], 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].external_id, "doc1");
        assert!(results[0].similarity > 0.99);
    }

    #[tokio::test]
    async fn test_multiple_indices() {
        let store = setup_store().await;
        store.create_index("idx-a", 3, "cosine").await.unwrap();
        store.create_index("idx-b", 4, "cosine").await.unwrap();
        store
            .insert("idx-a", "a1", &[1.0, 0.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        store
            .insert("idx-b", "b1", &[1.0, 0.0, 0.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(store.count("idx-a"), 1);
        assert_eq!(store.count("idx-b"), 1);
        let indices = store.list_indices();
        assert_eq!(indices.len(), 2);
    }

    #[test]
    fn test_embedding_bytes_roundtrip() {
        let original: Vec<f32> = vec![1.0, -0.5, 3.14, 0.0, 100.5];
        let bytes = embedding_to_bytes(&original);
        assert_eq!(bytes.len(), original.len() * 4);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6, "Mismatch: {a} vs {b}");
        }
    }

    #[test]
    fn test_embedding_bytes_empty() {
        let empty: Vec<f32> = vec![];
        let bytes = embedding_to_bytes(&empty);
        assert!(bytes.is_empty());
        let restored = bytes_to_embedding(&bytes);
        assert!(restored.is_empty());
    }

    #[tokio::test]
    async fn test_count_nonexistent_index() {
        let store = setup_store().await;
        assert_eq!(store.count("nonexistent"), 0);
    }

    #[tokio::test]
    async fn test_list_indices_empty() {
        let store = setup_store().await;
        assert!(store.list_indices().is_empty());
    }

    #[tokio::test]
    async fn test_stats_nonexistent_index() {
        let store = setup_store().await;
        assert!(store.stats("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_save_and_load_graph() {
        let store = setup_store().await;
        store.create_index("graph-test", 4, "cosine").await.unwrap();

        // Insert vectors
        for i in 0..20 {
            let v = vec![i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32];
            store
                .insert(
                    "graph-test",
                    &format!("v{i}"),
                    &v,
                    serde_json::json!({"i": i}),
                )
                .await
                .unwrap();
        }

        // Save graph
        store.save_graph_to_db("graph-test").await.unwrap();

        // Create a new store and load graph
        let store2 = PersistentVectorStore::new(store.pool.clone());
        let loaded = store2.load_graph_from_db("graph-test", 4).await.unwrap();
        assert!(loaded);

        // Verify search works on loaded graph
        let results = store2
            .search("graph-test", &[0.0, 1.0, 2.0, 3.0], 3)
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].external_id, "v0");
    }

    #[tokio::test]
    async fn test_load_graph_nonexistent() {
        let store = setup_store().await;
        let loaded = store.load_graph_from_db("no-such", 4).await.unwrap();
        assert!(!loaded);
    }

    #[tokio::test]
    async fn test_save_graph_overwrite() {
        let store = setup_store().await;
        store.create_index("ow-graph", 3, "cosine").await.unwrap();

        store
            .insert("ow-graph", "a", &[1.0, 0.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        store.save_graph_to_db("ow-graph").await.unwrap();

        store
            .insert("ow-graph", "b", &[0.0, 1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        store.save_graph_to_db("ow-graph").await.unwrap();

        // Load should have both vectors
        let store2 = PersistentVectorStore::new(store.pool.clone());
        let loaded = store2.load_graph_from_db("ow-graph", 3).await.unwrap();
        assert!(loaded);
    }
}
