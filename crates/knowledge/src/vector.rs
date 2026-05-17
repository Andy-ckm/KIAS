//! # Vector Embedding Retrieval
//!
//! Provides semantic similarity search over knowledge nodes using vector embeddings.
//! Uses the HNSW index from `kias-common::vector` and adds embedding engines
//! and graph-aware retrieval.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌───────────────┐
//! │  EmbeddingEngine │────▶│   VectorStore     │────▶│ VectorRetriever│
//! │  (text → vec)    │     │  (HNSW index)    │     │ (Retriever trait│
//! └─────────────────┘     └──────────────────┘     └───────────────┘
//! ```

use async_trait::async_trait;
use kias_common::KiasResult;
use std::sync::Arc;
use tracing::{debug, info};

use super::graph::{KnowledgeGraph, NodeType};
use super::retriever::{MatchType, Retriever, ScoredNode};

// Re-export vector types from common (canonical location)
pub use kias_common::vector::{
    cosine_distance, cosine_similarity, hnsw_ml, l2_distance, HnswSnapshot, LayerStats,
    VectorEntry, VectorStore, VectorStoreStats, DEFAULT_EMBEDDING_DIM, HNSW_EF_CONSTRUCTION,
    HNSW_EF_SEARCH, HNSW_M, HNSW_M_MAX,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, KnowledgeNode};
    use std::collections::HashMap;

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
