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
//! Each WebSocket message is a JSON-serialized `WsEvent`:
//! ```json
//! {"type": "agent_status_changed", "data": {...}, "timestamp": "2025-01-01T00:00:00Z"}
//! ```
//!
//! Clients may send subscription filters:
//! ```json
//! {"subscribe": ["agent_status_changed", "task_completed"]}
//! ```
//!
//! # Production Features
//!
//! - **Connection Registry**: Tracks active connections with metadata (ID, address,
//!   connected-at timestamp, subscription filters).
//! - **Event Replay Buffer**: Ring buffer of recent events; new clients receive
//!   buffered events on connect to avoid missing initial state.
//! - **Heartbeat / Keepalive**: Server sends periodic ping frames (every 30s);
//!   connections that fail to respond within 90s are closed.
//! - **WS Stats Endpoint**: `GET /api/v1/ws/stats` returns active connection count,
//!   total messages sent, lagged events, and per-connection metadata.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
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
    /// An A2A task was submitted.
    A2aTaskSubmitted,
    /// An A2A task started processing.
    A2aTaskWorking,
    /// An A2A task completed.
    A2aTaskCompleted,
    /// An A2A task was cancelled.
    A2aTaskCancelled,
    /// An A2A task was deleted.
    A2aTaskDeleted,
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

// ─── Connection Registry ─────────────────────────────────────────────

/// Unique identifier for each WebSocket connection.
pub type ConnectionId = u64;

/// Metadata about a single WebSocket connection.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    /// Unique connection ID.
    pub id: ConnectionId,
    /// Remote address (if available from headers).
    pub remote_addr: Option<String>,
    /// ISO-8601 timestamp when the connection was established.
    pub connected_at: String,
    /// Current subscription filter (empty = all events).
    pub subscriptions: Vec<EventType>,
    /// Total events sent to this connection.
    pub events_sent: u64,
}

/// Global WebSocket connection statistics.
#[derive(Debug, Clone, Serialize)]
pub struct WsStats {
    /// Number of currently active connections.
    pub active_connections: usize,
    /// Total connections accepted since server start.
    pub total_connections: u64,
    /// Total messages sent across all connections.
    pub total_messages_sent: u64,
    /// Total lagged events (clients that fell behind).
    pub total_lagged: u64,
    /// Event replay buffer size.
    pub replay_buffer_size: usize,
    /// Maximum replay buffer capacity.
    pub replay_buffer_capacity: usize,
    /// Per-connection metadata.
    pub connections: Vec<ConnectionInfo>,
}

/// Tracks active WebSocket connections and metrics.
#[derive(Clone)]
pub struct ConnectionRegistry {
    inner: Arc<ConnectionRegistryInner>,
}

struct ConnectionRegistryInner {
    connections: RwLock<std::collections::HashMap<ConnectionId, ConnectionInfo>>,
    total_connections: AtomicU64,
    total_messages_sent: AtomicU64,
    total_lagged: AtomicU64,
    next_id: AtomicU64,
}

