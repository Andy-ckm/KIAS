//! # Incremental Checkpoint Delta Compression
//!
//! Stores only diffs between consecutive agent state snapshots,
//! dramatically reducing storage for frequently-checkpointed agents.
//!
//! ## Design
//!
//! 1. Full snapshot stored every N checkpoints (configurable).
//! 2. Between full snapshots, only JSON diffs are stored.
//! 3. Recovery replays full snapshot + chain of deltas.
//! 4. Supports delta chain compaction (squash N deltas into one).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A full state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSnapshot {
    /// Unique snapshot ID.
    pub id: String,
    /// Agent this snapshot belongs to.
    pub agent_id: String,
    /// Sequence number (monotonically increasing).
    pub seq: u64,
    /// The complete state as JSON.
    pub state: serde_json::Value,
    /// When this snapshot was taken.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Size of the state in bytes (for metrics).
    pub state_bytes: u64,
}

/// A delta between two consecutive states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    /// Unique delta ID.
    pub id: String,
    /// Agent this delta belongs to.
    pub agent_id: String,
    /// Sequence number (must be contiguous within a chain).
    pub seq: u64,
    /// Reference to the base snapshot ID.
    pub base_snapshot_id: String,
    /// Key-level diffs: path → (old_value, new_value).
    /// None old = added, None new = removed.
    pub diffs: Vec<FieldDiff>,
    /// Compression ratio achieved (original_size / delta_size).
    pub compression_ratio: f64,
    /// When this delta was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A single field-level diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    /// JSON pointer path (e.g. "/status/health").
    pub path: String,
    /// Previous value (None if field was added).
    pub old_value: Option<serde_json::Value>,
    /// New value (None if field was removed).
    pub new_value: Option<serde_json::Value>,
}

/// Configuration for the delta store.
#[derive(Debug, Clone)]
pub struct DeltaConfig {
    /// Store a full snapshot every N checkpoints.
    pub full_snapshot_interval: u64,
    /// Maximum delta chain length before forced compaction.
    pub max_chain_length: u64,
    /// Enable compression ratio tracking.
    pub track_compression: bool,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            full_snapshot_interval: 10,
            max_chain_length: 50,
            track_compression: true,
        }
    }
}

/// Result of computing a delta between two states.
#[derive(Debug, Clone)]
pub struct DeltaResult {
    /// The computed diffs.
    pub diffs: Vec<FieldDiff>,
    /// Size of the delta in bytes.
    pub delta_bytes: u64,
    /// Size of the original full state in bytes.
    pub original_bytes: u64,
    /// Compression ratio (original / delta).
    pub compression_ratio: f64,
}

/// Incremental checkpoint store with delta compression.
pub struct DeltaStore {
    config: DeltaConfig,
    /// Full snapshots by agent_id.
    snapshots: HashMap<String, Vec<FullSnapshot>>,
    /// Delta chains by agent_id.
    deltas: HashMap<String, Vec<Delta>>,
    /// Sequence counters by agent_id.
    seq_counters: HashMap<String, u64>,
}

impl Default for DeltaStore {
    fn default() -> Self {
        Self::new(DeltaConfig::default())
    }
}

impl DeltaStore {
    pub fn new(config: DeltaConfig) -> Self {
        Self {
            config,
            snapshots: HashMap::new(),
            deltas: HashMap::new(),
            seq_counters: HashMap::new(),
        }
    }

    /// Store a new checkpoint. Automatically decides whether to store
    /// a full snapshot or a delta based on the interval.
    pub fn checkpoint(&mut self, agent_id: &str, state: serde_json::Value) -> CheckpointOutcome {
        let seq = self.next_seq(agent_id);
        let state_bytes = serde_json::to_string(&state)
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        // Check if we should store a full snapshot
        let should_full = seq == 0
            || seq % self.config.full_snapshot_interval == 0
            || self.exceeds_chain_length(agent_id);

        if should_full {
            let snapshot = FullSnapshot {
                id: format!("{}-full-{}", agent_id, seq),
                agent_id: agent_id.to_string(),
                seq,
                state: state.clone(),
                created_at: chrono::Utc::now(),
                state_bytes,
            };
            self.snapshots
                .entry(agent_id.to_string())
                .or_default()
                .push(snapshot);
            CheckpointOutcome::FullSnapshot {
                seq,
                bytes: state_bytes,
            }
        } else {
            // Compute delta against the last state
            let last_state = self.get_latest_state(agent_id);
            let delta_result = compute_delta(&last_state, &state);

            let base_id = self.get_latest_snapshot_id(agent_id).unwrap_or_default();

            let delta = Delta {
                id: format!("{}-delta-{}", agent_id, seq),
                agent_id: agent_id.to_string(),
                seq,
                base_snapshot_id: base_id,
                diffs: delta_result.diffs.clone(),
                compression_ratio: delta_result.compression_ratio,
                created_at: chrono::Utc::now(),
            };
            self.deltas
                .entry(agent_id.to_string())
                .or_default()
                .push(delta);

            CheckpointOutcome::Delta {
                seq,
                delta_bytes: delta_result.delta_bytes,
                original_bytes: delta_result.original_bytes,
                compression_ratio: delta_result.compression_ratio,
                diff_count: delta_result.diffs.len(),
            }
        }
    }

