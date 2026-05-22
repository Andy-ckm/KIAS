//! # TypedState — LangGraph-inspired typed state channels
//!
//! This module implements the core state channel abstraction inspired by
//! LangGraph's `TypedDict` + reducer pattern. Instead of an untyped
//! `HashMap<String, Value>`, each state field has a compile-time type and
//! a merge strategy (reducer).
//!
//! ## Design (参考 LangGraph)
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │  TypedState                                  │
//! │  ┌───────────┐  ┌───────────┐  ┌──────────┐ │
//! │  │ messages  │  │ context   │  │ result   │ │
//! │  │ Vec<Msg>  │  │ Map<K,V>  │  │ Option<T>│ │
//! │  │ Reducer:  │  │ Reducer:  │  │ Reducer: │ │
//! │  │  append   │  │  merge    │  │  replace │ │
//! │  └───────────┘  └───────────┘  └──────────┘ │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Reducer strategies
//!
//! - `Replace` — overwrites old value with new
//! - `Append`  — appends new items to a list
//! - `Merge`   — shallow-merges maps (new keys win)
//! - `Custom`  — user-supplied closure

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

// ───────────────────────── Reducer trait ─────────────────────────

/// Strategy for merging a new channel value into the existing one.
///
/// Every channel must carry a reducer so that concurrent or incremental
/// updates to the same channel are deterministic.
pub trait ChannelReducer<T>: Send + Sync + 'static {
    /// Merge `incoming` into `current`, returning the new value.
    fn reduce(&self, current: T, incoming: T) -> T;

    /// Human-readable name for logging/debugging.
    fn name(&self) -> &str;
}

// ───────────────────────── Built-in reducers ─────────────────────────

/// Overwrites the old value with the new one.
#[derive(Debug, Clone, Copy, Default)]
pub struct Replace;

impl<T: Send + Sync + 'static> ChannelReducer<T> for Replace {
    fn reduce(&self, _current: T, incoming: T) -> T {
        incoming
    }
    fn name(&self) -> &str {
        "replace"
    }
}

/// Appends incoming items to the current list.
#[derive(Debug, Clone, Copy, Default)]
pub struct Append;

impl<T: Send + Sync + 'static> ChannelReducer<Vec<T>> for Append {
    fn reduce(&self, mut current: Vec<T>, incoming: Vec<T>) -> Vec<T> {
        current.extend(incoming);
        current
    }
    fn name(&self) -> &str {
        "append"
    }
}

/// Shallow-merges two HashMaps — incoming keys overwrite existing ones.
#[derive(Debug, Clone, Copy, Default)]
pub struct Merge;

impl<K, V> ChannelReducer<HashMap<K, V>> for Merge
where
    K: Eq + std::hash::Hash + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    fn reduce(&self, mut current: HashMap<K, V>, incoming: HashMap<K, V>) -> HashMap<K, V> {
        current.extend(incoming);
        current
    }
    fn name(&self) -> &str {
        "merge"
    }
}

/// Keeps the first `Some` value, ignores subsequent updates.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeepFirst;

impl<T: Send + Sync + 'static> ChannelReducer<Option<T>> for KeepFirst {
    fn reduce(&self, current: Option<T>, incoming: Option<T>) -> Option<T> {
        if current.is_some() {
            current
        } else {
            incoming
        }
    }
    fn name(&self) -> &str {
        "keep_first"
    }
}

/// Numeric accumulator — adds incoming to current.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sum;

impl ChannelReducer<u64> for Sum {
    fn reduce(&self, current: u64, incoming: u64) -> u64 {
        current.saturating_add(incoming)
    }
    fn name(&self) -> &str {
        "sum"
    }
}

impl ChannelReducer<i64> for Sum {
    fn reduce(&self, current: i64, incoming: i64) -> i64 {
        current.saturating_add(incoming)
    }
    fn name(&self) -> &str {
        "sum"
    }
}

// ───────────────────── Type-erased channel ─────────────────────

