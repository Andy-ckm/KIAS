use async_trait::async_trait;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ttl: Option<std::time::Duration>,
}

impl CacheEntry {
    pub fn new(key: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            key: key.into(),
            value,
            created_at: chrono::Utc::now(),
            ttl: None,
        }
    }

    pub fn with_ttl(key: impl Into<String>, value: Vec<u8>, ttl: std::time::Duration) -> Self {
        Self {
            key: key.into(),
            value,
            created_at: chrono::Utc::now(),
            ttl: Some(ttl),
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let elapsed = chrono::Utc::now() - self.created_at;
            elapsed.to_std().unwrap_or(std::time::Duration::ZERO) > ttl
        } else {
            false
        }
    }
}

#[async_trait]
pub trait CacheStrategy: Send + Sync {
    async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>>;
    async fn set(&self, entry: CacheEntry) -> KiasResult<()>;
    async fn evict(&self, key: &str) -> KiasResult<()>;
}

pub struct CacheHub {
    strategy: Box<dyn CacheStrategy>,
}

impl CacheHub {
    pub fn new(strategy: Box<dyn CacheStrategy>) -> Self {
        Self { strategy }
    }

    pub async fn get(&self, key: &str) -> KiasResult<Option<CacheEntry>> {
        self.strategy.get(key).await
    }

    pub async fn set(&self, entry: CacheEntry) -> KiasResult<()> {
        self.strategy.set(entry).await
    }

    pub async fn evict(&self, key: &str) -> KiasResult<()> {
        self.strategy.evict(key).await
    }

    /// Get a value as UTF-8 string
    pub async fn get_string(&self, key: &str) -> KiasResult<Option<String>> {
        match self.get(key).await? {
            Some(entry) => Ok(Some(String::from_utf8_lossy(&entry.value).to_string())),
            None => Ok(None),
        }
    }

    /// Set a string value
    pub async fn set_string(
        &self,
        key: impl Into<String>,
        value: &str,
        ttl: Option<std::time::Duration>,
    ) -> KiasResult<()> {
        let key_str = key.into();
        let entry = match ttl {
            Some(t) => CacheEntry::with_ttl(key_str, value.as_bytes().to_vec(), t),
            None => CacheEntry::new(key_str, value.as_bytes().to_vec()),
        };
        self.set(entry).await
    }

    /// Get a value deserialized from JSON
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> KiasResult<Option<T>> {
        match self.get(key).await? {
            Some(entry) => {
                let value: T = serde_json::from_slice(&entry.value)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set a value serialized as JSON
    pub async fn set_json<T: Serialize>(
        &self,
        key: impl Into<String>,
        value: &T,
        ttl: Option<std::time::Duration>,
    ) -> KiasResult<()> {
        let bytes = serde_json::to_vec(value)?;
        let key_str = key.into();
        let entry = match ttl {
            Some(t) => CacheEntry::with_ttl(key_str, bytes, t),
            None => CacheEntry::new(key_str, bytes),
        };
        self.set(entry).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::LRUStrategy;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cache_entry_builder() {
        let entry = CacheEntry::new("k1", b"hello".to_vec());
        assert_eq!(entry.key, "k1");
        assert!(entry.ttl.is_none());
        assert!(!entry.is_expired());
    }

    #[tokio::test]
    async fn test_cache_entry_with_ttl() {
        let entry = CacheEntry::with_ttl("k1", b"hello".to_vec(), Duration::from_secs(60));
        assert!(entry.ttl.is_some());
        assert!(!entry.is_expired());
    }

    #[tokio::test]
    async fn test_cache_entry_expired() {
        let entry = CacheEntry::with_ttl("k1", b"hello".to_vec(), Duration::from_millis(1));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(entry.is_expired());
    }

    #[tokio::test]
    async fn test_hub_get_string() {
        let hub = CacheHub::new(Box::new(LRUStrategy::new()));
        hub.set_string("greeting", "hello world", None)
            .await
            .unwrap();
        let result = hub.get_string("greeting").await.unwrap();
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[tokio::test]
    async fn test_hub_get_string_miss() {
        let hub = CacheHub::new(Box::new(LRUStrategy::new()));
        let result = hub.get_string("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_hub_json_roundtrip() {
        let hub = CacheHub::new(Box::new(LRUStrategy::new()));
        let data = serde_json::json!({"name": "test", "value": 42});
        hub.set_json("json_key", &data, Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let result: Option<serde_json::Value> = hub.get_json("json_key").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["name"], "test");
    }

    #[tokio::test]
    async fn test_hub_evict() {
        let hub = CacheHub::new(Box::new(LRUStrategy::new()));
        hub.set_string("k1", "v1", None).await.unwrap();
        hub.evict("k1").await.unwrap();
        assert!(hub.get("k1").await.unwrap().is_none());
    }
}
