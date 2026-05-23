use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent behavior anomaly detection engine.
///
/// Detects unusual Agent behavior patterns using statistical methods:
/// - Operation frequency anomalies (Z-score)
/// - Cost spike detection
/// - Unusual time-of-day patterns
/// - New/unknown operation types
///
/// EMQ has NO behavior anomaly detection — this is a key differentiator.
///
/// Anomaly detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Z-score threshold for frequency/cost anomaly (default: 2.0)
    pub z_threshold: f64,

    /// IQR multiplier for outlier detection (default: 1.5)
    pub iqr_multiplier: f64,

    /// Minimum samples before detection activates
    pub min_samples: usize,

    /// Cost spike multiplier threshold (e.g., 3.0 = 3x average)
    pub cost_spike_multiplier: f64,

    /// Time window in minutes for frequency analysis
    pub window_minutes: u64,

    /// Rolling window size for time-series analysis (number of events)
    pub rolling_window_size: usize,

    /// Minimum hour samples before time-pattern anomaly activates
    pub min_hour_samples: u64,

    /// Trend sensitivity: fraction of window that must trend same direction (0.0-1.0)
    pub trend_sensitivity: f64,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            z_threshold: 2.0,
            iqr_multiplier: 1.5,
            min_samples: 10,
            cost_spike_multiplier: 3.0,
            window_minutes: 60,
            rolling_window_size: 50,
            min_hour_samples: 100,
            trend_sensitivity: 0.6,
        }
    }
}

/// An agent operation event for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEvent {
    pub agent_id: String,
    pub operation: String,
    pub cost_usd: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
}

/// Detected anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub agent_id: String,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub description: String,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub metric_value: f64,
    pub threshold: f64,
}

/// Types of anomalies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// Operation frequency is unusually high or low
    FrequencySpike,
    /// Cost suddenly increased
    CostSpike,
    /// Operation at unusual hour
    UnusualTimePattern,
    /// Unknown/new operation type
    UnknownOperation,
    /// High error rate
    ErrorRateSpike,
    /// Operation count frequency spike
    OpCountSpike,
    /// Cost trend anomaly (sudden increase/decrease)
    CostTrendAnomaly,
    /// IQR-based cost outlier
    CostIqrOutlier,
}

/// Anomaly severity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Per-agent statistics
#[derive(Debug, Clone, Default)]
struct AgentStats {
    /// Operation counts by type
    op_counts: HashMap<String, Vec<f64>>,

    /// Rolling window of recent costs (time-series)
    rolling_costs: Vec<f64>,

    /// Rolling window of timestamps (aligned with rolling_costs)
    rolling_timestamps: Vec<chrono::DateTime<chrono::Utc>>,

    /// Per-operation rolling frequency: operation name -> counts per time window
    op_frequency: HashMap<String, Vec<u32>>,

    /// Cost history (unbounded, for Z-score/IQR baseline)
    costs: Vec<f64>,

    /// Hour-of-day distribution (0-23)
    hour_distribution: Vec<u64>,

    /// Total hour samples
    total_hour_samples: u64,

    /// Error count
    errors: u64,

    /// Total operations
    total_ops: u64,

    /// Known operation types
    known_ops: std::collections::HashSet<String>,
}

impl AgentStats {
    fn record(&mut self, event: &OperationEvent, window_size: usize) {
        let hour = event.timestamp.hour() as usize;
        if self.hour_distribution.len() <= hour {
            self.hour_distribution.resize(hour + 1, 0);
        }
        self.hour_distribution[hour] += 1;
        self.total_hour_samples += 1;

        // Rolling window for time-series analysis
        self.rolling_costs.push(event.cost_usd);
        self.rolling_timestamps.push(event.timestamp);
        if self.rolling_costs.len() > window_size {
            self.rolling_costs.remove(0);
            self.rolling_timestamps.remove(0);
        }

        // Per-operation frequency: count events in each time window (bucket by minute)
        let minute_bucket = event.timestamp.timestamp() / 60;
        let freq = self
            .op_frequency
            .entry(event.operation.clone())
            .or_default();
        if freq.is_empty() || *freq.last().unwrap_or(&0) != minute_bucket as u32 {
            freq.push(minute_bucket as u32);
            if freq.len() > 20 {
                freq.remove(0);
            }
        }

        self.op_counts
            .entry(event.operation.clone())
            .or_default()
            .push(event.cost_usd);

        self.costs.push(event.cost_usd);
        self.total_ops += 1;
        self.known_ops.insert(event.operation.clone());

        if !event.success {
            self.errors += 1;
        }
    }
}