    /// Reconstruct the full state at a given sequence number.
    pub fn reconstruct(&self, agent_id: &str, target_seq: u64) -> Option<serde_json::Value> {
        // Find the base full snapshot at or before target_seq
        let snapshots = self.snapshots.get(agent_id)?;
        let base = snapshots.iter().rev().find(|s| s.seq <= target_seq)?;

        let mut state = base.state.clone();

        // Apply deltas from base.seq+1 to target_seq
        if let Some(deltas) = self.deltas.get(agent_id) {
            for delta in deltas
                .iter()
                .filter(|d| d.seq > base.seq && d.seq <= target_seq)
            {
                apply_diffs(&mut state, &delta.diffs);
            }
        }

        Some(state)
    }

    /// Get the latest state for an agent.
    pub fn get_latest_state(&self, agent_id: &str) -> serde_json::Value {
        let latest_seq = self.seq_counters.get(agent_id).copied().unwrap_or(0);
        if latest_seq == 0 {
            return serde_json::Value::Null;
        }
        self.reconstruct(agent_id, latest_seq - 1)
            .unwrap_or(serde_json::Value::Null)
    }

    /// Compact delta chain: squash all deltas between two full snapshots into one.
    pub fn compact(&mut self, agent_id: &str) -> CompactionResult {
        let deltas = match self.deltas.get_mut(agent_id) {
            Some(d) if d.len() >= 2 => d,
            _ => return CompactionResult::NoAction,
        };

        let original_count = deltas.len();
        let first = deltas.first().unwrap();
        let last = deltas.last().unwrap();

        // Merge all diffs
        let mut merged_diffs: HashMap<String, FieldDiff> = HashMap::new();
        for delta in deltas.iter() {
            for diff in &delta.diffs {
                merged_diffs.insert(diff.path.clone(), diff.clone());
            }
        }

        let compacted = Delta {
            id: format!("{}-compacted-{}", agent_id, last.seq),
            agent_id: agent_id.to_string(),
            seq: last.seq,
            base_snapshot_id: first.base_snapshot_id.clone(),
            diffs: merged_diffs.into_values().collect(),
            compression_ratio: last.compression_ratio,
            created_at: chrono::Utc::now(),
        };

        deltas.clear();
        deltas.push(compacted);

        CompactionResult::Compacted {
            original_deltas: original_count,
            compacted_to: 1,
        }
    }

    /// Get storage statistics for an agent.
    pub fn stats(&self, agent_id: &str) -> StorageStats {
        let snapshot_count = self.snapshots.get(agent_id).map(|s| s.len()).unwrap_or(0);
        let delta_count = self.deltas.get(agent_id).map(|d| d.len()).unwrap_or(0);
        let snapshot_bytes: u64 = self
            .snapshots
            .get(agent_id)
            .map(|s| s.iter().map(|s| s.state_bytes).sum())
            .unwrap_or(0);

        let avg_compression = if delta_count > 0 {
            self.deltas
                .get(agent_id)
                .map(|d| d.iter().map(|d| d.compression_ratio).sum::<f64>() / d.len() as f64)
                .unwrap_or(1.0)
        } else {
            1.0
        };

        StorageStats {
            agent_id: agent_id.to_string(),
            snapshot_count,
            delta_count,
            total_snapshots_bytes: snapshot_bytes,
            avg_compression_ratio: avg_compression,
        }
    }