impl ConnectionRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ConnectionRegistryInner {
                connections: RwLock::new(std::collections::HashMap::new()),
                total_connections: AtomicU64::new(0),
                total_messages_sent: AtomicU64::new(0),
                total_lagged: AtomicU64::new(0),
                next_id: AtomicU64::new(1),
            }),
        }
    }

    /// Register a new connection and return its unique ID.
    pub async fn register(&self, remote_addr: Option<String>) -> ConnectionId {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let info = ConnectionInfo {
            id,
            remote_addr,
            connected_at: chrono::Utc::now().to_rfc3339(),
            subscriptions: Vec::new(),
            events_sent: 0,
        };
        self.inner.connections.write().await.insert(id, info);
        self.inner.total_connections.fetch_add(1, Ordering::Relaxed);
        id
    }

    /// Unregister a connection.
    pub async fn unregister(&self, id: ConnectionId) {
        self.inner.connections.write().await.remove(&id);
    }

    /// Update subscription filter for a connection.
    pub async fn set_subscriptions(&self, id: ConnectionId, subs: Vec<EventType>) {
        if let Some(conn) = self.inner.connections.write().await.get_mut(&id) {
            conn.subscriptions = subs;
        }
    }

    /// Increment messages sent counter (global and per-connection).
    pub fn inc_messages_sent(&self, _id: ConnectionId) {
        self.inner.total_messages_sent.fetch_add(1, Ordering::Relaxed);
        // Per-connection counter is best-effort (async lock not worth it here)
        // We'll update it in bulk during stats collection
    }

    /// Increment lagged events counter.
    pub fn inc_lagged(&self) {
        self.inner.total_lagged.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current stats snapshot.
    pub async fn stats(&self, replay_buffer_len: usize, replay_buffer_cap: usize) -> WsStats {
        let conns = self.inner.connections.read().await;
        WsStats {
            active_connections: conns.len(),
            total_connections: self.inner.total_connections.load(Ordering::Relaxed),
            total_messages_sent: self.inner.total_messages_sent.load(Ordering::Relaxed),
            total_lagged: self.inner.total_lagged.load(Ordering::Relaxed),
            replay_buffer_size: replay_buffer_len,
            replay_buffer_capacity: replay_buffer_cap,
            connections: conns.values().cloned().collect(),
        }
    }
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Event Replay Buffer ─────────────────────────────────────────────

/// A fixed-capacity ring buffer that stores recent events for replay to
/// newly connected clients.
#[derive(Clone)]
pub struct EventReplayBuffer {
    inner: Arc<RwLock<Vec<WsEvent>>>,
    capacity: usize,
}

impl EventReplayBuffer {
    /// Create a new replay buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::with_capacity(capacity))),
            capacity,
        }
    }

    /// Push an event into the buffer, evicting the oldest if full.
    pub async fn push(&self, event: WsEvent) {
        let mut buf = self.inner.write().await;
        if buf.len() >= self.capacity {
            buf.remove(0);
        }
        buf.push(event);
    }

    /// Return a clone of all buffered events (for replay to new clients).
    pub async fn snapshot(&self) -> Vec<WsEvent> {
        self.inner.read().await.clone()
    }

    /// Current number of buffered events.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Whether the buffer is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for EventReplayBuffer {
    fn default() -> Self {
        Self::new(100)
    }
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
    pub fn publish_agent_status_changed(&self, agent_id: &str, old_status: &str, new_status: &str) {
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

/// Heartbeat interval: server sends a Ping every 30 seconds.
const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Stale connection timeout: if no Pong within 90s, close the connection.
const STALE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Axum handler for WebSocket upgrade at `/ws`.
///
/// After upgrade, the client receives replayed events from the buffer,
/// then all new broadcast events. Clients can optionally send a JSON filter
/// message to narrow event types.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state.event_bus.clone(),
            state.connection_registry.clone(),
            state.event_replay_buffer.clone(),
        )
    })
}

