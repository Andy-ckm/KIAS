
// src/observability/mod.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// KiasError from the error module
pub use crate::error::KiasError;

/// Unified trace ID type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub String);

impl TraceId {
    /// Generate a new trace ID
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let random: u64 = rand_simple();
        Self(format!("{:x}-{:x}", timestamp, random))
    }

    /// Create from existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get the underlying string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Simple random number generator (for demo purposes)
fn rand_simple() -> u64 {
    use std::time::Instant;
    let start = Instant::now();
    let duration = start.elapsed();
    (duration.as_nanos() as u64) ^ (std::process::id() as u64 * 0x5DEECE66D)
}

/// Span ID type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(pub String);

impl SpanId {
    /// Generate a new span ID
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(format!("{:016x}", timestamp))
    }

    /// Create from existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get the underlying string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trace context holding trace and span information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub service_name: String,
    pub operation_name: String,
    pub start_time: u64,
    pub attributes: HashMap<String, String>,
}

impl TraceContext {
    /// Create a new trace context for an initial request
    pub fn new_root(service_name: String, operation_name: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        Self {
            trace_id: TraceId::new(),
            span_id: SpanId::new(),
            parent_span_id: None,
            service_name,
            operation_name,
            start_time: now,
            attributes: HashMap::new(),
        }
    }

    /// Create a child span context
    pub fn child_span(&self, operation_name: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        Self {
            trace_id: self.trace_id.clone(),
            span_id: SpanId::new(),
            parent_span_id: Some(self.span_id.clone()),
            service_name: self.service_name.clone(),
            operation_name,
            start_time: now,
            attributes: HashMap::new(),
        }
    }

    /// Add attribute to the context
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Propagate context to headers (for HTTP, gRPC, etc.)
    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("x-trace-id".to_string(), self.trace_id.as_str().to_string());
        headers.insert("x-span-id".to_string(), self.span_id.as_str().to_string());
        if let Some(ref parent) = self.parent_span_id {
            headers.insert("x-parent-span-id".to_string(), parent.as_str().to_string());
        }
        headers.insert("x-service-name".to_string(), self.service_name.clone());
        headers.insert("x-operation-name".to_string(), self.operation_name.clone());
        headers
    }

    /// Extract context from headers
    pub fn from_headers(headers: &HashMap<String, String>) -> Result<Self, KiasError> {
        let trace_id = headers
            .get("x-trace-id")
            .ok_or_else(|| KiasError::ValidationError("Missing trace ID".to_string()))?
            .clone();
        let span_id = headers
            .get("x-span-id")
            .ok_or_else(|| KiasError::ValidationError("Missing span ID".to_string()))?
            .clone();
        let parent_span_id = headers.get("x-parent-span-id").cloned();
        let service_name = headers
            .get("x-service-name")
            .ok_or_else(|| KiasError::ValidationError("Missing service name".to_string()))?
            .clone();
        let operation_name = headers
            .get("x-operation-name")
            .ok_or_else(|| KiasError::ValidationError("Missing operation name".to_string()))?
            .clone();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Ok(Self {
            trace_id: TraceId::from_string(trace_id),
            span_id: SpanId::from_string(span_id),
            parent_span_id: parent_span_id.map(SpanId::from_string),
            service_name,
            operation_name,
            start_time: now,
            attributes: HashMap::new(),
        })
    }
}

/// Span naming conventions
#[derive(Debug, Clone)]
pub struct SpanName {
    pub service: String,
    pub component: String,
    pub operation: String,
}

impl SpanName {
    /// Create a new span name
    pub fn new(service: String, component: String, operation: String) -> Self {
        Self {
            service,
            component,
            operation,
        }
    }

    /// Format span name as "service/component/operation"
    pub fn to_string(&self) -> String {
        format!("{}/{}/{}", self.service, self.component, self.operation)
    }

    /// Format span name as "service.component.operation"
    pub fn to_qualified_name(&self) -> String {
        format!("{}.{}.{}", self.service, self.component, self.operation)
    }
}

impl fmt::Display for SpanName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Metric naming conventions
#[derive(Debug, Clone)]
pub struct MetricName {
    pub domain: String,        // e.g., "kias", "user", "system"
    pub service: String,
    pub subsystem: Option<String>,
    pub name: String,
    pub unit: Option<String>,
}

impl MetricName {
    /// Create a new metric name
    pub fn new(domain: String, service: String, name: String) -> Self {
        Self {
            domain,
            service,
            subsystem: None,
            name,
            unit: None,
        }
    }

    /// Add subsystem
    pub fn with_subsystem(mut self, subsystem: String) -> Self {
        self.subsystem = Some(subsystem);
        self
    }

