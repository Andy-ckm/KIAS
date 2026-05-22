//! # HNSW-rs Production Implementation
//!
//! Production-grade vector store backed by the `hnsw_rs` crate.
//! Provides the same public API as the hand-rolled `VectorStore` in `vector.rs`,
//! enabling drop-in replacement via `cfg(feature = "real-hnsw")`.

use hnsw_rs::dist::DistL2;
use hnsw_rs::hnsw::Hnsw;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Statistics about an HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreStats {
    pub vector_count: usize,
    pub dimension: usize,
    pub layer_count: usize,
    pub avg_connections_per_node: f64,
}

/// A single vector entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub node_id: String,
    pub vector: Vec<f32>,
}

/// Supported distance metrics for the HNSW index
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HnswMetric {
    L2,
    Cosine,
    L1,
    Dot,
}

impl Default for HnswMetric {
    fn default() -> Self {
        Self::L2
    }
}

/// Production HNSW vector store backed by `hnsw_rs`.
///
/// Uses `RwLock` for thread-safe concurrent reads and serial writes.
/// All `f32` vectors are stored with `DistL2` distance by default.
pub struct VectorStore {
    /// Dimension of vectors
    dimension: usize,
    /// The HNSW index (L2 distance)
    hnsw: RwLock<Hnsw<'static, f32, DistL2>>,
    /// Stored entries indexed by node_id for metadata lookup
    entries: RwLock<HashMap<String, VectorEntry>>,
    /// Mapping from node_id → hnsw internal integer id
    id_map: RwLock<HashMap<String, usize>>,
    /// Reverse mapping from hnsw internal id → node_id
    rev_id_map: RwLock<HashMap<usize, String>>,
    /// Next available HNSW internal ID
    next_id: RwLock<usize>,
    /// Max connections per node (M parameter)
    max_nb_connection: usize,
    /// Max layer count
    #[allow(dead_code)]
    max_layer: usize,
    /// ef_construction parameter
    #[allow(dead_code)]
    ef_construction: usize,
    /// ef_search parameter
    ef_search: usize,
}

impl VectorStore {
    /// Create a new vector store with the given dimension.
    /// Uses default HNSW parameters: M=16, M_max0=32, ef_construction=200, ef_search=100.
    pub fn new(dimension: usize) -> Self {
        Self::with_params(dimension, 16, 16, 200, 100)
    }

    /// Create a vector store with custom HNSW parameters.
    pub fn with_params(
        dimension: usize,
        max_nb_connection: usize,
        max_layer: usize,
        ef_construction: usize,
        ef_search: usize,
    ) -> Self {
        let hnsw = Hnsw::new(
            max_nb_connection,
            1_000_000, // max_elements (pre-allocation)
            max_layer,
            ef_construction,
            DistL2,
        );

        Self {
            dimension,
            hnsw: RwLock::new(hnsw),
            entries: RwLock::new(HashMap::new()),
            id_map: RwLock::new(HashMap::new()),
            rev_id_map: RwLock::new(HashMap::new()),
            next_id: RwLock::new(0),
            max_nb_connection,
            max_layer,
            ef_construction,
            ef_search,
        }
    }

    /// Insert a vector into the HNSW index.
    pub fn insert(&mut self, node_id: String, vector: Vec<f32>) {
        // Lock for writing
        let hnsw_id = {
            let mut next = self.next_id.write().expect("next_id lock poisoned");
            let id = *next;
            *next += 1;
            id
        };

        // Insert into HNSW index
        {
            let hnsw = self.hnsw.read().expect("hnsw lock poisoned");
            hnsw.insert((&vector, hnsw_id));
        }

        // Store entry
        let entry = VectorEntry {
            node_id: node_id.clone(),
            vector,
        };

        {
            let mut entries = self.entries.write().expect("entries lock poisoned");
            entries.insert(node_id.clone(), entry);
        }

        {
            let mut id_map = self.id_map.write().expect("id_map lock poisoned");
            id_map.insert(node_id.clone(), hnsw_id);
        }

        {
            let mut rev = self.rev_id_map.write().expect("rev_id_map lock poisoned");
            rev.insert(hnsw_id, node_id);
        }
    }

    /// Search for the K nearest neighbors using HNSW ANN search.
    /// Returns `(node_id, distance)` pairs sorted by distance ascending.
    pub fn search_knn(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let hnsw = self.hnsw.read().expect("hnsw lock poisoned");
        let neighbours = hnsw.search(query, k, self.ef_search);

        let rev = self.rev_id_map.read().expect("rev_id_map lock poisoned");

        neighbours
            .into_iter()
            .filter_map(|nb| {
                let hnsw_id = nb.get_origin_id();
                let distance = nb.get_distance();
                rev.get(&hnsw_id).map(|node_id| (node_id.clone(), distance))
            })
            .collect()
    }

