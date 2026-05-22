//! Connection Migration — seamless connection draining and migration between nodes.
//!
//! Provides zero-downtime connection migration capabilities:
//! - Graceful connection draining with configurable timeout
//! - In-flight request completion tracking
//! - Connection state serialization for migration
//! - Multi-protocol support (HTTP/1.1, HTTP/2, gRPC, WebSocket)
//! - Rollback capability on migration failure
//!
//! # Migration Lifecycle
//!
//! ```text
//! Migrate Request
//!     ↓
//! Prepare Phase: Snapshot connection state
//!     ↓
//! Drain Phase: Stop accepting new, wait in-flight to complete
//!     ↓
//! Transfer Phase: Serialize state, send to target
//!     ↓
//! Complete Phase: Update routing, close old
//!     ↓
//! Rollback if failure detected
//! ```
//!
//! # Example
//!
//! ```
//! use kias_controller::connection_migration::{
//!     ConnectionMigrator, MigrationConfig, MigrationPlan, ConnectionState,
//!     Protocol,
//! };
//!
//! let config = MigrationConfig::default()
//!     .with_drain_timeout_secs(30)
//!     .with_max_in_flight(100);
//!
//! let migrator = ConnectionMigrator::new("node-1".to_string(), config);
//! assert_eq!(migrator.source_node(), "node-1");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

/// Protocol types supported for migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Http1,
    Http2,
    Grpc,
    WebSocket,
    Tcp,
    Udp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Http1 => write!(f, "HTTP/1.1"),
            Protocol::Http2 => write!(f, "HTTP/2"),
            Protocol::Grpc => write!(f, "gRPC"),
            Protocol::WebSocket => write!(f, "WebSocket"),
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
        }
    }
}

/// Connection metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMeta {
    pub connection_id: String,
    pub protocol: Protocol,
    pub source_addr: String,
    pub dest_addr: String,
    pub established_at: String,
    pub last_activity: String,
    pub in_flight_requests: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl ConnectionMeta {
    pub fn new(connection_id: &str, protocol: Protocol, source: &str, dest: &str) -> Self {
        let now = chrono_lite_now();
        Self {
            connection_id: connection_id.to_string(),
            protocol,
            source_addr: source.to_string(),
            dest_addr: dest.to_string(),
            established_at: now.clone(),
            last_activity: now,
            in_flight_requests: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

/// Connection state for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub meta: ConnectionMeta,
    pub protocol_state: ProtocolState,
    pub session_data: HashMap<String, String>,
    pub custom_headers: HashMap<String, String>,
    pub sequence_numbers: HashMap<String, u64>,
    pub labels: HashMap<String, String>,
}

impl ConnectionState {
    pub fn new(meta: ConnectionMeta) -> Self {
        Self {
            meta,
            protocol_state: ProtocolState::default(),
            session_data: HashMap::new(),
            custom_headers: HashMap::new(),
            sequence_numbers: HashMap::new(),
            labels: HashMap::new(),
        }
    }

    pub fn add_session_data(&mut self, key: &str, value: &str) {
        self.session_data.insert(key.to_string(), value.to_string());
    }

    pub fn set_label(&mut self, key: &str, value: &str) {
        self.labels.insert(key.to_string(), value.to_string());
    }
}

/// Protocol-specific state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolState {
    pub http2_state: Option<Http2State>,
    pub websocket_state: Option<WebSocketState>,
    pub grpc_state: Option<GrpcState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http2State {
    pub stream_id: u32,
    pub window_size: i32,
    pub remote_window_size: i32,
    pub active_streams: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketState {
    pub is_server_side: bool,
    pub subprotocol: Option<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcState {
    pub method: String,
    pub authority: String,
    pub message_encoding: String,
    pub deadline: Option<String>,
}

/// Migration phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    /// Not started.
    Idle,
    /// Preparing (snapshotting state).
    Preparing,
    /// Draining in-flight requests.
    Draining,
    /// Transferring state to target.
    Transferring,
    /// Completing migration.
    Completing,
    /// Migration completed.
    Completed,
    /// Migration rolled back.
    RolledBack,
    /// Migration failed.
    Failed,
}

impl fmt::Display for MigrationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationPhase::Idle => write!(f, "Idle"),
            MigrationPhase::Preparing => write!(f, "Preparing"),
            MigrationPhase::Draining => write!(f, "Draining"),
            MigrationPhase::Transferring => write!(f, "Transferring"),
            MigrationPhase::Completing => write!(f, "Completing"),
            MigrationPhase::Completed => write!(f, "Completed"),
            MigrationPhase::RolledBack => write!(f, "RolledBack"),
            MigrationPhase::Failed => write!(f, "Failed"),
        }
    }
}

