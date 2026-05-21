//! Bias Detection — fairness and bias analysis for AI agent decisions.
//!
//! Implements statistical bias detection methods:
//! - Demographic Parity (DP): P(Ŷ=1|A=0) ≈ P(Ŷ=1|A=1)
//! - Equalized Odds (EO): P(Ŷ=1|Y=1,A=0) ≈ P(Ŷ=1|Y=1,A=1)
//! - Disparate Impact (DI): ratio of selection rates
//! - Z-score based anomaly detection for decision patterns
//!
//! Reference:
//! - Barocas & Hardt, "Fairness and Machine Learning" (fairmlbook.org)
//! - EU AI Act Article 10(5): bias detection obligations
//! - NIST SP 1270: "Towards a Standard for Identifying and Managing Bias in AI"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A decision record for bias analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub decision_id: String,
    pub agent_id: String,
    /// Protected attribute group (e.g., "region:EU", "dept:finance").
    pub group: String,
    /// Decision outcome (true = positive, false = negative).
    pub outcome: bool,
    /// Ground truth label (if available).
    pub ground_truth: Option<bool>,
    /// Timestamp millis.
    pub timestamp_ms: u64,
}

/// Bias metrics for a specific protected attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiasReport {
    pub attribute: String,
    pub group_metrics: HashMap<String, GroupMetrics>,
    /// Disparate Impact ratio (min/max selection rate).
    pub disparate_impact: f64,
    /// Demographic Parity difference.
    pub demographic_parity_diff: f64,
    /// Equalized Odds difference (if ground truth available).
    pub equalized_odds_diff: Option<f64>,
    /// Whether the attribute passes the 80% rule (DI >= 0.8).
    pub passes_80_rule: bool,
    /// Statistical parity violation detected.
    pub parity_violated: bool,
    pub total_decisions: usize,
    pub analyzed_at_ms: u64,
}

/// Metrics for a single group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetrics {
    pub group: String,
    pub total: usize,
    pub positive_outcomes: usize,
    pub negative_outcomes: usize,
    /// Selection rate: positive / total.
    pub selection_rate: f64,
    /// True positive rate (if ground truth available).
    pub true_positive_rate: Option<f64>,
    /// False positive rate (if ground truth available).
    pub false_positive_rate: Option<f64>,
}

/// Bias detector.
pub struct BiasDetector {
    /// Threshold for demographic parity difference (default: 0.1).
    pub parity_threshold: f64,
    /// Disparate impact threshold (default: 0.8 for 80% rule).
    pub di_threshold: f64,
}

impl BiasDetector {
    pub fn new() -> Self {
        Self {
            parity_threshold: 0.1,
            di_threshold: 0.8,
        }
    }

    pub fn with_thresholds(parity_threshold: f64, di_threshold: f64) -> Self {
        Self {
            parity_threshold,
            di_threshold,
        }
    }

