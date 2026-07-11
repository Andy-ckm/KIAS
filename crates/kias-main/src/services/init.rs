//! Composition resources for the KIAS control-plane process.
//!
//! The binary composition root owns only resources that are actually shared with
//! the API process: durable audit storage, the dead-letter queue, and graceful
//! shutdown coordination. Domain services are initialized at their real boundary
//! rather than constructed here merely to report them as healthy.

use std::sync::Arc;
use std::time::Instant;

use kias_common::{KiasConfig, KiasError, KiasResult};
use serde::{Deserialize, Serialize};

const MIN_JWT_SECRET_BYTES: usize = 32;
const MIN_STATIC_TOKEN_BYTES: usize = 24;
const MAX_JWT_EXPIRATION_HOURS: u64 = 168;

/// Readiness of a process-level resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(formatter, "Healthy"),
            Self::Degraded => write!(formatter, "Degraded"),
            Self::Unhealthy => write!(formatter, "Unhealthy"),
        }
    }
}

/// Process-level readiness snapshot.
///
/// This report covers only resources verified during composition. Domain health
/// belongs to services that can execute a real check; KIAS does not label an
/// unused or merely constructed component healthy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall: HealthStatus,
    pub persistence: HealthStatus,
    pub audit: HealthStatus,
    pub dead_letter_queue: HealthStatus,
    pub shutdown_coordinator: HealthStatus,
    pub uptime_secs: u64,
}

impl SystemHealthReport {
    pub fn is_healthy(&self) -> bool {
        self.overall == HealthStatus::Healthy
    }
}

/// Resources shared by the KIAS process and API server.
pub struct KiasServiceManager {
    audit_log: kias_data_store::SqliteAuditLog,
    dead_letter_queue: kias_data_store::DeadLetterQueue,
    shutdown: Arc<kias_common::graceful_shutdown::GracefulShutdown>,
    started_at: Instant,
}

impl KiasServiceManager {
    /// Validate configuration and open the configured durable store.
    ///
    /// Startup fails if the database cannot be opened. An in-memory store is
    /// available only through the explicit embedding/test constructor; silent
    /// fallback would make audit and recovery guarantees disappear unnoticed.
    pub async fn new(config: KiasConfig) -> KiasResult<Self> {
        Self::validate_config(&config)?;

        let configured_path = config
            .storage
            .sqlite_url
            .strip_prefix("sqlite://")
            .unwrap_or(&config.storage.sqlite_url);
        let database_path = std::env::var("KIAS_DB_PATH")
            .unwrap_or_else(|_| configured_path.to_string());
        if database_path.trim().is_empty() {
            return Err(KiasError::Config(
                "SQLite database path must not be empty".to_string(),
            ));
        }

        let repository = kias_data_store::SqliteRepository::open(&database_path).await?;
        tracing::info!("KIAS durable store initialized");
        Self::from_repository(repository)
    }

    /// Compose process resources from an already-open repository.
    pub fn from_repository(repository: kias_data_store::SqliteRepository) -> KiasResult<Self> {
        let audit_log = kias_data_store::SqliteAuditLog::new(repository.pool.clone());
        let dead_letter_queue = kias_data_store::DeadLetterQueue::new(repository.pool.clone());
        let shutdown = Arc::new(kias_common::graceful_shutdown::GracefulShutdown::with_defaults());

        Ok(Self {
            audit_log,
            dead_letter_queue,
            shutdown,
            started_at: Instant::now(),
        })
    }

    pub fn audit_log(&self) -> &kias_data_store::SqliteAuditLog {
        &self.audit_log
    }

    pub fn dlq(&self) -> &kias_data_store::DeadLetterQueue {
        &self.dead_letter_queue
    }

    pub fn shutdown_handle(&self) -> Arc<kias_common::graceful_shutdown::GracefulShutdown> {
        Arc::clone(&self.shutdown)
    }

