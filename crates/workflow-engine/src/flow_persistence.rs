//! Flow Persistence & State Recovery
//!
//! Provides declarative persistence for workflow state via `@persist` decorator pattern.
//!
//! # Example
//! ```rust,ignore
//! use flow_persistence::{persist, PersistenceKey, SqlitePersistence};
//!
//! #[persist(key = "user:123:checkout")]
//! async fn checkout_flow(state: &mut CheckoutState) {
//!     // workflow logic
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

use crate::error_handler::ErrorHandler;

// =============================================================================
// Errors
// =============================================================================

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("flow not found: {0}")]
    FlowNotFound(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("database error: {0}")]
    DatabaseError(String),

    #[error("invalid key format: {0}")]
    InvalidKey(String),

    #[error("key conflict: {0}")]
    KeyConflict(String),
}

pub type PersistenceResult<T> = Result<T, PersistenceError>;

// =============================================================================
// PersistenceKey
// =============================================================================

/// Key for flow state persistence.
/// Supports hierarchical naming for sharing/isolation between flows.
///
/// # Key Formats
/// - `flow:{flow_id}` - Single flow, isolated state
/// - `user:{user_id}:{flow_id}` - User-scoped flow
/// - `shared:{group}:{flow_id}` - Shared across multiple flows
/// - `global:{flow_id}` - Global singleton flow
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PersistenceKey {
    parts: Vec<String>,
}

impl PersistenceKey {
    /// Create a new persistence key from parts.
    pub fn new(parts: Vec<String>) -> Self {
        Self { parts }
    }

    /// Create a key for a single flow.
    pub fn flow(flow_id: &str) -> Self {
        Self {
            parts: vec!["flow".to_string(), flow_id.to_string()],
        }
    }

    /// Create a user-scoped key.
    pub fn user(user_id: &str, flow_id: &str) -> Self {
        Self {
            parts: vec!["user".to_string(), user_id.to_string(), flow_id.to_string()],
        }
    }

    /// Create a shared key for multi-flow sharing.
    pub fn shared(group: &str, flow_id: &str) -> Self {
        Self {
            parts: vec!["shared".to_string(), group.to_string(), flow_id.to_string()],
        }
    }

    /// Create a global singleton key.
    pub fn global(flow_id: &str) -> Self {
        Self {
            parts: vec!["global".to_string(), flow_id.to_string()],
        }
    }

    /// Parse a key from string format (e.g., "user:123:checkout").
    pub fn parse(s: &str) -> PersistenceResult<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 2 {
            return Err(PersistenceError::InvalidKey(s.to_string()));
        }
        Ok(Self {
            parts: parts.iter().map(|p| p.to_string()).collect(),
        })
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> String {
        self.parts.join(":")
    }

    /// Get the scope prefix (first part).
    pub fn scope(&self) -> Option<&str> {
        self.parts.first().map(|s| s.as_str())
    }

    /// Check if this key matches a pattern (e.g., "user:*:checkout").
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split(':').collect();
        if pattern_parts.len() != self.parts.len() {
            return false;
        }
        for (i, p) in pattern_parts.iter().enumerate() {
            if *p != "*" && self.parts[i].as_str() != *p {
                return false;
            }
        }
        true
    }
}

impl std::fmt::Display for PersistenceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// PersistedFlow
// =============================================================================

/// Metadata for a persisted flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMetadata {
    pub flow_id: String,
    pub flow_name: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

impl FlowMetadata {
    pub fn new(flow_id: &str) -> Self {
        Self {
            flow_id: flow_id.to_string(),
            flow_name: None,
            version: None,
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.flow_name = Some(name.to_string());
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }
}

/// A persisted workflow flow with its state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedFlow {
    pub flow_id: String,
    pub state_snapshot: Vec<u8>,
    pub metadata: FlowMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub checksum: Option<String>,
}

