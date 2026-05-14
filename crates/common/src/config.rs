//! Configuration loading and structures for KIAS.
//!
//! Configuration is loaded from `config/default.toml` (or the file pointed at
//! by the `KIAS_CONFIG` environment variable), then overridden by environment
//! variables prefixed with `KIAS_` (e.g. `KIAS_API_SERVER_PORT=9090`).

use serde::Deserialize;
use std::path::Path;

use crate::error::KiasError;

// ── Top-level configuration ───────────────────────────────────────────

/// Root configuration for the entire KIAS system.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct KiasConfig {
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// API server configuration.
    pub api_server: ApiServerConfig,
    /// Scheduler configuration.
    pub scheduler: SchedulerConfig,
    /// Controller configuration.
    pub controller: ControllerConfig,
    /// AgentSight (observability) configuration.
    pub agentsight: AgentSightConfig,
    /// Cache Hub configuration.
    pub cache_hub: CacheHubConfig,
    /// Knowledge service configuration.
    pub knowledge: KnowledgeConfig,
    /// External storage endpoints.
    pub storage: StorageConfig,
}

// ── Sub-configurations ────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).  Default: `info`.
    pub level: String,
    /// Output format: `text` or `json`.  Default: `text`.
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ApiServerConfig {
    /// Bind address.  Default: `0.0.0.0`.
    pub host: String,
    /// Bind port.  Default: `8080`.
    pub port: u16,
    /// Whether to enable TLS.
    pub tls: bool,
    /// Path to the TLS certificate file (PEM format). Required when `tls=true`.
    pub tls_cert_path: Option<String>,
    /// Path to the TLS private key file (PEM format). Required when `tls=true`.
    pub tls_key_path: Option<String>,
    /// Path to the CA certificate for mutual TLS (mTLS). If set, client
    /// certificates signed by this CA are required.
    pub tls_client_ca_path: Option<String>,
    /// Minimum TLS version: `1.2` or `1.3`.  Default: `1.3`.
    pub tls_min_version: String,
    /// Whether to enable API key authentication.
    pub auth_enabled: bool,
    /// List of valid API keys.
    pub api_keys: Vec<String>,
    /// JWT secret for token-based authentication (optional).
    pub jwt_secret: Option<String>,
    /// JWT issuer claim (optional).
    pub jwt_issuer: Option<String>,
    /// JWT token expiration in hours (optional, default 24).
    pub jwt_expiration_hours: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    /// Scheduling algorithm: `round_robin`, `least_loaded`, `resource_aware`,
    /// `cache_aware`.  Default: `cache_aware`.
    pub algorithm: String,
    /// Scheduling interval in milliseconds.  Default: `1000`.
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ControllerConfig {
    /// Heartbeat interval in seconds.  Default: `15`.
    pub heartbeat_interval_secs: u64,
    /// Failure detection timeout in seconds.  Default: `60`.
    pub failure_timeout_secs: u64,
    /// Max retries for agent recovery.  Default: `3`.
    pub max_retries: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentSightConfig {
    /// Whether AgentSight is enabled.  Default: `true`.
    pub enabled: bool,
    /// Metrics port.  Default: `9090`.
    pub metrics_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CacheHubConfig {
    /// Whether Cache Hub is enabled.  Default: `true`.
    pub enabled: bool,
    /// Maximum cache entries.  Default: `10_000`.
    pub max_entries: usize,
    /// TTL in seconds.  Default: `3600`.
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KnowledgeConfig {
    /// Whether Knowledge service is enabled.  Default: `false`.
    pub enabled: bool,
    /// Embedding model name.  Default: `text-embedding-ada-002`.
    pub embedding_model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// etcd endpoints (comma-separated).  Default: `http://localhost:2379`.
    pub etcd_endpoints: String,
    /// SQLite database URL.  Default: `sqlite://kias.db`.
    pub sqlite_url: String,
    /// Cache mode: `local` or `redis`.  Default: `local`.
    pub cache_mode: String,
}

// ── Default implementations ───────────────────────────────────────────

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "text".into(),
        }
    }
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
            tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            tls_client_ca_path: None,
            tls_min_version: "1.3".into(),
            auth_enabled: false,
            api_keys: vec![],
            jwt_secret: None,
            jwt_issuer: None,
            jwt_expiration_hours: 24,
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            algorithm: "cache_aware".into(),
            interval_ms: 1000,
        }
    }
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 15,
            failure_timeout_secs: 60,
            max_retries: 3,
        }
    }
}

impl Default for AgentSightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_port: 9090,
        }
    }
}

impl Default for CacheHubConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 10_000,
            ttl_secs: 3600,
        }
    }
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            embedding_model: "text-embedding-ada-002".into(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            etcd_endpoints: "http://localhost:2379".into(),
            sqlite_url: "sqlite://kias.db".into(),
            cache_mode: "local".into(),
        }
    }
}

// ── Loading helpers ───────────────────────────────────────────────────

impl KiasConfig {
    /// Load configuration from the default path (`config/default.toml`) or
    /// from the path in the `KIAS_CONFIG` env var, with env-var overrides.
    pub fn load() -> Result<Self, KiasError> {
        let config_path =
            std::env::var("KIAS_CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        Self::from_file(&config_path)
    }

    /// Load configuration from a specific file, with env-var overrides.
    pub fn from_file(path: &str) -> Result<Self, KiasError> {
        let path = Path::new(path);

        let builder = config::Config::builder();

        // If the file exists, add it as a source; otherwise fall back to defaults.
        let builder = if path.exists() {
            let format = match path.extension().and_then(|e| e.to_str()) {
                Some("toml") => config::FileFormat::Toml,
                Some("yaml") | Some("yml") => config::FileFormat::Yaml,
                Some("json") => config::FileFormat::Json,
                _ => config::FileFormat::Toml,
            };
            builder.add_source(config::File::from(path).format(format))
        } else {
            tracing::warn!(
                path = %path.display(),
                "Config file not found, using defaults with env overrides"
            );
            builder
        };

        // Layer environment variables prefixed with `KIAS_` on top.
        let builder = builder.add_source(
            config::Environment::with_prefix("KIAS")
                .prefix_separator("_")
                .separator("__"),
        );

        let cfg = builder.build()?;
        let config: KiasConfig = cfg.try_deserialize()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = KiasConfig::default();
        assert_eq!(cfg.api_server.port, 8080);
        assert_eq!(cfg.logging.level, "info");
        assert_eq!(cfg.scheduler.algorithm, "cache_aware");
    }

    #[test]
    fn test_load_from_toml_string() {
        let toml_str = r#"
[api_server]
port = 9090

[logging]
level = "debug"
"#;
        // Write to a temp file, then load.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.toml");
        std::fs::write(&path, toml_str).unwrap();

        let cfg = KiasConfig::from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(cfg.api_server.port, 9090);
        assert_eq!(cfg.logging.level, "debug");
        // Unset fields keep defaults.
        assert_eq!(cfg.scheduler.algorithm, "cache_aware");
    }
}
