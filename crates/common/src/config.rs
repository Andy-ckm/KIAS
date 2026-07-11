//! Configuration loading and structures for KIAS.
//!
//! Runtime configuration is loaded from `config/default.toml` or the file named
//! by `KIAS_CONFIG`, then overridden by environment variables prefixed with
//! `KIAS_`. Nested fields use a double underscore, for example
//! `KIAS_API_SERVER__PORT=9090`.

use serde::Deserialize;
use std::fmt;
use std::path::Path;

use crate::error::KiasError;

/// Root configuration for the KIAS process.
#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct KiasConfig {
    pub logging: LoggingConfig,
    pub api_server: ApiServerConfig,
    pub scheduler: SchedulerConfig,
    pub controller: ControllerConfig,
    /// Observability settings. The Rust field name is retained for pre-1.0 source
    /// compatibility; public configuration uses `[observability]`.
    #[serde(rename = "observability", alias = "agentsight")]
    pub agentsight: ObservabilityConfig,
    /// Optional cache settings. Public configuration uses `[cache]`.
    #[serde(rename = "cache", alias = "cache_hub")]
    pub cache_hub: CacheConfig,
    pub knowledge: KnowledgeConfig,
    pub storage: StorageConfig,
}

impl fmt::Debug for KiasConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiasConfig")
            .field("logging", &self.logging)
            .field("api_server", &self.api_server)
            .field("scheduler", &self.scheduler)
            .field("controller", &self.controller)
            .field("observability", &self.agentsight)
            .field("cache", &self.cache_hub)
            .field("knowledge", &self.knowledge)
            .field("storage", &"[ENDPOINTS REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: `trace`, `debug`, `info`, `warn`, or `error`.
    pub level: String,
    /// Output format: `text` or `json`.
    pub format: String,
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct ApiServerConfig {
    /// Bind address. The secure default is loopback only.
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub tls_client_ca_path: Option<String>,
    pub tls_min_version: String,
    pub auth_enabled: bool,
    /// Static API keys are supported for migration and local evaluation only.
    /// Prefer short-lived external identity credentials in deployments.
    pub auth_tokens: Vec<String>,
    pub jwt_secret: Option<String>,
    pub jwt_issuer: Option<String>,
    pub jwt_expiration_hours: u64,
}

impl fmt::Debug for ApiServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiServerConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .field("tls_cert_configured", &self.tls_cert_path.is_some())
            .field("tls_key_configured", &self.tls_key_path.is_some())
            .field("client_ca_configured", &self.tls_client_ca_path.is_some())
            .field("tls_min_version", &self.tls_min_version)
            .field("auth_enabled", &self.auth_enabled)
            .field("auth_token_count", &self.auth_tokens.len())
            .field("jwt_secret_configured", &self.jwt_secret.is_some())
            .field("jwt_issuer", &self.jwt_issuer)
            .field("jwt_expiration_hours", &self.jwt_expiration_hours)
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    /// `round_robin`, `least_loaded`, `resource_aware`, or `cache_aware`.
    pub algorithm: String,
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ControllerConfig {
    pub heartbeat_interval_secs: u64,
    pub failure_timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub enabled: bool,
    pub metrics_port: u16,
}

/// Pre-1.0 source compatibility alias. New code should use `ObservabilityConfig`.
#[deprecated(note = "use ObservabilityConfig")]
pub type AgentSightConfig = ObservabilityConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub ttl_secs: u64,
}

/// Pre-1.0 source compatibility alias. New code should use `CacheConfig`.
#[deprecated(note = "use CacheConfig")]
pub type CacheHubConfig = CacheConfig;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct KnowledgeConfig {
    pub enabled: bool,
    pub embedding_model: String,
    /// `local` or an explicitly configured remote provider.
    pub embedding_provider: String,
    /// Optional remote-provider credential. Never included in diagnostics.
    pub siliconflow_api_key: Option<String>,
    /// Optional remote-provider endpoint.
    pub siliconflow_base_url: String,
}

