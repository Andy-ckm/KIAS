//! # Vector Store with HNSW Index
//!
//! Core vector types and HNSW approximate nearest neighbor search.
//! Moved from `kias-knowledge` to `kias-common` to fix cross-layer dependency
//! (data-store L1 was depending on knowledge L2).
//!
//! ## HNSW Index
//!
//! The Hierarchical Navigable Small World index organizes vectors into layers:
//! - Layer 0: All vectors, dense connections
//! - Layer 1+: Sparse connections, used for navigation
//!
//! Search starts at the top layer and descends, achieving O(log N) query time.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Dimension of embedding vectors (configurable, default 128 for efficiency)
pub const DEFAULT_EMBEDDING_DIM: usize = 128;

/// Maximum number of connections per node in HNSW layer 0
pub const HNSW_M: usize = 16;

/// Maximum number of connections per node in higher layers
pub const HNSW_M_MAX: usize = 32;

/// Size of the dynamic candidate list during construction
pub const HNSW_EF_CONSTRUCTION: usize = 200;

/// Size of the dynamic candidate list during search
pub const HNSW_EF_SEARCH: usize = 100;

/// Probability level factor for layer assignment (1/ln(M))
pub fn hnsw_ml() -> f64 {
    1.0 / (HNSW_M as f64).ln()
}

// ============================================================
// Distance Functions
// ============================================================

/// Compute cosine similarity between two vectors.
/// Uses chunked f32x4-style accumulation for better ILP on modern CPUs.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let len = a.len();
    let chunks = len / 4;
    let remainder = len % 4;

    let mut dot4 = [0.0f32; 4];
    let mut na4 = [0.0f32; 4];
    let mut nb4 = [0.0f32; 4];

    for i in 0..chunks {
        let off = i * 4;
        for j in 0..4 {
            let x = a[off + j];
            let y = b[off + j];
            dot4[j] += x * y;
            na4[j] += x * x;
            nb4[j] += y * y;
        }
    }

    let mut dot = dot4[0] + dot4[1] + dot4[2] + dot4[3];
    let mut norm_a = na4[0] + na4[1] + na4[2] + na4[3];
    let mut norm_b = nb4[0] + nb4[1] + nb4[2] + nb4[3];

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        let x = a[i];
        let y = b[i];
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0)
}

/// Compute cosine distance (1 - similarity) for HNSW search
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_similarity(a, b)
}

/// Compute L2 (Euclidean) distance between two vectors
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

// ============================================================
// Vector Store with HNSW Index
// ============================================================

/// A stored vector entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Node ID from the knowledge graph
    pub node_id: String,
    /// The embedding vector
    pub vector: Vec<f32>,
    /// Which HNSW layer this entry appears in (highest layer)
    pub max_layer: usize,
}

/// A candidate in the priority queue during HNSW search
#[derive(Debug, Clone)]
struct Candidate {
    node_id: String,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap by distance (reverse for BinaryHeap max-heap)
        match (self.distance.is_finite(), other.distance.is_finite()) {
            (false, false) => std::cmp::Ordering::Equal,
            (false, true) => std::cmp::Ordering::Greater,
            (true, false) => std::cmp::Ordering::Less,
            (true, true) => other
                .distance
                .partial_cmp(&self.distance)
                .unwrap_or(std::cmp::Ordering::Equal),
        }
    }
}

/// HNSW layer: maps node_id -> set of neighbor node_ids
type Layer = HashMap<String, HashSet<String>>;

/// Serializable snapshot of the HNSW graph structure.
///
/// Captures the full topology (layers, connections, entry point) so that
/// after a restart the graph can be restored in O(N) instead of rebuilding
/// via O(N·M·logN) re-inserts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswSnapshot {
    /// All vector entries indexed by node_id.
    pub entries: HashMap<String, VectorEntry>,
    /// Adjacency lists per layer.  `layers[l][node_id]` = set of neighbor ids.
    pub layers: Vec<HashMap<String, Vec<String>>>,
    /// Entry point node id (highest-layer node).
    pub entry_point: Option<String>,
    /// Embedding dimension.
    pub dimension: usize,
}

/// Vector store with HNSW approximate nearest neighbor index
pub struct VectorStore {
    /// All stored vectors indexed by node_id
    entries: HashMap<String, VectorEntry>,
    /// HNSW layers (layer 0 is the bottom, most dense)
    layers: Vec<Layer>,
    /// Entry point for HNSW search (node at highest layer)
    entry_point: Option<String>,
    /// Embedding dimension
    dimension: usize,
}

