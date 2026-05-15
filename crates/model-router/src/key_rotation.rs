//! # API Key Rotation
//!
//! Manages a pool of API keys per provider with automatic rotation on:
//! - 429 Too Many Requests
//! - Quota exceeded errors
//! - Rate limit cooldowns
//!
//! Inspired by "Ollama Open Router" seamless key rotation pattern.
//!
//! ## Design
//!
//! Each provider can have multiple API keys. When a key hits a rate limit,
//! the router automatically switches to the next available key without
//! interrupting the request flow.
//!
//! ```text
//! Request → KeyRotator → Key A (429!) → Key B (OK) → Response
//!                        ↓ cooldown
//!                      Key A waits, then returns to pool
//! ```

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Strategy for selecting the next key
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum KeyRotationStrategy {
    /// Cycle through keys in order (default)
    #[default]
    RoundRobin,
    /// Pick the least-recently-used key
    LeastRecentlyUsed,
    /// Random selection
    Random,
}

/// Status of an individual API key
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeyStatus {
    /// Available for use
    Active,
    /// Temporarily cooling down after 429/rate limit
    Cooldown,
    /// Permanently exhausted (quota exceeded)
    Exhausted,
    /// Manually disabled
    Disabled,
}

/// An API key with its metadata and health tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// The actual key value
    pub key: String,
    /// Current status
    pub status: KeyStatus,
    /// Total successful requests
    pub success_count: u64,
    /// Total failed requests (429, quota, etc.)
    pub failure_count: u64,
    /// When this key was last used
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    /// When cooldown expires (if in Cooldown status)
    pub cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional label for identification
    pub label: Option<String>,
    /// Per-key monthly spend tracking
    pub spend_usd: f64,
    /// Per-key monthly budget
    pub budget_usd: Option<f64>,
}

impl ApiKey {
    /// Create a new active API key
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            status: KeyStatus::Active,
            success_count: 0,
            failure_count: 0,
            last_used: None,
            cooldown_until: None,
            label: None,
            spend_usd: 0.0,
            budget_usd: None,
        }
    }

    /// Create with a label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Create with a budget
    pub fn with_budget(mut self, budget_usd: f64) -> Self {
        self.budget_usd = Some(budget_usd);
        self
    }

    /// Check if this key is available for use
    pub fn is_available(&self) -> bool {
        match self.status {
            KeyStatus::Active => {
                // Check budget
                if let Some(budget) = self.budget_usd {
                    if self.spend_usd >= budget {
                        return false;
                    }
                }
                true
            }
            KeyStatus::Cooldown => {
                // Check if cooldown has expired
                if let Some(until) = self.cooldown_until {
                    chrono::Utc::now() >= until
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Mark as used successfully
    pub fn record_success(&mut self, cost_usd: f64) {
        self.success_count += 1;
        self.last_used = Some(chrono::Utc::now());
        self.spend_usd += cost_usd;
        // If was in cooldown and cooldown expired, reactivate
        if self.status == KeyStatus::Cooldown {
            if let Some(until) = self.cooldown_until {
                if chrono::Utc::now() >= until {
                    self.status = KeyStatus::Active;
                    self.cooldown_until = None;
                }
            }
        }
    }

    /// Record a rate limit (429) — put into cooldown
    pub fn record_rate_limit(&mut self, cooldown_secs: u64) {
        self.failure_count += 1;
        self.status = KeyStatus::Cooldown;
        self.cooldown_until =
            Some(chrono::Utc::now() + chrono::Duration::seconds(cooldown_secs as i64));
        warn!(
            key = self.label.as_deref().unwrap_or(&self.key[..8]),
            cooldown_secs = cooldown_secs,
            "API key rate limited, entering cooldown"
        );
    }

    /// Record quota exhausted — permanently disable for this billing period
    pub fn record_quota_exhausted(&mut self) {
        self.failure_count += 1;
        self.status = KeyStatus::Exhausted;
        warn!(
            key = self.label.as_deref().unwrap_or(&self.key[..8]),
            "API key quota exhausted"
        );
    }

    /// Manually disable this key
    pub fn disable(&mut self) {
        self.status = KeyStatus::Disabled;
    }

    /// Manually re-enable this key
    pub fn enable(&mut self) {
        self.status = KeyStatus::Active;
        self.cooldown_until = None;
    }

    /// Reset all counters (e.g., at billing period boundary)
    pub fn reset(&mut self) {
        self.status = KeyStatus::Active;
        self.failure_count = 0;
        self.spend_usd = 0.0;
        self.cooldown_until = None;
    }
}

/// Configuration for key rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationConfig {
    /// Rotation strategy
    pub strategy: KeyRotationStrategy,
    /// Cooldown duration after 429 (seconds)
    pub cooldown_secs: u64,
    /// Enable automatic key recovery (check cooldowns periodically)
    pub auto_recovery: bool,
    /// How many consecutive failures before marking key as exhausted
    pub exhaustion_threshold: u32,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            strategy: KeyRotationStrategy::RoundRobin,
            cooldown_secs: 60,
            auto_recovery: true,
            exhaustion_threshold: 10,
        }
    }
}

