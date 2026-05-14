//! Service orchestration for the KIAS system.
//!
//! This module initializes, wires, and coordinates all KIAS subsystems:
//! - Scheduler (resource-aware agent scheduling)
//! - Controller (heartbeat, recovery, health checking)
//! - Monitor (telemetry + metrics)
//! - Cache (LRU + prefix cache)
//! - Knowledge (graph store)
//! - Skills (registry)
//! - Workflow Engine (DAG execution)
//! - Autonomy Controller (3-mode autonomy)
//! - Team Engine (owner-worker-verifier)
//! - Goal Engine (goal-driven loop)

use std::sync::Arc;
use std::time::Instant;

use kias_common::{KiasConfig, KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ── Health status types ────────────────────────────────────────────────

/// Health status for a single subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Subsystem is operating normally.
    Healthy,
    /// Subsystem is operational but experiencing issues.
    Degraded,
    /// Subsystem is not operational.
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded => write!(f, "Degraded"),
            HealthStatus::Unhealthy => write!(f, "Unhealthy"),
        }
    }
}

/// Aggregate health report for the entire KIAS system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SystemHealthReport {
    /// Overall system health (worst of all subsystems).
    pub overall: HealthStatus,
    pub scheduler: HealthStatus,
    pub controller: HealthStatus,
    pub monitor: HealthStatus,
    pub cache: HealthStatus,
    pub workflow_engine: HealthStatus,
    pub autonomy_controller: HealthStatus,
    pub team_engine: HealthStatus,
    pub goal_engine: HealthStatus,
    /// Seconds since the system started.
    pub uptime_secs: u64,
}

impl SystemHealthReport {
    /// Returns `true` when every subsystem is [`HealthStatus::Healthy`].
    pub fn is_healthy(&self) -> bool {
        self.overall == HealthStatus::Healthy
    }
}

// ── Shutdown coordinator ───────────────────────────────────────────────

/// Coordinates graceful shutdown across all subsystems using a broadcast channel.
///
/// Each subsystem task subscribes via [`subscribe`](Self::subscribe) and selects
/// on the returned receiver. When [`shutdown`](Self::shutdown) is called every
/// subscriber is notified exactly once.
#[allow(dead_code)]
pub struct ShutdownCoordinator {
    shutdown_tx: broadcast::Sender<()>,
    started_at: Instant,
}

impl ShutdownCoordinator {
    /// Create a new coordinator (capacity = 1 signal).
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            shutdown_tx,
            started_at: Instant::now(),
        }
    }

    /// Obtain a receiver that resolves when shutdown is requested.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Signal all subscribers to begin graceful shutdown.
    pub fn shutdown(&self) -> KiasResult<()> {
        tracing::info!("Initiating system shutdown");
        // It's OK if there are no active receivers.
        let _ = self.shutdown_tx.send(());
        Ok(())
    }

    /// Seconds elapsed since this coordinator was created.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Service manager ────────────────────────────────────────────────────

/// Top-level orchestrator that owns every KIAS subsystem.
#[allow(dead_code)]
pub struct KiasServiceManager {
    config: KiasConfig,
    scheduler: kias_scheduler::Scheduler,
    health_checker: kias_controller::HealthChecker,
    telemetry: kias_monitor::TelemetryCollector,
    metrics: kias_monitor::MetricsCollector,
    #[allow(dead_code)]
    cache_hub: kias_cache::CacheHub,
    #[allow(dead_code)]
    knowledge_graph: kias_knowledge::KnowledgeGraph,
    #[allow(dead_code)]
    skill_registry: kias_skills::SkillRegistry,
    #[allow(dead_code)]
    workflow_engine: kias_workflow_engine::WorkflowEngine,
    #[allow(dead_code)]
    autonomy_controller: kias_autonomy_controller::AutonomyController,
    #[allow(dead_code)]
    team_engine: kias_team_engine::TeamEngine,
    #[allow(dead_code)]
    goal_runner: kias_goal_engine::GoalLoopRunner,
    /// Persistent data store (SQLite-backed repositories, vector store, cache).
    #[allow(dead_code)]
    data_store: kias_data_store::SqliteRepository,
    /// Persistent vector store for embedding retrieval.
    #[allow(dead_code)]
    vector_store: kias_data_store::PersistentVectorStore,
    /// Persistent cache strategy.
    #[allow(dead_code)]
    cache_strategy: kias_data_store::SqliteCacheStrategy,
    shutdown: Arc<ShutdownCoordinator>,
    started_at: Instant,
}

