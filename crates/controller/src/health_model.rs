//! Multi-layer Health Model — Healthy / Liveness / Readiness / Degraded / Draining.
//!
//! ## States
//! | State      | Traffic | Liveness | Readiness | Notes                     |
//! |------------|---------|----------|----------|---------------------------|
//! | Healthy    | ✓       | ✓        | ✓        | Full capacity             |
//! | Liveness   | partial | ✓        | ✗        | Passing liveness, not ready|
//! | Readiness  | partial | ✓        | ✓        | May have warmed caches     |
//! | Degraded   | reduced | ✓        | ✓        | Functional but slow        |
//! | Draining   | none    | ✓        | ✗        | Gracefully draining        |
//!
//! ## Usage
//! ```ignore
//! let mut model = HealthModel::new();
//! model.set_state(HealthState::Healthy);
//! assert!(!model.should_receive_traffic()); // only Healthy gets full traffic
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Health state enumeration ordered from most to least healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum HealthState {
    /// Fully operational — all systems nominal.
    #[default]
    Healthy = 4,
    /// Liveness probe passes, readiness not yet confirmed.
    Liveness = 3,
    /// Agent was ready but is now experiencing latency or elevated errors.
    Readiness = 2,
    /// Functional but operating at reduced capacity or elevated error rate.
    Degraded = 1,
    /// Not accepting new work; draining in-flight requests.
    Draining = 0,
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthState::Healthy => write!(f, "Healthy"),
            HealthState::Liveness => write!(f, "Liveness"),
            HealthState::Readiness => write!(f, "Readiness"),
            HealthState::Degraded => write!(f, "Degraded"),
            HealthState::Draining => write!(f, "Draining"),
        }
    }
}

/// Result of a health evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvaluation {
    pub state: HealthState,
    pub should_receive_traffic: bool,
    pub traffic_fraction: f64, // 0.0 – 1.0
    pub draining_inflight: bool,
    pub readiness_probe_pass: bool,
    pub liveness_probe_pass: bool,
    pub reasons: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// HealthModel — computes composite health from multiple probes.
#[derive(Debug, Clone)]
pub struct HealthModel {
    state: HealthState,
    readiness_probe: bool,
    liveness_probe: bool,
    error_rate: f64,
    latency_p50_ms: f64,
    /// Metadata for observability (labels, version, etc.)
    labels: HashMap<String, String>,
}

impl Default for HealthModel {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthModel {
    pub fn new() -> Self {
        Self {
            state: HealthState::Healthy,
            readiness_probe: true,
            liveness_probe: true,
            error_rate: 0.0,
            latency_p50_ms: 0.0,
            labels: HashMap::new(),
        }
    }

    /// Manually set the health state (e.g., from a Kubernetes probe result).
    pub fn set_state(&mut self, state: HealthState) {
        self.state = state;
    }

    /// Update probe results.
    pub fn update_probes(&mut self, readiness: bool, liveness: bool) {
        self.readiness_probe = readiness;
        self.liveness_probe = liveness;
        self.recompute();
    }

    /// Update metrics.
    pub fn update_metrics(&mut self, error_rate: f64, latency_p50_ms: f64) {
        self.error_rate = error_rate;
        self.latency_p50_ms = latency_p50_ms;
        self.recompute();
    }

    /// Add a label for filtering/grouping.
    pub fn set_label(&mut self, key: &str, value: &str) {
        self.labels.insert(key.to_string(), value.to_string());
    }

    fn recompute(&mut self) {
        if !self.liveness_probe {
            self.state = HealthState::Draining;
        } else if !self.readiness_probe {
            self.state = HealthState::Liveness;
        } else if self.error_rate > 0.1 || self.latency_p50_ms > 1000.0 {
            self.state = HealthState::Degraded;
        } else if self.error_rate > 0.01 || self.latency_p50_ms > 200.0 {
            self.state = HealthState::Readiness;
        } else {
            self.state = HealthState::Healthy;
        }
    }

    /// Return the current health state.
    pub fn state(&self) -> HealthState {
        self.state
    }

    /// Whether the load balancer should route traffic to this agent.
    pub fn should_receive_traffic(&self) -> bool {
        matches!(
            self.state,
            HealthState::Healthy
                | HealthState::Liveness
                | HealthState::Readiness
                | HealthState::Degraded
        )
    }