    /// Analyze decisions for bias on a specific attribute.
    pub fn analyze(&self, attribute: &str, records: &[DecisionRecord]) -> BiasReport {
        let mut groups: HashMap<String, Vec<&DecisionRecord>> = HashMap::new();
        for record in records {
            groups.entry(record.group.clone()).or_default().push(record);
        }

        let mut group_metrics = HashMap::new();
        for (group_name, group_records) in &groups {
            let total = group_records.len();
            let positive = group_records.iter().filter(|r| r.outcome).count();
            let negative = total - positive;
            let rate = if total > 0 {
                positive as f64 / total as f64
            } else {
                0.0
            };

            let (tpr, fpr) = if group_records.iter().any(|r| r.ground_truth.is_some()) {
                let tp = group_records
                    .iter()
                    .filter(|r| r.outcome && r.ground_truth == Some(true))
                    .count();
                let fn_count = group_records
                    .iter()
                    .filter(|r| !r.outcome && r.ground_truth == Some(true))
                    .count();
                let fp = group_records
                    .iter()
                    .filter(|r| r.outcome && r.ground_truth == Some(false))
                    .count();
                let tn = group_records
                    .iter()
                    .filter(|r| !r.outcome && r.ground_truth == Some(false))
                    .count();

                let tpr = if tp + fn_count > 0 {
                    tp as f64 / (tp + fn_count) as f64
                } else {
                    0.0
                };
                let fpr = if fp + tn > 0 {
                    fp as f64 / (fp + tn) as f64
                } else {
                    0.0
                };
                (Some(tpr), Some(fpr))
            } else {
                (None, None)
            };

            group_metrics.insert(
                group_name.clone(),
                GroupMetrics {
                    group: group_name.clone(),
                    total,
                    positive_outcomes: positive,
                    negative_outcomes: negative,
                    selection_rate: rate,
                    true_positive_rate: tpr,
                    false_positive_rate: fpr,
                },
            );
        }

        let rates: Vec<f64> = group_metrics.values().map(|m| m.selection_rate).collect();
        let min_rate = rates.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_rate = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let disparate_impact = if max_rate > 0.0 {
            min_rate / max_rate
        } else {
            1.0
        };
        let demographic_parity_diff = max_rate - min_rate;

        // Equalized Odds: max difference in TPR across groups
        let equalized_odds_diff = {
            let tprs: Vec<f64> = group_metrics
                .values()
                .filter_map(|m| m.true_positive_rate)
                .collect();
            if tprs.len() >= 2 {
                let min_tpr = tprs.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_tpr = tprs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                Some(max_tpr - min_tpr)
            } else {
                None
            }
        };

        BiasReport {
            attribute: attribute.to_string(),
            group_metrics,
            disparate_impact,
            demographic_parity_diff,
            equalized_odds_diff,
            passes_80_rule: disparate_impact >= self.di_threshold,
            parity_violated: demographic_parity_diff > self.parity_threshold,
            total_decisions: records.len(),
            analyzed_at_ms: now_ms(),
        }
    }

    /// Analyze multiple attributes and return all reports.
    pub fn analyze_all(&self, records: &[DecisionRecord]) -> Vec<BiasReport> {
        // Group records by attribute (extracted from group field "attribute:value")
        let mut attr_records: HashMap<String, Vec<DecisionRecord>> = HashMap::new();
        for record in records {
            // Group field is the attribute value; we analyze per attribute
            attr_records
                .entry(record.group.clone())
                .or_default()
                .push(record.clone());
        }

        // For simplicity, analyze as single attribute with all groups
        vec![self.analyze("group", records)]
    }

    /// Check if a decision pattern shows statistical anomaly (Z-score > 2).
    pub fn detect_anomaly(&self, records: &[DecisionRecord]) -> Vec<AnomalyAlert> {
        let mut alerts = Vec::new();

        // Group by agent_id
        let mut agent_records: HashMap<String, Vec<&DecisionRecord>> = HashMap::new();
        for record in records {
            agent_records
                .entry(record.agent_id.clone())
                .or_default()
                .push(record);
        }

        let overall_rate = if !records.is_empty() {
            records.iter().filter(|r| r.outcome).count() as f64 / records.len() as f64
        } else {
            return alerts;
        };

        let n = records.len() as f64;
        let std_err = if n > 0.0 {
            (overall_rate * (1.0 - overall_rate) / n).sqrt()
        } else {
            0.0
        };

        for (agent_id, agent_recs) in &agent_records {
            let agent_rate =
                agent_recs.iter().filter(|r| r.outcome).count() as f64 / agent_recs.len() as f64;
            let z_score = if std_err > 0.0 {
                (agent_rate - overall_rate).abs() / std_err
            } else {
                0.0
            };

            if z_score > 2.0 {
                alerts.push(AnomalyAlert {
                    agent_id: agent_id.clone(),
                    metric: "decision_rate".to_string(),
                    expected: overall_rate,
                    observed: agent_rate,
                    z_score,
                    severity: if z_score > 3.0 {
                        AlertSeverity::High
                    } else {
                        AlertSeverity::Medium
                    },
                });
            }
        }

        alerts
    }
}