impl fmt::Debug for KnowledgeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KnowledgeConfig")
            .field("enabled", &self.enabled)
            .field("embedding_model", &self.embedding_model)
            .field("embedding_provider", &self.embedding_provider)
            .field("remote_api_key_configured", &self.siliconflow_api_key.is_some())
            .field("remote_endpoint_configured", &!self.siliconflow_base_url.is_empty())
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub etcd_endpoints: String,
    pub sqlite_url: String,
    /// `sqlite` (durable) or `memory` (explicitly volatile).
    pub cache_mode: String,
}

impl fmt::Debug for StorageConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StorageConfig")
            .field("etcd_configured", &!self.etcd_endpoints.is_empty())
            .field("sqlite_configured", &!self.sqlite_url.is_empty())
            .field("cache_mode", &self.cache_mode)
            .finish()
    }
}

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
            host: "127.0.0.1".into(),
            port: 8080,
            tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            tls_client_ca_path: None,
            tls_min_version: "1.3".into(),
            // Library defaults remain convenient for isolated unit tests. The
            // shipped runtime configuration explicitly enables authentication.
            auth_enabled: false,
            auth_tokens: vec![],
            jwt_secret: None,
            jwt_issuer: None,
            jwt_expiration_hours: 24,
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            algorithm: "resource_aware".into(),
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

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_port: 9090,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_entries: 10_000,
            ttl_secs: 3600,
        }
    }
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            embedding_model: "local-default".into(),
            embedding_provider: "local".into(),
            siliconflow_api_key: None,
            siliconflow_base_url: String::new(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            etcd_endpoints: "http://127.0.0.1:2379".into(),
            sqlite_url: "sqlite://kias.db".into(),
            cache_mode: "sqlite".into(),
        }
    }
}

impl KiasConfig {
    /// Load the required runtime configuration file and apply environment
    /// overrides. Missing configuration is an error rather than an implicit
    /// insecure fallback.
    pub fn load() -> Result<Self, KiasError> {
        let config_path =
            std::env::var("KIAS_CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        Self::from_file(&config_path)
    }

    pub fn from_file(path: &str) -> Result<Self, KiasError> {
        let path = Path::new(path);
        if !path.is_file() {
            return Err(KiasError::Config(format!(
                "configuration file not found: {}",
                path.display()
            )));
        }

        let format = match path.extension().and_then(|extension| extension.to_str()) {
            Some("toml") => config::FileFormat::Toml,
            Some("yaml") | Some("yml") => config::FileFormat::Yaml,
            Some("json") => config::FileFormat::Json,
            _ => config::FileFormat::Toml,
        };

        let cfg = config::Config::builder()
            .add_source(config::File::from(path).format(format))
            .add_source(
                config::Environment::with_prefix("KIAS")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?;

        Ok(cfg.try_deserialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_loopback_and_optional_extensions_are_disabled() {
        let config = KiasConfig::default();
        assert_eq!(config.api_server.host, "127.0.0.1");
        assert_eq!(config.api_server.port, 8080);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.scheduler.algorithm, "resource_aware");
        assert!(!config.cache_hub.enabled);
        assert!(!config.knowledge.enabled);
    }

    #[test]
    fn api_server_debug_redacts_credentials() {
        let mut config = ApiServerConfig::default();
        config.auth_tokens.push("sensitive-test-value".into());
        config.jwt_secret = Some("another-sensitive-test-value".into());

        let debug = format!("{config:?}");
        assert!(!debug.contains("sensitive-test-value"));
        assert!(!debug.contains("another-sensitive-test-value"));
        assert!(debug.contains("auth_token_count"));
    }

    #[test]
    fn load_partial_file_uses_safe_struct_defaults() {
        let source = r#"
[api_server]
port = 9090

[logging]
level = "debug"
"#;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.toml");
        std::fs::write(&path, source).unwrap();

        let config = KiasConfig::from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(config.api_server.host, "127.0.0.1");
        assert_eq!(config.api_server.port, 9090);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.scheduler.algorithm, "resource_aware");
    }

    #[test]
    fn missing_configuration_file_is_an_error() {
        let error = KiasConfig::from_file("definitely-not-present.toml").unwrap_err();
        assert!(error.to_string().contains("not found"));
    }
}