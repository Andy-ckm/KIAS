//! # SLA Products - Service Level Agreement Tiers
//!
//! Implements SLA tiers (Standard/High-Availability/Regulatory),
//! metrics tracking, and violation detection with compensation triggers.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

/// SLA tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SLATier {
    /// Standard tier - basic SLA
    Standard,
    /// High Availability tier - enhanced uptime
    HighAvailability,
    /// Regulatory tier - for compliance-critical systems
    Regulatory,
}

impl SLATier {
    pub fn name(&self) -> &'static str {
        match self {
            SLATier::Standard => "Standard",
            SLATier::HighAvailability => "High Availability",
            SLATier::Regulatory => "Regulatory",
        }
    }

    pub fn default_thresholds(&self) -> SLAThresholds {
        match self {
            SLATier::Standard => SLAThresholds {
                availability: 99.5,
                latency_p99_ms: 500.0,
                throughput_rps: 100.0,
                recovery_time_minutes: 30.0,
            },
            SLATier::HighAvailability => SLAThresholds {
                availability: 99.9,
                latency_p99_ms: 200.0,
                throughput_rps: 500.0,
                recovery_time_minutes: 15.0,
            },
            SLATier::Regulatory => SLAThresholds {
                availability: 99.99,
                latency_p99_ms: 100.0,
                throughput_rps: 1000.0,
                recovery_time_minutes: 5.0,
            },
        }
    }
}

/// SLA metric thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAThresholds {
    pub availability: f64,           // Percentage uptime (e.g., 99.9%)
    pub latency_p99_ms: f64,         // P99 latency in milliseconds
    pub throughput_rps: f64,         // Requests per second
    pub recovery_time_minutes: f64,  // Max recovery time after incident
}

/// SLA metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SLAMetric {
    Availability,
    Latency,
    Throughput,
    RecoveryTime,
}

impl SLAMetric {
    pub fn name(&self) -> &'static str {
        match self {
            SLAMetric::Availability => "Availability",
            SLAMetric::Latency => "Latency",
            SLAMetric::Throughput => "Throughput",
            SLAMetric::RecoveryTime => "Recovery Time",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            SLAMetric::Availability => "%",
            SLAMetric::Latency => "ms",
            SLAMetric::Throughput => "req/s",
            SLAMetric::RecoveryTime => "min",
        }
    }
}

/// A recorded SLA metric measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAMeasurement {
    pub metric: SLAMetric,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub tier: SLATier,
}

impl SLAMeasurement {
    pub fn availability(percent: f64, tier: SLATier) -> Self {
        Self {
            metric: SLAMetric::Availability,
            value: percent,
            timestamp: Utc::now(),
            tier,
        }
    }

    pub fn latency(ms: f64, tier: SLATier) -> Self {
        Self {
            metric: SLAMetric::Latency,
            value: ms,
            timestamp: Utc::now(),
            tier,
        }
    }

    pub fn throughput(rps: f64, tier: SLATier) -> Self {
        Self {
            metric: SLAMetric::Throughput,
            value: rps,
            timestamp: Utc::now(),
            tier,
        }
    }

    pub fn recovery_time(minutes: f64, tier: SLATier) -> Self {
        Self {
            metric: SLAMetric::RecoveryTime,
            value: minutes,
            timestamp: Utc::now(),
            tier,
        }
    }
}

/// SLA violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAViolation {
    pub id: String,
    pub metric: SLAMetric,
    pub expected: f64,
    pub actual: f64,
    pub tier: SLATier,
    pub timestamp: DateTime<Utc>,
    pub duration_seconds: i64,
    pub compensation_eligible: bool,
    pub compensation_amount: f64,
}

impl SLAViolation {
    pub fn new(
        metric: SLAMetric,
        expected: f64,
        actual: f64,
        tier: SLATier,
        duration_seconds: i64,
    ) -> Self {
        let compensation_eligible = Self::calculate_eligibility(tier, metric, expected, actual);
        let compensation_amount = if compensation_eligible {
            Self::calculate_compensation(tier, metric, expected, actual, duration_seconds)
        } else {
            0.0
        };

        Self {
            id: format!("vio-{}", uuid::Uuid::new_v4()),
            metric,
            expected,
            actual,
            tier,
            timestamp: Utc::now(),
            duration_seconds,
            compensation_eligible,
            compensation_amount,
        }
    }