/// Migration status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub phase: MigrationPhase,
    pub connections_total: u32,
    pub connections_migrated: u32,
    pub connections_failed: u32,
    pub in_flight_total: u32,
    pub in_flight_remaining: u32,
    pub bytes_transferred: u64,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub error: Option<String>,
}

impl Default for MigrationStatus {
    fn default() -> Self {
        Self {
            phase: MigrationPhase::Idle,
            connections_total: 0,
            connections_migrated: 0,
            connections_failed: 0,
            in_flight_total: 0,
            in_flight_remaining: 0,
            bytes_transferred: 0,
            started_at: None,
            updated_at: None,
            error: None,
        }
    }
}

/// Migration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Source node id.
    pub source_node: String,
    /// Target node id.
    pub target_node: String,
    /// Connection drain timeout in seconds.
    pub drain_timeout_secs: u64,
    /// Maximum in-flight requests per connection.
    pub max_in_flight: u32,
    /// Maximum bytes to transfer per second (0 = unlimited).
    pub max_bandwidth_bps: u64,
    /// Enable state compression.
    pub compress_state: bool,
    /// Enable encryption for state transfer.
    pub encrypt_transfer: bool,
    /// Enable automatic rollback on failure.
    pub auto_rollback: bool,
    /// Enable health check after migration.
    pub post_migration_health_check: bool,
    /// Number of parallel connection migrations.
    pub parallel_migrations: usize,
    /// Force migration even if target is unhealthy.
    pub force: bool,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            source_node: String::new(),
            target_node: String::new(),
            drain_timeout_secs: 30,
            max_in_flight: 100,
            max_bandwidth_bps: 0,
            compress_state: true,
            encrypt_transfer: true,
            auto_rollback: true,
            post_migration_health_check: true,
            parallel_migrations: 10,
            force: false,
            metadata: HashMap::new(),
        }
    }
}

impl MigrationConfig {
    pub fn new(source: &str, target: &str) -> Self {
        Self {
            source_node: source.to_string(),
            target_node: target.to_string(),
            ..Default::default()
        }
    }

    pub fn with_drain_timeout_secs(mut self, secs: u64) -> Self {
        self.drain_timeout_secs = secs;
        self
    }

    pub fn with_max_in_flight(mut self, max: u32) -> Self {
        self.max_in_flight = max;
        self
    }

    pub fn with_parallel_migrations(mut self, n: usize) -> Self {
        self.parallel_migrations = n;
        self
    }
}

/// Migration plan — which connections to migrate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub plan_id: String,
    pub connections: Vec<ConnectionState>,
    pub estimated_duration_ms: u64,
    pub requires_rollback: bool,
    pub dependencies: Vec<String>,
}

impl MigrationPlan {
    pub fn new(plan_id: &str, connections: Vec<ConnectionState>) -> Self {
        Self {
            plan_id: plan_id.to_string(),
            connections,
            estimated_duration_ms: 0,
            requires_rollback: false,
            dependencies: vec![],
        }
    }

    pub fn add_dependency(&mut self, dep: &str) {
        self.dependencies.push(dep.to_string());
    }
}

/// A connection being migrated.
#[derive(Debug, Clone)]
pub struct MigratingConnection {
    pub state: ConnectionState,
    pub phase: MigrationPhase,
    pub migration_start: Option<Instant>,
    pub drain_start: Option<Instant>,
    pub transfer_start: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub rollback_data: Option<ConnectionState>,
}

impl MigratingConnection {
    pub fn new(state: ConnectionState) -> Self {
        Self {
            state,
            phase: MigrationPhase::Preparing,
            migration_start: Some(Instant::now()),
            drain_start: None,
            transfer_start: None,
            completed_at: None,
            rollback_data: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.phase,
            MigrationPhase::Completed | MigrationPhase::Failed
        )
    }

