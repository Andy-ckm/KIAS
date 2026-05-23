//! # Layered Cache System
//!
//! A four-tier caching architecture: L1(内存LRU) + L2(语义缓存) + L3(结果缓存) + L4(工具结果缓存).
//! Each layer has independent TTL and eviction policies with hit-rate statistics.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// Cache layer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheLayer {
    L1Memory,
    L2Semantic,
    L3Result,
    L4Tool,
}

impl std::fmt::Display for CacheLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheLayer::L1Memory => write!(f, "L1(Memory)"),
            CacheLayer::L2Semantic => write!(f, "L2(Semantic)"),
            CacheLayer::L3Result => write!(f, "L3(Result)"),
            CacheLayer::L4Tool => write!(f, "L4(Tool)"),
        }
    }
}

/// Layer configuration
#[derive(Debug, Clone)]
pub struct LayerConfig {
    pub name: String,
    pub ttl: std::time::Duration,
    pub max_entries: usize,
    pub eviction_policy: EvictionPolicy,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            ttl: std::time::Duration::from_secs(300),
            max_entries: 1000,
            eviction_policy: EvictionPolicy::Lru,
        }
    }
}

impl LayerConfig {
    pub fn new(name: &str, ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            name: name.to_string(),
            ttl: std::time::Duration::from_secs(ttl_secs),
            max_entries,
            eviction_policy: EvictionPolicy::Lru,
        }
    }
}

/// Eviction policy for a cache layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Fifo,
    Ttl,
}

/// A cached entry with metadata
#[derive(Debug, Clone)]
pub struct LayeredCacheEntry<V> {
    pub key: String,
    pub value: V,
    pub created_at: std::time::Instant,
    pub ttl: std::time::Duration,
    pub access_count: usize,
    pub last_access: std::time::Instant,
}

impl<V: Clone> LayeredCacheEntry<V> {
    pub fn new(key: String, value: V, ttl: std::time::Duration) -> Self {
        let now = std::time::Instant::now();
        Self {
            key,
            value,
            created_at: now,
            ttl,
            access_count: 0,
            last_access: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_access = std::time::Instant::now();
    }
}

// ── L1: In-memory LRU cache ─────────────────────────────────────────────────

pub struct L1MemoryCache<K, V> {
    config: LayerConfig,
    lru_order: VecDeque<K>,
    entries: HashMap<K, LayeredCacheEntry<V>>,
}

impl<K: Eq + Hash + Clone + std::fmt::Display, V: Clone> L1MemoryCache<K, V> {
    pub fn new(config: LayerConfig) -> Self {
        Self {
            config,
            lru_order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get_mut(key) {
            if entry.is_expired() {
                self.evict(key);
                return None;
            }
            entry.touch();
            // Move to end (most recently used)
            self.lru_order.retain(|k| k != key);
            self.lru_order.push_back(key.clone());
            return Some(entry.value.clone());
        }
        None
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.contains_key(&key) {
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.value = value;
                entry.touch();
            }
            self.lru_order.retain(|k| k != &key);
            self.lru_order.push_back(key);
            return;
        }

        // Evict if at capacity
        while self.entries.len() >= self.config.max_entries {
            if let Some(oldest) = self.lru_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }

        let entry = LayeredCacheEntry::new(key.to_string(), value, self.config.ttl);
        self.entries.insert(key.clone(), entry);
        self.lru_order.push_back(key);
    }

    fn evict(&mut self, key: &K) {
        self.entries.remove(key);
        self.lru_order.retain(|k| k != key);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
    }
}

// ── L2: Semantic cache (embedding-based similarity) ────────────────────────

#[derive(Debug, Clone)]
pub struct SemanticCacheEntry<V> {
    pub key: String,
    pub embedding: Vec<f32>,
    pub value: V,
    pub ttl: std::time::Duration,
    pub created_at: std::time::Instant,
    pub similarity_threshold: f32,
}

impl<V: Clone> SemanticCacheEntry<V> {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// L2 semantic cache using cosine similarity
pub struct L2SemanticCache<V> {
    config: LayerConfig,
    entries: Vec<SemanticCacheEntry<V>>,
    similarity_threshold: f32,
}

impl<V: Clone> L2SemanticCache<V> {
    pub fn new(config: LayerConfig, similarity_threshold: f32) -> Self {
        Self {
            config,
            entries: Vec::new(),
            similarity_threshold,
        }
    }