impl PersistedFlow {
    /// Create a new persisted flow.
    pub fn new(flow_id: &str, state: impl Serialize, metadata: FlowMetadata) -> PersistenceResult<Self> {
        let state_snapshot = serde_json::to_vec(&state)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let now = Utc::now();
        let checksum = Some(Self::compute_checksum(&state_snapshot));

        Ok(Self {
            flow_id: flow_id.to_string(),
            state_snapshot,
            metadata,
            created_at: now,
            updated_at: now,
            checksum,
        })
    }

    /// Load state from the snapshot.
    pub fn load_state<T: DeserializeOwned>(&self) -> PersistenceResult<T> {
        serde_json::from_slice(&self.state_snapshot)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))
    }

    /// Update the state snapshot.
    pub fn update_state(&mut self, state: impl Serialize) -> PersistenceResult<()> {
        self.state_snapshot = serde_json::to_vec(&state)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;
        self.updated_at = Utc::now();
        self.checksum = Some(Self::compute_checksum(&self.state_snapshot));
        Ok(())
    }

    fn compute_checksum(data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

// =============================================================================
// FlowPersistence Trait
// =============================================================================

/// Trait for flow state persistence backends.
#[async_trait]
pub trait FlowPersistence: Send + Sync {
    /// Save (create or update) flow state.
    async fn save_state(
        &self,
        key: &PersistenceKey,
        state: impl Serialize + Send,
        metadata: FlowMetadata,
    ) -> PersistenceResult<()>;

    /// Load flow state by key.
    async fn load_state(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<PersistedFlow>;

    /// Delete flow state by key.
    async fn delete_state(&self, key: &PersistenceKey) -> PersistenceResult<bool>;

    /// List all flows, optionally filtered by key pattern.
    async fn list_flows(
        &self,
        pattern: Option<&str>,
    ) -> PersistenceResult<Vec<PersistedFlow>>;

    /// Check if a flow exists.
    async fn exists(&self, key: &PersistenceKey) -> PersistenceResult<bool>;

    /// Get metadata only (without loading full state).
    async fn get_metadata(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<FlowMetadata>;
}

// =============================================================================
// InMemoryPersistence (for testing)
// =============================================================================

/// In-memory implementation of FlowPersistence (for testing).
pub struct InMemoryPersistence {
    flows: dashmap::DashMap<String, PersistedFlow>,
}

impl InMemoryPersistence {
    pub fn new() -> Self {
        Self {
            flows: dashmap::DashMap::new(),
        }
    }

    fn make_key(key: &PersistenceKey) -> String {
        key.as_str()
    }
}

impl Default for InMemoryPersistence {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FlowPersistence for InMemoryPersistence {
    async fn save_state(
        &self,
        key: &PersistenceKey,
        state: impl Serialize + Send,
        metadata: FlowMetadata,
    ) -> PersistenceResult<()> {
        let key_str = Self::make_key(key);
        let flow_id = metadata.flow_id.clone();
        let mut persisted = PersistedFlow::new(&flow_id, state, metadata)?;

        // Preserve created_at if already exists
        if let Some(existing) = self.flows.get(&key_str) {
            persisted.created_at = existing.created_at;
        }

        self.flows.insert(key_str, persisted);
        Ok(())
    }

    async fn load_state(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<PersistedFlow> {
        let key_str = Self::make_key(key);
        self.flows
            .get(&key_str)
            .map(|v| v.value().clone())
            .ok_or_else(|| PersistenceError::FlowNotFound(key_str))
    }

    async fn delete_state(&self, key: &PersistenceKey) -> PersistenceResult<bool> {
        let key_str = Self::make_key(key);
        Ok(self.flows.remove(&key_str).is_some())
    }

    async fn list_flows(
        &self,
        pattern: Option<&str>,
    ) -> PersistenceResult<Vec<PersistedFlow>> {
        let all: Vec<PersistedFlow> = self.flows.iter().map(|entry| entry.value().clone()).collect();

        match pattern {
            Some(p) => Ok(all
                .into_iter()
                .filter(|f| {
                    let key = PersistenceKey::parse(&format!("{}:{}", f.metadata.flow_id, ""))
                        .map(|k| k.as_str())
                        .unwrap_or_default();
                    key.contains(p)
                })
                .collect()),
            None => Ok(all),
        }
    }

    async fn exists(&self, key: &PersistenceKey) -> PersistenceResult<bool> {
        let key_str = Self::make_key(key);
        Ok(self.flows.contains_key(&key_str))
    }

    async fn get_metadata(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<FlowMetadata> {
        let key_str = Self::make_key(key);
        self.flows
            .get(&key_str)
            .map(|v| v.value().metadata.clone())
            .ok_or_else(|| PersistenceError::FlowNotFound(key_str))
    }
}

// =============================================================================
// SqlitePersistence
// =============================================================================

/// SQLite-backed implementation of FlowPersistence.
pub struct SqlitePersistence {
    conn: std::sync::Mutex<rusqlite::Connection>,
}

impl SqlitePersistence {
    /// Create a new SqlitePersistence with an in-memory database.
    pub fn new_in_memory() -> PersistenceResult<Self> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;
        let persistence = Self {
            conn: std::sync::Mutex::new(conn),
        };
        persistence.init_schema()?;
        Ok(persistence)
    }

    /// Create a new SqlitePersistence with a file-based database.
    pub fn new_file<P: AsRef<Path>>(path: P) -> PersistenceResult<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;
        let persistence = Self {
            conn: std::sync::Mutex::new(conn),
        };
        persistence.init_schema()?;
        Ok(persistence)
    }

    fn init_schema(&self) -> PersistenceResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS persisted_flows (
                key TEXT PRIMARY KEY,
                flow_id TEXT NOT NULL,
                state_snapshot BLOB NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                checksum TEXT
            )",
            [],
        )
        .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_flow_id ON persisted_flows(flow_id)",
            [],
        )
        .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn make_key(key: &PersistenceKey) -> String {
        key.as_str()
    }
}

#[async_trait]
impl FlowPersistence for SqlitePersistence {
    async fn save_state(
        &self,
        key: &PersistenceKey,
        state: impl Serialize + Send,
        metadata: FlowMetadata,
    ) -> PersistenceResult<()> {
        let key_str = Self::make_key(key);
        let flow_id = metadata.flow_id.clone();
        let mut persisted = PersistedFlow::new(&flow_id, state, metadata)?;

        // Check if exists to preserve created_at
        let existing_created = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT created_at FROM persisted_flows WHERE key = ?1")
                .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;
            stmt.query_row([&key_str], |row| row.get::<_, String>(0))
                .ok()
        };

        if let Some(ts) = existing_created {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&ts) {
                persisted.created_at = dt.with_timezone(&Utc);
            }
        }

        let metadata_json = serde_json::to_string(&persisted.metadata)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO persisted_flows
             (key, flow_id, state_snapshot, metadata, created_at, updated_at, checksum)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                key_str,
                persisted.flow_id,
                persisted.state_snapshot,
                metadata_json,
                persisted.created_at.to_rfc3339(),
                persisted.updated_at.to_rfc3339(),
                persisted.checksum,
            ],
        )
        .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn load_state(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<PersistedFlow> {
        let key_str = Self::make_key(key);

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT flow_id, state_snapshot, metadata, created_at, updated_at, checksum
                 FROM persisted_flows WHERE key = ?1",
            )
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        let result = stmt
            .query_row([&key_str], |row| {
                let flow_id: String = row.get(0)?;
                let state_snapshot: Vec<u8> = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                let created_at: String = row.get(3)?;
                let updated_at: String = row.get(4)?;
                let checksum: Option<String> = row.get(5)?;

                let metadata: FlowMetadata = serde_json::from_str(&metadata_json)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;

                Ok(PersistedFlow {
                    flow_id,
                    state_snapshot,
                    metadata,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    checksum,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    PersistenceError::FlowNotFound(key_str)
                }
                _ => PersistenceError::DatabaseError(e.to_string()),
            })?;

        Ok(result)
    }

    async fn delete_state(&self, key: &PersistenceKey) -> PersistenceResult<bool> {
        let key_str = Self::make_key(key);
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute("DELETE FROM persisted_flows WHERE key = ?1", [&key_str])
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;
        Ok(affected > 0)
    }

    async fn list_flows(
        &self,
        _pattern: Option<&str>,
    ) -> PersistenceResult<Vec<PersistedFlow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT flow_id, state_snapshot, metadata, created_at, updated_at, checksum
                 FROM persisted_flows",
            )
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        let flows = stmt
            .query_map([], |row| {
                let flow_id: String = row.get(0)?;
                let state_snapshot: Vec<u8> = row.get(1)?;
                let metadata_json: String = row.get(2)?;
                let created_at: String = row.get(3)?;
                let updated_at: String = row.get(4)?;
                let checksum: Option<String> = row.get(5)?;

                let metadata: FlowMetadata = serde_json::from_str(&metadata_json)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;

                Ok(PersistedFlow {
                    flow_id,
                    state_snapshot,
                    metadata,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    checksum,
                })
            })
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(flows)
    }

    async fn exists(&self, key: &PersistenceKey) -> PersistenceResult<bool> {
        let key_str = Self::make_key(key);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT 1 FROM persisted_flows WHERE key = ?1")
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;
        let exists = stmt
            .query_row([&key_str], |_| Ok(()))
            .map(|_| true)
            .unwrap_or(false);
        Ok(exists)
    }

    async fn get_metadata(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<FlowMetadata> {
        let key_str = Self::make_key(key);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT metadata FROM persisted_flows WHERE key = ?1")
            .map_err(|e| PersistenceError::DatabaseError(e.to_string()))?;

        let metadata_json: String = stmt
            .query_row([&key_str], |row| row.get(0))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    PersistenceError::FlowNotFound(key_str)
                }
                _ => PersistenceError::DatabaseError(e.to_string()),
            })?;

        serde_json::from_str(&metadata_json)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))
    }
}

