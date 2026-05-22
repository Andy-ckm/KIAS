//! Canary Release — progressive traffic shifting with automated rollback.
//!
//! Implements canary release strategy for agent deployments:
//! - Gradual traffic shifting (1% → 5% → 25% → 100%)
//! - Automatic rollback on error rate threshold
//! - Weighted routing between stable and canary versions
//! - Metric observation integration
//!
//! # Canary Strategy
//!
//! ```text
//! Stable (v1) ──────► Traffic
//!        └─► Canary (v2) ──► Progressively more traffic
//!                              ↓
//!                         Auto-rollback if error rate > threshold
//! ```
//!
//! # Example
//!
//! ```
//! use kias_scheduler::canary_release::{CanaryRelease, CanaryConfig, CanaryStatus, WeightMap};
//!
//! let config = CanaryConfig::default()
//!     .with_initial_weight(1.0)      // 1% to canary
//!     .with_max_weight(50.0)         // Max 50% canary before full rollout
//!     .with_rollout_interval_secs(60);
//!
//! let release = CanaryRelease::new("agent-v2".to_string(), config);
//! assert_eq!(release.status(), CanaryStatus::Pending);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

/// Weight map: version → traffic percentage (0.0-100.0).
pub type WeightMap = HashMap<String, f64>;

/// Canary release status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanaryStatus {
    /// Canary not yet started.
    Pending,
    /// Traffic being shifted gradually.
    InProgress,
    /// Canary fully promoted (100% traffic).
    Promoted,
    /// Canary rolled back to stable.
    RolledBack,
    /// Canary manually paused.
    Paused,
    /// Canary failed (error threshold exceeded).
    Failed,
}

impl fmt::Display for CanaryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanaryStatus::Pending => write!(f, "Pending"),
            CanaryStatus::InProgress => write!(f, "InProgress"),
            CanaryStatus::Promoted => write!(f, "Promoted"),
            CanaryStatus::RolledBack => write!(f, "RolledBack"),
            CanaryStatus::Paused => write!(f, "Paused"),
            CanaryStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Canary analysis result from metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryAnalysis {
    /// Error rate observed on stable version (%).
    pub stable_error_rate: f64,
    /// Error rate observed on canary version (%).
    pub canary_error_rate: f64,
    /// Latency p50 on stable (ms).
    pub stable_latency_p50_ms: f64,
    /// Latency p50 on canary (ms).
    pub canary_latency_p50_ms: f64,
    /// Latency p99 on stable (ms).
    pub stable_latency_p99_ms: f64,
    /// Latency p99 on canary (ms).
    pub canary_latency_p99_ms: f64,
    /// Whether analysis passed safety checks.
    pub passed: bool,
    /// Warning messages.
    pub warnings: Vec<String>,
}

impl Default for CanaryAnalysis {
    fn default() -> Self {
        Self {
            stable_error_rate: 0.0,
            canary_error_rate: 0.0,
            stable_latency_p50_ms: 0.0,
            canary_latency_p50_ms: 0.0,
            stable_latency_p99_ms: 0.0,
            canary_latency_p99_ms: 0.0,
            passed: true,
            warnings: vec![],
        }
    }
}

impl CanaryAnalysis {
    /// Check if canary error rate is acceptable compared to stable.
    pub fn is_error_rate_acceptable(&self, max_delta: f64) -> bool {
        (self.canary_error_rate - self.stable_error_rate).abs() <= max_delta
    }

    /// Check if canary latency is acceptable.
    pub fn is_latency_acceptable(&self, max_p99_increase_ratio: f64) -> bool {
        if self.stable_latency_p99_ms == 0.0 {
            return true;
        }
        self.canary_latency_p99_ms <= self.stable_latency_p99_ms * max_p99_increase_ratio
    }

