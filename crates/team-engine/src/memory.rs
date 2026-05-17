//! # Agent Memory System
//!
//! CrewAI-inspired shared memory for multi-agent collaboration.
//!
//! Three tiers of memory:
//! - **Short-term**: Current task context (cleared between tasks)
//! - **Long-term**: Persistent knowledge across tasks (survives restarts)
//! - **Entity Memory**: Facts about entities encountered during execution
//!
//! ## Design Principles
//!
//! 1. Thread-safe via `Arc<RwLock<>>`
//! 2. TTL-based eviction for short-term memory
//! 3. Relevance scoring for retrieval
//! 4. Context window management (max tokens)

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique entry ID
    pub id: String,
    /// Which agent created this memory
    pub agent_id: String,
    /// The memory content
    pub content: String,
    /// Optional structured data
    pub metadata: serde_json::Value,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Relevance score (0.0 - 1.0, higher = more relevant)
    pub relevance: f32,
    /// When this memory was created
    pub created_at: DateTime<Utc>,
    /// When this memory expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
    /// Access count (for LRU-like eviction)
    pub access_count: u32,
    /// Last accessed time
    pub last_accessed: DateTime<Utc>,
}

impl MemoryEntry {
    /// Check if this entry has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Record an access
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }
}

/// Short-term memory: task-scoped, TTL-based
#[derive(Debug)]
pub struct ShortTermMemory {
    entries: Vec<MemoryEntry>,
    /// Maximum entries before eviction
    max_entries: usize,
    /// Default TTL for entries
    default_ttl: Duration,
}

impl ShortTermMemory {
    pub fn new(max_entries: usize, default_ttl_secs: i64) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            default_ttl: Duration::seconds(default_ttl_secs),
        }
    }

    /// Store a new memory entry
    pub fn store(&mut self, agent_id: &str, content: &str, tags: Vec<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let entry = MemoryEntry {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            tags,
            relevance: 1.0,
            created_at: now,
            expires_at: Some(now + self.default_ttl),
            access_count: 0,
            last_accessed: now,
        };
        self.entries.push(entry);

        // Evict expired and excess entries
        self.evict();

        id
    }

    /// Retrieve memories matching a query (simple substring + tag match)
    pub fn search(&mut self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| !e.is_expired())
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower)
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect();

        // Sort by relevance (descending) then by recency
        results.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.created_at.cmp(&a.created_at))
        });

        results.truncate(limit);

        // Update access counts for returned entries
        for result in &results {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == result.id) {
                entry.touch();
            }
        }

        results
    }

    /// Evict expired entries and enforce max_entries
    fn evict(&mut self) {
        // Remove expired
        self.entries.retain(|e| !e.is_expired());

        // If still over capacity, remove least recently accessed
        if self.entries.len() > self.max_entries {
            self.entries.sort_by_key(|a| a.last_accessed);
            self.entries.drain(0..self.entries.len() - self.max_entries);
        }
    }

    /// Clear all short-term memory
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_expired()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Long-term memory: persistent, no TTL, relevance-decay based
#[derive(Debug)]
pub struct LongTermMemory {
    entries: Vec<MemoryEntry>,
    max_entries: usize,
}