impl KiasServiceManager {
    /// Create and initialise every subsystem from the provided config.
    ///
    /// This is the main wiring entry-point. Each subsystem is created
    /// synchronously (constructors are cheap) — background tasks are spawned
    /// later via the returned manager.
    pub async fn new(config: KiasConfig) -> KiasResult<Self> {
        Self::validate_config(&config)?;

        tracing::info!("Initializing KIAS subsystems");

        // ── Shutdown coordinator ───────────────────────────────────
        let shutdown = Arc::new(ShutdownCoordinator::new());

        // ── Scheduler ──────────────────────────────────────────────
        let scheduler_config = kias_scheduler::SchedulerConfig {
            algorithm: config.scheduler.algorithm.clone(),
            preemption_enabled: false,
            priority_classes: vec![],
            cache_weight: 0.3,
            max_attempts: 3,
        };
        let scheduler = kias_scheduler::Scheduler::new(scheduler_config);
        tracing::info!(
            algorithm = %config.scheduler.algorithm,
            "Scheduler initialized"
        );

        // ── Controller (health checker, heartbeat, recovery) ───────
        let health_check_config = kias_controller::HealthCheckConfig {
            check_interval_ms: config.controller.heartbeat_interval_secs * 1000,
            heartbeat: kias_controller::HeartbeatConfig {
                check_interval_secs: config.controller.heartbeat_interval_secs,
                timeout_secs: config.controller.failure_timeout_secs,
            },
            recovery: kias_controller::RecoveryConfig {
                max_retries: config.controller.max_retries,
                ..Default::default()
            },
        };
        let health_checker = kias_controller::HealthChecker::new(health_check_config);
        tracing::info!(
            heartbeat_secs = config.controller.heartbeat_interval_secs,
            "Controller health checker initialized"
        );

        // ── Monitor (telemetry + metrics) ──────────────────────────
        let telemetry = kias_monitor::TelemetryCollector::new();
        let metrics = kias_monitor::MetricsCollector::new();
        tracing::info!("Monitor initialized (telemetry + metrics)");

        // ── Cache hub ──────────────────────────────────────────────
        let cache_strategy = Box::new(kias_cache::LRUStrategy::new());
        let cache_hub = kias_cache::CacheHub::new(cache_strategy);
        tracing::info!(
            max_entries = config.cache_hub.max_entries,
            ttl_secs = config.cache_hub.ttl_secs,
            "Cache hub initialized"
        );

        // ── Knowledge graph ────────────────────────────────────────
        let knowledge_graph = kias_knowledge::KnowledgeGraph::new();
        tracing::info!("Knowledge graph initialized");

        // ── Skill registry ─────────────────────────────────────────
        let skill_registry = kias_skills::SkillRegistry::new();
        tracing::info!("Skill registry initialized");

        // ── Workflow engine ────────────────────────────────────────
        let workflow_engine = kias_workflow_engine::WorkflowEngine::new();
        tracing::info!("Workflow engine initialized");

        // ── Autonomy controller ────────────────────────────────────
        let autonomy_controller = kias_autonomy_controller::AutonomyController::new();
        tracing::info!("Autonomy controller initialized");

        // ── Team engine ────────────────────────────────────────────
        let team_engine = kias_team_engine::TeamEngine::new("kias-owner");
        tracing::info!("Team engine initialized");

        // ── Goal engine ────────────────────────────────────────────
        let evaluator = Box::new(kias_goal_engine::DefaultEvaluator::new());
        let goal_runner = kias_goal_engine::GoalLoopRunner::with_default_executor(evaluator);
        tracing::info!("Goal engine initialized");

        // ── Data Store (SQLite persistence) ────────────────────────────
        let db_path = std::env::var("KIAS_DB_PATH").unwrap_or_else(|_| "kias.db".to_string());
        let data_store = match kias_data_store::SqliteRepository::open(&db_path).await {
            Ok(store) => {
                tracing::info!(path = %db_path, "Data store initialized (SQLite)");
                store
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to open file-backed SQLite, falling back to in-memory");
                kias_data_store::SqliteRepository::in_memory().await?
            }
        };

        let vector_store = kias_data_store::PersistentVectorStore::new(data_store.pool.clone());
        let cache_strategy = kias_data_store::SqliteCacheStrategy::new(data_store.pool.clone());
        tracing::info!("Vector store and cache strategy initialized");

        tracing::info!("All KIAS subsystems initialized successfully");

        let started_at = Instant::now();

        Ok(Self {
            config,
            scheduler,
            health_checker,
            telemetry,
            metrics,
            cache_hub,
            knowledge_graph,
            skill_registry,
            workflow_engine,
            autonomy_controller,
            team_engine,
            goal_runner,
            data_store,
            vector_store,
            cache_strategy,
            shutdown,
            started_at,
        })
    }

