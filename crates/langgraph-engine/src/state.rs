//! Graph state and typed channel support.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// State metadata tracking execution progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMetadata {
    /// Unique run identifier.
    pub run_id: String,
    /// Current step number.
    pub step: usize,
    /// History of visited nodes.
    pub node_history: Vec<String>,
    /// Whether execution is currently interrupted.
    pub is_interrupted: bool,
    /// Associated checkpoint ID (if any).
    pub checkpoint_id: Option<String>,
    /// Error message from last failed node (if any).
    pub last_error: Option<String>,
}

impl Default for StateMetadata {
    fn default() -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            step: 0,
            node_history: Vec::new(),
            is_interrupted: false,
            checkpoint_id: None,
            last_error: None,
        }
    }
}

/// The core graph state — a typed key-value store with execution metadata.
///
/// Channels store typed values as `serde_json::Value` for runtime flexibility,
/// while the getter/setter API provides compile-time ergonomics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    /// Channel data: key → JSON value.
    pub channels: HashMap<String, serde_json::Value>,
    /// Execution metadata.
    pub metadata: StateMetadata,
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphState {
    /// Create a new empty state with fresh metadata.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            metadata: StateMetadata::default(),
        }
    }

    /// Create a new state with a specific run_id (useful for checkpoint restore).
    pub fn with_run_id(run_id: String) -> Self {
        Self {
            channels: HashMap::new(),
            metadata: StateMetadata {
                run_id,
                ..Default::default()
            },
        }
    }

    /// Get a typed value from a channel. Returns `None` if missing or deserialization fails.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.channels
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get a typed value from a channel, returning an error if missing or deserialization fails.
    pub fn get_required<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<T, kias_common::KiasError> {
        let val = self.channels.get(key).ok_or_else(|| {
            kias_common::KiasError::Validation(format!("Required channel '{}' not found", key))
        })?;
        serde_json::from_value(val.clone()).map_err(|e| {
            kias_common::KiasError::Validation(format!(
                "Failed to deserialize channel '{}': {}",
                key, e
            ))
        })
    }

    /// Set a typed value in a channel.
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.channels.insert(key.to_string(), v);
        }
    }

    /// Check whether a channel exists.
    pub fn has(&self, key: &str) -> bool {
        self.channels.contains_key(key)
    }

    /// Remove a channel, returning its value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.channels.remove(key)
    }

    /// Get all channel names.
    pub fn keys(&self) -> Vec<&str> {
        self.channels.keys().map(|s| s.as_str()).collect()
    }

    /// Merge another state into this one (overwrites existing channels).
    pub fn merge(&mut self, other: GraphState) {
        for (k, v) in other.channels {
            self.channels.insert(k, v);
        }
    }

    /// Merge another state, keeping existing values (only inserts missing channels).
    pub fn merge_keep_existing(&mut self, other: GraphState) {
        for (k, v) in other.channels {
            self.channels.entry(k).or_insert(v);
        }
    }

    /// Create a snapshot of this state for checkpointing.
    pub fn snapshot(&self) -> GraphStateSnapshot {
        GraphStateSnapshot {
            channels: self.channels.clone(),
            metadata: self.metadata.clone(),
        }
    }

    /// Restore from a snapshot.
    pub fn restore_from_snapshot(snapshot: &GraphStateSnapshot) -> Self {
        Self {
            channels: snapshot.channels.clone(),
            metadata: snapshot.metadata.clone(),
        }
    }
}

/// An immutable snapshot of graph state for checkpoint persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStateSnapshot {
    pub channels: HashMap<String, serde_json::Value>,
    pub metadata: StateMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_is_empty() {
        let state = GraphState::new();
        assert!(state.channels.is_empty());
        assert_eq!(state.metadata.step, 0);
        assert!(!state.metadata.is_interrupted);
        assert!(state.metadata.node_history.is_empty());
    }

    #[test]
    fn test_default_trait() {
        let state = GraphState::default();
        assert!(state.channels.is_empty());
    }

    #[test]
    fn test_with_run_id() {
        let state = GraphState::with_run_id("run-123".to_string());
        assert_eq!(state.metadata.run_id, "run-123");
        assert!(state.channels.is_empty());
    }

    #[test]
    fn test_set_and_get_i32() {
        let mut state = GraphState::new();
        state.set("count", 42i32);
        assert_eq!(state.get::<i32>("count"), Some(42));
    }

    #[test]
    fn test_get_string() {
        let mut state = GraphState::new();
        state.set("name", "hello");
        assert_eq!(state.get::<String>("name"), Some("hello".to_string()));
    }

    #[test]
    fn test_get_missing_key_returns_none() {
        let state = GraphState::new();
        assert_eq!(state.get::<i32>("missing"), None);
    }

    #[test]
    fn test_get_wrong_type_returns_none() {
        let mut state = GraphState::new();
        state.set("val", 42i32);
        assert_eq!(state.get::<String>("val"), None);
    }

    #[test]
    fn test_get_required_ok() {
        let mut state = GraphState::new();
        state.set("val", 100u64);
        assert_eq!(state.get_required::<u64>("val").unwrap(), 100);
    }

    #[test]
    fn test_get_required_missing_returns_err() {
        let state = GraphState::new();
        assert!(state.get_required::<i32>("missing").is_err());
    }

    #[test]
    fn test_has() {
        let mut state = GraphState::new();
        assert!(!state.has("key"));
        state.set("key", 1);
        assert!(state.has("key"));
    }

    #[test]
    fn test_remove() {
        let mut state = GraphState::new();
        state.set("key", 42i32);
        let removed = state.remove("key");
        assert!(removed.is_some());
        assert!(!state.has("key"));
        assert!(state.remove("key").is_none());
    }

    #[test]
    fn test_keys() {
        let mut state = GraphState::new();
        state.set("a", 1);
        state.set("b", 2);
        let mut keys = state.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn test_merge_overwrites() {
        let mut state1 = GraphState::new();
        state1.set("shared", 1i32);
        state1.set("only1", 10i32);

        let mut state2 = GraphState::new();
        state2.set("shared", 99i32);
        state2.set("only2", 20i32);

        state1.merge(state2);
        assert_eq!(state1.get::<i32>("shared"), Some(99));
        assert_eq!(state1.get::<i32>("only1"), Some(10));
        assert_eq!(state1.get::<i32>("only2"), Some(20));
    }

    #[test]
    fn test_merge_keep_existing() {
        let mut state1 = GraphState::new();
        state1.set("shared", 1i32);

        let mut state2 = GraphState::new();
        state2.set("shared", 99i32);
        state2.set("new", 42i32);

        state1.merge_keep_existing(state2);
        assert_eq!(state1.get::<i32>("shared"), Some(1));
        assert_eq!(state1.get::<i32>("new"), Some(42));
    }

    #[test]
    fn test_snapshot_and_restore() {
        let mut state = GraphState::new();
        state.set("data", "value");
        state.metadata.step = 5;

        let snap = state.snapshot();
        assert_eq!(snap.metadata.step, 5);

        let restored = GraphState::restore_from_snapshot(&snap);
        assert_eq!(restored.get::<String>("data"), Some("value".to_string()));
        assert_eq!(restored.metadata.step, 5);
    }

    #[test]
    fn test_set_complex_types() {
        let mut state = GraphState::new();
        let vec = vec![1, 2, 3];
        state.set("list", &vec);
        assert_eq!(state.get::<Vec<i32>>("list"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_overwrite_value() {
        let mut state = GraphState::new();
        state.set("key", 1i32);
        state.set("key", 2i32);
        assert_eq!(state.get::<i32>("key"), Some(2));
    }
}
