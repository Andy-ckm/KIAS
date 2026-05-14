//! WebSocket real-time push for KIAS API Server.
//!
//! Provides a pub/sub event bus that broadcasts system events to connected
//! WebSocket clients. Events include agent lifecycle changes, node health
//! updates, task completions, and scheduler decisions.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐    broadcast::Sender<Event>
//! │  Agent CRUD  │──────────────────────┐
//! ├─────────────┤                       │
//! │  Scheduler   │──────────────────────┤
//! ├─────────────┤                       ▼
//! │  Controller  │──────────────────► EventBus ──► WS Client 1
//! ├─────────────┤                       │    ──► WS Client 2
//! │  Node Health │──────────────────────┘    ──► WS Client N
//! └─────────────┘
//! ```
//!
//! # Wire Protocol
//!
//! Each WebSocket message is a JSON-serialized `WsMessage`:
//! ```json
//! {"type": "agent_status_changed", "data": {...}, "timestamp": "2025-01-01T00:00:00Z"}
//! ```
//!
//! Clients may send subscription filters:
//! ```json
//! {"subscribe": ["agent_status_changed", "task_completed"]}
//! ```

use std::collections::HashSet;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

// ─── Event Types ──────────────────────────────────────────────────────

/// All system events that can be pushed to WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// An agent's status changed (pending → running → completed/failed).
    AgentStatusChanged,
    /// A new agent was created.
    AgentCreated,
    /// An agent was deleted.
    AgentDeleted,
    /// A node's health status changed.
    NodeHealthChanged,
    /// A task completed on an agent.
    TaskCompleted,
    /// A task failed on an agent.
    TaskFailed,
    /// A workflow execution update.
    WorkflowUpdate,
    /// A scheduler decision (agent assigned to node).
    SchedulerDecision,
    /// System alert (high resource usage, etc).
    SystemAlert,
}

/// A broadcast event sent over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    /// The type of event.
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// JSON payload — structure depends on `event_type`.
    pub data: serde_json::Value,
    /// ISO-8601 timestamp when the event was created.
    pub timestamp: String,
}

/// Message from client → server (subscription filter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClientMessage {
    /// If present, only receive events of these types. Empty = all events.
    #[serde(default)]
    pub subscribe: Vec<EventType>,
}

// ─── Event Bus ────────────────────────────────────────────────────────

/// Thread-safe broadcast hub for real-time events.
///
/// Wraps a `tokio::sync::broadcast` channel with a fixed capacity.
/// When the channel is full the oldest events are dropped (lagged clients
/// receive a `WsEvent::system_alert` about missed messages).
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<WsEvent>,
}

impl EventBus {
    /// Create a new bus with the given channel capacity.
    ///
    /// Capacity determines how many events can queue before back-pressure
    /// kicks in. 1024 is suitable for most workloads.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all connected clients.
    pub fn publish(&self, event: WsEvent) {
        // Ignore error — means no active receivers, which is fine.
        let _ = self.sender.send(event);
    }

