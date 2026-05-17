//! # Semantic Skill Matcher
//!
//! HNSW-powered intent-to-skill matching, replacing pure keyword lookup
//! with approximate nearest neighbor search in embedding space.
//!
//! ## Sembr-Inspired Design
//!
//! Traditional skill matching does exact string comparison:
//!   "code_generation" == "code_generation"  ✓
//!   "code_generation" == "code_gen"         ✗  (miss!)
//!
//! Semantic matching embeds both the intent and skill names into a shared
//! vector space, then uses HNSW for O(log N) nearest neighbor search:
//!   embed("generate some code") ≈ embed("code_generation")  ✓
//!   embed("write unit tests")   ≈ embed("testing")          ✓
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐     embed      ┌──────────────┐
//! │  Intent Text  │ ──────────────▶│ Query Vector  │
//! └──────────────┘                └───────┬───────┘
//!                                         │
//!                                    HNSW KNN search
//!                                         │
//! ┌──────────────┐     embed      ┌───────▼───────┐
//! │  Skill Names  │ ──────────────▶│  HNSW Index   │
//! │  (per agent)  │                │  (all skills)  │
//! └──────────────┘                └───────┬───────┘
//!                                         │
//!                                    Top-K results
//!                                         │
//!                                  ┌──────▼──────┐
//!                                  │  Score Merge │
//!                                  │  (sem+other) │
//!                                  └─────────────┘
//! ```
//!
//! ## Scoring Formula
//!
//! ```text
//! final_score = semantic_score * semantic_weight
//!             + capability_score * capability_weight
//!             + availability * availability_weight
//!             + (1.0 - load) * load_weight
//!             + success_rate * success_weight
//! ```
//!
//! Where `semantic_score` comes from HNSW cosine similarity between
//! the embedded intent and the best-matching skill embedding.

use crate::embedder::{Embedder, HashingEmbedder, DEFAULT_EMBEDDING_DIM};
use crate::skill_matcher::{AgentProfile, MatcherConfig};
use kias_common::vector::VectorStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Result from semantic skill matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatchResult {
    /// Agent ID
    pub agent_id: String,
    /// Overall combined score (0.0 - 1.0)
    pub score: f32,
    /// Semantic similarity score (cosine similarity from HNSW)
    pub semantic_score: f32,
    /// Best-matching capability name (most semantically similar)
    pub best_matching_skill: Option<String>,
    /// Traditional capability match score (for hybrid mode)
    pub capability_score: f32,
    /// Whether all required capabilities were found (exact match)
    pub all_capabilities_met: bool,
    /// Missing capabilities (exact match)
    pub missing_capabilities: Vec<String>,
    /// Agent's current load
    pub agent_load: f32,
    /// Agent's success rate
    pub agent_success_rate: f32,
}

/// Configuration for the semantic skill matcher.
#[derive(Debug, Clone)]
pub struct SemanticMatcherConfig {
    /// Base matching config (weights, thresholds)
    pub base: MatcherConfig,
    /// Weight for semantic similarity score (default: 0.5)
    pub semantic_weight: f32,
    /// Weight for traditional capability score (default: 0.1, reduced from 0.6)
    pub capability_weight: f32,
    /// Weight for availability (default: 0.2)
    pub availability_weight: f32,
    /// Weight for low load (default: 0.15)
    pub load_weight: f32,
    /// Weight for historical success (default: 0.05)
    pub success_weight: f32,
    /// HNSW ef_search parameter (higher = better recall, slower)
    pub ef_search: usize,
    /// Number of candidates to retrieve from HNSW before scoring
    pub candidate_k: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Minimum semantic similarity to consider a match
    pub min_semantic_similarity: f32,
}

impl Default for SemanticMatcherConfig {
    fn default() -> Self {
        Self {
            base: MatcherConfig::default(),
            semantic_weight: 0.5,
            capability_weight: 0.1,
            availability_weight: 0.2,
            load_weight: 0.15,
            success_weight: 0.05,
            ef_search: 100,
            candidate_k: 20,
            embedding_dim: DEFAULT_EMBEDDING_DIM,
            min_semantic_similarity: 0.05,
        }
    }
}

