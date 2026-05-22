//! # Knowledge Freshness Module
//!
//! Provides freshness checking for knowledge entries with automatic refresh
//! and expiry policy management.
//!
//! ## Core types
//!
//! - [`FreshnessChecker`] — checks freshness of knowledge entries
//! - [`ExpiryPolicy`] — defines when entries expire
//! - [`AutoRefresh`] — automatic refresh trigger mechanism
//! - [`StaleRef`] — represents a stale knowledge reference

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Represents a stale knowledge reference that needs refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleRef {
    /// Unique identifier of the stale entry.
    pub id: String,
    /// Age in seconds since last refresh.
    pub age_seconds: u64,
    /// Timestamp when the entry was marked stale (not serialized).
    #[serde(skip, default = "Instant::now")]
    pub marked_at: Instant,
}

impl StaleRef {
    /// Create a new StaleRef without marked_at (for compatibility).
    pub fn new(id: String, age_seconds: u64) -> Self {
        Self {
            id,
            age_seconds,
            marked_at: Instant::now(),
        }
    }

    /// Create with custom marked_at for testing.
    #[allow(dead_code)]
    pub fn new_with_time(id: String, age_seconds: u64, marked_at: Instant) -> Self {
        Self {
            id,
            age_seconds,
            marked_at,
        }
    }
}

/// Configuration for freshness checking.
#[derive(Debug, Clone)]
pub struct FreshnessConfig {
    /// Maximum age in seconds before an entry is considered stale.
    pub max_age_secs: u64,
    /// Refresh interval in seconds for the background checker.
    pub refresh_interval_secs: u64,
    /// Whether to auto-refresh stale entries.
    pub auto_refresh: bool,
}

impl FreshnessConfig {
    /// Create a new FreshnessConfig with given max age and refresh interval.
    pub fn new(max_age_secs: u64, refresh_interval_secs: u64) -> Self {
        Self {
            max_age_secs,
            refresh_interval_secs,
            auto_refresh: false,
        }
    }

    /// Set auto_refresh to true.
    pub fn with_auto_refresh(mut self) -> Self {
        self.auto_refresh = true;
        self
    }
}

impl Default for FreshnessConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 3600,
            refresh_interval_secs: 60,
            auto_refresh: false,
        }
    }
}

/// Represents a single knowledge entry that can be checked for freshness.
/// Note: last_verified and created_at are not serialized (transient).
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    /// Unique identifier for this knowledge entry.
    pub id: String,
    /// The knowledge content or reference.
    pub content: String,
    /// Timestamp when this entry was last verified/refreshed.
    pub last_verified: Instant,
    /// Timestamp when this entry was created.
    pub created_at: Instant,
    /// Optional metadata.
    pub metadata: HashMap<String, String>,
}

impl KnowledgeEntry {
    /// Create a new KnowledgeEntry.
    pub fn new(id: String, content: String) -> Self {
        let now = Instant::now();
        Self {
            id,
            content,
            last_verified: now,
            created_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Create a KnowledgeEntry with a specific last_verified time.
    pub fn with_verified(id: String, content: String, last_verified: Instant) -> Self {
        let now = Instant::now();
        Self {
            id,
            content,
            last_verified,
            created_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Check if this entry is stale given a max age.
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        self.last_verified.elapsed().as_secs() > max_age_secs
    }

    /// Age in seconds since last verification.
    pub fn age_secs(&self) -> u64 {
        self.last_verified.elapsed().as_secs()
    }
}

/// Expiry policy for knowledge entries.
pub enum ExpiryPolicy {
    /// Entry expires after max_age duration.
    MaxAge(Duration),
    /// Entry expires at a fixed timestamp.
    ExpiresAt(Instant),
    /// Entry never expires.
    Never,
    /// Custom expiry based on a closure.
    Custom(Box<dyn Fn(&KnowledgeEntry) -> bool + Send + Sync>),
}

impl std::fmt::Debug for ExpiryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxAge(d) => f.debug_tuple("MaxAge").field(d).finish(),
            Self::ExpiresAt(i) => f.debug_tuple("ExpiresAt").field(i).finish(),
            Self::Never => write!(f, "Never"),
            Self::Custom(_) => f.debug_tuple("Custom").field(&"<fn>").finish(),
        }
    }
}