    fn calculate_eligibility(tier: SLATier, metric: SLAMetric, expected: f64, actual: f64) -> bool {
        let threshold = match tier {
            SLATier::Standard => 0.05,      // 5% deviation
            SLATier::HighAvailability => 0.02, // 2% deviation
            SLATier::Regulatory => 0.01,    // 1% deviation
        };

        match metric {
            SLAMetric::Availability => actual < expected * (1.0 - threshold),
            SLAMetric::Latency => actual > expected * (1.0 + threshold),
            SLAMetric::Throughput => actual < expected * (1.0 - threshold),
            SLAMetric::RecoveryTime => actual > expected * (1.0 + threshold),
        }
    }

    fn calculate_compensation(
        tier: SLATier,
        metric: SLAMetric,
        expected: f64,
        actual: f64,
        duration_seconds: i64,
    ) -> f64 {
        // Base compensation rates per tier (in dollars per hour of violation)
        let base_rate = match tier {
            SLATier::Standard => 100.0,
            SLATier::HighAvailability => 500.0,
            SLATier::Regulatory => 2000.0,
        };

        // Calculate severity factor
        let severity_factor = match metric {
            SLAMetric::Availability => {
                let deviation_pct = ((expected - actual) / expected * 100.0).abs();
                deviation_pct / 100.0
            }
            SLAMetric::Latency => {
                let deviation_pct = ((actual - expected) / expected * 100.0).abs();
                deviation_pct / 100.0
            }
            SLAMetric::Throughput => {
                let deviation_pct = ((expected - actual) / expected * 100.0).abs();
                deviation_pct / 100.0
            }
            SLAMetric::RecoveryTime => {
                let deviation_pct = ((actual - expected) / expected * 100.0).abs();
                deviation_pct / 100.0
            }
        };

        let hours = duration_seconds as f64 / 3600.0;
        base_rate * severity_factor * hours.max(1.0) // Minimum 1 hour
    }
}

/// SLA violation checker
#[derive(Debug, Clone)]
pub struct SLAViolationChecker {
    thresholds: HashMap<SLATier, SLAThresholds>,
}

impl Default for SLAViolationChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SLAViolationChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            thresholds: HashMap::new(),
        };
        checker.thresholds.insert(SLATier::Standard, SLATier::Standard.default_thresholds());
        checker.thresholds.insert(SLATier::HighAvailability, SLATier::HighAvailability.default_thresholds());
        checker.thresholds.insert(SLATier::Regulatory, SLATier::Regulatory.default_thresholds());
        checker
    }

    pub fn set_thresholds(&mut self, tier: SLATier, thresholds: SLAThresholds) {
        self.thresholds.insert(tier, thresholds);
    }

    pub fn get_thresholds(&self, tier: SLATier) -> Option<&SLAThresholds> {
        self.thresholds.get(&tier)
    }

    pub fn check(&self, measurement: &SLAMeasurement) -> Option<SLAViolation> {
        let thresholds = self.thresholds.get(&measurement.tier)?;

        let (expected, actual, is_violation) = match measurement.metric {
            SLAMetric::Availability => {
                (thresholds.availability, measurement.value, measurement.value < thresholds.availability)
            }
            SLAMetric::Latency => {
                (thresholds.latency_p99_ms, measurement.value, measurement.value > thresholds.latency_p99_ms)
            }
            SLAMetric::Throughput => {
                (thresholds.throughput_rps, measurement.value, measurement.value < thresholds.throughput_rps)
            }
            SLAMetric::RecoveryTime => {
                (thresholds.recovery_time_minutes, measurement.value, measurement.value > thresholds.recovery_time_minutes)
            }
        };

        if is_violation {
            Some(SLAViolation::new(
                measurement.metric,
                expected,
                actual,
                measurement.tier,
                3600, // Default 1 hour duration
            ))
        } else {
            None
        }
    }

    pub fn check_all(&self, measurements: &[SLAMeasurement]) -> Vec<SLAViolation> {
        measurements.iter().filter_map(|m| self.check(m)).collect()
    }
}

/// SLA product configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAProduct {
    pub name: String,
    pub tier: SLATier,
    pub monthly_price: f64,
    pub description: String,
    pub included_features: Vec<String>,
}

impl SLAProduct {
    pub fn standard() -> Self {
        Self {
            name: "Standard SLA".to_string(),
            tier: SLATier::Standard,
            monthly_price: 999.0,
            description: "Basic SLA with 99.5% uptime guarantee".to_string(),
            included_features: vec![
                "99.5% Availability".to_string(),
                "500ms P99 Latency".to_string(),
                "Email Support".to_string(),
                "Monthly Reports".to_string(),
            ],
        }
    }