// =============================================================================
// @persist Decorator
// =============================================================================

/// Decorator configuration for persist.
#[derive(Debug, Clone)]
pub struct PersistConfig {
    pub key: String,
    pub auto_save: bool,
    pub version: Option<String>,
}

impl PersistConfig {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            auto_save: false,
            version: None,
        }
    }

    pub fn with_auto_save(mut self) -> Self {
        self.auto_save = true;
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }
}

/// Attribute macro placeholder for declarative persistence marking.
/// In real implementation, this would be a proc macro.
/// Here we provide a trait-based alternative.
pub fn persist<S: Into<String>>(key: S) -> PersistConfig {
    PersistConfig::new(&key.into())
}

/// Extension trait to add persistence methods to workflow state.
#[async_trait]
pub trait PersistableState: Serialize + DeserializeOwned + Send + Sync {
    /// Get the persistence key for this state.
    fn persistence_key(&self) -> Option<String>;

    /// Save this state using the provided persistence backend.
    async fn save<P: FlowPersistence>(
        &self,
        backend: &P,
        key: &PersistenceKey,
    ) -> PersistenceResult<()> {
        let metadata = FlowMetadata::new(&self.persistence_key().unwrap_or_default());
        backend
            .save_state(key, self, metadata)
            .await
    }