    /// Create a new subscriber handle.
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.sender.subscribe()
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ─── Convenience Publishers ───────────────────────────────────────────

impl EventBus {
    /// Publish an agent status change event.
    pub fn publish_agent_status_changed(
        &self,
        agent_id: &str,
        old_status: &str,
        new_status: &str,
    ) {
        self.publish(WsEvent {
            event_type: EventType::AgentStatusChanged,
            data: serde_json::json!({
                "agent_id": agent_id,
                "old_status": old_status,
                "new_status": new_status,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish an agent created event.
    pub fn publish_agent_created(&self, agent_id: &str, agent_name: &str) {
        self.publish(WsEvent {
            event_type: EventType::AgentCreated,
            data: serde_json::json!({
                "agent_id": agent_id,
                "name": agent_name,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish an agent deleted event.
    pub fn publish_agent_deleted(&self, agent_id: &str) {
        self.publish(WsEvent {
            event_type: EventType::AgentDeleted,
            data: serde_json::json!({ "agent_id": agent_id }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a node health change event.
    pub fn publish_node_health_changed(&self, node_id: &str, status: &str) {
        self.publish(WsEvent {
            event_type: EventType::NodeHealthChanged,
            data: serde_json::json!({
                "node_id": node_id,
                "status": status,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a task completed event.
    pub fn publish_task_completed(&self, agent_id: &str, task_id: &str, duration_ms: u64) {
        self.publish(WsEvent {
            event_type: EventType::TaskCompleted,
            data: serde_json::json!({
                "agent_id": agent_id,
                "task_id": task_id,
                "duration_ms": duration_ms,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a task failed event.
    pub fn publish_task_failed(&self, agent_id: &str, task_id: &str, error: &str) {
        self.publish(WsEvent {
            event_type: EventType::TaskFailed,
            data: serde_json::json!({
                "agent_id": agent_id,
                "task_id": task_id,
                "error": error,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a scheduler decision event.
    pub fn publish_scheduler_decision(&self, agent_id: &str, node_id: &str, reason: &str) {
        self.publish(WsEvent {
            event_type: EventType::SchedulerDecision,
            data: serde_json::json!({
                "agent_id": agent_id,
                "node_id": node_id,
                "reason": reason,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a workflow update event.
    pub fn publish_workflow_update(&self, workflow_id: &str, status: &str) {
        self.publish(WsEvent {
            event_type: EventType::WorkflowUpdate,
            data: serde_json::json!({
                "workflow_id": workflow_id,
                "status": status,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Publish a system alert event.
    pub fn publish_system_alert(&self, alert_type: &str, message: &str) {
        self.publish(WsEvent {
            event_type: EventType::SystemAlert,
            data: serde_json::json!({
                "alert_type": alert_type,
                "message": message,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }
}

// ─── WebSocket Handler ────────────────────────────────────────────────

/// Axum handler for WebSocket upgrade at `/ws`.
///
/// After upgrade, the client receives a welcome message and then
/// all broadcast events. Clients can optionally send a JSON filter
/// message to narrow event types.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.event_bus.clone()))
}

/// Per-connection socket loop.
async fn handle_socket(socket: WebSocket, event_bus: EventBus) {
    let (mut sender, mut receiver) = socket.split();

    // Send welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "KIAS WebSocket connected. Send {\"subscribe\": [...]} to filter events.",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if let Err(e) = sender
        .send(Message::Text(welcome.to_string()))
        .await
    {
        warn!("Failed to send welcome message: {}", e);
        return;
    }

    // Subscribe to event bus
    let mut rx = event_bus.subscribe();
    let event_bus_clone = event_bus.clone();

    // Shared filter state between recv task and message handler
    let (filter_tx, mut filter_rx) = tokio::sync::watch::channel::<Option<HashSet<EventType>>>(None);

    // Spawn a task to forward events from bus → WebSocket
    let (tx, mut rx_local) = tokio::sync::mpsc::channel::<WsEvent>(64);
    let send_task = tokio::spawn(async move {
        while let Some(event) = rx_local.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json)).await {
                        debug!("WebSocket send error (client disconnected): {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize WsEvent: {}", e);
                }
            }
        }
    });

    // Spawn a task to receive broadcast events and apply filter
    let recv_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            let current_filter = filter_rx.borrow_and_update().clone();
                            let should_send = match &current_filter {
                                Some(types) => types.contains(&event.event_type),
                                None => true,
                            };
                            if should_send && tx.send(event).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("WebSocket client lagged, missed {} events", n);
                            let alert = WsEvent {
                                event_type: EventType::SystemAlert,
                                data: serde_json::json!({
                                    "alert_type": "lagged",
                                    "message": format!("Missed {} events due to slow consumer", n),
                                }),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };
                            let _ = tx.send(alert).await;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            debug!("Event bus closed");
                            break;
                        }
                    }
                }
                _ = filter_rx.changed() => {
                    // Filter updated, continue loop to use new filter
                }
            }
        }
    });

    // Handle incoming messages from the client (subscription filters)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<WsClientMessage>(&text) {
                    Ok(client_msg) => {
                        if client_msg.subscribe.is_empty() {
                            let _ = filter_tx.send(None);
                            info!("Client cleared subscription filter (all events)");
                        } else {
                            info!("Client subscribed to: {:?}", client_msg.subscribe);
                            let _ = filter_tx.send(Some(client_msg.subscribe.into_iter().collect()));
                        }
                    }
                    Err(e) => {
                        debug!("Invalid client message: {} — {}", text, e);
                        // Echo error back
                        let err = serde_json::json!({
                            "type": "error",
                            "message": format!("Invalid message: {}", e),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        });
                        // Best effort — if send fails, client is probably gone
                        event_bus_clone.publish(WsEvent {
                            event_type: EventType::SystemAlert,
                            data: err,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket client sent close frame");
                break;
            }
            Ok(Message::Ping(data)) => {
                // Axum handles Pong automatically, but log it
                debug!("Received Ping: {:?}", data);
            }
            Ok(_) => {
                // Binary, Pong — ignore
            }
            Err(e) => {
                warn!("WebSocket receive error: {}", e);
                break;
            }
        }
    }

    // Cleanup
    send_task.abort();
    recv_task.abort();
    info!("WebSocket connection closed");
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();

        let event = WsEvent {
            event_type: EventType::AgentCreated,
            data: serde_json::json!({"agent_id": "a1", "name": "test"}),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        bus.publish(event.clone());

        let received = rx.try_recv().expect("should receive event");
        assert_eq!(received.event_type, EventType::AgentCreated);
        assert_eq!(received.data["agent_id"], "a1");
    }

    #[test]
    fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new(64);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish_agent_status_changed("agent-1", "pending", "running");

        let e1 = rx1.try_recv().expect("rx1 should receive");
        let e2 = rx2.try_recv().expect("rx2 should receive");
        assert_eq!(e1.event_type, EventType::AgentStatusChanged);
        assert_eq!(e2.event_type, EventType::AgentStatusChanged);
        assert_eq!(e1.data, e2.data);
    }

    #[test]
    fn test_event_bus_no_subscribers() {
        let bus = EventBus::new(64);
        // Publishing with no subscribers should not panic
        bus.publish_system_alert("test", "no listeners");
    }

    #[test]
    fn test_subscriber_count() {
        let bus = EventBus::new(64);
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(_rx1);
        // Note: receiver_count may not update immediately
    }

    #[test]
    fn test_ws_event_serialization_roundtrip() {
        let event = WsEvent {
            event_type: EventType::TaskCompleted,
            data: serde_json::json!({
                "agent_id": "a1",
                "task_id": "t1",
                "duration_ms": 42,
            }),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: WsEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, EventType::TaskCompleted);
        assert_eq!(deserialized.data["agent_id"], "a1");
        assert_eq!(deserialized.data["duration_ms"], 42);
    }

    #[test]
    fn test_ws_client_message_deserialization() {
        // With subscription filter
        let msg = r#"{"subscribe": ["agent_created", "task_completed"]}"#;
        let parsed: WsClientMessage = serde_json::from_str(msg).unwrap();
        assert_eq!(parsed.subscribe.len(), 2);
        assert!(parsed.subscribe.contains(&EventType::AgentCreated));
        assert!(parsed.subscribe.contains(&EventType::TaskCompleted));

        // Empty subscribe = all events
        let msg = r#"{"subscribe": []}"#;
        let parsed: WsClientMessage = serde_json::from_str(msg).unwrap();
        assert!(parsed.subscribe.is_empty());

        // Missing field = default (empty)
        let msg = r#"{}"#;
        let parsed: WsClientMessage = serde_json::from_str(msg).unwrap();
        assert!(parsed.subscribe.is_empty());
    }

    #[test]
    fn test_event_type_serialization() {
        // Ensure event types serialize as snake_case
        assert_eq!(
            serde_json::to_string(&EventType::AgentStatusChanged).unwrap(),
            r#""agent_status_changed""#
        );
        assert_eq!(
            serde_json::to_string(&EventType::SystemAlert).unwrap(),
            r#""system_alert""#
        );
    }

    #[test]
    fn test_publish_all_convenience_methods() {
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();

        bus.publish_agent_created("a1", "my-agent");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::AgentCreated);

        bus.publish_agent_deleted("a1");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::AgentDeleted);

        bus.publish_node_health_changed("n1", "unhealthy");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::NodeHealthChanged);

        bus.publish_task_completed("a1", "t1", 100);
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::TaskCompleted);

        bus.publish_task_failed("a1", "t1", "timeout");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::TaskFailed);

        bus.publish_scheduler_decision("a1", "n1", "least-loaded");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::SchedulerDecision);

        bus.publish_workflow_update("wf1", "completed");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::WorkflowUpdate);

        bus.publish_system_alert("disk", "low space");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::SystemAlert);
    }

    #[test]
    fn test_event_bus_default_capacity() {
        let bus = EventBus::default();
        // Should work without issues
        let mut rx = bus.subscribe();
        bus.publish_system_alert("test", "default capacity");
        let e = rx.try_recv().unwrap();
        assert_eq!(e.event_type, EventType::SystemAlert);
    }
}