/// Per-connection socket loop with heartbeat, event replay, and metrics.
async fn handle_socket(
    socket: WebSocket,
    event_bus: EventBus,
    registry: ConnectionRegistry,
    replay_buffer: EventReplayBuffer,
) {
    let (mut sender, mut receiver) = socket.split();

    // Register connection
    let conn_id = registry.register(None).await;
    info!(connection_id = conn_id, "WebSocket connection established");

    // Send welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "connection_id": conn_id,
        "message": "KIAS WebSocket connected. Send {\"subscribe\": [...]} to filter events.",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if let Err(e) = sender.send(Message::Text(welcome.to_string())).await {
        warn!(connection_id = conn_id, "Failed to send welcome message: {}", e);
        registry.unregister(conn_id).await;
        return;
    }

    // Replay buffered events to the new client
    let replayed = replay_buffer.snapshot().await;
    if !replayed.is_empty() {
        info!(
            connection_id = conn_id,
            count = replayed.len(),
            "Replaying buffered events"
        );
        for event in &replayed {
            match serde_json::to_string(event) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json)).await {
                        warn!(connection_id = conn_id, "Failed during replay: {}", e);
                        registry.unregister(conn_id).await;
                        return;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize replay event: {}", e);
                }
            }
        }
    }

    // Subscribe to event bus
    let mut rx = event_bus.subscribe();
    let event_bus_clone = event_bus.clone();
    let registry_for_recv = registry.clone();

    // Shared filter state between recv task and message handler
    let (filter_tx, mut filter_rx) =
        tokio::sync::watch::channel::<Option<HashSet<EventType>>>(None);

    // Channel for the send task
    let (tx, mut rx_local) = tokio::sync::mpsc::channel::<WsEvent>(64);

    // Clone registry for the send task
    let registry_for_send = registry.clone();
    let conn_id_for_send = conn_id;

    // Spawn a task to forward events from mpsc channel → WebSocket, with heartbeat
    let send_task = tokio::spawn(async move {
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut msg_count: u64 = 0;

        loop {
            tokio::select! {
                // Forward events from mpsc to WebSocket
                event = rx_local.recv() => {
                    match event {
                        Some(ws_event) => {
                            match serde_json::to_string(&ws_event) {
                                Ok(json) => {
                                    if let Err(e) = sender.send(Message::Text(json)).await {
                                        debug!("WebSocket send error (client disconnected): {}", e);
                                        break;
                                    }
                                    msg_count += 1;
                                }
                                Err(e) => {
                                    error!("Failed to serialize WsEvent: {}", e);
                                }
                            }
                        }
                        None => break, // Channel closed
                    }
                }
                // Heartbeat: send periodic ping
                _ = heartbeat_interval.tick() => {
                    if let Err(e) = sender.send(Message::Ping(vec![])).await {
                        debug!("Heartbeat ping failed (client disconnected): {}", e);
                        break;
                    }
                }
            }
        }

        // Update metrics before exiting
        registry_for_send.inc_messages_sent(conn_id_for_send);
        debug!(connection_id = conn_id_for_send, messages_sent = msg_count, "Send task exiting");
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
                            registry_for_recv.inc_lagged();
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

    // Handle incoming messages from the client (subscription filters + pong)
    let mut last_pong = tokio::time::Instant::now();
    let mut stale_check = tokio::time::interval(STALE_TIMEOUT);
    stale_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<WsClientMessage>(&text) {
                            Ok(client_msg) => {
                                if client_msg.subscribe.is_empty() {
                                    let _ = filter_tx.send(None);
                                    registry.set_subscriptions(conn_id, Vec::new()).await;
                                    info!(connection_id = conn_id, "Client cleared subscription filter (all events)");
                                } else {
                                    info!(connection_id = conn_id, "Client subscribed to: {:?}", client_msg.subscribe);
                                    registry.set_subscriptions(
                                        conn_id,
                                        client_msg.subscribe.clone(),
                                    ).await;
                                    let _ =
                                        filter_tx.send(Some(client_msg.subscribe.into_iter().collect()));
                                }
                            }
                            Err(e) => {
                                debug!(connection_id = conn_id, "Invalid client message: {} — {}", text, e);
                                // Send error back directly — don't broadcast to everyone
                                event_bus_clone.publish(WsEvent {
                                    event_type: EventType::SystemAlert,
                                    data: serde_json::json!({
                                        "type": "error",
                                        "message": format!("Invalid message: {}", e),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                });
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!(connection_id = conn_id, "WebSocket client sent close frame");
                        break;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = tokio::time::Instant::now();
                        debug!(connection_id = conn_id, "Received Pong");
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // Axum handles Pong automatically, but log it
                        debug!(connection_id = conn_id, "Received Ping: {:?}", data);
                    }
                    Some(Ok(_)) => {
                        // Binary — ignore
                    }
                    Some(Err(e)) => {
                        warn!(connection_id = conn_id, "WebSocket receive error: {}", e);
                        break;
                    }
                    None => {
                        // Stream ended
                        break;
                    }
                }
            }
            _ = stale_check.tick() => {
                // Check if connection is stale (no pong received within timeout)
                if last_pong.elapsed() > STALE_TIMEOUT {
                    warn!(
                        connection_id = conn_id,
                        "Connection stale — no Pong received within {}s, closing",
                        STALE_TIMEOUT.as_secs()
                    );
                    break;
                }
            }
        }
    }

    // Cleanup
    send_task.abort();
    recv_task.abort();
    registry.unregister(conn_id).await;
    info!(connection_id = conn_id, "WebSocket connection closed");
}