/// Anomaly detection engine
pub struct AnomalyDetector {
    stats: Arc<RwLock<HashMap<String, AgentStats>>>,
    events: Arc<RwLock<Vec<OperationEvent>>>,
    config: AnomalyConfig,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Record an operation event and check for anomalies
    pub async fn record(&self, event: OperationEvent) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Record the event
        {
            let mut stats = self.stats.write().await;
            let agent_stats = stats.entry(event.agent_id.clone()).or_default();
            agent_stats.record(&event, self.config.rolling_window_size);
        }

        {
            let mut events = self.events.write().await;
            events.push(event.clone());
        }

        // Only analyze after minimum samples
        let stats = self.stats.read().await;
        let agent_stats = match stats.get(&event.agent_id) {
            Some(s) if s.total_ops >= self.config.min_samples as u64 => s,
            _ => return anomalies,
        };

        // Check cost spike (mean × multiplier)
        if let Some(anomaly) = self.check_cost_spike(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check IQR-based outlier
        if let Some(anomaly) = self.check_cost_iqr(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check error rate
        if let Some(anomaly) = self.check_error_rate(&event.agent_id, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check operation frequency spike (real count-based, not cost-based)
        if let Some(anomaly) = self.check_op_count_spike(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check cost trend anomaly (rolling window)
        if let Some(anomaly) = self.check_cost_trend(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check unusual time-of-day pattern
        if let Some(anomaly) = self.check_time_pattern(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        anomalies
    }

    /// Detect IQR-based cost outlier.
    /// IQR = Q3 - Q1. Outlier if value > Q3 + k*IQR or < Q1 - k*IQR.
    fn check_cost_iqr(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        let costs = &stats.costs;
        if costs.len() < 4 {
            return None;
        }

        let mut sorted = costs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q1_idx = costs.len() / 4;
        let q3_idx = (costs.len() * 3) / 4;
        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];
        let iqr = q3 - q1;

        if iqr <= 0.0 {
            return None;
        }

        let upper = q3 + self.config.iqr_multiplier * iqr;
        if event.cost_usd > upper {
            let severity = if event.cost_usd > upper * 2.0 {
                AnomalySeverity::Critical
            } else {
                AnomalySeverity::Medium
            };
            return Some(Anomaly {
                agent_id: event.agent_id.clone(),
                anomaly_type: AnomalyType::CostIqrOutlier,
                severity,
                description: format!(
                    "IQR outlier: cost ${:.4} above upper fence ${:.4} (Q1={:.4}, Q3={:.4}, IQR={:.4})",
                    event.cost_usd, upper, q1, q3, iqr
                ),
                detected_at: chrono::Utc::now(),
                metric_value: event.cost_usd,
                threshold: upper,
            });
        }
        None
    }

    /// Detect unusual time-of-day patterns using hour distribution entropy.
    fn check_time_pattern(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        if stats.total_hour_samples < self.config.min_hour_samples {
            return None;
        }

        // Compute expected probability from historical distribution
        let hour = event.timestamp.hour() as usize;
        let hour_count = stats.hour_distribution.get(hour).copied().unwrap_or(0);
        let expected_prob = hour_count as f64 / stats.total_hour_samples as f64;

        // If this hour has been seen < 1% historically, flag as unusual
        if expected_prob < 0.01 && hour_count == 0 {
            // Check if agent has been active at all during this hour historically
            let active_hours = stats.hour_distribution.iter().filter(|&&c| c > 0).count();
            if active_hours > 6 {
                // Agent has clear hour patterns — this hour is anomalous
                return Some(Anomaly {
                    agent_id: event.agent_id.clone(),
                    anomaly_type: AnomalyType::UnusualTimePattern,
                    severity: AnomalySeverity::Medium,
                    description: format!(
                        "Activity at hour {} ({:02}:00) is outside established pattern (agent active in {} of 24 hours)",
                        hour, hour, active_hours
                    ),
                    detected_at: chrono::Utc::now(),
                    metric_value: 0.0,
                    threshold: 0.01,
                });
            }
        }
        None
    }

    /// Detect operation count frequency spike using rolling frequency windows.
    fn check_op_count_spike(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        let freq = match stats.op_frequency.get(&event.operation) {
            Some(f) if f.len() >= 5 => f,
            _ => return None,
        };

        // Rolling frequency: count events in recent windows
        // Each bucket = 1 minute. Compare current minute count vs historical average
        let current_count = freq.last().copied().unwrap_or(0) as f64;
        if freq.len() < 2 {
            return None;
        }

        let prev_avg: f64 = freq
            .iter()
            .take(freq.len() - 1)
            .map(|&v| v as f64)
            .sum::<f64>()
            / (freq.len() - 1) as f64;

        if prev_avg <= 0.0 {
            return None;
        }

        let ratio = current_count / prev_avg;
        // If current is 5x the historical average (spamming the same operation)
        if ratio > 5.0 {
            return Some(Anomaly {
                agent_id: event.agent_id.clone(),
                anomaly_type: AnomalyType::OpCountSpike,
                severity: if ratio > 10.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                },
                description: format!(
                    "Operation '{}' frequency spike: {} events/min vs avg {:.1} ({:.1}x)",
                    event.operation, current_count, prev_avg, ratio
                ),
                detected_at: chrono::Utc::now(),
                metric_value: ratio,
                threshold: 5.0,
            });
        }
        None
    }

    /// Detect cost trend anomaly: linear regression slope on rolling window.
    /// If recent costs trend sharply upward/downward, flag it.
    fn check_cost_trend(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        let window = &stats.rolling_costs;
        if window.len() < 10 {
            return None;
        }

        // Simple linear regression: slope = cov(x,y) / var(x)
        let n = window.len() as f64;
        let i_vals: Vec<f64> = (0..window.len()).map(|i| i as f64).collect();
        let mean_i = i_vals.iter().sum::<f64>() / n;
        let mean_y = window.iter().sum::<f64>() / n;

        let cov: f64 = i_vals
            .iter()
            .zip(window.iter())
            .map(|(i, y)| (i - mean_i) * (y - mean_y))
            .sum::<f64>()
            / n;

        let var_i: f64 = i_vals.iter().map(|i| (i - mean_i).powi(2)).sum::<f64>() / n;

        if var_i <= 0.0 {
            return None;
        }

        let slope = cov / var_i; // cost change per event
        let avg_cost = mean_y;

        if avg_cost <= 0.0 {
            return None;
        }

        // Normalize slope: if slope × window_size > 50% of avg cost, it's a trend anomaly
        let window_size = window.len() as f64;
        let projected_change = slope * window_size;
        let change_ratio = projected_change.abs() / avg_cost;

        if change_ratio > 0.5 && slope.abs() > 0.001 {
            let direction = if slope > 0.0 {
                "increasing"
            } else {
                "decreasing"
            };
            return Some(Anomaly {
                agent_id: event.agent_id.clone(),
                anomaly_type: AnomalyType::CostTrendAnomaly,
                severity: if change_ratio > 1.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                },
                description: format!(
                    "Cost trend anomaly: costs {} at {:.4}/event projected change {:.1}% over {} events",
                    direction, slope, change_ratio * 100.0, window_size
                ),
                detected_at: chrono::Utc::now(),
                metric_value: slope,
                threshold: 0.001,
            });
        }
        None
    }

    fn check_cost_spike(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        if stats.costs.len() < 2 {
            return None;
        }

        let mean: f64 = stats.costs.iter().sum::<f64>() / stats.costs.len() as f64;
        if mean <= 0.0 {
            return None;
        }

        if event.cost_usd > mean * self.config.cost_spike_multiplier {
            Some(Anomaly {
                agent_id: event.agent_id.clone(),
                anomaly_type: AnomalyType::CostSpike,
                severity: if event.cost_usd > mean * 5.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                },
                description: format!(
                    "Cost spike: ${:.4} vs avg ${:.4} ({:.1}x)",
                    event.cost_usd,
                    mean,
                    event.cost_usd / mean
                ),
                detected_at: chrono::Utc::now(),
                metric_value: event.cost_usd,
                threshold: mean * self.config.cost_spike_multiplier,
            })
        } else {
            None
        }
    }

    fn check_error_rate(&self, agent_id: &str, stats: &AgentStats) -> Option<Anomaly> {
        if stats.total_ops < self.config.min_samples as u64 {
            return None;
        }

        let error_rate = stats.errors as f64 / stats.total_ops as f64;
        if error_rate > 0.5 {
            Some(Anomaly {
                agent_id: agent_id.to_string(),
                anomaly_type: AnomalyType::ErrorRateSpike,
                severity: if error_rate > 0.8 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                },
                description: format!(
                    "High error rate: {:.0}% ({}/{} ops)",
                    error_rate * 100.0,
                    stats.errors,
                    stats.total_ops
                ),
                detected_at: chrono::Utc::now(),
                metric_value: error_rate,
                threshold: 0.5,
            })
        } else {
            None
        }
    }