/// An entry in the HNSW index, mapping a vector to its agent + capability.
#[derive(Debug, Clone)]
struct SkillEntry {
    agent_id: String,
    capability: String,
    proficiency: f32,
}

/// Semantic skill matcher using HNSW for intent→skill matching.
///
/// Replaces exact keyword lookup with approximate nearest neighbor search
/// in a shared embedding space. All agent capabilities are pre-embedded
/// and stored in an HNSW index for O(log N) query time.
pub struct SemanticSkillMatcher {
    config: SemanticMatcherConfig,
    /// HNSW index for fast ANN search
    hnsw: VectorStore,
    /// Map from HNSW node_id → skill entry metadata
    entries: HashMap<String, SkillEntry>,
    /// Text embedder
    embedder: Box<dyn Embedder>,
}

impl SemanticSkillMatcher {
    /// Create a new semantic skill matcher with default hashing embedder.
    pub fn new(config: SemanticMatcherConfig) -> Self {
        let dim = config.embedding_dim;
        Self {
            config,
            hnsw: VectorStore::new(dim),
            entries: HashMap::new(),
            embedder: Box::new(HashingEmbedder::new(dim)),
        }
    }

    /// Create with a custom embedder (e.g., for external model-based embeddings).
    pub fn with_embedder(mut self, embedder: Box<dyn Embedder>) -> Self {
        self.config.embedding_dim = embedder.dimension();
        self.hnsw = VectorStore::new(embedder.dimension());
        self.embedder = embedder;
        self
    }

    /// Index all agent capabilities into the HNSW graph.
    ///
    /// Call this after creating the matcher and whenever the agent pool changes.
    /// Each capability is embedded as "{capability_name}" and stored with
    /// agent_id + proficiency metadata.
    pub fn index_agents(&mut self, agents: &[AgentProfile]) {
        let mut count = 0;
        for agent in agents {
            for (capability, &proficiency) in &agent.capabilities {
                let node_id = format!("{}::{}", agent.agent_id, capability);
                let vector = self.embedder.embed(capability);
                self.hnsw.insert(node_id.clone(), vector);
                self.entries.insert(
                    node_id,
                    SkillEntry {
                        agent_id: agent.agent_id.clone(),
                        capability: capability.clone(),
                        proficiency,
                    },
                );
                count += 1;
            }
        }
        debug!(count, "Indexed agent capabilities into HNSW");
    }

    /// Clear the HNSW index and re-index from scratch.
    pub fn rebuild_index(&mut self, agents: &[AgentProfile]) {
        self.hnsw = VectorStore::new(self.config.embedding_dim);
        self.entries.clear();
        self.index_agents(agents);
    }

