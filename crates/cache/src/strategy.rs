use super::hub::{CacheEntry, CacheStrategy};
use async_trait::async_trait;
use kias_common::{KiasError, KiasResult};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
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
    fn touch_key(&self, key: &str) -> KiasResult<()> {
        let mut order = self
            .access_order
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        order.retain(|k| k != key);
        order.push_back(key.to_string());
        Ok(())
    }

    /// Remove the least recently used entry
    fn evict_lru(&self) -> KiasResult<()> {
        let lru_key = {
            let order = self
                .access_order
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            order.front().cloned()
        };
        if let Some(key) = lru_key {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let mut order = self
                .access_order
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let mut times = self
                .insert_times
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            cache.remove(&key);
            order.retain(|k| k != &key);
            times.remove(&key);
        }
        Ok(())
    }

    /// Check if an entry has expired based on TTL
    fn is_expired(&self, key: &str, entry: &CacheEntry) -> KiasResult<bool> {
        if let Some(ttl) = entry.ttl {
            let times = self
                .insert_times
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if let Some(insert_time) = times.get(key) {
                return Ok(insert_time.elapsed() > ttl);
            }
        }
        Ok(false)
    }

    /// Purge all expired entries
    fn purge_expired(&self) -> KiasResult<()> {
        let expired_keys: Vec<String> = {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            cache
                .iter()
                .filter(|(k, v)| self.is_expired(k, v).unwrap_or(false))
                .map(|(k, _)| k.clone())
                .collect()
        };
        if !expired_keys.is_empty() {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let mut order = self
                .access_order
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let mut times = self
                .insert_times
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            for key in &expired_keys {
                cache.remove(key);
                order.retain(|k| k != key);
                times.remove(key);
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.cache.read().ok().map_or(0, |g| g.len())
    }

    pub fn is_empty(&self) -> bool {
        self.cache.read().ok().is_none_or(|g| g.is_empty())
    }
}

#[async_trait]
impl CacheStrategy for LRUStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        self.purge_expired()?;
        let cache = self
            .cache
            .read()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        if let Some(entry) = cache.get(key) {
            if self.is_expired(key, entry)? {
                drop(cache);
                // Remove expired entry
                let mut cache = self
                    .cache
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                let mut order = self
                    .access_order
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                let mut times = self
                    .insert_times
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                cache.remove(key);
                order.retain(|k| k != key);
                times.remove(key);
                return Ok(None);
            }
            let entry = entry.clone();
            drop(cache);
            self.touch_key(key)?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let key = entry.key.clone();
        {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if cache.contains_key(&key) {
                drop(cache);
                // Update existing entry
                let mut cache = self
                    .cache
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                cache.insert(key.clone(), entry);
                self.touch_key(&key)?;
                let mut times = self
                    .insert_times
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                times.insert(key, Instant::now());
                return Ok(());
            }
        }
        // New entry — check capacity
        if self.max_capacity > 0 {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if cache.len() >= self.max_capacity {
                drop(cache);
                self.evict_lru()?;
            }
        }
        let mut cache = self
            .cache
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        cache.insert(key.clone(), entry);
        self.touch_key(&key)?;
        let mut times = self
            .insert_times
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        times.insert(key, Instant::now());
        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let mut cache = self
            .cache
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        let mut order = self
            .access_order
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        let mut times = self
            .insert_times
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
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
    fn find_longest_prefix(&self, query_key: &str) -> KiasResult<Option<String>> {
        let cache = self
            .cache
            .read()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
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
        Ok(best_match.map(|(k, _)| k))
    }

    /// Evict the least-accessed entry
    fn evict_least_popular(&self) -> KiasResult<()> {
        let least_key = {
            let counts = self
                .access_counts
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            counts
                .iter()
                .min_by_key(|(_, &count)| count)
                .map(|(k, _)| k.clone())
        };
        if let Some(key) = least_key {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let mut counts = self
                .access_counts
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            cache.remove(&key);
            counts.remove(&key);
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.cache.read().ok().map_or(0, |g| g.len())
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.read().ok().is_none_or(|g| g.is_empty())
    }
}

#[async_trait]
impl CacheStrategy for PrefixCacheStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        // Try exact match first
        {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if let Some(entry) = cache.get(key) {
                let entry = entry.clone();
                drop(cache);
                let mut counts = self
                    .access_counts
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                *counts.entry(key.to_string()).or_insert(0) += 1;
                return Ok(Some(entry));
            }
        }
        // Try prefix match (longest prefix wins)
        if let Some(prefix_key) = self.find_longest_prefix(key)? {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            let entry = cache.get(&prefix_key).cloned();
            drop(cache);
            if entry.is_some() {
                let mut counts = self
                    .access_counts
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                *counts.entry(prefix_key).or_insert(0) += 1;
            }
            return Ok(entry);
        }
        Ok(None)
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let key = entry.key.clone();
        if self.max_capacity > 0 {
            let cache = self
                .cache
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if cache.len() >= self.max_capacity && !cache.contains_key(&key) {
                drop(cache);
                self.evict_least_popular()?;
            }
        }
        let mut cache = self
            .cache
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        cache.insert(key.clone(), entry);
        // Ensure new keys are tracked in access_counts with 0
        let mut counts = self
            .access_counts
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        counts.entry(key).or_insert(0);
        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let mut cache = self
            .cache
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        let mut counts = self
            .access_counts
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        cache.remove(key);
        counts.remove(key);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DeepSeek MLA (Multi-Latent Attention) Cache Strategy
// ---------------------------------------------------------------------------

/// Metrics for the DeepSeek MLA cache.
#[derive(Debug, Clone, Default)]
pub struct MLACacheMetrics {
    /// Total cache lookups.
    pub total_lookups: u64,
    /// Exact prefix hits (full KV block reuse).
    pub exact_hits: u64,
    /// Partial prefix hits (some blocks reused).
    pub partial_hits: u64,
    /// Cache misses.
    pub misses: u64,
    /// Total bytes stored.
    pub stored_bytes: u64,
    /// Total KV blocks cached.
    pub block_count: u64,
}

impl MLACacheMetrics {
    /// Overall hit rate (exact + partial) / total.
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        (self.exact_hits + self.partial_hits) as f64 / self.total_lookups as f64
    }

    /// Exact hit rate (full prefix match).
    pub fn exact_hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        self.exact_hits as f64 / self.total_lookups as f64
    }
}

