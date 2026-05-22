//! Data Bridge — cross-system data routing framework.
//!
//! Provides a unified abstraction for routing agent data to external systems.
//! Inspired by:
//! - EMQX Data Bridge (50+ connectors)
//! - Apache Camel message routing patterns
//! - AWS EventBridge event routing
//!
//! Pattern: Source → Transform → Sink with pluggable connectors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported bridge connector types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectorType {
    Kafka,
    Postgres,
    Mysql,
    Redis,
    S3,
    Http,
    Webhook,
    File,
    Mqtt,
    ElasticSearch,
}

/// Connection configuration for a bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub name: String,
    pub connector: ConnectorType,
    pub endpoint: String,
    pub credentials: Option<HashMap<String, String>>,
    /// Batch size for bulk operations.
    pub batch_size: usize,
    /// Flush interval in milliseconds.
    pub flush_interval_ms: u64,
    /// Max retry attempts on failure.
    pub max_retries: u32,
    /// Enable/disable this bridge.
    pub enabled: bool,
}

/// A data record to be routed through a bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRecord {
    pub record_id: String,
    pub source: String,
    pub topic: String,
    pub payload: serde_json::Value,
    pub timestamp_ms: u64,
    pub headers: HashMap<String, String>,
}

/// Result of a bridge send operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResult {
    pub success: bool,
    pub records_sent: usize,
    pub records_failed: usize,
    pub errors: Vec<String>,
    pub latency_ms: u64,
}

/// Transform function type: transforms a record before sending to sink.
pub type TransformFn = Box<dyn Fn(&BridgeRecord) -> BridgeRecord + Send + Sync>;

#[allow(dead_code)]
/// A data bridge connecting a source to a sink.
pub struct DataBridge {
    config: BridgeConfig,
    buffer: Vec<BridgeRecord>,
    total_sent: u64,
    total_failed: u64,
    transforms: Vec<String>, // Transform names (actual functions registered separately)
}

impl DataBridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            total_sent: 0,
            total_failed: 0,
            transforms: Vec::new(),
        }
    }

    /// Add a record to the buffer.
    pub fn enqueue(&mut self, record: BridgeRecord) {
        self.buffer.push(record);
    }

    /// Check if buffer should be flushed (batch size reached).
    pub fn should_flush(&self) -> bool {
        self.buffer.len() >= self.config.batch_size
    }

    /// Flush the buffer and return the result.
    pub fn flush(&mut self) -> BridgeResult {
        let records = std::mem::take(&mut self.buffer);
        let count = records.len();
        // In production, this would actually send to the connector
        self.total_sent += count as u64;
        BridgeResult {
            success: true,
            records_sent: count,
            records_failed: 0,
            errors: Vec::new(),
            latency_ms: 0,
        }
    }

    /// Get bridge statistics.
    pub fn stats(&self) -> BridgeStats {
        BridgeStats {
            name: self.config.name.clone(),
            connector: self.config.connector.clone(),
            total_sent: self.total_sent,
            total_failed: self.total_failed,
            buffer_size: self.buffer.len(),
            enabled: self.config.enabled,
        }
    }

    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStats {
    pub name: String,
    pub connector: ConnectorType,
    pub total_sent: u64,
    pub total_failed: u64,
    pub buffer_size: usize,
    pub enabled: bool,
}

/// Bridge manager — manages multiple data bridges.
pub struct BridgeManager {
    bridges: HashMap<String, DataBridge>,
}

impl BridgeManager {
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Register a new bridge.
    pub fn register(&mut self, config: BridgeConfig) -> Result<(), String> {
        let name = config.name.clone();
        if self.bridges.contains_key(&name) {
            return Err(format!("Bridge '{}' already registered", name));
        }
        self.bridges.insert(name, DataBridge::new(config));
        Ok(())
    }

    /// Remove a bridge.
    pub fn unregister(&mut self, name: &str) -> Result<(), String> {
        self.bridges
            .remove(name)
            .ok_or_else(|| format!("Bridge '{}' not found", name))?;
        Ok(())
    }