impl LongTermMemory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Store a long-term memory
    pub fn store(&mut self, agent_id: &str, content: &str, tags: Vec<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let entry = MemoryEntry {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            tags,
            relevance: 1.0,
            created_at: now,
            expires_at: None,
            access_count: 0,
            last_accessed: now,
        };
        self.entries.push(entry);
        self.evict_if_needed();
        id
    }

    /// Search long-term memories
    pub fn search(&mut self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower)
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        // Update access counts
        for result in &results {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == result.id) {
                entry.touch();
            }
        }

        results
    }

    fn evict_if_needed(&mut self) {
        if self.entries.len() > self.max_entries {
            // Keep most accessed and most recent
            self.entries.sort_by(|a, b| {
                b.access_count
                    .cmp(&a.access_count)
                    .then(b.last_accessed.cmp(&a.last_accessed))
            });
            self.entries.truncate(self.max_entries);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Entity memory: facts about specific entities (agents, tools, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFact {
    /// Entity name/ID
    pub entity: String,
    /// Fact type (e.g., "capability", "preference", "history")
    pub fact_type: String,
    /// The fact content
    pub content: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// When this fact was learned
    pub learned_at: DateTime<Utc>,
}

/// Entity memory store
#[derive(Debug)]
pub struct EntityMemory {
    facts: HashMap<String, Vec<EntityFact>>,
    max_facts_per_entity: usize,
}

impl EntityMemory {
    pub fn new(max_facts_per_entity: usize) -> Self {
        Self {
            facts: HashMap::new(),
            max_facts_per_entity,
        }
    }

    /// Record a fact about an entity
    pub fn record(&mut self, entity: &str, fact_type: &str, content: &str, confidence: f32) {
        let fact = EntityFact {
            entity: entity.to_string(),
            fact_type: fact_type.to_string(),
            content: content.to_string(),
            confidence: confidence.clamp(0.0, 1.0),
            learned_at: Utc::now(),
        };

        let facts = self.facts.entry(entity.to_string()).or_default();
        facts.push(fact);

        // Evict old facts
        if facts.len() > self.max_facts_per_entity {
            facts.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.learned_at.cmp(&a.learned_at))
            });
            facts.truncate(self.max_facts_per_entity);
        }
    }

    /// Get all facts about an entity
    pub fn get_facts(&self, entity: &str) -> Vec<EntityFact> {
        self.facts.get(entity).cloned().unwrap_or_default()
    }

    /// Get facts of a specific type about an entity
    pub fn get_facts_by_type(&self, entity: &str, fact_type: &str) -> Vec<EntityFact> {
        self.facts
            .get(entity)
            .map(|facts| {
                facts
                    .iter()
                    .filter(|f| f.fact_type == fact_type)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn entity_count(&self) -> usize {
        self.facts.len()
    }
}

/// Memory category for mid-term memory (Hermes-inspired)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryCategory {
    /// User preferences (e.g., "prefers concise responses")
    UserPreference,
    /// Environment facts (e.g., "OS: Ubuntu 22.04", "Python 3.11")
    EnvironmentFact,
    /// Tool quirks (e.g., "terminal timeout needs 300s for cargo test")
    ToolQuirk,
    /// Stable conventions (e.g., "always use main branch, never master")
    Convention,
    /// Recurring corrections (e.g., "don't use sed for file editing")
    Correction,
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserPreference => write!(f, "user_preference"),
            Self::EnvironmentFact => write!(f, "environment_fact"),
            Self::ToolQuirk => write!(f, "tool_quirk"),
            Self::Convention => write!(f, "convention"),
            Self::Correction => write!(f, "correction"),
        }
    }
}

/// Mid-term memory entry with category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidTermEntry {
    pub id: String,
    pub category: MemoryCategory,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub access_count: u32,
}

/// Mid-term memory: persistent across sessions, categorized
/// Inspired by Hermes MEMORY.md — user preferences, environment facts, conventions
#[derive(Debug)]
pub struct MidTermMemory {
    entries: Vec<MidTermEntry>,
    max_entries: usize,
}