/// A single KV cache block — represents a chunk of token-level key-value pairs.
#[derive(Debug, Clone)]
struct KVBlock {
    /// Block hash (hash of token IDs in this block).
    #[allow(dead_code)] // Stored for future eviction/pinning logic
    block_hash: u64,
    /// The serialized KV cache data.
    data: Vec<u8>,
    /// Number of tokens in this block.
    #[allow(dead_code)] // Stored for future metrics/accounting
    token_count: usize,
    /// When this block was last accessed.
    last_access: Instant,
    /// Access frequency score (for popularity-based eviction).
    access_score: f64,
}

/// DeepSeek MLA cache strategy with token-level prefix caching.
///
/// Design:
/// - Tokens are grouped into fixed-size blocks (default: 64 tokens per block).
/// - Each block is hashed by its token content.
/// - A request's KV cache is represented as a chain of block hashes.
/// - On lookup, we compute block hashes for the query tokens and find the
///   longest matching prefix of block hashes.
/// - Eviction uses a hybrid LRU + popularity score to balance recency and frequency.
pub struct DeepSeekMLAStrategy {
    /// Block hash → KV block data.
    blocks: RwLock<HashMap<u64, KVBlock>>,
    /// Sequence hash (hash of block hash chain) → full cache entry.
    entries: RwLock<HashMap<String, CacheEntry>>,
    /// Token sequence hash → list of block hashes that form the sequence.
    sequences: RwLock<HashMap<u64, Vec<u64>>>,
    /// Maximum number of KV blocks.
    max_blocks: usize,
    /// Tokens per block.
    block_size: usize,
    /// Maximum memory in bytes (0 = unlimited).
    max_memory_bytes: u64,
    /// Atomic counters for lock-free metric updates.
    total_lookups: AtomicU64,
    exact_hits: AtomicU64,
    partial_hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for DeepSeekMLAStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepSeekMLAStrategy {
    /// Create a new DeepSeek MLA cache with default settings.
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            entries: RwLock::new(HashMap::new()),
            sequences: RwLock::new(HashMap::new()),
            max_blocks: 4096,
            block_size: 64,
            max_memory_bytes: 0,
            total_lookups: AtomicU64::new(0),
            exact_hits: AtomicU64::new(0),
            partial_hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(max_blocks: usize, block_size: usize, max_memory_bytes: u64) -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            entries: RwLock::new(HashMap::new()),
            sequences: RwLock::new(HashMap::new()),
            max_blocks,
            block_size,
            max_memory_bytes,
            total_lookups: AtomicU64::new(0),
            exact_hits: AtomicU64::new(0),
            partial_hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Hash a sequence of token IDs into block hashes.
    fn compute_block_hashes(&self, token_ids: &[u64]) -> Vec<u64> {
        token_ids
            .chunks(self.block_size)
            .map(|chunk| {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                chunk.hash(&mut hasher);
                hasher.finish()
            })
            .collect()
    }

    /// Hash a chain of block hashes into a sequence hash.
    fn compute_sequence_hash(block_hashes: &[u64]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        block_hashes.hash(&mut hasher);
        hasher.finish()
    }

    /// Find the longest matching prefix of block hashes.
    fn find_longest_block_prefix(
        &self,
        query_blocks: &[u64],
    ) -> KiasResult<(usize, Option<String>)> {
        let sequences = self
            .sequences
            .read()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;

        let mut best_match_len = 0usize;
        let mut best_entry_key: Option<String> = None;

        for (seq_hash, cached_blocks) in sequences.iter() {
            let match_len = query_blocks
                .iter()
                .zip(cached_blocks.iter())
                .take_while(|(a, b)| a == b)
                .count();

            if match_len > best_match_len {
                best_match_len = match_len;
                // Find the corresponding entry key
                let entries = self
                    .entries
                    .read()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                for (key, _) in entries.iter() {
                    // Use sequence hash as part of the key
                    if key.contains(&seq_hash.to_string()) {
                        best_entry_key = Some(key.clone());
                        break;
                    }
                }
            }
        }

        Ok((best_match_len, best_entry_key))
    }

    /// Evict the least valuable block (hybrid LRU + popularity).
    fn evict_least_valuable(&self) -> KiasResult<()> {
        let mut blocks = self
            .blocks
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;

        // Find the block with the lowest score (recency + frequency)
        let now = Instant::now();
        let worst_key = blocks
            .iter()
            .map(|(hash, block)| {
                let age_secs = now.duration_since(block.last_access).as_secs_f64();
                // Lower score = more evictable
                let score = block.access_score / (1.0 + age_secs);
                (*hash, score)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(hash, _)| hash);

        if let Some(key) = worst_key {
            blocks.remove(&key);
        }
        Ok(())
    }

    /// Get current memory usage in bytes.
    fn current_memory_bytes(&self) -> u64 {
        self.blocks.read().ok().map_or(0, |blocks| {
            blocks.values().map(|b| b.data.len() as u64).sum()
        })
    }

    /// Get the number of cached blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.read().ok().map_or(0, |b| b.len())
    }

    /// Get the number of cached sequences.
    pub fn sequence_count(&self) -> usize {
        self.sequences.read().ok().map_or(0, |s| s.len())
    }

    /// Get cache metrics.
    pub fn metrics(&self) -> MLACacheMetrics {
        MLACacheMetrics {
            total_lookups: self.total_lookups.load(Ordering::Relaxed),
            exact_hits: self.exact_hits.load(Ordering::Relaxed),
            partial_hits: self.partial_hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            stored_bytes: self.current_memory_bytes(),
            block_count: self.block_count() as u64,
        }
    }

    /// Store KV blocks for a token sequence.
    pub fn store_blocks(&self, token_ids: &[u64], kv_data: &[u8]) -> KiasResult<()> {
        let block_hashes = self.compute_block_hashes(token_ids);
        let seq_hash = Self::compute_sequence_hash(&block_hashes);

        // Store individual blocks
        {
            let mut blocks = self
                .blocks
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;

            for (i, &block_hash) in block_hashes.iter().enumerate() {
                let start = i * self.block_size;
                let end = std::cmp::min(start + self.block_size, token_ids.len());
                let block_data = kv_data[start..end].to_vec();

                blocks.entry(block_hash).or_insert_with(|| KVBlock {
                    block_hash,
                    data: block_data,
                    token_count: end - start,
                    last_access: Instant::now(),
                    access_score: 1.0,
                });
            }

            // Evict if over capacity
            while blocks.len() > self.max_blocks {
                drop(blocks);
                self.evict_least_valuable()?;
                blocks = self
                    .blocks
                    .write()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            }
        }

        // Store sequence mapping
        {
            let mut sequences = self
                .sequences
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            sequences.insert(seq_hash, block_hashes);
        }

        Ok(())
    }

    /// Look up cached KV data for a token sequence, returning the number of reusable blocks.
    pub fn lookup_blocks(&self, token_ids: &[u64]) -> KiasResult<usize> {
        let block_hashes = self.compute_block_hashes(token_ids);

        self.total_lookups.fetch_add(1, Ordering::Relaxed);

        let (match_len, _) = self.find_longest_block_prefix(&block_hashes)?;

        if match_len == block_hashes.len() && match_len > 0 {
            self.exact_hits.fetch_add(1, Ordering::Relaxed);
        } else if match_len > 0 {
            self.partial_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        // Update access scores for matched blocks
        if match_len > 0 {
            let mut blocks = self
                .blocks
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            for &hash in block_hashes.iter().take(match_len) {
                if let Some(block) = blocks.get_mut(&hash) {
                    block.last_access = Instant::now();
                    block.access_score += 1.0;
                }
            }
        }

        Ok(match_len)
    }
}

#[async_trait]
impl CacheStrategy for DeepSeekMLAStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        self.total_lookups.fetch_add(1, Ordering::Relaxed);

        // Try exact match on entries
        {
            let entries = self
                .entries
                .read()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            if let Some(entry) = entries.get(key) {
                self.exact_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(entry.clone()));
            }
        }