    fn next_seq(&mut self, agent_id: &str) -> u64 {
        let counter = self.seq_counters.entry(agent_id.to_string()).or_insert(0);
        let seq = *counter;
        *counter += 1;
        seq
    }

    fn exceeds_chain_length(&self, agent_id: &str) -> bool {
        self.deltas
            .get(agent_id)
            .map(|d| d.len() as u64 >= self.config.max_chain_length)
            .unwrap_or(false)
    }

    fn get_latest_snapshot_id(&self, agent_id: &str) -> Option<String> {
        self.snapshots
            .get(agent_id)
            .and_then(|s| s.last().map(|s| s.id.clone()))
    }
}

/// Outcome of a checkpoint operation.
#[derive(Debug, Clone)]
pub enum CheckpointOutcome {
    FullSnapshot {
        seq: u64,
        bytes: u64,
    },
    Delta {
        seq: u64,
        delta_bytes: u64,
        original_bytes: u64,
        compression_ratio: f64,
        diff_count: usize,
    },
}

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub enum CompactionResult {
    Compacted {
        original_deltas: usize,
        compacted_to: usize,
    },
    NoAction,
}

/// Storage statistics for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub agent_id: String,
    pub snapshot_count: usize,
    pub delta_count: usize,
    pub total_snapshots_bytes: u64,
    pub avg_compression_ratio: f64,
}

// ── Delta computation helpers ──────────────────────────────────────────────

/// Compute field-level diffs between two JSON values.
pub fn compute_delta(old: &serde_json::Value, new: &serde_json::Value) -> DeltaResult {
    let mut diffs = Vec::new();
    compute_diffs_recursive(old, new, String::new(), &mut diffs);

    let delta_bytes = serde_json::to_string(&diffs)
        .map(|s| s.len() as u64)
        .unwrap_or(0);
    let original_bytes = serde_json::to_string(new)
        .map(|s| s.len() as u64)
        .unwrap_or(0);

    let compression_ratio = if delta_bytes > 0 {
        original_bytes as f64 / delta_bytes as f64
    } else {
        1.0
    };

    DeltaResult {
        diffs,
        delta_bytes,
        original_bytes,
        compression_ratio,
    }
}

fn compute_diffs_recursive(
    old: &serde_json::Value,
    new: &serde_json::Value,
    path: String,
    diffs: &mut Vec<FieldDiff>,
) {
    match (old, new) {
        (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) => {
            // Check for removed keys
            for key in old_map.keys() {
                if !new_map.contains_key(key) {
                    diffs.push(FieldDiff {
                        path: format!("{}/{}", path, key),
                        old_value: Some(old_map[key].clone()),
                        new_value: None,
                    });
                }
            }
            // Check for added/changed keys
            for key in new_map.keys() {
                let child_path = format!("{}/{}", path, key);
                if let Some(old_val) = old_map.get(key) {
                    compute_diffs_recursive(old_val, &new_map[key], child_path, diffs);
                } else {
                    diffs.push(FieldDiff {
                        path: child_path,
                        old_value: None,
                        new_value: Some(new_map[key].clone()),
                    });
                }
            }
        }
        _ if old != new => {
            diffs.push(FieldDiff {
                path,
                old_value: Some(old.clone()),
                new_value: Some(new.clone()),
            });
        }
        _ => {} // Equal, no diff
    }
}

/// Apply diffs to a JSON value.
pub fn apply_diffs(state: &mut serde_json::Value, diffs: &[FieldDiff]) {
    for diff in diffs {
        match &diff.new_value {
            Some(val) => set_nested_value(state, &diff.path, val.clone()),
            None => remove_nested_value(state, &diff.path),
        }
    }
}

fn set_nested_value(state: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }

    let mut current = state;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Set the value
            if let Some(obj) = current.as_object_mut() {
                obj.insert(part.to_string(), value);
            }
            return;
        }
        // Navigate deeper
        if current.get(part).is_none() {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(part.to_string(), serde_json::json!({}));
            }
        }
        if let Some(next) = current.get_mut(part) {
            current = next;
        } else {
            return;
        }
    }
}

