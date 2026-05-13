use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Metric types following the Prometheus data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// A single histogram bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub upper_bound: f64,
    pub count: u64,
}

/// Histogram metric for tracking value distributions (latencies, sizes, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    pub buckets: Vec<HistogramBucket>,
    pub sum: f64,
    pub count: u64,
}

impl Histogram {
    /// Create a histogram with standard buckets (good for latency in ms)
    pub fn with_standard_buckets() -> Self {
        let boundaries = [1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0];
        Self::with_boundaries(&boundaries)
    }

    pub fn with_boundaries(boundaries: &[f64]) -> Self {
        let buckets = boundaries.iter().map(|&upper_bound| HistogramBucket {
            upper_bound,
            count: 0,
        }).collect();
        Self {
            buckets,
            sum: 0.0,
            count: 0,
        }
    }

    /// Record a value into the histogram
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
        for bucket in &mut self.buckets {
            if value <= bucket.upper_bound {
                bucket.count += 1;
            }
        }
    }

    /// Calculate the mean value
    pub fn mean(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum / self.count as f64 }
    }

    /// Estimate a percentile from bucket counts
    /// Returns the upper_bound of the bucket containing the percentile
    pub fn percentile(&self, p: f64) -> f64 {
        if self.count == 0 { return 0.0; }
        let target = (p / 100.0 * self.count as f64) as u64;
        let mut cumulative = 0u64;
        for bucket in &self.buckets {
            cumulative += bucket.count;
            if cumulative >= target {
                return bucket.upper_bound;
            }
        }
        self.buckets.last().map_or(0.0, |b| b.upper_bound)
    }

    /// Estimate P50 (median)
    pub fn p50(&self) -> f64 { self.percentile(50.0) }

    /// Estimate P95
    pub fn p95(&self) -> f64 { self.percentile(95.0) }

    /// Estimate P99
    pub fn p99(&self) -> f64 { self.percentile(99.0) }
}

/// Metric snapshot for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub name: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram {
        sum: f64,
        count: u64,
        p50: f64,
        p95: f64,
        p99: f64,
        buckets: Vec<HistogramBucket>,
    },
}

/// Full metrics collector with counters, gauges, and histograms
pub struct MetricsCollector {
    counters: HashMap<String, u64>,
    gauges: HashMap<String, f64>,
    histograms: HashMap<String, Histogram>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    // ===== Counters =====

