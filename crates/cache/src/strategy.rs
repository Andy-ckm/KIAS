use async_trait::async_trait;
use kias_common::KiasResult;
use super::hub::{CacheStrategy, CacheEntry};
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use std::time::Instant;

/// Real LRU Cache Strategy with access-order tracking, capacity limits, and TTL expiry.
///
/// Uses a `VecDeque` to maintain access order — on every `get`, the key is moved
/// to the back (most recently used). On eviction, the front (least recently used)
/// entry is removed first.
pub struct LRUStrategy {
    cache: RwLock<HashMap<String, CacheEntry>>,
    /// Access order: back = most recently used, front = least recently used
    access_order: RwLock<VecDeque<String>>,
    /// Maximum number of entries (0 = unlimited)
    max_capacity: usize,
    /// When entries were inserted (for TTL checks)
    insert_times: RwLock<HashMap<String, Instant>>,
}

impl Default for LRUStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl LRUStrategy {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            access_order: RwLock::new(VecDeque::new()),
            max_capacity: 1000,
            insert_times: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            access_order: RwLock::new(VecDeque::new()),
            max_capacity,
            insert_times: RwLock::new(HashMap::new()),
        }
    }

    /// Touch a key — move it to the back of the access order (most recently used)
    fn touch_key(&self, key: &str) {
        let mut order = self.access_order.write().unwrap();
        order.retain(|k| k != key);
        order.push_back(key.to_string());
    }

    /// Remove the least recently used entry
    fn evict_lru(&self) {
        let lru_key = {
            let order = self.access_order.read().unwrap();
            order.front().cloned()
        };
        if let Some(key) = lru_key {
            let mut cache = self.cache.write().unwrap();
            let mut order = self.access_order.write().unwrap();
            let mut times = self.insert_times.write().unwrap();
            cache.remove(&key);
            order.retain(|k| k != &key);
            times.remove(&key);
        }
    }

    /// Check if an entry has expired based on TTL
    fn is_expired(&self, key: &str, entry: &CacheEntry) -> bool {
        if let Some(ttl) = entry.ttl {
            let times = self.insert_times.read().unwrap();
            if let Some(insert_time) = times.get(key) {
                return insert_time.elapsed() > ttl;
            }
        }
        false
    }

    /// Purge all expired entries
    fn purge_expired(&self) {
        let expired_keys: Vec<String> = {
            let cache = self.cache.read().unwrap();
            cache.iter()
                .filter(|(k, v)| self.is_expired(k, v))
                .map(|(k, _)| k.clone())
                .collect()
        };
        if !expired_keys.is_empty() {
            let mut cache = self.cache.write().unwrap();
            let mut order = self.access_order.write().unwrap();
            let mut times = self.insert_times.write().unwrap();
            for key in &expired_keys {
                cache.remove(key);
                order.retain(|k| k != key);
                times.remove(key);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.read().unwrap().is_empty()
    }
}

#[async_trait]
impl CacheStrategy for LRUStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        self.purge_expired();
        let cache = self.cache.read().unwrap();
        if let Some(entry) = cache.get(key) {
            if self.is_expired(key, entry) {
                drop(cache);
                // Remove expired entry
                let mut cache = self.cache.write().unwrap();
                let mut order = self.access_order.write().unwrap();
                let mut times = self.insert_times.write().unwrap();
                cache.remove(key);
                order.retain(|k| k != key);
                times.remove(key);
                return Ok(None);
            }
            let entry = entry.clone();
            drop(cache);
            self.touch_key(key);
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let key = entry.key.clone();
        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(&key) {
                drop(cache);
                // Update existing entry
                let mut cache = self.cache.write().unwrap();
                cache.insert(key.clone(), entry);
                self.touch_key(&key);
                let mut times = self.insert_times.write().unwrap();
                times.insert(key, Instant::now());
                return Ok(());
            }
        }
        // New entry — check capacity
        if self.max_capacity > 0 {
            let cache = self.cache.read().unwrap();
            if cache.len() >= self.max_capacity {
                drop(cache);
                self.evict_lru();
            }
        }
        let mut cache = self.cache.write().unwrap();
        cache.insert(key.clone(), entry);
        self.touch_key(&key);
        let mut times = self.insert_times.write().unwrap();
        times.insert(key, Instant::now());
        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let mut cache = self.cache.write().unwrap();
        let mut order = self.access_order.write().unwrap();
        let mut times = self.insert_times.write().unwrap();
        cache.remove(key);
        order.retain(|k| k != key);
        times.remove(key);
        Ok(())
    }
}

/// Prefix Cache Strategy for KV Cache optimization.
///
/// Designed for LLM inference prefix caching (DeepSeek-style):
/// - Keys are prefix hashes of prompts
/// - A lookup matches if any cached key is a prefix of the query key
///   (i.e., the cached prompt is a prefix of the requested prompt)
/// - This enables KV Cache reuse across requests with shared prefixes
pub struct PrefixCacheStrategy {
    cache: RwLock<HashMap<String, CacheEntry>>,
    /// Track access counts for popularity-based eviction
    access_counts: RwLock<HashMap<String, u64>>,
    max_capacity: usize,
}

