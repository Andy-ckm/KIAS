//! MCP Metrics Collection
//!
//! Provides:
//! - Request/response metrics
//! - Latency histograms
//! - Error rate tracking
//! - Tool usage statistics
//! - Resource access patterns
//! - Prometheus-compatible export

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Metric Types
// ---------------------------------------------------------------------------

/// Metric value types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MetricValue {
    /// Counter (monotonically increasing).
    Counter { value: u64 },
    /// Gauge (can go up and down).
    Gauge { value: f64 },
    /// Histogram (distribution of values).
    Histogram {
        count: u64,
        sum: f64,
        buckets: Vec<HistogramBucket>,
    },
    /// Summary (quantiles).
    Summary {
        count: u64,
        sum: f64,
        quantiles: Vec<Quantile>,
    },
}

/// Histogram bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    /// Upper bound.
    pub le: f64,
    /// Count of observations ≤ le.
    pub count: u64,
}

/// Summary quantile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantile {
    /// Quantile (0.0 to 1.0).
    pub quantile: f64,
    /// Value at this quantile.
    pub value: f64,
}

/// Metric labels.
pub type Labels = HashMap<String, String>;

/// A single metric sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    /// Metric name.
    pub name: String,
    /// Labels.
    pub labels: Labels,
    /// Value.
    pub value: MetricValue,
    /// Timestamp (Unix millis).
    pub timestamp_ms: u64,
    /// Help text.
    pub help: String,
}

// ---------------------------------------------------------------------------
// Metrics Collector
// ---------------------------------------------------------------------------

/// Configuration for metrics collection.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Enable metrics collection.
    pub enabled: bool,
    /// Histogram buckets for latency (milliseconds).
    pub latency_buckets: Vec<f64>,
    /// Retention period for detailed samples.
    pub retention: Duration,
    /// Enable per-tool metrics.
    pub per_tool_metrics: bool,
    /// Enable per-client metrics.
    pub per_client_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            latency_buckets: vec![
                1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
            ],
            retention: Duration::from_secs(3600), // 1 hour
            per_tool_metrics: true,
            per_client_metrics: true,
        }
    }
}

/// Request metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetrics {
    /// Total requests.
    pub total: u64,
    /// Successful requests.
    pub success: u64,
    /// Failed requests.
    pub failed: u64,
    /// Active requests (in-flight).
    pub active: u64,
    /// Request rate (requests per second, exponential moving average).
    pub rate: f64,
}

/// Latency metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Minimum latency (micros).
    pub min_us: u64,
    /// Maximum latency (micros).
    pub max_us: u64,
    /// Average latency (micros).
    pub avg_us: f64,
    /// P50 latency (micros).
    pub p50_us: f64,
    /// P90 latency (micros).
    pub p90_us: f64,
    /// P95 latency (micros).
    pub p95_us: f64,
    /// P99 latency (micros).
    pub p99_us: f64,
    /// Total observations.
    pub count: u64,
}

/// Tool usage metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolMetrics {
    /// Tool name.
    pub name: String,
    /// Total invocations.
    pub invocations: u64,
    /// Successful invocations.
    pub successes: u64,
    /// Failed invocations.
    pub failures: u64,
    /// Average latency (micros).
    pub avg_latency_us: f64,
    /// Error rate (0.0 to 1.0).
    pub error_rate: f64,
}

/// System metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Uptime (seconds).
    pub uptime_secs: u64,
    /// Memory usage (bytes).
    pub memory_bytes: u64,
    /// CPU usage (0.0 to 1.0).
    pub cpu_usage: f64,
    /// Open connections.
    pub open_connections: u64,
    /// Goroutine/thread count.
    pub thread_count: u64,
}

/// Comprehensive metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// When the snapshot was taken.
    pub timestamp_ms: u64,
    /// Request metrics.
    pub requests: RequestMetrics,
    /// Latency metrics.
    pub latency: LatencyMetrics,
    /// Per-tool metrics.
    pub tools: Vec<ToolMetrics>,
    /// System metrics.
    pub system: SystemMetrics,
    /// Custom counters.
    pub counters: HashMap<String, u64>,
    /// Custom gauges.
    pub gauges: HashMap<String, f64>,
}