    pub fn increment_counter(&mut self, name: &str, value: u64) {
        *self.counters.entry(name.to_string()).or_insert(0) += value;
    }

    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }

    // ===== Gauges =====

    pub fn set_gauge(&mut self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    pub fn get_gauge(&self, name: &str) -> f64 {
        self.gauges.get(name).copied().unwrap_or(0.0)
    }

    pub fn increment_gauge(&mut self, name: &str, delta: f64) {
        *self.gauges.entry(name.to_string()).or_insert(0.0) += delta;
    }

    pub fn decrement_gauge(&mut self, name: &str, delta: f64) {
        *self.gauges.entry(name.to_string()).or_insert(0.0) -= delta;
    }

    // ===== Histograms =====

    /// Create and register a histogram with standard latency buckets
    pub fn register_histogram(&mut self, name: &str) {
        self.histograms.insert(name.to_string(), Histogram::with_standard_buckets());
    }

    /// Create and register a histogram with custom buckets
    pub fn register_histogram_with_boundaries(&mut self, name: &str, boundaries: &[f64]) {
        self.histograms.insert(name.to_string(), Histogram::with_boundaries(boundaries));
    }

    /// Record a value into a histogram
    pub fn observe_histogram(&mut self, name: &str, value: f64) {
        if let Some(h) = self.histograms.get_mut(name) {
            h.observe(value);
        }
    }

    /// Get histogram percentile
    pub fn histogram_percentile(&self, name: &str, p: f64) -> f64 {
        self.histograms.get(name).map_or(0.0, |h| h.percentile(p))
    }

    /// Get histogram mean
    pub fn histogram_mean(&self, name: &str) -> f64 {
        self.histograms.get(name).map_or(0.0, |h| h.mean())
    }

    /// Get histogram count
    pub fn histogram_count(&self, name: &str) -> u64 {
        self.histograms.get(name).map_or(0, |h| h.count)
    }

    // ===== Export =====

    /// Export all metrics as a vector of snapshots
    pub fn export(&self) -> Vec<MetricSnapshot> {
        let mut snapshots = Vec::new();

        for (name, &value) in &self.counters {
            snapshots.push(MetricSnapshot {
                name: name.clone(),
                metric_type: MetricType::Counter,
                value: MetricValue::Counter(value),
                labels: HashMap::new(),
            });
        }

        for (name, &value) in &self.gauges {
            snapshots.push(MetricSnapshot {
                name: name.clone(),
                metric_type: MetricType::Gauge,
                value: MetricValue::Gauge(value),
                labels: HashMap::new(),
            });
        }

        for (name, hist) in &self.histograms {
            snapshots.push(MetricSnapshot {
                name: name.clone(),
                metric_type: MetricType::Histogram,
                value: MetricValue::Histogram {
                    sum: hist.sum,
                    count: hist.count,
                    p50: hist.p50(),
                    p95: hist.p95(),
                    p99: hist.p99(),
                    buckets: hist.buckets.clone(),
                },
                labels: HashMap::new(),
            });
        }

        snapshots
    }

    /// Export as JSON string
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.export()).unwrap_or_else(|_| "[]".to_string())
    }

    /// Export in Prometheus text exposition format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        for (name, &value) in &self.counters {
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }

        for (name, &value) in &self.gauges {
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }

        for (name, hist) in &self.histograms {
            output.push_str(&format!("# TYPE {} histogram\n", name));
            for bucket in &hist.buckets {
                output.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", name, bucket.upper_bound, bucket.count));
            }
            output.push_str(&format!("{}_sum {}\n", name, hist.sum));
            output.push_str(&format!("{}_count {}\n", name, hist.count));
        }

        output
    }

    // ===== Introspection =====

    pub fn counter_names(&self) -> Vec<String> {
        self.counters.keys().cloned().collect()
    }

    pub fn gauge_names(&self) -> Vec<String> {
        self.gauges.keys().cloned().collect()
    }

    pub fn histogram_names(&self) -> Vec<String> {
        self.histograms.keys().cloned().collect()
    }

    pub fn total_metrics(&self) -> usize {
        self.counters.len() + self.gauges.len() + self.histograms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.get_counter("any"), 0);
        assert!((collector.get_gauge("any") - 0.0).abs() < f64::EPSILON);
        assert_eq!(collector.total_metrics(), 0);
    }

    #[test]
    fn test_increment_counter() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("requests", 1);
        collector.increment_counter("requests", 5);
        assert_eq!(collector.get_counter("requests"), 6);
    }

    #[test]
    fn test_set_gauge() {
        let mut collector = MetricsCollector::new();
        collector.set_gauge("cpu_usage", 0.75);
        assert!((collector.get_gauge("cpu_usage") - 0.75).abs() < f64::EPSILON);

        collector.set_gauge("cpu_usage", 0.50);
        assert!((collector.get_gauge("cpu_usage") - 0.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_gauge_increment_decrement() {
        let mut collector = MetricsCollector::new();
        collector.set_gauge("connections", 10.0);
        collector.increment_gauge("connections", 5.0);
        assert!((collector.get_gauge("connections") - 15.0).abs() < f64::EPSILON);
        collector.decrement_gauge("connections", 3.0);
        assert!((collector.get_gauge("connections") - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_multiple_metrics() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("hits", 100);
        collector.increment_counter("misses", 5);
        collector.set_gauge("latency_ms", 42.5);

        assert_eq!(collector.get_counter("hits"), 100);
        assert_eq!(collector.get_counter("misses"), 5);
        assert!((collector.get_gauge("latency_ms") - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_basic() {
        let mut collector = MetricsCollector::new();
        collector.register_histogram("request_latency");
        collector.observe_histogram("request_latency", 10.0);
        collector.observe_histogram("request_latency", 50.0);
        collector.observe_histogram("request_latency", 100.0);
        collector.observe_histogram("request_latency", 200.0);

        assert_eq!(collector.histogram_count("request_latency"), 4);
        assert!((collector.histogram_mean("request_latency") - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_percentiles() {
        let mut collector = MetricsCollector::new();
        collector.register_histogram("latency");

        // Add 100 values: 1, 2, 3, ..., 100
        for i in 1..=100 {
            collector.observe_histogram("latency", i as f64);
        }

        let p50 = collector.histogram_percentile("latency", 50.0);
        let p95 = collector.histogram_percentile("latency", 95.0);
        let p99 = collector.histogram_percentile("latency", 99.0);

        assert!(p50 > 0.0);
        assert!(p95 >= p50);
        assert!(p99 >= p95);
    }

    #[test]
    fn test_histogram_custom_buckets() {
        let mut collector = MetricsCollector::new();
        collector.register_histogram_with_boundaries("size_bytes", &[100.0, 1000.0, 10000.0, 100000.0]);
        collector.observe_histogram("size_bytes", 500.0);
        collector.observe_histogram("size_bytes", 5000.0);
        assert_eq!(collector.histogram_count("size_bytes"), 2);
    }

    #[test]
    fn test_histogram_p50_p95_p99() {
        let mut hist = Histogram::with_standard_buckets();
        for i in 1..=100 {
            hist.observe(i as f64);
        }
        assert!(hist.p50() > 0.0);
        assert!(hist.p95() > 0.0);
        assert!(hist.p99() > 0.0);
    }

    #[test]
    fn test_export_snapshots() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("requests", 42);
        collector.set_gauge("cpu", 0.5);
        collector.register_histogram("latency");
        collector.observe_histogram("latency", 100.0);

        let snapshots = collector.export();
        assert_eq!(snapshots.len(), 3);

        let counter_snap = snapshots.iter().find(|s| s.name == "requests").unwrap();
        assert!(matches!(counter_snap.value, MetricValue::Counter(42)));
    }

    #[test]
    fn test_export_json() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("test", 1);
        let json = collector.export_json();
        assert!(json.contains("\"test\""));
        assert!(json.contains("1"));
    }

    #[test]
    fn test_export_prometheus() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("http_requests_total", 100);
        collector.set_gauge("memory_usage_bytes", 1024.0);
        collector.register_histogram("request_duration_ms");
        collector.observe_histogram("request_duration_ms", 50.0);

        let prom = collector.export_prometheus();
        assert!(prom.contains("# TYPE http_requests_total counter"));
        assert!(prom.contains("http_requests_total 100"));
        assert!(prom.contains("# TYPE memory_usage_bytes gauge"));
        assert!(prom.contains("# TYPE request_duration_ms histogram"));
        assert!(prom.contains("request_duration_ms_bucket"));
    }

    #[test]
    fn test_metric_introspection() {
        let mut collector = MetricsCollector::new();
        collector.increment_counter("a", 1);
        collector.set_gauge("b", 2.0);
        collector.register_histogram("c");

        assert_eq!(collector.counter_names().len(), 1);
        assert_eq!(collector.gauge_names().len(), 1);
        assert_eq!(collector.histogram_names().len(), 1);
        assert_eq!(collector.total_metrics(), 3);
    }
}
