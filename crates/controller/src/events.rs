//! Event-driven agent lifecycle event system.
//!
//! Provides a publish-subscribe [`EventBus`] for agent lifecycle events,
//! allowing multiple consumers to react to state changes asynchronously.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Agent lifecycle events.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Agent was created in the system.
    Created,
    /// Agent was scheduled to run on a node.
    Scheduled,
    /// Agent is now running.
    Running,
    /// Agent completed successfully.
    Completed,
    /// Agent failed.
    Failed,
    /// Agent is being recovered (restart attempt).
    Recovering,
    /// Agent was terminated (force-kill or graceful shutdown).
    Terminated,
    /// Agent health status changed.
    HealthChanged,
    /// Agent configuration was updated.
    ConfigUpdated,
}

/// Subscription filter key for the event bus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Subscribe to a specific event variant.
    Specific(AgentEvent),
    /// Subscribe to all events.
    All,
}

/// An event envelope carrying metadata about an agent event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEventEnvelope {
    /// The event that occurred.
    pub event: AgentEvent,
    /// The agent this event pertains to.
    pub agent_id: String,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
    /// Source component that generated this event.
    pub source: String,
}

impl AgentEventEnvelope {
    /// Create a new event envelope with the current timestamp.
    pub fn new(event: AgentEvent, agent_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            event,
            agent_id: agent_id.into(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            source: source.into(),
        }
    }

    /// Attach a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Internal subscriber entry with optional agent filter.
struct Subscription {
    /// If set, only deliver events for this agent.
    agent_filter: Option<String>,
    /// Channel sender to deliver events.
    sender: mpsc::Sender<AgentEventEnvelope>,
}

/// Concurrent event bus for publishing and subscribing to agent events.
///
/// Uses a lock-free [`DashMap`] for subscriber management. Supports filtering
/// by event type and optional agent ID.
pub struct EventBus {
    subscribers: DashMap<EventType, Vec<Subscription>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            subscribers: DashMap::new(),
        }
    }

    /// Subscribe to events matching the given type and optional agent filter.
    ///
    /// - `agent_id`: If `Some`, only events for this agent are delivered.
    ///   If `None`, events for all agents are delivered.
    /// - `event_type`: The event type filter (specific variant or `All`).
    ///
    /// Returns a receiver that will yield matching [`AgentEventEnvelope`]s.
    pub fn subscribe(
        &self,
        agent_id: Option<&str>,
        event_type: EventType,
    ) -> mpsc::Receiver<AgentEventEnvelope> {
        let (tx, rx) = mpsc::channel(256);
        let sub = Subscription {
            agent_filter: agent_id.map(|s| s.to_string()),
            sender: tx,
        };
        self.subscribers.entry(event_type).or_default().push(sub);
        rx
    }

    /// Publish an event envelope to all matching subscribers.
    ///
    /// Fans out to subscribers of the specific event type AND subscribers
    /// registered with `EventType::All`. Respects per-subscriber agent filters.
    pub async fn publish(&self, envelope: &AgentEventEnvelope) {
        let specific_key = EventType::Specific(envelope.event.clone());
        let all_key = EventType::All;

        // Collect matching senders to avoid holding DashMap guards across await.
        let mut senders: Vec<mpsc::Sender<AgentEventEnvelope>> = Vec::new();

        for key in [&specific_key, &all_key] {
            if let Some(subs) = self.subscribers.get(key) {
                for sub in subs.iter() {
                    let matches = match &sub.agent_filter {
                        Some(filter) => *filter == envelope.agent_id,
                        None => true, // no filter = all agents
                    };
                    if matches {
                        senders.push(sub.sender.clone());
                    }
                }
            }
        }

        // Best-effort delivery; ignore closed channels.
        for sender in senders {
            let _ = sender.send(envelope.clone()).await;
        }
    }

    /// Total number of active subscriptions across all event types.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers
            .iter()
            .map(|entry| entry.value().len())
            .sum()
    }
}

/// Trait for asynchronous event processors.
///
/// Implementors can be plugged into event-processing pipelines to react
/// to agent events (logging, metrics, alerting, etc.).
#[async_trait::async_trait]
pub trait EventProcessor: Send + Sync {
    /// Process a single event envelope.
    async fn handle_event(&self, envelope: &AgentEventEnvelope) -> anyhow::Result<()>;
}

/// Logs every event at INFO level via `tracing`.
pub struct LoggingProcessor;

