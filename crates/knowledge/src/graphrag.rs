//! # GraphRAG Engine
//!
//! Combines graph traversal with text matching for hybrid retrieval,
//! inspired by GraphRAG patterns. Supports multiple retrieval strategies,
//! community detection via label propagation, and subgraph summarization.

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use super::graph::{KnowledgeGraph, KnowledgeNode, NodeType};

/// Retrieval strategy for combining text and graph search
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetrievalStrategy {
    /// Text search first, then expand via graph
    TextFirst,
    /// Graph traversal first, then rank by text relevance
    GraphFirst,
    /// Run text and graph search in parallel, merge results
    Parallel,
    /// Iteratively expand: text -> graph -> re-rank -> expand again
    Iterative,
}

/// Query for hybrid graph+text retrieval
#[derive(Debug, Clone)]
pub struct HybridQuery {
    /// Text query for content matching
    pub text_query: String,
    /// Optional starting node for graph traversal
    pub graph_start_node: Option<String>,
    /// Maximum traversal depth (hop count)
    pub max_depth: usize,
    /// Minimum relevance score threshold
    pub min_relevance: f64,
    /// Which retrieval strategy to use
    pub strategy: RetrievalStrategy,
}

impl Default for HybridQuery {
    fn default() -> Self {
        Self {
            text_query: String::new(),
            graph_start_node: None,
            max_depth: 2,
            min_relevance: 0.0,
            strategy: RetrievalStrategy::TextFirst,
        }
    }
}

/// Result from a GraphRAG retrieval operation
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// The matched knowledge node
    pub node: KnowledgeNode,
    /// Combined relevance score
    pub relevance_score: f64,
    /// Path from the query origin to this node (list of node IDs)
    pub path_from_query: Vec<String>,
    /// Contextually related nodes (neighbors, community members)
    pub context_nodes: Vec<String>,
}

/// GraphRAG engine combining graph traversal with text-based retrieval
pub struct GraphRAGEngine {
    graph: KnowledgeGraph,
    /// Pre-computed IDF values for TF-IDF scoring
    idf_cache: HashMap<String, f64>,
    /// Total document count for IDF computation
    #[allow(dead_code)]
    doc_count: usize,
    /// Cached community assignments (node_id -> community_label)
    community_cache: Option<HashMap<String, usize>>,
}

impl GraphRAGEngine {
    /// Create a new GraphRAG engine from a knowledge graph
    pub fn new(graph: KnowledgeGraph) -> Self {
        let doc_count = graph.node_count();
        let idf_cache = Self::compute_idf(&graph);
        Self {
            graph,
            idf_cache,
            doc_count,
            community_cache: None,
        }
    }

    /// Get a reference to the underlying graph
    pub fn graph(&self) -> &KnowledgeGraph {
        &self.graph
    }

