//! # Multi-Datasource Access
//!
//! Abstract data source trait and registry for managing multiple database backends.
//!
//! The [`DataSource`] trait provides a uniform interface over different storage engines
//! (SQLite, PostgreSQL, etc.), while [`DataSourceRegistry`] manages named instances
//! that can be looked up at runtime.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use kias_common::{KiasError, KiasResult};

/// Supported data source backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceType {
    /// Embedded SQLite (default, zero-config).
    Sqlite,
    /// PostgreSQL (production-grade, multi-tenant).
    Postgres,
    /// MySQL / MariaDB.
    Mysql,
    /// In-memory (testing only).
    Memory,
}

impl std::fmt::Display for DataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite => write!(f, "sqlite"),
            Self::Postgres => write!(f, "postgres"),
            Self::Mysql => write!(f, "mysql"),
            Self::Memory => write!(f, "memory"),
        }
    }
}

impl std::str::FromStr for DataSourceType {
    type Err = KiasError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sqlite" => Ok(Self::Sqlite),
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "mysql" | "mariadb" => Ok(Self::Mysql),
            "memory" => Ok(Self::Memory),
            _ => Err(KiasError::Validation(format!(
                "Unknown data source type: {s}"
            ))),
        }
    }
}

/// Connection status for a data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceStatus {
    /// Configured but not yet connected.
    Pending,
    /// Connected and healthy.
    Connected,
    /// Connection lost or health check failed.
    Disconnected,
    /// Permanently disabled.
    Disabled,
}

impl std::fmt::Display for DataSourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Connected => write!(f, "connected"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

/// Configuration for a data source connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// Unique name for this data source.
    pub name: String,
    /// Backend type.
    pub ds_type: DataSourceType,
    /// Connection string (DSN) or file path for SQLite.
    pub connection_string: String,
    /// Maximum number of connections in the pool (default: 10).
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Whether this is the default data source.
    #[serde(default)]
    pub is_default: bool,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

fn default_max_connections() -> u32 {
    10
}

/// Health check result for a data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub latency_ms: u64,
    pub message: String,
    pub checked_at: String,
}

/// Abstract data source trait.
///
/// Implementations wrap a specific database backend and expose a uniform
/// interface for health checks, raw queries, and connection management.
#[async_trait]
pub trait DataSource: Send + Sync + std::fmt::Debug {
    /// Return the name of this data source.
    fn name(&self) -> &str;

    /// Return the backend type.
    fn ds_type(&self) -> DataSourceType;

    /// Return the current status.
    fn status(&self) -> DataSourceStatus;

    /// Perform a health check (ping the database).
    async fn health_check(&self) -> HealthCheckResult;

    /// Execute a raw SQL query and return the number of affected rows.
    async fn execute_raw(&self, sql: &str) -> KiasResult<u64>;

    /// Return connection pool statistics (active, idle, total).
    fn pool_stats(&self) -> (u32, u32, u32);
}

/// SQLite data source backed by sqlx.
#[derive(Debug, Clone)]
pub struct SqliteDataSource {
    name: String,
    pool: sqlx::SqlitePool,
    status: DataSourceStatus,
}

impl SqliteDataSource {
    /// Create a new SQLite data source from a connection string.
    pub async fn new(name: impl Into<String>, connection_string: &str) -> KiasResult<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(connection_string)
            .await
            .map_err(|e| KiasError::Config(format!("SQLite connect failed: {e}")))?;

        Ok(Self {
            name: name.into(),
            pool,
            status: DataSourceStatus::Connected,
        })
    }

    /// Create from an existing pool (useful for sharing with data-store).
    pub fn from_pool(name: impl Into<String>, pool: sqlx::SqlitePool) -> Self {
        Self {
            name: name.into(),
            pool,
            status: DataSourceStatus::Connected,
        }
    }

    /// Access the underlying pool.
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl DataSource for SqliteDataSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn ds_type(&self) -> DataSourceType {
        DataSourceType::Sqlite
    }

    fn status(&self) -> DataSourceStatus {
        self.status
    }

    async fn health_check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let result = sqlx::query("SELECT 1").execute(&self.pool).await;
        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) => HealthCheckResult {
                healthy: true,
                latency_ms: latency,
                message: "OK".to_string(),
                checked_at: chrono::Utc::now().to_rfc3339(),
            },
            Err(e) => HealthCheckResult {
                healthy: false,
                latency_ms: latency,
                message: format!("Health check failed: {e}"),
                checked_at: chrono::Utc::now().to_rfc3339(),
            },
        }
    }

    async fn execute_raw(&self, sql: &str) -> KiasResult<u64> {
        let result = sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Raw query failed: {e}")))?;
        Ok(result.rows_affected())
    }

    fn pool_stats(&self) -> (u32, u32, u32) {
        let size = self.pool.size();
        let idle = self.pool.num_idle() as u32;
        (size.saturating_sub(idle), idle, size)
    }
}

