use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent operation span — compatible with OpenTelemetry Span format.
///
/// Each Agent action (tool call, LLM request, decision) is recorded as a span
/// with timing, attributes, and causality links.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpan {
    /// Unique span ID (hex string)
    pub span_id: String,

    /// Trace ID — groups spans from a single agent task (hex string)
    pub trace_id: String,

    /// Parent span ID (for nested operations)
    pub parent_span_id: Option<String>,

    /// Agent ID that created this span
    pub agent_id: String,

    /// Operation name (e.g., "llm.chat", "tool.execute", "agent.decide")
    pub name: String,

    /// Span kind
    pub kind: SpanKind,

    /// Start timestamp
    pub start_time: DateTime<Utc>,

    /// End timestamp (None if still in progress)
    pub end_time: Option<DateTime<Utc>>,

    /// Span status
    pub status: SpanStatus,

    /// Key-value attributes (agent metadata, token counts, costs)
    pub attributes: Vec<KeyValue>,

    /// Causal links to other spans
    pub links: Vec<SpanLink>,

    /// Events within this span (errors, warnings, milestones)
    pub events: Vec<SpanEvent>,
}

/// Span kind — indicates the role of the span in the trace
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    /// Internal operation within an agent
    Internal,
    /// Agent calling an external tool
    Client,
    /// Agent receiving a request
    Server,
    /// Agent producing a message for another agent
    Producer,
    /// Agent consuming a message from another agent
    Consumer,
}

/// Span status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    /// Operation completed successfully
    Ok,
    /// Operation encountered an error
    Error,
    /// Operation was cancelled or timed out
    Cancelled,
    /// Status unset
    Unset,
}

/// Key-value attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: AttributeValue,
}

/// Attribute value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Double(f64),
    Bool(bool),
}

/// Link to another span (causal relationship)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLink {
    /// Trace ID of the linked span
    pub trace_id: String,
    /// Span ID of the linked span
    pub span_id: String,
    /// Attributes describing the relationship
    pub attributes: Vec<KeyValue>,
}

/// Event within a span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event attributes
    pub attributes: Vec<KeyValue>,
}

impl AgentSpan {
    /// Create a new span with the given name and agent ID
    pub fn new(name: impl Into<String>, agent_id: impl Into<String>, kind: SpanKind) -> Self {
        use uuid::Uuid;
        Self {
            span_id: Uuid::new_v4().to_string().replace('-', "")[..16].to_string(),
            trace_id: Uuid::new_v4().to_string().replace('-', ""),
            parent_span_id: None,
            agent_id: agent_id.into(),
            name: name.into(),
            kind,
            start_time: Utc::now(),
            end_time: None,
            status: SpanStatus::Unset,
            attributes: vec![],
            links: vec![],
            events: vec![],
        }
    }

    /// Set the parent span ID
    pub fn with_parent(mut self, parent_span_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(parent_span_id.into());
        self
    }

    /// Set the trace ID (to link spans in the same trace)
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = trace_id.into();
        self
    }

    /// Add a key-value attribute
    pub fn with_attribute(mut self, key: impl Into<String>, value: AttributeValue) -> Self {
        self.attributes.push(KeyValue {
            key: key.into(),
            value,
        });
        self
    }

    /// Add a link to another span
    pub fn with_link(mut self, trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        self.links.push(SpanLink {
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            attributes: vec![],
        });
        self
    }

    /// Add an event
    pub fn with_event(mut self, name: impl Into<String>) -> Self {
        self.events.push(SpanEvent {
            name: name.into(),
            timestamp: Utc::now(),
            attributes: vec![],
        });
        self
    }

    /// Mark the span as completed with the given status
    pub fn finish(mut self, status: SpanStatus) -> Self {
        self.end_time = Some(Utc::now());
        self.status = status;
        self
    }

    /// Get duration in milliseconds (if finished)
    pub fn duration_ms(&self) -> Option<i64> {
        self.end_time.map(|end| {
            (end - self.start_time).num_milliseconds()
        })
    }

    /// Check if the span is still in progress
    pub fn is_in_progress(&self) -> bool {
        self.end_time.is_none()
    }

    /// Get token count from attributes (if present)
    pub fn token_count(&self) -> Option<i64> {
        self.attributes.iter().find_map(|kv| {
            if kv.key == "llm.token_count" {
                match &kv.value {
                    AttributeValue::Int(n) => Some(*n),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    /// Get cost from attributes (if present)
    pub fn cost_usd(&self) -> Option<f64> {
        self.attributes.iter().find_map(|kv| {
            if kv.key == "agent.cost_usd" {
                match &kv.value {
                    AttributeValue::Double(n) => Some(*n),
                    _ => None,
                }
            } else {
                None
            }
        })
    }
}

/// Export format for OTLP (OpenTelemetry Protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpExportRequest {
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSpans {
    pub resource: Resource,
    pub scope_spans: Vec<ScopeSpans>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub attributes: Vec<KeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeSpans {
    pub scope: Scope,
    pub spans: Vec<AgentSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub name: String,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_creation() {
        let span = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal);
        assert_eq!(span.name, "llm.chat");
        assert_eq!(span.agent_id, "agent-1");
        assert_eq!(span.kind, SpanKind::Internal);
        assert!(span.is_in_progress());
    }

    #[test]
    fn test_span_finish() {
        let span = AgentSpan::new("tool.execute", "agent-1", SpanKind::Client)
            .finish(SpanStatus::Ok);

        assert!(!span.is_in_progress());
        assert_eq!(span.status, SpanStatus::Ok);
        assert!(span.duration_ms().is_some());
    }

    #[test]
    fn test_span_attributes() {
        let span = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
            .with_attribute("llm.model", AttributeValue::String("gpt-4".to_string()))
            .with_attribute("llm.token_count", AttributeValue::Int(1500))
            .with_attribute("agent.cost_usd", AttributeValue::Double(0.045));

        assert_eq!(span.token_count(), Some(1500));
        assert_eq!(span.cost_usd(), Some(0.045));
    }

    #[test]
    fn test_span_parent() {
        let parent = AgentSpan::new("agent.decide", "agent-1", SpanKind::Internal);
        let child = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
            .with_parent(&parent.span_id)
            .with_trace_id(&parent.trace_id);

        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_eq!(child.trace_id, parent.trace_id);
    }

    #[test]
    fn test_span_events() {
        let span = AgentSpan::new("tool.execute", "agent-1", SpanKind::Client)
            .with_event("tool_invoked")
            .with_event("tool_completed")
            .finish(SpanStatus::Ok);

        assert_eq!(span.events.len(), 2);
        assert_eq!(span.events[0].name, "tool_invoked");
        assert_eq!(span.events[1].name, "tool_completed");
    }

    #[test]
    fn test_span_serialization() {
        let span = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
            .with_attribute("model", AttributeValue::String("gpt-4".to_string()))
            .finish(SpanStatus::Ok);

        let json = serde_json::to_string(&span).unwrap();
        let deserialized: AgentSpan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "llm.chat");
        assert_eq!(deserialized.status, SpanStatus::Ok);
    }
}
