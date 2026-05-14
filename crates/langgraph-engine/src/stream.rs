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
    Failed {
        node: String,
        error: String,
    },
    /// A checkpoint was saved.
    CheckpointSaved {
        checkpoint_id: String,
        node: String,
    },
    /// Execution was resumed from a checkpoint.
    Resumed {
        checkpoint_id: String,
        node: String,
    },
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
                checkpoint_id, node, ..
            } => {
                let short_id = &checkpoint_id[..8.min(checkpoint_id.len())];
                format!("💾 Checkpoint {} @ {}", short_id, node)
            }
            Self::Resumed {
                checkpoint_id, node, ..
            } => {
                let short_id = &checkpoint_id[..8.min(checkpoint_id.len())];
                format!("↻ Resumed from {} @ {}", short_id, node)
            }
            Self::BranchStart { source, branches } => {
                format!("⑂ Fan-out from {} → [{}]", source, branches.join(", "))
            }
            Self::BranchComplete { source, branches } => {
                format!(
                    "⚑ Fan-in at {} ← [{}]",
                    source,
                    branches.join(", ")
                )
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
        self.events
            .lock()
            .map(|e| e.clone())
            .unwrap_or_default()
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