    /// Compute canary health score (0.0-100.0).
    pub fn health_score(&self) -> f64 {
        let error_score = 50.0 * (1.0 - self.canary_error_rate.min(1.0));
        let latency_ratio = if self.stable_latency_p99_ms > 0.0 {
            (self.stable_latency_p99_ms / self.canary_latency_p99_ms.max(1.0)).min(2.0)
        } else {
            50.0
        };
        let latency_score = 50.0 * (latency_ratio / 2.0).min(1.0);
        error_score + latency_score
    }
}

/// Canary release configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Stable version name.
    pub stable_version: String,
    /// Canary version name.
    pub canary_version: String,
    /// Initial weight for canary (%).
    pub initial_weight: f64,
    /// Maximum weight for canary before promotion (%).
    pub max_weight: f64,
    /// Weight increment per step (%).
    pub weight_increment: f64,
    /// Interval between weight increases in seconds.
    pub rollout_interval_secs: u64,
    /// Maximum error rate delta allowed (% difference).
    pub max_error_rate_delta: f64,
    /// Maximum latency p99 increase ratio.
    pub max_latency_increase_ratio: f64,
    /// Minimum health score to continue (0-100).
    pub min_health_score: f64,
    /// Enable automatic rollback.
    pub auto_rollback: bool,
    /// Abort signal channel name.
    pub abort_signal: Option<String>,
}

impl Default for CanaryConfig {
    fn default() -> Self {
        Self {
            stable_version: "stable".to_string(),
            canary_version: "canary".to_string(),
            initial_weight: 1.0,
            max_weight: 100.0,
            weight_increment: 10.0,
            rollout_interval_secs: 60,
            max_error_rate_delta: 1.0,
            max_latency_increase_ratio: 1.5,
            min_health_score: 70.0,
            auto_rollback: true,
            abort_signal: None,
        }
    }
}

impl CanaryConfig {
    pub fn new(stable: &str, canary: &str) -> Self {
        Self {
            stable_version: stable.to_string(),
            canary_version: canary.to_string(),
            ..Default::default()
        }
    }

    pub fn with_initial_weight(mut self, weight: f64) -> Self {
        self.initial_weight = weight;
        self
    }

    pub fn with_max_weight(mut self, weight: f64) -> Self {
        self.max_weight = weight;
        self
    }

    pub fn with_rollout_interval_secs(mut self, secs: u64) -> Self {
        self.rollout_interval_secs = secs;
        self
    }
}

/// Canary release state.
#[derive(Debug, Clone)]
pub struct CanaryState {
    pub status: CanaryStatus,
    pub current_weight: f64,
    pub step_count: u32,
    pub last_analysis: Option<CanaryAnalysis>,
    pub started_at: Option<Instant>,
    pub updated_at: Option<Instant>,
    pub rollback_reason: Option<String>,
}

impl CanaryState {
    pub fn new(initial_weight: f64) -> Self {
        Self {
            status: CanaryStatus::Pending,
            current_weight: initial_weight,
            step_count: 0,
            last_analysis: None,
            started_at: None,
            updated_at: Some(Instant::now()),
            rollback_reason: None,
        }
    }
}

/// Canary release manager.
#[derive(Debug)]
pub struct CanaryRelease {
    name: String,
    config: CanaryConfig,
    state: CanaryState,
}

impl CanaryRelease {
    /// Create a new canary release.
    pub fn new(name: String, config: CanaryConfig) -> Self {
        let state = CanaryState::new(config.initial_weight);
        Self {
            name,
            config,
            state,
        }
    }

    /// Get the release name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current status.
    pub fn status(&self) -> CanaryStatus {
        self.state.status
    }

    /// Get current canary weight (%).
    pub fn canary_weight(&self) -> f64 {
        self.state.current_weight
    }

    /// Get stable weight (%).
    pub fn stable_weight(&self) -> f64 {
        100.0 - self.state.current_weight
    }

    /// Get current weight map.
    pub fn weight_map(&self) -> WeightMap {
        WeightMap::from([
            (self.config.stable_version.clone(), self.stable_weight()),
            (
                self.config.canary_version.clone(),
                self.state.current_weight,
            ),
        ])
    }