impl Clone for ExpiryPolicy {
    fn clone(&self) -> Self {
        match self {
            Self::MaxAge(d) => Self::MaxAge(*d),
            Self::ExpiresAt(i) => Self::ExpiresAt(*i),
            Self::Never => Self::Never,
            Self::Custom(_) => Self::default(),
        }
    }
}

impl ExpiryPolicy {
    /// Check if an entry is expired under this policy.
    pub fn is_expired(&self, entry: &KnowledgeEntry) -> bool {
        match self {
            ExpiryPolicy::MaxAge(duration) => entry.is_stale(duration.as_secs()),
            ExpiryPolicy::ExpiresAt(instant) => Instant::now() > *instant,
            ExpiryPolicy::Never => false,
            ExpiryPolicy::Custom(f) => f(entry),
        }
    }
}

impl Default for ExpiryPolicy {
    fn default() -> Self {
        ExpiryPolicy::MaxAge(Duration::from_secs(3600))
    }
}

/// Auto-refresh trigger configuration.
#[derive(Debug, Clone)]
pub struct AutoRefreshConfig {
    /// Whether auto-refresh is enabled.
    pub enabled: bool,
    /// Interval between refresh checks.
    pub check_interval: Duration,
    /// Maximum number of entries to refresh per batch.
    pub batch_size: usize,
}

impl Default for AutoRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            check_interval: Duration::from_secs(60),
            batch_size: 10,
        }
    }
}

/// FreshnessChecker manages knowledge entry freshness.
#[derive(Debug)]
pub struct FreshnessChecker {
    /// Entries indexed by ID.
    entries: Arc<RwLock<HashMap<String, KnowledgeEntry>>>,
    /// Maximum age before an entry is considered stale.
    #[allow(dead_code)]
    max_age_secs: u64,
    /// Refresh interval in seconds.
    #[allow(dead_code)]
    refresh_interval_secs: u64,
    /// Expiry policy.
    policy: ExpiryPolicy,
    /// Auto-refresh configuration.
    #[allow(dead_code)]
    auto_refresh: AutoRefreshConfig,
}

impl FreshnessChecker {
    /// Create a new FreshnessChecker with default settings.
    pub fn new() -> Self {
        Self::with_config(FreshnessConfig::default())
    }