/// A type-erased channel value with its reducer logic.
///
/// This is the internal storage unit of `TypedState`. Each channel holds:
/// - its current value (as `Box<dyn Any>`)
/// - a closure that applies the reducer
/// - the reducer's name for diagnostics
struct ErasedChannel {
    value: Box<dyn Any + Send + Sync>,
    #[allow(clippy::type_complexity)]
    reduce_fn: Box<
        dyn Fn(Box<dyn Any + Send + Sync>, Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync>
            + Send
            + Sync,
    >,
    reducer_name: String,
}

impl fmt::Debug for ErasedChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedChannel")
            .field("reducer", &self.reducer_name)
            .finish()
    }
}

impl ErasedChannel {
    fn new<T, R>(value: T, reducer: R) -> Self
    where
        T: Send + Sync + Clone + 'static,
        R: ChannelReducer<T> + Clone,
    {
        let reducer_name = reducer.name().to_string();
        let reducer_for_closure = reducer.clone();
        let reduce_fn = Arc::new(
            move |current: Box<dyn Any + Send + Sync>,
                  incoming: Box<dyn Any + Send + Sync>|
                  -> Box<dyn Any + Send + Sync> {
                let current = current
                    .downcast::<T>()
                    .expect("type mismatch in reduce (current)");
                let incoming = incoming
                    .downcast::<T>()
                    .expect("type mismatch in reduce (incoming)");
                let result = reducer_for_closure.reduce(*current, *incoming);
                Box::new(result)
            },
        );

        Self {
            value: Box::new(value),
            reduce_fn: Box::new(move |c, i| reduce_fn(c, i)),
            reducer_name,
        }
    }

    /// Apply a new value through the reducer.
    fn apply(&mut self, incoming: Box<dyn Any + Send + Sync>) {
        let current = std::mem::replace(&mut self.value, Box::new(()));
        self.value = (self.reduce_fn)(current, incoming);
    }
}

// ───────────────────── TypedState ─────────────────────

/// A collection of typed state channels.
///
/// Each channel is identified by a string key and carries its own type
/// and reducer. This mirrors LangGraph's `TypedDict` with reducers.
///
/// # Example
///
/// ```ignore
/// let mut state = TypedState::new();
/// state.register("messages", Vec::<String>::new(), Append);
/// state.register("count", 0u64, Sum);
/// state.register("result", None::<String>, Replace);
///
/// state.update("messages", vec!["hello".to_string()]);
/// state.update("count", 1u64);
/// ```
#[derive(Debug)]
pub struct TypedState {
    channels: HashMap<String, ErasedChannel>,
    /// Revision counter — incremented on every update.
    revision: u64,
}