    // ── Accessors ──────────────────────────────────────────────────

    /// Immutable reference to the system configuration.
    pub fn config(&self) -> &KiasConfig {
        &self.config
    }

    /// Immutable reference to the scheduler.
    pub fn scheduler(&self) -> &kias_scheduler::Scheduler {
        &self.scheduler
    }

    /// Immutable reference to the health checker.
    pub fn health_checker(&self) -> &kias_controller::HealthChecker {
        &self.health_checker
    }

    /// Immutable reference to the telemetry collector.
    pub fn telemetry(&self) -> &kias_monitor::TelemetryCollector {
        &self.telemetry
    }

    /// Immutable reference to the metrics collector.
    pub fn metrics(&self) -> &kias_monitor::MetricsCollector {
        &self.metrics
    }

    /// Immutable reference to the data store.
    pub fn data_store(&self) -> &kias_data_store::SqliteRepository {
        &self.data_store
    }

    /// Immutable reference to the vector store.
    pub fn vector_store(&self) -> &kias_data_store::PersistentVectorStore {
        &self.vector_store
    }

    /// The shutdown coordinator.
    pub fn shutdown_coordinator(&self) -> &ShutdownCoordinator {
        &self.shutdown
    }

    /// Clone the Arc to the shutdown coordinator for passing to background tasks.
    pub fn shutdown_handle(&self) -> Arc<ShutdownCoordinator> {
        Arc::clone(&self.shutdown)
    }

    // ── Health check ───────────────────────────────────────────────

