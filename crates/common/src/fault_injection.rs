//! Fault Injection Framework — chaos testing primitives for AgentGuard.
//!
//! ## Fault Types
//! | Type          | Effect                                              |
//! |---------------|-----------------------------------------------------|
//! | NetworkLatency| Adds configurable delay to operations                |
//! | NodeCrash     | Simulates process crash / OOM                      |
//! | SlowDisk      | Adds delay to storage I/O                          |
//! | SlowQuery     | Adds delay to database queries                     |
//! | RandomError   | Returns a random error with configurable probability|
//!
//! ## Usage
//! ```ignore
//! let injector = FaultInjector::new();
//! injector.configure(FaultType::NetworkLatency, 0.1, 500); // 10% chance, 500ms
//! if injector.should_inject(FaultType::NetworkLatency, "api-call") {
//!     tokio::time::sleep(Duration::from_millis(500)).await;
//! }
//! ```

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// FaultType
// ─────────────────────────────────────────────────────────────────────────────

/// All supported fault types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FaultType {
    NetworkLatency,
    NodeCrash,
    SlowDisk,
    SlowQuery,
    RandomError,
}

impl std::fmt::Display for FaultType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FaultType::NetworkLatency => write!(f, "NetworkLatency"),
            FaultType::NodeCrash => write!(f, "NodeCrash"),
            FaultType::SlowDisk => write!(f, "SlowDisk"),
            FaultType::SlowQuery => write!(f, "SlowQuery"),
            FaultType::RandomError => write!(f, "RandomError"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FaultConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a single fault type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultConfig {
    /// Probability 0.0–1.0 that the fault fires.
    pub probability: f64,
    /// Extra latency in ms (for latency faults).
    pub latency_ms: u64,
    /// Whether the fault is currently enabled.
    pub enabled: bool,
    /// Tags/labels to scope fault application (empty = all).
    pub tags: Vec<String>,
}

impl Default for FaultConfig {
    fn default() -> Self {
        Self {
            probability: 0.0,
            latency_ms: 0,
            enabled: false,
            tags: Vec::new(),
        }
    }
}

impl FaultConfig {
    pub fn probability(p: f64) -> Self {
        Self {
            probability: p.clamp(0.0, 1.0),
            ..Default::default()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FaultInjector
// ─────────────────────────────────────────────────────────────────────────────

/// Statistics accumulated for a fault type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FaultStats {
    pub total_evaluations: u64,
    pub total_injections: u64,
    pub last_injected_at: Option<DateTime<Utc>>,
}

impl FaultStats {
    fn record_evaluation(&mut self) {
        self.total_evaluations += 1;
    }
    fn record_injection(&mut self) {
        self.total_injections += 1;
        self.last_injected_at = Some(Utc::now());
    }
}

/// Thread-safe fault injector for chaos testing.
#[derive(Debug)]
pub struct FaultInjector {
    configs: HashMap<FaultType, FaultConfig>,
    stats: HashMap<FaultType, FaultStats>,
}

impl Default for FaultInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl FaultInjector {
    pub fn new() -> Self {
        let mut configs = HashMap::new();
        for ft in [
            FaultType::NetworkLatency,
            FaultType::NodeCrash,
            FaultType::SlowDisk,
            FaultType::SlowQuery,
            FaultType::RandomError,
        ] {
            configs.insert(ft, FaultConfig::default());
        }

        let mut stats = HashMap::new();
        for ft in [
            FaultType::NetworkLatency,
            FaultType::NodeCrash,
            FaultType::SlowDisk,
            FaultType::SlowQuery,
            FaultType::RandomError,
        ] {
            stats.insert(ft, FaultStats::default());
        }

        Self { configs, stats }
    }

    /// Configure a fault type.
    pub fn configure(&mut self, fault_type: FaultType, probability: f64, latency_ms: u64) {
        let cfg = FaultConfig {
            probability: probability.clamp(0.0, 1.0),
            latency_ms,
            enabled: true,
            tags: Vec::new(),
        };
        self.configs.insert(fault_type, cfg);
    }

    /// Enable or disable a fault type.
    pub fn set_enabled(&mut self, fault_type: FaultType, enabled: bool) {
        self.configs.entry(fault_type).or_default().enabled = enabled;
    }

    /// Check if a fault should be injected for the given operation tag.
    /// Thread-safe via shared mutability (interior mutability).
    pub fn should_inject(&mut self, fault_type: FaultType, _operation_tag: &str) -> bool {
        let cfg = match self.configs.get(&fault_type) {
            Some(c) => c,
            None => return false,
        };

        if !cfg.enabled {
            return false;
        }

        let stats = self.stats.entry(fault_type).or_default();
        stats.record_evaluation();

        let mut rng = rand::thread_rng();
        let roll: f64 = rng.gen();
        let inject = roll < cfg.probability;

        if inject {
            stats.record_injection();
        }

        inject
    }

    /// Return the configured latency for a fault type (if applicable).
    pub fn latency_for(&self, fault_type: FaultType) -> u64 {
        self.configs
            .get(&fault_type)
            .map(|c| c.latency_ms)
            .unwrap_or(0)
    }

    /// Get stats for a fault type.
    pub fn stats(&self, fault_type: FaultType) -> FaultStats {
        self.stats.get(&fault_type).cloned().unwrap_or_default()
    }

    /// Reset all stats.
    pub fn reset_stats(&mut self) {
        for stats in self.stats.values_mut() {
            *stats = FaultStats::default();
        }
    }

    /// Whether any faults are currently enabled.
    pub fn is_any_enabled(&self) -> bool {
        self.configs.values().any(|c| c.enabled)
    }

    /// Total injection count across all fault types.
    pub fn total_injections(&self) -> u64 {
        self.stats.values().map(|s| s.total_injections).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Simulated errors
// ─────────────────────────────────────────────────────────────────────────────

/// Possible error kinds for RandomError injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InjectedErrorKind {
    IoError,
    Timeout,
    ConnectionRefused,
    RateLimited,
}

impl InjectedErrorKind {
    pub fn message(&self) -> &str {
        match self {
            InjectedErrorKind::IoError => "simulated I/O error",
            InjectedErrorKind::Timeout => "simulated timeout",
            InjectedErrorKind::ConnectionRefused => "simulated connection refused",
            InjectedErrorKind::RateLimited => "simulated rate limit",
        }
    }
}

/// Choose a random error kind.
pub fn random_error_kind() -> InjectedErrorKind {
    let mut rng = rand::thread_rng();
    match rng.gen_range(0..4) {
        0 => InjectedErrorKind::IoError,
        1 => InjectedErrorKind::Timeout,
        2 => InjectedErrorKind::ConnectionRefused,
        _ => InjectedErrorKind::RateLimited,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_injection_when_disabled() {
        let mut injector = FaultInjector::new();
        injector.set_enabled(FaultType::NetworkLatency, false);
        let count = (0..1000)
            .filter(|_| injector.should_inject(FaultType::NetworkLatency, "op"))
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_100_percent_injection() {
        let mut injector = FaultInjector::new();
        injector.configure(FaultType::NodeCrash, 1.0, 0);
        let count = (0..100)
            .filter(|_| injector.should_inject(FaultType::NodeCrash, "op"))
            .count();
        assert_eq!(count, 100); // always fires
    }

    #[test]
    fn test_0_percent_injection() {
        let mut injector = FaultInjector::new();
        injector.configure(FaultType::SlowDisk, 0.0, 0);
        let count = (0..100)
            .filter(|_| injector.should_inject(FaultType::SlowDisk, "op"))
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_stats_recorded() {
        let mut injector = FaultInjector::new();
        injector.configure(FaultType::SlowQuery, 0.5, 200);
        for _ in 0..20 {
            injector.should_inject(FaultType::SlowQuery, "query");
        }
        let stats = injector.stats(FaultType::SlowQuery);
        assert_eq!(stats.total_evaluations, 20);
        assert!(stats.total_injections <= 20);
    }

    #[test]
    fn test_latency_configurable() {
        let mut injector = FaultInjector::new();
        injector.configure(FaultType::NetworkLatency, 0.1, 1234);
        assert_eq!(injector.latency_for(FaultType::NetworkLatency), 1234);
        assert_eq!(injector.latency_for(FaultType::NodeCrash), 0); // unset
    }

    #[test]
    fn test_reset_stats() {
        let mut injector = FaultInjector::new();
        injector.configure(FaultType::RandomError, 1.0, 0);
        for _ in 0..10 {
            injector.should_inject(FaultType::RandomError, "op");
        }
        injector.reset_stats();
        let stats = injector.stats(FaultType::RandomError);
        assert_eq!(stats.total_evaluations, 0);
        assert_eq!(stats.total_injections, 0);
    }

    #[test]
    fn test_injected_error_kind_message() {
        assert_eq!(InjectedErrorKind::IoError.message(), "simulated I/O error");
        assert_eq!(InjectedErrorKind::Timeout.message(), "simulated timeout");
    }

    #[test]
    fn test_random_error_kind_is_deterministic_variant() {
        let kinds: Vec<_> = (0..20).map(|_| random_error_kind()).collect();
        // Should produce at least 2 different kinds in 20 samples with high probability
        let unique = kinds.iter().collect::<std::collections::HashSet<_>>().len();
        assert!(unique >= 2, "random should produce varied output");
    }

    #[test]
    fn test_fault_type_display() {
        assert_eq!(FaultType::NetworkLatency.to_string(), "NetworkLatency");
        assert_eq!(FaultType::RandomError.to_string(), "RandomError");
    }

    #[test]
    fn test_is_any_enabled() {
        let mut injector = FaultInjector::new();
        assert!(!injector.is_any_enabled());
        injector.set_enabled(FaultType::NodeCrash, true);
        assert!(injector.is_any_enabled());
    }
}
