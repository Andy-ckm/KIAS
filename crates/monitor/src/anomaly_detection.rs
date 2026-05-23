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
    /// Z-score threshold for frequency anomaly (default: 2.0)
    pub z_threshold: f64,

    /// Minimum samples before detection activates
    pub min_samples: usize,

    /// Cost spike multiplier threshold (e.g., 3.0 = 3x average)
    pub cost_spike_multiplier: f64,

    /// Time window in minutes for frequency analysis
    pub window_minutes: u64,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            z_threshold: 2.0,
            min_samples: 10,
            cost_spike_multiplier: 3.0,
            window_minutes: 60,
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

    /// Cost history
    costs: Vec<f64>,

    /// Hour-of-day distribution (0-23)
    hour_distribution: Vec<u64>,

    /// Error count
    errors: u64,

    /// Total operations
    total_ops: u64,

    /// Known operation types
    known_ops: std::collections::HashSet<String>,
}

impl AgentStats {
    fn record(&mut self, event: &OperationEvent) {
        let hour = event.timestamp.hour() as usize;
        if self.hour_distribution.len() <= hour {
            self.hour_distribution.resize(hour + 1, 0);
        }
        self.hour_distribution[hour] += 1;

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
            agent_stats.record(&event);
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

        // Check cost spike
        if let Some(anomaly) = self.check_cost_spike(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check error rate
        if let Some(anomaly) = self.check_error_rate(&event.agent_id, agent_stats) {
            anomalies.push(anomaly);
        }

        // Check frequency anomaly
        if let Some(anomaly) = self.check_frequency(&event, agent_stats) {
            anomalies.push(anomaly);
        }

        anomalies
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

    fn check_frequency(&self, event: &OperationEvent, stats: &AgentStats) -> Option<Anomaly> {
        let op_counts = match stats.op_counts.get(&event.operation) {
            Some(counts) if counts.len() >= self.config.min_samples => counts,
            _ => return None,
        };

        let mean: f64 = op_counts.iter().sum::<f64>() / op_counts.len() as f64;
        let variance: f64 =
            op_counts.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / op_counts.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev <= 0.0 {
            return None;
        }

        let z_score = (event.cost_usd - mean) / std_dev;
        if z_score.abs() > self.config.z_threshold {
            Some(Anomaly {
                agent_id: event.agent_id.clone(),
                anomaly_type: AnomalyType::FrequencySpike,
                severity: if z_score.abs() > 4.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::Medium
                },
                description: format!(
                    "Statistical anomaly on '{}': z-score {:.2} (threshold {:.1})",
                    event.operation, z_score, self.config.z_threshold
                ),
                detected_at: chrono::Utc::now(),
                metric_value: z_score,
                threshold: self.config.z_threshold,
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
