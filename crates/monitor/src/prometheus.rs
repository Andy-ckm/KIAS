use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Prometheus metric family (group of metrics with same name)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFamily {
    pub name: String,
    pub help: String,
    pub metric_type: PrometheusType,
    pub metrics: Vec<PrometheusMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PrometheusType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// A single Prometheus metric with labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusMetric {
    pub labels: HashMap<String, String>,
    pub value: PrometheusValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrometheusValue {
    Counter(u64),
    Gauge(f64),
    Histogram {
        buckets: Vec<(f64, u64)>,
        sum: f64,
        count: u64,
    },
}

/// Prometheus metrics registry
pub struct PrometheusRegistry {
    families: Vec<MetricFamily>,
}

impl Default for PrometheusRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PrometheusRegistry {
    pub fn new() -> Self {
        Self { families: Vec::new() }
    }

    pub fn register_counter(&mut self, name: &str, help: &str) {
        self.families.push(MetricFamily {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: PrometheusType::Counter,
            metrics: Vec::new(),
        });
    }

    pub fn register_gauge(&mut self, name: &str, help: &str) {
        self.families.push(MetricFamily {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: PrometheusType::Gauge,
            metrics: Vec::new(),
        });
    }

    pub fn register_histogram(&mut self, name: &str, help: &str) {
        self.families.push(MetricFamily {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: PrometheusType::Histogram,
            metrics: Vec::new(),
        });
    }

    pub fn set_counter(&mut self, name: &str, labels: HashMap<String, String>, value: u64) {
        if let Some(family) = self.families.iter_mut().find(|f| f.name == name) {
            if let Some(metric) = family.metrics.iter_mut().find(|m| m.labels == labels) {
                metric.value = PrometheusValue::Counter(value);
            } else {
                family.metrics.push(PrometheusMetric {
                    labels,
                    value: PrometheusValue::Counter(value),
                });
            }
        }
    }

    pub fn set_gauge(&mut self, name: &str, labels: HashMap<String, String>, value: f64) {
        if let Some(family) = self.families.iter_mut().find(|f| f.name == name) {
            if let Some(metric) = family.metrics.iter_mut().find(|m| m.labels == labels) {
                metric.value = PrometheusValue::Gauge(value);
            } else {
                family.metrics.push(PrometheusMetric {
                    labels,
                    value: PrometheusValue::Gauge(value),
                });
            }
        }
    }

    /// Render all metrics in Prometheus text exposition format
    pub fn render(&self) -> String {
        let mut output = String::new();

        for family in &self.families {
            output.push_str(&format!("# HELP {} {}\n", family.name, family.help));
            output.push_str(&format!("# TYPE {} {}\n", family.name, prom_type_str(&family.metric_type)));

            for metric in &family.metrics {
                let labels_str = render_labels(&metric.labels);
                match &metric.value {
                    PrometheusValue::Counter(v) => {
                        output.push_str(&format!("{}{} {}\n", family.name, labels_str, v));
                    }
                    PrometheusValue::Gauge(v) => {
                        output.push_str(&format!("{}{} {}\n", family.name, labels_str, v));
                    }
                    PrometheusValue::Histogram { buckets, sum, count } => {
                        for (le, count) in buckets {
                            output.push_str(&format!("{}{}_bucket{{le=\"{}\",{}}} {}\n",
                                family.name, labels_str.trim_start_matches('{').trim_end_matches('}'),
                                le, labels_str.trim_start_matches('{').trim_end_matches('}'), count));
                        }
                        output.push_str(&format!("{}_sum{} {}\n", family.name, labels_str, sum));
                        output.push_str(&format!("{}_count{} {}\n", family.name, labels_str, count));
                    }
                }
            }
        }

        output
    }

    pub fn families(&self) -> &[MetricFamily] {
        &self.families
    }

    pub fn clear(&mut self) {
        for family in &mut self.families {
            family.metrics.clear();
        }
    }
}

fn prom_type_str(t: &PrometheusType) -> &'static str {
    match t {
        PrometheusType::Counter => "counter",
        PrometheusType::Gauge => "gauge",
        PrometheusType::Histogram => "histogram",
        PrometheusType::Summary => "summary",
    }
}

fn render_labels(labels: &HashMap<String, String>) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let pairs: Vec<String> = labels.iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect();
    format!("{{{}}}", pairs.join(","))
}

/// KIAS-specific metric names following Prometheus conventions
pub mod kias_metrics {
    pub const AGENTS_TOTAL: &str = "kias_agents_total";
    pub const AGENTS_ACTIVE: &str = "kias_agents_active";
    pub const AGENTS_HEALTHY: &str = "kias_agents_healthy";
    pub const TASKS_TOTAL: &str = "kias_tasks_total";
    pub const TASKS_SUCCESS: &str = "kias_tasks_success_total";
    pub const TASKS_FAILED: &str = "kias_tasks_failed_total";
    pub const TASK_DURATION_MS: &str = "kias_task_duration_milliseconds";
    pub const SCHEDULER_DECISIONS: &str = "kias_scheduler_decisions_total";
    pub const CACHE_HITS: &str = "kias_cache_hits_total";
    pub const CACHE_MISSES: &str = "kias_cache_misses_total";
    pub const TOKENS_USED: &str = "kias_tokens_used_total";
    pub const API_REQUESTS: &str = "kias_api_requests_total";
    pub const API_LATENCY_MS: &str = "kias_api_latency_milliseconds";
    pub const MEMORY_BYTES: &str = "kias_memory_usage_bytes";
    pub const CPU_PERCENT: &str = "kias_cpu_usage_percent";
}

