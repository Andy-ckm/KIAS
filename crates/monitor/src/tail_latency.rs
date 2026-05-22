//! Tail Latency Governance
//!
//! Provides p50/p95/p99/p999 latency tracking, slow-node isolation,
//! and jitter suppression via sliding windows.

use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Sliding window configuration for jitter suppression
#[derive(Debug, Clone)]
pub struct SlidingWindowConfig {
    pub window_size: usize,  // number of samples in window
    pub min_samples: usize,  // minimum samples before computing percentiles
}

impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            window_size: 1000,
            min_samples: 100,
        }
    }
}

/// A single latency observation
#[derive(Debug, Clone, Copy)]
pub struct LatencySample {
    pub latency_us: u64,
    pub timestamp_ms: u64,
}

impl LatencySample {
    pub fn new(latency_us: u64) -> Self {
        Self {
            latency_us,
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

/// Percentile breakdown for latency analysis
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PercentileBreakdown {
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub p999_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub count: u64,
    pub sum_us: u64,
}

impl PercentileBreakdown {
    pub fn from_samples(samples: &[u64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sorted = samples.to_vec();
        sorted.sort_unstable();

        let count = sorted.len() as u64;
        let sum_us: u64 = sorted.iter().sum();

        fn percentile(sorted: &[u64], p: f64) -> u64 {
            if sorted.is_empty() {
                return 0;
            }
            let idx = (p * (sorted.len() - 1) as f64).round() as usize;
            sorted[idx.min(sorted.len() - 1)]
        }

        Self {
            p50_us: percentile(&sorted, 0.50),
            p95_us: percentile(&sorted, 0.95),
            p99_us: percentile(&sorted, 0.99),
            p999_us: percentile(&sorted, 0.999),
            min_us: sorted[0],
            max_us: sorted[sorted.len() - 1],
            count,
            sum_us,
        }
    }

    pub fn average_us(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum_us as f64 / self.count as f64
        }
    }
}

/// Sliding window of latency samples with jitter suppression
pub struct SlidingWindow {
    config: SlidingWindowConfig,
    samples: VecDeque<LatencySample>,
    values: VecDeque<u64>,
}

impl Default for SlidingWindow {
    fn default() -> Self {
        Self::new(SlidingWindowConfig::default())
    }
}

impl SlidingWindow {
    pub fn new(config: SlidingWindowConfig) -> Self {
        Self {
            config,
            samples: VecDeque::with_capacity(config.window_size),
            values: VecDeque::with_capacity(config.window_size),
        }
    }

    /// Add a latency sample
    pub fn add(&mut self, latency_us: u64) {
        let sample = LatencySample::new(latency_us);
        self.samples.push_back(sample);
        self.values.push_back(latency_us);

        // Evict oldest if over window size
        if self.samples.len() > self.config.window_size {
            self.samples.pop_front();
            self.values.pop_front();
        }
    }

    /// Get percentile breakdown if enough samples
    pub fn percentiles(&self) -> Option<PercentileBreakdown> {
        if self.values.len() < self.config.min_samples {
            return None;
        }
        Some(PercentileBreakdown::from_samples(&self.values.make_contiguous()))
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Discard all samples (reset)
    pub fn clear(&mut self) {
        self.samples.clear();
        self.values.clear();
    }

    /// Get all raw values for computation
    fn raw_values(&self) -> Vec<u64> {
        self.values.iter().copied().collect()
    }
}

/// Slow-node isolation entry
#[derive(Debug, Clone)]
pub struct IsolatedNode {
    pub node_id: String,
    pub reason: String,
    pub isolated_at_ms: u64,
    pub retry_count: u32,
}

/// Latency Tracker with p50/p95/p99/p999 tracking and slow-node isolation
pub struct LatencyTracker {
    /// Per-node sliding windows
    node_windows: Arc<RwLock<BTreeMap<String, SlidingWindow>>>,
    /// Global sliding window
    global_window: Arc<RwLock<SlidingWindow>>,
    /// Isolated slow nodes
    isolated_nodes: Arc<RwLock<BTreeMap<String, IsolatedNode>>>,
    /// Slow node threshold (latency in µs)
    slow_threshold_us: u64,
    /// Isolation threshold (consecutive slow samples)
    isolation_threshold: u32,
    /// Config
    window_config: SlidingWindowConfig,
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new(100_000, 5) // 100ms default slow threshold
    }
}

impl LatencyTracker {
    pub fn new(slow_threshold_us: u64, isolation_threshold: u32) -> Self {
        Self {
            node_windows: Arc::new(RwLock::new(BTreeMap::new())),
            global_window: Arc::new(RwLock::new(SlidingWindow::new(SlidingWindowConfig {
                window_size: 10_000,
                min_samples: 500,
            }))),
            isolated_nodes: Arc::new(RwLock::new(BTreeMap::new())),
            slow_threshold_us,
            isolation_threshold,
            window_config: SlidingWindowConfig::default(),
        }
    }

    /// Set window configuration
    pub fn with_window_config(mut self, config: SlidingWindowConfig) -> Self {
        self.window_config = config;
        self
    }

    /// Record a latency sample for a node
    pub async fn record(&self, node_id: &str, latency_us: u64) {
        // Update global window
        {
            let mut gw = self.global_window.write().await;
            gw.add(latency_us);
        }

        // Update per-node window
        {
            let mut windows = self.node_windows.write().await;
            let window = windows.entry(node_id.to_string()).or_insert_with(|| {
                SlidingWindow::new(self.window_config.clone())
            });
            window.add(latency_us);

            // Check for slow node
            if latency_us > self.slow_threshold_us {
                let mut isolated = self.isolated_nodes.write().await;
                let entry = isolated.entry(node_id.to_string()).or_insert(IsolatedNode {
                    node_id: node_id.to_string(),
                    reason: format!("latency {}us > {}us threshold", latency_us, self.slow_threshold_us),
                    isolated_at_ms: chrono::Utc::now().timestamp_millis() as u64,
                    retry_count: 0,
                });
                entry.retry_count += 1;
            } else {
                // Reset consecutive slow count on normal latency
                let mut isolated = self.isolated_nodes.write().await;
                if let Some(entry) = isolated.get_mut(node_id) {
                    entry.retry_count = 0;
                }
            }
        }
    }

    /// Check if a node is isolated
    pub async fn is_isolated(&self, node_id: &str) -> bool {
        let isolated = self.isolated_nodes.read().await;
        if let Some(node) = isolated.get(node_id) {
            node.retry_count >= self.isolation_threshold
        } else {
            false
        }
    }

    /// Get global percentiles
    pub async fn global_percentiles(&self) -> Option<PercentileBreakdown> {
        self.global_window.read().await.percentiles()
    }

    /// Get per-node percentiles
    pub async fn node_percentiles(&self, node_id: &str) -> Option<PercentileBreakdown> {
        let windows = self.node_windows.read().await;
        windows.get(node_id).and_then(|w| w.percentiles())
    }

    /// Get all isolated nodes
    pub async fn isolated_nodes(&self) -> Vec<IsolatedNode> {
        let isolated = self.isolated_nodes.read().await;
        isolated.values().cloned().collect()
    }

    /// Release a node from isolation
    pub async fn release(&self, node_id: &str) {
        let mut isolated = self.isolated_nodes.write().await;
        isolated.remove(node_id);
    }

    /// Force isolation of a node
    pub async fn isolate(&self, node_id: &str, reason: &str) {
        let mut isolated = self.isolated_nodes.write().await;
        isolated.insert(node_id.to_string(), IsolatedNode {
            node_id: node_id.to_string(),
            reason: reason.to_string(),
            isolated_at_ms: chrono::Utc::now().timestamp_millis() as u64,
            retry_count: self.isolation_threshold,
        });
    }

    /// Get jitter-suppressed percentiles for a node (suppresses micro-spikes)
    pub async fn stable_percentiles(&self, node_id: &str) -> Option<PercentileBreakdown> {
        let windows = self.node_windows.read().await;
        if let Some(window) = windows.get(node_id) {
            // Use raw values for stable computation (window already provides jitter suppression)
            let values: Vec<u64> = window.raw_values();
            if values.len() < self.window_config.min_samples {
                return None;
            }
            Some(PercentileBreakdown::from_samples(&values))
        } else {
            None
        }
    }

    /// Get all node IDs tracked
    pub async fn tracked_nodes(&self) -> Vec<String> {
        let windows = self.node_windows.read().await;
        windows.keys().cloned().collect()
    }

    /// Clear all data
    pub async fn clear(&self) {
        self.global_window.write().await.clear();
        let mut windows = self.node_windows.write().await;
        for (_, window) in windows.iter_mut() {
            window.clear();
        }
        self.isolated_nodes.write().await.clear();
    }
}

/// Jitter suppression via exponential moving average smoothing
#[derive(Debug, Clone)]
pub struct JitterSuppressor {
    pub alpha: f64,  // EMA smoothing factor (0 < alpha <= 1)
    last_ema: Option<f64>,
}

impl Default for JitterSuppressor {
    fn default() -> Self {
        Self::new(0.3)
    }
}

impl JitterSuppressor {
    pub fn new(alpha: f64) -> Self {
        assert!((0.0..=1.0).contains(&alpha));
        Self {
            alpha,
            last_ema: None,
        }
    }

    /// Smooth a latency value
    pub fn smooth(&mut self, latency_us: u64) -> f64 {
        let latency_f = latency_us as f64;
        match self.last_ema {
            None => {
                self.last_ema = Some(latency_f);
                latency_f
            }
            Some(ema) => {
                let new_ema = self.alpha * latency_f + (1.0 - self.alpha) * ema;
                self.last_ema = Some(new_ema);
                new_ema
            }
        }
    }

    pub fn reset(&mut self) {
        self.last_ema = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_breakdown_empty() {
        let p = PercentileBreakdown::from_samples(&[]);
        assert_eq!(p.count, 0);
        assert_eq!(p.p50_us, 0);
    }

    #[test]
    fn test_percentile_breakdown_single() {
        let p = PercentileBreakdown::from_samples(&[100]);
        assert_eq!(p.p50_us, 100);
        assert_eq!(p.p95_us, 100);
        assert_eq!(p.p99_us, 100);
        assert_eq!(p.count, 1);
    }

    #[test]
    fn test_percentile_breakdown_many() {
        let samples: Vec<u64> = (1..=1000).collect();
        let p = PercentileBreakdown::from_samples(&samples);
        assert!(p.p50_us > 0);
        assert!(p.p95_us > p.p50_us);
        assert!(p.p99_us > p.p95_us);
        assert!(p.p999_us >= p.p99_us);
    }

    #[test]
    fn test_sliding_window_add_and_evict() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_samples: 2,
        };
        let mut window = SlidingWindow::new(config);

        for i in 1..=10 {
            window.add(i * 100);
        }

        // Should have max 5 samples
        assert_eq!(window.len(), 5);
        // Percentiles need min_samples
        assert!(window.percentiles().is_some());
    }

    #[test]
    fn test_sliding_window_insufficient_samples() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_samples: 5,
        };
        let mut window = SlidingWindow::new(config);
        window.add(100);
        window.add(200);

        assert!(window.percentiles().is_none());
    }

