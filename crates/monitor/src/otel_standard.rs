//! # OTel Standard - OpenTelemetry Observability Conventions
//!
//! Implements standardized trace, log, and metric naming conventions
//! for consistent observability across the AgentGuard system.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trace ID format convention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceConvention {
    pub trace_id_format: String,
    pub span_name_pattern: String,
    pub attribute_prefix: String,
}

impl Default for TraceConvention {
    fn default() -> Self {
        Self {
            trace_id_format: "32 hex chars".to_string(),
            span_name_pattern: r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$".to_string(),
            attribute_prefix: "kias.".to_string(),
        }
    }
}

impl TraceConvention {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a span name against the convention
    pub fn validate_span_name(&self, name: &str) -> bool {
        let re = Regex::new(&self.span_name_pattern).expect("span_name_pattern should always be a valid regex");
        re.is_match(name)
    }

    /// Format a compliant span name
    pub fn format_span_name(&self, components: &[&str]) -> String {
        components.join(".")
    }
}

/// Log schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSchema {
    pub version: String,
    pub required_fields: Vec<String>,
    pub optional_fields: Vec<String>,
    pub severity_mapping: HashMap<String, String>,
}

impl Default for LogSchema {
    fn default() -> Self {
        let mut severity_mapping = HashMap::new();
        severity_mapping.insert("TRACE".to_string(), "debug".to_string());
        severity_mapping.insert("DEBUG".to_string(), "debug".to_string());
        severity_mapping.insert("INFO".to_string(), "info".to_string());
        severity_mapping.insert("WARN".to_string(), "warn".to_string());
        severity_mapping.insert("WARNING".to_string(), "warn".to_string());
        severity_mapping.insert("ERROR".to_string(), "error".to_string());
        severity_mapping.insert("FATAL".to_string(), "error".to_string());
        severity_mapping.insert("CRITICAL".to_string(), "error".to_string());

        Self {
            version: "1.0.0".to_string(),
            required_fields: vec![
                "timestamp".to_string(),
                "level".to_string(),
                "message".to_string(),
                "service".to_string(),
            ],
            optional_fields: vec![
                "trace_id".to_string(),
                "span_id".to_string(),
                "user_id".to_string(),
                "tenant_id".to_string(),
                "component".to_string(),
                "duration_ms".to_string(),
            ],
            severity_mapping,
        }
    }
}

