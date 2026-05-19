//! # Hybrid Knowledge Retriever
//!
//! Combines graph-based retrieval with text-based TF-IDF scoring.
//! Inspired by RAG (Retrieval Augmented Generation) patterns.

use super::graph::{KnowledgeGraph, KnowledgeNode, NodeType};
use async_trait::async_trait;
use kias_common::KiasResult;
use std::collections::HashMap;

/// Trait for knowledge retrieval strategies
#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, limit: usize) -> KiasResult<Vec<ScoredNode>>;
    async fn retrieve_by_type(
        &self,
        query: &str,
        node_type: NodeType,
        limit: usize,
    ) -> KiasResult<Vec<ScoredNode>>;
}

/// A knowledge node with a relevance score
#[derive(Debug, Clone)]
pub struct ScoredNode {
    pub node: KnowledgeNode,
    pub score: f64,
    pub match_type: MatchType,
}

/// How the match was found
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchType {
    /// Direct text match in content
    ContentMatch,
    /// Match via graph relationship (neighbor of a content match)
    GraphExpansion,
    /// Match via metadata
    MetadataMatch,
    /// Match via tags
    TagMatch,
}

/// Hybrid Retriever combining TF-IDF text search with graph traversal.
///
/// Algorithm:
/// 1. Score all nodes using TF-IDF (term frequency * inverse document frequency)
/// 2. Expand results via graph relationships (neighbors of top-scoring nodes)
/// 3. Merge and re-rank results
pub struct HybridRetriever {
    graph: KnowledgeGraph,
    /// Pre-computed IDF values for terms across the corpus
    idf_cache: HashMap<String, f64>,
    #[allow(dead_code)]
    /// Total number of documents in the corpus
    doc_count: usize,
}

impl HybridRetriever {
    pub fn new(graph: KnowledgeGraph) -> Self {
        let doc_count = graph.node_count();
        let idf_cache = Self::compute_idf(&graph);
        Self {
            graph,
            idf_cache,
            doc_count,
        }
    }

    /// Compute Inverse Document Frequency for all terms in the corpus
    fn compute_idf(graph: &KnowledgeGraph) -> HashMap<String, f64> {
        let mut term_doc_count: HashMap<String, usize> = HashMap::new();
        let total_docs = graph.node_count().max(1) as f64;

        for node in graph.get_all_nodes() {
            let terms = Self::tokenize(&node.content);
            let unique_terms: std::collections::HashSet<String> = terms.into_iter().collect();
            for term in unique_terms {
                *term_doc_count.entry(term).or_insert(0) += 1;
            }
        }

        term_doc_count
            .into_iter()
            .map(|(term, count)| {
                let idf = (total_docs / count as f64).ln() + 1.0;
                (term, idf)
            })
            .collect()
    }

    /// Tokenize text into lowercase terms, removing common English stop words.
    /// Technical terms (rust, python, kubernetes, etc.) are preserved.
    fn tokenize(text: &str) -> Vec<String> {
        let stop_words: std::collections::HashSet<&str> = [
            "a", "an", "the", "is", "it", "to", "of", "and", "or", "in", "for", "on", "with", "at",
            "by", "from", "as", "into", "this", "that", "are", "was", "were", "be", "been",
            "being", "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
            "may", "might", "can", "not", "no", "but", "if", "then", "than", "so", "just", "about",
            "up", "out", "all", "its", "my", "your", "his", "her", "our", "their", "i", "we",
            "you", "he", "she", "they", "me", "him", "us", "them",
        ]
        .iter()
        .cloned()
        .collect();

        text.split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty() && !stop_words.contains(w.as_str()))
            .collect()
    }

    /// Calculate TF (Term Frequency) for a query in a document
    fn term_frequency(query_terms: &[String], doc_content: &str) -> f64 {
        let doc_terms = Self::tokenize(doc_content);
        let doc_len = doc_terms.len().max(1) as f64;

        let mut count = 0usize;
        for qt in query_terms {
            for dt in &doc_terms {
                if dt == qt || dt.contains(qt.as_str()) || qt.contains(dt.as_str()) {
                    count += 1;
                }
            }
        }
        count as f64 / doc_len
    }

    /// Calculate TF-IDF score for a query against a document
    fn tfidf_score(&self, query_terms: &[String], node: &KnowledgeNode) -> f64 {
        let tf = Self::term_frequency(query_terms, &node.content);

        let idf_sum: f64 = query_terms
            .iter()
            .map(|t| self.idf_cache.get(t).copied().unwrap_or(1.0))
            .sum();

        let base_score = tf.max(0.0) * idf_sum;

        // Boost for exact substring match
        let query_lower = query_terms.join(" ");
        let content_lower = node.content.to_lowercase();
        let exact_boost = if content_lower.contains(&query_lower) {
            2.0
        } else {
            1.0
        };

        // Boost for metadata/tag matches
        let metadata_boost = if node
            .metadata
            .values()
            .any(|v| v.to_lowercase().contains(&query_lower))
        {
            1.5
        } else {
            1.0
        };

        base_score * exact_boost * metadata_boost
    }

    /// Expand results via graph relationships
    fn graph_expand(&self, top_nodes: &[ScoredNode], limit: usize) -> Vec<ScoredNode> {
        let mut expanded = Vec::new();
        let seen_ids: std::collections::HashSet<String> =
            top_nodes.iter().map(|sn| sn.node.id.clone()).collect();

        for scored in top_nodes.iter().take(3) {
            let neighbors = self.graph.get_neighbors(&scored.node.id);
            for neighbor in neighbors {
                if !seen_ids.contains(&neighbor.id) {
                    expanded.push(ScoredNode {
                        node: neighbor.clone(),
                        score: scored.score * 0.5, // Decay score for graph expansion
                        match_type: MatchType::GraphExpansion,
                    });
                }
            }
        }

        expanded.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        expanded.truncate(limit);
        expanded
    }
}

