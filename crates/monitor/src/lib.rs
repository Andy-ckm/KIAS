pub mod agent_span;
pub mod alert;
pub mod anomaly_detection;
pub mod metrics;
pub mod prometheus;
pub mod slow_trace;
pub mod span_collector;
pub mod telemetry;
pub mod time_travel_debugger;
// pub // mod tail_latency; // TODO: fix compilation // TODO: fix compilation

// pub use tail_latency::{JitterSuppressor, LatencySample, LatencyTracker, PercentileBreakdown, SlidingWindow, SlidingWindowConfig}; // TODO: fix

pub use alert::{
    AlertCondition, AlertInstance, AlertManager, AlertRule, AlertSeverity, AlertState,
};
pub use metrics::{Histogram, MetricSnapshot, MetricType, MetricValue, MetricsCollector};
pub use prometheus::{
    build_kias_registry, kias_metrics, MetricFamily, PrometheusMetric, PrometheusRegistry,
    PrometheusType, PrometheusValue,
};
pub use slow_trace::{
    ActionCategory, RootCauseHint, SlowAgentSummary, SlowSeverity, SlowTrace, SlowTraceCollector,
    SlowTraceConfig, SlowTraceSummary,
};
pub use telemetry::{
    EventFilter, EventStats, EventType, Severity, TelemetryCollector, TelemetryEvent,
};

// pub // mod health_monitor; // TODO: fix compilation // TODO: fix compilation
// pub // mod anomaly_detector; // TODO: fix compilation // TODO: fix compilation
// pub // mod latency_governor; // TODO: fix compilation // TODO: fix compilation
// pub // mod regression_gate; // TODO: fix compilation // TODO: fix compilation
// pub // mod observability; // TODO: fix compilation // TODO: fix compilation
pub mod otel_standard;