#[async_trait::async_trait]
impl EventProcessor for LoggingProcessor {
    async fn handle_event(&self, envelope: &AgentEventEnvelope) -> anyhow::Result<()> {
        tracing::info!(
            event = ?envelope.event,
            agent_id = %envelope.agent_id,
            source = %envelope.source,
            "Agent event received"
        );
        Ok(())
    }
}

/// Tracks in-memory event counts by event type.
pub struct MetricsProcessor {
    counts: DashMap<AgentEvent, u64>,
}

impl Default for MetricsProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsProcessor {
    pub fn new() -> Self {
        Self {
            counts: DashMap::new(),
        }
    }

    /// Get the count for a specific event type.
    pub fn count(&self, event: &AgentEvent) -> u64 {
        self.counts.get(event).map(|c| *c).unwrap_or(0)
    }

    /// Get total event count across all types.
    pub fn total_count(&self) -> u64 {
        self.counts.iter().map(|entry| *entry.value()).sum()
    }
}

#[async_trait::async_trait]
impl EventProcessor for MetricsProcessor {
    async fn handle_event(&self, envelope: &AgentEventEnvelope) -> anyhow::Result<()> {
        *self.counts.entry(envelope.event.clone()).or_insert(0) += 1;
        Ok(())
    }
}

/// Forwards critical events (`Failed`, `Terminated`) to an alert channel.
pub struct AlertProcessor {
    alerts: mpsc::Sender<AgentEventEnvelope>,
}

impl AlertProcessor {
    pub fn new(alerts_tx: mpsc::Sender<AgentEventEnvelope>) -> Self {
        Self { alerts: alerts_tx }
    }

    fn is_critical(event: &AgentEvent) -> bool {
        matches!(event, AgentEvent::Failed | AgentEvent::Terminated)
    }
}

#[async_trait::async_trait]
impl EventProcessor for AlertProcessor {
    async fn handle_event(&self, envelope: &AgentEventEnvelope) -> anyhow::Result<()> {
        if Self::is_critical(&envelope.event) {
            tracing::warn!(
                agent_id = %envelope.agent_id,
                event = ?envelope.event,
                "Critical agent event — alerting"
            );
            let _ = self.alerts.send(envelope.clone()).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe_basic() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(None, EventType::All);

        let envelope = AgentEventEnvelope::new(AgentEvent::Created, "a1", "test");
        bus.publish(&envelope).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event, AgentEvent::Created);
        assert_eq!(received.agent_id, "a1");
        assert_eq!(received.source, "test");
    }

