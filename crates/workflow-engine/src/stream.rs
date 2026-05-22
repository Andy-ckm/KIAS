//! Streaming transformer infrastructure for workflow event processing.
//!
//! Provides `StreamEvent` variants, a `StreamTransformer` trait, broadcast-based
//! event distribution, and multiple projection modes (custom, updates,
//! checkpoints, debug, tasks).

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// StreamEvent
// ---------------------------------------------------------------------------

/// Events emitted during workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StreamEvent {
    /// A node has started execution.
    NodeStarted {
        node_id: String,
        timestamp: String,
    },

    /// A token (partial output) has been emitted by a node.
    TokenEmitted {
        node_id: String,
        token: String,
        timestamp: String,
    },

    /// A node has completed execution.
    NodeCompleted {
        node_id: String,
        output: String,
        timestamp: String,
    },

    /// A node encountered an error.
    NodeError {
        node_id: String,
        error: String,
        timestamp: String,
    },

    /// A checkpoint has been saved.
    CheckpointSaved {
        checkpoint_id: String,
        node_id: String,
        timestamp: String,
    },
}

impl StreamEvent {
    /// Shorthand constructor for `NodeStarted`.
    pub fn node_started(node_id: impl Into<String>) -> Self {
        Self::NodeStarted {
            node_id: node_id.into(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Shorthand constructor for `TokenEmitted`.
    pub fn token_emitted(node_id: impl Into<String>, token: impl Into<String>) -> Self {
        Self::TokenEmitted {
            node_id: node_id.into(),
            token: token.into(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Shorthand constructor for `NodeCompleted`.
    pub fn node_completed(node_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self::NodeCompleted {
            node_id: node_id.into(),
            output: output.into(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Shorthand constructor for `NodeError`.
    pub fn node_error(node_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::NodeError {
            node_id: node_id.into(),
            error: error.into(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Shorthand constructor for `CheckpointSaved`.
    pub fn checkpoint_saved(
        checkpoint_id: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Self {
        Self::CheckpointSaved {
            checkpoint_id: checkpoint_id.into(),
            node_id: node_id.into(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Returns the node id associated with this event (if any).
    pub fn node_id(&self) -> &str {
        match self {
            Self::NodeStarted { node_id, .. }
            | Self::TokenEmitted { node_id, .. }
            | Self::NodeCompleted { node_id, .. }
            | Self::NodeError { node_id, .. }
            | Self::CheckpointSaved { node_id, .. } => node_id,
        }
    }

    /// Returns the variant name as a static string.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NodeStarted { .. } => "NodeStarted",
            Self::TokenEmitted { .. } => "TokenEmitted",
            Self::NodeCompleted { .. } => "NodeCompleted",
            Self::NodeError { .. } => "NodeError",
            Self::CheckpointSaved { .. } => "CheckpointSaved",
        }
    }
}

impl fmt::Display for StreamEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeStarted { node_id, timestamp } => {
                write!(f, "[{timestamp}] NodeStarted: {node_id}")
            }
            Self::TokenEmitted {
                node_id,
                token,
                timestamp,
            } => {
                write!(f, "[{timestamp}] TokenEmitted({node_id}): {token}")
            }
            Self::NodeCompleted {
                node_id,
                output,
                timestamp,
            } => {
                write!(f, "[{timestamp}] NodeCompleted({node_id}): {output}")
            }
            Self::NodeError {
                node_id,
                error,
                timestamp,
            } => {
                write!(f, "[{timestamp}] NodeError({node_id}): {error}")
            }
            Self::CheckpointSaved {
                checkpoint_id,
                node_id,
                timestamp,
            } => {
                write!(
                    f,
                    "[{timestamp}] CheckpointSaved({checkpoint_id}) at node {node_id}"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// StreamTransformer trait
// ---------------------------------------------------------------------------

/// A transformer receives each `StreamEvent` and optionally emits a
/// (possibly modified) event. Returning `None` suppresses the event.
#[async_trait]
pub trait StreamTransformer: Send + Sync {
    fn transform(&self, event: StreamEvent) -> Option<StreamEvent>;
}

// ---------------------------------------------------------------------------
// Projection modes (built-in transformers)
// ---------------------------------------------------------------------------

/// Projection mode determines which events are forwarded to subscribers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionMode {
    /// Forward all events unchanged.
    Custom,
    /// Only `NodeStarted`, `NodeCompleted`, and `NodeError` (skip tokens).
    Updates,
    /// Only `CheckpointSaved`.
    Checkpoints,
    /// All events, with added debug prefix.
    Debug,
    /// Only `NodeStarted` and `NodeCompleted` (task lifecycle).
    Tasks,
}

impl fmt::Display for ProjectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom => write!(f, "custom"),
            Self::Updates => write!(f, "updates"),
            Self::Checkpoints => write!(f, "checkpoints"),
            Self::Debug => write!(f, "debug"),
            Self::Tasks => write!(f, "tasks"),
        }
    }
}

impl ProjectionMode {
    /// Parse a projection mode from a string.
    pub fn from_str_mode(s: &str) -> Option<Self> {
        match s {
            "custom" => Some(Self::Custom),
            "updates" => Some(Self::Updates),
            "checkpoints" => Some(Self::Checkpoints),
            "debug" => Some(Self::Debug),
            "tasks" => Some(Self::Tasks),
            _ => None,
        }
    }
}

/// Built-in transformer that applies a projection mode.
pub struct ProjectionTransformer {
    mode: ProjectionMode,
}

impl ProjectionTransformer {
    pub fn new(mode: ProjectionMode) -> Self {
        Self { mode }
    }
}

#[async_trait]
impl StreamTransformer for ProjectionTransformer {
    fn transform(&self, event: StreamEvent) -> Option<StreamEvent> {
        match self.mode {
            ProjectionMode::Custom => Some(event),
            ProjectionMode::Updates => match event {
                StreamEvent::NodeStarted { .. }
                | StreamEvent::NodeCompleted { .. }
                | StreamEvent::NodeError { .. } => Some(event),
                _ => None,
            },
            ProjectionMode::Checkpoints => match event {
                StreamEvent::CheckpointSaved { .. } => Some(event),
                _ => None,
            },
            ProjectionMode::Debug => {
                // Wrap the event kind into a debug-flavoured token event.
                let kind = event.kind().to_string();
                Some(StreamEvent::TokenEmitted {
                    node_id: event.node_id().to_string(),
                    token: format!("[DEBUG:{kind}] {event}"),
                    timestamp: Utc::now().to_rfc3339(),
                })
            }
            ProjectionMode::Tasks => match event {
                StreamEvent::NodeStarted { .. } | StreamEvent::NodeCompleted { .. } => {
                    Some(event)
                }
                _ => None,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// EventBroadcaster
// ---------------------------------------------------------------------------

/// Broadcasts `StreamEvent`s to subscribers using a `tokio::sync::broadcast`
/// channel. Supports attaching `StreamTransformer` instances that are applied
/// to every event before it is sent.
pub struct EventBroadcaster {
    tx: broadcast::Sender<StreamEvent>,
    transformers: Vec<Arc<dyn StreamTransformer>>,
    dropped_count: Arc<AtomicU64>,
}

impl EventBroadcaster {
    /// Create a new broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            transformers: Vec::new(),
            dropped_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a broadcaster pre-configured for a projection mode.
    pub fn with_projection(capacity: usize, mode: ProjectionMode) -> Self {
        let mut b = Self::new(capacity);
        b.add_transformer(Arc::new(ProjectionTransformer::new(mode)));
        b
    }

    /// Register a transformer. Transformers are applied in registration order.
    pub fn add_transformer(&mut self, transformer: Arc<dyn StreamTransformer>) {
        self.transformers.push(transformer);
    }

    /// Publish an event, applying all registered transformers in order.
    /// Returns the number of receivers that got the event, or 0 / an error
    /// if there are no active subscribers.
    pub fn publish(&self, event: StreamEvent) -> Result<usize, broadcast::error::SendError<StreamEvent>> {
        let mut current = Some(event);
        for t in &self.transformers {
            current = match current {
                Some(e) => t.transform(e),
                None => return Ok(0),
            };
        }
        if let Some(final_event) = current {
            self.tx.send(final_event)
        } else {
            Ok(0)
        }
    }

    /// Obtain a new subscriber receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    /// Returns the number of active subscribers.
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Returns a clone of the dropped-message counter.
    pub fn dropped_count(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.dropped_count)
    }
}

impl Clone for EventBroadcaster {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            // Transformers are shared behind Arc; clone the vec of pointers.
            transformers: self.transformers.clone(),
            dropped_count: Arc::clone(&self.dropped_count),
        }
    }
}

impl fmt::Debug for EventBroadcaster {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBroadcaster")
            .field("transformers", &self.transformers.len())
            .field("receivers", &self.tx.receiver_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Helper: subscribe with lag handling
// ---------------------------------------------------------------------------

/// Receive events from a receiver, reporting lagged messages via the dropped
/// counter. Returns `None` if the channel is closed.
pub async fn recv_event(
    rx: &mut broadcast::Receiver<StreamEvent>,
    dropped: &AtomicU64,
) -> Option<StreamEvent> {
    match rx.recv().await {
        Ok(event) => Some(event),
        Err(broadcast::error::RecvError::Lagged(n)) => {
            dropped.fetch_add(n, Ordering::Relaxed);
            None
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    // -- helpers --
    fn dummy_node_started() -> StreamEvent {
        StreamEvent::node_started("node-1")
    }

    fn dummy_token() -> StreamEvent {
        StreamEvent::token_emitted("node-1", "hello")
    }

    fn dummy_completed() -> StreamEvent {
        StreamEvent::node_completed("node-1", "done")
    }

    fn dummy_error() -> StreamEvent {
        StreamEvent::node_error("node-1", "boom")
    }

    fn dummy_checkpoint() -> StreamEvent {
        StreamEvent::checkpoint_saved("cp-1", "node-1")
    }

    // -- 1. basic broadcast: subscriber receives published events --
    #[tokio::test]
    async fn test_broadcast_basic() {
        let broadcaster = EventBroadcaster::new(64);
        let mut rx = broadcaster.subscribe();
        let dropped = broadcaster.dropped_count();

        let ev = dummy_node_started();
        broadcaster.publish(ev.clone()).unwrap();

        let received = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(received.kind(), "NodeStarted");
        assert_eq!(received.node_id(), "node-1");
    }

    // -- 2. projection: updates mode drops tokens --
    #[tokio::test]
    async fn test_projection_updates_filters_tokens() {
        let b = EventBroadcaster::with_projection(64, ProjectionMode::Updates);
        let mut rx = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_node_started()).unwrap();
        b.publish(dummy_token()).unwrap(); // should be filtered
        b.publish(dummy_completed()).unwrap();

        let ev1 = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev1.kind(), "NodeStarted");

        let ev2 = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev2.kind(), "NodeCompleted");

        // No more events
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), recv_event(&mut rx, &dropped))
            .await
            .is_err());
    }

    // -- 3. projection: checkpoints mode --
    #[tokio::test]
    async fn test_projection_checkpoints() {
        let b = EventBroadcaster::with_projection(64, ProjectionMode::Checkpoints);
        let mut rx = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_node_started()).unwrap(); // filtered
        b.publish(dummy_checkpoint()).unwrap();
        b.publish(dummy_error()).unwrap(); // filtered

        let ev = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev.kind(), "CheckpointSaved");
    }

    // -- 4. projection: tasks mode (start + complete only) --
    #[tokio::test]
    async fn test_projection_tasks() {
        let b = EventBroadcaster::with_projection(64, ProjectionMode::Tasks);
        let mut rx = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_node_started()).unwrap();
        b.publish(dummy_token()).unwrap(); // filtered
        b.publish(dummy_error()).unwrap(); // filtered
        b.publish(dummy_completed()).unwrap();

        let ev1 = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev1.kind(), "NodeStarted");
        let ev2 = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev2.kind(), "NodeCompleted");
    }

    // -- 5. projection: debug mode wraps events into TokenEmitted --
    #[tokio::test]
    async fn test_projection_debug_wraps() {
        let b = EventBroadcaster::with_projection(64, ProjectionMode::Debug);
        let mut rx = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_node_started()).unwrap();

        let ev = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev.kind(), "TokenEmitted");
        if let StreamEvent::TokenEmitted { token, .. } = &ev {
            assert!(token.contains("[DEBUG:NodeStarted]"));
        } else {
            panic!("expected TokenEmitted");
        }
    }

    // -- 6. multiple subscribers receive the same event --
    #[tokio::test]
    async fn test_multiple_subscribers() {
        let b = EventBroadcaster::new(64);
        let mut rx1 = b.subscribe();
        let mut rx2 = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_completed()).unwrap();

        let ev1 = recv_event(&mut rx1, &dropped).await.unwrap();
        let ev2 = recv_event(&mut rx2, &dropped).await.unwrap();
        assert_eq!(ev1.kind(), "NodeCompleted");
        assert_eq!(ev2.kind(), "NodeCompleted");
        assert_eq!(ev1, ev2);
    }

    // -- 8. transformer suppresses events (returns None) --
    struct SuppressTokens;

    #[async_trait]
    impl StreamTransformer for SuppressTokens {
        fn transform(&self, event: StreamEvent) -> Option<StreamEvent> {
            match event {
                StreamEvent::TokenEmitted { .. } => None,
                other => Some(other),
            }
        }
    }

    #[tokio::test]
    async fn test_custom_transformer_suppresses() {
        let mut b = EventBroadcaster::new(64);
        b.add_transformer(Arc::new(SuppressTokens));
        let mut rx = b.subscribe();
        let dropped = b.dropped_count();

        b.publish(dummy_token()).unwrap(); // suppressed
        b.publish(dummy_error()).unwrap(); // passes

        let ev = recv_event(&mut rx, &dropped).await.unwrap();
        assert_eq!(ev.kind(), "NodeError");
    }

    // -- 9. Display trait --
    #[test]
    fn test_display_format() {
        let ev = StreamEvent::NodeStarted {
            node_id: "n1".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
        };
        let s = format!("{ev}");
        assert!(s.contains("NodeStarted"));
        assert!(s.contains("n1"));
    }

    // -- 10. ProjectionMode string round-trip --
    #[test]
    fn test_projection_mode_roundtrip() {
        for mode in [
            ProjectionMode::Custom,
            ProjectionMode::Updates,
            ProjectionMode::Checkpoints,
            ProjectionMode::Debug,
            ProjectionMode::Tasks,
        ] {
            let s = mode.to_string();
            let parsed = ProjectionMode::from_str_mode(&s);
            assert_eq!(parsed, Some(mode));
        }
        assert!(ProjectionMode::from_str_mode("bogus").is_none());
    }
}