    /// Compute cosine similarity between two embedding vectors
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }

    pub fn get(&mut self, embedding: &[f32]) -> Option<V> {
        // Evict expired entries first
        self.entries.retain(|e| !e.is_expired());

        // Find best matching entry
        let mut best_idx = None;
        let mut best_score = self.similarity_threshold;

        for (i, entry) in self.entries.iter().enumerate() {
            let score = Self::cosine_similarity(embedding, &entry.embedding);
            if score > best_score {
                best_score = score;
                best_idx = Some(i);
            }
        }

        best_idx.and_then(|i| self.entries.get(i).map(|e| e.value.clone()))
    }

    pub fn insert(&mut self, key: String, embedding: Vec<f32>, value: V) {
        // Evict expired or oldest if at capacity
        self.entries.retain(|e| !e.is_expired());
        while self.entries.len() >= self.config.max_entries {
            self.entries.remove(0);
        }

        let entry = SemanticCacheEntry {
            key,
            embedding,
            value,
            ttl: self.config.ttl,
            created_at: std::time::Instant::now(),
            similarity_threshold: self.similarity_threshold,
        };
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── L3: Result cache (KV store with TTL) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultCacheEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ttl_secs: i64,
}

impl ResultCacheEntry {
    pub fn is_expired(&self) -> bool {
        let elapsed = chrono::Utc::now() - self.created_at;
        elapsed.num_seconds() > self.ttl_secs
    }
}

pub struct L3ResultCache {
    config: LayerConfig,
    entries: HashMap<String, ResultCacheEntry>,
}

impl L3ResultCache {
    pub fn new(config: LayerConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                return None;
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub fn insert(&mut self, key: String, value: Vec<u8>) {
        // Evict expired entries first
        self.entries.retain(|_, e| !e.is_expired());

        // Evict oldest if at capacity
        if self.entries.len() >= self.config.max_entries {
            if let Some(oldest_key) = self.entries.keys().next().cloned() {
                self.entries.remove(&oldest_key);
            }
        }

        let entry = ResultCacheEntry {
            key: key.clone(),
            value,
            created_at: chrono::Utc::now(),
            ttl_secs: self.config.ttl.as_secs() as i64,
        };
        self.entries.insert(key, entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── L4: Tool result cache ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolCacheEntry<V> {
    pub tool_name: String,
    pub input_hash: String,
    pub output: V,
    pub created_at: std::time::Instant,
    pub ttl: std::time::Duration,
}

impl<V: Clone> ToolCacheEntry<V> {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

pub struct L4ToolCache<V> {
    config: LayerConfig,
    entries: HashMap<String, ToolCacheEntry<V>>,
}

impl<V: Clone> L4ToolCache<V> {
    pub fn new(config: LayerConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
        }
    }

    fn make_key(tool_name: &str, input_hash: &str) -> String {
        format!("{}:{}", tool_name, input_hash)
    }

    pub fn get(&self, tool_name: &str, input_hash: &str) -> Option<V> {
        let key = Self::make_key(tool_name, input_hash);
        if let Some(entry) = self.entries.get(&key) {
            if entry.is_expired() {
                return None;
            }
            return Some(entry.output.clone());
        }
        None
    }

    pub fn insert(&mut self, tool_name: String, input_hash: String, output: V) {
        // Evict expired
        self.entries.retain(|_, e| !e.is_expired());

        // Evict oldest if at capacity
        if self.entries.len() >= self.config.max_entries {
            if let Some(oldest_key) = self.entries.keys().next().cloned() {
                self.entries.remove(&oldest_key);
            }
        }

        let entry = ToolCacheEntry {
            tool_name: tool_name.clone(),
            input_hash: input_hash.clone(),
            output,
            created_at: std::time::Instant::now(),
            ttl: self.config.ttl,
        };
        let key = Self::make_key(&tool_name, &input_hash);
        self.entries.insert(key, entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── Cache statistics ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub layer_hits: HashMap<CacheLayer, u64>,
    pub layer_misses: HashMap<CacheLayer, u64>,
    pub layer_inserts: HashMap<CacheLayer, u64>,
    pub layer_evictions: HashMap<CacheLayer, u64>,
}

impl CacheStats {
    pub fn hit_rate(&self, layer: CacheLayer) -> f64 {
        let hits = *self.layer_hits.get(&layer).unwrap_or(&0) as f64;
        let misses = *self.layer_misses.get(&layer).unwrap_or(&0) as f64;
        let total = hits + misses;
        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    pub fn record_hit(&mut self, layer: CacheLayer) {
        *self.layer_hits.entry(layer).or_insert(0) += 1;
    }

    pub fn record_miss(&mut self, layer: CacheLayer) {
        *self.layer_misses.entry(layer).or_insert(0) += 1;
    }

    pub fn record_insert(&mut self, layer: CacheLayer) {
        *self.layer_inserts.entry(layer).or_insert(0) += 1;
    }

    pub fn record_eviction(&mut self, layer: CacheLayer) {
        *self.layer_evictions.entry(layer).or_insert(0) += 1;
    }
}

// ── Layered Cache (unified 4-layer cache) ────────────────────────────────────

pub struct LayeredCache {
    l1: Arc<RwLock<L1MemoryCache<String, Vec<u8>>>>,
    l2: Arc<RwLock<L2SemanticCache<Vec<u8>>>>,
    l3: Arc<RwLock<L3ResultCache>>,
    l4: Arc<RwLock<L4ToolCache<Vec<u8>>>>,
    stats: Arc<RwLock<CacheStats>>,
}

impl Default for LayeredCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LayeredCache {
    pub fn new() -> Self {
        let l1_config = LayerConfig::new("L1-Memory", 60, 5000);
        let l2_config = LayerConfig::new("L2-Semantic", 300, 2000);
        let l3_config = LayerConfig::new("L3-Result", 600, 1000);
        let l4_config = LayerConfig::new("L4-Tool", 3600, 500);

        Self {
            l1: Arc::new(RwLock::new(L1MemoryCache::new(l1_config))),
            l2: Arc::new(RwLock::new(L2SemanticCache::new(l2_config, 0.85))),
            l3: Arc::new(RwLock::new(L3ResultCache::new(l3_config))),
            l4: Arc::new(RwLock::new(L4ToolCache::new(l4_config))),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get from L1 first, then cascade to lower layers
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        // L1: Memory LRU
        {
            let mut l1 = self.l1.write().await;
            if let Some(v) = l1.get(&key.to_string()) {
                let mut stats = self.stats.write().await;
                stats.record_hit(CacheLayer::L1Memory);
                return Some(v);
            }
        }
        {
            let mut stats = self.stats.write().await;
            stats.record_miss(CacheLayer::L1Memory);
        }

        // L2: Semantic — string key cannot query embedding index, record miss
        {
            let l2_len = { self.l2.read().await.len() };
            if l2_len > 0 {
                let mut stats = self.stats.write().await;
                stats.record_miss(CacheLayer::L2Semantic);
            }
        }

        // L3: Result cache
        {
            let l3 = self.l3.read().await;
            if let Some(v) = l3.get(key) {
                let mut stats = self.stats.write().await;
                stats.record_hit(CacheLayer::L3Result);
                // Populate L1 on L3 hit
                drop(l3);
                let mut l1 = self.l1.write().await;
                l1.insert(key.to_string(), v.clone());
                return Some(v);
            }
        }
        {
            let mut stats = self.stats.write().await;
            stats.record_miss(CacheLayer::L3Result);
        }

        None
    }

    /// Insert into all layers
    pub async fn insert(&self, key: String, value: Vec<u8>) {
        // L1
        {
            let mut l1 = self.l1.write().await;
            l1.insert(key.clone(), value.clone());
            let mut stats = self.stats.write().await;
            stats.record_insert(CacheLayer::L1Memory);
        }
        // L3
        {
            let mut l3 = self.l3.write().await;
            l3.insert(key.clone(), value.clone());
            let mut stats = self.stats.write().await;
            stats.record_insert(CacheLayer::L3Result);
        }
    }

    /// Insert into L2 semantic cache
    pub async fn insert_semantic(&self, key: String, embedding: Vec<f32>, value: Vec<u8>) {
        let mut l2 = self.l2.write().await;
        l2.insert(key, embedding, value);
        let mut stats = self.stats.write().await;
        stats.record_insert(CacheLayer::L2Semantic);
    }

    /// Insert into L4 tool cache
    pub async fn insert_tool(&self, tool_name: String, input_hash: String, output: Vec<u8>) {
        let mut l4 = self.l4.write().await;
        l4.insert(tool_name, input_hash, output);
        let mut stats = self.stats.write().await;
        stats.record_insert(CacheLayer::L4Tool);
    }

    /// Get from L4 tool cache
    pub async fn get_tool(&self, tool_name: &str, input_hash: &str) -> Option<Vec<u8>> {
        let l4 = self.l4.read().await;
        let result = l4.get(tool_name, input_hash);
        let mut stats = self.stats.write().await;
        if result.is_some() {
            stats.record_hit(CacheLayer::L4Tool);
        } else {
            stats.record_miss(CacheLayer::L4Tool);
        }
        result
    }

    /// Get statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Clear all layers
    pub async fn clear(&self) {
        self.l1.write().await.clear();
        self.l2.write().await.clear();
        self.l3.write().await.clear();
        self.l4.write().await.clear();
    }

    /// Get size of each layer
    pub async fn layer_sizes(&self) -> HashMap<CacheLayer, usize> {
        let mut sizes = HashMap::new();
        sizes.insert(CacheLayer::L1Memory, self.l1.read().await.len());
        sizes.insert(CacheLayer::L2Semantic, self.l2.read().await.len());
        sizes.insert(CacheLayer::L3Result, self.l3.read().await.len());
        sizes.insert(CacheLayer::L4Tool, self.l4.read().await.len());
        sizes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_l1_memory_cache_basic() {
        let config = LayerConfig::new("L1", 60, 3);
        let mut cache = L1MemoryCache::new(config);

        cache.insert("a".to_string(), b"1".to_vec());
        cache.insert("b".to_string(), b"2".to_vec());
        assert_eq!(cache.get(&"a".to_string()), Some(b"1".to_vec()));
        assert_eq!(cache.get(&"b".to_string()), Some(b"2".to_vec()));
        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn test_l1_lru_eviction() {
        let config = LayerConfig::new("L1", 60, 2);
        let mut cache = L1MemoryCache::new(config);

        cache.insert("a".to_string(), b"1".to_vec());
        cache.insert("b".to_string(), b"2".to_vec());
        cache.insert("c".to_string(), b"3".to_vec()); // Should evict "a"

        assert_eq!(cache.get(&"a".to_string()), None);
        assert_eq!(cache.get(&"b".to_string()), Some(b"2".to_vec()));
        assert_eq!(cache.get(&"c".to_string()), Some(b"3".to_vec()));
    }

    #[tokio::test]
    async fn test_l1_ttl_expiry() {
        let config = LayerConfig {
            name: "L1".to_string(),
            ttl: std::time::Duration::from_millis(10),
            max_entries: 10,
            eviction_policy: EvictionPolicy::Lru,
        };
        let mut cache = L1MemoryCache::new(config);

        cache.insert("a".to_string(), b"1".to_vec());
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(cache.get(&"a".to_string()), None);
    }

    #[tokio::test]
    async fn test_l1_access_count() {
        let config = LayerConfig::new("L1", 60, 10);
        let mut cache = L1MemoryCache::new(config);

        cache.insert("a".to_string(), b"1".to_vec());
        cache.get(&"a".to_string());
        cache.get(&"a".to_string());
        cache.get(&"a".to_string());

        let entry = cache.entries.get(&"a".to_string()).unwrap();
        assert_eq!(entry.access_count, 3);
    }

    #[tokio::test]
    async fn test_l2_semantic_similarity() {
        let config = LayerConfig::new("L2", 60, 10);
        let mut cache = L2SemanticCache::new(config, 0.85);

        let emb1 = vec![1.0, 0.0, 0.0];
        let emb2 = vec![0.9, 0.1, 0.0];
        let emb3 = vec![0.0, 1.0, 0.0];

        cache.insert("key1".to_string(), emb1.clone(), b"value1".to_vec());

        // Similar embedding should hit
        let result = cache.get(&emb2);
        assert!(result.is_some());

        // Dissimilar embedding should miss
        let result = cache.get(&emb3);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_l3_result_cache() {
        let config = LayerConfig::new("L3", 60, 3);
        let mut cache = L3ResultCache::new(config);

        cache.insert("key1".to_string(), b"value1".to_vec());
        cache.insert("key2".to_string(), b"value2".to_vec());

        assert_eq!(cache.get("key1"), Some(b"value1".to_vec()));
        assert_eq!(cache.get("key2"), Some(b"value2".to_vec()));
        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn test_l4_tool_cache() {
        let config = LayerConfig::new("L4", 60, 5);
        let mut cache = L4ToolCache::new(config);

        cache.insert(
            "web_search".to_string(),
            "hash1".to_string(),
            b"result1".to_vec(),
        );
        cache.insert(
            "calculator".to_string(),
            "hash2".to_string(),
            b"result2".to_vec(),
        );

        assert_eq!(cache.get("web_search", "hash1"), Some(b"result1".to_vec()));
        assert_eq!(cache.get("calculator", "hash2"), Some(b"result2".to_vec()));
        assert_eq!(cache.get("web_search", "wrong_hash"), None);
    }

    #[tokio::test]
    async fn test_layered_cache_l1_hit() {
        let cache = LayeredCache::new();
        cache.insert("key1".to_string(), b"value1".to_vec()).await;
        let result = cache.get("key1").await;
        assert_eq!(result, Some(b"value1".to_vec()));
        let stats = cache.stats().await;
        assert!(stats.hit_rate(CacheLayer::L1Memory) > 0.0);
    }

    #[tokio::test]
    async fn test_layered_cache_l1_miss() {
        let cache = LayeredCache::new();
        let result = cache.get("nonexistent").await;
        assert_eq!(result, None);
        let stats = cache.stats().await;
        assert_eq!(stats.hit_rate(CacheLayer::L1Memory), 0.0);
    }

    #[tokio::test]
    async fn test_cache_stats_recording() {
        let cache = LayeredCache::new();
        cache.insert("k1".to_string(), b"v1".to_vec()).await;
        cache.insert("k2".to_string(), b"v2".to_vec()).await;
        cache.get("k1").await;
        cache.get("nonexistent").await;

        let stats = cache.stats().await;
        assert_eq!(
            *stats.layer_inserts.get(&CacheLayer::L1Memory).unwrap_or(&0),
            2
        );
        assert_eq!(
            *stats.layer_hits.get(&CacheLayer::L1Memory).unwrap_or(&0),
            1
        );
        assert_eq!(
            *stats.layer_misses.get(&CacheLayer::L1Memory).unwrap_or(&0),
            1
        );
    }
}
