//! Descheduler configuration.

use serde::Deserialize;

/// Top-level descheduler configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DeschedulerConfig {
    /// Which strategies to enable.
    #[serde(default = "default_strategies")]
    pub strategies: Vec<String>,

    /// If true, generate eviction plans but do not execute them.
    #[serde(default)]
    pub dry_run: bool,

    /// Maximum agents evicted per cycle (safety cap).
    #[serde(default = "default_max_evictions")]
    pub max_evictions_per_cycle: usize,

    /// Node utilization thresholds.
    #[serde(default)]
    pub thresholds: UtilizationThresholds,
}

/// Thresholds for the LowNodeUtilization strategy.
#[derive(Debug, Clone, Deserialize)]
pub struct UtilizationThresholds {
    /// CPU utilization above which a node is "overloaded" (0.0–1.0).
    #[serde(default = "default_high_cpu")]
    pub high_cpu: f64,

    /// Memory utilization above which a node is "overloaded" (0.0–1.0).
    #[serde(default = "default_high_memory")]
    pub high_memory: f64,

    /// CPU utilization below which a node is "underutilized" (0.0–1.0).
    #[serde(default = "default_low_cpu")]
    pub low_cpu: f64,

    /// Memory utilization below which a node is "underutilized" (0.0–1.0).
    #[serde(default = "default_low_memory")]
    pub low_memory: f64,
}

// ── Defaults ────────────────────────────────────────────────────────

fn default_strategies() -> Vec<String> {
    vec![
        "low-node-utilization".to_string(),
        "remove-duplicates".to_string(),
        "remove-anti-affinity-violations".to_string(),
    ]
}

fn default_max_evictions() -> usize {
    10
}

fn default_high_cpu() -> f64 {
    0.80
}

fn default_high_memory() -> f64 {
    0.80
}

fn default_low_cpu() -> f64 {
    0.20
}

fn default_low_memory() -> f64 {
    0.20
}

impl Default for UtilizationThresholds {
    fn default() -> Self {
        Self {
            high_cpu: default_high_cpu(),
            high_memory: default_high_memory(),
            low_cpu: default_low_cpu(),
            low_memory: default_low_memory(),
        }
    }
}

impl Default for DeschedulerConfig {
    fn default() -> Self {
        Self {
            strategies: default_strategies(),
            dry_run: false,
            max_evictions_per_cycle: default_max_evictions(),
            thresholds: UtilizationThresholds::default(),
        }
    }
}
