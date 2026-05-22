use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Multi-protocol gateway framework for AgentGuard.
///
/// Inspired by EMQX's emqx_gateway — a plugin-based architecture where
/// each protocol is a gateway that can be loaded/unloaded/started/stopped.
///
/// Supported protocols:
/// - HTTP/REST (existing)
/// - A2A over MQTT (new)
/// - MCP Protocol (existing)
/// - gRPC (new)
/// - WebSocket (existing)
/// - CoAP (new - IoT)
/// - NATS (new - microservices)
/// - STOMP (new - messaging)
 ///
 ///   Gateway descriptor — defines a protocol gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayDescriptor {
    /// Unique gateway name
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Protocol name (http, mqtt, grpc, websocket, coap, nats, stomp)
    pub protocol: String,

    /// Default port
    pub default_port: u16,

    /// Whether this gateway supports TLS
    pub supports_tls: bool,

    /// Whether this gateway supports authentication
    pub supports_auth: bool,

    /// Gateway version
    pub version: String,
}

/// Runtime gateway instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayInstance {
    /// Gateway name
    pub name: String,

    /// Current status
    pub status: GatewayStatus,

    /// Bound address (host:port)
    pub bind_address: String,

    /// Configuration
    pub config: GatewayConfig,

    /// When it was started
    pub started_at: Option<DateTime<Utc>>,

    /// Connection count
    pub connections: u64,

    /// Error count
    pub errors: u64,
}

/// Gateway status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayStatus {
    /// Gateway is loaded but not started
    Loaded,
    /// Gateway is running
    Running,
    /// Gateway is stopped
    Stopped,
    /// Gateway encountered an error
    Error,
}

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Bind host
    pub host: String,

    /// Bind port
    pub port: u16,

    /// Enable TLS
    pub tls: bool,

    /// TLS cert path
    pub tls_cert: Option<String>,

    /// TLS key path
    pub tls_key: Option<String>,

    /// Max connections
    pub max_connections: u64,

    /// Enable authentication
    pub auth_enabled: bool,

    /// Custom protocol-specific settings
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            tls: false,
            tls_cert: None,
            tls_key: None,
            max_connections: 10000,
            auth_enabled: true,
            custom: HashMap::new(),
        }
    }
}

/// Gateway manager — manages all protocol gateways
pub struct GatewayManager {
    /// Registered gateway descriptors
    descriptors: Arc<RwLock<HashMap<String, GatewayDescriptor>>>,

    /// Running gateway instances
    instances: Arc<RwLock<HashMap<String, GatewayInstance>>>,
}

impl GatewayManager {
    pub fn new() -> Self {
        let manager = Self {
            descriptors: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
        };

        // Register built-in gateways
        manager.register_builtin();
        manager
    }