/// Metrics collector.
pub struct MetricsCollector {
    /// Configuration.
    config: MetricsConfig,
    /// Start time.
    start_time: Instant,
    /// Request metrics.
    requests: Arc<RwLock<RequestMetrics>>,
    /// Latency samples (ring buffer).
    latency_samples: Arc<RwLock<Vec<u64>>>,
    /// Per-tool metrics.
    tool_metrics: Arc<RwLock<HashMap<String, ToolMetrics>>>,
    /// Custom counters.
    counters: Arc<RwLock<HashMap<String, u64>>>,
    /// Custom gauges.
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    /// Request timestamps for rate calculation.
    request_times: Arc<RwLock<Vec<Instant>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            config,
            start_time: Instant::now(),
            requests: Arc::new(RwLock::new(RequestMetrics::default())),
            latency_samples: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            tool_metrics: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            request_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(MetricsConfig::default())
    }

    /// Record a request start.
    pub async fn request_start(&self) {
        if !self.config.enabled {
            return;
        }

        let mut requests = self.requests.write().await;
        requests.total += 1;
        requests.active += 1;

        let now = Instant::now();
        let mut times = self.request_times.write().await;
        times.push(now);

        // Calculate rate (requests in last 60 seconds)
        let cutoff = now - Duration::from_secs(60);
        times.retain(|t| *t >= cutoff);
        requests.rate = times.len() as f64 / 60.0;
    }

    /// Record a request completion.
    pub async fn request_end(&self, latency: Duration, success: bool, tool: Option<&str>) {
        if !self.config.enabled {
            return;
        }

        let latency_us = latency.as_micros() as u64;

        // Update request metrics
        let mut requests = self.requests.write().await;
        requests.active = requests.active.saturating_sub(1);
        if success {
            requests.success += 1;
        } else {
            requests.failed += 1;
        }
        drop(requests);

        // Record latency
        let mut samples = self.latency_samples.write().await;
        if samples.len() >= 10000 {
            samples.remove(0);
        }
        samples.push(latency_us);
        drop(samples);

        // Record tool metrics
        if let Some(tool_name) = tool {
            let mut tools = self.tool_metrics.write().await;
            let entry = tools
                .entry(tool_name.to_string())
                .or_insert_with(|| ToolMetrics {
                    name: tool_name.to_string(),
                    ..Default::default()
                });

            entry.invocations += 1;
            if success {
                entry.successes += 1;
            } else {
                entry.failures += 1;
            }
            entry.error_rate = entry.failures as f64 / entry.invocations as f64;
            entry.avg_latency_us = (entry.avg_latency_us * (entry.invocations - 1) as f64
                + latency_us as f64)
                / entry.invocations as f64;
        }
    }

    /// Increment a counter.
    pub async fn inc_counter(&self, name: &str, value: u64) {
        if !self.config.enabled {
            return;
        }

        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += value;
    }

    /// Set a gauge value.
    pub async fn set_gauge(&self, name: &str, value: f64) {
        if !self.config.enabled {
            return;
        }

        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    /// Get a comprehensive metrics snapshot.
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let requests = self.requests.read().await;
        let samples = self.latency_samples.read().await;
        let tools = self.tool_metrics.read().await;
        let counters = self.counters.read().await;
        let gauges = self.gauges.read().await;

        let latency = calculate_latency_metrics(&samples);

        MetricsSnapshot {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            requests: requests.clone(),
            latency,
            tools: tools.values().cloned().collect(),
            system: SystemMetrics {
                uptime_secs: self.start_time.elapsed().as_secs(),
                ..Default::default()
            },
            counters: counters.clone(),
            gauges: gauges.clone(),
        }
    }

    /// Export metrics in Prometheus format.
    pub async fn prometheus_export(&self) -> String {
        let snapshot = self.snapshot().await;
        let mut output = String::new();

        // Request metrics
        output.push_str("# HELP mcp_requests_total Total requests\n");
        output.push_str("# TYPE mcp_requests_total counter\n");
        output.push_str(&format!("mcp_requests_total {}\n", snapshot.requests.total));

        output.push_str("# HELP mcp_requests_active Active requests\n");
        output.push_str("# TYPE mcp_requests_active gauge\n");
        output.push_str(&format!(
            "mcp_requests_active {}\n",
            snapshot.requests.active
        ));

        output.push_str("# HELP mcp_request_rate Requests per second\n");
        output.push_str("# TYPE mcp_request_rate gauge\n");
        output.push_str(&format!("mcp_request_rate {}\n", snapshot.requests.rate));

        // Latency metrics
        output.push_str("# HELP mcp_latency_us Request latency in microseconds\n");
        output.push_str("# TYPE mcp_latency_us summary\n");
        output.push_str(&format!(
            "mcp_latency_us {{quantile=\"0.5\"}} {}\n",
            snapshot.latency.p50_us
        ));
        output.push_str(&format!(
            "mcp_latency_us {{quantile=\"0.9\"}} {}\n",
            snapshot.latency.p90_us
        ));
        output.push_str(&format!(
            "mcp_latency_us {{quantile=\"0.95\"}} {}\n",
            snapshot.latency.p95_us
        ));
        output.push_str(&format!(
            "mcp_latency_us {{quantile=\"0.99\"}} {}\n",
            snapshot.latency.p99_us
        ));

        // Per-tool metrics
        for tool in &snapshot.tools {
            output.push_str(&format!(
                "mcp_tool_invocations_total {{tool=\"{}\"}} {}\n",
                tool.name, tool.invocations
            ));
            output.push_str(&format!(
                "mcp_tool_latency_us {{tool=\"{}\"}} {}\n",
                tool.name, tool.avg_latency_us
            ));
            output.push_str(&format!(
                "mcp_tool_error_rate {{tool=\"{}\"}} {}\n",
                tool.name, tool.error_rate
            ));
        }

        // Custom counters
        for (name, value) in &snapshot.counters {
            output.push_str(&format!("mcp_{} {}\n", name, value));
        }

        // Custom gauges
        for (name, value) in &snapshot.gauges {
            output.push_str(&format!("mcp_{} {}\n", name, value));
        }

        output
    }

    /// Reset all metrics.
    pub async fn reset(&self) {
        let mut requests = self.requests.write().await;
        *requests = RequestMetrics::default();

        let mut samples = self.latency_samples.write().await;
        samples.clear();

        let mut tools = self.tool_metrics.write().await;
        tools.clear();

        let mut counters = self.counters.write().await;
        counters.clear();

        let mut gauges = self.gauges.write().await;
        gauges.clear();
    }
}