impl TypedState {
    /// Create an empty typed state.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            revision: 0,
        }
    }

    /// Register a new channel with an initial value and reducer.
    ///
    /// Returns `Err` if the channel already exists.
    pub fn register<T, R>(
        &mut self,
        key: impl Into<String>,
        initial: T,
        reducer: R,
    ) -> Result<(), StateError>
    where
        T: Send + Sync + Clone + 'static,
        R: ChannelReducer<T> + Clone,
    {
        let key = key.into();
        if self.channels.contains_key(&key) {
            return Err(StateError::ChannelExists(key));
        }
        self.channels
            .insert(key, ErasedChannel::new(initial, reducer));
        Ok(())
    }

    /// Register or overwrite a channel (convenience for initial setup).
    pub fn register_or_overwrite<T, R>(&mut self, key: impl Into<String>, initial: T, reducer: R)
    where
        T: Send + Sync + Clone + 'static,
        R: ChannelReducer<T> + Clone,
    {
        let key = key.into();
        self.channels
            .insert(key, ErasedChannel::new(initial, reducer));
    }

    /// Update a channel's value by applying `incoming` through its reducer.
    ///
    /// Returns `Err` if the channel doesn't exist or the value has a type mismatch.
    pub fn update<T: Send + Sync + Clone + 'static>(
        &mut self,
        key: &str,
        incoming: T,
    ) -> Result<(), StateError> {
        let channel = self
            .channels
            .get_mut(key)
            .ok_or_else(|| StateError::ChannelNotFound(key.to_string()))?;

        channel.apply(Box::new(incoming));
        self.revision += 1;
        Ok(())
    }

    /// Read a channel's current value by downcasting.
    pub fn get<T: Clone + 'static>(&self, key: &str) -> Result<&T, StateError> {
        let channel = self
            .channels
            .get(key)
            .ok_or_else(|| StateError::ChannelNotFound(key.to_string()))?;

        channel
            .value
            .downcast_ref::<T>()
            .ok_or_else(|| StateError::TypeMismatch {
                channel: key.to_string(),
                expected: std::any::type_name::<T>().to_string(),
            })
    }

    /// Read a channel's current value, returning `None` if not found.
    pub fn try_get<T: Clone + 'static>(&self, key: &str) -> Option<&T> {
        self.channels
            .get(key)
            .and_then(|ch| ch.value.downcast_ref::<T>())
    }

    /// Check if a channel exists.
    pub fn has_channel(&self, key: &str) -> bool {
        self.channels.contains_key(key)
    }

    /// List all channel names.
    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.keys().map(|s| s.as_str()).collect()
    }

    /// Current revision number (incremented on every update).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Number of channels.
    pub fn len(&self) -> usize {
        self.channels.len()
    }

    /// Whether the state has no channels.
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Snapshot: serialize all channels to a JSON-compatible HashMap.
    ///
    /// This is best-effort — channels whose values aren't JSON-serializable
    /// will produce `Value::Null`.
    pub fn snapshot(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        for (key, channel) in &self.channels {
            let value = channel
                .value
                .downcast_ref::<serde_json::Value>()
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            map.insert(key.clone(), value);
        }
        map
    }
}

impl Default for TypedState {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────── Errors ─────────────────────

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("channel '{0}' already exists")]
    ChannelExists(String),

    #[error("channel '{0}' not found")]
    ChannelNotFound(String),

    #[error("type mismatch on channel '{channel}': expected {expected}")]
    TypeMismatch { channel: String, expected: String },
}

// ───────────────────── StateDiff ─────────────────────

/// Records which channels changed between two revisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub from_revision: u64,
    pub to_revision: u64,
    pub changed_channels: Vec<String>,
}

// ───────────────────── Streaming events ─────────────────────