    /// Find and rank agents matching the given intent using semantic search.
    ///
    /// The intent can be a natural language description of the required task
    /// (e.g., "help me write SQL queries to analyze sales data") rather than
    /// exact capability names.
    pub fn find_matches(&self, agents: &[AgentProfile], intent: &str) -> Vec<SemanticMatchResult> {
        if intent.is_empty() || self.entries.is_empty() {
            return Vec::new();
        }

        // Embed the intent query
        let query_vector = self.embedder.embed(intent);

        // HNSW search for nearest skill embeddings
        let k = self.config.candidate_k.min(self.entries.len());
        let knn_results = self.hnsw.search_knn(&query_vector, k);

        // Collect per-agent best semantic scores
        let mut agent_best_semantic: HashMap<String, (f32, String)> = HashMap::new();
        for (node_id, distance) in &knn_results {
            let similarity = 1.0 - distance; // cosine_distance → cosine_similarity
            if similarity < self.config.min_semantic_similarity {
                continue;
            }
            if let Some(entry) = self.entries.get(node_id) {
                let weighted_similarity = similarity * entry.proficiency;
                agent_best_semantic
                    .entry(entry.agent_id.clone())
                    .and_modify(|(best, _)| {
                        if weighted_similarity > *best {
                            *best = weighted_similarity;
                        }
                    })
                    .or_insert((weighted_similarity, entry.capability.clone()));
            }
        }

        // Build results for all agents
        let mut results: Vec<SemanticMatchResult> = agents
            .iter()
            .map(|agent| {
                let (semantic_score, best_skill) = agent_best_semantic
                    .get(&agent.agent_id)
                    .cloned()
                    .unwrap_or((0.0, String::new()));

                let availability_score = if agent.available { 1.0 } else { 0.0 };
                let load_score = 1.0 - agent.load;
                let success_score = agent.success_rate;

                // For semantic mode, capability_score is derived from semantic match
                let capability_score = semantic_score;

                let score = semantic_score * self.config.semantic_weight
                    + capability_score * self.config.capability_weight
                    + availability_score * self.config.availability_weight
                    + load_score * self.config.load_weight
                    + success_score * self.config.success_weight;

                SemanticMatchResult {
                    agent_id: agent.agent_id.clone(),
                    score,
                    semantic_score,
                    best_matching_skill: if best_skill.is_empty() {
                        None
                    } else {
                        Some(best_skill)
                    },
                    capability_score,
                    all_capabilities_met: semantic_score > 0.5, // semantic threshold
                    missing_capabilities: Vec::new(),
                    agent_load: agent.load,
                    agent_success_rate: agent.success_rate,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Find the single best agent for the given intent.
    pub fn find_best(&self, agents: &[AgentProfile], intent: &str) -> Option<SemanticMatchResult> {
        self.find_matches(agents, intent).into_iter().next()
    }

    /// Hybrid matching: combine semantic search with exact keyword matching.
    ///
    /// Uses semantic search as the primary signal, but boosts agents that
    /// also have exact keyword matches for the required capabilities.
    pub fn find_matches_hybrid(
        &self,
        agents: &[AgentProfile],
        intent: &str,
        required_capabilities: &[String],
    ) -> Vec<SemanticMatchResult> {
        let mut results = self.find_matches(agents, intent);

        // Boost agents with exact capability matches
        for result in &mut results {
            if let Some(agent) = agents.iter().find(|a| a.agent_id == result.agent_id) {
                let exact_matches: f32 = required_capabilities
                    .iter()
                    .filter(|cap| agent.capabilities.contains_key(cap.as_str()))
                    .count() as f32;
                let total_required = required_capabilities.len().max(1) as f32;
                let exact_ratio = exact_matches / total_required;

                // Boost: up to 20% bonus for exact matches
                result.score = (result.score + exact_ratio * 0.2).min(1.0);

                // Track missing capabilities for exact match
                result.missing_capabilities = required_capabilities
                    .iter()
                    .filter(|cap| !agent.capabilities.contains_key(cap.as_str()))
                    .cloned()
                    .collect();
                result.all_capabilities_met = result.missing_capabilities.is_empty();
            }
        }

        // Re-sort after boosting
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Get the number of indexed skills.
    pub fn indexed_skill_count(&self) -> usize {
        self.entries.len()
    }

    /// Get the number of indexed agents (unique agent_ids).
    pub fn indexed_agent_count(&self) -> usize {
        let mut agents: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for entry in self.entries.values() {
            agents.insert(&entry.agent_id);
        }
        agents.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agents() -> Vec<AgentProfile> {
        vec![
            AgentProfile::new("a1", "Code Expert")
                .with_capability("code_generation", 0.95)
                .with_capability("code_review", 0.9)
                .with_capability("testing", 0.7)
                .with_load(0.3)
                .with_success_rate(0.95),
            AgentProfile::new("a2", "Research Specialist")
                .with_capability("web_search", 0.9)
                .with_capability("document_analysis", 0.85)
                .with_capability("summarization", 0.8)
                .with_load(0.1)
                .with_success_rate(0.9),
            AgentProfile::new("a3", "Data Engineer")
                .with_capability("sql_query", 0.9)
                .with_capability("data_transform", 0.85)
                .with_capability("csv_process", 0.8)
                .with_load(0.5)
                .with_success_rate(0.88),
            AgentProfile::new("a4", "Full Stack")
                .with_capability("code_generation", 0.7)
                .with_capability("web_search", 0.6)
                .with_capability("testing", 0.8)
                .with_load(0.8)
                .with_success_rate(0.85),
        ]
    }

    #[test]
    fn test_semantic_matcher_indexing() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        // 4 agents × 3 capabilities each = 12 indexed skills
        assert_eq!(matcher.indexed_skill_count(), 12);
        assert_eq!(matcher.indexed_agent_count(), 4);
    }

    #[test]
    fn test_semantic_matcher_finds_code_agent() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        let results = matcher.find_matches(&agents, "code_generation");
        assert!(!results.is_empty(), "Should find at least one match");
        // Code Expert (a1) should rank high
        let top = &results[0];
        assert!(
            top.semantic_score > 0.0,
            "Top result should have semantic score > 0"
        );
    }

    #[test]
    fn test_semantic_matcher_semantic_similarity() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        // "write code" should match code_generation better than sql_query
        let results = matcher.find_matches(&agents, "write code");
        assert!(!results.is_empty());

        // Find code agent and data agent scores
        let code_score = results
            .iter()
            .find(|r| r.agent_id == "a1")
            .map(|r| r.semantic_score);
        let data_score = results
            .iter()
            .find(|r| r.agent_id == "a3")
            .map(|r| r.semantic_score);

        if let (Some(cs), Some(ds)) = (code_score, data_score) {
            assert!(
                cs > ds,
                "Code agent ({cs}) should match 'write code' better than data agent ({ds})"
            );
        }
    }

    #[test]
    fn test_semantic_matcher_sql_intent() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        // "run a database query" should match sql_query agent
        let results = matcher.find_matches(&agents, "run a database query");
        assert!(!results.is_empty());

        // Data Engineer (a3) should be in top results
        let data_result = results.iter().find(|r| r.agent_id == "a3");
        assert!(
            data_result.is_some(),
            "Data agent should match database query intent"
        );
    }

    #[test]
    fn test_semantic_matcher_hybrid_mode() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        let results = matcher.find_matches_hybrid(
            &agents,
            "code_generation",
            &["code_generation".to_string()],
        );
        assert!(!results.is_empty());

        // a1 should get a boost for exact match
        let a1 = results.iter().find(|r| r.agent_id == "a1").unwrap();
        assert!(
            a1.all_capabilities_met,
            "a1 should have exact match for code_generation"
        );
    }

    #[test]
    fn test_semantic_matcher_empty_intent() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        let results = matcher.find_matches(&agents, "");
        assert!(results.is_empty(), "Empty intent should return no results");
    }

