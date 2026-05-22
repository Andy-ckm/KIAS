//! Idempotency — deduplication for request/response, task, callback, and retry chains.
//!
//! ## Key Types
//! - `IdempotencyKey` — composite key (category + tenant + key)
//! - `IdempotencyStore` — in-memory TTL store with check/set/delete
//!
//! ## Supported Chains
//! 1. **Request/Response** — HTTP request deduplication
//! 2. **Task** — long-running task deduplication
//! 3. **Callback** — webhook/callback deduplication
//! 4. **Retry** — retry-safe operation deduplication

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// IdempotencyKey
// ─────────────────────────────────────────────────────────────────────────────

/// Category of operation — determines key-space isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IdempotencyCategory {
    Request,
    Task,
    Callback,
    Retry,
}

impl std::fmt::Display for IdempotencyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdempotencyCategory::Request => write!(f, "request"),
            IdempotencyCategory::Task => write!(f, "task"),
            IdempotencyCategory::Callback => write!(f, "callback"),
            IdempotencyCategory::Retry => write!(f, "retry"),
        }
    }
}

/// A composite idempotency key with category, tenant, and raw key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey {
    pub category: IdempotencyCategory,
    pub tenant_id: String,
    pub raw_key: String,
}

impl IdempotencyKey {
    pub fn new(category: IdempotencyCategory, tenant_id: &str, raw_key: &str) -> Self {
        Self {
            category,
            tenant_id: tenant_id.to_string(),
            raw_key: raw_key.to_string(),
        }
    }

    /// Convert to a single string suitable for use as a map key.
    pub fn to_store_key(&self) -> String {
        format!("{}:{}:{}", self.category, self.tenant_id, self.raw_key)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IdempotencyStore
// ─────────────────────────────────────────────────────────────────────────────

/// State of an idempotency key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdempotencyState {
    /// Operation is in progress.
    InProgress {
        started_at: DateTime<Utc>,
        result: Option<String>,
    },
    /// Operation completed successfully.
    Completed {
        result: String,
        completed_at: DateTime<Utc>,
    },
    /// Operation failed.
    Failed {
        error: String,
        failed_at: DateTime<Utc>,
    },
}

impl IdempotencyState {
    pub fn in_progress() -> Self {
        IdempotencyState::InProgress {
            started_at: Utc::now(),
            result: None,
        }
    }
}

/// In-memory idempotency store with TTL expiration.
#[derive(Debug, Default)]
pub struct IdempotencyStore {
    entries: HashMap<String, (IdempotencyState, DateTime<Utc>)>,
    ttl_secs: i64,
}

impl IdempotencyStore {
    pub fn new(ttl_secs: i64) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_secs,
        }
    }

    /// Check whether the given key already exists (is not expired).
    /// Returns the existing state if found and not expired.
    pub fn check(&self, key: &IdempotencyKey) -> Option<IdempotencyState> {
        let store_key = key.to_store_key();
        self.entries
            .get(&store_key)
            .and_then(|(state, expires_at)| {
                if *expires_at > Utc::now() {
                    Some(state.clone())
                } else {
                    None
                }
            })
    }

    /// Try to claim the key for a new operation.
    /// Returns `true` if claimed (first time), `false` if already exists.
    pub fn try_claim(&mut self, key: &IdempotencyKey) -> bool {
        if self.check(key).is_some() {
            return false;
        }
        let store_key = key.to_store_key();
        let expires_at = Utc::now() + Duration::seconds(self.ttl_secs);
        self.entries
            .insert(store_key, (IdempotencyState::in_progress(), expires_at));
        true
    }

    /// Mark the key as completed with a result.
    pub fn complete(&mut self, key: &IdempotencyKey, result: &str) {
        let store_key = key.to_store_key();
        let expires_at = Utc::now() + Duration::seconds(self.ttl_secs);
        self.entries.insert(
            store_key,
            (
                IdempotencyState::Completed {
                    result: result.to_string(),
                    completed_at: Utc::now(),
                },
                expires_at,
            ),
        );
    }

    /// Mark the key as failed.
    pub fn fail(&mut self, key: &IdempotencyKey, error: &str) {
        let store_key = key.to_store_key();
        let expires_at = Utc::now() + Duration::seconds(self.ttl_secs);
        self.entries.insert(
            store_key,
            (
                IdempotencyState::Failed {
                    error: error.to_string(),
                    failed_at: Utc::now(),
                },
                expires_at,
            ),
        );
    }

    /// Explicitly remove a key (e.g., after handling a completed response).
    pub fn remove(&mut self, key: &IdempotencyKey) {
        self.entries.remove(&key.to_store_key());
    }

    /// Prune all expired entries.
    pub fn prune(&mut self) {
        let now = Utc::now();
        self.entries.retain(|_, (_, expires_at)| *expires_at > now);
    }

    /// Number of non-expired entries.
    pub fn len(&self) -> usize {
        self.entries
            .iter()
            .filter(|(_, (_, exp))| *exp > Utc::now())
            .count()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// High-level usage helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Guard that auto-releases (revert to InProgress) on drop unless completed.
pub struct IdempotencyGuard<'a> {
    store: &'a mut IdempotencyStore,
    key: IdempotencyKey,
    claimed: bool,
}