    /// Load state from backend and deserialize.
    async fn load<P: FlowPersistence, T: DeserializeOwned>(
        backend: &P,
        key: &PersistenceKey,
    ) -> PersistenceResult<T> {
        let persisted = backend.load_state(key).await?;
        persisted.load_state()
    }
}

// =============================================================================
// FlowStateManager
// =============================================================================

/// Manager for flow persistence operations with convenient API.
pub struct FlowStateManager<P: FlowPersistence> {
    backend: P,
}

impl<P: FlowPersistence> FlowStateManager<P> {
    pub fn new(backend: P) -> Self {
        Self { backend }
    }

    /// Save flow state with automatic key generation.
    pub async fn save<S: Serialize + Send>(
        &self,
        flow_id: &str,
        state: S,
        metadata: FlowMetadata,
    ) -> PersistenceResult<()> {
        let key = PersistenceKey::flow(flow_id);
        self.backend.save_state(&key, state, metadata).await
    }

    /// Save with custom key.
    pub async fn save_with_key<S: Serialize + Send>(
        &self,
        key: &PersistenceKey,
        state: S,
        metadata: FlowMetadata,
    ) -> PersistenceResult<()> {
        self.backend.save_state(key, state, metadata).await
    }

    /// Load flow state.
    pub async fn load<T: DeserializeOwned>(
        &self,
        flow_id: &str,
    ) -> PersistenceResult<T> {
        let key = PersistenceKey::flow(flow_id);
        let persisted = self.backend.load_state(&key).await?;
        persisted.load_state()
    }