/// In-memory data source for testing.
#[derive(Debug)]
pub struct MemoryDataSource {
    name: String,
    status: DataSourceStatus,
}

impl MemoryDataSource {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DataSourceStatus::Connected,
        }
    }
}

#[async_trait]
impl DataSource for MemoryDataSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn ds_type(&self) -> DataSourceType {
        DataSourceType::Memory
    }

    fn status(&self) -> DataSourceStatus {
        self.status
    }

    async fn health_check(&self) -> HealthCheckResult {
        HealthCheckResult {
            healthy: true,
            latency_ms: 0,
            message: "In-memory data source".to_string(),
            checked_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    async fn execute_raw(&self, _sql: &str) -> KiasResult<u64> {
        Ok(0)
    }

    fn pool_stats(&self) -> (u32, u32, u32) {
        (0, 0, 0)
    }
}

/// Registry for managing named data sources.
///
/// Thread-safe, allows runtime registration and lookup of data sources.
/// One data source can be marked as the default.
#[derive(Debug)]
pub struct DataSourceRegistry {
    sources: RwLock<HashMap<String, Arc<dyn DataSource>>>,
    default_name: RwLock<Option<String>>,
}

impl DataSourceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
            default_name: RwLock::new(None),
        }
    }

    /// Register a data source. If `is_default` is true, it becomes the default.
    pub async fn register(&self, source: Arc<dyn DataSource>, is_default: bool) {
        let name = source.name().to_string();
        info!(name = %name, ds_type = %source.ds_type(), "Registering data source");
        if is_default {
            *self.default_name.write().await = Some(name.clone());
        }
        self.sources.write().await.insert(name, source);
    }

    /// Get a data source by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn DataSource>> {
        self.sources.read().await.get(name).cloned()
    }

    /// Get the default data source.
    pub async fn get_default(&self) -> Option<Arc<dyn DataSource>> {
        let name = self.default_name.read().await.clone()?;
        self.sources.read().await.get(&name).cloned()
    }

    /// List all registered data source names.
    pub async fn list_names(&self) -> Vec<String> {
        self.sources.read().await.keys().cloned().collect()
    }

    /// Remove a data source by name.
    pub async fn remove(&self, name: &str) -> bool {
        let removed = self.sources.write().await.remove(name).is_some();
        if removed {
            let mut default = self.default_name.write().await;
            if default.as_deref() == Some(name) {
                *default = None;
            }
        }
        removed
    }

    /// Run health checks on all registered data sources.
    pub async fn health_check_all(&self) -> HashMap<String, HealthCheckResult> {
        let sources = self.sources.read().await;
        let mut results = HashMap::new();
        for (name, source) in sources.iter() {
            let result = source.health_check().await;
            debug!(name = %name, healthy = result.healthy, latency_ms = result.latency_ms, "Data source health check");
            results.insert(name.clone(), result);
        }
        results
    }
}