    /// Send a record to a specific bridge.
    pub fn send(&mut self, bridge_name: &str, record: BridgeRecord) -> Result<(), String> {
        let bridge = self
            .bridges
            .get_mut(bridge_name)
            .ok_or_else(|| format!("Bridge '{}' not found", bridge_name))?;

        if !bridge.is_enabled() {
            return Err(format!("Bridge '{}' is disabled", bridge_name));
        }

        bridge.enqueue(record);
        if bridge.should_flush() {
            bridge.flush();
        }
        Ok(())
    }

    /// Broadcast a record to all enabled bridges.
    pub fn broadcast(&mut self, record: BridgeRecord) -> Vec<(String, Result<(), String>)> {
        let names: Vec<String> = self.bridges.keys().cloned().collect();
        let mut results = Vec::new();
        for name in names {
            let r = self.send(&name, record.clone());
            results.push((name, r));
        }
        results
    }

    /// Flush all bridges.
    pub fn flush_all(&mut self) -> Vec<(String, BridgeResult)> {
        self.bridges
            .iter_mut()
            .filter(|(_, b)| !b.buffer.is_empty())
            .map(|(name, bridge)| (name.clone(), bridge.flush()))
            .collect()
    }

    /// Get stats for all bridges.
    pub fn all_stats(&self) -> Vec<BridgeStats> {
        self.bridges.values().map(|b| b.stats()).collect()
    }

    /// Get a bridge by name.
    pub fn get(&self, name: &str) -> Option<&DataBridge> {
        self.bridges.get(name)
    }
}

impl Default for BridgeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config(name: &str) -> BridgeConfig {
        BridgeConfig {
            name: name.to_string(),
            connector: ConnectorType::Kafka,
            endpoint: "localhost:9092".to_string(),
            credentials: None,
            batch_size: 2,
            flush_interval_ms: 1000,
            max_retries: 3,
            enabled: true,
        }
    }

    fn sample_record() -> BridgeRecord {
        BridgeRecord {
            record_id: "r1".to_string(),
            source: "test".to_string(),
            topic: "events".to_string(),
            payload: serde_json::json!({"key": "value"}),
            timestamp_ms: 0,
            headers: HashMap::new(),
        }
    }

    #[test]
    fn test_register_and_send() {
        let mut mgr = BridgeManager::new();
        mgr.register(sample_config("kafka")).unwrap();
        mgr.send("kafka", sample_record()).unwrap();
        // Buffer has 1, batch_size is 2, so not flushed yet
        assert_eq!(mgr.get("kafka").unwrap().stats().buffer_size, 1);
    }

    #[test]
    fn test_auto_flush_on_batch_size() {
        let mut mgr = BridgeManager::new();
        mgr.register(sample_config("kafka")).unwrap();
        mgr.send("kafka", sample_record()).unwrap();
        mgr.send("kafka", sample_record()).unwrap();
        // batch_size=2, should auto-flush
        assert_eq!(mgr.get("kafka").unwrap().stats().buffer_size, 0);
        assert_eq!(mgr.get("kafka").unwrap().stats().total_sent, 2);
    }

    #[test]
    fn test_broadcast() {
        let mut mgr = BridgeManager::new();
        mgr.register(sample_config("k1")).unwrap();
        mgr.register(sample_config("k2")).unwrap();
        let results = mgr.broadcast(sample_record());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
    }

    #[test]
    fn test_disabled_bridge_rejects() {
        let mut config = sample_config("disabled");
        config.enabled = false;
        let mut mgr = BridgeManager::new();
        mgr.register(config).unwrap();
        assert!(mgr.send("disabled", sample_record()).is_err());
    }

    #[test]
    fn test_unregister() {
        let mut mgr = BridgeManager::new();
        mgr.register(sample_config("k1")).unwrap();
        mgr.unregister("k1").unwrap();
        assert!(mgr.get("k1").is_none());
    }

    #[test]
    fn test_connector_types() {
        let types = vec![
            ConnectorType::Kafka,
            ConnectorType::Postgres,
            ConnectorType::S3,
            ConnectorType::Redis,
        ];
        for t in types {
            let config = BridgeConfig {
                name: format!("{:?}", t),
                connector: t.clone(),
                endpoint: "localhost".to_string(),
                credentials: None,
                batch_size: 100,
                flush_interval_ms: 1000,
                max_retries: 3,
                enabled: true,
            };
            let mut mgr = BridgeManager::new();
            mgr.register(config).unwrap();
        }
    }
}
