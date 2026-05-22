//! Cost anomaly detection for agent execution with statistical deviation alerting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A cost sample for an agent execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostSample {
    pub agent_id: String,
    pub cost_usd: f64,
    pub tokens: u64,
    pub timestamp: DateTime<Utc>,
}

/// Anomaly detection method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyMethod {
    ZScore,
    IQR,
    MovingAverage,
}

impl Default for AnomalyMethod {
    fn default() -> Self {
        AnomalyMethod::ZScore
    }
}

/// Configuration for anomaly detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnomalyConfig {
    pub method: AnomalyMethod,
    pub z_threshold: f64,
    pub iqr_multiplier: f64,
    pub window_size: usize,
    pub budget_usd_per_agent: f64,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            method: AnomalyMethod::ZScore,
            z_threshold: 2.5,
            iqr_multiplier: 1.5,
            window_size: 100,
            budget_usd_per_agent: 100.0,
        }
    }
}

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// An anomaly alert for a detected cost deviation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub agent_id: String,
    pub detected_cost: f64,
    pub expected_range: (f64, f64),
    pub method: AnomalyMethod,
    pub severity: AlertSeverity,
    pub message: String,
    pub detected_at: DateTime<Utc>,
}

/// Cost anomaly detector using statistical methods.
#[derive(Debug, Clone)]
pub struct CostAnomalyDetector {
    config: AnomalyConfig,
    samples: HashMap<String, Vec<CostSample>>,
}

impl CostAnomalyDetector {
    /// Create a new detector with the given configuration.
    pub fn new(config: AnomalyConfig) -> Self {
        Self {
            config,
            samples: HashMap::new(),
        }
    }

    /// Record a cost sample for an agent.
    pub fn record(&mut self, sample: CostSample) {
        let agent_samples = self.samples.entry(sample.agent_id.clone()).or_default();
        agent_samples.push(sample);

        // Keep only the most recent `window_size` samples
        if agent_samples.len() > self.config.window_size {
            agent_samples.remove(0);
        }
    }

