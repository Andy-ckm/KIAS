//! Safety Net — autonomy level dynamic升降级 with error-budget and risk scoring.
//!
//! ## Autonomy Gradient
//! | Level | Name          | Behaviour                              |
//! |-------|---------------|----------------------------------------|
//! | 0     | Suggest       | Proposes actions; never auto-executes  |
//! | 1     | AutoEdit      | Auto-executes low-risk; proposes high  |
//! | 2     | FullAuto      | Auto-executes all, reports after       |
//!
//! ## Safety Net Rules
//! 1. **Error-rate triggers降级** — if error rate > `error_threshold` in a window → demote one level.
//! 2. **Risk-score gating** — actions above current risk ceiling are blocked.
//! 3. **Trust recovery** — after `recovery_window` with no errors, attempt one-level upgrade.
//! 4. **Manual override** — operators can lock to a specific level.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Autonomy level enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AutonomyLevel {
    Suggest = 0,
    AutoEdit = 1,
    FullAuto = 2,
}

impl Default for AutonomyLevel {
    fn default() -> Self {
        AutonomyLevel::Suggest
    }
}

impl std::fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutonomyLevel::Suggest => write!(f, "Suggest"),
            AutonomyLevel::AutoEdit => write!(f, "AutoEdit"),
            AutonomyLevel::FullAuto => write!(f, "FullAuto"),
        }
    }
}

/// A scored candidate action presented for approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredAction {
    pub action_id: String,
    pub description: String,
    pub risk_score: f64,     // 0.0 (safe) → 1.0 (dangerous)
    pub auto_allowed: bool,
}

impl ScoredAction {
    pub fn new(action_id: &str, description: &str, risk_score: f64) -> Self {
        Self {
            action_id: action_id.to_string(),
            description: description.to_string(),
            risk_score,
            auto_allowed: risk_score <= 0.5,
        }
    }
}

/// Outcome of an autonomy gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyGateResult {
    pub allowed: bool,
    pub requires_approval: bool,
    pub current_level: AutonomyLevel,
    pub blocked_reason: Option<String>,
    pub suggestions: Vec<String>,
}

/// Configuration for the safety net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyNetConfig {
    /// Error rate (errors / total) above which we demote.
    pub error_threshold: f64,
    /// Minimum number of actions in a window before checking rate.
    pub min_window_size: usize,
    /// Duration of the sliding window.
    pub window_secs: i64,
    /// How long without errors before attempting upgrade.
    pub recovery_secs: i64,
    /// Maximum risk score allowed at Suggest level.
    pub suggest_max_risk: f64,
    /// Maximum risk score allowed at AutoEdit level.
    pub autoedit_max_risk: f64,
    /// FullAuto allows any risk score ≤ this cap.
    pub fullauto_max_risk: f64,
}

impl Default for SafetyNetConfig {
    fn default() -> Self {
        Self {
            error_threshold: 0.2,
            min_window_size: 10,
            window_secs: 300,
            recovery_secs: 600,
            suggest_max_risk: 0.1,
            autoedit_max_risk: 0.5,
            fullauto_max_risk: 0.95,
        }
    }
}

/// Internal record of a single action execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActionRecord {
    timestamp: DateTime<Utc>,
    success: bool,
    risk_score: f64,
}

/// The Safety Net itself — wraps an autonomy level and enforces升降级.
#[derive(Debug, Clone)]
pub struct SafetyNet {
    config: SafetyNetConfig,
    current_level: AutonomyLevel,
    locked: bool,
    history: Vec<ActionRecord>,
    last_error_at: Option<DateTime<Utc>>,
    last_upgrade_attempt_at: Option<DateTime<Utc>>,
}

impl SafetyNet {
    pub fn new(config: SafetyNetConfig) -> Self {
        Self {
            config,
            current_level: AutonomyLevel::Suggest,
            locked: false,
            history: Vec::new(),
            last_error_at: None,
            last_upgrade_attempt_at: None,
        }
    }

