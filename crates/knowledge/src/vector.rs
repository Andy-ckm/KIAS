//! # Vector Embedding Retrieval
//!
//! Provides semantic similarity search over knowledge nodes using vector embeddings.
//! Implements both exact cosine similarity search and HNSW-inspired approximate
//! nearest neighbor (ANN) search for sub-linear query time at scale.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌───────────────┐
//! │  EmbeddingEngine │────▶│   VectorStore     │────▶│ VectorRetriever│
//! │  (text → vec)    │     │  (HNSW index)    │     │ (Retriever trait│
//! └─────────────────┘     └──────────────────┘     └───────────────┘
//! ```
//!
//! ## HNSW Index
//!
//! The Hierarchical Navigable Small World index organizes vectors into layers:
//! - Layer 0: All vectors, dense connections
//! - Layer 1+: Sparse connections, used for navigation
//!
//! Search starts at the top layer and descends, achieving O(log N) query time.

use async_trait::async_trait;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info};

use super::graph::{KnowledgeGraph, NodeType};
use super::retriever::{MatchType, Retriever, ScoredNode};

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
// Embedding Engine
// ============================================================

/// Trait for text-to-vector embedding providers
#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    /// Convert text to an embedding vector
    async fn embed(&self, text: &str) -> KiasResult<Vec<f32>>;

    /// Batch embed multiple texts
    async fn embed_batch(&self, texts: &[&str]) -> KiasResult<Vec<Vec<f32>>>;

    /// Return the embedding dimension
    fn dimension(&self) -> usize;
}

/// Lightweight local embedding using SimHash-inspired approach.
///
/// Produces deterministic, fast embeddings without external API calls.
/// Uses a combination of character n-grams, word hashing, and random
/// projections to create semantically meaningful vectors.
///
/// This is NOT as powerful as transformer-based embeddings but provides
/// good enough similarity for local/development use.
pub struct LocalEmbeddingEngine {
    dimension: usize,
    /// Random projection matrix (seeded for determinism)
    projection: Vec<Vec<f32>>,
}

impl LocalEmbeddingEngine {
    /// Create a new local embedding engine
    pub fn new(dimension: usize) -> Self {
        let projection = Self::generate_projection_matrix(dimension);
        Self {
            dimension,
            projection,
        }
    }

    /// Create with default dimension
    pub fn default_dim() -> Self {
        Self::new(DEFAULT_EMBEDDING_DIM)
    }

    /// Generate a deterministic random projection matrix using a seeded PRNG
    fn generate_projection_matrix(dim: usize) -> Vec<Vec<f32>> {
        let mut seed: u64 = 42;
        let mut matrix = Vec::with_capacity(dim);

        for _ in 0..dim {
            let mut row = Vec::with_capacity(256);
            for _ in 0..256 {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let val = ((seed >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
                row.push(val);
            }
            matrix.push(row);
        }
        matrix
    }

    /// Extract character n-gram features from text
    fn extract_features(text: &str) -> Vec<f32> {
        let text_lower = text.to_lowercase();
        let chars: Vec<char> = text_lower.chars().collect();
        let mut features = vec![0.0f32; 256];

        // Word-level features (hash words into feature buckets)
        for word in text_lower.split_whitespace() {
            let hash = Self::hash_string(word);
            let bucket = (hash % 256) as usize;
            features[bucket] += 1.0;

            // Bigram features
            let bytes = word.as_bytes();
            for window in bytes.windows(2) {
                let bigram_hash = Self::hash_bytes(window);
                let bigram_bucket = (bigram_hash % 256) as usize;
                features[bigram_bucket] += 0.5;
            }
        }

        // Character trigram features for richer representation
        for window in chars.windows(3) {
            let trigram: String = window.iter().collect();
            let hash = Self::hash_string(&trigram);
            let bucket = (hash % 256) as usize;
            features[bucket] += 0.3;
        }

        // L2 normalize
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    /// Hash a string to u64 using FNV-1a
    fn hash_string(s: &str) -> u64 {
        Self::hash_bytes(s.as_bytes())
    }

    /// Hash bytes using FNV-1a
    fn hash_bytes(bytes: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Project 256-dim features to target dimension using random projection
    fn project(&self, features: &[f32]) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.dimension);
        for row in &self.projection {
            let val: f32 = row.iter().zip(features.iter()).map(|(a, b)| a * b).sum();
            result.push(val);
        }

        // L2 normalize the result
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut result {
                *v /= norm;
            }
        }

        result
    }
}

#[async_trait]
impl EmbeddingEngine for LocalEmbeddingEngine {
    async fn embed(&self, text: &str) -> KiasResult<Vec<f32>> {
        let features = Self::extract_features(text);
        Ok(self.project(&features))
    }