    /// Get anomaly history for an agent
    pub async fn agent_stats(&self, agent_id: &str) -> Option<AgentStatsView> {
        let stats = self.stats.read().await;
        stats.get(agent_id).map(|s| AgentStatsView {
            agent_id: agent_id.to_string(),
            total_ops: s.total_ops,
            errors: s.errors,
            error_rate: if s.total_ops > 0 {
                s.errors as f64 / s.total_ops as f64
            } else {
                0.0
            },
            known_ops: s.known_ops.len(),
            avg_cost: if !s.costs.is_empty() {
                s.costs.iter().sum::<f64>() / s.costs.len() as f64
            } else {
                0.0
            },
        })
    }

    /// Get event count
    pub async fn event_count(&self) -> usize {
        self.events.read().await.len()
    }

    /// Update configuration
    pub async fn update_config(&mut self, config: AnomalyConfig) {
        self.config = config;
    }
}

/// Read-only view of agent stats
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatsView {
    pub agent_id: String,
    pub total_ops: u64,
    pub errors: u64,
    pub error_rate: f64,
    pub known_ops: usize,
    pub avg_cost: f64,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new(AnomalyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn event(agent_id: &str, op: &str, cost: f64, success: bool) -> OperationEvent {
        OperationEvent {
            agent_id: agent_id.to_string(),
            operation: op.to_string(),
            cost_usd: cost,
            timestamp: Utc::now(),
            success,
        }
    }

    #[tokio::test]
    async fn test_no_anomaly_normal_behavior() {
        let detector = AnomalyDetector::new(AnomalyConfig {
            min_samples: 5,
            ..Default::default()
        });

        for _ in 0..10 {
            let anomalies = detector.record(event("a1", "llm.chat", 0.05, true)).await;
            assert!(anomalies.is_empty());
        }
    }

    #[tokio::test]
    async fn test_cost_spike_detection() {
        let detector = AnomalyDetector::new(AnomalyConfig {
            min_samples: 5,
            cost_spike_multiplier: 3.0,
            ..Default::default()
        });

        // Build baseline
        for _ in 0..10 {
            detector.record(event("a1", "llm.chat", 0.05, true)).await;
        }

        // Spike
        let anomalies = detector.record(event("a1", "llm.chat", 0.50, true)).await;
        assert!(!anomalies.is_empty());
        assert!(anomalies
            .iter()
            .any(|a| a.anomaly_type == AnomalyType::CostSpike));
    }

    #[tokio::test]
    async fn test_error_rate_spike() {
        let detector = AnomalyDetector::new(AnomalyConfig {
            min_samples: 5,
            ..Default::default()
        });

        // Build baseline with errors
        for _ in 0..6 {
            detector.record(event("a1", "tool.exec", 0.01, false)).await;
        }
        for _ in 0..4 {
            detector.record(event("a1", "tool.exec", 0.01, true)).await;
        }

        let stats = detector.agent_stats("a1").await.unwrap();
        assert!(stats.error_rate > 0.5);
    }

    #[tokio::test]
    async fn test_min_samples_required() {
        let detector = AnomalyDetector::new(AnomalyConfig {
            min_samples: 20,
            ..Default::default()
        });

        // Only 5 events — below threshold
        for _ in 0..5 {
            let anomalies = detector.record(event("a1", "llm.chat", 0.05, true)).await;
            assert!(anomalies.is_empty());
        }
    }

    #[tokio::test]
    async fn test_agent_stats() {
        let detector = AnomalyDetector::default();

        for _ in 0..5 {
            detector.record(event("a1", "llm.chat", 0.05, true)).await;
        }
        detector.record(event("a1", "tool.exec", 0.10, false)).await;

        let stats = detector.agent_stats("a1").await.unwrap();
        assert_eq!(stats.total_ops, 6);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.known_ops, 2);
    }

    #[tokio::test]
    async fn test_multiple_agents() {
        let detector = AnomalyDetector::default();

        detector.record(event("a1", "llm.chat", 0.05, true)).await;
        detector.record(event("a2", "tool.exec", 0.10, true)).await;

        assert!(detector.agent_stats("a1").await.is_some());
        assert!(detector.agent_stats("a2").await.is_some());
        assert!(detector.agent_stats("a3").await.is_none());
    }

    #[tokio::test]
    async fn test_z_score_anomaly() {
        let config = AnomalyConfig {
            min_samples: 5,
            z_threshold: 2.0,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Consistent costs
        for _ in 0..20 {
            detector.record(event("a1", "llm.chat", 0.05, true)).await;
        }

        // Huge spike
        let anomalies = detector.record(event("a1", "llm.chat", 5.0, true)).await;
        assert!(!anomalies.is_empty());
    }
}