#[async_trait]
impl Retriever for HybridRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> KiasResult<Vec<ScoredNode>> {
        tracing::info!(query = %query, limit = limit, "Retrieving knowledge");

        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: TF-IDF scoring across all nodes
        let mut scored: Vec<ScoredNode> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter_map(|node| {
                let score = self.tfidf_score(&query_terms, node);
                if score > 0.0 {
                    Some(ScoredNode {
                        node: node.clone(),
                        score,
                        match_type: MatchType::ContentMatch,
                    })
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 2: Graph expansion from top results
        let top_limit = (limit / 2).max(1);
        let graph_expanded = self.graph_expand(&scored, limit.saturating_sub(top_limit));

        // Step 3: Merge results
        scored.truncate(top_limit);
        scored.extend(graph_expanded);
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);

        Ok(scored)
    }

    async fn retrieve_by_type(
        &self,
        query: &str,
        node_type: NodeType,
        limit: usize,
    ) -> KiasResult<Vec<ScoredNode>> {
        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut scored: Vec<ScoredNode> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter(|n| n.node_type == node_type)
            .filter_map(|node| {
                let score = self.tfidf_score(&query_terms, node);
                if score > 0.0 {
                    Some(ScoredNode {
                        node: node.clone(),
                        score,
                        match_type: MatchType::ContentMatch,
                    })
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }
}

/// Simple keyword-based retriever (lighter weight alternative)
pub struct KeywordRetriever {
    graph: KnowledgeGraph,
}

impl KeywordRetriever {
    pub fn new(graph: KnowledgeGraph) -> Self {
        Self { graph }
    }
}

#[async_trait]
impl Retriever for KeywordRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> KiasResult<Vec<ScoredNode>> {
        let query_lower = query.to_lowercase();
        let results: Vec<ScoredNode> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter(|n| n.content.to_lowercase().contains(&query_lower))
            .map(|n| ScoredNode {
                node: n.clone(),
                score: 1.0,
                match_type: MatchType::ContentMatch,
            })
            .take(limit)
            .collect();
        Ok(results)
    }

    async fn retrieve_by_type(
        &self,
        query: &str,
        node_type: NodeType,
        limit: usize,
    ) -> KiasResult<Vec<ScoredNode>> {
        let query_lower = query.to_lowercase();
        let results: Vec<ScoredNode> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter(|n| n.node_type == node_type && n.content.to_lowercase().contains(&query_lower))
            .map(|n| ScoredNode {
                node: n.clone(),
                score: 1.0,
                match_type: MatchType::ContentMatch,
            })
            .take(limit)
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, NodeType};

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

        // Graph relationships: n1 (Rust) -> n3 (borrow checker), n1 -> n5 (ownership)
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
        graph.add_edge(Edge {
            from: "n3".to_string(),
            to: "n5".to_string(),
            relationship: "related_to".to_string(),
            weight: 0.7,
        });

        graph
    }

    #[tokio::test]
    async fn test_retriever_basic_search() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever.retrieve("Rust programming", 5).await.unwrap();
        assert!(!results.is_empty());
        // Rust-related documents should rank highest
        assert!(
            results[0].node.content.contains("Rust") || results[0].node.content.contains("rust")
        );
    }

    #[tokio::test]
    async fn test_retriever_no_results() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever
            .retrieve("blockchain cryptocurrency NFT", 5)
            .await
            .unwrap();
        // Should return empty or very low scoring results
        assert!(results.is_empty() || results[0].score < 0.01);
    }