    async fn embed_batch(&self, texts: &[&str]) -> KiasResult<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
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
        // NaN distances are filtered out during search, but handle gracefully if they slip through
        match (self.distance.is_finite(), other.distance.is_finite()) {
            (false, false) => std::cmp::Ordering::Equal,
            (false, true) => std::cmp::Ordering::Greater, // NaN sorts last
            (true, false) => std::cmp::Ordering::Less,    // NaN sorts last
            (true, true) => other
                .distance
                .partial_cmp(&self.distance)
                .unwrap_or(std::cmp::Ordering::Equal),
        }
    }
}

/// HNSW layer: maps node_id -> set of neighbor node_ids
type Layer = HashMap<String, HashSet<String>>;

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
    /// Create a new empty vector store
    pub fn new(dimension: usize) -> Self {
        Self {
            entries: HashMap::new(),
            layers: vec![Layer::new()],
            entry_point: None,
            dimension,
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

        let entry_point = self.entry_point.clone().unwrap();

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
    /// Uses a hash of the current entry count for deterministic but
    /// well-distributed layer assignment.
    fn assign_layer(&self, node_id: &str) -> usize {
        // Use FNV-1a hash of node_id for deterministic randomness
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in node_id.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        // Mix with entry count to avoid pathological cases
        hash ^= self.entries.len() as u64;
        hash = hash
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        // Map to [0, 1)
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

// ============================================================
// Vector Retriever
// ============================================================

/// Retriever that uses vector similarity search
pub struct VectorRetriever {
    graph: KnowledgeGraph,
    vector_store: VectorStore,
    #[allow(dead_code)]
    embedding_engine: Arc<dyn EmbeddingEngine>,
}

impl VectorRetriever {
    /// Create a new vector retriever, building the index from the graph
    pub async fn new(
        graph: KnowledgeGraph,
        embedding_engine: Arc<dyn EmbeddingEngine>,
    ) -> KiasResult<Self> {
        let dimension = embedding_engine.dimension();
        let mut vector_store = VectorStore::new(dimension);

        let nodes = graph.get_all_nodes();
        let texts: Vec<&str> = nodes.iter().map(|n| n.content.as_str()).collect();
        let embeddings = embedding_engine.embed_batch(&texts).await?;

        for (node, embedding) in nodes.iter().zip(embeddings.iter()) {
            vector_store.insert(node.id.clone(), embedding.clone());
        }

        info!(
            node_count = nodes.len(),
            dimension = dimension,
            layers = vector_store.layer_count(),
            "Built vector index"
        );

        Ok(Self {
            graph,
            vector_store,
            embedding_engine,
        })
    }

    /// Get the underlying vector store
    pub fn vector_store(&self) -> &VectorStore {
        &self.vector_store
    }

    /// Search by embedding vector directly
    pub fn search_by_vector(&self, query_vector: &[f32], limit: usize) -> Vec<ScoredNode> {
        let results = self.vector_store.search_knn(query_vector, limit);
        results
            .into_iter()
            .filter_map(|(node_id, distance)| {
                self.graph.get_node(&node_id).map(|node| ScoredNode {
                    node: node.clone(),
                    score: f64::from(1.0 - distance),
                    match_type: MatchType::ContentMatch,
                })
            })
            .collect()
    }
}

#[async_trait]
impl Retriever for VectorRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> KiasResult<Vec<ScoredNode>> {
        debug!(query = %query, limit = limit, "Vector retrieval");

        let query_embedding = self.embedding_engine.embed(query).await?;
        let results = self.vector_store.search_knn(&query_embedding, limit);

        Ok(results
            .into_iter()
            .filter_map(|(node_id, distance)| {
                self.graph.get_node(&node_id).map(|node| {
                    let similarity = 1.0 - distance;
                    ScoredNode {
                        node: node.clone(),
                        score: f64::from(similarity),
                        match_type: MatchType::ContentMatch,
                    }
                })
            })
            .collect())
    }

    async fn retrieve_by_type(
        &self,
        query: &str,
        node_type: NodeType,
        limit: usize,
    ) -> KiasResult<Vec<ScoredNode>> {
        let query_embedding = self.embedding_engine.embed(query).await?;
        let results = self.vector_store.search_knn(&query_embedding, limit * 3);

        Ok(results
            .into_iter()
            .filter_map(|(node_id, distance)| {
                self.graph.get_node(&node_id).and_then(|node| {
                    if node.node_type == node_type {
                        Some(ScoredNode {
                            node: node.clone(),
                            score: f64::from(1.0 - distance),
                            match_type: MatchType::ContentMatch,
                        })
                    } else {
                        None
                    }
                })
            })
            .take(limit)
            .collect())
    }
}

// ============================================================
// Utility Functions
// ============================================================

/// Compute cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, KnowledgeNode};

    // ===== Utility function tests =====

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

    // ===== LocalEmbeddingEngine tests =====

    #[tokio::test]
    async fn test_embedding_engine_deterministic() {
        let engine = LocalEmbeddingEngine::new(64);
        let e1 = engine.embed("Rust programming language").await.unwrap();
        let e2 = engine.embed("Rust programming language").await.unwrap();
        assert_eq!(e1, e2, "Embeddings must be deterministic");
    }

    #[tokio::test]
    async fn test_embedding_engine_dimension() {
        let engine = LocalEmbeddingEngine::new(128);
        let embedding = engine.embed("test text").await.unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[tokio::test]
    async fn test_embedding_engine_normalized() {
        let engine = LocalEmbeddingEngine::new(64);
        let embedding = engine.embed("some text for testing").await.unwrap();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Embedding should be L2-normalized, got norm={}",
            norm,
        );
    }

    #[tokio::test]
    async fn test_embedding_similar_texts_higher_similarity() {
        let engine = LocalEmbeddingEngine::new(64);

        let e_rust1 = engine
            .embed("Rust systems programming language")
            .await
            .unwrap();
        let e_rust2 = engine
            .embed("Rust is a systems language for performance")
            .await
            .unwrap();
        let e_python = engine
            .embed("Python interpreted scripting language")
            .await
            .unwrap();

        let sim_same = cosine_similarity(&e_rust1, &e_rust2);
        let sim_diff = cosine_similarity(&e_rust1, &e_python);

        assert!(
            sim_same > sim_diff,
            "Same-topic embeddings should be more similar: sim_same={}, sim_diff={}",
            sim_same,
            sim_diff,
        );
    }

    #[tokio::test]
    async fn test_embedding_batch() {
        let engine = LocalEmbeddingEngine::new(32);
        let texts = vec!["hello", "world", "rust"];
        let embeddings = engine.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 32);
        }
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
        // Use higher dimensions and fewer vectors for more distinct clusters
        let dim = 32;
        let mut store = VectorStore::new(dim);

        let mut seed: u64 = 12345;
        for i in 0..50 {
            let mut v = vec![0.0f32; dim];
            // Create semi-structured vectors (not fully random)
            // Each vector has a dominant dimension
            v[i % dim] = 5.0; // Strong signal
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

        // Query strongly aligned with dimension 0
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

    // ===== VectorRetriever tests =====

    fn build_test_graph() -> KnowledgeGraph {
        let mut graph = KnowledgeGraph::new();

        graph.add_node(KnowledgeNode {
            id: "n1".to_string(),
            content: "Rust is a systems programming language focused on safety and performance"
                .to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::from([("topic".to_string(), "programming".to_string())]),
        });
        graph.add_node(KnowledgeNode {
            id: "n2".to_string(),
            content: "Python is an interpreted high-level programming language".to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::new(),
        });
        graph.add_node(KnowledgeNode {
            id: "n3".to_string(),
            content: "The borrow checker ensures memory safety in Rust without garbage collection"
                .to_string(),
            node_type: NodeType::Concept,
            metadata: HashMap::new(),
        });
        graph.add_node(KnowledgeNode {
            id: "n4".to_string(),
            content: "Kubernetes orchestrates containerized applications across clusters"
                .to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::new(),
        });
        graph.add_node(KnowledgeNode {
            id: "n5".to_string(),
            content: "Rust uses ownership and borrowing for memory management".to_string(),
            node_type: NodeType::Concept,
            metadata: HashMap::new(),
        });

        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n3".to_string(),
            relationship: "has_concept".to_string(),
            weight: 0.9,
        });
        graph.add_edge(Edge {
            from: "n1".to_string(),
            to: "n5".to_string(),
            relationship: "has_concept".to_string(),
            weight: 0.8,
        });

        graph
    }

    #[tokio::test]
    async fn test_vector_retriever_basic() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let results = retriever.retrieve("Rust programming", 5).await.unwrap();
        assert!(!results.is_empty());
        let has_rust = results
            .iter()
            .any(|r| r.node.content.to_lowercase().contains("rust"));
        assert!(has_rust, "Should find Rust-related content");
    }

    #[tokio::test]
    async fn test_vector_retriever_by_type() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let results = retriever
            .retrieve_by_type("memory safety", NodeType::Concept, 5)
            .await
            .unwrap();

        for result in &results {
            assert_eq!(result.node.node_type, NodeType::Concept);
        }
    }

    #[tokio::test]
    async fn test_vector_retriever_empty_query() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let results = retriever.retrieve("", 5).await.unwrap();
        for r in &results {
            assert!(r.score >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_vector_retriever_ranking() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let results = retriever
            .retrieve("Rust borrow checker memory", 5)
            .await
            .unwrap();
        assert!(!results.is_empty());

        for i in 1..results.len() {
            assert!(
                results[i - 1].score >= results[i].score,
                "Results should be sorted by score descending"
            );
        }
    }

    #[tokio::test]
    async fn test_vector_retriever_limit() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let results = retriever.retrieve("programming", 2).await.unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn test_vector_retriever_index_stats() {
        let graph = build_test_graph();
        let engine = Arc::new(LocalEmbeddingEngine::new(64));
        let retriever = VectorRetriever::new(graph, engine).await.unwrap();

        let stats = retriever.vector_store().stats();
        assert_eq!(stats.total_vectors, 5);
        assert_eq!(stats.dimension, 64);
    }
}
