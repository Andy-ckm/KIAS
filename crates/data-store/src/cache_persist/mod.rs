//! # SQLite-backed Cache Strategy
//!
//! Provides a persistent key-value cache with TTL support, backed by SQLite.
//! This is a self-contained cache implementation that doesn't depend on
//! `kias-cache` — keeping `data-store` at L1 (depends only on `kias-common`).
//!
//! ## Features
//!
//! - TTL-based expiration (lazy cleanup on read)
//! - Namespace support (multiple independent caches)
//! - Access counting for hit-rate monitoring
//! - Write-through semantics
//! - Compatible with the `CacheStrategy` trait pattern from kias-cache

use async_trait::async_trait;
use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::debug;

/// A cache entry with key, value, and TTL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ttl: Option<std::time::Duration>,
}

impl CacheEntry {
    /// Create a new cache entry without TTL.
    pub fn new(key: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            key: key.into(),
            value,
            created_at: chrono::Utc::now(),
            ttl: None,
        }
    }

    /// Create a new cache entry with TTL.
    pub fn with_ttl(key: impl Into<String>, value: Vec<u8>, ttl: std::time::Duration) -> Self {
        Self {
            key: key.into(),
            value,
            created_at: chrono::Utc::now(),
            ttl: Some(ttl),
        }
    }
}

/// Cache strategy trait (mirrors `kias_cache::CacheStrategy` for L1 independence).
#[async_trait]
pub trait CacheStrategy: Send + Sync {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>>;
    async fn set(&self, entry: CacheEntry) -> KiasResult<()>;
    async fn evict(&self, key: &str) -> KiasResult<()>;
}

/// SQLite-backed cache strategy.
///
/// Drop-in replacement for in-memory caches with persistence.
pub struct SqliteCacheStrategy {
    pool: SqlitePool,
    namespace: String,
}