/// Calculate latency metrics from samples.
fn calculate_latency_metrics(samples: &[u64]) -> LatencyMetrics {
    if samples.is_empty() {
        return LatencyMetrics::default();
    }

    let mut sorted = samples.to_vec();
    sorted.sort_unstable();

    let count = sorted.len() as u64;
    let sum: u64 = sorted.iter().sum();
    let avg = sum as f64 / count as f64;

    LatencyMetrics {
        min_us: sorted[0],
        max_us: sorted[sorted.len() - 1],
        avg_us: avg,
        p50_us: percentile(&sorted, 0.5),
        p90_us: percentile(&sorted, 0.9),
        p95_us: percentile(&sorted, 0.95),
        p99_us: percentile(&sorted, 0.99),
        count,
    }
}

/// Calculate percentile from sorted data.
fn percentile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let index = ((p * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1);
    sorted[index] as f64
}

/// RAII guard for timing requests.
pub struct RequestTimer {
    start: Instant,
    collector: Arc<MetricsCollector>,
    tool: Option<String>,
}

impl RequestTimer {
    /// Start timing a request.
    pub fn start(collector: Arc<MetricsCollector>, tool: Option<String>) -> Self {
        let timer = Self {
            start: Instant::now(),
            collector,
            tool,
        };

        // Record request start in background
        let collector = timer.collector.clone();
        tokio::spawn(async move {
            collector.request_start().await;
        });

        timer
    }

    /// Finish timing and record metrics.
    pub async fn finish(self, success: bool) {
        let latency = self.start.elapsed();
        self.collector
            .request_end(latency, success, self.tool.as_deref())
            .await;
    }
}

impl Drop for RequestTimer {
    fn drop(&mut self) {
        // If not explicitly finished, record as failure
        let collector = self.collector.clone();
        let latency = self.start.elapsed();
        let tool = self.tool.clone();
        tokio::spawn(async move {
            collector.request_end(latency, false, tool.as_deref()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::with_defaults();

        // Record some requests
        collector.request_start().await;
        collector
            .request_end(Duration::from_millis(10), true, Some("echo"))
            .await;

        collector.request_start().await;
        collector
            .request_end(Duration::from_millis(20), true, Some("echo"))
            .await;

        collector.request_start().await;
        collector
            .request_end(Duration::from_millis(5), false, Some("fail"))
            .await;

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.requests.total, 3);
        assert_eq!(snapshot.requests.success, 2);
        assert_eq!(snapshot.requests.failed, 1);
        assert_eq!(snapshot.latency.count, 3);
        assert_eq!(snapshot.tools.len(), 2);
    }

    #[tokio::test]
    async fn test_custom_metrics() {
        let collector = MetricsCollector::with_defaults();

        collector.inc_counter("custom_counter", 42).await;
        collector.set_gauge("custom_gauge", 3.15).await;

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.counters.get("custom_counter"), Some(&42));
        assert_eq!(snapshot.gauges.get("custom_gauge"), Some(&3.15));
    }

    #[tokio::test]
    async fn test_prometheus_export() {
        let collector = MetricsCollector::with_defaults();

        collector.request_start().await;
        collector
            .request_end(Duration::from_millis(10), true, Some("test"))
            .await;

        let prom = collector.prometheus_export().await;
        assert!(prom.contains("mcp_requests_total 1"));
        assert!(prom.contains("mcp_requests_active 0"));
    }

    #[test]
    fn test_percentile() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        // With rounding: p50 -> index 5 (value 6), p90 -> index 8 (value 9), p95 -> index 9 (value 10)
        assert_eq!(percentile(&data, 0.5), 6.0);
        assert_eq!(percentile(&data, 0.9), 9.0);
        assert_eq!(percentile(&data, 0.95), 10.0);
    }
}