    /// Add unit
    pub fn with_unit(mut self, unit: String) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Format metric name as "domain.service.name"
    pub fn to_prometheus_name(&self) -> String {
        match self.subsystem {
            Some(ref sub) => format!("{}_{}_{}_{}", self.domain, self.service, sub, self.name),
            None => format!("{}_{}_{}", self.domain, self.service, self.name),
        }
    }

    /// Format metric name with unit suffix
    pub fn to_prometheus_name_with_unit(&self) -> String {
        let base = self.to_prometheus_name();
        match self.unit {
            Some(ref u) => format!("{}_{}", base, u),
            None => base,
        }
    }

    /// Format for statsd/dogstatsd
    pub fn to_statsd_name(&self) -> String {
        match self.subsystem {
            Some(ref sub) => format!("{}.{}.{}.{}", self.domain, self.service, sub, self.name),
            None => format!("{}.{}.{}", self.domain, self.service, self.name),
        }
    }
}

impl fmt::Display for MetricName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_prometheus_name())
    }
}

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Unified log schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSchema {
    pub timestamp: String,
    pub level: LogLevel,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub service: String,
    pub operation: String,
    pub message: String,
    pub attributes: HashMap<String, serde_json::Value>,
    pub error: Option<LogError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogError {
    pub message: String,
    pub code: Option<String>,
    pub stack_trace: Option<String>,
}

impl LogSchema {
    /// Create a new log entry
    pub fn new(service: String, operation: String, level: LogLevel, message: String) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            timestamp,
            level,
            trace_id: None,
            span_id: None,
            service,
            operation,
            message,
            attributes: HashMap::new(),
            error: None,
        }
    }

    /// Attach trace context
    pub fn with_trace_context(mut self, context: &TraceContext) -> Self {
        self.trace_id = Some(context.trace_id.as_str().to_string());
        self.span_id = Some(context.span_id.as_str().to_string());
        self
    }

    /// Add attribute
    pub fn with_attribute<T: Serialize>(mut self, key: String, value: T) -> Result<Self, KiasError> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| KiasError::SerializationError(e.to_string()))?;
        self.attributes.insert(key, json_value);
        Ok(self)
    }

    /// Add error information
    pub fn with_error(mut self, error: LogError) -> Self {
        self.error = Some(error);
        self
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, KiasError> {
        serde_json::to_string(self)
            .map_err(|e| KiasError::SerializationError(e.to_string()))
    }

    /// Parse from JSON
    pub fn from_json(json: &str) -> Result<Self, KiasError> {
        serde_json::from_str(json)
            .map_err(|e| KiasError::DeserializationError(e.to_string()))
    }
}

/// Observability manager for centralized control
pub struct ObservabilityManager {
    service_name: String,
    exporter: Arc<dyn LogExporter>,
}

pub trait LogExporter: Send + Sync {
    fn export(&self, log: &LogSchema) -> Result<(), KiasError>;
    fn flush(&self) -> Result<(), KiasError>;
}

/// Console exporter for development/testing
pub struct ConsoleExporter;

impl LogExporter for ConsoleExporter {
    fn export(&self, log: &LogSchema) -> Result<(), KiasError> {
        println!("[{}] {} - {}: {} | trace_id={:?} span_id={:?}",
            log.timestamp, log.level, log.service, log.operation, log.trace_id, log.span_id);
        if let Some(ref err) = log.error {
            println!("  Error: {} (code: {:?})", err.message, err.code);
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), KiasError> {
        Ok(())
    }
}

impl ObservabilityManager {
    /// Create a new observability manager
    pub fn new(service_name: String) -> Self {
        Self {
            service_name,
            exporter: Arc::new(ConsoleExporter),
        }
    }

    /// Set custom exporter
    pub fn with_exporter(mut self, exporter: Arc<dyn LogExporter>) -> Self {
        self.exporter = exporter;
        self
    }

    /// Create a root trace context
    pub fn start_trace(&self, operation_name: String) -> TraceContext {
        TraceContext::new_root(self.service_name.clone(), operation_name)
    }

    /// Create a child span
    pub fn start_span(&self, parent: &TraceContext, operation_name: String) -> TraceContext {
        parent.child_span(operation_name)
    }

    /// Log an entry
    pub fn log(&self, context: &TraceContext, level: LogLevel, message: String) -> Result<(), KiasError> {
        let log_entry = LogSchema::new(
            context.service_name.clone(),
            context.operation_name.clone(),
            level,
            message,
        ).with_trace_context(context);

        self.exporter.export(&log_entry)
    }