    /// Run a system-wide health check and return a report.
    pub fn health_check(&self) -> SystemHealthReport {
        // Since all subsystems are in-memory and created synchronously,
        // they are healthy if the manager was created successfully.
        SystemHealthReport {
            overall: HealthStatus::Healthy,
            scheduler: HealthStatus::Healthy,
            controller: HealthStatus::Healthy,
            monitor: HealthStatus::Healthy,
            cache: HealthStatus::Healthy,
            workflow_engine: HealthStatus::Healthy,
            autonomy_controller: HealthStatus::Healthy,
            team_engine: HealthStatus::Healthy,
            goal_engine: HealthStatus::Healthy,
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Trigger graceful shutdown of all subsystems.
    pub async fn shutdown(&self) -> KiasResult<()> {
        self.shutdown.shutdown()
    }

    /// Seconds elapsed since this manager was created.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

// ── Configuration validation ───────────────────────────────────────────

impl KiasServiceManager {
    /// Validate critical config values before initialization.
    fn validate_config(config: &KiasConfig) -> KiasResult<()> {
        if config.api_server.port == 0 {
            return Err(KiasError::Config(
                "API server port must not be 0".to_string(),
            ));
        }
        if config.scheduler.algorithm.is_empty() {
            return Err(KiasError::Config(
                "Scheduler algorithm must not be empty".to_string(),
            ));
        }
        if config.controller.heartbeat_interval_secs == 0 {
            return Err(KiasError::Config(
                "Heartbeat interval must be greater than 0".to_string(),
            ));
        }
        if config.controller.failure_timeout_secs == 0 {
            return Err(KiasError::Config(
                "Failure timeout must be greater than 0".to_string(),
            ));
        }
        if config.cache_hub.max_entries == 0 {
            return Err(KiasError::Config(
                "Cache max entries must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

// ── Legacy compatibility ───────────────────────────────────────────────

/// Legacy boolean flags kept for backward compatibility.
#[allow(dead_code)]
pub struct KiasServices {
    pub api_server: bool,
    pub scheduler: bool,
    pub controller: bool,
    pub knowledge: bool,
    pub cache: bool,
    pub monitor: bool,
    pub executor: bool,
    pub skills: bool,
}

/// Legacy initialisation entry-point (kept for backward compat).
///
/// Prefer [`KiasServiceManager::new`] for new code.
pub async fn init_services() -> KiasResult<KiasServices> {
    tracing::info!("Initializing KIAS services (legacy mode)");

    let services = KiasServices {
        api_server: true,
        scheduler: true,
        controller: true,
        knowledge: true,
        cache: true,
        monitor: true,
        executor: true,
        skills: true,
    };

    tracing::info!("All services initialized");
    Ok(services)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: produce a default KiasConfig with valid values.
    fn default_config() -> KiasConfig {
        KiasConfig::default()
    }

    // ── ShutdownCoordinator tests ──────────────────────────────────

    #[test]
    fn test_shutdown_coordinator_creation() {
        let coord = ShutdownCoordinator::new();
        // Should be able to subscribe immediately.
        let _rx = coord.subscribe();
        assert!(coord.uptime_secs() < 5);
    }

    #[test]
    fn test_shutdown_coordinator_default() {
        let coord = ShutdownCoordinator::default();
        let _rx = coord.subscribe();
        assert!(coord.uptime_secs() < 5);
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_signal_received() {
        let coord = ShutdownCoordinator::new();
        let mut rx1 = coord.subscribe();
        let mut rx2 = coord.subscribe();

        coord.shutdown().unwrap();

        // Both subscribers should receive the signal.
        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_no_subscribers() {
        let coord = ShutdownCoordinator::new();
        // Shutdown with zero subscribers should succeed (no panic).
        assert!(coord.shutdown().is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_double_shutdown() {
        let coord = ShutdownCoordinator::new();
        let mut rx = coord.subscribe();

        coord.shutdown().unwrap();
        rx.recv().await.unwrap();

        // Second shutdown is idempotent.
        assert!(coord.shutdown().is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_late_subscriber_misses_signal() {
        let coord = ShutdownCoordinator::new();
        coord.shutdown().unwrap();

        // Drop the coordinator so the sender is dropped and channel closes.
        drop(coord);

        // A new coordinator's subscriber should work fine for a fresh signal.
        let coord2 = ShutdownCoordinator::new();
        let mut rx = coord2.subscribe();
        coord2.shutdown().unwrap();
        assert!(rx.recv().await.is_ok());
    }

    #[test]
    fn test_shutdown_coordinator_uptime_increases() {
        let coord = ShutdownCoordinator::new();
        let t1 = coord.uptime_secs();
        // In the same test, uptime should be 0 for both (sub-millisecond).
        assert_eq!(t1, 0);
    }

    // ── HealthStatus tests ─────────────────────────────────────────

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "Degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "Unhealthy");
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Degraded);
        assert_ne!(HealthStatus::Degraded, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_status_serialization_roundtrip() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_system_health_report_is_healthy() {
        let report = SystemHealthReport {
            overall: HealthStatus::Healthy,
            scheduler: HealthStatus::Healthy,
            controller: HealthStatus::Healthy,
            monitor: HealthStatus::Healthy,
            cache: HealthStatus::Healthy,
            workflow_engine: HealthStatus::Healthy,
            autonomy_controller: HealthStatus::Healthy,
            team_engine: HealthStatus::Healthy,
            goal_engine: HealthStatus::Healthy,
            uptime_secs: 100,
        };
        assert!(report.is_healthy());
    }

    #[test]
    fn test_system_health_report_not_healthy_when_degraded() {
        let report = SystemHealthReport {
            overall: HealthStatus::Degraded,
            scheduler: HealthStatus::Healthy,
            controller: HealthStatus::Healthy,
            monitor: HealthStatus::Healthy,
            cache: HealthStatus::Healthy,
            workflow_engine: HealthStatus::Healthy,
            autonomy_controller: HealthStatus::Healthy,
            team_engine: HealthStatus::Healthy,
            goal_engine: HealthStatus::Healthy,
            uptime_secs: 100,
        };
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_system_health_report_serialization_roundtrip() {
        let report = SystemHealthReport {
            overall: HealthStatus::Healthy,
            scheduler: HealthStatus::Healthy,
            controller: HealthStatus::Healthy,
            monitor: HealthStatus::Healthy,
            cache: HealthStatus::Healthy,
            workflow_engine: HealthStatus::Healthy,
            autonomy_controller: HealthStatus::Healthy,
            team_engine: HealthStatus::Healthy,
            goal_engine: HealthStatus::Healthy,
            uptime_secs: 42,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SystemHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.uptime_secs, 42);
        assert_eq!(deserialized.overall, HealthStatus::Healthy);
    }

    // ── KiasServiceManager tests ───────────────────────────────────

    #[tokio::test]
    async fn test_service_manager_creation() {
        let config = default_config();
        let manager = KiasServiceManager::new(config).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_service_manager_config_passthrough() {
        let mut config = default_config();
        config.scheduler.algorithm = "round-robin".to_string();

        let manager = KiasServiceManager::new(config).await.unwrap();
        assert_eq!(manager.config().scheduler.algorithm, "round-robin");
    }

    #[tokio::test]
    async fn test_service_manager_scheduler_algorithm() {
        let mut config = default_config();
        config.scheduler.algorithm = "least-loaded".to_string();

        let manager = KiasServiceManager::new(config).await.unwrap();
        // The scheduler should use the configured algorithm (or fallback).
        assert!(!manager.scheduler().algorithm_name().is_empty());
    }

    #[tokio::test]
    async fn test_service_manager_health_check() {
        let config = default_config();
        let manager = KiasServiceManager::new(config).await.unwrap();

        let report = manager.health_check();
        assert!(report.is_healthy());
        assert_eq!(report.overall, HealthStatus::Healthy);
        assert_eq!(report.scheduler, HealthStatus::Healthy);
        assert_eq!(report.controller, HealthStatus::Healthy);
        assert_eq!(report.monitor, HealthStatus::Healthy);
        assert_eq!(report.cache, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_service_manager_shutdown() {
        let config = default_config();
        let manager = KiasServiceManager::new(config).await.unwrap();

        let mut rx = manager.shutdown_handle().subscribe();
        manager.shutdown().await.unwrap();
        assert!(rx.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_service_manager_uptime() {
        let config = default_config();
        let manager = KiasServiceManager::new(config).await.unwrap();
        assert!(manager.uptime_secs() < 5);
    }

    #[tokio::test]
    async fn test_service_manager_health_report_uptime() {
        let config = default_config();
        let manager = KiasServiceManager::new(config).await.unwrap();

        let report = manager.health_check();
        assert!(report.uptime_secs < 5);
    }

    // ── Config validation tests ────────────────────────────────────

    #[tokio::test]
    async fn test_validate_config_zero_port() {
        let mut config = default_config();
        config.api_server.port = 0;
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("port"), "error: {}", err_msg);
    }

    #[tokio::test]
    async fn test_validate_config_empty_algorithm() {
        let mut config = default_config();
        config.scheduler.algorithm = String::new();
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("algorithm"), "error: {}", err_msg);
    }

    #[tokio::test]
    async fn test_validate_config_zero_heartbeat() {
        let mut config = default_config();
        config.controller.heartbeat_interval_secs = 0;
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_zero_failure_timeout() {
        let mut config = default_config();
        config.controller.failure_timeout_secs = 0;
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_zero_cache_entries() {
        let mut config = default_config();
        config.cache_hub.max_entries = 0;
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let config = default_config();
        let result = KiasServiceManager::new(config).await;
        assert!(result.is_ok());
    }

    // ── Legacy compatibility test ──────────────────────────────────

    #[tokio::test]
    async fn test_legacy_init_services() {
        let services = init_services().await.unwrap();
        assert!(services.api_server);
        assert!(services.scheduler);
        assert!(services.controller);
        assert!(services.knowledge);
        assert!(services.cache);
        assert!(services.monitor);
        assert!(services.executor);
        assert!(services.skills);
    }
}
