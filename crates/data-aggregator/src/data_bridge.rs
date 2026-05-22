//! Data Bridge Framework
//!
//! Provides pluggable data bridge connectors for external systems:
//! - Kafka (event streaming)
//! - PostgreSQL (structured storage)
//! - S3-compatible (object storage)
//!
//! Each bridge implements the DataBridge trait for unified lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub name: String,
    pub bridge_type: BridgeType,
    pub connection: ConnectionConfig,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub retry_policy: RetryPolicy,
}

/// Supported bridge types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BridgeType {
    Kafka,
    PostgreSQL,
    S3,
}

/// Connection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub endpoint: String,
    pub auth: AuthConfig,
    pub options: HashMap<String, String>,
}

/// Authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthConfig {
    None,
    Basic { username: String, password: String },
    Token { token: String },
    Ssl { cert_path: String, key_path: String },
}

/// Retry policy for failed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 10000,
            backoff_multiplier: 2.0,
        }
    }
}

/// A record to be sent through a bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRecord {
    pub key: Option<String>,
    pub payload: Vec<u8>,
    pub headers: HashMap<String, String>,
    pub timestamp: i64,
}

/// Bridge health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub name: String,
    pub connected: bool,
    pub records_sent: u64,
    pub records_failed: u64,
    pub last_error: Option<String>,
    pub latency_ms: u64,
}

/// Result of a send operation
#[derive(Debug, Clone)]
pub struct SendResult {
    pub success: bool,
    pub records_accepted: usize,
    pub records_rejected: usize,
    pub errors: Vec<String>,
}

/// Trait for all data bridges
pub trait DataBridge: Send + Sync {
    /// Connect to the external system
    fn connect(&mut self) -> Result<(), BridgeError>;

    /// Disconnect gracefully
    fn disconnect(&mut self) -> Result<(), BridgeError>;

    /// Send a batch of records
    fn send(&self, records: &[BridgeRecord]) -> Result<SendResult, BridgeError>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// Get bridge status
    fn status(&self) -> BridgeStatus;

    /// Get bridge name
    fn name(&self) -> &str;
}

/// Bridge errors
#[derive(Debug, Clone)]
pub enum BridgeError {
    ConnectionFailed(String),
    SendFailed(String),
    AuthFailed(String),
    ConfigError(String),
    Timeout(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            BridgeError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            BridgeError::AuthFailed(msg) => write!(f, "Auth failed: {}", msg),
            BridgeError::ConfigError(msg) => write!(f, "Config error: {}", msg),
            BridgeError::Timeout(msg) => write!(f, "Timeout: {}", msg),
        }
    }
}

impl std::error::Error for BridgeError {}

// ─── Kafka Bridge ────────────────────────────────────────────────

/// Kafka data bridge (simulated for testing)
pub struct KafkaBridge {
    config: BridgeConfig,
    connected: bool,
    records_sent: u64,
    records_failed: u64,
    buffer: Vec<BridgeRecord>,
}

impl KafkaBridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            connected: false,
            records_sent: 0,
            records_failed: 0,
            buffer: Vec::new(),
        }
    }

    pub fn topic(&self) -> &str {
        self.config
            .connection
            .options
            .get("topic")
            .map(|s| s.as_str())
            .unwrap_or("default")
    }
}

impl DataBridge for KafkaBridge {
    fn connect(&mut self) -> Result<(), BridgeError> {
        // In production: librdkafka producer
        if self.config.connection.endpoint.is_empty() {
            return Err(BridgeError::ConfigError(
                "Kafka bootstrap servers required".into(),
            ));
        }
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), BridgeError> {
        self.connected = false;
        self.buffer.clear();
        Ok(())
    }