    /// Create a metric name builder
    pub fn metric(&self, name: String) -> MetricName {
        MetricName::new("kias".to_string(), self.service_name.clone(), name)
    }
}

/// Builder for creating span names
pub struct SpanNameBuilder {
    service: String,
    component: String,
    operation: String,
}

impl SpanNameBuilder {
    pub fn new(service: String) -> Self {
        Self {
            service,
            component: String::new(),
            operation: String::new(),
        }
    }

    pub fn component(mut self, component: String) -> Self {
        self.component = component;
        self
    }

    pub fn operation(mut self, operation: String) -> Self {
        self.operation = operation;
        self
    }

    pub fn build(self) -> SpanName {
        SpanName::new(self.service, self.component, self.operation)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let trace_id = TraceId::new();
        assert!(!trace_id.as_str().is_empty());
        
        let trace_id2 = TraceId::new();
        assert_ne!(trace_id.as_str(), trace_id2.as_str());
    }

    #[test]
    fn test_span_id_generation() {
        let span_id = SpanId::new();
        assert!(!span_id.as_str().is_empty());
        assert_eq!(span_id.as_str().len(), 16);
    }

    #[test]
    fn test_trace_context_creation() {
        let context = TraceContext::new_root("test-service".to_string(), "test-operation".to_string());
        
        assert_eq!(context.service_name, "test-service");
        assert_eq!(context.operation_name, "test-operation");
        assert!(context.parent_span_id.is_none());
        assert!(context.attributes.is_empty());
    }

    #[test]
    fn test_trace_context_propagation() {
        let parent = TraceContext::new_root("test-service".to_string(), "test-operation".to_string());
        let child = parent.child_span("child-operation".to_string());
        
        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_eq!(child.operation_name, "child-operation");
    }

    #[test]
    fn test_headers_propagation() {
        let context = TraceContext::new_root("test-service".to_string(), "test-operation".to_string());
        let headers = context.to_headers();
        
        assert_eq!(headers.get("x-trace-id").unwrap(), context.trace_id.as_str());
        assert_eq!(headers.get("x-span-id").unwrap(), context.span_id.as_str());
        assert_eq!(headers.get("x-service-name").unwrap(), "test-service");
    }

    #[test]
    fn test_span_name_formatting() {
        let span_name = SpanName::new("user-service".to_string(), "api".to_string(), "getUser".to_string());
        
        assert_eq!(span_name.to_string(), "user-service/api/getUser");
        assert_eq!(span_name.to_qualified_name(), "user-service.api.getUser");
    }

    #[test]
    fn test_metric_name_formatting() {
        let metric = MetricName::new("kias".to_string(), "user-service".to_string(), "request_duration".to_string())
            .with_subsystem("api".to_string())
            .with_unit("seconds".to_string());
        
        assert_eq!(metric.to_prometheus_name(), "kias_user-service_api_request_duration");
        assert_eq!(metric.to_prometheus_name_with_unit(), "kias_user-service_api_request_duration_seconds");
        assert_eq!(metric.to_statsd_name(), "kias.user-service.api.request_duration");
    }

    #[test]
    fn test_log_schema_serialization() {
        let log = LogSchema::new(
            "test-service".to_string(),
            "test-operation".to_string(),
            LogLevel::Info,
            "Test message".to_string(),
        ).with_attribute("key".to_string(), "value".to_string()).unwrap();
        
        let json = log.to_json().unwrap();
        let parsed = LogSchema::from_json(&json).unwrap();
        
        assert_eq!(parsed.service, "test-service");
        assert_eq!(parsed.operation, "test-operation");
        assert_eq!(parsed.level, LogLevel::Info);
        assert_eq!(parsed.message, "Test message");
    }

    #[test]
    fn test_log_with_trace_context() {
        let context = TraceContext::new_root("test-service".to_string(), "test-operation".to_string());
        let log = LogSchema::new(
            "test-service".to_string(),
            "test-operation".to_string(),
            LogLevel::Error,
            "Error occurred".to_string(),
        ).with_trace_context(&context);
        
        assert_eq!(log.trace_id, Some(context.trace_id.as_str().to_string()));
        assert_eq!(log.span_id, Some(context.span_id.as_str().to_string()));
    }

    #[test]
    fn test_log_with_error() {
        let error = LogError {
            message: "Something went wrong".to_string(),
            code: Some("ERR_001".to_string()),
            stack_trace: None,
        };
        
        let log = LogSchema::new(
            "test-service".to_string(),
            "test-operation".to_string(),
            LogLevel::Error,
            "Operation failed".to_string(),
        ).with_error(error);
        
        assert!(log.error.is_some());
        let err = log.error.unwrap();
        assert_eq!(err.message, "Something went wrong");
        assert_eq!(err.code, Some("ERR_001".to_string()));
    }
}