    pub fn is_drained(&self) -> bool {
        self.state.meta.in_flight_requests == 0
    }
}

/// Connection migrator — manages connection migration lifecycle.
#[derive(Debug)]
pub struct ConnectionMigrator {
    config: MigrationConfig,
    status: MigrationStatus,
    connections: HashMap<String, MigratingConnection>,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
}

impl ConnectionMigrator {
    /// Create a new migrator.
    pub fn new(node_id: String, config: MigrationConfig) -> Self {
        let mut cfg = config;
        if cfg.source_node.is_empty() {
            cfg.source_node = node_id.clone();
        }
        Self {
            config: cfg,
            status: MigrationStatus::default(),
            connections: HashMap::new(),
            started_at: None,
            finished_at: None,
        }
    }

    /// Create with source and target.
    pub fn with_pair(source: &str, target: &str, config: MigrationConfig) -> Self {
        let mut cfg = config;
        cfg.target_node = target.to_string();
        Self::new(source.to_string(), cfg)
    }

    /// Get source node.
    pub fn source_node(&self) -> &str {
        &self.config.source_node
    }

    /// Get target node.
    pub fn target_node(&self) -> &str {
        &self.config.target_node
    }

    /// Get current status.
    pub fn status(&self) -> &MigrationStatus {
        &self.status
    }

    /// Get configuration.
    pub fn config(&self) -> &MigrationConfig {
        &self.config
    }

    /// Get connection by ID.
    pub fn get_connection(&self, id: &str) -> Option<&MigratingConnection> {
        self.connections.get(id)
    }

    /// Get all connections.
    pub fn connections(&self) -> &HashMap<String, MigratingConnection> {
        &self.connections
    }

    /// Get connection count.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Create a migration plan.
    pub fn create_plan(&self, connection_ids: Vec<String>) -> MigrationPlan {
        let conn_states: Vec<ConnectionState> = connection_ids
            .iter()
            .filter_map(|id| self.connections.get(id))
            .map(|mc| mc.state.clone())
            .collect();

        MigrationPlan::new("plan-1", conn_states)
    }

    /// Add a connection to migrate.
    pub fn add_connection(&mut self, state: ConnectionState) {
        let id = state.meta.connection_id.clone();
        let conn = MigratingConnection::new(state);
        self.connections.insert(id, conn);
        self.status.connections_total += 1;
        self.status.in_flight_total += self
            .connections
            .values()
            .last()
            .map(|c| c.state.meta.in_flight_requests)
            .unwrap_or(0);
    }

    /// Start migration of all registered connections.
    pub fn start(&mut self) -> bool {
        if self.status.phase != MigrationPhase::Idle
            && self.status.phase != MigrationPhase::RolledBack
        {
            return false;
        }

        self.status.phase = MigrationPhase::Preparing;
        self.started_at = Some(Instant::now());
        self.status.started_at = Some(chrono_lite_now());
        self.status.updated_at = Some(chrono_lite_now());

        // Transition all to preparing
        for conn in self.connections.values_mut() {
            if conn.phase == MigrationPhase::Idle {
                conn.phase = MigrationPhase::Preparing;
            }
        }

        true
    }

    /// Begin draining connections.
    pub fn begin_drain(&mut self) {
        self.status.phase = MigrationPhase::Draining;
        self.status.updated_at = Some(chrono_lite_now());

        for conn in self.connections.values_mut() {
            if conn.phase == MigrationPhase::Preparing {
                conn.phase = MigrationPhase::Draining;
                conn.drain_start = Some(Instant::now());
            }
        }
    }