/// Build KIAS-specific Prometheus registry with standard metrics
pub fn build_kias_registry() -> PrometheusRegistry {
    use kias_metrics::*;

    let mut registry = PrometheusRegistry::new();
    registry.register_gauge(AGENTS_TOTAL, "Total number of registered agents");
    registry.register_gauge(AGENTS_ACTIVE, "Number of currently active agents");
    registry.register_gauge(AGENTS_HEALTHY, "Number of healthy agents");
    registry.register_counter(TASKS_TOTAL, "Total number of tasks processed");
    registry.register_counter(TASKS_SUCCESS, "Total number of successful tasks");
    registry.register_counter(TASKS_FAILED, "Total number of failed tasks");
    registry.register_histogram(TASK_DURATION_MS, "Task execution duration in milliseconds");
    registry.register_counter(SCHEDULER_DECISIONS, "Total scheduler decisions made");
    registry.register_counter(CACHE_HITS, "Total cache hits");
    registry.register_counter(CACHE_MISSES, "Total cache misses");
    registry.register_counter(TOKENS_USED, "Total tokens consumed");
    registry.register_counter(API_REQUESTS, "Total API requests");
    registry.register_histogram(API_LATENCY_MS, "API request latency in milliseconds");
    registry.register_gauge(MEMORY_BYTES, "Memory usage in bytes");
    registry.register_gauge(CPU_PERCENT, "CPU usage percentage");
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = PrometheusRegistry::new();
        assert_eq!(registry.families().len(), 0);
    }

    #[test]
    fn test_register_counter() {
        let mut registry = PrometheusRegistry::new();
        registry.register_counter("test_total", "A test counter");
        assert_eq!(registry.families().len(), 1);
        assert_eq!(registry.families()[0].metric_type, PrometheusType::Counter);
    }

    #[test]
    fn test_set_counter_value() {
        let mut registry = PrometheusRegistry::new();
        registry.register_counter("req_total", "Requests");
        registry.set_counter("req_total", HashMap::new(), 42);
        let output = registry.render();
        assert!(output.contains("req_total 42"));
    }

    #[test]
    fn test_set_gauge_with_labels() {
        let mut registry = PrometheusRegistry::new();
        registry.register_gauge("cpu_usage", "CPU");
        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "node1".to_string());
        registry.set_gauge("cpu_usage", labels, 75.5);
        let output = registry.render();
        assert!(output.contains("host=\"node1\""));
        assert!(output.contains("75.5"));
    }

    #[test]
    fn test_render_prometheus_format() {
        let mut registry = PrometheusRegistry::new();
        registry.register_counter("http_requests_total", "Total HTTP requests");
        registry.register_gauge("memory_bytes", "Memory usage");
        registry.set_counter("http_requests_total", HashMap::new(), 100);
        registry.set_gauge("memory_bytes", HashMap::new(), 1024.0);

        let output = registry.render();
        assert!(output.contains("# HELP http_requests_total Total HTTP requests"));
        assert!(output.contains("# TYPE http_requests_total counter"));
        assert!(output.contains("http_requests_total 100"));
        assert!(output.contains("# TYPE memory_bytes gauge"));
        assert!(output.contains("memory_bytes 1024"));
    }

    #[test]
    fn test_update_existing_metric() {
        let mut registry = PrometheusRegistry::new();
        registry.register_gauge("temp", "temperature");
        registry.set_gauge("temp", HashMap::new(), 20.0);
        registry.set_gauge("temp", HashMap::new(), 25.0);
        let output = registry.render();
        assert!(output.contains("temp 25"));
        assert!(!output.contains("temp 20"));
    }

    #[test]
    fn test_kias_registry() {
        let registry = build_kias_registry();
        assert!(registry.families().len() >= 15);
        let names: Vec<&str> = registry.families().iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"kias_agents_total"));
        assert!(names.contains(&"kias_tasks_total"));
        assert!(names.contains(&"kias_api_requests_total"));
    }

    #[test]
    fn test_render_labels_empty() {
        let labels = HashMap::new();
        assert_eq!(render_labels(&labels), "");
    }

    #[test]
    fn test_render_labels_single() {
        let mut labels = HashMap::new();
        labels.insert("env".to_string(), "prod".to_string());
        assert_eq!(render_labels(&labels), "{env=\"prod\"}");
    }

    #[test]
    fn test_render_labels_multiple() {
        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "node1".to_string());
        labels.insert("env".to_string(), "prod".to_string());
        let rendered = render_labels(&labels);
        assert!(rendered.contains("host=\"node1\""));
        assert!(rendered.contains("env=\"prod\""));
    }

    #[test]
    fn test_clear() {
        let mut registry = PrometheusRegistry::new();
        registry.register_counter("test", "test");
        registry.set_counter("test", HashMap::new(), 42);
        registry.clear();
        assert_eq!(registry.families()[0].metrics.len(), 0);
    }

    #[test]
    fn test_histogram_render() {
        let mut registry = PrometheusRegistry::new();
        registry.register_histogram("request_duration", "Request duration");
        let metric = PrometheusMetric {
            labels: HashMap::new(),
            value: PrometheusValue::Histogram {
                buckets: vec![(0.1, 10), (0.5, 50), (1.0, 90), (f64::INFINITY, 100)],
                sum: 45.5,
                count: 100,
            },
        };
        if let Some(family) = registry.families.iter_mut().find(|f| f.name == "request_duration") {
            family.metrics.push(metric);
        }
        let output = registry.render();
        assert!(output.contains("request_duration_bucket"));
        assert!(output.contains("request_duration_sum"));
        assert!(output.contains("request_duration_count"));
    }
}