impl VectorStore {
    /// Create a new empty vector store.
    pub fn new(dimension: usize) -> Self {
        Self {
            entries: HashMap::new(),
            layers: vec![Layer::new()],
            entry_point: None,
            dimension,
        }
    }

    /// Save the full HNSW graph structure to a serializable snapshot.
    ///
    /// Use [`load_graph`] to restore.  The snapshot captures all layers and
    /// connections so the topology survives restarts without re-insertion.
    pub fn save_graph(&self) -> HnswSnapshot {
        let layers_ser: Vec<HashMap<String, Vec<String>>> = self
            .layers
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                    .collect()
            })
            .collect();

        HnswSnapshot {
            entries: self.entries.clone(),
            layers: layers_ser,
            entry_point: self.entry_point.clone(),
            dimension: self.dimension,
        }
    }

    /// Restore an HNSW graph from a previously saved snapshot.
    ///
    /// This is O(N) and preserves the original graph topology, unlike
    /// re-inserting entries one by one which is O(N·M·logN) and may
    /// produce a different graph.
    pub fn load_graph(snapshot: HnswSnapshot) -> Self {
        let layers: Vec<Layer> = snapshot
            .layers
            .into_iter()
            .map(|layer| {
                layer
                    .into_iter()
                    .map(|(k, v)| (k, v.into_iter().collect()))
                    .collect()
            })
            .collect();

        Self {
            entries: snapshot.entries,
            layers,
            entry_point: snapshot.entry_point,
            dimension: snapshot.dimension,
        }
    }

    /// Insert a vector entry into the HNSW index
    pub fn insert(&mut self, node_id: String, vector: Vec<f32>) {
        assert_eq!(vector.len(), self.dimension, "Vector dimension mismatch");

        let max_layer = self.assign_layer(&node_id);

        while self.layers.len() <= max_layer {
            self.layers.push(Layer::new());
        }

        let entry = VectorEntry {
            node_id: node_id.clone(),
            vector,
            max_layer,
        };

        if self.entries.is_empty() {
            self.entry_point = Some(node_id.clone());
            self.entries.insert(node_id.clone(), entry);
            for layer in 0..=max_layer {
                self.layers[layer].insert(node_id.clone(), HashSet::new());
            }
            return;
        }

        let Some(entry_point) = self.entry_point.clone() else {
            return;
        };

        // Search from top layer down to max_layer+1
        let mut current_nearest = entry_point;
        for layer in (max_layer + 1..self.layers.len()).rev() {
            current_nearest = self
                .search_layer(&entry.vector, &current_nearest, layer, 1)
                .into_iter()
                .next()
                .map(|(id, _)| id)
                .unwrap_or(current_nearest);
        }

        // For each layer from max_layer down to 0
        for layer in (0..=max_layer.min(self.layers.len() - 1)).rev() {
            let neighbors =
                self.search_layer(&entry.vector, &current_nearest, layer, HNSW_EF_CONSTRUCTION);

            let m = if layer == 0 { HNSW_M_MAX } else { HNSW_M };
            let selected: Vec<String> =
                neighbors.iter().take(m).map(|(id, _)| id.clone()).collect();

            self.layers[layer].insert(node_id.clone(), selected.iter().cloned().collect());
            for neighbor_id in &selected {
                if let Some(neighbor_conns) = self.layers[layer].get_mut(neighbor_id) {
                    neighbor_conns.insert(node_id.clone());
                    let max_conn = if layer == 0 { HNSW_M_MAX } else { HNSW_M };
                    if neighbor_conns.len() > max_conn {
                        self.prune_connections(neighbor_id, layer, max_conn);
                    }
                }
            }

            current_nearest = neighbors
                .first()
                .map(|(id, _)| id.clone())
                .unwrap_or(current_nearest);
        }

        if let Some(ref ep) = self.entry_point {
            if let Some(ep_entry) = self.entries.get(ep) {
                if max_layer > ep_entry.max_layer {
                    self.entry_point = Some(node_id.clone());
                }
            }
        }

        self.entries.insert(node_id, entry);
    }

    /// Search for k nearest neighbors using HNSW
    pub fn search_knn(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if self.entries.is_empty() || k == 0 {
            return Vec::new();
        }

        let entry_point = match &self.entry_point {
            Some(ep) => ep.clone(),
            None => return Vec::new(),
        };

        let mut current_nearest = entry_point;
        for layer in (1..self.layers.len()).rev() {
            let results = self.search_layer(query, &current_nearest, layer, 1);
            current_nearest = results
                .into_iter()
                .next()
                .map(|(id, _)| id)
                .unwrap_or(current_nearest);
        }

        let results = self.search_layer(query, &current_nearest, 0, HNSW_EF_SEARCH.max(k));
        results.into_iter().take(k).collect()
    }

    /// Exact brute-force search (for comparison / small datasets)
    pub fn search_exact(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut scored: Vec<(String, f32)> = self
            .entries
            .values()
            .map(|entry| {
                let sim = cosine_similarity(query, &entry.vector);
                (entry.node_id.clone(), sim)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Get a vector by node_id
    pub fn get(&self, node_id: &str) -> Option<&VectorEntry> {
        self.entries.get(node_id)
    }

    /// Number of stored vectors
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get number of HNSW layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Remove a vector entry
    pub fn remove(&mut self, node_id: &str) -> bool {
        if self.entries.remove(node_id).is_none() {
            return false;
        }

        for layer in &mut self.layers {
            if let Some(neighbors) = layer.remove(node_id) {
                for neighbor_id in &neighbors {
                    if let Some(neighbor_conns) = layer.get_mut(neighbor_id) {
                        neighbor_conns.remove(node_id);
                    }
                }
            }
        }

        if self.entry_point.as_deref() == Some(node_id) {
            self.entry_point = self.entries.keys().next().cloned();
        }

        true
    }

    /// Get statistics about the HNSW index
    pub fn stats(&self) -> VectorStoreStats {
        let layer_stats: Vec<LayerStats> = self
            .layers
            .iter()
            .enumerate()
            .map(|(i, layer)| {
                let total_edges: usize = layer.values().map(|conns| conns.len()).sum();
                let avg_edges = if layer.is_empty() {
                    0.0
                } else {
                    total_edges as f64 / layer.len() as f64
                };
                LayerStats {
                    layer_index: i,
                    node_count: layer.len(),
                    total_edges,
                    avg_edges_per_node: avg_edges,
                }
            })
            .collect();

        VectorStoreStats {
            total_vectors: self.entries.len(),
            dimension: self.dimension,
            layers: layer_stats,
            entry_point: self.entry_point.clone(),
        }
    }

    // ==================== Internal HNSW methods ====================

    /// Assign a layer to a new node using exponential decay.
    fn assign_layer(&self, node_id: &str) -> usize {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in node_id.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= self.entries.len() as u64;
        hash = hash
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        let rand_val = ((hash >> 11) as f64) / ((1u64 << 53) as f64);

        if rand_val < 1e-10 {
            return 3;
        }
        let layer = (-rand_val.ln() * hnsw_ml()).floor() as usize;
        layer.min(5)
    }

    /// Search a layer for ef nearest neighbors using greedy beam search
    fn search_layer(
        &self,
        query: &[f32],
        entry_point: &str,
        layer: usize,
        ef: usize,
    ) -> Vec<(String, f32)> {
        if layer >= self.layers.len() {
            return Vec::new();
        }

        let layer_data = &self.layers[layer];

        let ep_vector = match self.entries.get(entry_point) {
            Some(e) => &e.vector,
            None => return Vec::new(),
        };
        let ep_dist = cosine_distance(query, ep_vector);

        let mut candidates = BinaryHeap::new();
        candidates.push(Candidate {
            node_id: entry_point.to_string(),
            distance: ep_dist,
        });

        let mut visited = HashSet::new();
        visited.insert(entry_point.to_string());

        let mut results = BinaryHeap::new();
        results.push(Candidate {
            node_id: entry_point.to_string(),
            distance: ep_dist,
        });

        while let Some(current) = candidates.pop() {
            let worst_result_dist = results.peek().map(|c| c.distance).unwrap_or(f32::MAX);

            if current.distance > worst_result_dist && results.len() >= ef {
                break;
            }

            if let Some(neighbors) = layer_data.get(&current.node_id) {
                for neighbor_id in neighbors {
                    if visited.contains(neighbor_id) {
                        continue;
                    }
                    visited.insert(neighbor_id.clone());

                    let neighbor_vector = match self.entries.get(neighbor_id) {
                        Some(e) => &e.vector,
                        None => continue,
                    };
                    let dist = cosine_distance(query, neighbor_vector);
                    let worst = results.peek().map(|c| c.distance).unwrap_or(f32::MAX);

                    if dist < worst || results.len() < ef {
                        candidates.push(Candidate {
                            node_id: neighbor_id.clone(),
                            distance: dist,
                        });
                        results.push(Candidate {
                            node_id: neighbor_id.clone(),
                            distance: dist,
                        });
                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        let mut output: Vec<(String, f32)> = results
            .into_iter()
            .map(|c| (c.node_id, c.distance))
            .collect();
        output.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        output
    }

    /// Prune connections to keep only the M best
    fn prune_connections(&mut self, node_id: &str, layer: usize, max_conn: usize) {
        let connections = match self.layers[layer].get(node_id) {
            Some(c) => c.clone(),
            None => return,
        };

        let node_vector = match self.entries.get(node_id) {
            Some(e) => e.vector.clone(),
            None => return,
        };

        let mut scored: Vec<(String, f32)> = connections
            .iter()
            .filter_map(|conn_id| {
                self.entries.get(conn_id).map(|e| {
                    let dist = cosine_distance(&node_vector, &e.vector);
                    (conn_id.clone(), dist)
                })
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let keep: HashSet<String> = scored
            .into_iter()
            .take(max_conn)
            .map(|(id, _)| id)
            .collect();

        if let Some(conns) = self.layers[layer].get_mut(node_id) {
            conns.retain(|id| keep.contains(id));
        }
    }
}

/// Statistics about a VectorStore
#[derive(Debug, Clone)]
pub struct VectorStoreStats {
    pub total_vectors: usize,
    pub dimension: usize,
    pub layers: Vec<LayerStats>,
    pub entry_point: Option<String>,
}

/// Statistics about a single HNSW layer
#[derive(Debug, Clone)]
pub struct LayerStats {
    pub layer_index: usize,
    pub node_count: usize,
    pub total_edges: usize,
    pub avg_edges_per_node: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Distance function tests =====

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_l2_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((l2_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    // ===== VectorStore tests =====

    #[test]
    fn test_vector_store_insert_and_search() {
        let mut store = VectorStore::new(4);

        store.insert("a".to_string(), vec![1.0, 0.0, 0.0, 0.0]);
        store.insert("b".to_string(), vec![0.0, 1.0, 0.0, 0.0]);
        store.insert("c".to_string(), vec![0.9, 0.1, 0.0, 0.0]);

        assert_eq!(store.len(), 3);

        let results = store.search_knn(&[1.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "c");
    }

    #[test]
    fn test_vector_store_exact_search() {
        let mut store = VectorStore::new(3);

        store.insert("x".to_string(), vec![1.0, 0.0, 0.0]);
        store.insert("y".to_string(), vec![0.0, 1.0, 0.0]);
        store.insert("z".to_string(), vec![0.7, 0.7, 0.0]);

        let results = store.search_exact(&[1.0, 0.0, 0.0], 3);
        assert_eq!(results[0].0, "x");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_vector_store_remove() {
        let mut store = VectorStore::new(3);

        store.insert("a".to_string(), vec![1.0, 0.0, 0.0]);
        store.insert("b".to_string(), vec![0.0, 1.0, 0.0]);

        assert_eq!(store.len(), 2);
        assert!(store.remove("a"));
        assert_eq!(store.len(), 1);

        let results = store.search_knn(&[1.0, 0.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "b");
    }

    #[test]
    fn test_vector_store_empty_search() {
        let store = VectorStore::new(3);
        let results = store.search_knn(&[1.0, 0.0, 0.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_vector_store_stats() {
        let mut store = VectorStore::new(4);

        for i in 0..10 {
            let v = vec![i as f32 / 10.0, 1.0 - i as f32 / 10.0, 0.0, 0.0];
            store.insert(format!("n{}", i), v);
        }

        let stats = store.stats();
        assert_eq!(stats.total_vectors, 10);
        assert_eq!(stats.dimension, 4);
        assert!(!stats.layers.is_empty());
        assert!(stats.entry_point.is_some());
    }

    #[test]
    fn test_vector_store_large_scale() {
        let dim = 32;
        let mut store = VectorStore::new(dim);

        for i in 0..500 {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 1.0;
            store.insert(format!("node_{}", i), v);
        }

        assert_eq!(store.len(), 500);

        let mut query = vec![0.0f32; dim];
        query[0] = 1.0;
        let results = store.search_knn(&query, 10);
        assert_eq!(results.len(), 10);
        for (id, dist) in &results {
            assert!(id.starts_with("node_"));
            assert!(*dist >= 0.0 && *dist <= 2.0);
        }
    }

    #[test]
    fn test_hnsw_recall_quality() {
        let dim = 32;
        let mut store = VectorStore::new(dim);

        let mut seed: u64 = 12345;
        for i in 0..50 {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 5.0;
            for (j, val) in v.iter_mut().enumerate().take(dim) {
                if j != i % dim {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    *val = ((seed >> 33) as f32) / (u32::MAX as f32) * 0.5;
                }
            }
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            for x in &mut v {
                *x /= norm;
            }
            store.insert(format!("v{}", i), v);
        }

        let mut query = vec![0.01f32; dim];
        query[0] = 5.0;
        let norm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut query {
            *x /= norm;
        }

        let exact = store.search_exact(&query, 10);
        let hnsw = store.search_knn(&query, 10);

        let exact_ids: HashSet<&String> = exact.iter().map(|(id, _)| id).collect();
        let hnsw_ids: HashSet<&String> = hnsw.iter().map(|(id, _)| id).collect();
        let overlap = exact_ids.intersection(&hnsw_ids).count();

        assert!(
            overlap >= 3,
            "HNSW recall too low: {}/10 overlap with exact search",
            overlap,
        );
    }

    #[test]
    fn test_save_load_graph_roundtrip() {
        let dim = 8;
        let mut store = VectorStore::new(dim);

        // Insert enough vectors to build multi-layer graph
        for i in 0..100 {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 1.0;
            store.insert(format!("node_{}", i), v);
        }

        let original_stats = store.stats();
        let original_entry_point = store.entry_point.clone();

        // Save graph
        let snapshot = store.save_graph();
        assert_eq!(snapshot.dimension, dim);
        assert_eq!(snapshot.entries.len(), 100);
        assert!(snapshot.layers.len() >= 1);

        // Serialize to JSON and back (simulates disk I/O)
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let restored_snapshot: HnswSnapshot = serde_json::from_str(&json).expect("deserialize");

        // Load into new store
        let restored = VectorStore::load_graph(restored_snapshot);

        // Verify structure matches
        assert_eq!(restored.len(), 100);
        assert_eq!(restored.dimension(), dim);
        assert_eq!(restored.layer_count(), store.layer_count());
        assert_eq!(restored.entry_point, original_entry_point);

        // Verify search produces similar results (HNSW is approximate,
        // and HashSet iteration order may differ after roundtrip)
        let mut query = vec![0.0f32; dim];
        query[0] = 1.0;
        let original_results = store.search_knn(&query, 5);
        let restored_results = restored.search_knn(&query, 5);
        assert_eq!(original_results.len(), restored_results.len());

        // At least 3 of 5 results should match (recall check)
        let original_ids: std::collections::HashSet<&String> =
            original_results.iter().map(|(id, _)| id).collect();
        let restored_ids: std::collections::HashSet<&String> =
            restored_results.iter().map(|(id, _)| id).collect();
        let overlap = original_ids.intersection(&restored_ids).count();
        assert!(
            overlap >= 3,
            "HNSW search recall too low after graph roundtrip: {}/5",
            overlap
        );

        // Verify stats match
        let restored_stats = restored.stats();
        assert_eq!(original_stats.total_vectors, restored_stats.total_vectors);
        assert_eq!(original_stats.layers.len(), restored_stats.layers.len());
        for (orig_layer, rest_layer) in original_stats
            .layers
            .iter()
            .zip(restored_stats.layers.iter())
        {
            assert_eq!(orig_layer.node_count, rest_layer.node_count);
            assert_eq!(orig_layer.total_edges, rest_layer.total_edges);
        }
    }

    #[test]
    fn test_save_load_empty_graph() {
        let store = VectorStore::new(4);
        let snapshot = store.save_graph();
        let restored = VectorStore::load_graph(snapshot);
        assert!(restored.is_empty());
        assert_eq!(restored.dimension(), 4);
        assert!(restored.entry_point.is_none());
    }
}