impl Default for PrefixCacheStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefixCacheStrategy {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            access_counts: RwLock::new(HashMap::new()),
            max_capacity: 500,
        }
    }

    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            access_counts: RwLock::new(HashMap::new()),
            max_capacity,
        }
    }

    /// Find the longest matching prefix key for a given query.
    /// Returns the key whose value is a prefix of `query_key`.
    fn find_longest_prefix(&self, query_key: &str) -> Option<String> {
        let cache = self.cache.read().unwrap();
        let mut best_match: Option<(String, usize)> = None;
        for k in cache.keys() {
            // The cached key is a prefix of the query — meaning the query extends the cached prompt
            if query_key.starts_with(k.as_str()) {
                let match_len = k.len();
                if best_match.as_ref().is_none_or(|(_, len)| match_len > *len) {
                    best_match = Some((k.clone(), match_len));
                }
            }
        }
        best_match.map(|(k, _)| k)
    }

    /// Evict the least-accessed entry
    fn evict_least_popular(&self) {
        let least_key = {
            let counts = self.access_counts.read().unwrap();
            counts.iter()
                .min_by_key(|(_, &count)| count)
                .map(|(k, _)| k.clone())
        };
        if let Some(key) = least_key {
            let mut cache = self.cache.write().unwrap();
            let mut counts = self.access_counts.write().unwrap();
            cache.remove(&key);
            counts.remove(&key);
        }
    }

    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.read().unwrap().is_empty()
    }
}