    /// Return the current autonomy level.
    pub fn current_level(&self) -> AutonomyLevel {
        self.current_level
    }

    /// Manually set (lock) the autonomy level.
    pub fn lock(&mut self, level: AutonomyLevel) {
        self.locked = true;
        self.current_level = level;
    }

    /// Unlock and resume automatic management.
    pub fn unlock(&mut self) {
        self.locked = false;
    }

    /// Check whether an action is permitted at the current level.
    pub fn check_action(&self, action: &ScoredAction) -> AutonomyGateResult {
        let max_risk = match self.current_level {
            AutonomyLevel::Suggest => self.config.suggest_max_risk,
            AutonomyLevel::AutoEdit => self.config.autoedit_max_risk,
            AutonomyLevel::FullAuto => self.config.fullauto_max_risk,
        };

        if action.risk_score > max_risk {
            let blocked_reason = format!(
                "risk_score {:.2} exceeds {:.2} for {}",
                action.risk_score, max_risk, self.current_level
            );
            return AutonomyGateResult {
                allowed: false,
                requires_approval: true,
                current_level: self.current_level,
                blocked_reason: Some(blocked_reason),
                suggestions: vec![format!(
                    "upgrade to {:?} to auto-execute this action",
                    next_level(self.current_level)
                )],
            };
        }

        AutonomyGateResult {
            allowed: true,
            requires_approval: false,
            current_level: self.current_level,
            blocked_reason: None,
            suggestions: Vec::new(),
        }
    }

    /// Record the outcome of an action execution.
    pub fn record_outcome(&mut self, risk_score: f64, success: bool) {
        if !success {
            self.last_error_at = Some(Utc::now());
        }
        self.history.push(ActionRecord {
            timestamp: Utc::now(),
            success,
            risk_score,
        });
        self.trim_history();

        if !self.locked {
            self.try_auto_adjust();
        }
    }

    fn trim_history(&mut self) {
        let cutoff = Utc::now() - Duration::seconds(self.config.window_secs);
        self.history.retain(|r| r.timestamp > cutoff);
    }

    fn error_rate(&self) -> f64 {
        if self.history.len() < self.config.min_window_size {
            return 0.0;
        }
        let total = self.history.len() as f64;
        let errors = self.history.iter().filter(|r| !r.success).count() as f64;
        errors / total
    }

    fn try_auto_adjust(&mut self) {
        // ── Demotion: error rate too high ────────────────────────────────
        if self.error_rate() > self.config.error_threshold && self.current_level != AutonomyLevel::Suggest
        {
            self.demote();
            return;
        }

        // ── Recovery: window has no errors, try upgrade ───────────────────
        let window_clean = self
            .history
            .iter()
            .all(|r| r.success);

        if window_clean && self.current_level != AutonomyLevel::FullAuto {
            let recovery_window = Duration::seconds(self.config.recovery_secs);
            let time_since_last_error = self
                .last_error_at
                .map(|t| Utc::now() - t)
                .unwrap_or(recovery_window * 2);

            let time_since_last_upgrade = self
                .last_upgrade_attempt_at
                .map(|t| Utc::now() - t)
                .unwrap_or(recovery_window * 2);

            if time_since_last_error >= recovery_window && time_since_last_upgrade >= recovery_window {
                self.upgrade();
            }
        }
    }

    fn demote(&mut self) {
        if let Some(next) = prev_level(self.current_level) {
            self.current_level = next;
        }
    }

    fn upgrade(&mut self) {
        if let Some(next) = next_level(self.current_level) {
            self.current_level = next;
            self.last_upgrade_attempt_at = Some(Utc::now());
            // Reset history on successful upgrade to give fresh slate
            self.history.clear();
        }
    }

    /// Summary snapshot for observability.
    pub fn summary(&self) -> SafetyNetSummary {
        SafetyNetSummary {
            current_level: self.current_level,
            locked: self.locked,
            history_size: self.history.len(),
            error_rate: self.error_rate(),
            last_error_at: self.last_error_at,
            config: self.config.clone(),
        }
    }
}