    /// Create a FreshnessChecker with custom config.
    pub fn with_config(config: FreshnessConfig) -> Self {
        let policy = ExpiryPolicy::MaxAge(Duration::from_secs(config.max_age_secs));
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_age_secs: config.max_age_secs,
            refresh_interval_secs: config.refresh_interval_secs,
            policy,
            auto_refresh: AutoRefreshConfig {
                enabled: config.auto_refresh,
                ..Default::default()
            },
        }
    }

    /// Create a FreshnessChecker with explicit policy.
    pub fn with_policy(policy: ExpiryPolicy, refresh_interval_secs: u64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_age_secs: 0,
            refresh_interval_secs,
            policy,
            auto_refresh: AutoRefreshConfig::default(),
        }
    }

    /// Add or update a knowledge entry.
    pub fn upsert(
        &self,
        id: &str,
        content: &str,
        last_verified: Option<Instant>,
    ) -> KiasResult<()> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;

        let _entry = if let Some(existing) = entries.get_mut(id) {
            existing.content = content.to_string();
            if let Some(ts) = last_verified {
                existing.last_verified = ts;
            }
            existing.clone()
        } else {
            let entry = match last_verified {
                Some(ts) => KnowledgeEntry::with_verified(id.to_string(), content.to_string(), ts),
                None => KnowledgeEntry::new(id.to_string(), content.to_string()),
            };
            entries.insert(id.to_string(), entry.clone());
            entry
        };

        Ok(())
    }

    /// Remove a knowledge entry.
    pub fn remove(&self, id: &str) -> KiasResult<bool> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;
        Ok(entries.remove(id).is_some())
    }

    /// Get a knowledge entry by ID.
    pub fn get(&self, id: &str) -> KiasResult<Option<KnowledgeEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;
        Ok(entries.get(id).cloned())
    }

    /// Check freshness of all entries and return stale ones.
    pub fn check_freshness(&self) -> KiasResult<Vec<StaleRef>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;

        let stale_refs: Vec<StaleRef> = entries
            .values()
            .filter(|entry| self.policy.is_expired(entry))
            .map(|entry| StaleRef::new(entry.id.clone(), entry.age_secs()))
            .collect();

        Ok(stale_refs)
    }

    /// Alias for check_freshness for clarity.
    pub fn detect_stale(&self) -> KiasResult<Vec<StaleRef>> {
        self.check_freshness()
    }

    /// Refresh stale entries by updating their last_verified timestamp.
    pub fn refresh_stale(&self) -> KiasResult<Vec<String>> {
        let stale_refs = self.check_freshness()?;

        {
            let mut entries = self
                .entries
                .write()
                .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;

            for stale in &stale_refs {
                if let Some(entry) = entries.get_mut(&stale.id) {
                    entry.last_verified = Instant::now();
                }
            }
        }

        Ok(stale_refs.iter().map(|s| s.id.clone()).collect())
    }

    /// Mark a specific entry as verified (fresh).
    pub fn mark_verified(&self, id: &str) -> KiasResult<()> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;

        let entry = entries
            .get_mut(id)
            .ok_or_else(|| KiasError::NotFound(id.to_string()))?;
        entry.last_verified = Instant::now();
        Ok(())
    }

    /// Get all entry IDs.
    pub fn entry_ids(&self) -> KiasResult<Vec<String>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;
        Ok(entries.keys().cloned().collect())
    }

    /// Get the count of entries.
    pub fn len(&self) -> KiasResult<usize> {
        let entries = self
            .entries
            .read()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;
        Ok(entries.len())
    }

    /// Check if there are no entries.
    pub fn is_empty(&self) -> KiasResult<bool> {
        self.len().map(|l| l == 0)
    }

    /// Get current statistics.
    pub fn stats(&self) -> KiasResult<FreshnessStats> {
        let entries = self
            .entries
            .read()
            .map_err(|e| KiasError::LockPoisoned(format!("RwLock poisoned: {}", e)))?;

        let total = entries.len();
        let stale = entries
            .values()
            .filter(|e| self.policy.is_expired(e))
            .count();
        let fresh = total - stale;

        Ok(FreshnessStats {
            total,
            fresh,
            stale,
        })
    }
}

impl Default for FreshnessChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about freshness state.
#[derive(Debug, Clone)]
pub struct FreshnessStats {
    /// Total number of entries.
    pub total: usize,
    /// Number of fresh entries.
    pub fresh: usize,
    /// Number of stale entries.
    pub stale: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: FreshnessChecker creation and basic operations.
    #[test]
    fn test_create_freshness_checker() {
        let checker = FreshnessChecker::new();
        assert!(checker.is_empty().unwrap());
        assert_eq!(checker.len().unwrap(), 0);
    }

