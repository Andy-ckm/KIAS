//! # Agent Memory System
//!
//! Long-term memory for agents inspired by Memory Palace.
//! Provides episodic memory (what happened), semantic memory (what is known),
//! and procedural memory (how to do things).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory entry identifier
pub type MemoryId = String;

/// Agent identifier
pub type AgentId = String;

/// Memory types following cognitive science taxonomy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryType {
    /// Episodic: events and experiences (what happened)
    Episodic,
    /// Semantic: facts and knowledge (what is known)
    Semantic,
    /// Procedural: skills and procedures (how to do)
    Procedural,
}

/// Importance level for memory consolidation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Importance {
    /// Low importance, can be forgotten
    Low = 1,
    /// Normal importance
    #[default]
    Normal = 5,
    /// High importance, should be retained
    High = 10,
    /// Critical, never forget
    Critical = 100,
}

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique memory ID
    pub id: MemoryId,
    /// Agent that owns this memory
    pub agent_id: AgentId,
    /// Type of memory
    pub memory_type: MemoryType,
    /// The content of the memory
    pub content: String,
    /// Tags for categorization and retrieval
    pub tags: Vec<String>,
    /// Importance level
    pub importance: Importance,
    /// Access count (for frequency-based retrieval)
    pub access_count: u64,
    /// When the memory was created
    pub created_at: DateTime<Utc>,
    /// When the memory was last accessed
    pub last_accessed: DateTime<Utc>,
    /// Optional embedding vector for similarity search
    pub embedding: Option<Vec<f32>>,
    /// Metadata key-value pairs
    pub metadata: HashMap<String, String>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(
        agent_id: impl Into<String>,
        memory_type: MemoryType,
        content: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            memory_type,
            content: content.into(),
            tags: Vec::new(),
            importance: Importance::default(),
            access_count: 0,
            created_at: now,
            last_accessed: now,
            embedding: None,
            metadata: HashMap::new(),
        }
    }

    /// Add tags to the memory
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set importance level
    pub fn with_importance(mut self, importance: Importance) -> Self {
        self.importance = importance;
        self
    }

    /// Record an access to this memory
    pub fn access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }

    /// Calculate recency score (0.0 to 1.0, higher = more recent)
    pub fn recency_score(&self) -> f64 {
        let age = Utc::now().signed_duration_since(self.last_accessed);
        let hours = age.num_seconds() as f64 / 3600.0;
        1.0 / (1.0 + hours.ln_1p()) // Logarithmic decay
    }

    /// Calculate composite relevance score
    pub fn relevance_score(&self) -> f64 {
        let recency = self.recency_score();
        let frequency = (self.access_count as f64).ln_1p() / 10.0;
        let importance = self.importance as u64 as f64 / 100.0;
        recency * 0.4 + frequency * 0.3 + importance * 0.3
    }
}

/// Agent Memory Store - per-agent memory management
pub struct AgentMemoryStore {
    /// In-memory store: agent_id -> memories
    stores: Arc<RwLock<HashMap<AgentId, Vec<MemoryEntry>>>>,
    /// Maximum memories per agent
    max_per_agent: usize,
}

impl AgentMemoryStore {
    /// Create a new memory store
    pub fn new() -> Self {
        Self::with_limit(10000)
    }

    /// Create a memory store with per-agent limit
    pub fn with_limit(max_per_agent: usize) -> Self {
        Self {
            stores: Arc::new(RwLock::new(HashMap::new())),
            max_per_agent,
        }
    }