impl Default for DataSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datasource_type_display_parse() {
        for ds_type in [
            DataSourceType::Sqlite,
            DataSourceType::Postgres,
            DataSourceType::Mysql,
            DataSourceType::Memory,
        ] {
            let s = ds_type.to_string();
            let parsed: DataSourceType = s.parse().unwrap();
            assert_eq!(ds_type, parsed);
        }
    }

    #[test]
    fn test_datasource_type_parse_invalid() {
        let result: Result<DataSourceType, _> = "oracle".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_datasource_status_display() {
        assert_eq!(DataSourceStatus::Pending.to_string(), "pending");
        assert_eq!(DataSourceStatus::Connected.to_string(), "connected");
        assert_eq!(DataSourceStatus::Disconnected.to_string(), "disconnected");
        assert_eq!(DataSourceStatus::Disabled.to_string(), "disabled");
    }

    #[tokio::test]
    async fn test_memory_datasource() {
        let ds = MemoryDataSource::new("test-mem");
        assert_eq!(ds.name(), "test-mem");
        assert_eq!(ds.ds_type(), DataSourceType::Memory);
        assert_eq!(ds.status(), DataSourceStatus::Connected);

        let health = ds.health_check().await;
        assert!(health.healthy);

        let affected = ds.execute_raw("anything").await.unwrap();
        assert_eq!(affected, 0);
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = DataSourceRegistry::new();
        let ds: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("mem1"));

        registry.register(ds, false).await;
        assert!(registry.get("mem1").await.is_some());
        assert!(registry.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_registry_default() {
        let registry = DataSourceRegistry::new();
        let ds: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("default-ds"));

        registry.register(ds, true).await;
        let default = registry.get_default().await.unwrap();
        assert_eq!(default.name(), "default-ds");
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let registry = DataSourceRegistry::new();
        let ds: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("rm-test"));

        registry.register(ds, true).await;
        assert!(registry.remove("rm-test").await);
        assert!(registry.get("rm-test").await.is_none());
        assert!(registry.get_default().await.is_none());
    }

    #[tokio::test]
    async fn test_registry_list_names() {
        let registry = DataSourceRegistry::new();
        let ds1: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("a"));
        let ds2: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("b"));

        registry.register(ds1, false).await;
        registry.register(ds2, false).await;

        let mut names = registry.list_names().await;
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_registry_health_check_all() {
        let registry = DataSourceRegistry::new();
        let ds: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("hc"));
        registry.register(ds, false).await;

        let results = registry.health_check_all().await;
        assert_eq!(results.len(), 1);
        assert!(results["hc"].healthy);
    }

    #[test]
    fn test_datasource_config_serde() {
        let config = DataSourceConfig {
            name: "main".to_string(),
            ds_type: DataSourceType::Sqlite,
            connection_string: "sqlite:///data/kias.db".to_string(),
            max_connections: 20,
            is_default: true,
            description: "Primary database".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DataSourceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "main");
        assert_eq!(deserialized.ds_type, DataSourceType::Sqlite);
        assert_eq!(deserialized.max_connections, 20);
        assert!(deserialized.is_default);
    }

    #[tokio::test]
    async fn test_sqlite_datasource() {
        let ds = SqliteDataSource::new("test-sqlite", "sqlite::memory:")
            .await
            .unwrap();
        assert_eq!(ds.name(), "test-sqlite");
        assert_eq!(ds.ds_type(), DataSourceType::Sqlite);
        assert_eq!(ds.status(), DataSourceStatus::Connected);

        let health = ds.health_check().await;
        assert!(health.healthy);
        assert_eq!(health.message, "OK");

        let (active, idle, total) = ds.pool_stats();
        assert!(total > 0 || (active == 0 && idle == 0)); // Valid pool state
    }

    #[test]
    fn test_datasource_type_parse_case_insensitive() {
        let t: DataSourceType = "SQLITE".parse().unwrap();
        assert_eq!(t, DataSourceType::Sqlite);
        let t: DataSourceType = "Postgres".parse().unwrap();
        assert_eq!(t, DataSourceType::Postgres);
    }

    #[test]
    fn test_datasource_type_parse_postgresql_alias() {
        let t: DataSourceType = "postgresql".parse().unwrap();
        assert_eq!(t, DataSourceType::Postgres);
    }

    #[test]
    fn test_datasource_type_parse_mariadb_alias() {
        let t: DataSourceType = "mariadb".parse().unwrap();
        assert_eq!(t, DataSourceType::Mysql);
    }

    #[tokio::test]
    async fn test_memory_datasource_pool_stats() {
        let ds = MemoryDataSource::new("pool-test");
        let (active, idle, total) = ds.pool_stats();
        assert_eq!((active, idle, total), (0, 0, 0));
    }

    #[tokio::test]
    async fn test_registry_empty_default() {
        let registry = DataSourceRegistry::new();
        assert!(registry.get_default().await.is_none());
        assert!(registry.list_names().await.is_empty());
    }

    #[tokio::test]
    async fn test_registry_remove_nonexistent() {
        let registry = DataSourceRegistry::new();
        assert!(!registry.remove("nonexistent").await);
    }

    #[tokio::test]
    async fn test_registry_overwrite_default() {
        let registry = DataSourceRegistry::new();
        let ds1: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("first"));
        let ds2: Arc<dyn DataSource> = Arc::new(MemoryDataSource::new("second"));

        registry.register(ds1, true).await;
        assert_eq!(registry.get_default().await.unwrap().name(), "first");

        registry.register(ds2, true).await;
        assert_eq!(registry.get_default().await.unwrap().name(), "second");
    }

    #[test]
    fn test_datasource_config_default_max_connections() {
        let json = r#"{"name":"test","ds_type":"sqlite","connection_string":"test"}"#;
        let config: DataSourceConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_connections, 10);
        assert!(!config.is_default);
        assert!(config.description.is_empty());
    }

    #[tokio::test]
    async fn test_sqlite_datasource_execute_raw() {
        let ds = SqliteDataSource::new("exec-test", "sqlite::memory:")
            .await
            .unwrap();
        // Create a table
        let affected = ds
            .execute_raw("CREATE TABLE IF NOT EXISTS test (id INTEGER)")
            .await
            .unwrap();
        assert_eq!(affected, 0); // CREATE TABLE returns 0 rows affected
    }
}