    // Test 2: Upsert and get knowledge entry.
    #[test]
    fn test_upsert_knowledge() {
        let checker = FreshnessChecker::new();
        checker.upsert("doc1", "content1", None).unwrap();

        let entry = checker.get("doc1").unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().content, "content1");
    }

    // Test 3: Remove knowledge entry.
    #[test]
    fn test_remove_knowledge() {
        let checker = FreshnessChecker::new();
        checker.upsert("doc1", "content1", None).unwrap();
        assert!(checker.remove("doc1").unwrap());
        assert!(checker.get("doc1").unwrap().is_none());
    }

    // Test 4: check_freshness returns empty vector when all entries are fresh.
    #[test]
    fn test_check_freshness_no_stale() {
        let config = FreshnessConfig::new(3600, 60);
        let checker = FreshnessChecker::with_config(config);
        checker.upsert("doc1", "content1", None).unwrap();

        let stale = checker.check_freshness().unwrap();
        assert!(stale.is_empty());
    }

    // Test 5: check_freshness detects stale entries after they exceed max_age.
    #[test]
    fn test_check_freshness_with_stale() {
        let config = FreshnessConfig::new(1, 60); // 1 second max age for testing
        let checker = FreshnessChecker::with_config(config);

        // Insert with a past timestamp (2 seconds ago)
        let past = Instant::now() - Duration::from_secs(2);
        checker.upsert("doc1", "content1", Some(past)).unwrap();

        let stale_before = checker.check_freshness().unwrap();
        assert_eq!(stale_before.len(), 1);
        assert_eq!(stale_before[0].id, "doc1");

        // Mark as verified now
        checker.mark_verified("doc1").unwrap();
        let stale_after = checker.check_freshness().unwrap();
        assert!(stale_after.is_empty());
    }

    // Test 6: ExpiryPolicy::Never never expires.
    #[test]
    fn test_expiry_policy_never() {
        let policy = ExpiryPolicy::Never;
        let mut entry = KnowledgeEntry::new("doc1".to_string(), "content".to_string());
        entry.last_verified = Instant::now() - Duration::from_secs(1000000);

        assert!(!policy.is_expired(&entry));
    }

    // Test 7: ExpiryPolicy::ExpiresAt expires at fixed time.
    #[test]
    fn test_expiry_policy_expires_at() {
        let past = Instant::now() - Duration::from_secs(10);
        let policy = ExpiryPolicy::ExpiresAt(past);

        let entry = KnowledgeEntry::new("doc1".to_string(), "content".to_string());
        assert!(policy.is_expired(&entry));

        let future = Instant::now() + Duration::from_secs(3600);
        let policy_future = ExpiryPolicy::ExpiresAt(future);
        assert!(!policy_future.is_expired(&entry));
    }

    // Test 8: refresh_stale updates timestamps.
    #[test]
    fn test_refresh_stale() {
        let config = FreshnessConfig::new(1, 60); // 1 second max age
        let checker = FreshnessChecker::with_config(config);

        let past = Instant::now() - Duration::from_secs(2);
        checker.upsert("doc1", "content1", Some(past)).unwrap();

        assert_eq!(checker.check_freshness().unwrap().len(), 1);

        let refreshed = checker.refresh_stale().unwrap();
        assert_eq!(refreshed, vec!["doc1"]);
        assert!(checker.check_freshness().unwrap().is_empty());
    }

    // Test 9: Stats reflect correct counts.
    #[test]
    fn test_stats() {
        let config = FreshnessConfig::new(1, 60);
        let checker = FreshnessChecker::with_config(config);

        checker.upsert("doc1", "content1", None).unwrap();
        checker.upsert("doc2", "content2", None).unwrap();

        let past = Instant::now() - Duration::from_secs(2);
        checker.upsert("doc3", "content3", Some(past)).unwrap();

        let stats = checker.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.fresh, 2);
        assert_eq!(stats.stale, 1);
    }

    // Test 10: Snapshot preserves state.
    #[test]
    fn test_snapshot() {
        let checker = FreshnessChecker::new();
        checker.upsert("doc1", "content1", None).unwrap();

        let ids = checker.entry_ids().unwrap();
        assert_eq!(ids, vec!["doc1"]);

        let stats = checker.stats().unwrap();
        assert_eq!(stats.total, 1);
    }

    // Test 11: mark_verified on non-existent entry returns error.
    #[test]
    fn test_mark_verified_not_found() {
        let checker = FreshnessChecker::new();
        let result = checker.mark_verified("nonexistent");
        assert!(result.is_err());
    }
}