    /// Detect anomaly for a specific agent.
    pub fn detect(&self, agent_id: &str) -> Option<AnomalyAlert> {
        let agent_samples = self.samples.get(agent_id)?;

        if agent_samples.len() < 3 {
            return None;
        }

        let costs: Vec<f64> = agent_samples.iter().map(|s| s.cost_usd).collect();
        let latest_cost = agent_samples.last()?.cost_usd;

        let expected_range = match self.config.method {
            AnomalyMethod::ZScore => Self::zscore_bounds(&costs, self.config.z_threshold),
            AnomalyMethod::IQR => Self::iqr_bounds(&costs),
            AnomalyMethod::MovingAverage => {
                Self::moving_average_bounds(&costs, self.config.window_size)
            }
        };

        if latest_cost < expected_range.0 || latest_cost > expected_range.1 {
            let severity = self.calculate_severity(latest_cost, expected_range);
            let message = format!(
                "Cost ${:.4} exceeds expected range ${:.4} - ${:.4} using {:?} method",
                latest_cost, expected_range.0, expected_range.1, self.config.method
            );

            Some(AnomalyAlert {
                agent_id: agent_id.to_string(),
                detected_cost: latest_cost,
                expected_range,
                method: self.config.method,
                severity,
                message,
                detected_at: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Detect anomalies for all agents.
    pub fn detect_all(&self) -> Vec<AnomalyAlert> {
        self.samples
            .keys()
            .filter_map(|agent_id| self.detect(agent_id))
            .collect()
    }

    /// Check if an agent has exceeded its budget.
    pub fn check_budget(&self, agent_id: &str) -> Option<AnomalyAlert> {
        let agent_samples = self.samples.get(agent_id)?;

        if agent_samples.is_empty() {
            return None;
        }

        let total_cost: f64 = agent_samples.iter().map(|s| s.cost_usd).sum();

        if total_cost > self.config.budget_usd_per_agent {
            Some(AnomalyAlert {
                agent_id: agent_id.to_string(),
                detected_cost: total_cost,
                expected_range: (0.0, self.config.budget_usd_per_agent),
                method: self.config.method,
                severity: if total_cost > self.config.budget_usd_per_agent * 2.0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                message: format!(
                    "Agent {} exceeded budget: ${:.4} > ${:.4}",
                    agent_id, total_cost, self.config.budget_usd_per_agent
                ),
                detected_at: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Calculate mean and standard deviation of samples.
    pub fn mean_and_stddev(samples: &[f64]) -> (f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0);
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;

        if samples.len() == 1 {
            return (mean, 0.0);
        }

        let variance = samples
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>()
            / (samples.len() - 1) as f64;

        let stddev = variance.sqrt();

        (mean, stddev)
    }

    /// Calculate IQR (Interquartile Range) bounds.
    pub fn iqr_bounds(samples: &[f64]) -> (f64, f64) {
        if samples.len() < 4 {
            let (mean, stddev) = Self::mean_and_stddev(samples);
            return (mean - 2.0 * stddev, mean + 2.0 * stddev);
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let q1_idx = sorted.len() / 4;
        let q3_idx = 3 * sorted.len() / 4;

        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];
        let iqr = q3 - q1;

        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;

        (lower, upper)
    }

    fn zscore_bounds(samples: &[f64], threshold: f64) -> (f64, f64) {
        let (mean, stddev) = Self::mean_and_stddev(samples);

        if stddev == 0.0 {
            return (mean - threshold, mean + threshold);
        }

        (mean - threshold * stddev, mean + threshold * stddev)
    }

    fn moving_average_bounds(samples: &[f64], _window_size: usize) -> (f64, f64) {
        let (mean, stddev) = Self::mean_and_stddev(samples);

        if stddev == 0.0 {
            return (mean * 0.5, mean * 1.5);
        }

        (mean - 2.0 * stddev, mean + 2.0 * stddev)
    }

    fn calculate_severity(&self, cost: f64, range: (f64, f64)) -> AlertSeverity {
        let deviation = if range.1 > range.0 {
            let max_dev = cost.max(range.1).abs() - cost.min(range.1).abs();
            if range.1 > range.0 {
                ((cost - range.1) / (range.1 - range.0)).abs()
            } else {
                0.0
            }
        } else {
            0.0
        };

        if deviation > 3.0 {
            AlertSeverity::Critical
        } else if deviation > 2.0 {
            AlertSeverity::Warning
        } else {
            AlertSeverity::Info
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample(agent_id: &str, cost: f64) -> CostSample {
        CostSample {
            agent_id: agent_id.to_string(),
            cost_usd: cost,
            tokens: 1000,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_zscore_anomaly_detection() {
        let config = AnomalyConfig {
            method: AnomalyMethod::ZScore,
            z_threshold: 2.0,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        // Normal samples
        for cost in [1.0, 1.1, 0.9, 1.05, 0.95, 1.0, 1.02, 0.98].iter() {
            detector.record(create_sample("agent1", *cost));
        }

        // No anomaly yet
        assert!(detector.detect("agent1").is_none());

        // Add a clear anomaly
        detector.record(create_sample("agent1", 10.0));

        let alert = detector.detect("agent1");
        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.agent_id, "agent1");
        assert_eq!(alert.detected_cost, 10.0);
        assert_eq!(alert.method, AnomalyMethod::ZScore);
    }

    #[test]
    fn test_iqr_detection() {
        let config = AnomalyConfig {
            method: AnomalyMethod::IQR,
            iqr_multiplier: 1.5,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        // Normal samples with clear IQR
        for cost in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].iter() {
            detector.record(create_sample("agent2", *cost));
        }

        // No anomaly
        assert!(detector.detect("agent2").is_none());

        // Add extreme outlier
        detector.record(create_sample("agent2", 50.0));

        let alert = detector.detect("agent2");
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().method, AnomalyMethod::IQR);
    }

    #[test]
    fn test_moving_average() {
        let config = AnomalyConfig {
            method: AnomalyMethod::MovingAverage,
            window_size: 5,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        for cost in [1.0, 1.0, 1.0, 1.0, 1.0].iter() {
            detector.record(create_sample("agent3", *cost));
        }

        assert!(detector.detect("agent3").is_none());

        // Spike
        detector.record(create_sample("agent3", 5.0));

        let alert = detector.detect("agent3");
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().method, AnomalyMethod::MovingAverage);
    }

    #[test]
    fn test_budget_exceeded() {
        let config = AnomalyConfig {
            budget_usd_per_agent: 10.0,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        detector.record(create_sample("agent4", 5.0));
        assert!(detector.check_budget("agent4").is_none());

        detector.record(create_sample("agent4", 6.0));

        let alert = detector.check_budget("agent4");
        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert!(alert.detected_cost > 10.0);
    }

    #[test]
    fn test_budget_critical() {
        let config = AnomalyConfig {
            budget_usd_per_agent: 10.0,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        detector.record(create_sample("agent5", 25.0));

        let alert = detector.check_budget("agent5");
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().severity, AlertSeverity::Critical);
    }

    #[test]
    fn test_no_anomaly_for_normal_data() {
        let config = AnomalyConfig::default();
        let mut detector = CostAnomalyDetector::new(config);

        // All similar costs
        for _ in 0..50 {
            detector.record(create_sample("agent6", 1.0));
        }

        assert!(detector.detect("agent6").is_none());
    }

    #[test]
    fn test_empty_samples() {
        let config = AnomalyConfig::default();
        let detector = CostAnomalyDetector::new(config);

        assert!(detector.detect("nonexistent").is_none());
        assert!(detector.check_budget("nonexistent").is_none());
        assert!(detector.detect_all().is_empty());
    }

    #[test]
    fn test_insufficient_samples() {
        let config = AnomalyConfig::default();
        let mut detector = CostAnomalyDetector::new(config);

        // Only 2 samples (needs at least 3 for reliable detection)
        detector.record(create_sample("agent7", 1.0));
        detector.record(create_sample("agent7", 2.0));

        assert!(detector.detect("agent7").is_none());
    }

    #[test]
    fn test_mean_and_stddev() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (mean, stddev) = CostAnomalyDetector::mean_and_stddev(&samples);

        assert!((mean - 3.0).abs() < 1e-10);
        // stddev of 1,2,3,4,5 is sqrt(2.5) ≈ 1.581
        assert!((stddev - std::f64::consts::SQRT_2_5).abs() < 0.01);
    }

    #[test]
    fn test_mean_and_stddev_empty() {
        let (mean, stddev) = CostAnomalyDetector::mean_and_stddev(&[]);
        assert_eq!(mean, 0.0);
        assert_eq!(stddev, 0.0);
    }

    #[test]
    fn test_mean_and_stddev_single() {
        let (mean, stddev) = CostAnomalyDetector::mean_and_stddev(&[5.0]);
        assert_eq!(mean, 5.0);
        assert_eq!(stddev, 0.0);
    }

    #[test]
    fn test_iqr_bounds_normal() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let (lower, upper) = CostAnomalyDetector::iqr_bounds(&samples);

        // Q1 ≈ 3, Q3 ≈ 8, IQR ≈ 5, bounds ≈ -4.5 to 15.5
        assert!(lower < 0.0);
        assert!(upper > 10.0);
    }

    #[test]
    fn test_iqr_bounds_small_sample() {
        // Fallback to mean±2*stddev for < 4 samples
        let samples = vec![1.0, 2.0, 3.0];
        let (lower, upper) = CostAnomalyDetector::iqr_bounds(&samples);

        let (mean, stddev) = CostAnomalyDetector::mean_and_stddev(&samples);
        assert!((lower - (mean - 2.0 * stddev)).abs() < 1e-10);
        assert!((upper - (mean + 2.0 * stddev)).abs() < 1e-10);
    }

    #[test]
    fn test_detect_all() {
        let config = AnomalyConfig {
            method: AnomalyMethod::ZScore,
            z_threshold: 2.0,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        // Normal agent
        for cost in [1.0; 10].iter() {
            detector.record(create_sample("normal", *cost));
        }

        // Anomalous agent
        for cost in [1.0; 5].iter() {
            detector.record(create_sample("anomaly", *cost));
        }
        detector.record(create_sample("anomaly", 20.0));

        let alerts = detector.detect_all();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].agent_id, "anomaly");
    }

    // === Serde roundtrip tests ===

    #[test]
    fn test_cost_sample_serde_roundtrip() {
        let sample = CostSample {
            agent_id: "test-agent".to_string(),
            cost_usd: 1.23,
            tokens: 456,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&sample).unwrap();
        let deserialized: CostSample = serde_json::from_str(&json).unwrap();

        assert_eq!(sample.agent_id, deserialized.agent_id);
        assert!((sample.cost_usd - deserialized.cost_usd).abs() < 1e-10);
        assert_eq!(sample.tokens, deserialized.tokens);
    }

    #[test]
    fn test_anomaly_config_serde_roundtrip() {
        let config = AnomalyConfig {
            method: AnomalyMethod::IQR,
            z_threshold: 3.0,
            iqr_multiplier: 2.0,
            window_size: 200,
            budget_usd_per_agent: 50.0,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AnomalyConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.method, deserialized.method);
        assert!((config.z_threshold - deserialized.z_threshold).abs() < 1e-10);
        assert!((config.iqr_multiplier - deserialized.iqr_multiplier).abs() < 1e-10);
        assert_eq!(config.window_size, deserialized.window_size);
        assert!((config.budget_usd_per_agent - deserialized.budget_usd_per_agent).abs() < 1e-10);
    }

    #[test]
    fn test_anomaly_alert_serde_roundtrip() {
        let alert = AnomalyAlert {
            agent_id: "agent-123".to_string(),
            detected_cost: 15.67,
            expected_range: (5.0, 10.0),
            method: AnomalyMethod::ZScore,
            severity: AlertSeverity::Critical,
            message: "Test alert".to_string(),
            detected_at: Utc::now(),
        };

        let json = serde_json::to_string(&alert).unwrap();
        let deserialized: AnomalyAlert = serde_json::from_str(&json).unwrap();

        assert_eq!(alert.agent_id, deserialized.agent_id);
        assert!((alert.detected_cost - deserialized.detected_cost).abs() < 1e-10);
        assert_eq!(alert.expected_range, deserialized.expected_range);
        assert_eq!(alert.method, deserialized.method);
        assert_eq!(alert.severity, deserialized.severity);
        assert_eq!(alert.message, deserialized.message);
    }

    #[test]
    fn test_anomaly_method_serde() {
        let methods = vec![
            (AnomalyMethod::ZScore, "z_score"),
            (AnomalyMethod::IQR, "iqr"),
            (AnomalyMethod::MovingAverage, "moving_average"),
        ];

        for (method, expected_str) in methods {
            let json = serde_json::to_string(&method).unwrap();
            assert_eq!(json, format!("\"{}\"", expected_str));

            let deserialized: AnomalyMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(method, deserialized);
        }
    }

    #[test]
    fn test_alert_severity_serde() {
        let severities = vec![
            (AlertSeverity::Info, "info"),
            (AlertSeverity::Warning, "warning"),
            (AlertSeverity::Critical, "critical"),
        ];

        for (severity, expected_str) in severities {
            let json = serde_json::to_string(&severity).unwrap();
            assert_eq!(json, format!("\"{}\"", expected_str));

            let deserialized: AlertSeverity = serde_json::from_str(&json).unwrap();
            assert_eq!(severity, deserialized);
        }
    }

    #[test]
    fn test_default_config() {
        let config = AnomalyConfig::default();

        assert_eq!(config.method, AnomalyMethod::ZScore);
        assert!((config.z_threshold - 2.5).abs() < 1e-10);
        assert!((config.iqr_multiplier - 1.5).abs() < 1e-10);
        assert_eq!(config.window_size, 100);
        assert!((config.budget_usd_per_agent - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_record_maintains_window_size() {
        let config = AnomalyConfig {
            window_size: 5,
            ..Default::default()
        };
        let mut detector = CostAnomalyDetector::new(config);

        for i in 0..10 {
            detector.record(create_sample("agent8", i as f64));
        }

        let samples = detector.samples.get("agent8").unwrap();
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn test_different_agents_independent() {
        let config = AnomalyConfig::default();
        let mut detector = CostAnomalyDetector::new(config);

        // Agent A has normal costs
        for cost in [1.0; 10].iter() {
            detector.record(create_sample("agent_a", *cost));
        }

        // Agent B has anomalous costs
        for cost in [1.0; 5].iter() {
            detector.record(create_sample("agent_b", *cost));
        }
        detector.record(create_sample("agent_b", 50.0));

        assert!(detector.detect("agent_a").is_none());

        let alert = detector.detect("agent_b");
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().agent_id, "agent_b");
    }
}