    #[test]
    fn test_semantic_matcher_no_agents() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        matcher.index_agents(&[]);

        let results = matcher.find_matches(&[], "anything");
        assert!(results.is_empty());
    }

    #[test]
    fn test_semantic_matcher_best_single() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        let best = matcher.find_best(&agents, "search the web");
        assert!(best.is_some(), "Should find a best match");
    }

    #[test]
    fn test_semantic_matcher_rebuild_index() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);
        assert_eq!(matcher.indexed_skill_count(), 12);

        // Rebuild with fewer agents
        matcher.rebuild_index(&agents[0..2]);
        assert_eq!(matcher.indexed_skill_count(), 6);
        assert_eq!(matcher.indexed_agent_count(), 2);
    }

    #[test]
    fn test_semantic_matcher_load_affects_ranking() {
        let mut matcher = SemanticSkillMatcher::new(SemanticMatcherConfig::default());
        let agents = make_agents();
        matcher.index_agents(&agents);

        let results = matcher.find_matches(&agents, "code_generation");
        // Find a1 (load=0.3) and a4 (load=0.8) — both have code_generation
        let a1 = results.iter().find(|r| r.agent_id == "a1");
        let a4 = results.iter().find(|r| r.agent_id == "a4");

        if let (Some(r1), Some(r4)) = (a1, a4) {
            // a1 should rank higher due to lower load + higher proficiency
            assert!(
                r1.score >= r4.score,
                "Lower-loaded agent should score higher: a1={}, a4={}",
                r1.score,
                r4.score
            );
        }
    }

    #[test]
    fn test_semantic_matcher_custom_embedder() {
        // Use a smaller dimension for testing
        let embedder = HashingEmbedder::new(32);
        let config = SemanticMatcherConfig {
            embedding_dim: 32,
            ..SemanticMatcherConfig::default()
        };
        let matcher = SemanticSkillMatcher::new(config).with_embedder(Box::new(embedder));
        assert_eq!(matcher.indexed_skill_count(), 0);
    }
}