/// Error from key rotation
#[derive(Debug, thiserror::Error)]
pub enum KeyRotatorError {
    #[error("no API keys available (all exhausted, cooling down, or disabled)")]
    NoKeysAvailable,
    #[error("key not found: {0}")]
    KeyNotFound(String),
}

/// Manages a pool of API keys with automatic rotation on failure
#[derive(Debug)]
pub struct KeyRotator {
    /// All keys for this provider
    keys: RwLock<Vec<ApiKey>>,
    /// Configuration
    config: KeyRotationConfig,
    /// Round-robin index
    rr_index: std::sync::atomic::AtomicUsize,
    /// Last failed key (for smart_pick deprioritization)
    last_failed_key: RwLock<Option<String>>,
}

impl KeyRotator {
    /// Create a new key rotator with the given keys
    pub fn new(keys: Vec<ApiKey>, config: KeyRotationConfig) -> Self {
        info!(key_count = keys.len(), strategy = ?config.strategy, "KeyRotator initialized");
        Self {
            keys: RwLock::new(keys),
            config,
            rr_index: std::sync::atomic::AtomicUsize::new(0),
            last_failed_key: RwLock::new(None),
        }
    }

    /// Create with a single key (no rotation needed)
    pub fn single(key: impl Into<String>) -> Self {
        Self::new(vec![ApiKey::new(key)], KeyRotationConfig::default())
    }

    /// Create from a list of key strings
    pub fn from_keys(keys: Vec<String>, config: KeyRotationConfig) -> Self {
        let api_keys = keys.into_iter().map(ApiKey::new).collect();
        Self::new(api_keys, config)
    }

    /// Get the next available key (does NOT record usage)
    pub async fn get_key(&self) -> Result<String, KeyRotatorError> {
        // Read last_failed_key BEFORE acquiring keys lock to avoid deadlock
        let last_failed = self.last_failed_key.read().await.clone();
        let mut keys = self.keys.write().await;

        // First pass: try to find an available key
        let len = keys.len();
        if len == 0 {
            return Err(KeyRotatorError::NoKeysAvailable);
        }

        // Wake up any cooled-down keys whose cooldown has expired
        if self.config.auto_recovery {
            for key in keys.iter_mut() {
                if key.status == KeyStatus::Cooldown {
                    if let Some(until) = key.cooldown_until {
                        if chrono::Utc::now() >= until {
                            key.status = KeyStatus::Active;
                            key.cooldown_until = None;
                            debug!(
                                key = key.label.as_deref().unwrap_or("unknown"),
                                "Key recovered from cooldown"
                            );
                        }
                    }
                }
            }
        }

        match self.config.strategy {
            KeyRotationStrategy::RoundRobin => {
                for _ in 0..len {
                    let idx = self
                        .rr_index
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        % len;
                    if keys[idx].is_available() {
                        return Ok(keys[idx].key.clone());
                    }
                }
            }
            KeyRotationStrategy::LeastRecentlyUsed => {
                // Sort by last_used (None = never used = highest priority)
                let mut candidates: Vec<(usize, Option<chrono::DateTime<chrono::Utc>>)> = keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.is_available())
                    .map(|(i, k)| (i, k.last_used))
                    .collect();
                candidates.sort_by_key(|(_, last)| *last);
                if let Some((idx, _)) = candidates.first() {
                    return Ok(keys[*idx].key.clone());
                }
            }
            KeyRotationStrategy::Random => {
                if let Some(idx) = smart_pick(&keys, &last_failed) {
                    return Ok(keys[idx].key.clone());
                }
            }
        }

