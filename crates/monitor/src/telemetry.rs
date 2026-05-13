use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    AgentCreated,
    AgentDestroyed,
    AgentHealthChanged,
    ErrorOccurred,
    SchedulerDecision,
    CacheHit,
    CacheMiss,
    HandoffInitiated,
    HandoffCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event_id: String,
    pub event_type: EventType,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub severity: Severity,
    pub data: serde_json::Value,
}

impl TelemetryEvent {
    pub fn new(event_type: EventType, agent_id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type,
            agent_id: agent_id.into(),
            timestamp: Utc::now(),
            severity: Severity::Info,
            data,
        }
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

/// Query filter for telemetry events
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub agent_id: Option<String>,
    pub event_type: Option<EventType>,
    pub severity: Option<Severity>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

impl EventFilter {
    pub fn matches(&self, event: &TelemetryEvent) -> bool {
        if let Some(ref agent_id) = self.agent_id {
            if &event.agent_id != agent_id {
                return false;
            }
        }
        if let Some(ref event_type) = self.event_type {
            if &event.event_type != event_type {
                return false;
            }
        }
        if let Some(ref severity) = self.severity {
            if &event.severity != severity {
                return false;
            }
        }
        if let Some(since) = self.since {
            if event.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if event.timestamp > until {
                return false;
            }
        }
        true
    }
}

/// Aggregated event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_events: usize,
    pub events_by_type: HashMap<String, usize>,
    pub events_by_agent: HashMap<String, usize>,
    pub events_by_severity: HashMap<String, usize>,
    pub error_rate: f64,
}

