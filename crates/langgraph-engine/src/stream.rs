//! Streaming execution events via broadcast channels.
//!
//! Provides `ExecutionStream` for multi-consumer event streaming,
//! enabling real-time monitoring of graph execution progress.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted during graph execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
    /// A node started executing.
    NodeStart {
        node: String,
        step: usize,
        timestamp_ms: i64,
    },
    /// A node completed successfully.
    NodeComplete {
        node: String,
        step: usize,
        duration_ms: u64,
        timestamp_ms: i64,
    },
    /// A node failed with an error.
    NodeError {
        node: String,
        step: usize,
        error: String,
        timestamp_ms: i64,
    },
    /// A conditional edge was evaluated and taken.
    EdgeTaken {
        from: String,
        to: String,
        is_conditional: bool,
    },
    /// Execution was interrupted (human-in-the-loop).
    Interrupted {
        node: String,
        reason: String,
        checkpoint_id: Option<String>,
    },
    /// Execution completed successfully.
    Completed {
        total_steps: usize,
        total_duration_ms: u64,
    },
    /// Execution failed with an error.
    Failed { node: String, error: String },
    /// A checkpoint was saved.
    CheckpointSaved { checkpoint_id: String, node: String },
    /// Execution was resumed from a checkpoint.
    Resumed { checkpoint_id: String, node: String },
    /// A parallel branch started (fan-out).
    BranchStart {
        source: String,
        branches: Vec<String>,
    },
    /// All parallel branches completed (fan-in).
    BranchComplete {
        source: String,
        branches: Vec<String>,
    },
}

impl ExecutionEvent {
    /// Get a human-readable summary of this event.
    pub fn summary(&self) -> String {
        match self {
            Self::NodeStart { node, step, .. } => format!("[step {}] ▶ {}", step, node),
            Self::NodeComplete {
                node, duration_ms, ..
            } => format!("✔ {} ({}ms)", node, duration_ms),
            Self::NodeError { node, error, .. } => format!("✘ {}: {}", node, error),
            Self::EdgeTaken { from, to, .. } => format!("→ {} → {}", from, to),
            Self::Interrupted { node, reason, .. } => {
                format!("⏸ {} — {}", node, reason)
            }
            Self::Completed {
                total_steps,
                total_duration_ms,
            } => format!("✅ Done in {} steps ({}ms)", total_steps, total_duration_ms),
            Self::Failed { node, error } => format!("❌ Failed at {}: {}", node, error),
            Self::CheckpointSaved {
                checkpoint_id,
                node,
                ..
            } => {
                let short_id = &checkpoint_id[..8.min(checkpoint_id.len())];
                format!("💾 Checkpoint {} @ {}", short_id, node)
            }
            Self::Resumed {
                checkpoint_id,
                node,
                ..
            } => {
                let short_id = &checkpoint_id[..8.min(checkpoint_id.len())];
                format!("↻ Resumed from {} @ {}", short_id, node)
            }
            Self::BranchStart { source, branches } => {
                format!("⑂ Fan-out from {} → [{}]", source, branches.join(", "))
            }
            Self::BranchComplete { source, branches } => {
                format!("⚑ Fan-in at {} ← [{}]", source, branches.join(", "))
            }
        }
    }
}

/// Multi-consumer execution event stream.
///
/// Uses `tokio::sync::broadcast` for efficient fan-out to multiple subscribers.
pub struct ExecutionStream {
    tx: broadcast::Sender<ExecutionEvent>,
}

impl Default for ExecutionStream {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionStream {
    /// Create a new stream with default capacity (256 events).
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Create a new stream with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Emit an event to all subscribers. Ignores send errors (no subscribers).
    pub fn emit(&self, event: ExecutionEvent) {
        // Best-effort — ignore if no receivers
        let _ = self.tx.send(event);
    }