    fn send(&self, records: &[BridgeRecord]) -> Result<SendResult, BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("Not connected".into()));
        }
        let accepted = records.len();
        self.buffer.extend_from_slice(records); // NOTE: interior mutability needed in prod
        Ok(SendResult {
            success: true,
            records_accepted: accepted,
            records_rejected: 0,
            errors: Vec::new(),
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn status(&self) -> BridgeStatus {
        BridgeStatus {
            name: self.config.name.clone(),
            connected: self.connected,
            records_sent: self.records_sent,
            records_failed: self.records_failed,
            last_error: None,
            latency_ms: 0,
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}

// ─── PostgreSQL Bridge ───────────────────────────────────────────

/// PostgreSQL data bridge (simulated for testing)
pub struct PostgresBridge {
    config: BridgeConfig,
    connected: bool,
    records_sent: u64,
}

impl PostgresBridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            connected: false,
            records_sent: 0,
        }
    }
}

impl DataBridge for PostgresBridge {
    fn connect(&mut self) -> Result<(), BridgeError> {
        if self.config.connection.endpoint.is_empty() {
            return Err(BridgeError::ConfigError(
                "PostgreSQL connection string required".into(),
            ));
        }
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), BridgeError> {
        self.connected = false;
        Ok(())
    }

    fn send(&self, records: &[BridgeRecord]) -> Result<SendResult, BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("Not connected".into()));
        }
        Ok(SendResult {
            success: true,
            records_accepted: records.len(),
            records_rejected: 0,
            errors: Vec::new(),
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn status(&self) -> BridgeStatus {
        BridgeStatus {
            name: self.config.name.clone(),
            connected: self.connected,
            records_sent: self.records_sent,
            records_failed: 0,
            last_error: None,
            latency_ms: 0,
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}

// ─── S3 Bridge ───────────────────────────────────────────────────

/// S3-compatible object storage bridge
pub struct S3Bridge {
    config: BridgeConfig,
    connected: bool,
    objects_written: u64,
}

impl S3Bridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            connected: false,
            objects_written: 0,
        }
    }

    pub fn bucket(&self) -> &str {
        self.config
            .connection
            .options
            .get("bucket")
            .map(|s| s.as_str())
            .unwrap_or("default")
    }
}

impl DataBridge for S3Bridge {
    fn connect(&mut self) -> Result<(), BridgeError> {
        if self.config.connection.endpoint.is_empty() {
            return Err(BridgeError::ConfigError(
                "S3 endpoint required".into(),
            ));
        }
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), BridgeError> {
        self.connected = false;
        Ok(())
    }