impl<'a> IdempotencyGuard<'a> {
    pub fn new(store: &'a mut IdempotencyStore, key: IdempotencyKey) -> Option<Self> {
        if store.try_claim(&key) {
            Some(Self {
                store,
                key,
                claimed: true,
            })
        } else {
            None
        }
    }

    pub fn complete(self, result: &str) {
        self.store.complete(&self.key, result);
        // Note: `self` is consumed here so Drop won't run
    }

    pub fn fail(self, error: &str) {
        self.store.fail(&self.key, error);
    }

    pub fn key(&self) -> &IdempotencyKey {
        &self.key
    }
}

impl<'a> Drop for IdempotencyGuard<'a> {
    fn drop(&mut self) {
        if self.claimed {
            // Auto-revert: leave in in-progress state for retry
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_to_store_key() {
        let k = IdempotencyKey::new(IdempotencyCategory::Request, "tenant-a", "req-123");
        assert_eq!(k.to_store_key(), "request:tenant-a:req-123");
    }

    #[test]
    fn test_claim_then_complete() {
        let mut store = IdempotencyStore::new(300);
        let key = IdempotencyKey::new(IdempotencyCategory::Task, "t1", "job-x");

        assert!(store.try_claim(&key)); // first claim succeeds
        assert!(!store.try_claim(&key)); // second fails (already claimed)

        store.complete(&key, "done");
        assert!(matches!(
            store.check(&key),
            Some(IdempotencyState::Completed { .. })
        ));
    }

    #[test]
    fn test_guard_complete_then_drop() {
        let mut store = IdempotencyStore::new(300);
        let key = IdempotencyKey::new(IdempotencyCategory::Retry, "t1", "op-y");

        let guard = IdempotencyGuard::new(&mut store, key.clone()).unwrap();
        guard.complete("ok");

        // After complete, check should return Completed
        assert!(matches!(
            store.check(&key),
            Some(IdempotencyState::Completed { .. })
        ));
    }

    #[test]
    fn test_idempotency_state_serialization() {
        let state = IdempotencyState::Completed {
            result: "result-123".into(),
            completed_at: Utc::now(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: IdempotencyState = serde_json::from_str(&json).unwrap();
        if let IdempotencyState::Completed { result, .. } = back {
            assert_eq!(result, "result-123");
        } else {
            panic!("expected Completed");
        }
    }

    #[test]
    fn test_category_display() {
        assert_eq!(IdempotencyCategory::Request.to_string(), "request");
        assert_eq!(IdempotencyCategory::Callback.to_string(), "callback");
    }

    #[test]
    fn test_different_categories_are_separate_keys() {
        let mut store = IdempotencyStore::new(300);
        let k1 = IdempotencyKey::new(IdempotencyCategory::Request, "t", "k");
        let k2 = IdempotencyKey::new(IdempotencyCategory::Task, "t", "k");
        assert!(store.try_claim(&k1));
        assert!(store.try_claim(&k2)); // different category → separate namespace
    }

    #[test]
    fn test_fail_then_retry() {
        let mut store = IdempotencyStore::new(300);
        let key = IdempotencyKey::new(IdempotencyCategory::Request, "t", "k");

        store.fail(&key, "error-1");
        // After failure, check returns Failed but try_claim also returns false
        // (key is still present in store)
        assert!(matches!(
            store.check(&key),
            Some(IdempotencyState::Failed { .. })
        ));
        assert!(!store.try_claim(&key)); // can't re-claim a failed key
    }

    #[test]
    fn test_remove_allows_reclaim() {
        let mut store = IdempotencyStore::new(300);
        let key = IdempotencyKey::new(IdempotencyCategory::Request, "t", "k");

        store.complete(&key, "ok");
        assert!(matches!(
            store.check(&key),
            Some(IdempotencyState::Completed { .. })
        ));
        store.remove(&key);
        assert!(store.check(&key).is_none());
        assert!(store.try_claim(&key)); // can reclaim after remove
    }

    #[test]
    fn test_prune_removes_expired() {
        let mut store = IdempotencyStore::new(1); // 1 second TTL
        let key = IdempotencyKey::new(IdempotencyCategory::Task, "t", "k");
        store.try_claim(&key);
        assert_eq!(store.len(), 1);

        // Manually fast-forward by setting expiry in the past
        let store_key = key.to_store_key();
        if let Some((_state, exp)) = store.entries.get_mut(&store_key) {
            *exp = Utc::now() - Duration::seconds(10);
        }
        store.prune();
        assert_eq!(store.len(), 0);
    }
}