    /// Subscribe to the event stream. Returns a receiver that gets all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.tx.subscribe()
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Collect events into a vector for testing/debugging.
pub struct EventCollector {
    events: std::sync::Mutex<Vec<ExecutionEvent>>,
}

impl Default for EventCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EventCollector {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Record an event.
    pub fn push(&self, event: ExecutionEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    /// Get all collected events.
    pub fn events(&self) -> Vec<ExecutionEvent> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Get the number of collected events.
    pub fn len(&self) -> usize {
        self.events.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// Check if no events have been collected.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get summaries of all collected events.
    pub fn summaries(&self) -> Vec<String> {
        self.events().iter().map(|e| e.summary()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_emit_and_subscribe() {
        let stream = ExecutionStream::new();
        let mut rx = stream.subscribe();

        stream.emit(ExecutionEvent::NodeStart {
            node: "a".to_string(),
            step: 0,
            timestamp_ms: 100,
        });

        let event = rx.try_recv().unwrap();
        match event {
            ExecutionEvent::NodeStart { node, step, .. } => {
                assert_eq!(node, "a");
                assert_eq!(step, 0);
            }
            _ => panic!("Expected NodeStart"),
        }
    }

    #[test]
    fn test_stream_with_capacity() {
        let stream = ExecutionStream::with_capacity(64);
        let mut rx = stream.subscribe();

        stream.emit(ExecutionEvent::Completed {
            total_steps: 5,
            total_duration_ms: 1000,
        });

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, ExecutionEvent::Completed { .. }));
    }

    #[test]
    fn test_stream_subscriber_count() {
        let stream = ExecutionStream::new();
        assert_eq!(stream.subscriber_count(), 0);

        let _rx1 = stream.subscribe();
        assert_eq!(stream.subscriber_count(), 1);

        let _rx2 = stream.subscribe();
        assert_eq!(stream.subscriber_count(), 2);

        drop(_rx1);
        assert_eq!(stream.subscriber_count(), 1);
    }

    #[test]
    fn test_stream_emit_no_subscribers() {
        let stream = ExecutionStream::new();
        // Should not panic even with no subscribers
        stream.emit(ExecutionEvent::Failed {
            node: "x".to_string(),
            error: "test".to_string(),
        });
    }

    #[test]
    fn test_default_stream() {
        let stream = ExecutionStream::default();
        assert_eq!(stream.subscriber_count(), 0);
    }

    #[test]
    fn test_event_collector() {
        let collector = EventCollector::new();
        assert!(collector.is_empty());
        assert_eq!(collector.len(), 0);

        collector.push(ExecutionEvent::NodeStart {
            node: "a".to_string(),
            step: 0,
            timestamp_ms: 100,
        });
        assert!(!collector.is_empty());
        assert_eq!(collector.len(), 1);

        collector.push(ExecutionEvent::Completed {
            total_steps: 1,
            total_duration_ms: 50,
        });
        assert_eq!(collector.len(), 2);
    }

    #[test]
    fn test_event_collector_events() {
        let collector = EventCollector::new();
        collector.push(ExecutionEvent::NodeStart {
            node: "a".to_string(),
            step: 0,
            timestamp_ms: 100,
        });

        let events = collector.events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_event_collector_summaries() {
        let collector = EventCollector::new();
        collector.push(ExecutionEvent::NodeStart {
            node: "process".to_string(),
            step: 2,
            timestamp_ms: 100,
        });
        collector.push(ExecutionEvent::Completed {
            total_steps: 5,
            total_duration_ms: 500,
        });

        let summaries = collector.summaries();
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].contains("process"));
        assert!(summaries[1].contains("Done"));
    }

    #[test]
    fn test_default_collector() {
        let collector = EventCollector::default();
        assert!(collector.is_empty());
    }

    #[test]
    fn test_event_summary_all_variants() {
        let events = vec![
            ExecutionEvent::NodeStart {
                node: "a".into(),
                step: 0,
                timestamp_ms: 1,
            },
            ExecutionEvent::NodeComplete {
                node: "a".into(),
                step: 0,
                duration_ms: 10,
                timestamp_ms: 2,
            },
            ExecutionEvent::NodeError {
                node: "a".into(),
                step: 0,
                error: "err".into(),
                timestamp_ms: 3,
            },
            ExecutionEvent::EdgeTaken {
                from: "a".into(),
                to: "b".into(),
                is_conditional: false,
            },
            ExecutionEvent::Interrupted {
                node: "a".into(),
                reason: "human".into(),
                checkpoint_id: None,
            },
            ExecutionEvent::Completed {
                total_steps: 3,
                total_duration_ms: 100,
            },
            ExecutionEvent::Failed {
                node: "a".into(),
                error: "boom".into(),
            },
            ExecutionEvent::CheckpointSaved {
                checkpoint_id: "12345678-abcd".into(),
                node: "a".into(),
            },
            ExecutionEvent::Resumed {
                checkpoint_id: "12345678-abcd".into(),
                node: "a".into(),
            },
            ExecutionEvent::BranchStart {
                source: "a".into(),
                branches: vec!["b".into(), "c".into()],
            },
            ExecutionEvent::BranchComplete {
                source: "a".into(),
                branches: vec!["b".into(), "c".into()],
            },
        ];

        for event in &events {
            let summary = event.summary();
            assert!(
                !summary.is_empty(),
                "Summary should not be empty for {:?}",
                event
            );
        }
    }
}