    /// Load with custom key.
    pub async fn load_with_key<T: DeserializeOwned>(
        &self,
        key: &PersistenceKey,
    ) -> PersistenceResult<T> {
        let persisted = self.backend.load_state(key).await?;
        persisted.load_state()
    }

    /// Delete flow state.
    pub async fn delete(&self, flow_id: &str) -> PersistenceResult<bool> {
        let key = PersistenceKey::flow(flow_id);
        self.backend.delete_state(&key).await
    }

    /// Check if flow exists.
    pub async fn exists(&self, flow_id: &str) -> PersistenceResult<bool> {
        let key = PersistenceKey::flow(flow_id);
        self.backend.exists(&key).await
    }

    /// List all flows.
    pub async fn list_all(&self) -> PersistenceResult<Vec<PersistedFlow>> {
        self.backend.list_flows(None).await
    }

    /// Get the underlying backend.
    pub fn backend(&self) -> &P {
        &self.backend
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        counter: i32,
        name: String,
    }

    // =============================================================================
    // PersistenceKey Tests
    // =============================================================================

    #[tokio::test]
    async fn test_persistence_key_flow() {
        let key = PersistenceKey::flow("my-workflow");
        assert_eq!(key.as_str(), "flow:my-workflow");
        assert_eq!(key.scope(), Some("flow"));
    }

    #[tokio::test]
    async fn test_persistence_key_user() {
        let key = PersistenceKey::user("user123", "checkout");
        assert_eq!(key.as_str(), "user:user123:checkout");
        assert_eq!(key.scope(), Some("user"));
    }

    #[tokio::test]
    async fn test_persistence_key_shared() {
        let key = PersistenceKey::shared("team-a", "pipeline");
        assert_eq!(key.as_str(), "shared:team-a:pipeline");
    }

    #[tokio::test]
    async fn test_persistence_key_global() {
        let key = PersistenceKey::global("singleton");
        assert_eq!(key.as_str(), "global:singleton");
    }

    #[tokio::test]
    async fn test_persistence_key_parse() {
        let key = PersistenceKey::parse("user:123:checkout").unwrap();
        assert_eq!(key.parts, vec!["user", "123", "checkout"]);
    }