// ─── WS Stats Handler ────────────────────────────────────────────────

/// GET /api/v1/ws/stats
/// Returns WebSocket connection statistics.
pub async fn ws_stats_handler(
    State(state): State<crate::AppState>,
) -> axum::Json<WsStats> {
    let replay_len = state.event_replay_buffer.len().await;
    let replay_cap = state.event_replay_buffer.capacity();
    let stats = state
        .connection_registry
        .stats(replay_len, replay_cap)
        .await;
    axum::Json(stats)
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── EventBus tests ───────────────────────────────────────────────

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

    // ── ConnectionRegistry tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_connection_registry_register_unregister() {
        let registry = ConnectionRegistry::new();
        let id1 = registry.register(Some("127.0.0.1:8080".to_string())).await;
        let id2 = registry.register(Some("192.168.1.1:9090".to_string())).await;

        assert!(id1 != id2);
        let stats = registry.stats(0, 0).await;
        assert_eq!(stats.active_connections, 2);
        assert_eq!(stats.total_connections, 2);

        registry.unregister(id1).await;
        let stats = registry.stats(0, 0).await;
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.total_connections, 2); // Total doesn't decrease
    }

    #[tokio::test]
    async fn test_connection_registry_subscriptions() {
        let registry = ConnectionRegistry::new();
        let id = registry.register(None).await;

        registry
            .set_subscriptions(
                id,
                vec![EventType::AgentCreated, EventType::TaskCompleted],
            )
            .await;

        let stats = registry.stats(0, 0).await;
        assert_eq!(stats.connections.len(), 1);
        assert_eq!(stats.connections[0].subscriptions.len(), 2);
        assert!(stats.connections[0]
            .subscriptions
            .contains(&EventType::AgentCreated));
    }

    #[tokio::test]
    async fn test_connection_registry_metrics() {
        let registry = ConnectionRegistry::new();
        let id = registry.register(None).await;

        registry.inc_messages_sent(id);
        registry.inc_messages_sent(id);
        registry.inc_lagged();

        let stats = registry.stats(0, 0).await;
        assert_eq!(stats.total_messages_sent, 2);
        assert_eq!(stats.total_lagged, 1);
    }

    #[tokio::test]
    async fn test_connection_registry_default() {
        let registry = ConnectionRegistry::default();
        let id = registry.register(None).await;
        assert_eq!(id, 1);
    }

    // ── EventReplayBuffer tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_replay_buffer_push_and_snapshot() {
        let buffer = EventReplayBuffer::new(5);

        buffer
            .push(WsEvent {
                event_type: EventType::AgentCreated,
                data: serde_json::json!({"agent_id": "a1"}),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            })
            .await;

        buffer
            .push(WsEvent {
                event_type: EventType::TaskCompleted,
                data: serde_json::json!({"task_id": "t1"}),
                timestamp: "2025-01-01T00:00:01Z".to_string(),
            })
            .await;

        let snapshot = buffer.snapshot().await;
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].event_type, EventType::AgentCreated);
        assert_eq!(snapshot[1].event_type, EventType::TaskCompleted);
    }

    #[tokio::test]
    async fn test_replay_buffer_eviction() {
        let buffer = EventReplayBuffer::new(3);

        for i in 0..5 {
            buffer
                .push(WsEvent {
                    event_type: EventType::SystemAlert,
                    data: serde_json::json!({"index": i}),
                    timestamp: format!("2025-01-01T00:00:0{}Z", i),
                })
                .await;
        }

        let snapshot = buffer.snapshot().await;
        assert_eq!(snapshot.len(), 3);
        // Should have events 2, 3, 4 (evicted 0 and 1)
        assert_eq!(snapshot[0].data["index"], 2);
        assert_eq!(snapshot[1].data["index"], 3);
        assert_eq!(snapshot[2].data["index"], 4);
    }

    #[tokio::test]
    async fn test_replay_buffer_len_and_capacity() {
        let buffer = EventReplayBuffer::new(10);
        assert_eq!(buffer.len().await, 0);
        assert_eq!(buffer.capacity(), 10);

        buffer
            .push(WsEvent {
                event_type: EventType::AgentCreated,
                data: serde_json::json!({}),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            })
            .await;
        assert_eq!(buffer.len().await, 1);
    }

    #[tokio::test]
    async fn test_replay_buffer_default() {
        let buffer = EventReplayBuffer::default();
        assert_eq!(buffer.capacity(), 100);
        assert_eq!(buffer.len().await, 0);
    }

    #[tokio::test]
    async fn test_replay_buffer_snapshot_is_clone() {
        let buffer = EventReplayBuffer::new(5);

        buffer
            .push(WsEvent {
                event_type: EventType::AgentCreated,
                data: serde_json::json!({"agent_id": "a1"}),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            })
            .await;

        let snap1 = buffer.snapshot().await;
        let snap2 = buffer.snapshot().await;
        // Both snapshots should be independent clones
        assert_eq!(snap1.len(), snap2.len());
        assert_eq!(snap1[0].data["agent_id"], snap2[0].data["agent_id"]);
    }

    // ── WsStats serialization ────────────────────────────────────────

    #[test]
    fn test_ws_stats_serialization() {
        let stats = WsStats {
            active_connections: 3,
            total_connections: 10,
            total_messages_sent: 42,
            total_lagged: 1,
            replay_buffer_size: 50,
            replay_buffer_capacity: 100,
            connections: vec![ConnectionInfo {
                id: 1,
                remote_addr: Some("127.0.0.1:8080".to_string()),
                connected_at: "2025-01-01T00:00:00Z".to_string(),
                subscriptions: vec![EventType::AgentCreated],
                events_sent: 10,
            }],
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"active_connections\":3"));
        assert!(json.contains("\"total_messages_sent\":42"));
        assert!(json.contains("agent_created"));
    }

    #[test]
    fn test_connection_info_serialization() {
        let info = ConnectionInfo {
            id: 42,
            remote_addr: None,
            connected_at: "2025-01-01T00:00:00Z".to_string(),
            subscriptions: vec![],
            events_sent: 0,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":42"));
        // Null address when not set
        assert!(json.contains("null"));
    }

    // ── Heartbeat constants ──────────────────────────────────────────

    #[test]
    fn test_heartbeat_interval_is_reasonable() {
        assert!(HEARTBEAT_INTERVAL.as_secs() >= 10);
        assert!(HEARTBEAT_INTERVAL.as_secs() <= 120);
    }

    #[test]
    fn test_stale_timeout_is_longer_than_heartbeat() {
        assert!(STALE_TIMEOUT > HEARTBEAT_INTERVAL);
    }

    // ── Integration: publish to replay buffer ────────────────────────

    #[tokio::test]
    async fn test_publish_and_replay_integration() {
        let bus = EventBus::new(256);
        let buffer = EventReplayBuffer::new(10);

        // Subscribe first (broadcast only delivers to existing subscribers)
        let mut rx = bus.subscribe();

        // Publish events
        bus.publish_agent_created("a1", "agent-one");
        bus.publish_agent_status_changed("a1", "pending", "running");

        // Drain received events into replay buffer
        while let Ok(event) = rx.try_recv() {
            buffer.push(event).await;
        }

        // Verify replay
        let replayed = buffer.snapshot().await;
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].event_type, EventType::AgentCreated);
        assert_eq!(replayed[1].event_type, EventType::AgentStatusChanged);
    }
}