    fn send(&self, records: &[BridgeRecord]) -> Result<SendResult, BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("Not connected".into()));
        }
        Ok(SendResult {
            success: true,
            records_accepted: records.len(),
            records_rejected: 0,
            errors: Vec::new(),
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn status(&self) -> BridgeStatus {
        BridgeStatus {
            name: self.config.name.clone(),
            connected: self.connected,
            records_sent: self.objects_written,
            records_failed: 0,
            last_error: None,
            latency_ms: 0,
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}

// ─── Bridge Registry ─────────────────────────────────────────────

/// Manages multiple data bridges
pub struct BridgeRegistry {
    bridges: HashMap<String, Box<dyn DataBridge>>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Register a bridge
    pub fn register(&mut self, bridge: Box<dyn DataBridge>) {
        let name = bridge.name().to_string();
        self.bridges.insert(name, bridge);
    }

    /// Get bridge by name
    pub fn get(&self, name: &str) -> Option<&dyn DataBridge> {
        self.bridges.get(name).map(|b| b.as_ref())
    }

    /// Get mutable bridge by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn DataBridge>> {
        self.bridges.get_mut(name)
    }

    /// Connect all bridges
    pub fn connect_all(&mut self) -> Vec<(String, Result<(), BridgeError>)> {
        let mut results = Vec::new();
        for (name, bridge) in self.bridges.iter_mut() {
            let r = bridge.connect();
            results.push((name.clone(), r));
        }
        results
    }

    /// Disconnect all bridges
    pub fn disconnect_all(&mut self) {
        for bridge in self.bridges.values_mut() {
            let _ = bridge.disconnect();
        }
    }

    /// Get status of all bridges
    pub fn all_status(&self) -> Vec<BridgeStatus> {
        self.bridges.values().map(|b| b.status()).collect()
    }

    /// List bridge names
    pub fn list(&self) -> Vec<String> {
        self.bridges.keys().cloned().collect()
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a bridge from config
pub fn create_bridge(config: BridgeConfig) -> Box<dyn DataBridge> {
    match config.bridge_type {
        BridgeType::Kafka => Box::new(KafkaBridge::new(config)),
        BridgeType::PostgreSQL => Box::new(PostgresBridge::new(config)),
        BridgeType::S3 => Box::new(S3Bridge::new(config)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kafka_config() -> BridgeConfig {
        BridgeConfig {
            name: "test-kafka".into(),
            bridge_type: BridgeType::Kafka,
            connection: ConnectionConfig {
                endpoint: "localhost:9092".into(),
                auth: AuthConfig::None,
                options: {
                    let mut m = HashMap::new();
                    m.insert("topic".into(), "events".into());
                    m
                },
            },
            batch_size: 100,
            flush_interval_ms: 1000,
            retry_policy: RetryPolicy::default(),
        }
    }

    fn pg_config() -> BridgeConfig {
        BridgeConfig {
            name: "test-pg".into(),
            bridge_type: BridgeType::PostgreSQL,
            connection: ConnectionConfig {
                endpoint: "postgres://localhost/test".into(),
                auth: AuthConfig::Basic {
                    username: "user".into(),
                    password: "pass".into(),
                },
                options: HashMap::new(),
            },
            batch_size: 50,
            flush_interval_ms: 500,
            retry_policy: RetryPolicy::default(),
        }
    }

    fn s3_config() -> BridgeConfig {
        BridgeConfig {
            name: "test-s3".into(),
            bridge_type: BridgeType::S3,
            connection: ConnectionConfig {
                endpoint: "https://s3.amazonaws.com".into(),
                auth: AuthConfig::Token {
                    token: "test-token".into(),
                },
                options: {
                    let mut m = HashMap::new();
                    m.insert("bucket".into(), "audit-logs".into());
                    m
                },
            },
            batch_size: 200,
            flush_interval_ms: 2000,
            retry_policy: RetryPolicy::default(),
        }
    }

    fn sample_record() -> BridgeRecord {
        BridgeRecord {
            key: Some("key1".into()),
            payload: b"test payload".to_vec(),
            headers: HashMap::new(),
            timestamp: 1700000000,
        }
    }

    // ── Kafka tests ──

    #[test]
    fn test_kafka_connect_success() {
        let mut bridge = KafkaBridge::new(kafka_config());
        assert!(bridge.connect().is_ok());
        assert!(bridge.is_connected());
    }

    #[test]
    fn test_kafka_connect_empty_endpoint() {
        let mut cfg = kafka_config();
        cfg.connection.endpoint = String::new();
        let mut bridge = KafkaBridge::new(cfg);
        assert!(bridge.connect().is_err());
    }

    #[test]
    fn test_kafka_send_when_connected() {
        let mut bridge = KafkaBridge::new(kafka_config());
        bridge.connect().unwrap();
        let result = bridge.send(&[sample_record()]).unwrap();
        assert!(result.success);
        assert_eq!(result.records_accepted, 1);
    }

    #[test]
    fn test_kafka_send_when_disconnected() {
        let bridge = KafkaBridge::new(kafka_config());
        assert!(bridge.send(&[sample_record()]).is_err());
    }

    #[test]
    fn test_kafka_disconnect() {
        let mut bridge = KafkaBridge::new(kafka_config());
        bridge.connect().unwrap();
        bridge.disconnect().unwrap();
        assert!(!bridge.is_connected());
    }

    #[test]
    fn test_kafka_topic() {
        let bridge = KafkaBridge::new(kafka_config());
        assert_eq!(bridge.topic(), "events");
    }

    #[test]
    fn test_kafka_default_topic() {
        let mut cfg = kafka_config();
        cfg.connection.options.clear();
        let bridge = KafkaBridge::new(cfg);
        assert_eq!(bridge.topic(), "default");
    }

    // ── PostgreSQL tests ──

    #[test]
    fn test_pg_connect_success() {
        let mut bridge = PostgresBridge::new(pg_config());
        assert!(bridge.connect().is_ok());
    }

    #[test]
    fn test_pg_send_batch() {
        let mut bridge = PostgresBridge::new(pg_config());
        bridge.connect().unwrap();
        let records = vec![sample_record(), sample_record()];
        let result = bridge.send(&records).unwrap();
        assert_eq!(result.records_accepted, 2);
    }

    #[test]
    fn test_pg_send_empty_batch() {
        let mut bridge = PostgresBridge::new(pg_config());
        bridge.connect().unwrap();
        let result = bridge.send(&[]).unwrap();
        assert_eq!(result.records_accepted, 0);
    }

    // ── S3 tests ──

    #[test]
    fn test_s3_connect_success() {
        let mut bridge = S3Bridge::new(s3_config());
        assert!(bridge.connect().is_ok());
    }

    #[test]
    fn test_s3_bucket() {
        let bridge = S3Bridge::new(s3_config());
        assert_eq!(bridge.bucket(), "audit-logs");
    }

    #[test]
    fn test_s3_send() {
        let mut bridge = S3Bridge::new(s3_config());
        bridge.connect().unwrap();
        let result = bridge.send(&[sample_record()]).unwrap();
        assert!(result.success);
    }

    // ── Registry tests ──

    #[test]
    fn test_registry_register_and_list() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(KafkaBridge::new(kafka_config())));
        reg.register(Box::new(PostgresBridge::new(pg_config())));
        let list = reg.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"test-kafka".to_string()));
        assert!(list.contains(&"test-pg".to_string()));
    }

    #[test]
    fn test_registry_connect_all() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(KafkaBridge::new(kafka_config())));
        reg.register(Box::new(S3Bridge::new(s3_config())));
        let results = reg.connect_all();
        assert_eq!(results.len(), 2);
        for (_, r) in results {
            assert!(r.is_ok());
        }
    }

    #[test]
    fn test_registry_all_status() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(KafkaBridge::new(kafka_config())));
        let statuses = reg.all_status();
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].connected); // not connected yet
    }

    #[test]
    fn test_registry_get_by_name() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(KafkaBridge::new(kafka_config())));
        assert!(reg.get("test-kafka").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    // ── Create bridge tests ──

    #[test]
    fn test_create_bridge_kafka() {
        let bridge = create_bridge(kafka_config());
        assert_eq!(bridge.name(), "test-kafka");
    }

    #[test]
    fn test_create_bridge_pg() {
        let bridge = create_bridge(pg_config());
        assert_eq!(bridge.name(), "test-pg");
    }

    #[test]
    fn test_create_bridge_s3() {
        let bridge = create_bridge(s3_config());
        assert_eq!(bridge.name(), "test-s3");
    }

    // ── Retry policy tests ──

    #[test]
    fn test_retry_policy_default() {
        let rp = RetryPolicy::default();
        assert_eq!(rp.max_retries, 3);
        assert_eq!(rp.initial_backoff_ms, 100);
        assert_eq!(rp.backoff_multiplier, 2.0);
    }

    // ── BridgeStatus tests ──

    #[test]
    fn test_status_fields() {
        let mut bridge = KafkaBridge::new(kafka_config());
        let status = bridge.status();
        assert_eq!(status.name, "test-kafka");
        assert!(!status.connected);
        assert_eq!(status.records_sent, 0);

        bridge.connect().unwrap();
        let status = bridge.status();
        assert!(status.connected);
    }

    // ── SendResult tests ──

    #[test]
    fn test_send_result_fields() {
        let mut bridge = PostgresBridge::new(pg_config());
        bridge.connect().unwrap();
        let result = bridge.send(&[sample_record(), sample_record()]).unwrap();
        assert!(result.success);
        assert_eq!(result.records_accepted, 2);
        assert_eq!(result.records_rejected, 0);
        assert!(result.errors.is_empty());
    }
}