impl MidTermMemory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add a new memory entry
    pub fn add(&mut self, category: MemoryCategory, content: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let entry = MidTermEntry {
            id: id.clone(),
            category,
            content: content.to_string(),
            created_at: now,
            updated_at: now,
            access_count: 0,
        };
        self.entries.push(entry);
        self.evict_if_needed();
        id
    }

    /// Replace an existing entry by id
    pub fn replace(&mut self, id: &str, new_content: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.content = new_content.to_string();
            entry.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Remove an entry by id
    pub fn remove(&mut self, id: &str) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len_before
    }

    /// Search entries by query (substring match on content)
    pub fn search(&mut self, query: &str, limit: usize) -> Vec<MidTermEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<MidTermEntry> = self
            .entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();
        results.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        results.truncate(limit);

        // Update access counts
        for result in &results {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == result.id) {
                entry.access_count += 1;
            }
        }
        results
    }

    /// Get all entries of a specific category
    pub fn get_by_category(&self, category: &MemoryCategory) -> Vec<MidTermEntry> {
        self.entries
            .iter()
            .filter(|e| &e.category == category)
            .cloned()
            .collect()
    }

    /// Build prompt injection string from all memories
    pub fn build_prompt_context(&self) -> String {
        let mut sections: HashMap<String, Vec<String>> = HashMap::new();
        for entry in &self.entries {
            sections
                .entry(entry.category.to_string())
                .or_default()
                .push(entry.content.clone());
        }

        let mut context = String::from("## Memory (injected)\n\n");
        for (category, items) in &sections {
            context.push_str(&format!("### {}\n", category));
            for item in items {
                context.push_str(&format!("- {}\n", item));
            }
            context.push('\n');
        }
        context
    }

    /// Export to MEMORY.md format
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("# Agent Memory\n\n");
        let mut by_category: HashMap<String, Vec<&MidTermEntry>> = HashMap::new();
        for entry in &self.entries {
            by_category
                .entry(entry.category.to_string())
                .or_default()
                .push(entry);
        }
        for (cat, entries) in &by_category {
            md.push_str(&format!("## {}\n\n", cat));
            for entry in entries {
                md.push_str(&format!("- {}\n", entry.content));
            }
            md.push('\n');
        }
        md
    }

    /// Import from MEMORY.md markdown string
    pub fn from_markdown(&mut self, markdown: &str) {
        let mut current_category = MemoryCategory::Convention;
        for line in markdown.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("## ") {
                let cat_str = trimmed.trim_start_matches("## ").trim();
                current_category = match cat_str.to_lowercase().as_str() {
                    "user_preference" | "user preference" => MemoryCategory::UserPreference,
                    "environment_fact" | "environment fact" => MemoryCategory::EnvironmentFact,
                    "tool_quirk" | "tool quirk" => MemoryCategory::ToolQuirk,
                    "convention" => MemoryCategory::Convention,
                    "correction" => MemoryCategory::Correction,
                    _ => continue,
                };
            } else if trimmed.starts_with("- ") {
                let content = trimmed.trim_start_matches("- ").trim();
                if !content.is_empty() {
                    self.add(current_category.clone(), content);
                }
            }
        }
    }

    fn evict_if_needed(&mut self) {
        if self.entries.len() > self.max_entries {
            self.entries.sort_by(|a, b| {
                b.access_count
                    .cmp(&a.access_count)
                    .then(b.updated_at.cmp(&a.updated_at))
            });
            self.entries.truncate(self.max_entries);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for MidTermMemory {
    fn default() -> Self {
        Self::new(500)
    }
}

/// Unified memory manager combining all three tiers
pub struct MemoryManager {
    pub short_term: Arc<RwLock<ShortTermMemory>>,
    pub long_term: Arc<RwLock<LongTermMemory>>,
    pub entity: Arc<RwLock<EntityMemory>>,
    pub mid_term: Arc<RwLock<MidTermMemory>>,
}

impl MemoryManager {
    pub fn new(
        short_term_max: usize,
        short_term_ttl_secs: i64,
        long_term_max: usize,
        entity_max_per: usize,
    ) -> Self {
        Self {
            short_term: Arc::new(RwLock::new(ShortTermMemory::new(
                short_term_max,
                short_term_ttl_secs,
            ))),
            long_term: Arc::new(RwLock::new(LongTermMemory::new(long_term_max))),
            entity: Arc::new(RwLock::new(EntityMemory::new(entity_max_per))),
            mid_term: Arc::new(RwLock::new(MidTermMemory::default())),
        }
    }

    /// Build full prompt context from all memory tiers
    pub async fn build_full_context(&self, query: &str, max_tokens: usize) -> String {
        let mut context = String::new();

        // Mid-term memories (user preferences, conventions, etc.)
        let mid = self.mid_term.read().await;
        context.push_str(&mid.build_prompt_context());
        drop(mid);

        // Relevant short-term memories
        let mut stm = self.short_term.write().await;
        let stm_results = stm.search(query, 5);
        if !stm_results.is_empty() {
            context.push_str("## Recent Context\n\n");
            for entry in &stm_results {
                context.push_str(&format!("- [{}] {}\n", entry.agent_id, entry.content));
            }
            context.push('\n');
        }
        drop(stm);

        // Relevant long-term memories
        let mut ltm = self.long_term.write().await;
        let ltm_results = ltm.search(query, 5);
        if !ltm_results.is_empty() {
            context.push_str("## Knowledge\n\n");
            for entry in &ltm_results {
                context.push_str(&format!("- {}\n", entry.content));
            }
        }

        // Truncate to max_tokens (approximate)
        if context.len() > max_tokens * 4 {
            context.truncate(max_tokens * 4);
        }
        context
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new(100, 3600, 1000, 50)
    }
}

/// Context window builder - assembles relevant context for an agent
pub struct ContextBuilder {
    max_tokens_approx: usize,
}

impl ContextBuilder {
    pub fn new(max_tokens_approx: usize) -> Self {
        Self { max_tokens_approx }
    }

    /// Build a context string from memory entries
    pub fn build_context(&self, entries: &[MemoryEntry]) -> String {
        let mut context = String::new();
        let mut approx_tokens = 0;

        for entry in entries {
            let entry_text = format!("[{}] {}\n", entry.agent_id, entry.content);
            let entry_tokens = entry_text.len() / 4; // rough approximation
            if approx_tokens + entry_tokens > self.max_tokens_approx {
                break;
            }
            context.push_str(&entry_text);
            approx_tokens += entry_tokens;
        }

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry_expiry() {
        let entry = MemoryEntry {
            id: "1".to_string(),
            agent_id: "a1".to_string(),
            content: "test".to_string(),
            metadata: serde_json::json!({}),
            tags: vec![],
            relevance: 1.0,
            created_at: Utc::now(),
            expires_at: Some(Utc::now() - Duration::seconds(10)),
            access_count: 0,
            last_accessed: Utc::now(),
        };
        assert!(entry.is_expired());

        let entry_no_expiry = MemoryEntry {
            expires_at: None,
            ..entry
        };
        assert!(!entry_no_expiry.is_expired());
    }

    #[test]
    fn test_short_term_memory_store_and_search() {
        let mut stm = ShortTermMemory::new(100, 3600);
        stm.store(
            "a1",
            "The server is running on port 8080",
            vec!["server".to_string()],
        );
        stm.store(
            "a2",
            "Database connection established",
            vec!["db".to_string()],
        );

        let results = stm.search("server", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("port 8080"));
    }

    #[test]
    fn test_short_term_memory_tag_search() {
        let mut stm = ShortTermMemory::new(100, 3600);
        stm.store("a1", "some content", vec!["important".to_string()]);

        let results = stm.search("important", 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_short_term_memory_eviction() {
        let mut stm = ShortTermMemory::new(3, 3600);
        stm.store("a1", "entry 1", vec![]);
        stm.store("a1", "entry 2", vec![]);
        stm.store("a1", "entry 3", vec![]);
        stm.store("a1", "entry 4", vec![]);

        assert_eq!(stm.len(), 3);
    }

    #[test]
    fn test_short_term_memory_clear() {
        let mut stm = ShortTermMemory::new(100, 3600);
        stm.store("a1", "test", vec![]);
        assert_eq!(stm.len(), 1);
        stm.clear();
        assert_eq!(stm.len(), 0);
    }

    #[test]
    fn test_long_term_memory_store_and_search() {
        let mut ltm = LongTermMemory::new(1000);
        ltm.store(
            "a1",
            "KIAS uses Rust for performance",
            vec!["tech".to_string()],
        );
        ltm.store(
            "a1",
            "The scheduler uses round-robin",
            vec!["scheduler".to_string()],
        );

        let results = ltm.search("Rust", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Rust"));
    }

    #[test]
    fn test_long_term_memory_no_expiry() {
        let mut ltm = LongTermMemory::new(1000);
        let _id = ltm.store("a1", "permanent knowledge", vec![]);
        let results = ltm.search("permanent", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].expires_at.is_none());
    }

    #[test]
    fn test_entity_memory_record_and_retrieve() {
        let mut em = EntityMemory::new(50);
        em.record("worker-1", "capability", "log analysis", 0.9);
        em.record("worker-1", "capability", "code review", 0.8);
        em.record("worker-2", "capability", "data processing", 0.95);

        let w1_facts = em.get_facts("worker-1");
        assert_eq!(w1_facts.len(), 2);

        let w2_caps = em.get_facts_by_type("worker-2", "capability");
        assert_eq!(w2_caps.len(), 1);
    }

    #[test]
    fn test_entity_memory_eviction() {
        let mut em = EntityMemory::new(2);
        em.record("e1", "type", "fact1", 0.5);
        em.record("e1", "type", "fact2", 0.8);
        em.record("e1", "type", "fact3", 0.9);

        let facts = em.get_facts("e1");
        assert_eq!(facts.len(), 2);
        // Higher confidence facts should survive
        let contents: Vec<&str> = facts.iter().map(|f| f.content.as_str()).collect();
        assert!(contents.contains(&"fact3"));
        assert!(contents.contains(&"fact2"));
    }

    #[test]
    fn test_entity_memory_confidence_clamping() {
        let mut em = EntityMemory::new(50);
        em.record("e1", "type", "over", 1.5);
        em.record("e1", "type", "under", -0.5);

        let facts = em.get_facts("e1");
        assert_eq!(facts.len(), 2);
        for fact in &facts {
            assert!((0.0..=1.0).contains(&fact.confidence));
        }
    }

    #[test]
    fn test_memory_manager_default() {
        let mm = MemoryManager::default();
        // Just verify it constructs
        assert!(mm.short_term.try_read().is_ok());
        assert!(mm.long_term.try_read().is_ok());
        assert!(mm.entity.try_read().is_ok());
    }

    #[test]
    fn test_context_builder() {
        let builder = ContextBuilder::new(1000);
        let entries = vec![
            MemoryEntry {
                id: "1".to_string(),
                agent_id: "a1".to_string(),
                content: "first entry".to_string(),
                metadata: serde_json::json!({}),
                tags: vec![],
                relevance: 1.0,
                created_at: Utc::now(),
                expires_at: None,
                access_count: 0,
                last_accessed: Utc::now(),
            },
            MemoryEntry {
                id: "2".to_string(),
                agent_id: "a2".to_string(),
                content: "second entry".to_string(),
                metadata: serde_json::json!({}),
                tags: vec![],
                relevance: 0.8,
                created_at: Utc::now(),
                expires_at: None,
                access_count: 0,
                last_accessed: Utc::now(),
            },
        ];

        let ctx = builder.build_context(&entries);
        assert!(ctx.contains("first entry"));
        assert!(ctx.contains("second entry"));
    }

    #[test]
    fn test_context_builder_token_limit() {
        let builder = ContextBuilder::new(10); // Very small limit
        let entries = vec![
            MemoryEntry {
                id: "1".to_string(),
                agent_id: "a1".to_string(),
                content: "a".repeat(100),
                metadata: serde_json::json!({}),
                tags: vec![],
                relevance: 1.0,
                created_at: Utc::now(),
                expires_at: None,
                access_count: 0,
                last_accessed: Utc::now(),
            },
            MemoryEntry {
                id: "2".to_string(),
                agent_id: "a2".to_string(),
                content: "should not appear".to_string(),
                metadata: serde_json::json!({}),
                tags: vec![],
                relevance: 0.8,
                created_at: Utc::now(),
                expires_at: None,
                access_count: 0,
                last_accessed: Utc::now(),
            },
        ];

        let ctx = builder.build_context(&entries);
        // Second entry should be cut off due to token limit
        assert!(!ctx.contains("should not appear"));
    }

    #[test]
    fn test_memory_entry_touch() {
        let mut entry = MemoryEntry {
            id: "1".to_string(),
            agent_id: "a1".to_string(),
            content: "test".to_string(),
            metadata: serde_json::json!({}),
            tags: vec![],
            relevance: 1.0,
            created_at: Utc::now(),
            expires_at: None,
            access_count: 0,
            last_accessed: Utc::now() - Duration::hours(1),
        };
        let old_access = entry.last_accessed;
        entry.touch();
        assert_eq!(entry.access_count, 1);
        assert!(entry.last_accessed > old_access);
    }

    #[test]
    fn test_entity_memory_empty_entity() {
        let em = EntityMemory::new(50);
        let facts = em.get_facts("nonexistent");
        assert!(facts.is_empty());
        assert_eq!(em.entity_count(), 0);
    }

    #[test]
    fn test_mid_term_memory_add_and_search() {
        let mut mtm = MidTermMemory::new(100);
        mtm.add(MemoryCategory::UserPreference, "User prefers concise responses");
        mtm.add(MemoryCategory::EnvironmentFact, "OS: Ubuntu 22.04");
        mtm.add(MemoryCategory::Convention, "Always use main branch");

        let results = mtm.search("concise", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("concise"));

        let results = mtm.search("Ubuntu", 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_mid_term_memory_replace() {
        let mut mtm = MidTermMemory::new(100);
        let id = mtm.add(MemoryCategory::Convention, "Use master branch");
        assert!(mtm.replace(&id, "Use main branch"));
        let results = mtm.search("main", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("main"));
    }

    #[test]
    fn test_mid_term_memory_remove() {
        let mut mtm = MidTermMemory::new(100);
        let id = mtm.add(MemoryCategory::Correction, "Don't use sed");
        assert_eq!(mtm.len(), 1);
        assert!(mtm.remove(&id));
        assert_eq!(mtm.len(), 0);
    }

    #[test]
    fn test_mid_term_memory_get_by_category() {
        let mut mtm = MidTermMemory::new(100);
        mtm.add(MemoryCategory::UserPreference, "Prefers concise");
        mtm.add(MemoryCategory::UserPreference, "Prefers Chinese");
        mtm.add(MemoryCategory::EnvironmentFact, "OS: Ubuntu");

        let prefs = mtm.get_by_category(&MemoryCategory::UserPreference);
        assert_eq!(prefs.len(), 2);
        let env = mtm.get_by_category(&MemoryCategory::EnvironmentFact);
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn test_mid_term_memory_prompt_context() {
        let mut mtm = MidTermMemory::new(100);
        mtm.add(MemoryCategory::UserPreference, "Prefers concise responses");
        mtm.add(MemoryCategory::Convention, "Always use main branch");

        let ctx = mtm.build_prompt_context();
        assert!(ctx.contains("## Memory (injected)"));
        assert!(ctx.contains("Prefers concise"));
        assert!(ctx.contains("main branch"));
    }

    #[test]
    fn test_mid_term_memory_markdown_roundtrip() {
        let mut mtm = MidTermMemory::new(100);
        mtm.add(MemoryCategory::UserPreference, "Prefers concise");
        mtm.add(MemoryCategory::Convention, "Use main branch");

        let md = mtm.to_markdown();
        assert!(md.contains("# Agent Memory"));
        assert!(md.contains("Prefers concise"));

        let mut mtm2 = MidTermMemory::new(100);
        mtm2.from_markdown(&md);
        assert_eq!(mtm2.len(), 2);
    }

    #[test]
    fn test_mid_term_memory_eviction() {
        let mut mtm = MidTermMemory::new(3);
        mtm.add(MemoryCategory::Convention, "entry 1");
        mtm.add(MemoryCategory::Convention, "entry 2");
        mtm.add(MemoryCategory::Convention, "entry 3");
        mtm.add(MemoryCategory::Convention, "entry 4");
        assert_eq!(mtm.len(), 3);
    }
}