fn next_level(l: AutonomyLevel) -> Option<AutonomyLevel> {
    match l {
        AutonomyLevel::Suggest => Some(AutonomyLevel::AutoEdit),
        AutonomyLevel::AutoEdit => Some(AutonomyLevel::FullAuto),
        AutonomyLevel::FullAuto => None,
    }
}

fn prev_level(l: AutonomyLevel) -> Option<AutonomyLevel> {
    match l {
        AutonomyLevel::Suggest => None,
        AutonomyLevel::AutoEdit => Some(AutonomyLevel::Suggest),
        AutonomyLevel::FullAuto => Some(AutonomyLevel::AutoEdit),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyNetSummary {
    pub current_level: AutonomyLevel,
    pub locked: bool,
    pub history_size: usize,
    pub error_rate: f64,
    pub last_error_at: Option<DateTime<Utc>>,
    pub config: SafetyNetConfig,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_net() -> SafetyNet {
        SafetyNet::new(SafetyNetConfig::default())
    }

    #[test]
    fn test_suggest_blocks_high_risk() {
        let net = default_net(); // starts at Suggest
        let action = ScoredAction::new("a1", "delete all data", 0.8);
        let result = net.check_action(&action);
        assert!(!result.allowed);
        assert!(result.requires_approval);
        assert!(result.blocked_reason.is_some());
    }

    #[test]
    fn test_suggest_allows_low_risk() {
        let net = default_net();
        let action = ScoredAction::new("a1", "read log", 0.05);
        let result = net.check_action(&action);
        assert!(result.allowed);
    }

    #[test]
    fn test_fullauto_allows_high_risk() {
        let mut net = default_net();
        net.lock(AutonomyLevel::FullAuto);
        let action = ScoredAction::new("a1", "deploy to prod", 0.9);
        let result = net.check_action(&action);
        assert!(result.allowed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_demote_on_high_error_rate() {
        let mut net = default_net();
        net.lock(AutonomyLevel::AutoEdit);
        // Record 10 failures in the window to push error rate above 0.2
        for _ in 0..10 {
            net.record_outcome(0.1, false);
        }
        net.unlock();
        // Add a few successes to trigger adjustment
        for _ in 0..5 {
            net.record_outcome(0.1, true);
        }
        // Force adjustment check
        net.record_outcome(0.1, false);
        assert_eq!(net.current_level(), AutonomyLevel::Suggest);
    }

    #[test]
    fn test_manual_lock_prevents_auto_adjust() {
        let mut net = default_net();
        net.lock(AutonomyLevel::FullAuto);
        // Even with many errors, locked level should not change
        for _ in 0..20 {
            net.record_outcome(0.5, false);
        }
        assert_eq!(net.current_level(), AutonomyLevel::FullAuto);
    }

    #[test]
    fn test_recovery_upgrades_after_clean_window() {
        let mut net = default_net();
        // Start at Suggest
        assert_eq!(net.current_level(), AutonomyLevel::Suggest);

        // Simulate a long clean history (needs to exceed min_window_size)
        // SafetyNet uses config.window_secs = 300, so we need recent entries
        for _ in 0..15 {
            net.record_outcome(0.05, true);
        }
        // After clean window + recovery_secs, try to upgrade
        net.unlock();
        // Force a record that triggers try_auto_adjust
        net.record_outcome(0.05, true);
        // Should have upgraded to AutoEdit
        assert_eq!(net.current_level(), AutonomyLevel::AutoEdit);
    }

    #[test]
    fn test_summary_contains_expected_fields() {
        let net = default_net();
        let summary = net.summary();
        assert_eq!(summary.current_level, AutonomyLevel::Suggest);
        assert!(!summary.locked);
        assert_eq!(summary.history_size, 0);
        assert!((summary.error_rate - 0.0).abs() < f64::EPSILON);
    }
}