#[async_trait]
impl CacheStrategy for PrefixCacheStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        // Try exact match first
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(key) {
                let entry = entry.clone();
                drop(cache);
                let mut counts = self.access_counts.write().unwrap();
                *counts.entry(key.to_string()).or_insert(0) += 1;
                return Ok(Some(entry));
            }
        }
        // Try prefix match (longest prefix wins)
        if let Some(prefix_key) = self.find_longest_prefix(key) {
            let cache = self.cache.read().unwrap();
            let entry = cache.get(&prefix_key).cloned();
            drop(cache);
            if entry.is_some() {
                let mut counts = self.access_counts.write().unwrap();
                *counts.entry(prefix_key).or_insert(0) += 1;
            }
            return Ok(entry);
        }
        Ok(None)
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let key = entry.key.clone();
        if self.max_capacity > 0 {
            let cache = self.cache.read().unwrap();
            if cache.len() >= self.max_capacity && !cache.contains_key(&key) {
                drop(cache);
                self.evict_least_popular();
            }
        }
        let mut cache = self.cache.write().unwrap();
        cache.insert(key.clone(), entry);
        // Ensure new keys are tracked in access_counts with 0
        let mut counts = self.access_counts.write().unwrap();
        counts.entry(key).or_insert(0);
        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let mut cache = self.cache.write().unwrap();
        let mut counts = self.access_counts.write().unwrap();
        cache.remove(key);
        counts.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::CacheHub;
    use std::time::Duration;

    fn make_entry(key: &str, value: &[u8]) -> CacheEntry {
        CacheEntry {
            key: key.to_string(),
            value: value.to_vec(),
            created_at: chrono::Utc::now(),
            ttl: None,
        }
    }

    fn make_entry_with_ttl(key: &str, value: &[u8], ttl: Duration) -> CacheEntry {
        CacheEntry {
            key: key.to_string(),
            value: value.to_vec(),
            created_at: chrono::Utc::now(),
            ttl: Some(ttl),
        }
    }

    // ===== LRU Strategy Tests =====

    #[tokio::test]
    async fn test_lru_set_and_get() {
        let strategy = LRUStrategy::new();
        strategy.set(make_entry("k1", b"hello")).await.unwrap();
        let result = strategy.get("k1").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, b"hello");
    }

    #[tokio::test]
    async fn test_lru_get_miss() {
        let strategy = LRUStrategy::new();
        let result = strategy.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lru_evict() {
        let strategy = LRUStrategy::new();
        strategy.set(make_entry("k1", b"data")).await.unwrap();
        strategy.evict("k1").await.unwrap();
        let result = strategy.get("k1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lru_capacity_eviction() {
        let strategy = LRUStrategy::with_capacity(3);
        strategy.set(make_entry("a", b"1")).await.unwrap();
        strategy.set(make_entry("b", b"2")).await.unwrap();
        strategy.set(make_entry("c", b"3")).await.unwrap();
        // Now at capacity — inserting "d" should evict "a" (LRU)
        strategy.set(make_entry("d", b"4")).await.unwrap();
        assert_eq!(strategy.len(), 3);
        assert!(strategy.get("a").await.unwrap().is_none());
        assert!(strategy.get("d").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_lru_access_order() {
        let strategy = LRUStrategy::with_capacity(3);
        strategy.set(make_entry("a", b"1")).await.unwrap();
        strategy.set(make_entry("b", b"2")).await.unwrap();
        strategy.set(make_entry("c", b"3")).await.unwrap();
        // Access "a" — now "a" is MRU, "b" is LRU
        strategy.get("a").await.unwrap();
        // Insert "d" — should evict "b" (LRU)
        strategy.set(make_entry("d", b"4")).await.unwrap();
        assert!(strategy.get("a").await.unwrap().is_some());
        assert!(strategy.get("b").await.unwrap().is_none());
        assert!(strategy.get("c").await.unwrap().is_some());
        assert!(strategy.get("d").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_lru_update_doesnt_increase_count() {
        let strategy = LRUStrategy::with_capacity(3);
        strategy.set(make_entry("a", b"1")).await.unwrap();
        strategy.set(make_entry("b", b"2")).await.unwrap();
        strategy.set(make_entry("c", b"3")).await.unwrap();
        // Update "a" — now "a" is MRU
        strategy.set(make_entry("a", b"updated")).await.unwrap();
        // Insert "d" — should evict "b" (LRU)
        strategy.set(make_entry("d", b"4")).await.unwrap();
        assert_eq!(strategy.len(), 3);
        assert_eq!(strategy.get("a").await.unwrap().unwrap().value, b"updated");
        assert!(strategy.get("b").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_lru_ttl_expiry() {
        let strategy = LRUStrategy::new();
        // Entry with 1ms TTL
        strategy.set(make_entry_with_ttl("k1", b"data", Duration::from_millis(1))).await.unwrap();
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = strategy.get("k1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lru_ttl_not_expired() {
        let strategy = LRUStrategy::new();
        strategy.set(make_entry_with_ttl("k1", b"data", Duration::from_secs(60))).await.unwrap();
        let result = strategy.get("k1").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_lru_len_and_empty() {
        let strategy = LRUStrategy::new();
        assert!(strategy.is_empty());
        assert_eq!(strategy.len(), 0);
        strategy.set(make_entry("k1", b"v1")).await.unwrap();
        assert_eq!(strategy.len(), 1);
        assert!(!strategy.is_empty());
    }

    // ===== Prefix Cache Strategy Tests =====

    #[tokio::test]
    async fn test_prefix_exact_match() {
        let strategy = PrefixCacheStrategy::new();
        strategy.set(make_entry("hello", b"cached")).await.unwrap();
        let result = strategy.get("hello").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, b"cached");
    }

    #[tokio::test]
    async fn test_prefix_match() {
        let strategy = PrefixCacheStrategy::new();
        // Cache the prefix "hello"
        strategy.set(make_entry("hello", b"prefix_cache")).await.unwrap();
        // Query with a longer key that starts with "hello"
        let result = strategy.get("hello_world_extra").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, b"prefix_cache");
    }

    #[tokio::test]
    async fn test_prefix_no_match() {
        let strategy = PrefixCacheStrategy::new();
        strategy.set(make_entry("hello", b"data")).await.unwrap();
        let result = strategy.get("goodbye").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_prefix_longest_match() {
        let strategy = PrefixCacheStrategy::new();
        strategy.set(make_entry("h", b"short")).await.unwrap();
        strategy.set(make_entry("hello", b"longer")).await.unwrap();
        // Should match the longest prefix "hello"
        let result = strategy.get("hello_world").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, b"longer");
    }

    #[tokio::test]
    async fn test_prefix_eviction() {
        let strategy = PrefixCacheStrategy::with_capacity(2);
        strategy.set(make_entry("a", b"1")).await.unwrap();
        strategy.set(make_entry("b", b"2")).await.unwrap();
        // Access "a" to increase its count
        strategy.get("a").await.unwrap();
        strategy.get("a").await.unwrap();
        // Insert "c" — should evict "b" (least accessed)
        strategy.set(make_entry("c", b"3")).await.unwrap();
        assert_eq!(strategy.len(), 2);
        assert!(strategy.get("a").await.unwrap().is_some());
        assert!(strategy.get("b").await.unwrap().is_none());
        assert!(strategy.get("c").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_prefix_cache_hub_integration() {
        let strategy = Box::new(PrefixCacheStrategy::new());
        let hub = CacheHub::new(strategy);
        hub.set(make_entry("prompt_prefix", b"kv_cache")).await.unwrap();
        let result = hub.get("prompt_prefix_extended").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_lru_cache_hub_integration() {
        let strategy = Box::new(LRUStrategy::new());
        let hub = CacheHub::new(strategy);
        let entry = CacheEntry {
            key: "test".to_string(),
            value: b"value".to_vec(),
            created_at: chrono::Utc::now(),
            ttl: Some(Duration::from_secs(60)),
        };
        hub.set(entry).await.unwrap();
        let result = hub.get("test").await.unwrap();
        assert!(result.is_some());
    }
}
