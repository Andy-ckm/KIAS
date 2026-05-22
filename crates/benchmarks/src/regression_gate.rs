//! # Performance Regression Gate
//!
//! Detects performance regressions in latency, throughput, and memory usage.
//! Compares current benchmarks against baselines and blocks if thresholds exceeded.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── Benchmark Result ──────────────────────────────────────────────────────────

/// A single benchmark measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub metric: MetricType,
    pub value: f64,
    pub unit: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sample_count: u64,
    pub std_dev: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType { Latency, Throughput, Memory, Custom }

impl BenchmarkResult {
    pub fn new(name: String, metric: MetricType, value: f64, unit: &str) -> Self {
        Self { name, metric, value, unit: unit.to_string(), timestamp: chrono::Utc::now(), sample_count: 1, std_dev: 0.0 }
    }
    pub fn with_samples(mut self, count: u64, std_dev: f64) -> Self { self.sample_count = count; self.std_dev = std_dev; self }
}

/// A baseline for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub name: String,
    pub metric: MetricType,
    pub value: f64,
    pub upper_bound: f64,  // regression threshold (upper for latency/memory, 0 for throughput)
    pub lower_bound: f64,  // for throughput: minimum acceptable
    pub established_at: chrono::DateTime<chrono::Utc>,
}

impl Baseline {
    pub fn new(name: String, metric: MetricType, value: f64, threshold_pct: f64) -> Self {
        match metric {
            MetricType::Latency | MetricType::Memory => {
                let bound = value * (1.0 + threshold_pct / 100.0);
                Self { name, metric, value, upper_bound: bound, lower_bound: 0.0, established_at: chrono::Utc::now() }
            }
            MetricType::Throughput => {
                let lower = value * (1.0 - threshold_pct / 100.0);
                Self { name, metric, value, upper_bound: f64::MAX, lower_bound: lower, established_at: chrono::Utc::now() }
            }
            MetricType::Custom => Self { name, metric, value, upper_bound: f64::MAX, lower_bound: 0.0, established_at: chrono::Utc::now() },
        }
    }
}

// ── Regression Decision ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionDecision { Pass, Fail, Warn }

impl RegressionDecision {
    pub fn is_blocking(&self) -> bool { matches!(self, RegressionDecision::Fail) }
}

#[derive(Debug, Clone)]
pub struct RegressionReport {
    pub name: String,
    pub decision: RegressionDecision,
    pub baseline_value: f64,
    pub current_value: f64,
    pub delta_pct: f64,
    pub message: String,
    pub blocked: bool,
}

impl RegressionReport {
    pub fn pass(name: &str, baseline: f64, current: f64) -> Self {
        let delta_pct = if baseline == 0.0 { 0.0 } else { (current - baseline) / baseline * 100.0 };
        Self { name: name.to_string(), decision: RegressionDecision::Pass, baseline_value: baseline, current_value: current, delta_pct, message: format!("PASS: {} (+{:.2}%)", name, delta_pct), blocked: false }
    }
    pub fn fail(name: &str, baseline: f64, current: f64, threshold_pct: f64) -> Self {
        let delta_pct = if baseline == 0.0 { 0.0 } else { (current - baseline) / baseline * 100.0 };
        Self { name: name.to_string(), decision: RegressionDecision::Fail, baseline_value: baseline, current_value: current, delta_pct, message: format!("FAIL: {} regressed by {:.2}% (threshold: {}%)", name, delta_pct.abs(), threshold_pct), blocked: true }
    }
    pub fn warn(name: &str, baseline: f64, current: f64, threshold_pct: f64) -> Self {
        let delta_pct = if baseline == 0.0 { 0.0 } else { (current - baseline) / baseline * 100.0 };
        Self { name: name.to_string(), decision: RegressionDecision::Warn, baseline_value: baseline, current_value: current, delta_pct, message: format!("WARN: {} changed by {:.2}%", name, delta_pct), blocked: false }
    }
}

// ── Regression Detector ──────────────────────────────────────────────────────

pub struct RegressionDetector {
    baselines: HashMap<String, Baseline>,
    default_threshold_pct: f64,
    warn_threshold_pct: f64,
}

impl Default for RegressionDetector {
    fn default() -> Self { Self::new(10.0, 5.0) }
}

impl RegressionDetector {
    pub fn new(default_threshold_pct: f64, warn_threshold_pct: f64) -> Self {
        Self { baselines: HashMap::new(), default_threshold_pct, warn_threshold_pct }
    }

    pub fn register_baseline(&mut self, baseline: Baseline) { self.baselines.insert(baseline.name.clone(), baseline); }
    pub fn register_baseline_simple(&mut self, name: &str, metric: MetricType, value: f64) { self.register_baseline(Baseline::new(name.to_string(), metric, value, self.default_threshold_pct)); }