impl SqliteCacheStrategy {
    /// Create a new SQLite cache strategy.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            namespace: "default".to_string(),
        }
    }

    /// Create with a specific namespace.
    pub fn with_namespace(pool: SqlitePool, namespace: impl Into<String>) -> Self {
        Self {
            pool,
            namespace: namespace.into(),
        }
    }

    /// Evict all expired entries (manual cleanup).
    pub async fn evict_expired(&self) -> KiasResult<u64> {
        let result = sqlx::query(
            "DELETE FROM cache_entries WHERE ttl_seconds IS NOT NULL AND datetime(created_at, '+' || ttl_seconds || ' seconds') < datetime('now')"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to evict expired cache entries: {e}")))?;

        let count = result.rows_affected();
        if count > 0 {
            debug!("Evicted {count} expired cache entries");
        }
        Ok(count)
    }

    /// Clear all entries in this namespace.
    pub async fn clear(&self) -> KiasResult<()> {
        sqlx::query("DELETE FROM cache_entries WHERE namespace = ?")
            .bind(&self.namespace)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to clear cache: {e}")))?;
        Ok(())
    }

    /// Count entries in this namespace.
    pub async fn size(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries WHERE namespace = ?")
            .bind(&self.namespace)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count cache entries: {e}")))?;
        Ok(row.0)
    }

    /// Get the namespace for this cache instance.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

#[async_trait]
impl CacheStrategy for SqliteCacheStrategy {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        let row: Option<(Vec<u8>, Option<i64>, String, i64)> = sqlx::query_as(
            "SELECT value, ttl_seconds, created_at, access_count FROM cache_entries WHERE key = ? AND namespace = ?"
        )
        .bind(key)
        .bind(&self.namespace)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to get cache entry: {e}")))?;

        match row {
            Some((value, ttl_seconds, _created_at, _access_count)) => {
                // Check TTL using SQL for consistent datetime handling
                if ttl_seconds.is_some() {
                    let expired: Option<(i64,)> = sqlx::query_as(
                        "SELECT 1 FROM cache_entries WHERE key = ? AND namespace = ? AND ttl_seconds IS NOT NULL AND datetime(created_at, '+' || ttl_seconds || ' seconds') < datetime('now')"
                    )
                    .bind(key)
                    .bind(&self.namespace)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| KiasError::Config(format!("Failed to check TTL: {e}")))?;

                    if expired.is_some() {
                        sqlx::query("DELETE FROM cache_entries WHERE key = ? AND namespace = ?")
                            .bind(key)
                            .bind(&self.namespace)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| {
                                KiasError::Config(format!("Failed to delete expired entry: {e}"))
                            })?;
                        return Ok(None);
                    }
                }

                // Update access count
                sqlx::query(
                    "UPDATE cache_entries SET access_count = access_count + 1, accessed_at = datetime('now') WHERE key = ? AND namespace = ?"
                )
                .bind(key)
                .bind(&self.namespace)
                .execute(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to update access count: {e}")))?;

                let ttl = ttl_seconds.map(|s| std::time::Duration::from_secs(s as u64));
                let mut entry = CacheEntry::new(key, value);
                entry.ttl = ttl;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        let ttl_seconds = entry.ttl.map(|d| d.as_secs() as i64);
        let created_at = entry.created_at.to_rfc3339();

        sqlx::query(
            "INSERT OR REPLACE INTO cache_entries (key, value, namespace, ttl_seconds, created_at, accessed_at) VALUES (?, ?, ?, ?, ?, datetime('now'))"
        )
        .bind(&entry.key)
        .bind(&entry.value)
        .bind(&self.namespace)
        .bind(ttl_seconds)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to set cache entry: {e}")))?;

        debug!("Cache set: {} ({} bytes)", entry.key, entry.value.len());
        Ok(())
    }

    async fn evict(&self, key: &str) -> KiasResult<()> {
        let result = sqlx::query("DELETE FROM cache_entries WHERE key = ? AND namespace = ?")
            .bind(key)
            .bind(&self.namespace)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to evict cache entry: {e}")))?;

        if result.rows_affected() == 0 {
            debug!("Cache evict: {key} not found");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::MigrationRunner;

    async fn setup_cache() -> SqliteCacheStrategy {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        MigrationRunner::new(pool.clone())
            .run_all()
            .await
            .expect("Failed to run migrations");
        SqliteCacheStrategy::new(pool)
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let cache = setup_cache().await;

        let entry = CacheEntry::new("key1", b"hello".to_vec());
        cache.set(entry).await.expect("Failed to set");

        let result = cache.get("key1").await.expect("Failed to get");
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.value, b"hello");
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let cache = setup_cache().await;
        let result = cache.get("missing").await.expect("Failed to get");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_evict() {
        let cache = setup_cache().await;

        cache
            .set(CacheEntry::new("key1", b"val".to_vec()))
            .await
            .unwrap();
        assert!(cache.get("key1").await.unwrap().is_some());

        cache.evict("key1").await.unwrap();
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_overwrite() {
        let cache = setup_cache().await;

        cache
            .set(CacheEntry::new("key1", b"first".to_vec()))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("key1", b"second".to_vec()))
            .await
            .unwrap();

        let result = cache.get("key1").await.unwrap().unwrap();
        assert_eq!(result.value, b"second");
    }

    #[tokio::test]
    async fn test_namespace_isolation() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        MigrationRunner::new(pool.clone()).run_all().await.unwrap();

        let cache_a = SqliteCacheStrategy::with_namespace(pool.clone(), "ns-a");
        let cache_b = SqliteCacheStrategy::with_namespace(pool, "ns-b");

        cache_a
            .set(CacheEntry::new("shared-key", b"from-a".to_vec()))
            .await
            .unwrap();
        cache_b
            .set(CacheEntry::new("shared-key", b"from-b".to_vec()))
            .await
            .unwrap();

        let val_a = cache_a.get("shared-key").await.unwrap().unwrap();
        let val_b = cache_b.get("shared-key").await.unwrap().unwrap();

        assert_eq!(val_a.value, b"from-a");
        assert_eq!(val_b.value, b"from-b");
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = setup_cache().await;

        cache
            .set(CacheEntry::new("k1", b"v1".to_vec()))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("k2", b"v2".to_vec()))
            .await
            .unwrap();
        assert_eq!(cache.size().await.unwrap(), 2);

        cache.clear().await.unwrap();
        assert_eq!(cache.size().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_access_count() {
        let cache = setup_cache().await;
        cache
            .set(CacheEntry::new("key1", b"val".to_vec()))
            .await
            .unwrap();

        cache.get("key1").await.unwrap();
        cache.get("key1").await.unwrap();
        cache.get("key1").await.unwrap();

        let count: (i64,) =
            sqlx::query_as("SELECT access_count FROM cache_entries WHERE key = 'key1'")
                .fetch_one(&cache.pool)
                .await
                .unwrap();
        assert_eq!(count.0, 3);
    }

    #[tokio::test]
    async fn test_size() {
        let cache = setup_cache().await;
        assert_eq!(cache.size().await.unwrap(), 0);

        cache.set(CacheEntry::new("a", vec![1])).await.unwrap();
        cache.set(CacheEntry::new("b", vec![2])).await.unwrap();
        assert_eq!(cache.size().await.unwrap(), 2);

        cache.evict("a").await.unwrap();
        assert_eq!(cache.size().await.unwrap(), 1);
    }
}