    /// Fraction of traffic to send (for gradual traffic shifting).
    pub fn traffic_fraction(&self) -> f64 {
        match self.state {
            HealthState::Healthy => 1.0,
            HealthState::Liveness | HealthState::Readiness => 0.5,
            HealthState::Degraded => 0.25,
            HealthState::Draining => 0.0,
        }
    }

    /// Whether in-flight requests should be completed before shutdown.
    pub fn draining_inflight(&self) -> bool {
        matches!(self.state, HealthState::Draining)
    }

    /// Full health evaluation snapshot.
    pub fn evaluate(&self) -> HealthEvaluation {
        HealthEvaluation {
            state: self.state,
            should_receive_traffic: self.should_receive_traffic(),
            traffic_fraction: self.traffic_fraction(),
            draining_inflight: self.draining_inflight(),
            readiness_probe_pass: self.readiness_probe,
            liveness_probe_pass: self.liveness_probe,
            reasons: self.build_reasons(),
            timestamp: Utc::now(),
        }
    }

    fn build_reasons(&self) -> Vec<String> {
        let mut r = Vec::new();
        if !self.liveness_probe {
            r.push("liveness probe failing".into());
        }
        if !self.readiness_probe {
            r.push("readiness probe failing".into());
        }
        if self.error_rate > 0.1 {
            r.push(format!("high error rate: {:.1}%", self.error_rate * 100.0));
        }
        if self.latency_p50_ms > 1000.0 {
            r.push(format!("high latency p50: {:.0}ms", self.latency_p50_ms));
        } else if self.latency_p50_ms > 200.0 {
            r.push(format!(
                "elevated latency p50: {:.0}ms",
                self.latency_p50_ms
            ));
        }
        if r.is_empty() {
            r.push("all checks passing".into());
        }
        r
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healthy_by_default() {
        let model = HealthModel::new();
        assert_eq!(model.state(), HealthState::Healthy);
        assert!(model.should_receive_traffic());
        assert!((model.traffic_fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_liveness_failing_draining() {
        let mut model = HealthModel::new();
        model.update_probes(false, false); // both failing
        assert_eq!(model.state(), HealthState::Draining);
        assert!(!model.should_receive_traffic());
        assert!((model.traffic_fraction() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_readiness_failing_liveness_passes() {
        let mut model = HealthModel::new();
        model.update_probes(false, true); // readiness fails, liveness passes
        assert_eq!(model.state(), HealthState::Liveness);
        assert!(model.should_receive_traffic()); // liveness passes → can receive
        assert!((model.traffic_fraction() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_degraded_on_high_error_rate() {
        let mut model = HealthModel::new();
        model.update_probes(true, true);
        model.update_metrics(0.15, 50.0); // 15% error rate
        assert_eq!(model.state(), HealthState::Degraded);
        assert!((model.traffic_fraction() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_readiness_on_moderate_latency() {
        let mut model = HealthModel::new();
        model.update_probes(true, true);
        model.update_metrics(0.005, 350.0); // slightly elevated
        assert_eq!(model.state(), HealthState::Readiness);
    }

    #[test]
    fn test_healthy_with_nominal_metrics() {
        let mut model = HealthModel::new();
        model.update_probes(true, true);
        model.update_metrics(0.0, 20.0);
        assert_eq!(model.state(), HealthState::Healthy);
    }

    #[test]
    fn test_evaluate_includes_reasons() {
        let mut model = HealthModel::new();
        model.update_metrics(0.5, 0.0); // catastrophic error rate
        let eval = model.evaluate();
        assert!(!eval.reasons.is_empty());
        assert!(eval.reasons.iter().any(|r| r.contains("error")));
    }

    #[test]
    fn test_draining_inflight_is_true_when_draining() {
        let mut model = HealthModel::new();
        model.set_state(HealthState::Draining);
        assert!(model.draining_inflight());
    }

    #[test]
    fn test_labels_stored() {
        let mut model = HealthModel::new();
        model.set_label("version", "1.2.3");
        model.set_label("region", "us-east");
        assert_eq!(model.labels.get("version"), Some(&"1.2.3".into()));
    }
}