    #[tokio::test]
    async fn test_persistence_key_parse_invalid() {
        let result = PersistenceKey::parse("invalid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_persistence_key_pattern() {
        let key = PersistenceKey::user("123", "checkout");
        assert!(key.matches_pattern("user:123:checkout"));
        assert!(key.matches_pattern("user:*:checkout"));
        assert!(key.matches_pattern("user:123:*"));
        assert!(!key.matches_pattern("user:456:checkout"));
        assert!(!key.matches_pattern("user:123"));
    }

    // =============================================================================
    // PersistedFlow Tests
    // =============================================================================

    #[tokio::test]
    async fn test_persisted_flow_create() {
        let state = TestState {
            counter: 42,
            name: "test".to_string(),
        };
        let metadata = FlowMetadata::new("test-flow");
        let flow = PersistedFlow::new("test-flow", &state, metadata).unwrap();

        assert_eq!(flow.flow_id, "test-flow");
        assert!(flow.checksum.is_some());

        let loaded: TestState = flow.load_state().unwrap();
        assert_eq!(loaded.counter, 42);
        assert_eq!(loaded.name, "test");
    }

    #[tokio::test]
    async fn test_persisted_flow_update() {
        let state = TestState {
            counter: 0,
            name: "initial".to_string(),
        };
        let metadata = FlowMetadata::new("test-flow");
        let mut flow = PersistedFlow::new("test-flow", &state, metadata).unwrap();

        let new_state = TestState {
            counter: 100,
            name: "updated".to_string(),
        };
        flow.update_state(&new_state).unwrap();

        let loaded: TestState = flow.load_state().unwrap();
        assert_eq!(loaded.counter, 100);
        assert_eq!(loaded.name, "updated");
    }

    // =============================================================================
    // InMemoryPersistence Tests
    // =============================================================================

    #[tokio::test]
    async fn test_in_memory_save_load() {
        let persistence = InMemoryPersistence::new();
        let key = PersistenceKey::flow("test-flow");

        let state = TestState {
            counter: 10,
            name: "hello".to_string(),
        };
        let metadata = FlowMetadata::new("test-flow");
        persistence
            .save_state(&key, &state, metadata)
            .await
            .unwrap();

        let loaded = persistence.load_state(&key).await.unwrap();
        let loaded_state: TestState = loaded.load_state().unwrap();
        assert_eq!(loaded_state.counter, 10);
    }

    #[tokio::test]
    async fn test_in_memory_delete() {
        let persistence = InMemoryPersistence::new();
        let key = PersistenceKey::flow("test-flow");

        let state = TestState {
            counter: 1,
            name: "to-delete".to_string(),
        };
        let metadata = FlowMetadata::new("test-flow");
        persistence
            .save_state(&key, &state, metadata)
            .await
            .unwrap();

        assert!(persistence.exists(&key).await.unwrap());
        persistence.delete_state(&key).await.unwrap();
        assert!(!persistence.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_not_found() {
        let persistence = InMemoryPersistence::new();
        let key = PersistenceKey::flow("nonexistent");

        let result = persistence.load_state(&key).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_list_flows() {
        let persistence = InMemoryPersistence::new();

        for i in 0..3 {
            let key = PersistenceKey::flow(&format!("flow-{}", i));
            let state = TestState {
                counter: i,
                name: format!("flow-{}", i),
            };
            let metadata = FlowMetadata::new(&format!("flow-{}", i));
            persistence
                .save_state(&key, &state, metadata)
                .await
                .unwrap();
        }

        let all = persistence.list_flows(None).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    // =============================================================================
    // SqlitePersistence Tests
    // =============================================================================

    #[tokio::test]
    async fn test_sqlite_save_load() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("sqlite-test");

        let state = TestState {
            counter: 99,
            name: "sqlite".to_string(),
        };
        let metadata = FlowMetadata::new("sqlite-test");
        persistence
            .save_state(&key, &state, metadata)
            .await
            .unwrap();

        let loaded = persistence.load_state(&key).await.unwrap();
        let loaded_state: TestState = loaded.load_state().unwrap();
        assert_eq!(loaded_state.counter, 99);
    }

    #[tokio::test]
    async fn test_sqlite_update() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("update-test");

        let state1 = TestState {
            counter: 1,
            name: "v1".to_string(),
        };
        let metadata = FlowMetadata::new("update-test");
        persistence
            .save_state(&key, &state1, metadata)
            .await
            .unwrap();

        let state2 = TestState {
            counter: 2,
            name: "v2".to_string(),
        };
        persistence
            .save_state(&key, &state2, FlowMetadata::new("update-test"))
            .await
            .unwrap();

        let loaded = persistence.load_state(&key).await.unwrap();
        let loaded_state: TestState = loaded.load_state().unwrap();
        assert_eq!(loaded_state.counter, 2);
        assert_eq!(loaded_state.name, "v2");
    }

    #[tokio::test]
    async fn test_sqlite_delete() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("delete-test");

        let state = TestState {
            counter: 1,
            name: "delete".to_string(),
        };
        let metadata = FlowMetadata::new("delete-test");
        persistence
            .save_state(&key, &state, metadata)
            .await
            .unwrap();

        persistence.delete_state(&key).await.unwrap();
        assert!(!persistence.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_metadata() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("metadata-test");

        let state = TestState {
            counter: 1,
            name: "meta".to_string(),
        };
        let mut metadata = FlowMetadata::new("metadata-test");
        metadata = metadata.with_name("My Flow").with_version("1.0.0");
        persistence
            .save_state(&key, &state, metadata)
            .await
            .unwrap();

        let loaded_meta = persistence.get_metadata(&key).await.unwrap();
        assert_eq!(loaded_meta.flow_name, Some("My Flow".to_string()));
        assert_eq!(loaded_meta.version, Some("1.0.0".to_string()));
    }

    // =============================================================================
    // FlowStateManager Tests
    // =============================================================================

    #[tokio::test]
    async fn test_flow_state_manager() {
        let persistence = InMemoryPersistence::new();
        let manager = FlowStateManager::new(persistence);

        let state = TestState {
            counter: 50,
            name: "manager".to_string(),
        };
        manager
            .save(
                "managed-flow",
                &state,
                FlowMetadata::new("managed-flow"),
            )
            .await
            .unwrap();

        let loaded: TestState = manager.load("managed-flow").await.unwrap();
        assert_eq!(loaded.counter, 50);
    }

    #[tokio::test]
    async fn test_flow_state_manager_custom_key() {
        let persistence = InMemoryPersistence::new();
        let manager = FlowStateManager::new(persistence);

        let key = PersistenceKey::user("admin", "admin-flow");
        let state = TestState {
            counter: 123,
            name: "admin-state".to_string(),
        };
        manager
            .save_with_key(&key, &state, FlowMetadata::new("admin-flow"))
            .await
            .unwrap();

        let loaded: TestState = manager.load_with_key(&key).await.unwrap();
        assert_eq!(loaded.counter, 123);
    }

    // =============================================================================
    // PersistConfig Tests
    // =============================================================================

    #[tokio::test]
    async fn test_persist_config() {
        let config = persist("my-flow")
            .with_auto_save()
            .with_version("2.0");

        assert_eq!(config.key, "my-flow");
        assert!(config.auto_save);
        assert_eq!(config.version, Some("2.0".to_string()));
    }

    // =============================================================================
    // FlowMetadata Tests
    // =============================================================================

    #[tokio::test]
    async fn test_flow_metadata() {
        let metadata = FlowMetadata::new("flow-1")
            .with_name("Checkout Flow")
            .with_version("1.0.0")
            .with_tag("payment")
            .with_tag("critical");

        assert_eq!(metadata.flow_id, "flow-1");
        assert_eq!(metadata.flow_name, Some("Checkout Flow".to_string()));
        assert_eq!(metadata.version, Some("1.0.0".to_string()));
        assert_eq!(metadata.tags, vec!["payment", "critical"]);
    }

    // ── New enhanced flow persistence tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_persistence_key_display() {
        let key = PersistenceKey::user("alice", "checkout");
        let display = format!("{}", key);
        assert_eq!(display, "user:alice:checkout");
    }

    #[tokio::test]
    async fn test_persistence_key_scope() {
        assert_eq!(PersistenceKey::flow("wf").scope(), Some("flow"));
        assert_eq!(PersistenceKey::user("u1", "wf").scope(), Some("user"));
        assert_eq!(PersistenceKey::shared("team", "wf").scope(), Some("shared"));
        assert_eq!(PersistenceKey::global("wf").scope(), Some("global"));
        assert_eq!(PersistenceKey::new(vec!["custom".into(), "path".into()]).scope(), Some("custom"));
    }

    #[tokio::test]
    async fn test_persistence_key_from_parts() {
        let key = PersistenceKey::new(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(key.as_str(), "a:b:c");
        assert_eq!(key.parts.len(), 3);
    }

    #[tokio::test]
    async fn test_persisted_flow_checksum() {
        let state = TestState { counter: 42, name: "checksum-test".into() };
        let metadata = FlowMetadata::new("checksum-flow");
        let flow = PersistedFlow::new("checksum-flow", &state, metadata.clone()).unwrap();
        assert!(flow.checksum.is_some());
        let checksum = flow.checksum.clone().unwrap();

        // Same data should produce same checksum
        let flow2 = PersistedFlow::new("checksum-flow", &state, metadata).unwrap();
        assert_eq!(flow2.checksum.unwrap(), checksum);
    }

    #[tokio::test]
    async fn test_persisted_flow_load_state_deserializes() {
        let state = TestState { counter: 999, name: "load-test".into() };
        let metadata = FlowMetadata::new("load-flow");
        let flow = PersistedFlow::new("load-flow", &state, metadata).unwrap();
        let loaded: TestState = flow.load_state().unwrap();
        assert_eq!(loaded.counter, 999);
        assert_eq!(loaded.name, "load-test");
    }

    #[tokio::test]
    async fn test_persisted_flow_update_preserves_created_at() {
        let state1 = TestState { counter: 1, name: "v1".into() };
        let metadata = FlowMetadata::new("update-preserve");
        let mut flow = PersistedFlow::new("update-preserve", &state1, metadata).unwrap();
        let created = flow.created_at;

        // Update state
        let state2 = TestState { counter: 2, name: "v2".into() };
        flow.update_state(&state2).unwrap();
        assert_eq!(flow.created_at, created); // created_at unchanged
        assert!(flow.updated_at >= created);
    }

    #[tokio::test]
    async fn test_sqlite_update_preserves_created_at() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("sqlite-created-at");

        let state1 = TestState { counter: 1, name: "v1".into() };
        persistence
            .save_state(&key, &state1, FlowMetadata::new("sqlite-created-at"))
            .await
            .unwrap();

        let loaded1 = persistence.load_state(&key).await.unwrap();
        let created_at = loaded1.created_at;

        // Update
        let state2 = TestState { counter: 2, name: "v2".into() };
        persistence
            .save_state(&key, &state2, FlowMetadata::new("sqlite-created-at"))
            .await
            .unwrap();

        let loaded2 = persistence.load_state(&key).await.unwrap();
        assert_eq!(loaded2.created_at, created_at);
        assert!(loaded2.updated_at >= created_at);
    }

    #[tokio::test]
    async fn test_sqlite_not_found_error() {
        let persistence = SqlitePersistence::new_in_memory().unwrap();
        let key = PersistenceKey::flow("nonexistent");
        let result = persistence.load_state(&key).await;
        assert!(result.is_err());
        if let Err(PersistenceError::FlowNotFound(_)) = result {
            // expected
        } else {
            panic!("Expected FlowNotFound error");
        }
    }

    #[tokio::test]
    async fn test_in_memory_overwrite_same_key() {
        let persistence = InMemoryPersistence::new();
        let key = PersistenceKey::flow("overwrite-test");

        let state1 = TestState { counter: 1, name: "first".into() };
        persistence
            .save_state(&key, &state1, FlowMetadata::new("overwrite-test"))
            .await
            .unwrap();

        let state2 = TestState { counter: 2, name: "second".into() };
        persistence
            .save_state(&key, &state2, FlowMetadata::new("overwrite-test"))
            .await
            .unwrap();

        let all = persistence.list_flows(None).await.unwrap();
        assert_eq!(all.len(), 1); // only one flow, overwritten
        let loaded: TestState = persistence.load_state(&key).await.unwrap().load_state().unwrap();
        assert_eq!(loaded.counter, 2);
    }
}