impl LogSchema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate log record has all required fields
    pub fn validate(&self, record: &LogRecord) -> ValidationResult {
        let mut missing = Vec::new();

        for field in &self.required_fields {
            match field.as_str() {
                "timestamp" => {
                    if record.timestamp.is_none() {
                        missing.push(field.clone());
                    }
                }
                "level" => {
                    if record.level.is_none() {
                        missing.push(field.clone());
                    }
                }
                "message" => {
                    if record.message.is_empty() {
                        missing.push(field.clone());
                    }
                }
                "service" => {
                    if record.service.is_none() {
                        missing.push(field.clone());
                    }
                }
                _ => {}
            }
        }

        ValidationResult {
            valid: missing.is_empty(),
            missing_fields: missing,
        }
    }

    /// Normalize severity level
    pub fn normalize_severity(&self, level: &str) -> String {
        self.severity_mapping
            .get(&level.to_uppercase())
            .cloned()
            .unwrap_or_else(|| "info".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub level: Option<String>,
    pub message: String,
    pub service: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub component: Option<String>,
    pub duration_ms: Option<u64>,
}

impl LogRecord {
    pub fn new(message: &str) -> Self {
        Self {
            timestamp: Some(chrono::Utc::now()),
            level: Some("INFO".to_string()),
            message: message.to_string(),
            service: None,
            trace_id: None,
            span_id: None,
            user_id: None,
            tenant_id: None,
            component: None,
            duration_ms: None,
        }
    }

    pub fn with_level(mut self, level: &str) -> Self {
        self.level = Some(level.to_string());
        self
    }

    pub fn with_service(mut self, service: &str) -> Self {
        self.service = Some(service.to_string());
        self
    }

    pub fn with_trace(mut self, trace_id: &str, span_id: &str) -> Self {
        self.trace_id = Some(trace_id.to_string());
        self.span_id = Some(span_id.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub missing_fields: Vec<String>,
}

/// Metric naming convention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricNaming {
    pub namespace: String,
    pub unit_mapping: HashMap<String, String>,
    pub recommended_aggregations: HashMap<String, Vec<String>>,
}

impl Default for MetricNaming {
    fn default() -> Self {
        let mut unit_mapping = HashMap::new();
        unit_mapping.insert("duration".to_string(), "ms".to_string());
        unit_mapping.insert("count".to_string(), "1".to_string());
        unit_mapping.insert("size".to_string(), "By".to_string());
        unit_mapping.insert("throughput".to_string(), "req/s".to_string());
        unit_mapping.insert("percentage".to_string(), "%".to_string());
        unit_mapping.insert("temperature".to_string(), "Cel".to_string());

        let mut recommended_aggregations = HashMap::new();
        recommended_aggregations.insert(
            "latency".to_string(),
            vec!["p50".to_string(), "p95".to_string(), "p99".to_string()],
        );
        recommended_aggregations.insert(
            "throughput".to_string(),
            vec!["rate".to_string(), "total".to_string()],
        );
        recommended_aggregations.insert(
            "count".to_string(),
            vec!["total".to_string(), "delta".to_string()],
        );

        Self {
            namespace: "kias".to_string(),
            unit_mapping,
            recommended_aggregations,
        }
    }
}

impl MetricNaming {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a compliant metric name
    pub fn build_name(&self, category: &str, name: &str, unit: Option<&str>) -> String {
        let mut parts = vec![
            self.namespace.clone(),
            category.to_string(),
            name.to_string(),
        ];
        if let Some(u) = unit {
            parts.push(u.to_string());
        }
        parts.join("_")
    }

    /// Validate a metric name
    pub fn validate_name(&self, name: &str) -> bool {
        // Must start with namespace
        if !name.starts_with(&format!("{}_", self.namespace)) {
            return false;
        }
        // Only alphanumeric and underscore
        let re = Regex::new(r"^[a-z][a-z0-9_]*$").expect("metric name pattern should always be a valid regex");
        re.is_match(name)
    }

    /// Get recommended aggregations for a metric type
    pub fn get_aggregations(&self, metric_type: &str) -> Vec<String> {
        self.recommended_aggregations
            .get(metric_type)
            .cloned()
            .unwrap_or_else(|| vec!["total".to_string()])
    }
}

/// Compliance checker for observability standards
#[derive(Debug, Clone)]
pub struct OTelComplianceChecker {
    trace_convention: TraceConvention,
    log_schema: LogSchema,
    metric_naming: MetricNaming,
    violations: Vec<ComplianceViolation>,
}

impl Default for OTelComplianceChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl OTelComplianceChecker {
    pub fn new() -> Self {
        Self {
            trace_convention: TraceConvention::new(),
            log_schema: LogSchema::new(),
            metric_naming: MetricNaming::new(),
            violations: Vec::new(),
        }
    }

    /// Check span name compliance
    pub fn check_span_name(&mut self, name: &str) -> bool {
        if self.trace_convention.validate_span_name(name) {
            true
        } else {
            self.violations.push(ComplianceViolation {
                violation_type: ViolationType::InvalidSpanName,
                value: name.to_string(),
                message: format!(
                    "Span name '{}' does not match convention pattern '{}'",
                    name, self.trace_convention.span_name_pattern
                ),
            });
            false
        }
    }

    /// Check metric name compliance
    pub fn check_metric_name(&mut self, name: &str) -> bool {
        if self.metric_naming.validate_name(name) {
            true
        } else {
            self.violations.push(ComplianceViolation {
                violation_type: ViolationType::InvalidMetricName,
                value: name.to_string(),
                message: format!("Metric name '{}' does not follow naming convention", name),
            });
            false
        }
    }

    /// Check log record compliance
    pub fn check_log_record(&mut self, record: &LogRecord) -> bool {
        let result = self.log_schema.validate(record);
        if result.valid {
            true
        } else {
            for field in &result.missing_fields {
                self.violations.push(ComplianceViolation {
                    violation_type: ViolationType::MissingLogField,
                    value: field.clone(),
                    message: format!("Log record missing required field: {}", field),
                });
            }
            false
        }
    }

    /// Get all violations
    pub fn get_violations(&self) -> &[ComplianceViolation] {
        &self.violations
    }

    /// Clear violations
    pub fn clear_violations(&mut self) {
        self.violations.clear();
    }

    /// Generate compliance report
    pub fn generate_report(&self) -> String {
        if self.violations.is_empty() {
            "OTel Compliance: All checks passed".to_string()
        } else {
            format!(
                "OTel Compliance: {} violation(s) found\n\n{}",
                self.violations.len(),
                self.violations
                    .iter()
                    .enumerate()
                    .map(|(i, v)| format!("{}. {}", i + 1, v.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    InvalidSpanName,
    InvalidMetricName,
    MissingLogField,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub violation_type: ViolationType,
    pub value: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_convention_default() {
        let conv = TraceConvention::default();
        assert_eq!(conv.trace_id_format, "32 hex chars");
        assert!(conv.validate_span_name("api.server.request"));
        assert!(conv.validate_span_name("workflow.engine.execute"));
        assert!(!conv.validate_span_name("InvalidName"));
        assert!(!conv.validate_span_name("123invalid"));
    }

    #[test]
    fn test_trace_convention_span_name_formatting() {
        let conv = TraceConvention::new();
        let name = conv.format_span_name(&["api", "server", "request"]);
        assert_eq!(name, "api.server.request");
    }

    #[test]
    fn test_log_schema_validate_pass() {
        let schema = LogSchema::new();
        let record = LogRecord::new("test message")
            .with_level("INFO")
            .with_service("api-server");
        assert!(schema.validate(&record).valid);
    }

    #[test]
    fn test_log_schema_validate_missing_fields() {
        let schema = LogSchema::new();
        let record = LogRecord {
            timestamp: None,
            level: None,
            message: "test".to_string(),
            service: None,
            trace_id: None,
            span_id: None,
            user_id: None,
            tenant_id: None,
            component: None,
            duration_ms: None,
        };
        let result = schema.validate(&record);
        assert!(!result.valid);
        assert!(result.missing_fields.contains(&"timestamp".to_string()));
        assert!(result.missing_fields.contains(&"level".to_string()));
        assert!(result.missing_fields.contains(&"service".to_string()));
    }

    #[test]
    fn test_log_schema_severity_normalization() {
        let schema = LogSchema::new();
        assert_eq!(schema.normalize_severity("DEBUG"), "debug");
        assert_eq!(schema.normalize_severity("WARN"), "warn");
        assert_eq!(schema.normalize_severity("WARNING"), "warn");
        assert_eq!(schema.normalize_severity("ERROR"), "error");
        assert_eq!(schema.normalize_severity("UNKNOWN"), "info"); // Default
    }

    #[test]
    fn test_log_record_builder() {
        let record = LogRecord::new("Test message")
            .with_level("ERROR")
            .with_service("scheduler")
            .with_trace("abc123", "span456");

        assert_eq!(record.message, "Test message");
        assert_eq!(record.level, Some("ERROR".to_string()));
        assert_eq!(record.service, Some("scheduler".to_string()));
        assert_eq!(record.trace_id, Some("abc123".to_string()));
        assert_eq!(record.span_id, Some("span456".to_string()));
    }

    #[test]
    fn test_metric_naming_build_name() {
        let naming = MetricNaming::new();
        let name = naming.build_name("api", "latency", Some("ms"));
        assert_eq!(name, "kias_api_latency_ms");
    }

    #[test]
    fn test_metric_naming_validate() {
        let naming = MetricNaming::new();
        assert!(naming.validate_name("kias_api_latency_ms"));
        assert!(naming.validate_name("kias_scheduler_queue_size"));
        assert!(!naming.validate_name("invalid_name"));
        assert!(!naming.validate_name("kias")); // Missing parts
    }

    #[test]
    fn test_metric_naming_aggregations() {
        let naming = MetricNaming::new();
        let latency_aggs = naming.get_aggregations("latency");
        assert!(latency_aggs.contains(&"p50".to_string()));
        assert!(latency_aggs.contains(&"p95".to_string()));
        assert!(latency_aggs.contains(&"p99".to_string()));
    }

    #[test]
    fn test_otel_checker_span_name_compliance() {
        let mut checker = OTelComplianceChecker::new();
        assert!(checker.check_span_name("api.server.request"));
        assert!(!checker.check_span_name("InvalidName"));
        assert_eq!(checker.violations.len(), 1);
    }

    #[test]
    fn test_otel_checker_metric_name_compliance() {
        let mut checker = OTelComplianceChecker::new();
        assert!(checker.check_metric_name("kias_api_latency_ms"));
        assert!(!checker.check_metric_name("invalid"));
        assert_eq!(checker.violations.len(), 1);
    }

    #[test]
    fn test_otel_checker_log_record_compliance() {
        let mut checker = OTelComplianceChecker::new();
        let good_record = LogRecord::new("test")
            .with_level("INFO")
            .with_service("test");
        assert!(checker.check_log_record(&good_record));

        let bad_record = LogRecord {
            timestamp: None,
            level: None,
            message: "test".to_string(),
            service: None,
            trace_id: None,
            span_id: None,
            user_id: None,
            tenant_id: None,
            component: None,
            duration_ms: None,
        };
        assert!(!checker.check_log_record(&bad_record));
        assert!(checker.violations.len() >= 3);
    }

    #[test]
    fn test_otel_checker_report() {
        let checker = OTelComplianceChecker::new();
        let report = checker.generate_report();
        assert!(report.contains("All checks passed"));
    }

    #[test]
    fn test_otel_checker_clear_violations() {
        let mut checker = OTelComplianceChecker::new();
        checker.check_span_name("Invalid");
        assert!(!checker.violations.is_empty());
        checker.clear_violations();
        assert!(checker.violations.is_empty());
    }
}