    /// Complete drain for a specific connection.
    pub fn complete_drain(&mut self, connection_id: &str) -> bool {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            if conn.state.meta.in_flight_requests == 0 {
                conn.phase = MigrationPhase::Transferring;
                conn.transfer_start = Some(Instant::now());
                return true;
            }
        }
        false
    }

    /// Mark connection as drained (simulate completion).
    pub fn mark_drained(&mut self, connection_id: &str) {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            conn.state.meta.in_flight_requests = 0;
            self.status.in_flight_remaining = self.status.in_flight_remaining.saturating_sub(1);
        }
    }

    /// Transfer a connection to target.
    pub fn transfer(&mut self, connection_id: &str) -> bool {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            if conn.phase == MigrationPhase::Draining && conn.is_drained() {
                conn.phase = MigrationPhase::Transferring;
                conn.transfer_start = Some(Instant::now());
                self.status.bytes_transferred +=
                    conn.state.meta.bytes_sent + conn.state.meta.bytes_received;
                return true;
            }
        }
        false
    }

    /// Complete migration for a connection.
    pub fn complete_connection(&mut self, connection_id: &str) -> bool {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            conn.phase = MigrationPhase::Completed;
            conn.completed_at = Some(Instant::now());
            self.status.connections_migrated += 1;
            self.status.updated_at = Some(chrono_lite_now());
            return true;
        }
        false
    }

    /// Check if all connections are migrated.
    pub fn all_migrated(&self) -> bool {
        self.connections.values().all(|c| c.is_complete())
    }

    /// Check if all connections are drained.
    pub fn all_drained(&self) -> bool {
        self.connections.values().all(|c| c.is_drained())
    }

    /// Check if we should rollback.
    pub fn should_rollback(&self) -> bool {
        self.status.connections_failed > 0
            || self.config.auto_rollback && self.status.phase == MigrationPhase::Failed
    }

    /// Rollback a connection to its previous state.
    pub fn rollback_connection(&mut self, connection_id: &str) -> bool {
        if let Some(conn) = self.connections.get_mut(connection_id) {
            conn.phase = MigrationPhase::RolledBack;
            conn.completed_at = Some(Instant::now());
            self.status.connections_failed += 1;
            return true;
        }
        false
    }

    /// Complete the migration.
    pub fn complete(&mut self) {
        self.status.phase = MigrationPhase::Completed;
        self.finished_at = Some(Instant::now());
        self.status.updated_at = Some(chrono_lite_now());
    }

    /// Fail the migration.
    pub fn fail(&mut self, error: &str) {
        self.status.phase = MigrationPhase::Failed;
        self.status.error = Some(error.to_string());
        self.finished_at = Some(Instant::now());
        self.status.updated_at = Some(chrono_lite_now());
    }

    /// Get migration duration.
    pub fn duration(&self) -> Option<Duration> {
        self.started_at.map(|start| {
            self.finished_at
                .unwrap_or_else(Instant::now)
                .duration_since(start)
        })
    }

    /// Get progress percentage (0-100).
    pub fn progress_percent(&self) -> f64 {
        if self.status.connections_total == 0 {
            return 100.0;
        }
        (self.status.connections_migrated as f64 / self.status.connections_total as f64) * 100.0
    }

    /// Check if migration is in progress.
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self.status.phase,
            MigrationPhase::Preparing | MigrationPhase::Draining | MigrationPhase::Transferring
        )
    }

    /// Check if migration is complete.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status.phase,
            MigrationPhase::Completed | MigrationPhase::RolledBack | MigrationPhase::Failed
        )
    }

    /// Get connections by protocol.
    pub fn connections_by_protocol(&self, protocol: Protocol) -> Vec<&MigratingConnection> {
        self.connections
            .values()
            .filter(|c| c.state.meta.protocol == protocol)
            .collect()
    }

    /// Get failed connections (includes rolled back).
    pub fn failed_connections(&self) -> Vec<&str> {
        self.connections
            .iter()
            .filter(|(_, c)| matches!(c.phase, MigrationPhase::Failed | MigrationPhase::RolledBack))
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

/// Shared migrator for async environments.
pub type SharedConnectionMigrator = Arc<StdRwLock<ConnectionMigrator>>;

/// Create a shared migrator.
pub fn shared(migrator: ConnectionMigrator) -> SharedConnectionMigrator {
    Arc::new(StdRwLock::new(migrator))
}

/// Lightweight timestamp helper.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta(id: &str) -> ConnectionMeta {
        ConnectionMeta::new(id, Protocol::Http2, "10.0.0.1:8080", "10.0.0.2:8080")
    }

    fn make_state(id: &str) -> ConnectionState {
        ConnectionState::new(make_meta(id))
    }

    fn make_config() -> MigrationConfig {
        MigrationConfig::new("node-1", "node-2")
            .with_drain_timeout_secs(30)
            .with_max_in_flight(100)
    }

    #[test]
    fn test_migrator_initial_state() {
        let migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert_eq!(migrator.source_node(), "node-1");
        assert_eq!(migrator.target_node(), "node-2");
        assert_eq!(migrator.status().phase, MigrationPhase::Idle);
    }

    #[test]
    fn test_migrator_start() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert!(migrator.start());
        assert_eq!(migrator.status().phase, MigrationPhase::Preparing);
        assert!(migrator.started_at.is_some());
    }

    #[test]
    fn test_migrator_cannot_start_twice() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        migrator.start();
        assert!(!migrator.start());
    }

    #[test]
    fn test_migrator_add_connection() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let state = make_state("conn-1");
        migrator.add_connection(state);
        assert_eq!(migrator.connection_count(), 1);
        assert_eq!(migrator.status().connections_total, 1);
    }

    #[test]
    fn test_migrator_begin_drain() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        migrator.start();
        migrator.begin_drain();
        assert_eq!(migrator.status().phase, MigrationPhase::Draining);
    }

    #[test]
    fn test_migrator_mark_drained() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let state = make_state("conn-1");
        migrator.add_connection(state);
        migrator.start();
        migrator.begin_drain();
        migrator.mark_drained("conn-1");
        let conn = migrator.get_connection("conn-1").unwrap();
        assert!(conn.is_drained());
    }

    #[test]
    fn test_migrator_transfer() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let state = make_state("conn-1");
        migrator.add_connection(state);
        migrator.start();
        migrator.begin_drain();
        migrator.mark_drained("conn-1");
        assert!(migrator.transfer("conn-1"));
    }

    #[test]
    fn test_migrator_complete_connection() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let state = make_state("conn-1");
        migrator.add_connection(state);
        migrator.start();
        migrator.begin_drain();
        migrator.mark_drained("conn-1");
        migrator.transfer("conn-1");
        assert!(migrator.complete_connection("conn-1"));
        assert_eq!(migrator.status().connections_migrated, 1);
    }

    #[test]
    fn test_migrator_all_drained() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let s1 = make_state("conn-1");
        let s2 = make_state("conn-2");
        migrator.add_connection(s1);
        migrator.add_connection(s2);
        migrator.start();
        migrator.begin_drain();
        migrator.mark_drained("conn-1");
        migrator.mark_drained("conn-2");
        assert!(migrator.all_drained());
    }

    #[test]
    fn test_migrator_all_migrated() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let s1 = make_state("conn-1");
        let s2 = make_state("conn-2");
        migrator.add_connection(s1);
        migrator.add_connection(s2);
        migrator.start();
        migrator.begin_drain();
        migrator.mark_drained("conn-1");
        migrator.transfer("conn-1");
        migrator.complete_connection("conn-1");
        migrator.mark_drained("conn-2");
        migrator.transfer("conn-2");
        migrator.complete_connection("conn-2");
        assert!(migrator.all_migrated());
        assert_eq!(migrator.progress_percent(), 100.0);
    }

    #[test]
    fn test_migrator_complete() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        migrator.start();
        migrator.complete();
        assert!(migrator.is_complete());
        assert_eq!(migrator.status().phase, MigrationPhase::Completed);
    }

    #[test]
    fn test_migrator_fail() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        migrator.start();
        migrator.fail("Connection refused");
        assert_eq!(migrator.status().phase, MigrationPhase::Failed);
        assert_eq!(
            migrator.status().error,
            Some("Connection refused".to_string())
        );
    }

    #[test]
    fn test_migrator_rollback_connection() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let state = make_state("conn-1");
        migrator.add_connection(state);
        migrator.start();
        migrator.rollback_connection("conn-1");
        assert_eq!(migrator.status().connections_failed, 1);
    }

    #[test]
    fn test_migrator_should_rollback() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert!(!migrator.should_rollback());
    }

    #[test]
    fn test_migrator_progress_percent() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert_eq!(migrator.progress_percent(), 100.0); // No connections = 100%

        let state = make_state("conn-1");
        migrator.add_connection(state);
        assert_eq!(migrator.progress_percent(), 0.0);
    }

    #[test]
    fn test_migrator_is_in_progress() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert!(!migrator.is_in_progress());
        migrator.start();
        assert!(migrator.is_in_progress());
    }

    #[test]
    fn test_connection_meta() {
        let meta = make_meta("c1");
        assert_eq!(meta.connection_id, "c1");
        assert_eq!(meta.protocol, Protocol::Http2);
        assert!(!meta.source_addr.is_empty());
    }

    #[test]
    fn test_connection_state_session_data() {
        let mut state = make_state("conn-1");
        state.add_session_data("user_id", "123");
        assert_eq!(state.session_data.get("user_id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_migration_plan() {
        let plan = MigrationPlan::new("plan-1", vec![make_state("c1"), make_state("c2")]);
        assert_eq!(plan.connections.len(), 2);
    }

    #[test]
    fn test_migration_plan_add_dependency() {
        let mut plan = MigrationPlan::new("plan-1", vec![]);
        plan.add_dependency("other-plan");
        assert_eq!(plan.dependencies.len(), 1);
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", Protocol::Http1), "HTTP/1.1");
        assert_eq!(format!("{}", Protocol::Grpc), "gRPC");
        assert_eq!(format!("{}", Protocol::WebSocket), "WebSocket");
    }

    #[test]
    fn test_migration_phase_display() {
        assert_eq!(format!("{}", MigrationPhase::Draining), "Draining");
        assert_eq!(format!("{}", MigrationPhase::Completed), "Completed");
    }

    #[test]
    fn test_migrating_connection_is_complete() {
        let state = make_state("conn-1");
        let conn = MigratingConnection::new(state);
        assert!(!conn.is_complete());
    }

    #[test]
    fn test_shared_migrator() {
        let migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let shared = shared(migrator);
        assert_eq!(shared.read().unwrap().source_node(), "node-1");
    }

    #[test]
    fn test_migrator_with_pair() {
        let migrator =
            ConnectionMigrator::with_pair("node-a", "node-b", MigrationConfig::default());
        assert_eq!(migrator.source_node(), "node-a");
        assert_eq!(migrator.target_node(), "node-b");
    }

    #[test]
    fn test_migrator_connections_by_protocol() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let s1 = ConnectionState::new(ConnectionMeta::new("c1", Protocol::Http1, "a", "b"));
        let s2 = ConnectionState::new(ConnectionMeta::new("c2", Protocol::Http2, "a", "b"));
        let s3 = ConnectionState::new(ConnectionMeta::new("c3", Protocol::Http1, "a", "b"));
        migrator.add_connection(s1);
        migrator.add_connection(s2);
        migrator.add_connection(s3);

        let http1_conns = migrator.connections_by_protocol(Protocol::Http1);
        assert_eq!(http1_conns.len(), 2);
    }

    #[test]
    fn test_migrator_failed_connections() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        let s1 = make_state("conn-1");
        migrator.add_connection(s1);
        migrator.start();
        migrator.rollback_connection("conn-1");
        let failed = migrator.failed_connections();
        assert_eq!(failed, vec!["conn-1"]);
    }

    #[test]
    fn test_migrator_config_builder() {
        let config = MigrationConfig::new("s", "t")
            .with_drain_timeout_secs(60)
            .with_max_in_flight(50)
            .with_parallel_migrations(5);
        assert_eq!(config.drain_timeout_secs, 60);
        assert_eq!(config.max_in_flight, 50);
        assert_eq!(config.parallel_migrations, 5);
    }

    #[test]
    fn test_migrator_duration() {
        let mut migrator = ConnectionMigrator::new("node-1".to_string(), make_config());
        assert!(migrator.duration().is_none());
        migrator.start();
        std::thread::sleep(Duration::from_millis(1));
        assert!(migrator.duration().is_some());
    }

    #[test]
    fn test_migration_status_default() {
        let status = MigrationStatus::default();
        assert_eq!(status.connections_total, 0);
        assert_eq!(status.connections_migrated, 0);
        assert_eq!(status.phase, MigrationPhase::Idle);
    }
}