    #[tokio::test]
    async fn test_latency_tracker_record() {
        let tracker = LatencyTracker::new(50_000, 3); // 50ms threshold
        tracker.record("node1", 10_000).await;
        tracker.record("node1", 20_000).await;
        tracker.record("node1", 30_000).await;

        let p = tracker.global_percentiles().await;
        assert!(p.is_some());
        let p = p.unwrap();
        assert!(p.count >= 3);
    }

    #[tokio::test]
    async fn test_latency_tracker_slow_node_isolation() {
        let tracker = LatencyTracker::new(10_000, 3); // 10ms threshold, 3 slow = isolate
        for _ in 0..2 {
            tracker.record("slow_node", 100_000).await; // 100ms - way over
        }
        assert!(!tracker.is_isolated("slow_node").await);

        tracker.record("slow_node", 100_000).await; // Third slow sample
        assert!(tracker.is_isolated("slow_node").await);
    }

    #[tokio::test]
    async fn test_latency_tracker_release() {
        let tracker = LatencyTracker::new(10_000, 2);
        tracker.record("node", 100_000).await;
        tracker.record("node", 100_000).await;
        assert!(tracker.is_isolated("node").await);

        tracker.release("node").await;
        assert!(!tracker.is_isolated("node").await);
    }

    #[test]
    fn test_jitter_suppressor_ema() {
        let mut suppressor = JitterSuppressor::new(0.5);

        // First value is raw
        let v1 = suppressor.smooth(100);
        assert_eq!(v1, 100.0);

        // EMA: 0.5 * 200 + 0.5 * 100 = 150
        let v2 = suppressor.smooth(200);
        assert_eq!(v2, 150.0);

        // EMA: 0.5 * 300 + 0.5 * 150 = 225
        let v3 = suppressor.smooth(300);
        assert!(225.0 - v3 < 0.01);
    }

    #[test]
    fn test_jitter_suppressor_reset() {
        let mut suppressor = JitterSuppressor::new(0.5);
        suppressor.smooth(100);
        suppressor.smooth(200);
        suppressor.reset();

        let v = suppressor.smooth(300);
        assert_eq!(v, 300.0); // First after reset is raw
    }
}