    /// Get the canary config.
    pub fn config(&self) -> &CanaryConfig {
        &self.config
    }

    /// Get current state.
    pub fn state(&self) -> &CanaryState {
        &self.state
    }

    /// Start the canary release.
    pub fn start(&mut self) {
        if self.state.status == CanaryStatus::Pending {
            self.state.status = CanaryStatus::InProgress;
            self.state.started_at = Some(Instant::now());
            self.state.updated_at = Some(Instant::now());
        }
    }

    /// Pause the canary release.
    pub fn pause(&mut self) {
        if self.state.status == CanaryStatus::InProgress {
            self.state.status = CanaryStatus::Paused;
            self.state.updated_at = Some(Instant::now());
        }
    }

    /// Resume a paused release.
    pub fn resume(&mut self) {
        if self.state.status == CanaryStatus::Paused {
            self.state.status = CanaryStatus::InProgress;
            self.state.updated_at = Some(Instant::now());
        }
    }

    /// Record metrics analysis result.
    pub fn record_analysis(&mut self, analysis: CanaryAnalysis) -> bool {
        self.state.last_analysis = Some(analysis.clone());
        self.state.updated_at = Some(Instant::now());
        self.state.step_count += 1;

        // Check if we should rollback
        if !analysis.is_error_rate_acceptable(self.config.max_error_rate_delta) {
            if self.config.auto_rollback {
                self.rollback("Error rate threshold exceeded");
                return false;
            }
        }

        if !analysis.is_latency_acceptable(self.config.max_latency_increase_ratio) {
            if self.config.auto_rollback {
                self.rollback("Latency threshold exceeded");
                return false;
            }
        }

        if analysis.health_score() < self.config.min_health_score {
            if self.config.auto_rollback {
                self.rollback("Health score below threshold");
                return false;
            }
        }

        true
    }

    /// Proceed to next weight step if analysis passed.
    pub fn proceed(&mut self, analysis: &CanaryAnalysis) -> bool {
        if self.state.status != CanaryStatus::InProgress {
            return false;
        }

        if !self.record_analysis(analysis.clone()) {
            return false;
        }

        // Increase weight
        let new_weight =
            (self.state.current_weight + self.config.weight_increment).min(self.config.max_weight);
        self.state.current_weight = new_weight;

        // Check if fully promoted
        if self.state.current_weight >= 100.0 {
            self.state.status = CanaryStatus::Promoted;
            return true;
        }

        true
    }

    /// Rollback to stable version.
    pub fn rollback(&mut self, reason: &str) {
        self.state.status = CanaryStatus::RolledBack;
        self.state.rollback_reason = Some(reason.to_string());
        self.state.updated_at = Some(Instant::now());
    }

    /// Promote canary to 100% traffic.
    pub fn promote(&mut self) {
        self.state.current_weight = 100.0;
        self.state.status = CanaryStatus::Promoted;
        self.state.updated_at = Some(Instant::now());
    }

    /// Abort and cleanup.
    pub fn abort(&mut self) {
        self.state.status = CanaryStatus::Failed;
        self.state.updated_at = Some(Instant::now());
    }

    /// Check if ready for next step.
    pub fn can_proceed(&self) -> bool {
        self.state.status == CanaryStatus::InProgress
            && self.state.current_weight < self.config.max_weight
    }

    /// Get time since last update.
    pub fn time_since_update(&self) -> Option<Duration> {
        self.state.updated_at.map(|t| t.elapsed())
    }

    /// Get time since start.
    pub fn time_since_start(&self) -> Option<Duration> {
        self.state.started_at.map(|t| t.elapsed())
    }

    /// Get remaining steps before full rollout.
    pub fn remaining_steps(&self) -> u32 {
        if self.state.current_weight >= self.config.max_weight {
            return 0;
        }
        let remaining = self.config.max_weight - self.state.current_weight;
        (remaining / self.config.weight_increment).ceil() as u32
    }
}