    #[tokio::test]
    async fn test_retriever_graph_expansion() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        // Search for "Rust" should also return graph neighbors (borrow checker, ownership)
        let results = retriever.retrieve("Rust safety", 10).await.unwrap();
        let ids: Vec<String> = results.iter().map(|r| r.node.id.clone()).collect();
        // Should include n1 (direct match) and n3/n5 (graph expansion)
        assert!(ids.contains(&"n1".to_string()));
    }

    #[tokio::test]
    async fn test_retriever_by_type() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever
            .retrieve_by_type("Rust", NodeType::Concept, 5)
            .await
            .unwrap();
        for result in &results {
            assert_eq!(result.node.node_type, NodeType::Concept);
        }
    }

    #[tokio::test]
    async fn test_retriever_limit() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever.retrieve("programming", 2).await.unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn test_retriever_empty_query() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever.retrieve("", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_keyword_retriever() {
        let graph = build_test_graph();
        let retriever = KeywordRetriever::new(graph);

        let results = retriever.retrieve("Rust", 5).await.unwrap();
        assert!(!results.is_empty());
        for result in &results {
            assert!(result.node.content.to_lowercase().contains("rust"));
        }
    }

    #[tokio::test]
    async fn test_tokenize() {
        let terms = HybridRetriever::tokenize("The Rust programming language is great");
        // "The" and "is" should be removed as stop words
        assert!(terms.contains(&"rust".to_string()));
        assert!(terms.contains(&"programming".to_string()));
        assert!(terms.contains(&"language".to_string()));
        assert!(terms.contains(&"great".to_string()));
        assert!(!terms.contains(&"the".to_string()));
        assert!(!terms.contains(&"is".to_string()));
    }

    #[tokio::test]
    async fn test_scored_node_ordering() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever.retrieve("memory safety Rust", 10).await.unwrap();
        // Results should be sorted by score (descending)
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[tokio::test]
    async fn test_metadata_boost() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);

        let results = retriever.retrieve("programming", 10).await.unwrap();
        // n1 has metadata "topic: programming" so should get a boost
        if !results.is_empty() {
            let n1_result = results.iter().find(|r| r.node.id == "n1");
            if let Some(n1) = n1_result {
                assert!(n1.score > 0.0);
            }
        }
    }
    #[tokio::test]
    async fn test_keyword_retriever_by_type() {
        let graph = build_test_graph();
        let retriever = KeywordRetriever::new(graph);
        let results = retriever
            .retrieve_by_type("Rust", NodeType::Concept, 5)
            .await
            .unwrap();
        for r in &results {
            assert_eq!(r.node.node_type, NodeType::Concept);
            assert!(r.node.content.to_lowercase().contains("rust"));
        }
    }

    #[tokio::test]
    async fn test_keyword_retriever_empty_query() {
        let graph = build_test_graph();
        let retriever = KeywordRetriever::new(graph);
        let results = retriever.retrieve("", 5).await.unwrap();
        // Empty string matches everything (contains check)
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_keyword_retriever_no_match() {
        let graph = build_test_graph();
        let retriever = KeywordRetriever::new(graph);
        let results = retriever.retrieve("quantum entanglement", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_keyword_retriever_by_type_empty_query() {
        let graph = build_test_graph();
        let retriever = KeywordRetriever::new(graph);
        let results = retriever
            .retrieve_by_type("", NodeType::Document, 5)
            .await
            .unwrap();
        // Empty string matches all documents
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_empty_graph() {
        let graph = KnowledgeGraph::new();
        let retriever = HybridRetriever::new(graph);
        let results = retriever.retrieve("anything", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_by_type_empty_query() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);
        let results = retriever
            .retrieve_by_type("", NodeType::Document, 5)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_by_type_no_match() {
        let graph = build_test_graph();
        let retriever = HybridRetriever::new(graph);
        let results = retriever
            .retrieve_by_type("blockchain", NodeType::Document, 5)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_tokenize_special_chars() {
        let terms = HybridRetriever::tokenize("hello-world foo_bar baz!");
        assert!(terms.contains(&"hello-world".to_string()));
        assert!(terms.contains(&"foo_bar".to_string()));
        // "baz!" - the ! is stripped, leaving "baz"
        assert!(terms.contains(&"baz".to_string()));
    }

    #[test]
    fn test_tokenize_empty() {
        let terms = HybridRetriever::tokenize("");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_tokenize_all_stop_words() {
        let terms = HybridRetriever::tokenize("the is a an");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_term_frequency() {
        let query = vec!["rust".to_string(), "memory".to_string()];
        let tf = HybridRetriever::term_frequency(&query, "Rust uses memory safety via ownership");
        assert!(tf > 0.0);
    }

    #[test]
    fn test_term_frequency_no_match() {
        let query = vec!["quantum".to_string()];
        let tf = HybridRetriever::term_frequency(&query, "Rust programming language");
        assert_eq!(tf, 0.0);
    }

    #[test]
    fn test_match_type_equality() {
        assert_eq!(MatchType::ContentMatch, MatchType::ContentMatch);
        assert_ne!(MatchType::ContentMatch, MatchType::GraphExpansion);
        assert_ne!(MatchType::MetadataMatch, MatchType::TagMatch);
    }
}