pub struct TelemetryCollector {
    events: Vec<TelemetryEvent>,
    max_events: usize,
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryCollector {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            max_events: 100_000,
        }
    }

    pub fn with_max_events(max_events: usize) -> Self {
        Self {
            events: Vec::new(),
            max_events,
        }
    }

    pub fn collect(&mut self, event: TelemetryEvent) {
        tracing::debug!(event_id = %event.event_id, event_type = ?event.event_type, "Collecting telemetry event");
        self.events.push(event);
        // Evict old events if over limit
        if self.events.len() > self.max_events {
            let drain_count = self.events.len() - self.max_events;
            self.events.drain(0..drain_count);
        }
    }

    pub fn get_events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Query events with filters
    pub fn query(&self, filter: &EventFilter) -> Vec<&TelemetryEvent> {
        self.events.iter().filter(|e| filter.matches(e)).collect()
    }

    /// Get events for a specific agent
    pub fn events_for_agent(&self, agent_id: &str) -> Vec<&TelemetryEvent> {
        let filter = EventFilter {
            agent_id: Some(agent_id.to_string()),
            ..Default::default()
        };
        self.query(&filter)
    }

    /// Get events by type
    pub fn events_by_type(&self, event_type: EventType) -> Vec<&TelemetryEvent> {
        let filter = EventFilter {
            event_type: Some(event_type),
            ..Default::default()
        };
        self.query(&filter)
    }

    /// Get events since a specific time
    pub fn events_since(&self, since: DateTime<Utc>) -> Vec<&TelemetryEvent> {
        let filter = EventFilter {
            since: Some(since),
            ..Default::default()
        };
        self.query(&filter)
    }

    /// Get aggregated statistics
    pub fn stats(&self) -> EventStats {
        let mut events_by_type: HashMap<String, usize> = HashMap::new();
        let mut events_by_agent: HashMap<String, usize> = HashMap::new();
        let mut events_by_severity: HashMap<String, usize> = HashMap::new();
        let mut error_count = 0usize;

        for event in &self.events {
            *events_by_type.entry(format!("{:?}", event.event_type)).or_insert(0) += 1;
            *events_by_agent.entry(event.agent_id.clone()).or_insert(0) += 1;
            *events_by_severity.entry(format!("{:?}", event.severity)).or_insert(0) += 1;
            if matches!(event.severity, Severity::Error | Severity::Critical) {
                error_count += 1;
            }
        }

        let total = self.events.len();
        EventStats {
            total_events: total,
            events_by_type,
            events_by_agent,
            events_by_severity,
            error_rate: if total > 0 { error_count as f64 / total as f64 } else { 0.0 },
        }
    }

    /// Export events as JSON
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.events).unwrap_or_else(|_| "[]".to_string())
    }

    /// Export events in NDJSON (newline-delimited JSON) format
    pub fn export_ndjson(&self) -> String {
        self.events.iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Get event count
    pub fn count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: EventType, agent_id: &str) -> TelemetryEvent {
        TelemetryEvent::new(event_type, agent_id, serde_json::json!({}))
    }

    #[test]
    fn test_telemetry_collector_creation() {
        let collector = TelemetryCollector::new();
        assert_eq!(collector.count(), 0);
    }

    #[test]
    fn test_collect_event() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "agent-1"));
        assert_eq!(collector.count(), 1);
    }

    #[test]
    fn test_event_types() {
        assert_ne!(EventType::TaskStarted, EventType::TaskCompleted);
        assert_ne!(EventType::AgentCreated, EventType::AgentDestroyed);
    }

    #[test]
    fn test_multiple_events() {
        let mut collector = TelemetryCollector::new();
        for i in 0..5 {
            collector.collect(TelemetryEvent::new(
                EventType::TaskCompleted,
                "agent-1",
                serde_json::json!({"index": i}),
            ));
        }
        assert_eq!(collector.count(), 5);
    }

    #[test]
    fn test_max_events_eviction() {
        let mut collector = TelemetryCollector::with_max_events(3);
        for i in 0..5 {
            collector.collect(TelemetryEvent::new(
                EventType::TaskCompleted,
                "agent-1",
                serde_json::json!({"index": i}),
            ));
        }
        assert_eq!(collector.count(), 3);
        // The oldest events should have been evicted
        let events = collector.get_events();
        assert_eq!(events[0].data["index"], 2);
    }

    #[test]
    fn test_query_by_agent() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "agent-1"));
        collector.collect(make_event(EventType::TaskCompleted, "agent-2"));
        collector.collect(make_event(EventType::TaskFailed, "agent-1"));

        let agent1_events = collector.events_for_agent("agent-1");
        assert_eq!(agent1_events.len(), 2);

        let agent2_events = collector.events_for_agent("agent-2");
        assert_eq!(agent2_events.len(), 1);
    }

    #[test]
    fn test_query_by_type() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "a1"));
        collector.collect(make_event(EventType::TaskCompleted, "a1"));
        collector.collect(make_event(EventType::TaskStarted, "a2"));

        let started = collector.events_by_type(EventType::TaskStarted);
        assert_eq!(started.len(), 2);

        let completed = collector.events_by_type(EventType::TaskCompleted);
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn test_query_combined_filter() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "a1"));
        collector.collect(make_event(EventType::TaskCompleted, "a1"));
        collector.collect(make_event(EventType::TaskStarted, "a2"));

        let filter = EventFilter {
            agent_id: Some("a1".to_string()),
            event_type: Some(EventType::TaskStarted),
            ..Default::default()
        };
        let results = collector.query(&filter);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_stats() {
        let mut collector = TelemetryCollector::new();
        collector.collect(TelemetryEvent::new(EventType::TaskStarted, "a1", serde_json::json!({})).with_severity(Severity::Info));
        collector.collect(TelemetryEvent::new(EventType::TaskFailed, "a1", serde_json::json!({})).with_severity(Severity::Error));
        collector.collect(TelemetryEvent::new(EventType::TaskCompleted, "a2", serde_json::json!({})).with_severity(Severity::Info));

        let stats = collector.stats();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.events_by_agent.get("a1"), Some(&2));
        assert_eq!(stats.events_by_agent.get("a2"), Some(&1));
        assert!((stats.error_rate - 1.0/3.0).abs() < 0.01);
    }

    #[test]
    fn test_export_json() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "a1"));
        let json = collector.export_json();
        assert!(json.contains("TaskStarted"));
    }

    #[test]
    fn test_export_ndjson() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "a1"));
        collector.collect(make_event(EventType::TaskCompleted, "a1"));
        let ndjson = collector.export_ndjson();
        let lines: Vec<&str> = ndjson.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut collector = TelemetryCollector::new();
        collector.collect(make_event(EventType::TaskStarted, "a1"));
        assert_eq!(collector.count(), 1);
        collector.clear();
        assert_eq!(collector.count(), 0);
    }

    #[test]
    fn test_event_severity() {
        let event = TelemetryEvent::new(EventType::ErrorOccurred, "a1", serde_json::json!({}))
            .with_severity(Severity::Critical);
        assert_eq!(event.severity, Severity::Critical);
    }
}