        Err(KeyRotatorError::NoKeysAvailable)
    }

    /// Record a successful request for the given key
    pub async fn record_success(&self, key: &str, cost_usd: f64) {
        let mut keys = self.keys.write().await;
        if let Some(k) = keys.iter_mut().find(|k| k.key == key) {
            k.record_success(cost_usd);
        }
    }

    /// Record a rate limit (429) for the given key — auto-rotates to next
    pub async fn record_rate_limit(&self, key: &str) {
        let mut keys = self.keys.write().await;
        if let Some(k) = keys.iter_mut().find(|k| k.key == key) {
            k.record_rate_limit(self.config.cooldown_secs);
        }
        drop(keys);
        // Track last failed key for smart_pick deprioritization
        *self.last_failed_key.write().await = Some(key.to_string());
    }

    /// Record quota exhaustion for the given key
    pub async fn record_quota_exhausted(&self, key: &str) {
        let mut keys = self.keys.write().await;
        if let Some(k) = keys.iter_mut().find(|k| k.key == key) {
            k.record_quota_exhausted();
        }
    }

    /// Get the number of available keys
    pub async fn available_count(&self) -> usize {
        let keys = self.keys.read().await;
        keys.iter().filter(|k| k.is_available()).count()
    }

    /// Get total key count
    pub async fn total_count(&self) -> usize {
        let keys = self.keys.read().await;
        keys.len()
    }

    /// Get stats for all keys (for monitoring/dashboard)
    pub async fn stats(&self) -> Vec<KeyStats> {
        let keys = self.keys.read().await;
        keys.iter()
            .map(|k| KeyStats {
                label: k.label.clone().unwrap_or_else(|| mask_key(&k.key)),
                status: k.status.clone(),
                success_count: k.success_count,
                failure_count: k.failure_count,
                spend_usd: k.spend_usd,
                budget_usd: k.budget_usd,
                last_used: k.last_used,
            })
            .collect()
    }

    /// Add a new key to the pool
    pub async fn add_key(&self, key: ApiKey) {
        let mut keys = self.keys.write().await;
        info!(
            label = key.label.as_deref().unwrap_or("unlabeled"),
            "Adding new API key"
        );
        keys.push(key);
    }

    /// Remove a key by value
    pub async fn remove_key(&self, key_value: &str) -> bool {
        let mut keys = self.keys.write().await;
        let before = keys.len();
        keys.retain(|k| k.key != key_value);
        keys.len() < before
    }

    /// Reset all keys (e.g., at billing period boundary)
    pub async fn reset_all(&self) {
        let mut keys = self.keys.write().await;
        for key in keys.iter_mut() {
            key.reset();
        }
        info!("All API keys reset");
    }
}

/// Stats for a single key (for monitoring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStats {
    pub label: String,
    pub status: KeyStatus,
    pub success_count: u64,
    pub failure_count: u64,
    pub spend_usd: f64,
    pub budget_usd: Option<f64>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

