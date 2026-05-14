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