        // Try prefix match using block hashes
        // Interpret the key as a token sequence representation
        let token_ids: Vec<u64> = key
            .as_bytes()
            .chunks(self.block_size)
            .map(|chunk| {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                chunk.hash(&mut hasher);
                hasher.finish()
            })
            .collect();

        let block_hashes = self.compute_block_hashes(&token_ids);
        let (match_len, best_key) = self.find_longest_block_prefix(&block_hashes)?;

        if match_len > 0 {
            self.partial_hits.fetch_add(1, Ordering::Relaxed);
            if let Some(ref entry_key) = best_key {
                let entries = self
                    .entries
                    .read()
                    .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
                return Ok(entries.get(entry_key).cloned());
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let key = entry.key.clone();
        let key_for_blocks = key.clone();
        let value_len = entry.value.len() as u64;

        // Check memory limit
        if self.max_memory_bytes > 0 {
            let current = self.current_memory_bytes();
            if current + value_len > self.max_memory_bytes {
                self.evict_least_valuable()?;
            }
        }

        // Store the entry
        {
            let mut entries = self
                .entries
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            entries.insert(key.clone(), entry);
        }

        // Also store as blocks for prefix matching
        let token_ids: Vec<u64> = key_for_blocks
            .as_bytes()
            .chunks(self.block_size)
            .map(|chunk| {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                chunk.hash(&mut hasher);
                hasher.finish()
            })
            .collect();

        let block_hashes = self.compute_block_hashes(&token_ids);
        let seq_hash = Self::compute_sequence_hash(&block_hashes);

        {
            let mut sequences = self
                .sequences
                .write()
                .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
            sequences.insert(seq_hash, block_hashes);
        }

        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| KiasError::LockPoisoned(e.to_string()))?;
        entries.remove(key);
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
        strategy
            .set(make_entry_with_ttl("k1", b"data", Duration::from_millis(1)))
            .await
            .unwrap();
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = strategy.get("k1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lru_ttl_not_expired() {
        let strategy = LRUStrategy::new();
        strategy
            .set(make_entry_with_ttl("k1", b"data", Duration::from_secs(60)))
            .await
            .unwrap();
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
        strategy
            .set(make_entry("hello", b"prefix_cache"))
            .await
            .unwrap();
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
        hub.set(make_entry("prompt_prefix", b"kv_cache"))
            .await
            .unwrap();
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

    // ===== DeepSeek MLA Strategy Tests =====

    #[test]
    fn test_mla_creation() {
        let strategy = DeepSeekMLAStrategy::new();
        assert_eq!(strategy.block_count(), 0);
        assert_eq!(strategy.sequence_count(), 0);
        let metrics = strategy.metrics();
        assert_eq!(metrics.total_lookups, 0);
        assert_eq!(metrics.hit_rate(), 0.0);
    }

    #[test]
    fn test_mla_custom_config() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 32, 1024 * 1024);
        assert_eq!(strategy.block_count(), 0);
    }