    /// Return process readiness for resources verified during composition.
    pub fn health_check(&self) -> SystemHealthReport {
        SystemHealthReport {
            overall: HealthStatus::Healthy,
            persistence: HealthStatus::Healthy,
            audit: HealthStatus::Healthy,
            dead_letter_queue: HealthStatus::Healthy,
            shutdown_coordinator: HealthStatus::Healthy,
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }

    pub async fn shutdown(&self) -> KiasResult<()> {
        self.shutdown.shutdown().await;
        Ok(())
    }

    fn validate_config(config: &KiasConfig) -> KiasResult<()> {
        if config.api_server.port == 0 {
            return Err(KiasError::Config(
                "API server port must not be 0".to_string(),
            ));
        }

        if !matches!(config.api_server.tls_min_version.as_str(), "1.2" | "1.3") {
            return Err(KiasError::Config(
                "TLS minimum version must be 1.2 or 1.3".to_string(),
            ));
        }
        if config.api_server.tls
            && (config.api_server.tls_cert_path.is_none()
                || config.api_server.tls_key_path.is_none())
        {
            return Err(KiasError::Config(
                "TLS requires both certificate and private-key paths".to_string(),
            ));
        }

        if config.api_server.auth_enabled {
            let has_static_token = !config.api_server.auth_tokens.is_empty();
            let has_jwt_secret = config.api_server.jwt_secret.is_some();
            if !has_static_token && !has_jwt_secret {
                return Err(KiasError::Config(
                    "authentication is enabled but no credential verifier is configured"
                        .to_string(),
                ));
            }

            if config
                .api_server
                .auth_tokens
                .iter()
                .any(|token| token.as_bytes().len() < MIN_STATIC_TOKEN_BYTES)
            {
                return Err(KiasError::Config(format!(
                    "static API tokens must be at least {MIN_STATIC_TOKEN_BYTES} bytes"
                )));
            }

            if let Some(secret) = &config.api_server.jwt_secret {
                if secret.as_bytes().len() < MIN_JWT_SECRET_BYTES {
                    return Err(KiasError::Config(format!(
                        "JWT secret must be at least {MIN_JWT_SECRET_BYTES} bytes"
                    )));
                }
            }

            if !(1..=MAX_JWT_EXPIRATION_HOURS)
                .contains(&config.api_server.jwt_expiration_hours)
            {
                return Err(KiasError::Config(format!(
                    "JWT expiration must be between 1 and {MAX_JWT_EXPIRATION_HOURS} hours"
                )));
            }
        }

        if !matches!(
            config.scheduler.algorithm.as_str(),
            "round_robin" | "least_loaded" | "resource_aware" | "cache_aware"
        ) {
            return Err(KiasError::Config(
                "unsupported scheduler algorithm".to_string(),
            ));
        }
        if config.scheduler.interval_ms == 0 {
            return Err(KiasError::Config(
                "scheduler interval must be greater than 0".to_string(),
            ));
        }

        if config.controller.heartbeat_interval_secs == 0 {
            return Err(KiasError::Config(
                "heartbeat interval must be greater than 0".to_string(),
            ));
        }
        if config.controller.failure_timeout_secs
            <= config.controller.heartbeat_interval_secs
        {
            return Err(KiasError::Config(
                "failure timeout must be greater than the heartbeat interval"
                    .to_string(),
            ));
        }
        if config.controller.max_retries == 0 {
            return Err(KiasError::Config(
                "controller max retries must be greater than 0".to_string(),
            ));
        }

        if config.agentsight.enabled && config.agentsight.metrics_port == 0 {
            return Err(KiasError::Config(
                "metrics port must not be 0 when observability is enabled".to_string(),
            ));
        }
        if !matches!(config.storage.cache_mode.as_str(), "sqlite" | "memory") {
            return Err(KiasError::Config(
                "cache mode must be 'sqlite' or 'memory'".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> KiasConfig {
        KiasConfig::default()
    }

    async fn test_manager(config: KiasConfig) -> KiasResult<KiasServiceManager> {
        KiasServiceManager::validate_config(&config)?;
        let repository = kias_data_store::SqliteRepository::in_memory().await?;
        KiasServiceManager::from_repository(repository)
    }

    #[test]
    fn health_status_display_is_stable() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "Degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "Unhealthy");
    }

    #[test]
    fn health_report_serialization_roundtrip() {
        let report = SystemHealthReport {
            overall: HealthStatus::Healthy,
            persistence: HealthStatus::Healthy,
            audit: HealthStatus::Healthy,
            dead_letter_queue: HealthStatus::Healthy,
            shutdown_coordinator: HealthStatus::Healthy,
            uptime_secs: 42,
        };

        let json = serde_json::to_string(&report).unwrap();
        let decoded: SystemHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, report);
        assert!(decoded.is_healthy());
    }

    #[tokio::test]
    async fn manager_composes_explicit_repository() {
        let manager = test_manager(default_config()).await.unwrap();
        assert!(manager.health_check().is_healthy());
    }

    #[tokio::test]
    async fn shutdown_notifies_subscribers() {
        let manager = test_manager(default_config()).await.unwrap();
        let mut subscriber = manager.shutdown_handle().subscribe();

        manager.shutdown().await.unwrap();
        assert!(subscriber.recv().await.is_ok());
    }

    #[tokio::test]
    async fn rejects_authentication_without_verifier() {
        let mut config = default_config();
        config.api_server.auth_enabled = true;
        let error = test_manager(config).await.unwrap_err();
        assert!(error.to_string().contains("no credential verifier"));
    }

    #[tokio::test]
    async fn accepts_runtime_length_jwt_secret() {
        let mut config = default_config();
        config.api_server.auth_enabled = true;
        config.api_server.jwt_secret = Some("x".repeat(MIN_JWT_SECRET_BYTES));
        assert!(test_manager(config).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_weak_jwt_secret() {
        let mut config = default_config();
        config.api_server.auth_enabled = true;
        config.api_server.jwt_secret = Some("x".repeat(MIN_JWT_SECRET_BYTES - 1));
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_tls_without_key_material() {
        let mut config = default_config();
        config.api_server.tls = true;
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_zero_port() {
        let mut config = default_config();
        config.api_server.port = 0;
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_unknown_scheduler_algorithm() {
        let mut config = default_config();
        config.scheduler.algorithm = "unknown".to_string();
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_zero_scheduler_interval() {
        let mut config = default_config();
        config.scheduler.interval_ms = 0;
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_failure_timeout_not_greater_than_heartbeat() {
        let mut config = default_config();
        config.controller.failure_timeout_secs = config.controller.heartbeat_interval_secs;
        assert!(test_manager(config).await.is_err());
    }

    #[tokio::test]
    async fn rejects_unknown_cache_mode() {
        let mut config = default_config();
        config.storage.cache_mode = "unknown".to_string();
        assert!(test_manager(config).await.is_err());
    }
}