    pub fn high_availability() -> Self {
        Self {
            name: "High Availability SLA".to_string(),
            tier: SLATier::HighAvailability,
            monthly_price: 4999.0,
            description: "Enhanced SLA with 99.9% uptime guarantee".to_string(),
            included_features: vec![
                "99.9% Availability".to_string(),
                "200ms P99 Latency".to_string(),
                "24/7 Phone Support".to_string(),
                "Real-time Monitoring".to_string(),
                "Priority Incident Response".to_string(),
            ],
        }
    }

    pub fn regulatory() -> Self {
        Self {
            name: "Regulatory SLA".to_string(),
            tier: SLATier::Regulatory,
            monthly_price: 19999.0,
            description: "Compliance-grade SLA with 99.99% uptime".to_string(),
            included_features: vec![
                "99.99% Availability".to_string(),
                "100ms P99 Latency".to_string(),
                "Dedicated Support Engineer".to_string(),
                "Real-time Monitoring".to_string(),
                "Compliance Reporting".to_string(),
                "SLA Credits for Violations".to_string(),
                "Quarterly Business Reviews".to_string(),
            ],
        }
    }
}

/// SLA manager for tracking and reporting
#[derive(Debug, Clone)]
pub struct SLAManager {
    checker: SLAViolationChecker,
    measurements: Vec<SLAMeasurement>,
    violations: Vec<SLAViolation>,
}

impl Default for SLAManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SLAManager {
    pub fn new() -> Self {
        Self {
            checker: SLAViolationChecker::new(),
            measurements: Vec::new(),
            violations: Vec::new(),
        }
    }

    pub fn record(&mut self, measurement: SLAMeasurement) {
        self.measurements.push(measurement.clone());
        if let Some(violation) = self.checker.check(&measurement) {
            self.violations.push(violation);
        }
    }

    pub fn get_violations(&self) -> &[SLAViolation] {
        &self.violations
    }

    pub fn get_measurements(&self) -> &[SLAMeasurement] {
        &self.measurements
    }

    pub fn total_compensation(&self) -> f64 {
        self.violations.iter().map(|v| v.compensation_amount).sum()
    }

    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    pub fn generate_report(&self, tier: SLATier) -> SLAReport {
        let tier_measurements: Vec<_> = self.measurements.iter().filter(|m| m.tier == tier).collect();
        let tier_violations: Vec<_> = self.violations.iter().filter(|v| v.tier == tier).collect();

        let latest_availability = tier_measurements
            .iter()
            .filter(|m| m.metric == SLAMetric::Availability)
            .last()
            .map(|m| m.value);

        let latest_latency = tier_measurements
            .iter()
            .filter(|m| m.metric == SLAMetric::Latency)
            .last()
            .map(|m| m.value);

        let total_compensation: f64 = tier_violations.iter().map(|v| v.compensation_amount).sum();

        SLAReport {
            tier,
            period_start: tier_measurements.first().map(|m| m.timestamp),
            period_end: tier_measurements.last().map(|m| m.timestamp),
            violation_count: tier_violations.len(),
            total_compensation,
            latest_availability,
            latest_latency,
            compliance_status: if tier_violations.is_empty() { "Compliant" } else { "Violated" }.to_string(),
        }
    }
}