    /// Exact brute-force search (fallback for validation / small datasets).
    pub fn search_exact(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let entries = self.entries.read().expect("entries lock poisoned");

        let mut scored: Vec<(String, f32)> = entries
            .values()
            .map(|e| {
                let dist = l2_distance(query, &e.vector);
                (e.node_id.clone(), dist)
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Get a reference to a stored entry by node_id.
    pub fn get(&self, node_id: &str) -> Option<VectorEntry> {
        let entries = self.entries.read().expect("entries lock poisoned");
        entries.get(node_id).cloned()
    }

    /// Number of stored vectors.
    pub fn len(&self) -> usize {
        let entries = self.entries.read().expect("entries lock poisoned");
        entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The configured vector dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Number of layers observed in the HNSW index.
    pub fn layer_count(&self) -> usize {
        let hnsw = self.hnsw.read().expect("hnsw lock poisoned");
        hnsw.get_max_level_observed() as usize + 1
    }

    /// Remove a vector by node_id.
    /// Note: hnsw_rs does not support incremental deletion; the entry is
    /// removed from the metadata map but remains in the graph.
    /// A full rebuild is required for true deletion.
    pub fn remove(&mut self, node_id: &str) -> bool {
        let hnsw_id = {
            let mut id_map = self.id_map.write().expect("id_map lock poisoned");
            id_map.remove(node_id)
        };

        if let Some(hnsw_id) = hnsw_id {
            {
                let mut entries = self.entries.write().expect("entries lock poisoned");
                entries.remove(node_id);
            }
            {
                let mut rev = self.rev_id_map.write().expect("rev_id_map lock poisoned");
                rev.remove(&hnsw_id);
            }
            true
        } else {
            false
        }
    }

    /// Compute statistics about the current index.
    pub fn stats(&self) -> VectorStoreStats {
        let entries = self.entries.read().expect("entries lock poisoned");
        let hnsw = self.hnsw.read().expect("hnsw lock poisoned");

        VectorStoreStats {
            vector_count: entries.len(),
            dimension: self.dimension,
            layer_count: hnsw.get_max_level_observed() as usize + 1,
            avg_connections_per_node: self.max_nb_connection as f64,
        }
    }

    /// Serialize the HNSW graph to a snapshot (for persistence).
    pub fn save_graph(&self) -> HnswSnapshot {
        let entries = self.entries.read().expect("entries lock poisoned");
        let id_map = self.id_map.read().expect("id_map lock poisoned");
        let rev = self.rev_id_map.read().expect("rev_id_map lock poisoned");

        HnswSnapshot {
            entries: entries.clone(),
            id_map: id_map.clone(),
            rev_id_map: rev.clone(),
            dimension: self.dimension,
        }
    }

    /// Restore from a snapshot.
    pub fn load_graph(snapshot: HnswSnapshot) -> Self {
        let mut store = Self::new(snapshot.dimension);

        // Re-insert all entries to rebuild the HNSW graph
        for (node_id, entry) in &snapshot.entries {
            store.insert(node_id.clone(), entry.vector.clone());
        }

        store
    }
}

/// Serializable snapshot of the HNSW graph for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswSnapshot {
    pub entries: HashMap<String, VectorEntry>,
    pub id_map: HashMap<String, usize>,
    pub rev_id_map: HashMap<usize, String>,
    pub dimension: usize,
}

/// Compute L2 (Euclidean) distance between two vectors.
fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vec(dim: usize, seed: usize) -> Vec<f32> {
        (0..dim).map(|i| ((i + seed) as f32 * 0.1).sin()).collect()
    }

    #[test]
    fn test_insert_and_search() {
        let mut store = VectorStore::new(32);
        for i in 0..100 {
            store.insert(format!("item_{i}"), random_vec(32, i));
        }

        let query = random_vec(32, 0);
        let results = store.search_knn(&query, 5);
        assert_eq!(results.len(), 5);
        // item_0 should be closest
        assert_eq!(results[0].0, "item_0");
        assert!(results[0].1 < 0.001);
    }

    #[test]
    fn test_exact_search_agrees_with_hnsw() {
        let mut store = VectorStore::new(16);
        for i in 0..50 {
            store.insert(format!("e_{i}"), random_vec(16, i));
        }

        let query = random_vec(16, 25);
        let hnsw_results = store.search_knn(&query, 5);
        let exact_results = store.search_exact(&query, 5);

        // Top-1 should agree
        assert_eq!(hnsw_results[0].0, exact_results[0].0);
    }

    #[test]
    fn test_get_and_len() {
        let mut store = VectorStore::new(8);
        assert!(store.is_empty());

        store.insert("a".into(), vec![1.0; 8]);
        store.insert("b".into(), vec![2.0; 8]);

        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());

        let entry = store.get("a").expect("entry not found");
        assert_eq!(entry.vector, vec![1.0; 8]);
    }

    #[test]
    fn test_remove() {
        let mut store = VectorStore::new(4);
        store.insert("x".into(), vec![0.0; 4]);
        assert_eq!(store.len(), 1);

        assert!(store.remove("x"));
        assert_eq!(store.len(), 0);
        assert!(!store.remove("x")); // already removed
    }

    #[test]
    fn test_stats() {
        let mut store = VectorStore::new(64);
        for i in 0..10 {
            store.insert(format!("s_{i}"), random_vec(64, i));
        }

        let stats = store.stats();
        assert_eq!(stats.vector_count, 10);
        assert_eq!(stats.dimension, 64);
        assert!(stats.layer_count >= 1);
    }

    #[test]
    fn test_save_and_load_graph() {
        let mut store = VectorStore::new(16);
        for i in 0..20 {
            store.insert(format!("p_{i}"), random_vec(16, i));
        }

        let snapshot = store.save_graph();
        let restored = VectorStore::load_graph(snapshot);

        assert_eq!(restored.len(), 20);
        let query = random_vec(16, 5);
        let results = restored.search_knn(&query, 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "p_5");
    }

    #[test]
    fn test_search_empty_store() {
        let store = VectorStore::new(8);
        let results = store.search_knn(&[0.0; 8], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_with_params() {
        let store = VectorStore::with_params(128, 32, 32, 400, 200);
        assert_eq!(store.dimension(), 128);
    }

    #[test]
    fn test_layer_count() {
        let mut store = VectorStore::new(16);
        for i in 0..100 {
            store.insert(format!("l_{i}"), random_vec(16, i));
        }
        assert!(store.layer_count() >= 1);
    }
}