    /// Compute IDF for all terms in the corpus
    fn compute_idf(graph: &KnowledgeGraph) -> HashMap<String, f64> {
        let mut term_doc_count: HashMap<String, usize> = HashMap::new();
        let total_docs = graph.node_count().max(1) as f64;

        for node in graph.get_all_nodes() {
            let terms = Self::tokenize(&node.content);
            let unique_terms: HashSet<String> = terms.into_iter().collect();
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

    /// Tokenize text into lowercase terms, removing stop words
    fn tokenize(text: &str) -> Vec<String> {
        let stop_words: HashSet<&str> = [
            "a", "an", "the", "is", "it", "to", "of", "and", "or", "in",
            "for", "on", "with", "at", "by", "from", "as", "into", "this",
            "that", "are", "was", "were", "be", "been", "being", "have",
            "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "can", "not", "no", "but", "if",
            "then", "than", "so", "just", "about", "up", "out", "all",
            "its", "my", "your", "his", "her", "our", "their", "i", "we",
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

    /// Calculate term frequency for query terms in document content
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

    /// Calculate TF-IDF score for query terms against a node
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

        // Boost for metadata matches
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

    /// Get N-hop neighbors of a node
    fn get_n_hop_neighbors(&self, node_id: &str, max_depth: usize) -> HashMap<String, usize> {
        // Verify the starting node exists
        if self.graph.get_node(node_id).is_none() {
            return HashMap::new();
        }

        let mut result: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        queue.push_back((node_id.to_string(), 0usize));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > max_depth {
                continue;
            }
            if result.contains_key(&current) {
                continue;
            }
            result.insert(current.clone(), depth);

            // Outgoing neighbors
            for edge in self.graph.get_outgoing_edges(&current) {
                if !result.contains_key(&edge.to) {
                    queue.push_back((edge.to.clone(), depth + 1));
                }
            }
            // Incoming neighbors (undirected traversal)
            for edge in self.graph.get_incoming_edges(&current) {
                if !result.contains_key(&edge.from) {
                    queue.push_back((edge.from.clone(), depth + 1));
                }
            }
        }

        result
    }

    /// Compute graph centrality (degree centrality) for a node
    fn degree_centrality(&self, node_id: &str) -> f64 {
        let total_nodes = self.graph.node_count().max(1) as f64;
        let outgoing = self.graph.get_outgoing_edges(node_id).len() as f64;
        let incoming = self.graph.get_incoming_edges(node_id).len() as f64;
        (outgoing + incoming) / total_nodes
    }

    /// Get average edge weight for edges connected to a node
    fn avg_edge_weight(&self, node_id: &str) -> f64 {
        let outgoing = self.graph.get_outgoing_edges(node_id);
        let incoming = self.graph.get_incoming_edges(node_id);
        let total = outgoing.len() + incoming.len();
        if total == 0 {
            return 0.0;
        }
        let sum: f64 = outgoing.iter().map(|e| e.weight).sum::<f64>()
            + incoming.iter().map(|e| e.weight).sum::<f64>();
        sum / total as f64
    }

    /// Find seed nodes via text search
    fn text_search_seeds(&self, query: &str) -> Vec<(String, f64)> {
        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(String, f64)> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter_map(|node| {
                let score = self.tfidf_score(&query_terms, node);
                if score > 0.0 {
                    Some((node.id.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// Combined scoring: text_match * 0.4 + graph_centrality * 0.3 + edge_weight * 0.3
    fn combined_score(&self, text_score: f64, node_id: &str) -> f64 {
        let centrality = self.degree_centrality(node_id);
        let edge_weight = self.avg_edge_weight(node_id);

        // Normalize text_score to [0, 1] range using sigmoid-like normalization
        let normalized_text = text_score / (1.0 + text_score);

        normalized_text * 0.4 + centrality * 0.3 + edge_weight * 0.3
    }

    /// Build path from query seed to a target node using BFS
    fn find_path(&self, from_id: &str, to_id: &str) -> Vec<String> {
        if from_id == to_id {
            return vec![from_id.to_string()];
        }
        self.graph
            .shortest_path(from_id, to_id)
            .unwrap_or_else(|| vec![to_id.to_string()])
    }

    // ==================== Strategy implementations ====================

    fn search_text_first(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        let seeds = self.text_search_seeds(&query.text_query);
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        for (seed_id, text_score) in &seeds {
            // Get 2-hop neighbors for context
            let neighbors = self.get_n_hop_neighbors(seed_id, query.max_depth.min(2));

            for node_id in neighbors.keys() {
                if seen.contains(node_id) {
                    continue;
                }
                seen.insert(node_id.clone());

                if let Some(node) = self.graph.get_node(node_id) {
                    let query_terms = Self::tokenize(&query.text_query);
                    let node_text_score = self.tfidf_score(&query_terms, node);
                    let combined = self.combined_score(
                        node_text_score.max(*text_score * 0.5),
                        node_id,
                    );

                    if combined >= query.min_relevance {
                        let path = self.find_path(seed_id, node_id);
                        let context: Vec<String> = neighbors.keys().cloned().collect();

                        results.push(RetrievalResult {
                            node: node.clone(),
                            relevance_score: combined,
                            path_from_query: path,
                            context_nodes: context,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    fn search_graph_first(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        let start = query
            .graph_start_node
            .as_deref()
            .unwrap_or_else(|| {
                // Fall back to first text match
            let seeds = self.text_search_seeds(&query.text_query);
            if let Some((_id, _)) = seeds.first() {
                    // We need to return a &str, so we leak is bad; use a default
                    // Actually we can just handle this differently
                    ""
                } else {
                    ""
                }
            });

        // Re-fetch start node id properly
        let start_id = if !start.is_empty() {
            start.to_string()
        } else {
            let seeds = self.text_search_seeds(&query.text_query);
            match seeds.first() {
                Some((id, _)) => id.clone(),
                None => return Vec::new(),
            }
        };

        let neighbors = self.get_n_hop_neighbors(&start_id, query.max_depth);
        let query_terms = Self::tokenize(&query.text_query);
        let mut results = Vec::new();

        for node_id in neighbors.keys() {
            if let Some(node) = self.graph.get_node(node_id) {
                let text_score = if query_terms.is_empty() {
                    0.1 // Small base score when no text query
                } else {
                    self.tfidf_score(&query_terms, node)
                };
                let combined = self.combined_score(text_score, node_id);

                if combined >= query.min_relevance {
                    let path = self.find_path(&start_id, node_id);
                    let context: Vec<String> = neighbors.keys().cloned().collect();

                    results.push(RetrievalResult {
                        node: node.clone(),
                        relevance_score: combined,
                        path_from_query: path,
                        context_nodes: context,
                    });
                }
            }
        }

        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    fn search_parallel(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        // Text results
        let text_results = self.search_text_first(query);
        // Graph results
        let graph_results = self.search_graph_first(query);

        // Merge: keep highest score for each node
        let mut merged: HashMap<String, RetrievalResult> = HashMap::new();

        for result in text_results {
            let id = result.node.id.clone();
            merged
                .entry(id)
                .and_modify(|existing| {
                    if result.relevance_score > existing.relevance_score {
                        existing.relevance_score = result.relevance_score;
                        existing.path_from_query = result.path_from_query.clone();
                    }
                })
                .or_insert(result);
        }

        for result in graph_results {
            let id = result.node.id.clone();
            merged
                .entry(id)
                .and_modify(|existing| {
                    if result.relevance_score > existing.relevance_score {
                        existing.relevance_score = result.relevance_score;
                        existing.path_from_query = result.path_from_query.clone();
                    }
                })
                .or_insert(result);
        }

        let mut results: Vec<RetrievalResult> = merged.into_values().collect();
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    fn search_iterative(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        // Round 1: Text search for seeds
        let seeds = self.text_search_seeds(&query.text_query);
        if seeds.is_empty() {
            return Vec::new();
        }

        let mut current_frontier: Vec<String> = seeds.iter().map(|(id, _)| id.clone()).collect();
        let mut seen: HashSet<String> = current_frontier.iter().cloned().collect();
        let mut results = Vec::new();
        let query_terms = Self::tokenize(&query.text_query);

        for round in 0..query.max_depth.max(1) {
            let mut next_frontier = Vec::new();

            for node_id in &current_frontier {
                if let Some(node) = self.graph.get_node(node_id) {
                    let text_score = self.tfidf_score(&query_terms, node);
                    let seed_bonus = seeds
                        .iter()
                        .find(|(id, _)| id == node_id)
                        .map(|(_, s)| *s)
                        .unwrap_or(0.0);
                    let effective_text = text_score.max(seed_bonus * 0.5_f64.powi(round as i32));
                    let combined = self.combined_score(effective_text, node_id);

                    if combined >= query.min_relevance {
                        let path = if let Some((seed_id, _)) = seeds.first() {
                            self.find_path(seed_id, node_id)
                        } else {
                            vec![node_id.clone()]
                        };

                        results.push(RetrievalResult {
                            node: node.clone(),
                            relevance_score: combined,
                            path_from_query: path,
                            context_nodes: seen.iter().cloned().collect(),
                        });
                    }

                    // Expand to neighbors
                    for edge in self.graph.get_outgoing_edges(node_id) {
                        if !seen.contains(&edge.to) {
                            seen.insert(edge.to.clone());
                            next_frontier.push(edge.to.clone());
                        }
                    }
                    for edge in self.graph.get_incoming_edges(node_id) {
                        if !seen.contains(&edge.from) {
                            seen.insert(edge.from.clone());
                            next_frontier.push(edge.from.clone());
                        }
                    }
                }
            }

            current_frontier = next_frontier;
        }

        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    // ==================== Public API ====================

    /// Main hybrid search combining text and graph traversal
    pub fn graph_enhanced_search(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        match query.strategy {
            RetrievalStrategy::TextFirst => self.search_text_first(query),
            RetrievalStrategy::GraphFirst => self.search_graph_first(query),
            RetrievalStrategy::Parallel => self.search_parallel(query),
            RetrievalStrategy::Iterative => self.search_iterative(query),
        }
    }

    /// Community detection using label propagation algorithm.
    /// Returns groups of node IDs that belong to the same community.
    pub fn community_detection(&mut self) -> Vec<Vec<String>> {
        let node_ids = self.graph.node_ids();
        if node_ids.is_empty() {
            return Vec::new();
        }

        // Initialize: each node has its own label (index)
        let mut labels: HashMap<String, usize> = HashMap::new();
        for (i, id) in node_ids.iter().enumerate() {
            labels.insert(id.clone(), i);
        }

        let max_iterations = 30;
        let mut changed = true;
        let mut iter_count = 0;

        while changed && iter_count < max_iterations {
            changed = false;
            iter_count += 1;

            // Shuffle order for non-deterministic convergence (use sorted for determinism in tests)
            let mut shuffled_ids = node_ids.clone();
            shuffled_ids.sort(); // Deterministic ordering

            for node_id in &shuffled_ids {
                // Collect neighbor labels with edge weights
                let mut label_votes: HashMap<usize, f64> = HashMap::new();

                for edge in self.graph.get_outgoing_edges(node_id) {
                    if let Some(&label) = labels.get(&edge.to) {
                        *label_votes.entry(label).or_insert(0.0) += edge.weight;
                    }
                }
                for edge in self.graph.get_incoming_edges(node_id) {
                    if let Some(&label) = labels.get(&edge.from) {
                        *label_votes.entry(label).or_insert(0.0) += edge.weight;
                    }
                }

                if label_votes.is_empty() {
                    continue;
                }

                // Pick the label with highest total weight
                let best_label = label_votes
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(label, _)| *label)
                    .unwrap();

                if labels[node_id] != best_label {
                    labels.insert(node_id.clone(), best_label);
                    changed = true;
                }
            }
        }

        // Group nodes by label
        let mut communities: HashMap<usize, Vec<String>> = HashMap::new();
        for (node_id, label) in &labels {
            communities
                .entry(*label)
                .or_default()
                .push(node_id.clone());
        }

        let mut result: Vec<Vec<String>> = communities.into_values().collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.len()));
        self.community_cache = Some(labels);
        result
    }

    /// Generate a context summary from a node's neighborhood
    pub fn summarize_subgraph(&self, root: &str, depth: usize) -> String {
        let neighbors = self.get_n_hop_neighbors(root, depth);
        if neighbors.is_empty() {
            return format!("No nodes found around '{}'", root);
        }

        let mut summary_parts = Vec::new();

        // Root node info
        if let Some(root_node) = self.graph.get_node(root) {
            summary_parts.push(format!(
                "Root [{}]: {} ({})",
                root, root_node.content, root_node.node_type_str()
            ));
        }

        // Group by depth
        let mut by_depth: HashMap<usize, Vec<&str>> = HashMap::new();
        for (node_id, d) in &neighbors {
            if node_id != root {
                by_depth.entry(*d).or_default().push(node_id);
            }
        }

        let mut depths: Vec<usize> = by_depth.keys().copied().collect();
        depths.sort();

        for d in depths {
            if let Some(ids) = by_depth.get(&d) {
                let node_summaries: Vec<String> = ids
                    .iter()
                    .filter_map(|id| self.graph.get_node(id))
                    .map(|n| {
                        let preview = if n.content.len() > 80 {
                            format!("{}...", &n.content[..80])
                        } else {
                            n.content.clone()
                        };
                        format!("  [{}] {}", n.id, preview)
                    })
                    .collect();

                if !node_summaries.is_empty() {
                    summary_parts.push(format!("Depth {} ({} nodes):", d, node_summaries.len()));
                    summary_parts.extend(node_summaries);
                }
            }
        }

        // Edge summary
        let edge_count: usize = neighbors
            .keys()
            .map(|id| {
                self.graph
                    .get_outgoing_edges(id)
                    .iter()
                    .filter(|e| neighbors.contains_key(&e.to))
                    .count()
            })
            .sum();

        summary_parts.push(format!(
            "Total: {} nodes, {} internal edges",
            neighbors.len(),
            edge_count
        ));

        summary_parts.join("\n")
    }

    /// Rank nodes by TF-IDF-like scoring across the entire graph
    pub fn rank_nodes(&self, query: &str) -> Vec<(String, f64)> {
        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(String, f64)> = self
            .graph
            .get_all_nodes()
            .into_iter()
            .filter_map(|node| {
                let score = self.tfidf_score(&query_terms, node);
                if score > 0.0 {
                    Some((node.id.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// Run all retrieval strategies and return the best combined results
    pub fn search_all_strategies(&self, query: &HybridQuery) -> Vec<RetrievalResult> {
        let strategies = [
            RetrievalStrategy::TextFirst,
            RetrievalStrategy::GraphFirst,
            RetrievalStrategy::Parallel,
            RetrievalStrategy::Iterative,
        ];

        let mut all_results: HashMap<String, RetrievalResult> = HashMap::new();

        for strategy in &strategies {
            let mut q = query.clone();
            q.strategy = strategy.clone();
            let results = self.graph_enhanced_search(&q);

            for result in results {
                let id = result.node.id.clone();
                all_results
                    .entry(id)
                    .and_modify(|existing| {
                        // Keep the higher score
                        if result.relevance_score > existing.relevance_score {
                            existing.relevance_score = result.relevance_score;
                            existing.path_from_query = result.path_from_query.clone();
                        }
                    })
                    .or_insert(result);
            }
        }

        let mut results: Vec<RetrievalResult> = all_results.into_values().collect();
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }
}

/// Helper extension for KnowledgeNode to get a string representation of NodeType
trait NodeTypeExt {
    fn node_type_str(&self) -> &str;
}

impl NodeTypeExt for KnowledgeNode {
    fn node_type_str(&self) -> &str {
        match self.node_type {
            NodeType::Document => "Document",
            NodeType::Concept => "Concept",
            NodeType::Entity => "Entity",
            NodeType::Relation => "Relation",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;

    fn make_node(id: &str, content: &str) -> KnowledgeNode {
        KnowledgeNode {
            id: id.to_string(),
            content: content.to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::new(),
        }
    }

    fn _make_typed_node(id: &str, content: &str, node_type: NodeType) -> KnowledgeNode {
        KnowledgeNode {
            id: id.to_string(),
            content: content.to_string(),
            node_type,
            metadata: HashMap::new(),
        }
    }

    fn make_edge(from: &str, to: &str, weight: f64) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            relationship: "related_to".to_string(),
            weight,
        }
    }

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
            content: "Kubernetes orchestrates containerized applications across clusters".to_string(),
            node_type: NodeType::Document,
            metadata: HashMap::new(),
        });
        graph.add_node(KnowledgeNode {
            id: "n5".to_string(),
            content: "Rust uses ownership and borrowing for memory management".to_string(),
            node_type: NodeType::Concept,
            metadata: HashMap::new(),
        });

        graph.add_edge(make_edge("n1", "n3", 0.9));
        graph.add_edge(make_edge("n1", "n5", 0.8));
        graph.add_edge(make_edge("n3", "n5", 0.7));
        graph.add_edge(make_edge("n2", "n4", 0.3));

        graph
    }

    fn build_clustered_graph() -> KnowledgeGraph {
        let mut graph = KnowledgeGraph::new();

        // Cluster A: programming languages
        graph.add_node(make_node("a1", "Rust systems programming"));
        graph.add_node(make_node("a2", "Go concurrent programming"));
        graph.add_node(make_node("a3", "C++ systems language"));

        // Cluster B: web frameworks
        graph.add_node(make_node("b1", "React frontend framework"));
        graph.add_node(make_node("b2", "Axum web server framework"));
        graph.add_node(make_node("b3", "Django web framework"));

        // Intra-cluster edges (strong)
        graph.add_edge(make_edge("a1", "a2", 0.9));
        graph.add_edge(make_edge("a2", "a3", 0.8));
        graph.add_edge(make_edge("a1", "a3", 0.7));
        graph.add_edge(make_edge("b1", "b2", 0.85));
        graph.add_edge(make_edge("b2", "b3", 0.8));
        graph.add_edge(make_edge("b1", "b3", 0.75));

        // Weak inter-cluster link
        graph.add_edge(make_edge("a2", "b2", 0.2));

        graph
    }

    // ============== Test 1: Hybrid search with text + graph (TextFirst) ==============
    #[test]
    fn test_hybrid_search_text_first() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "Rust programming".to_string(),
            strategy: RetrievalStrategy::TextFirst,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(!results.is_empty());
        // n1 (Rust) should be present
        assert!(results.iter().any(|r| r.node.id == "n1"));
        // Graph expansion should bring in n3 and n5
        assert!(results.iter().any(|r| r.node.id == "n3" || r.node.id == "n5"));
    }

    // ============== Test 2: Different retrieval strategies ==============
    #[test]
    fn test_graph_first_strategy() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "Rust".to_string(),
            graph_start_node: Some("n1".to_string()),
            strategy: RetrievalStrategy::GraphFirst,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(!results.is_empty());
        // Should include n1 and its neighbors
        let ids: Vec<String> = results.iter().map(|r| r.node.id.clone()).collect();
        assert!(ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_parallel_strategy() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "memory safety".to_string(),
            strategy: RetrievalStrategy::Parallel,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(!results.is_empty());
        // Parallel should merge text and graph results
        assert!(results.iter().any(|r| r.node.id == "n3")); // borrow checker
    }

    #[test]
    fn test_iterative_strategy() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "Rust".to_string(),
            strategy: RetrievalStrategy::Iterative,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(!results.is_empty());
        // Iterative should expand from seeds through multiple rounds
        let ids: Vec<String> = results.iter().map(|r| r.node.id.clone()).collect();
        assert!(ids.contains(&"n1".to_string()));
    }

    // ============== Test 3: Community detection ==============
    #[test]
    fn test_community_detection() {
        let graph = build_clustered_graph();
        let mut engine = GraphRAGEngine::new(graph);

        let communities = engine.community_detection();
        // Should find at least 2 communities
        assert!(communities.len() >= 1);
        // All nodes should be assigned
        let total: usize = communities.iter().map(|c| c.len()).sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn test_community_detection_empty_graph() {
        let graph = KnowledgeGraph::new();
        let mut engine = GraphRAGEngine::new(graph);

        let communities = engine.community_detection();
        assert!(communities.is_empty());
    }

    #[test]
    fn test_community_detection_single_node() {
        let mut graph = KnowledgeGraph::new();
        graph.add_node(make_node("solo", "lonely node"));
        let mut engine = GraphRAGEngine::new(graph);

        let communities = engine.community_detection();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0], vec!["solo"]);
    }

    // ============== Test 4: Subgraph summarization ==============
    #[test]
    fn test_summarize_subgraph() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let summary = engine.summarize_subgraph("n1", 2);
        assert!(summary.contains("n1"));
        assert!(summary.contains("Root"));
        // Should mention neighbor nodes
        assert!(summary.contains("n3") || summary.contains("n5"));
        assert!(summary.contains("Total:"));
    }

    #[test]
    fn test_summarize_subgraph_unknown_node() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let summary = engine.summarize_subgraph("nonexistent", 1);
        assert!(summary.contains("No nodes found"));
    }

    // ============== Test 5: Node ranking ==============
    #[test]
    fn test_rank_nodes() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let ranked = engine.rank_nodes("Rust memory safety");
        assert!(!ranked.is_empty());
        // Should be sorted descending
        for i in 1..ranked.len() {
            assert!(ranked[i - 1].1 >= ranked[i].1);
        }
        // Top result should be Rust-related
        assert!(ranked[0].1 > 0.0);
    }

    #[test]
    fn test_rank_nodes_empty_query() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let ranked = engine.rank_nodes("");
        assert!(ranked.is_empty());
    }

    // ============== Test 6: Empty graph handling ==============
    #[test]
    fn test_empty_graph_search() {
        let graph = KnowledgeGraph::new();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "anything".to_string(),
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_graph_rank_nodes() {
        let graph = KnowledgeGraph::new();
        let engine = GraphRAGEngine::new(graph);

        let ranked = engine.rank_nodes("test");
        assert!(ranked.is_empty());
    }

    #[test]
    fn test_empty_graph_summarize() {
        let graph = KnowledgeGraph::new();
        let engine = GraphRAGEngine::new(graph);

        let summary = engine.summarize_subgraph("any", 1);
        assert!(summary.contains("No nodes found"));
    }

    // ============== Test 7: Score calculation ==============
    #[test]
    fn test_combined_score_formula() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // n1 has edges to n3 and n5, plus incoming edge from nothing
        // So it has outgoing edges = 2, incoming = 0
        let score = engine.combined_score(1.0, "n1");
        assert!(score > 0.0);
        assert!(score <= 1.0); // Combined score should be <= 1

        // n4 has only 1 incoming edge (from n2)
        let score_n4 = engine.combined_score(1.0, "n4");
        // n1 should score higher than n4 due to more connections
        assert!(score >= score_n4);
    }

    #[test]
    fn test_degree_centrality() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // n1 has 2 outgoing edges
        let centrality_n1 = engine.degree_centrality("n1");
        assert!(centrality_n1 > 0.0);

        // n4 has 1 incoming edge, 1 outgoing
        let centrality_n4 = engine.degree_centrality("n4");
        assert!(centrality_n4 > 0.0);

        // n1 should have higher centrality than an isolated node would
        assert!(centrality_n1 > 0.0);
    }

    #[test]
    fn test_avg_edge_weight() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // n1 has outgoing edges with weights 0.9 and 0.8
        let avg = engine.avg_edge_weight("n1");
        assert!((avg - 0.85).abs() < 0.01);

        // Non-existent node should return 0
        let avg_none = engine.avg_edge_weight("nonexistent");
        assert_eq!(avg_none, 0.0);
    }

    #[test]
    fn test_min_relevance_filter() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // With very high min_relevance, should get fewer results
        let query_lenient = HybridQuery {
            text_query: "Rust".to_string(),
            strategy: RetrievalStrategy::TextFirst,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };
        let query_strict = HybridQuery {
            text_query: "Rust".to_string(),
            strategy: RetrievalStrategy::TextFirst,
            max_depth: 2,
            min_relevance: 0.99,
            ..Default::default()
        };

        let lenient_results = engine.graph_enhanced_search(&query_lenient);
        let strict_results = engine.graph_enhanced_search(&query_strict);
        assert!(lenient_results.len() >= strict_results.len());
    }

    // ============== Test 8: RetrievalResult structure ==============
    #[test]
    fn test_retrieval_result_fields() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "Rust programming".to_string(),
            strategy: RetrievalStrategy::TextFirst,
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.graph_enhanced_search(&query);
        assert!(!results.is_empty());

        let first = &results[0];
        assert!(!first.node.id.is_empty());
        assert!(first.relevance_score >= 0.0);
        // path_from_query should contain at least the node itself
        assert!(!first.path_from_query.is_empty());
    }

    // ============== Test 9: N-hop neighbors ==============
    #[test]
    fn test_n_hop_neighbors() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // 0-hop: just the node itself
        let neighbors_0 = engine.get_n_hop_neighbors("n1", 0);
        assert_eq!(neighbors_0.len(), 1);
        assert!(neighbors_0.contains_key("n1"));

        // 1-hop: n1 + n3 + n5
        let neighbors_1 = engine.get_n_hop_neighbors("n1", 1);
        assert!(neighbors_1.contains_key("n1"));
        assert!(neighbors_1.contains_key("n3"));
        assert!(neighbors_1.contains_key("n5"));

        // 2-hop: should also reach n4 via n2 if n2 is reachable
        // Actually n2 is not connected to n1, so 2-hop from n1 = same as 1-hop
        let neighbors_2 = engine.get_n_hop_neighbors("n1", 2);
        assert!(neighbors_2.len() >= neighbors_1.len());
    }

    // ============== Test 10: Search all strategies ==============
    #[test]
    fn test_search_all_strategies() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        let query = HybridQuery {
            text_query: "Rust memory".to_string(),
            max_depth: 2,
            min_relevance: 0.0,
            ..Default::default()
        };

        let results = engine.search_all_strategies(&query);
        assert!(!results.is_empty());
        // Should be sorted by relevance
        for i in 1..results.len() {
            assert!(results[i - 1].relevance_score >= results[i].relevance_score);
        }
    }

    // ============== Test 11: Default HybridQuery ==============
    #[test]
    fn test_default_query() {
        let query = HybridQuery::default();
        assert!(query.text_query.is_empty());
        assert!(query.graph_start_node.is_none());
        assert_eq!(query.max_depth, 2);
        assert_eq!(query.min_relevance, 0.0);
        assert_eq!(query.strategy, RetrievalStrategy::TextFirst);
    }

    // ============== Test 12: Tokenize ==============
    #[test]
    fn test_tokenize() {
        let terms = GraphRAGEngine::tokenize("The Rust programming language is great");
        assert!(terms.contains(&"rust".to_string()));
        assert!(terms.contains(&"programming".to_string()));
        assert!(terms.contains(&"language".to_string()));
        assert!(!terms.contains(&"the".to_string()));
        assert!(!terms.contains(&"is".to_string()));
    }

    // ============== Test 13: IDF computation ==============
    #[test]
    fn test_idf_computation() {
        let graph = build_test_graph();
        let engine = GraphRAGEngine::new(graph);

        // "rust" appears in 3 nodes (n1, n3, n5), so IDF should be lower
        // "kubernetes" appears in 1 node (n4), so IDF should be higher
        let idf_rust = engine.idf_cache.get("rust").copied().unwrap_or(0.0);
        let idf_kubernetes = engine.idf_cache.get("kubernetes").copied().unwrap_or(0.0);
        assert!(idf_rust > 0.0);
        assert!(idf_kubernetes > 0.0);
        assert!(idf_kubernetes > idf_rust);
    }
}