fn remove_nested_value(state: &mut serde_json::Value, path: &str) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }

    let mut current = state;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                obj.remove(*part);
            }
            return;
        }
        current = match current.get_mut(part) {
            Some(v) => v,
            None => return,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DeltaConfig ──────────────────────────────────────────────────────

    #[test]
    fn test_delta_config_default() {
        let config = DeltaConfig::default();
        assert_eq!(config.full_snapshot_interval, 10);
        assert_eq!(config.max_chain_length, 50);
        assert!(config.track_compression);
    }

    // ── Full snapshot path ───────────────────────────────────────────────

    #[test]
    fn test_first_checkpoint_always_full() {
        let mut store = DeltaStore::default();
        let state = serde_json::json!({"status": "running", "cpu": 0.5});
        let outcome = store.checkpoint("agent1", state);
        match outcome {
            CheckpointOutcome::FullSnapshot { seq, .. } => assert_eq!(seq, 0),
            _ => panic!("expected full snapshot"),
        }
    }

    #[test]
    fn test_interval_triggers_full_snapshot() {
        let config = DeltaConfig {
            full_snapshot_interval: 3,
            ..Default::default()
        };
        let mut store = DeltaStore::new(config);

        // seq 0 → full, seq 1 → delta, seq 2 → delta, seq 3 → full
        store.checkpoint("a", serde_json::json!({"v": 0}));
        store.checkpoint("a", serde_json::json!({"v": 1}));
        store.checkpoint("a", serde_json::json!({"v": 2}));
        let outcome = store.checkpoint("a", serde_json::json!({"v": 3}));
        match outcome {
            CheckpointOutcome::FullSnapshot { seq, .. } => assert_eq!(seq, 3),
            _ => panic!("expected full snapshot at seq 3"),
        }
    }

    // ── Delta path ───────────────────────────────────────────────────────

    #[test]
    fn test_delta_between_full_snapshots() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 1, "y": 2}));
        let outcome = store.checkpoint("a", serde_json::json!({"x": 1, "y": 3}));
        match outcome {
            CheckpointOutcome::Delta { diff_count, .. } => assert_eq!(diff_count, 1),
            _ => panic!("expected delta"),
        }
    }

    #[test]
    fn test_delta_compression_ratio() {
        let mut store = DeltaStore::default();
        let big_state = serde_json::json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5,
            "f": 6, "g": 7, "h": 8, "i": 9, "j": 10
        });
        store.checkpoint("a", big_state.clone());

        // Small change in big state → high compression
        let mut changed = big_state.clone();
        changed["a"] = serde_json::json!(99);
        let outcome = store.checkpoint("a", changed);
        match outcome {
            CheckpointOutcome::Delta {
                compression_ratio, ..
            } => {
                assert!(compression_ratio > 1.0, "should compress");
            }
            _ => panic!("expected delta"),
        }
    }

    // ── Reconstruction ───────────────────────────────────────────────────

    #[test]
    fn test_reconstruct_from_full_only() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 42}));
        let state = store.reconstruct("a", 0).unwrap();
        assert_eq!(state["x"], 42);
    }

    #[test]
    fn test_reconstruct_with_deltas() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 1, "y": 2}));
        store.checkpoint("a", serde_json::json!({"x": 1, "y": 3}));
        store.checkpoint("a", serde_json::json!({"x": 5, "y": 3}));

        let state = store.reconstruct("a", 2).unwrap();
        assert_eq!(state["x"], 5);
        assert_eq!(state["y"], 3);
    }

    #[test]
    fn test_reconstruct_nonexistent_agent() {
        let store = DeltaStore::default();
        assert!(store.reconstruct("nope", 0).is_none());
    }

    // ── Compaction ───────────────────────────────────────────────────────

    #[test]
    fn test_compact_merges_deltas() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 1, "y": 2}));
        store.checkpoint("a", serde_json::json!({"x": 1, "y": 3}));
        store.checkpoint("a", serde_json::json!({"x": 5, "y": 3}));

        let result = store.compact("a");
        match result {
            CompactionResult::Compacted {
                original_deltas,
                compacted_to,
            } => {
                assert_eq!(original_deltas, 2);
                assert_eq!(compacted_to, 1);
            }
            _ => panic!("expected compaction"),
        }

        // State should still be correct
        let state = store.reconstruct("a", 2).unwrap();
        assert_eq!(state["x"], 5);
        assert_eq!(state["y"], 3);
    }

    #[test]
    fn test_compact_single_delta_no_action() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 1}));
        store.checkpoint("a", serde_json::json!({"x": 2}));
        let result = store.compact("a");
        assert!(matches!(result, CompactionResult::NoAction));
    }

    // ── Stats ────────────────────────────────────────────────────────────

    #[test]
    fn test_stats_empty() {
        let store = DeltaStore::default();
        let stats = store.stats("nope");
        assert_eq!(stats.snapshot_count, 0);
        assert_eq!(stats.delta_count, 0);
    }

    #[test]
    fn test_stats_after_checkpoints() {
        let mut store = DeltaStore::default();
        store.checkpoint("a", serde_json::json!({"x": 1}));
        store.checkpoint("a", serde_json::json!({"x": 2}));
        let stats = store.stats("a");
        assert_eq!(stats.snapshot_count, 1);
        assert_eq!(stats.delta_count, 1);
    }

    // ── compute_delta ────────────────────────────────────────────────────

    #[test]
    fn test_compute_delta_no_changes() {
        let old = serde_json::json!({"x": 1});
        let new = serde_json::json!({"x": 1});
        let result = compute_delta(&old, &new);
        assert!(result.diffs.is_empty());
    }

    #[test]
    fn test_compute_delta_added_field() {
        let old = serde_json::json!({"x": 1});
        let new = serde_json::json!({"x": 1, "y": 2});
        let result = compute_delta(&old, &new);
        assert_eq!(result.diffs.len(), 1);
        assert_eq!(result.diffs[0].path, "/y");
        assert_eq!(result.diffs[0].new_value, Some(serde_json::json!(2)));
    }

    #[test]
    fn test_compute_delta_removed_field() {
        let old = serde_json::json!({"x": 1, "y": 2});
        let new = serde_json::json!({"x": 1});
        let result = compute_delta(&old, &new);
        assert_eq!(result.diffs.len(), 1);
        assert_eq!(result.diffs[0].path, "/y");
        assert_eq!(result.diffs[0].old_value, Some(serde_json::json!(2)));
        assert_eq!(result.diffs[0].new_value, None);
    }

    #[test]
    fn test_compute_delta_changed_value() {
        let old = serde_json::json!({"x": 1});
        let new = serde_json::json!({"x": 99});
        let result = compute_delta(&old, &new);
        assert_eq!(result.diffs.len(), 1);
        assert_eq!(result.diffs[0].old_value, Some(serde_json::json!(1)));
        assert_eq!(result.diffs[0].new_value, Some(serde_json::json!(99)));
    }

    #[test]
    fn test_compute_delta_nested_changes() {
        let old = serde_json::json!({"a": {"b": 1, "c": 2}});
        let new = serde_json::json!({"a": {"b": 1, "c": 3}});
        let result = compute_delta(&old, &new);
        assert_eq!(result.diffs.len(), 1);
        assert_eq!(result.diffs[0].path, "/a/c");
    }

    // ── apply_diffs ──────────────────────────────────────────────────────

    #[test]
    fn test_apply_diffs_set_value() {
        let mut state = serde_json::json!({"x": 1});
        let diffs = vec![FieldDiff {
            path: "/x".to_string(),
            old_value: Some(serde_json::json!(1)),
            new_value: Some(serde_json::json!(42)),
        }];
        apply_diffs(&mut state, &diffs);
        assert_eq!(state["x"], 42);
    }

    #[test]
    fn test_apply_diffs_add_value() {
        let mut state = serde_json::json!({"x": 1});
        let diffs = vec![FieldDiff {
            path: "/y".to_string(),
            old_value: None,
            new_value: Some(serde_json::json!(2)),
        }];
        apply_diffs(&mut state, &diffs);
        assert_eq!(state["y"], 2);
    }

    #[test]
    fn test_apply_diffs_remove_value() {
        let mut state = serde_json::json!({"x": 1, "y": 2});
        let diffs = vec![FieldDiff {
            path: "/y".to_string(),
            old_value: Some(serde_json::json!(2)),
            new_value: None,
        }];
        apply_diffs(&mut state, &diffs);
        assert!(state.get("y").is_none());
    }

    // ── StorageStats serde ───────────────────────────────────────────────

    #[test]
    fn test_storage_stats_serde() {
        let stats = StorageStats {
            agent_id: "a1".to_string(),
            snapshot_count: 5,
            delta_count: 20,
            total_snapshots_bytes: 1024,
            avg_compression_ratio: 3.5,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: StorageStats = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_id, "a1");
        assert_eq!(deserialized.avg_compression_ratio, 3.5);
    }
}