    /// Store a new memory
    pub async fn remember(&self, entry: MemoryEntry) -> MemoryId {
        let id = entry.id.clone();
        let agent_id = entry.agent_id.clone();
        let mut stores = self.stores.write().await;
        let agent_store = stores.entry(agent_id).or_default();
        agent_store.push(entry);

        // Evict lowest-scoring memories if over limit
        if agent_store.len() > self.max_per_agent {
            agent_store.sort_by(|a, b| {
                a.relevance_score()
                    .partial_cmp(&b.relevance_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            agent_store.remove(0); // Remove lowest scoring
        }

        id
    }

    /// Retrieve memories for an agent, sorted by relevance
    pub async fn recall(
        &self,
        agent_id: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Vec<MemoryEntry> {
        let stores = self.stores.read().await;
        let mut entries: Vec<MemoryEntry> = stores
            .get(agent_id)
            .map(|mems| {
                mems.iter()
                    .filter(|m| {
                        memory_type
                            .as_ref()
                            .is_none_or(|t| m.memory_type == *t)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        // Sort by relevance
        entries.sort_by(|a, b| {
            b.relevance_score()
                .partial_cmp(&a.relevance_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        entries.into_iter().take(limit).collect()
    }

    /// Search memories by content substring match
    pub async fn search(&self, agent_id: &str, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let stores = self.stores.read().await;
        let query_lower = query.to_lowercase();

        let mut entries: Vec<MemoryEntry> = stores
            .get(agent_id)
            .map(|mems| {
                mems.iter()
                    .filter(|m| {
                        m.content.to_lowercase().contains(&query_lower)
                            || m.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        entries.sort_by(|a, b| {
            b.relevance_score()
                .partial_cmp(&a.relevance_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        entries.into_iter().take(limit).collect()
    }

    /// Get total memory count for an agent
    pub async fn count(&self, agent_id: &str) -> usize {
        let stores = self.stores.read().await;
        stores.get(agent_id).map_or(0, |m| m.len())
    }

    /// Get all agent IDs with memories
    pub async fn agents(&self) -> Vec<AgentId> {
        let stores = self.stores.read().await;
        stores.keys().cloned().collect()
    }

    /// Forget (delete) a specific memory
    pub async fn forget(&self, agent_id: &str, memory_id: &str) -> bool {
        let mut stores = self.stores.write().await;
        if let Some(mems) = stores.get_mut(agent_id) {
            let len_before = mems.len();
            mems.retain(|m| m.id != memory_id);
            return mems.len() < len_before;
        }
        false
    }

    /// Clear all memories for an agent
    pub async fn clear(&self, agent_id: &str) {
        let mut stores = self.stores.write().await;
        stores.remove(agent_id);
    }
}

impl Default for AgentMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry_creation() {
        let entry = MemoryEntry::new("agent-1", MemoryType::Episodic, "Had a meeting");
        assert_eq!(entry.agent_id, "agent-1");
        assert_eq!(entry.memory_type, MemoryType::Episodic);
        assert_eq!(entry.content, "Had a meeting");
        assert_eq!(entry.access_count, 0);
    }

    #[test]
    fn test_memory_entry_with_tags() {
        let entry = MemoryEntry::new("a1", MemoryType::Semantic, "Rust is fast")
            .with_tags(vec!["rust".to_string(), "programming".to_string()]);
        assert_eq!(entry.tags.len(), 2);
    }

    #[test]
    fn test_memory_entry_with_importance() {
        let entry = MemoryEntry::new("a1", MemoryType::Procedural, "deploy steps")
            .with_importance(Importance::Critical);
        assert_eq!(entry.importance, Importance::Critical);
    }

    #[test]
    fn test_memory_access() {
        let mut entry = MemoryEntry::new("a1", MemoryType::Episodic, "test");
        assert_eq!(entry.access_count, 0);
        entry.access();
        assert_eq!(entry.access_count, 1);
        entry.access();
        assert_eq!(entry.access_count, 2);
    }

    #[test]
    fn test_recency_score() {
        let entry = MemoryEntry::new("a1", MemoryType::Episodic, "recent");
        let score = entry.recency_score();
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_relevance_score() {
        let mut entry = MemoryEntry::new("a1", MemoryType::Episodic, "test");
        entry.access_count = 5;
        let score = entry.relevance_score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_importance_ordering() {
        assert!(Importance::Low < Importance::Normal);
        assert!(Importance::Normal < Importance::High);
        assert!(Importance::High < Importance::Critical);
    }

    #[tokio::test]
    async fn test_memory_store_remember_and_recall() {
        let store = AgentMemoryStore::new();
        store
            .remember(MemoryEntry::new("a1", MemoryType::Episodic, "event 1"))
            .await;
        store
            .remember(MemoryEntry::new("a1", MemoryType::Semantic, "fact 1"))
            .await;

        let all = store.recall("a1", None, 10).await;
        assert_eq!(all.len(), 2);

        let episodic = store
            .recall("a1", Some(MemoryType::Episodic), 10)
            .await;
        assert_eq!(episodic.len(), 1);
        assert_eq!(episodic[0].content, "event 1");
    }

    #[tokio::test]
    async fn test_memory_search() {
        let store = AgentMemoryStore::new();
        store
            .remember(
                MemoryEntry::new("a1", MemoryType::Semantic, "Rust is a systems language")
                    .with_tags(vec!["rust".to_string()]),
            )
            .await;
        store
            .remember(MemoryEntry::new(
                "a1",
                MemoryType::Semantic,
                "Python is interpreted",
            ))
            .await;

        let results = store.search("a1", "rust", 10).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Rust"));
    }

    #[tokio::test]
    async fn test_memory_forget() {
        let store = AgentMemoryStore::new();
        let id = store
            .remember(MemoryEntry::new("a1", MemoryType::Episodic, "temp"))
            .await;

        assert_eq!(store.count("a1").await, 1);
        assert!(store.forget("a1", &id).await);
        assert_eq!(store.count("a1").await, 0);
    }

    #[tokio::test]
    async fn test_memory_clear() {
        let store = AgentMemoryStore::new();
        store
            .remember(MemoryEntry::new("a1", MemoryType::Episodic, "a"))
            .await;
        store
            .remember(MemoryEntry::new("a1", MemoryType::Episodic, "b"))
            .await;

        assert_eq!(store.count("a1").await, 2);
        store.clear("a1").await;
        assert_eq!(store.count("a1").await, 0);
    }

    #[tokio::test]
    async fn test_memory_agents() {
        let store = AgentMemoryStore::new();
        store
            .remember(MemoryEntry::new("a1", MemoryType::Episodic, "x"))
            .await;
        store
            .remember(MemoryEntry::new("a2", MemoryType::Episodic, "y"))
            .await;

        let agents = store.agents().await;
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_eviction() {
        let store = AgentMemoryStore::with_limit(3);
        for i in 0..5 {
            store
                .remember(MemoryEntry::new(
                    "a1",
                    MemoryType::Episodic,
                    format!("memory {}", i),
                ))
                .await;
        }
        assert_eq!(store.count("a1").await, 3);
    }
}