    fn register_builtin(&self) {
        // Use tokio::spawn to register built-in gateways asynchronously
        let descriptors = self.descriptors.clone();
        tokio::spawn(async move {
            let mut descs = descriptors.write().await;

            descs.insert(
                "http".to_string(),
                GatewayDescriptor {
                    name: "http".to_string(),
                    description: "HTTP/REST API gateway".to_string(),
                    protocol: "http".to_string(),
                    default_port: 8080,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "grpc".to_string(),
                GatewayDescriptor {
                    name: "grpc".to_string(),
                    description: "gRPC protocol gateway".to_string(),
                    protocol: "grpc".to_string(),
                    default_port: 9090,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "websocket".to_string(),
                GatewayDescriptor {
                    name: "websocket".to_string(),
                    description: "WebSocket gateway for real-time communication".to_string(),
                    protocol: "websocket".to_string(),
                    default_port: 8081,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "coap".to_string(),
                GatewayDescriptor {
                    name: "coap".to_string(),
                    description: "CoAP gateway for IoT devices (RFC 7252)".to_string(),
                    protocol: "coap".to_string(),
                    default_port: 5683,
                    supports_tls: true,
                    supports_auth: false,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "nats".to_string(),
                GatewayDescriptor {
                    name: "nats".to_string(),
                    description: "NATS messaging gateway for microservices".to_string(),
                    protocol: "nats".to_string(),
                    default_port: 4222,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "stomp".to_string(),
                GatewayDescriptor {
                    name: "stomp".to_string(),
                    description:
                        "STOMP messaging gateway (Simple Text Oriented Messaging Protocol)"
                            .to_string(),
                    protocol: "stomp".to_string(),
                    default_port: 61613,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "mqtt".to_string(),
                GatewayDescriptor {
                    name: "mqtt".to_string(),
                    description: "MQTT gateway for IoT/A2A communication".to_string(),
                    protocol: "mqtt".to_string(),
                    default_port: 1883,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );

            descs.insert(
                "mcp".to_string(),
                GatewayDescriptor {
                    name: "mcp".to_string(),
                    description: "Model Context Protocol gateway for Agent tool calls".to_string(),
                    protocol: "mcp".to_string(),
                    default_port: 3000,
                    supports_tls: true,
                    supports_auth: true,
                    version: "1.0.0".to_string(),
                },
            );
        });
    }

    /// Register a new gateway descriptor
    pub async fn register(&self, descriptor: GatewayDescriptor) {
        let mut descs = self.descriptors.write().await;
        descs.insert(descriptor.name.clone(), descriptor);
    }

    /// List all registered gateways
    pub async fn list_descriptors(&self) -> Vec<GatewayDescriptor> {
        let descs = self.descriptors.read().await;
        descs.values().cloned().collect()
    }

    /// Get a gateway descriptor
    pub async fn get_descriptor(&self, name: &str) -> Option<GatewayDescriptor> {
        let descs = self.descriptors.read().await;
        descs.get(name).cloned()
    }

    /// Start a gateway
    pub async fn start(&self, name: &str, config: GatewayConfig) -> Result<(), GatewayError> {
        let descs = self.descriptors.read().await;
        let descriptor = descs.get(name).ok_or_else(|| GatewayError::NotFound {
            name: name.to_string(),
        })?;

        let bind_address = format!("{}:{}", config.host, config.port);

        let instance = GatewayInstance {
            name: name.to_string(),
            status: GatewayStatus::Running,
            bind_address,
            config,
            started_at: Some(Utc::now()),
            connections: 0,
            errors: 0,
        };

        let mut instances = self.instances.write().await;
        instances.insert(name.to_string(), instance);

        tracing::info!(gateway = %name, protocol = %descriptor.protocol, "Gateway started");
        Ok(())
    }

    /// Stop a gateway
    pub async fn stop(&self, name: &str) -> Result<(), GatewayError> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(name)
            .ok_or_else(|| GatewayError::NotFound {
                name: name.to_string(),
            })?;

        instance.status = GatewayStatus::Stopped;
        tracing::info!(gateway = %name, "Gateway stopped");
        Ok(())
    }

    /// Get gateway instance status
    pub async fn status(&self, name: &str) -> Option<GatewayInstance> {
        let instances = self.instances.read().await;
        instances.get(name).cloned()
    }

    /// List all running instances
    pub async fn list_instances(&self) -> Vec<GatewayInstance> {
        let instances = self.instances.read().await;
        instances.values().cloned().collect()
    }

    /// Remove a gateway instance
    pub async fn remove(&self, name: &str) -> Result<(), GatewayError> {
        let mut instances = self.instances.write().await;
        instances
            .remove(name)
            .ok_or_else(|| GatewayError::NotFound {
                name: name.to_string(),
            })?;
        Ok(())
    }

    /// Get total connection count across all gateways
    pub async fn total_connections(&self) -> u64 {
        let instances = self.instances.read().await;
        instances.values().map(|i| i.connections).sum()
    }

    /// Get gateway count
    pub async fn gateway_count(&self) -> usize {
        let descs = self.descriptors.read().await;
        descs.len()
    }

    /// Get running instance count
    pub async fn running_count(&self) -> usize {
        let instances = self.instances.read().await;
        instances
            .values()
            .filter(|i| i.status == GatewayStatus::Running)
            .count()
    }
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Gateway errors
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("Gateway not found: {name}")]
    NotFound { name: String },

    #[error("Gateway already running: {name}")]
    AlreadyRunning { name: String },

    #[error("Gateway config error: {0}")]
    ConfigError(String),

    #[error("Gateway bind error: {0}")]
    BindError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_gateways_registered() {
        let manager = GatewayManager::new();
        // Wait for async registration
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let descs = manager.list_descriptors().await;
        assert!(
            descs.len() >= 8,
            "Expected at least 8 built-in gateways, got {}",
            descs.len()
        );

        let names: Vec<String> = descs.iter().map(|d| d.name.clone()).collect();
        assert!(names.contains(&"http".to_string()));
        assert!(names.contains(&"grpc".to_string()));
        assert!(names.contains(&"websocket".to_string()));
        assert!(names.contains(&"coap".to_string()));
        assert!(names.contains(&"nats".to_string()));
        assert!(names.contains(&"stomp".to_string()));
        assert!(names.contains(&"mqtt".to_string()));
        assert!(names.contains(&"mcp".to_string()));
    }

    #[tokio::test]
    async fn test_start_and_stop_gateway() {
        let manager = GatewayManager::new();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let config = GatewayConfig {
            port: 9999,
            ..Default::default()
        };

        manager.start("http", config).await.unwrap();

        let status = manager.status("http").await.unwrap();
        assert_eq!(status.status, GatewayStatus::Running);
        assert_eq!(status.bind_address, "0.0.0.0:9999");

        manager.stop("http").await.unwrap();
        let status = manager.status("http").await.unwrap();
        assert_eq!(status.status, GatewayStatus::Stopped);
    }

    #[tokio::test]
    async fn test_gateway_not_found() {
        let manager = GatewayManager::new();

        let result = manager.start("nonexistent", GatewayConfig::default()).await;
        assert!(matches!(result, Err(GatewayError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_list_instances() {
        let manager = GatewayManager::new();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        manager
            .start("http", GatewayConfig::default())
            .await
            .unwrap();
        manager
            .start(
                "grpc",
                GatewayConfig {
                    port: 9090,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let instances = manager.list_instances().await;
        assert_eq!(instances.len(), 2);
    }

    #[tokio::test]
    async fn test_custom_gateway() {
        let manager = GatewayManager::new();

        manager
            .register(GatewayDescriptor {
                name: "custom-protocol".to_string(),
                description: "Custom protocol gateway".to_string(),
                protocol: "custom".to_string(),
                default_port: 7777,
                supports_tls: false,
                supports_auth: false,
                version: "0.1.0".to_string(),
            })
            .await;

        let descs = manager.list_descriptors().await;
        assert!(descs.iter().any(|d| d.name == "custom-protocol"));
    }

    #[tokio::test]
    async fn test_total_connections() {
        let manager = GatewayManager::new();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(manager.total_connections().await, 0);
    }
}