    #[tokio::test]
    async fn test_subscribe_specific_event() {
        let bus = EventBus::new();
        let mut rx_running = bus.subscribe(None, EventType::Specific(AgentEvent::Running));
        let mut rx_completed = bus.subscribe(None, EventType::Specific(AgentEvent::Completed));

        // Publish a Running event.
        bus.publish(&AgentEventEnvelope::new(AgentEvent::Running, "a1", "test"))
            .await;

        let received = rx_running.recv().await.unwrap();
        assert_eq!(received.event, AgentEvent::Running);

        // Completed subscriber should NOT receive the Running event.
        assert!(rx_completed.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_subscribe_filtered_by_agent() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe(Some("a1"), EventType::All);

        // Event for a1 — should be received.
        bus.publish(&AgentEventEnvelope::new(AgentEvent::Running, "a1", "test"))
            .await;
        let received = rx.recv().await.unwrap();
        assert_eq!(received.agent_id, "a1");

        // Event for a2 — should NOT be received.
        bus.publish(&AgentEventEnvelope::new(AgentEvent::Running, "a2", "test"))
            .await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_fan_out_to_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe(None, EventType::All);
        let mut rx2 = bus.subscribe(None, EventType::All);
        let mut rx3 = bus.subscribe(Some("a1"), EventType::All);

        let envelope = AgentEventEnvelope::new(AgentEvent::Created, "a1", "test");
        bus.publish(&envelope).await;

        // All three subscribers should receive the event.
        assert_eq!(rx1.recv().await.unwrap().event, AgentEvent::Created);
        assert_eq!(rx2.recv().await.unwrap().event, AgentEvent::Created);
        assert_eq!(rx3.recv().await.unwrap().event, AgentEvent::Created);
    }

    #[tokio::test]
    async fn test_publish_with_no_subscribers() {
        let bus = EventBus::new();
        // Should not panic.
        bus.publish(&AgentEventEnvelope::new(AgentEvent::Created, "a1", "test"))
            .await;
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe(None, EventType::All);
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe(None, EventType::Specific(AgentEvent::Running));
        assert_eq!(bus.subscriber_count(), 2);

        let _rx3 = bus.subscribe(Some("a1"), EventType::All);
        assert_eq!(bus.subscriber_count(), 3);
    }

    #[tokio::test]
    async fn test_metrics_processor() {
        let processor = MetricsProcessor::new();

        processor
            .handle_event(&AgentEventEnvelope::new(AgentEvent::Running, "a1", "test"))
            .await
            .unwrap();
        processor
            .handle_event(&AgentEventEnvelope::new(AgentEvent::Running, "a2", "test"))
            .await
            .unwrap();
        processor
            .handle_event(&AgentEventEnvelope::new(AgentEvent::Failed, "a3", "test"))
            .await
            .unwrap();

        assert_eq!(processor.count(&AgentEvent::Running), 2);
        assert_eq!(processor.count(&AgentEvent::Failed), 1);
        assert_eq!(processor.count(&AgentEvent::Completed), 0);
        assert_eq!(processor.total_count(), 3);
    }

    #[tokio::test]
    async fn test_alert_processor_fires_on_critical_events() {
        let (tx, mut rx) = mpsc::channel(16);
        let processor = AlertProcessor::new(tx);

        // Failed should trigger an alert.
        processor
            .handle_event(&AgentEventEnvelope::new(AgentEvent::Failed, "a1", "test"))
            .await
            .unwrap();
        let alert = rx.recv().await.unwrap();
        assert_eq!(alert.event, AgentEvent::Failed);

        // Terminated should trigger an alert.
        processor
            .handle_event(&AgentEventEnvelope::new(
                AgentEvent::Terminated,
                "a2",
                "test",
            ))
            .await
            .unwrap();
        let alert = rx.recv().await.unwrap();
        assert_eq!(alert.event, AgentEvent::Terminated);
    }

    #[tokio::test]
    async fn test_alert_processor_ignores_non_critical() {
        let (tx, mut rx) = mpsc::channel(16);
        let processor = AlertProcessor::new(tx);

        processor
            .handle_event(&AgentEventEnvelope::new(AgentEvent::Running, "a1", "test"))
            .await
            .unwrap();
        processor
            .handle_event(&AgentEventEnvelope::new(
                AgentEvent::Completed,
                "a1",
                "test",
            ))
            .await
            .unwrap();

        // No alerts should have been sent.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_event_envelope_metadata() {
        let envelope = AgentEventEnvelope::new(AgentEvent::ConfigUpdated, "a1", "test")
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");

        assert_eq!(envelope.metadata.get("key1").unwrap(), "value1");
        assert_eq!(envelope.metadata.get("key2").unwrap(), "value2");
        assert_eq!(envelope.metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_agents_with_filtered_subscribers() {
        let bus = EventBus::new();
        let mut rx_a1 = bus.subscribe(Some("a1"), EventType::Specific(AgentEvent::Running));
        let mut rx_a2 = bus.subscribe(Some("a2"), EventType::Specific(AgentEvent::Running));
        let mut rx_all = bus.subscribe(None, EventType::Specific(AgentEvent::Running));

        bus.publish(&AgentEventEnvelope::new(AgentEvent::Running, "a1", "test"))
            .await;
        bus.publish(&AgentEventEnvelope::new(AgentEvent::Running, "a2", "test"))
            .await;

        // a1 subscriber gets only a1 events.
        assert_eq!(rx_a1.recv().await.unwrap().agent_id, "a1");
        assert!(rx_a1.try_recv().is_err());

        // a2 subscriber gets only a2 events.
        assert_eq!(rx_a2.recv().await.unwrap().agent_id, "a2");
        assert!(rx_a2.try_recv().is_err());

        // "all" subscriber gets both.
        assert_eq!(rx_all.recv().await.unwrap().agent_id, "a1");
        assert_eq!(rx_all.recv().await.unwrap().agent_id, "a2");
    }

    #[tokio::test]
    async fn test_logging_processor_does_not_panic() {
        let processor = LoggingProcessor;
        let envelope = AgentEventEnvelope::new(AgentEvent::HealthChanged, "a1", "test");
        // Should succeed without panicking.
        processor.handle_event(&envelope).await.unwrap();
    }
}