/// Shared canary release for async environments.
pub type SharedCanaryRelease = Arc<StdRwLock<CanaryRelease>>;

/// Create a shared canary release.
pub fn shared(release: CanaryRelease) -> SharedCanaryRelease {
    Arc::new(StdRwLock::new(release))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> CanaryConfig {
        CanaryConfig::default()
            .with_initial_weight(1.0)
            .with_max_weight(50.0)
            .with_rollout_interval_secs(60)
    }

    #[test]
    fn test_canary_initial_state() {
        let release = CanaryRelease::new("agent-v2".to_string(), make_config());
        assert_eq!(release.status(), CanaryStatus::Pending);
        assert_eq!(release.canary_weight(), 1.0);
        assert_eq!(release.stable_weight(), 99.0);
    }

    #[test]
    fn test_canary_start() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        assert_eq!(release.status(), CanaryStatus::InProgress);
        assert!(release.time_since_start().is_some());
    }

    #[test]
    fn test_canary_pause_and_resume() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        release.pause();
        assert_eq!(release.status(), CanaryStatus::Paused);
        release.resume();
        assert_eq!(release.status(), CanaryStatus::InProgress);
    }

    #[test]
    fn test_canary_promote() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        release.promote();
        assert_eq!(release.status(), CanaryStatus::Promoted);
        assert_eq!(release.canary_weight(), 100.0);
        assert_eq!(release.stable_weight(), 0.0);
    }

    #[test]
    fn test_canary_rollback() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        release.rollback("Manual rollback");
        assert_eq!(release.status(), CanaryStatus::RolledBack);
        assert_eq!(
            release.state.rollback_reason,
            Some("Manual rollback".to_string())
        );
    }

    #[test]
    fn test_canary_weight_map() {
        let release = CanaryRelease::new("agent-v2".to_string(), make_config());
        let map = release.weight_map();
        assert_eq!(map.get("stable"), Some(&99.0));
        assert_eq!(map.get("canary"), Some(&1.0));
    }

    #[test]
    fn test_canary_record_analysis_ok() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        let analysis = CanaryAnalysis {
            stable_error_rate: 0.5,
            canary_error_rate: 0.6,
            stable_latency_p50_ms: 100.0,
            canary_latency_p50_ms: 105.0,
            stable_latency_p99_ms: 500.0,
            canary_latency_p99_ms: 510.0,
            passed: true,
            warnings: vec![],
        };
        let ok = release.record_analysis(analysis);
        assert!(ok);
        assert!(release.state.last_analysis.is_some());
    }

    #[test]
    fn test_canary_record_analysis_error_rate_fail() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        let analysis = CanaryAnalysis {
            stable_error_rate: 0.5,
            canary_error_rate: 5.0, // Much higher than 1% delta
            stable_latency_p50_ms: 100.0,
            canary_latency_p50_ms: 100.0,
            stable_latency_p99_ms: 500.0,
            canary_latency_p99_ms: 500.0,
            passed: false,
            warnings: vec![],
        };
        let ok = release.record_analysis(analysis);
        assert!(!ok);
        assert_eq!(release.status(), CanaryStatus::RolledBack);
    }

    #[test]
    fn test_canary_proceed() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        let analysis = CanaryAnalysis::default();
        let ok = release.proceed(&analysis);
        assert!(ok);
        assert_eq!(release.canary_weight(), 11.0); // 1 + 10 increment
        assert_eq!(release.status(), CanaryStatus::InProgress);
    }

    #[test]
    fn test_canary_proceed_to_promotion() {
        let mut release = CanaryRelease::new(
            "agent-v2".to_string(),
            CanaryConfig {
                initial_weight: 90.0,
                max_weight: 100.0,
                weight_increment: 20.0,
                ..make_config()
            },
        );
        release.start();
        let analysis = CanaryAnalysis::default();
        let ok = release.proceed(&analysis);
        assert!(ok);
        assert_eq!(release.status(), CanaryStatus::Promoted);
    }

    #[test]
    fn test_canary_cannot_proceed_when_paused() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        release.pause();
        assert!(!release.can_proceed());
    }

    #[test]
    fn test_canary_remaining_steps() {
        let release = CanaryRelease::new("agent-v2".to_string(), make_config());
        // 1% initial, max 50%, increment 10% -> 5 steps
        assert_eq!(release.remaining_steps(), 5);
    }

    #[test]
    fn test_canary_analysis_error_rate_check() {
        let analysis = CanaryAnalysis {
            stable_error_rate: 1.0,
            canary_error_rate: 1.5,
            ..Default::default()
        };
        assert!(analysis.is_error_rate_acceptable(1.0)); // delta 0.5 <= 1.0
        assert!(!analysis.is_error_rate_acceptable(0.3)); // delta 0.5 > 0.3
    }

    #[test]
    fn test_canary_analysis_latency_check() {
        let analysis = CanaryAnalysis {
            stable_latency_p99_ms: 100.0,
            canary_latency_p99_ms: 140.0,
            ..Default::default()
        };
        assert!(analysis.is_latency_acceptable(1.5)); // 140 <= 150
        assert!(!analysis.is_latency_acceptable(1.2)); // 140 > 120
    }

    #[test]
    fn test_canary_analysis_health_score() {
        // Good: 0% error rate, equal latency (100.0 ratio)
        let good = CanaryAnalysis {
            stable_error_rate: 0.0,
            canary_error_rate: 0.0,
            stable_latency_p99_ms: 100.0,
            canary_latency_p99_ms: 100.0,
            ..Default::default()
        };
        assert!(good.health_score() > 90.0); // 100.0 at 0% error

        // Acceptable: 0.1% error, equal latency (~75)
        let acceptable = CanaryAnalysis {
            stable_error_rate: 0.0,
            canary_error_rate: 0.1,
            stable_latency_p99_ms: 100.0,
            canary_latency_p99_ms: 100.0,
            ..Default::default()
        };
        assert!(acceptable.health_score() > 70.0);

        // Bad: high error + 5x latency degradation
        let bad = CanaryAnalysis {
            stable_error_rate: 0.0,
            canary_error_rate: 10.0,
            stable_latency_p99_ms: 100.0,
            canary_latency_p99_ms: 500.0,
            ..Default::default()
        };
        assert!(bad.health_score() < 30.0);
    }

    #[test]
    fn test_canary_config_builder() {
        let config = CanaryConfig::new("v1", "v2")
            .with_initial_weight(5.0)
            .with_max_weight(25.0)
            .with_rollout_interval_secs(120);

        assert_eq!(config.stable_version, "v1");
        assert_eq!(config.canary_version, "v2");
        assert_eq!(config.initial_weight, 5.0);
        assert_eq!(config.max_weight, 25.0);
        assert_eq!(config.rollout_interval_secs, 120);
    }

    #[test]
    fn test_canary_status_display() {
        assert_eq!(format!("{}", CanaryStatus::InProgress), "InProgress");
        assert_eq!(format!("{}", CanaryStatus::RolledBack), "RolledBack");
    }

    #[test]
    fn test_shared_canary() {
        let release = shared(CanaryRelease::new("agent-v2".to_string(), make_config()));
        assert_eq!(release.read().unwrap().canary_weight(), 1.0);
    }

    #[test]
    fn test_canary_abort() {
        let mut release = CanaryRelease::new("agent-v2".to_string(), make_config());
        release.start();
        release.abort();
        assert_eq!(release.status(), CanaryStatus::Failed);
    }

    #[test]
    fn test_canary_time_since_update() {
        let release = CanaryRelease::new("agent-v2".to_string(), make_config());
        std::thread::sleep(Duration::from_millis(1));
        let elapsed = release.time_since_update();
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap().as_millis() >= 1);
    }
}