    pub fn check(&self, result: &BenchmarkResult) -> RegressionReport {
        match self.baselines.get(&result.name) {
            None => RegressionReport { name: result.name.clone(), decision: RegressionDecision::Warn, baseline_value: 0.0, current_value: result.value, delta_pct: 0.0, message: format!("No baseline for {}", result.name), blocked: false },
            Some(baseline) => {
                let delta_pct = if baseline.value == 0.0 { 0.0 } else { (result.value - baseline.value) / baseline.value * 100.0 };
                match result.metric {
                    MetricType::Latency | MetricType::Memory => {
                        if result.value > baseline.upper_bound { RegressionReport::fail(&result.name, baseline.value, result.value, self.default_threshold_pct) }
                        else if result.value > baseline.value * (1.0 + self.warn_threshold_pct / 100.0) { RegressionReport::warn(&result.name, baseline.value, result.value, self.warn_threshold_pct) }
                        else { RegressionReport::pass(&result.name, baseline.value, result.value) }
                    }
                    MetricType::Throughput => {
                        if result.value < baseline.lower_bound { RegressionReport::fail(&result.name, baseline.value, result.value, self.default_threshold_pct) }
                        else if result.value < baseline.value * (1.0 - self.warn_threshold_pct / 100.0) { RegressionReport::warn(&result.name, baseline.value, result.value, self.warn_threshold_pct) }
                        else { RegressionReport::pass(&result.name, baseline.value, result.value) }
                    }
                    MetricType::Custom => RegressionReport::pass(&result.name, baseline.value, result.value),
                }
            }
        }
    }

    pub fn check_multiple(&self, results: &[BenchmarkResult]) -> Vec<RegressionReport> { results.iter().map(|r| self.check(r)).collect() }

    pub fn has_blocking_failure(&self, reports: &[RegressionReport]) -> bool { reports.iter().any(|r| r.blocked) }

    pub fn get_baseline(&self, name: &str) -> Option<&Baseline> { self.baselines.get(name) }
}

// ── Benchmark Runner ──────────────────────────────────────────────────────────

pub struct BenchmarkRunner { detector: RegressionDetector }

impl BenchmarkRunner {
    pub fn new(detector: RegressionDetector) -> Self { Self { detector } }

    pub fn establish_baseline(&mut self, name: &str, metric: MetricType, value: f64) { self.detector.register_baseline_simple(name, metric, value); }

    pub fn run_and_check(&self, name: &str, metric: MetricType, value: f64) -> RegressionReport {
        let result = BenchmarkResult::new(name.to_string(), metric, value, match metric { MetricType::Latency => "ms", MetricType::Throughput => "ops/s", MetricType::Memory => "MB", MetricType::Custom => "" });
        self.detector.check(&result)
    }

    pub fn run(&self, name: &str, metric: MetricType, value: f64, unit: &str) -> BenchmarkResult { BenchmarkResult::new(name.to_string(), metric, value, unit) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_latency() {
        let baseline = Baseline::new("api_latency".to_string(), MetricType::Latency, 100.0, 10.0);
        assert!(baseline.upper_bound > 100.0);
        assert_eq!(baseline.lower_bound, 0.0);
    }

    #[test]
    fn test_baseline_throughput() {
        let baseline = Baseline::new("throughput".to_string(), MetricType::Throughput, 1000.0, 10.0);
        assert!(baseline.lower_bound < 1000.0);
        assert_eq!(baseline.upper_bound, f64::MAX);
    }

    #[test]
    fn test_regression_detector_no_baseline() {
        let detector = RegressionDetector::default();
        let result = BenchmarkResult::new("unknown".to_string(), MetricType::Latency, 50.0, "ms");
        let report = detector.check(&result);
        assert_eq!(report.decision, RegressionDecision::Warn);
    }

    #[test]
    fn test_regression_detector_latency_pass() {
        let mut detector = RegressionDetector::default();
        detector.register_baseline_simple("api", MetricType::Latency, 100.0);
        let result = BenchmarkResult::new("api".to_string(), MetricType::Latency, 105.0, "ms");
        let report = detector.check(&result);
        assert_eq!(report.decision, RegressionDecision::Pass);
    }

    #[test]
    fn test_regression_detector_latency_fail() {
        let mut detector = RegressionDetector::default();
        detector.register_baseline_simple("api", MetricType::Latency, 100.0);
        let result = BenchmarkResult::new("api".to_string(), MetricType::Latency, 150.0, "ms");
        let report = detector.check(&result);
        assert_eq!(report.decision, RegressionDecision::Fail);
        assert!(report.blocked);
    }

    #[test]
    fn test_regression_detector_throughput_pass() {
        let mut detector = RegressionDetector::default();
        detector.register_baseline_simple("throughput", MetricType::Throughput, 1000.0);
        let result = BenchmarkResult::new("throughput".to_string(), MetricType::Throughput, 950.0, "ops/s");
        let report = detector.check(&result);
        assert_eq!(report.decision, RegressionDecision::Pass);
    }

    #[test]
    fn test_regression_detector_throughput_fail() {
        let mut detector = RegressionDetector::default();
        detector.register_baseline_simple("throughput", MetricType::Throughput, 1000.0);
        let result = BenchmarkResult::new("throughput".to_string(), MetricType::Throughput, 500.0, "ops/s");
        let report = detector.check(&result);
        assert_eq!(report.decision, RegressionDecision::Fail);
        assert!(report.blocked);
    }

    #[test]
    fn test_has_blocking_failure() {
        let detector = RegressionDetector::default();
        let reports = vec![
            RegressionReport::pass("a", 100.0, 100.0),
            RegressionReport::fail("b", 100.0, 200.0, 10.0),
        ];
        assert!(detector.has_blocking_failure(&reports));
    }

    #[test]
    fn test_delta_pct_calculation() {
        let mut detector = RegressionDetector::default();
        detector.register_baseline_simple("api", MetricType::Latency, 100.0);
        let result = BenchmarkResult::new("api".to_string(), MetricType::Latency, 110.0, "ms");
        let report = detector.check(&result);
        assert!((report.delta_pct - 10.0).abs() < 0.01);
    }
}