/// Events emitted during workflow execution for real-time observability.
///
/// This enables streaming execution semantics — consumers can subscribe
/// to a channel and receive events as the workflow progresses, similar
/// to LangGraph's `.stream()` API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingEvent {
    /// Workflow execution started.
    WorkflowStarted {
        workflow_id: String,
        entry_node: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// A node is about to execute.
    NodeStart {
        workflow_id: String,
        node_id: String,
        node_type: String,
        revision: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// A node finished executing.
    NodeComplete {
        workflow_id: String,
        node_id: String,
        success: bool,
        duration_ms: u64,
        revision: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// A state channel was updated.
    ChannelUpdate {
        workflow_id: String,
        channel: String,
        reducer: String,
        revision: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// An edge was traversed (state transition).
    EdgeTraversed {
        workflow_id: String,
        from: String,
        to: String,
        condition: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Workflow execution completed.
    WorkflowComplete {
        workflow_id: String,
        status: String,
        total_steps: u64,
        total_duration_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Workflow execution failed.
    WorkflowFailed {
        workflow_id: String,
        error: String,
        failed_node: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Human intervention requested.
    HumanInterrupt {
        workflow_id: String,
        node_id: String,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

impl StreamingEvent {
    /// Get the workflow_id associated with this event.
    pub fn workflow_id(&self) -> &str {
        match self {
            Self::WorkflowStarted { workflow_id, .. }
            | Self::NodeStart { workflow_id, .. }
            | Self::NodeComplete { workflow_id, .. }
            | Self::ChannelUpdate { workflow_id, .. }
            | Self::EdgeTraversed { workflow_id, .. }
            | Self::WorkflowComplete { workflow_id, .. }
            | Self::WorkflowFailed { workflow_id, .. }
            | Self::HumanInterrupt { workflow_id, .. } => workflow_id,
        }
    }

    /// Get the event timestamp.
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::WorkflowStarted { timestamp, .. }
            | Self::NodeStart { timestamp, .. }
            | Self::NodeComplete { timestamp, .. }
            | Self::ChannelUpdate { timestamp, .. }
            | Self::EdgeTraversed { timestamp, .. }
            | Self::WorkflowComplete { timestamp, .. }
            | Self::WorkflowFailed { timestamp, .. }
            | Self::HumanInterrupt { timestamp, .. } => *timestamp,
        }
    }
}

/// A thread-safe event sink for collecting streaming events.
///
/// Producers push events, consumers can take them via `take_events()`
/// or drain them with `drain_events()`.
#[derive(Debug, Clone, Default)]
pub struct EventSink {
    events: Arc<tokio::sync::Mutex<Vec<StreamingEvent>>>,
}

impl EventSink {
    pub fn new() -> Self {
        Self {
            events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Push a streaming event.
    pub async fn emit(&self, event: StreamingEvent) {
        let mut events = self.events.lock().await;
        events.push(event);
    }

    /// Take all accumulated events and clear the buffer.
    pub async fn take_events(&self) -> Vec<StreamingEvent> {
        let mut events = self.events.lock().await;
        std::mem::take(&mut *events)
    }

    /// Read all events without clearing.
    pub async fn peek_events(&self) -> Vec<StreamingEvent> {
        let events = self.events.lock().await;
        events.clone()
    }

    /// Number of events buffered.
    pub async fn len(&self) -> usize {
        let events = self.events.lock().await;
        events.len()
    }

    pub async fn is_empty(&self) -> bool {
        let events = self.events.lock().await;
        events.is_empty()
    }
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_reducer() {
        let r = Replace;
        assert_eq!(r.reduce(1, 2), 2);
        assert_eq!(r.reduce("old", "new"), "new");
    }

    #[test]
    fn test_append_reducer() {
        let r = Append;
        let a = vec![1, 2];
        let b = vec![3, 4];
        assert_eq!(r.reduce(a, b), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_append_reducer_empty() {
        let r = Append;
        assert_eq!(r.reduce(vec![1], vec![]), vec![1]);
        assert_eq!(r.reduce(vec![], vec![2]), vec![2]);
    }

    #[test]
    fn test_merge_reducer() {
        let r = Merge;
        let mut a = HashMap::new();
        a.insert("x", 1);
        let mut b = HashMap::new();
        b.insert("y", 2);
        b.insert("x", 99); // overwrite
        let result = r.reduce(a, b);
        assert_eq!(result.get("x"), Some(&99));
        assert_eq!(result.get("y"), Some(&2));
    }

    #[test]
    fn test_keep_first_reducer() {
        let r = KeepFirst;
        assert_eq!(r.reduce(None, Some(1)), Some(1));
        assert_eq!(r.reduce(Some(1), Some(99)), Some(1));
        assert_eq!(r.reduce(Some(1), None), Some(1));
    }

    #[test]
    fn test_sum_reducer_u64() {
        let r = Sum;
        assert_eq!(r.reduce(10u64, 20u64), 30);
        assert_eq!(r.reduce(0u64, 0u64), 0);
    }

    #[test]
    fn test_sum_reducer_i64() {
        let r = Sum;
        assert_eq!(r.reduce(-5i64, 3i64), -2);
    }

    #[test]
    fn test_typed_state_register_and_get() {
        let mut state = TypedState::new();
        state.register("count", 0u64, Sum).unwrap();
        state
            .register("messages", Vec::<String>::new(), Append)
            .unwrap();
        state
            .register("name", "initial".to_string(), Replace)
            .unwrap();

        assert_eq!(*state.get::<u64>("count").unwrap(), 0);
        assert!(state.get::<Vec<String>>("messages").unwrap().is_empty());
        assert_eq!(state.get::<String>("name").unwrap(), "initial");
    }

    #[test]
    fn test_typed_state_update_through_reducer() {
        let mut state = TypedState::new();
        state.register("count", 0u64, Sum).unwrap();

        state.update("count", 5u64).unwrap();
        state.update("count", 3u64).unwrap();
        assert_eq!(*state.get::<u64>("count").unwrap(), 8);
        assert_eq!(state.revision(), 2);
    }

    #[test]
    fn test_typed_state_append_messages() {
        let mut state = TypedState::new();
        state
            .register("messages", Vec::<String>::new(), Append)
            .unwrap();

        state.update("messages", vec!["hello".to_string()]).unwrap();
        state.update("messages", vec!["world".to_string()]).unwrap();

        let msgs = state.get::<Vec<String>>("messages").unwrap();
        assert_eq!(msgs, &vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn test_typed_state_register_duplicate_errors() {
        let mut state = TypedState::new();
        state.register("x", 1u64, Replace).unwrap();
        let err = state.register("x", 2u64, Replace);
        assert!(err.is_err());
    }

    #[test]
    fn test_typed_state_update_nonexistent_errors() {
        let mut state = TypedState::new();
        let err = state.update::<u64>("missing", 42);
        assert!(err.is_err());
    }

    #[test]
    fn test_typed_state_try_get() {
        let mut state = TypedState::new();
        state.register("x", 42u64, Replace).unwrap();
        assert_eq!(state.try_get::<u64>("x"), Some(&42));
        assert_eq!(state.try_get::<u64>("nope"), None);
    }

    #[test]
    fn test_typed_state_has_channel() {
        let mut state = TypedState::new();
        assert!(!state.has_channel("x"));
        state.register("x", 0u64, Sum).unwrap();
        assert!(state.has_channel("x"));
    }

    #[test]
    fn test_typed_state_channel_names() {
        let mut state = TypedState::new();
        state.register("alpha", 0u64, Sum).unwrap();
        state.register("beta", "".to_string(), Replace).unwrap();
        let mut names: Vec<_> = state.channel_names();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_typed_state_default_is_empty() {
        let state = TypedState::default();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn test_register_or_overwrite() {
        let mut state = TypedState::new();
        state.register("x", 1u64, Replace).unwrap();
        state.register_or_overwrite("x", 99u64, Replace);
        assert_eq!(*state.get::<u64>("x").unwrap(), 99);
    }

    #[test]
    fn test_sum_saturating() {
        let r = Sum;
        assert_eq!(r.reduce(u64::MAX, 1), u64::MAX);
    }

    #[tokio::test]
    async fn test_event_sink_emit_and_take() {
        let sink = EventSink::new();
        assert!(sink.is_empty().await);

        sink.emit(StreamingEvent::WorkflowStarted {
            workflow_id: "wf-1".into(),
            entry_node: "start".into(),
            timestamp: chrono::Utc::now(),
        })
        .await;

        assert_eq!(sink.len().await, 1);
        let events = sink.take_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].workflow_id(), "wf-1");

        // Should be empty after take
        assert!(sink.is_empty().await);
    }

    #[tokio::test]
    async fn test_event_sink_peek_does_not_consume() {
        let sink = EventSink::new();
        sink.emit(StreamingEvent::WorkflowComplete {
            workflow_id: "wf-1".into(),
            status: "completed".into(),
            total_steps: 5,
            total_duration_ms: 100,
            timestamp: chrono::Utc::now(),
        })
        .await;

        let peeked = sink.peek_events().await;
        assert_eq!(peeked.len(), 1);

        // Still there
        let taken = sink.take_events().await;
        assert_eq!(taken.len(), 1);
    }

    #[test]
    fn test_streaming_event_workflow_id() {
        let event = StreamingEvent::WorkflowFailed {
            workflow_id: "wf-42".into(),
            error: "timeout".into(),
            failed_node: Some("node-3".into()),
            timestamp: chrono::Utc::now(),
        };
        assert_eq!(event.workflow_id(), "wf-42");
    }
}