impl Default for BiasDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub agent_id: String,
    pub metric: String,
    pub expected: f64,
    pub observed: f64,
    pub z_score: f64,
    pub severity: AlertSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_records() -> Vec<DecisionRecord> {
        vec![
            DecisionRecord {
                decision_id: "d1".into(),
                agent_id: "a1".into(),
                group: "A".into(),
                outcome: true,
                ground_truth: Some(true),
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d2".into(),
                agent_id: "a1".into(),
                group: "A".into(),
                outcome: true,
                ground_truth: Some(true),
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d3".into(),
                agent_id: "a1".into(),
                group: "A".into(),
                outcome: false,
                ground_truth: Some(false),
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d4".into(),
                agent_id: "a2".into(),
                group: "B".into(),
                outcome: true,
                ground_truth: Some(true),
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d5".into(),
                agent_id: "a2".into(),
                group: "B".into(),
                outcome: false,
                ground_truth: Some(false),
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d6".into(),
                agent_id: "a2".into(),
                group: "B".into(),
                outcome: false,
                ground_truth: Some(false),
                timestamp_ms: 0,
            },
        ]
    }

    #[test]
    fn test_bias_analysis() {
        let detector = BiasDetector::new();
        let report = detector.analyze("group", &make_records());
        assert_eq!(report.group_metrics.len(), 2);
        assert!(report.disparate_impact > 0.0);
        assert!(report.disparate_impact <= 1.0);
    }

    #[test]
    fn test_no_bias_fair_groups() {
        let detector = BiasDetector::new();
        let records = vec![
            DecisionRecord {
                decision_id: "d1".into(),
                agent_id: "a1".into(),
                group: "X".into(),
                outcome: true,
                ground_truth: None,
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d2".into(),
                agent_id: "a1".into(),
                group: "X".into(),
                outcome: false,
                ground_truth: None,
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d3".into(),
                agent_id: "a2".into(),
                group: "Y".into(),
                outcome: true,
                ground_truth: None,
                timestamp_ms: 0,
            },
            DecisionRecord {
                decision_id: "d4".into(),
                agent_id: "a2".into(),
                group: "Y".into(),
                outcome: false,
                ground_truth: None,
                timestamp_ms: 0,
            },
        ];
        let report = detector.analyze("group", &records);
        assert!(report.passes_80_rule);
        assert!(!report.parity_violated);
    }

    #[test]
    fn test_bias_detected_skewed() {
        let detector = BiasDetector::new();
        let mut records = Vec::new();
        // Group A: 90% positive
        for i in 0..10 {
            records.push(DecisionRecord {
                decision_id: format!("d{}", i),
                agent_id: "a1".into(),
                group: "A".into(),
                outcome: i < 9,
                ground_truth: None,
                timestamp_ms: 0,
            });
        }
        // Group B: 10% positive
        for i in 0..10 {
            records.push(DecisionRecord {
                decision_id: format!("d{}", i + 10),
                agent_id: "a2".into(),
                group: "B".into(),
                outcome: i < 1,
                ground_truth: None,
                timestamp_ms: 0,
            });
        }
        let report = detector.analyze("group", &records);
        assert!(!report.passes_80_rule);
        assert!(report.parity_violated);
    }

    #[test]
    fn test_anomaly_detection() {
        let detector = BiasDetector::new();
        let mut records = Vec::new();
        // Normal agents: 50% positive rate
        for i in 0..100 {
            records.push(DecisionRecord {
                decision_id: format!("d{}", i),
                agent_id: "normal".into(),
                group: "A".into(),
                outcome: i % 2 == 0,
                ground_truth: None,
                timestamp_ms: 0,
            });
        }
        // Anomalous agent: 100% positive rate
        for i in 0..20 {
            records.push(DecisionRecord {
                decision_id: format!("d{}", i + 100),
                agent_id: "anomaly".into(),
                group: "A".into(),
                outcome: true,
                ground_truth: None,
                timestamp_ms: 0,
            });
        }
        let alerts = detector.detect_anomaly(&records);
        assert!(alerts.iter().any(|a| a.agent_id == "anomaly"));
    }

    #[test]
    fn test_serialization() {
        let detector = BiasDetector::new();
        let report = detector.analyze("group", &make_records());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("disparate_impact"));
    }
}