/// Mask a key for display: show first 4 and last 4 chars
fn mask_key(key: &str) -> String {
    if key.len() <= 12 {
        "***".to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Smart key selection (inspired by ollama_open_router KeySelector._smart_pick).
///
/// 1. Filter available keys
/// 2. Shuffle candidates (Fisher-Yates)
/// 3. Deprioritize the last failed key (move to end)
/// 4. Return first candidate
fn smart_pick(keys: &[ApiKey], last_failed_key: &Option<String>) -> Option<usize> {
    let available: Vec<usize> = keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.is_available())
        .map(|(i, _)| i)
        .collect();

    if available.is_empty() {
        return None;
    }
    if available.len() == 1 {
        return Some(available[0]);
    }

    // Fisher-Yates shuffle on indices
    let mut candidates = available;
    let len = candidates.len();
    for i in (1..len).rev() {
        let j = rand_index(i + 1);
        candidates.swap(i, j);
    }

    // Deprioritize last failed key (move to end)
    if let Some(ref failed) = last_failed_key {
        if let Some(pos) = candidates.iter().position(|&i| keys[i].key == *failed) {
            let item = candidates.remove(pos);
            candidates.push(item);
        }
    }

    candidates.first().copied()
}

/// Simple deterministic random index (no external rand dependency)
fn rand_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos as usize) % len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(label: &str) -> ApiKey {
        ApiKey::new(format!("sk-test-{}-{}", label, "x".repeat(20))).with_label(label)
    }

    #[tokio::test]
    async fn test_single_key_rotation() {
        let rotator = KeyRotator::single("sk-only-key");
        let key = rotator.get_key().await.unwrap();
        assert_eq!(key, "sk-only-key");
    }

    #[tokio::test]
    async fn test_round_robin_rotation() {
        let rotator = KeyRotator::new(
            vec![make_key("a"), make_key("b"), make_key("c")],
            KeyRotationConfig {
                strategy: KeyRotationStrategy::RoundRobin,
                ..Default::default()
            },
        );
        let k1 = rotator.get_key().await.unwrap();
        let k2 = rotator.get_key().await.unwrap();
        let k3 = rotator.get_key().await.unwrap();
        // All different (round robin cycles through)
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
    }

    #[tokio::test]
    async fn test_rate_limit_cooldown() {
        let rotator = KeyRotator::new(
            vec![make_key("a"), make_key("b")],
            KeyRotationConfig::default(),
        );
        // Get first key
        let k1 = rotator.get_key().await.unwrap();
        // Rate limit it
        rotator.record_rate_limit(&k1).await;
        // Next get should return the other key
        let k2 = rotator.get_key().await.unwrap();
        assert_ne!(k1, k2);
    }

    #[tokio::test]
    async fn test_all_keys_exhausted() {
        let rotator = KeyRotator::new(vec![make_key("a")], KeyRotationConfig::default());
        let k = rotator.get_key().await.unwrap();
        rotator.record_rate_limit(&k).await;
        // Only one key, in cooldown — should fail
        let result = rotator.get_key().await;
        assert!(matches!(result, Err(KeyRotatorError::NoKeysAvailable)));
    }

    #[tokio::test]
    async fn test_quota_exhausted() {
        let rotator = KeyRotator::new(
            vec![make_key("a"), make_key("b")],
            KeyRotationConfig::default(),
        );
        let k1 = rotator.get_key().await.unwrap();
        rotator.record_quota_exhausted(&k1).await;
        // Should fall back to second key
        let k2 = rotator.get_key().await.unwrap();
        assert_ne!(k1, k2);
    }

    #[tokio::test]
    async fn test_success_tracking() {
        let rotator = KeyRotator::new(vec![make_key("a")], KeyRotationConfig::default());
        let k = rotator.get_key().await.unwrap();
        rotator.record_success(&k, 0.01).await;
        rotator.record_success(&k, 0.02).await;
        let stats = rotator.stats().await;
        assert_eq!(stats[0].success_count, 2);
        assert!((stats[0].spend_usd - 0.03).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_budget_enforcement() {
        let key = make_key("budget").with_budget(0.10);
        let rotator = KeyRotator::new(vec![key], KeyRotationConfig::default());
        let k = rotator.get_key().await.unwrap();
        // Spend up to budget
        rotator.record_success(&k, 0.05).await;
        rotator.record_success(&k, 0.05).await;
        // At budget — key should be unavailable
        assert_eq!(rotator.available_count().await, 0);
        assert!(rotator.get_key().await.is_err());
    }

    #[tokio::test]
    async fn test_add_and_remove_key() {
        let rotator = KeyRotator::new(vec![make_key("a")], KeyRotationConfig::default());
        assert_eq!(rotator.total_count().await, 1);
        rotator.add_key(make_key("b")).await;
        assert_eq!(rotator.total_count().await, 2);
        let removed = rotator.remove_key(&make_key("a").key).await;
        assert!(removed);
        assert_eq!(rotator.total_count().await, 1);
    }

    #[tokio::test]
    async fn test_reset_all() {
        let rotator = KeyRotator::new(vec![make_key("a")], KeyRotationConfig::default());
        let k = rotator.get_key().await.unwrap();
        rotator.record_rate_limit(&k).await;
        assert_eq!(rotator.available_count().await, 0);
        rotator.reset_all().await;
        assert_eq!(rotator.available_count().await, 1);
    }

    #[tokio::test]
    async fn test_mask_key() {
        assert_eq!(mask_key("sk-1234567890abcdef"), "sk-1...cdef");
        assert_eq!(mask_key("short"), "***");
    }

    #[tokio::test]
    async fn test_lru_rotation() {
        let rotator = KeyRotator::new(
            vec![make_key("a"), make_key("b")],
            KeyRotationConfig {
                strategy: KeyRotationStrategy::LeastRecentlyUsed,
                ..Default::default()
            },
        );
        // Use key a first
        let k1 = rotator.get_key().await.unwrap();
        rotator.record_success(&k1, 0.0).await;
        // LRU should now prefer the other key
        let k2 = rotator.get_key().await.unwrap();
        assert_ne!(k1, k2);
    }
}