/// SLA report summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAReport {
    pub tier: SLATier,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub violation_count: usize,
    pub total_compensation: f64,
    pub latest_availability: Option<f64>,
    pub latest_latency: Option<f64>,
    pub compliance_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sla_tier_names() {
        assert_eq!(SLATier::Standard.name(), "Standard");
        assert_eq!(SLATier::HighAvailability.name(), "High Availability");
        assert_eq!(SLATier::Regulatory.name(), "Regulatory");
    }

    #[test]
    fn test_sla_tier_default_thresholds() {
        let thresholds = SLATier::Standard.default_thresholds();
        assert_eq!(thresholds.availability, 99.5);

        let ha_thresholds = SLATier::HighAvailability.default_thresholds();
        assert_eq!(ha_thresholds.availability, 99.9);

        let reg_thresholds = SLATier::Regulatory.default_thresholds();
        assert_eq!(reg_thresholds.availability, 99.99);
    }

    #[test]
    fn test_sla_metric_creation() {
        let m = SLAMeasurement::availability(99.5, SLATier::Standard);
        assert_eq!(m.metric, SLAMetric::Availability);
        assert_eq!(m.value, 99.5);
    }

    #[test]
    fn test_sla_violation_creation() {
        let v = SLAViolation::new(SLAMetric::Availability, 99.9, 99.0, SLATier::HighAvailability, 3600);
        assert_eq!(v.expected, 99.9);
        assert_eq!(v.actual, 99.0);
        assert!(v.compensation_eligible);
    }

    #[test]
    fn test_sla_violation_no_eligibility() {
        // Very small deviation should not be eligible
        let v = SLAViolation::new(SLAMetric::Latency, 100.0, 101.0, SLATier::Standard, 3600);
        assert!(!v.compensation_eligible);
    }

    #[test]
    fn test_violation_checker_new() {
        let checker = SLAViolationChecker::new();
        assert!(checker.get_thresholds(SLATier::Standard).is_some());
        assert!(checker.get_thresholds(SLATier::HighAvailability).is_some());
        assert!(checker.get_thresholds(SLATier::Regulatory).is_some());
    }

    #[test]
    fn test_violation_checker_no_violation() {
        let checker = SLAViolationChecker::new();
        let m = SLAMeasurement::availability(99.9, SLATier::HighAvailability);
        assert!(checker.check(&m).is_none());
    }

    #[test]
    fn test_violation_checker_with_violation() {
        let checker = SLAViolationChecker::new();
        let m = SLAMeasurement::availability(99.0, SLATier::HighAvailability); // Below 99.9
        assert!(checker.check(&m).is_some());
    }

    #[test]
    fn test_violation_checker_latency_violation() {
        let checker = SLAViolationChecker::new();
        let m = SLAMeasurement::latency(300.0, SLATier::Standard); // Above 500ms threshold
        assert!(checker.check(&m).is_none()); // 300 < 500, no violation

        let m2 = SLAMeasurement::latency(600.0, SLATier::Standard); // Above 500ms
        assert!(checker.check(&m2).is_some());
    }

    #[test]
    fn test_sla_product_standard() {
        let product = SLAProduct::standard();
        assert_eq!(product.tier, SLATier::Standard);
        assert_eq!(product.monthly_price, 999.0);
    }

    #[test]
    fn test_sla_product_high_availability() {
        let product = SLAProduct::high_availability();
        assert_eq!(product.tier, SLATier::HighAvailability);
        assert_eq!(product.monthly_price, 4999.0);
    }

    #[test]
    fn test_sla_product_regulatory() {
        let product = SLAProduct::regulatory();
        assert_eq!(product.tier, SLATier::Regulatory);
        assert_eq!(product.monthly_price, 19999.0);
    }

    #[test]
    fn test_sla_manager_record() {
        let mut manager = SLAManager::new();
        manager.record(SLAMeasurement::availability(99.9, SLATier::HighAvailability));
        assert_eq!(manager.measurements.len(), 1);
    }

    #[test]
    fn test_sla_manager_violation_tracking() {
        let mut manager = SLAManager::new();
        manager.record(SLAMeasurement::availability(99.0, SLATier::HighAvailability)); // violation
        manager.record(SLAMeasurement::availability(99.9, SLATier::HighAvailability)); // ok

        assert_eq!(manager.violation_count(), 1);
    }

    #[test]
    fn test_sla_manager_total_compensation() {
        let mut manager = SLAManager::new();
        manager.record(SLAMeasurement::availability(99.0, SLATier::Regulatory)); // violation
        assert!(manager.total_compensation() > 0.0);
    }

    #[test]
    fn test_sla_manager_report() {
        let mut manager = SLAManager::new();
        manager.record(SLAMeasurement::availability(99.9, SLATier::Standard));
        manager.record(SLAMeasurement::latency(200.0, SLATier::Standard));

        let report = manager.generate_report(SLATier::Standard);
        assert_eq!(report.violation_count, 0);
        assert_eq!(report.compliance_status, "Compliant");
    }

    #[test]
    fn test_measurement_factory_methods() {
        let a = SLAMeasurement::availability(99.5, SLATier::Standard);
        let l = SLAMeasurement::latency(100.0, SLATier::Standard);
        let t = SLAMeasurement::throughput(500.0, SLATier::Standard);
        let r = SLAMeasurement::recovery_time(15.0, SLATier::Standard);

        assert_eq!(a.metric, SLAMetric::Availability);
        assert_eq!(l.metric, SLAMetric::Latency);
        assert_eq!(t.metric, SLAMetric::Throughput);
        assert_eq!(r.metric, SLAMetric::RecoveryTime);
    }
}