    #[test]
    fn test_mla_compute_block_hashes() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 4, 0); // 4 tokens per block
        let tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8, 9];
        let hashes = strategy.compute_block_hashes(&tokens);
        // 9 tokens / 4 per block = 3 blocks
        assert_eq!(hashes.len(), 3);
        // Same input should produce same hashes
        let hashes2 = strategy.compute_block_hashes(&tokens);
        assert_eq!(hashes, hashes2);
    }

    #[test]
    fn test_mla_compute_sequence_hash() {
        let blocks = vec![100u64, 200, 300];
        let hash = DeepSeekMLAStrategy::compute_sequence_hash(&blocks);
        assert_ne!(hash, 0);
        // Same blocks should produce same hash
        let hash2 = DeepSeekMLAStrategy::compute_sequence_hash(&blocks);
        assert_eq!(hash, hash2);
        // Different blocks should produce different hash
        let blocks2 = vec![100u64, 200, 301];
        let hash3 = DeepSeekMLAStrategy::compute_sequence_hash(&blocks2);
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_mla_store_and_lookup_blocks() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 4, 0);
        let tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8];
        let kv_data: Vec<u8> = (0..8).map(|i| i as u8).collect();

        strategy.store_blocks(&tokens, &kv_data).unwrap();

        // Exact match should return all blocks
        let matched = strategy.lookup_blocks(&tokens).unwrap();
        assert_eq!(matched, 2); // 8 tokens / 4 per block = 2 blocks

        let metrics = strategy.metrics();
        assert_eq!(metrics.total_lookups, 1);
        assert_eq!(metrics.exact_hits, 1);
    }

    #[test]
    fn test_mla_partial_prefix_match() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 4, 0);

        // Store a long sequence
        let long_tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let long_kv: Vec<u8> = (0..12).map(|i| i as u8).collect();
        strategy.store_blocks(&long_tokens, &long_kv).unwrap();

        // Query with a shorter sequence that shares prefix blocks
        let short_tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8]; // same first 8 tokens
        let matched = strategy.lookup_blocks(&short_tokens).unwrap();
        assert_eq!(matched, 2); // 2 matching blocks

        // This is counted as an exact hit since all query blocks matched
        let metrics = strategy.metrics();
        assert_eq!(metrics.exact_hits, 1);
        assert_eq!(metrics.total_lookups, 1);
    }

    #[test]
    fn test_mla_cache_miss() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 4, 0);

        let tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8];
        let kv_data: Vec<u8> = (0..8).map(|i| i as u8).collect();
        strategy.store_blocks(&tokens, &kv_data).unwrap();

        // Query with completely different tokens
        let different = vec![100u64, 200, 300, 400];
        let matched = strategy.lookup_blocks(&different).unwrap();
        assert_eq!(matched, 0);

        let metrics = strategy.metrics();
        assert_eq!(metrics.misses, 1);
    }

    #[test]
    fn test_mla_metrics_hit_rate() {
        let strategy = DeepSeekMLAStrategy::with_config(1024, 4, 0);
        let tokens = vec![1u64, 2, 3, 4, 5, 6, 7, 8];
        let kv_data: Vec<u8> = (0..8).map(|i| i as u8).collect();
        strategy.store_blocks(&tokens, &kv_data).unwrap();

        // 1 exact hit
        strategy.lookup_blocks(&tokens).unwrap();
        // 1 miss
        strategy.lookup_blocks(&[99, 100, 101, 102]).unwrap();

        let metrics = strategy.metrics();
        assert_eq!(metrics.total_lookups, 2);
        assert_eq!(metrics.exact_hits, 1);
        assert_eq!(metrics.misses, 1);
        assert!((metrics.hit_rate() - 0.5).abs() < f64::EPSILON);
        assert!((metrics.exact_hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_mla_cache_strategy_get_set() {
        let strategy = DeepSeekMLAStrategy::new();
        let entry = make_entry("test_key", b"test_value");
        strategy.set(entry).await.unwrap();

        let result = strategy.get("test_key").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, b"test_value");
    }

    #[tokio::test]
    async fn test_mla_cache_strategy_miss() {
        let strategy = DeepSeekMLAStrategy::new();
        let result = strategy.get("nonexistent").await.unwrap();
        assert!(result.is_none());

        let metrics = strategy.metrics();
        assert_eq!(metrics.misses, 1);
    }

    #[tokio::test]
    async fn test_mla_cache_strategy_evict() {
        let strategy = DeepSeekMLAStrategy::new();
        let entry = make_entry("to_evict", b"data");
        strategy.set(entry).await.unwrap();
        assert!(strategy.get("to_evict").await.unwrap().is_some());

        strategy.evict("to_evict").await.unwrap();
        assert!(strategy.get("to_evict").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mla_hub_integration() {
        let strategy = Box::new(DeepSeekMLAStrategy::new());
        let hub = CacheHub::new(strategy);
        hub.set_string("deepseek_cache_key", "cached_value", None)
            .await
            .unwrap();
        let result = hub.get_string("deepseek_cache_key").await.unwrap();
        assert_eq!(result, Some("cached_value".to_string()));
    }
}
