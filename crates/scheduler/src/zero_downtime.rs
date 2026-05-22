//! Zero-Downtime Rolling Upgrade — orchestrating rolling updates with health gating.
//!
//! This module provides zero-downtime upgrade capabilities:
//! - Rolling update with configurable batch size
//! - Health check gating between batches
//! - Connection draining before termination
//! - Backoff on failed batches
//! - Pre/post upgrade hooks
//!
//! # Upgrade Lifecycle
//!
//! ```text
//! Upgrade Request
//!     ↓
//! PreUpgrade Hook
//!     ↓
//! For each batch:
//!     ┌──────────────────────────────┐
//!     │ 1. Drain connections (drain) │
//!     │ 2. Stop old version          │
//!     │ 3. Start new version         │
//!     │ 4. Wait health check         │
//!     │ 5. If healthy → next batch   │
//!     │   If unhealthy → rollback    │
//!     └──────────────────────────────┘
//!     ↓
//! PostUpgrade Hook
//! ```
//!
//! # Example
//!
//! ```
//! use kias_scheduler::zero_downtime::{RollingUpgrade, UpgradeConfig, UpgradePhase};
//!
//! let config = UpgradeConfig::default()
//!     .with_batch_size(2)
//!     .with_drain_timeout_secs(30)
//!     .with_health_check_interval_secs(5);
//!
//! let upgrade = RollingUpgrade::new("agent-pool".to_string(), config);
//! assert_eq!(upgrade.phase(), UpgradePhase::Pending);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

/// Upgrade phase enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradePhase {
    /// Upgrade not yet started.
    Pending,
    /// Pre-upgrade hooks executing.
    PreUpgrade,
    /// Upgrading in progress (batches).
    Upgrading,
    /// Waiting between batches.
    WaitingForHealth,
    /// Finalizing upgrade.
    Finalizing,
    /// Upgrade completed successfully.
    Completed,
    /// Upgrade paused.
    Paused,
    /// Upgrade rolled back.
    RolledBack,
    /// Upgrade failed.
    Failed,
}

impl fmt::Display for UpgradePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpgradePhase::Pending => write!(f, "Pending"),
            UpgradePhase::PreUpgrade => write!(f, "PreUpgrade"),
            UpgradePhase::Upgrading => write!(f, "Upgrading"),
            UpgradePhase::WaitingForHealth => write!(f, "WaitingForHealth"),
            UpgradePhase::Finalizing => write!(f, "Finalizing"),
            UpgradePhase::Completed => write!(f, "Completed"),
            UpgradePhase::Paused => write!(f, "Paused"),
            UpgradePhase::RolledBack => write!(f, "RolledBack"),
            UpgradePhase::Failed => write!(f, "Failed"),
        }
    }
}

/// Health check result for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    pub node_id: String,
    pub healthy: bool,
    pub latency_ms: u64,
    pub error_message: Option<String>,
}

impl HealthResult {
    pub fn healthy(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            healthy: true,
            latency_ms: 0,
            error_message: None,
        }
    }

    pub fn unhealthy(node_id: &str, error: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            healthy: false,
            latency_ms: 0,
            error_message: Some(error.to_string()),
        }
    }
}

/// A node participating in the upgrade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeNode {
    pub node_id: String,
    pub current_version: String,
    pub target_version: String,
    pub in_batch: bool,
    pub upgraded: bool,
    pub health_check_count: u32,
    pub consecutive_failures: u32,
    pub drain_started_at: Option<String>,
    pub upgraded_at: Option<String>,
}

impl UpgradeNode {
    pub fn new(node_id: &str, current: &str, target: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            current_version: current.to_string(),
            target_version: target.to_string(),
            in_batch: false,
            upgraded: false,
            health_check_count: 0,
            consecutive_failures: 0,
            drain_started_at: None,
            upgraded_at: None,
        }
    }
}

/// A single batch of nodes being upgraded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeBatch {
    pub batch_id: u32,
    pub node_ids: Vec<String>,
    pub phase: UpgradePhase,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub health_checks_passed: u32,
    pub health_checks_total: u32,
}

impl UpgradeBatch {
    pub fn new(batch_id: u32, node_ids: Vec<String>) -> Self {
        Self {
            batch_id,
            node_ids,
            phase: UpgradePhase::Pending,
            started_at: None,
            completed_at: None,
            health_checks_passed: 0,
            health_checks_total: 0,
        }
    }
}

/// Upgrade configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeConfig {
    /// Total number of nodes to upgrade.
    pub total_nodes: usize,
    /// Number of nodes per batch.
    pub batch_size: usize,
    /// Maximum number of batches to run in parallel (for parallel upgrades).
    pub max_parallel_batches: usize,
    /// Time to wait for connections to drain (seconds).
    pub drain_timeout_secs: u64,
    /// Time between health checks (seconds).
    pub health_check_interval_secs: u64,
    /// Number of successful health checks required.
    pub required_health_checks: u32,
    /// Maximum consecutive failures before rollback.
    pub max_consecutive_failures: u32,
    /// Enable automatic rollback on failure.
    pub auto_rollback: bool,
    /// Enable pre-upgrade hook.
    pub pre_upgrade_hook: Option<String>,
    /// Enable post-upgrade hook.
    pub post_upgrade_hook: Option<String>,
    /// Backoff multiplier on failure (exponential).
    pub backoff_multiplier: f64,
    /// Initial backoff base in seconds.
    pub backoff_base_secs: u64,
    /// Force upgrade even if health checks fail.
    pub force: bool,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

impl Default for UpgradeConfig {
    fn default() -> Self {
        Self {
            total_nodes: 0,
            batch_size: 1,
            max_parallel_batches: 1,
            drain_timeout_secs: 30,
            health_check_interval_secs: 5,
            required_health_checks: 3,
            max_consecutive_failures: 3,
            auto_rollback: true,
            pre_upgrade_hook: None,
            post_upgrade_hook: None,
            backoff_multiplier: 2.0,
            backoff_base_secs: 10,
            force: false,
            metadata: HashMap::new(),
        }
    }
}

impl UpgradeConfig {
    pub fn new(total_nodes: usize) -> Self {
        Self {
            total_nodes,
            ..Default::default()
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn with_drain_timeout_secs(mut self, secs: u64) -> Self {
        self.drain_timeout_secs = secs;
        self
    }

    pub fn with_health_check_interval_secs(mut self, secs: u64) -> Self {
        self.health_check_interval_secs = secs;
        self
    }

    /// Compute number of batches needed.
    pub fn num_batches(&self) -> usize {
        (self.total_nodes + self.batch_size - 1) / self.batch_size
    }

    /// Compute backoff duration for a given attempt.
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let secs = self.backoff_base_secs as f64 * self.backoff_multiplier.powi(attempt as i32);
        Duration::from_secs(secs.min(300.0) as u64) // Cap at 5 minutes
    }
}

/// Statistics from upgrade execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeStats {
    pub total_nodes: usize,
    pub upgraded_nodes: usize,
    pub failed_nodes: u32,
    pub total_batches: usize,
    pub completed_batches: usize,
    pub current_batch: u32,
    pub total_duration_ms: u64,
    pub health_check_duration_ms: u64,
    pub drain_duration_ms: u64,
    pub rollback_count: u32,
}

impl Default for UpgradeStats {
    fn default() -> Self {
        Self {
            total_nodes: 0,
            upgraded_nodes: 0,
            failed_nodes: 0,
            total_batches: 0,
            completed_batches: 0,
            current_batch: 0,
            total_duration_ms: 0,
            health_check_duration_ms: 0,
            drain_duration_ms: 0,
            rollback_count: 0,
        }
    }
}

/// Upgrade state.
#[derive(Debug, Clone)]
pub struct UpgradeState {
    pub phase: UpgradePhase,
    pub nodes: Vec<UpgradeNode>,
    pub batches: VecDeque<UpgradeBatch>,
    pub stats: UpgradeStats,
    pub current_batch_index: usize,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub failure_reason: Option<String>,
    pub rollback_version: Option<String>,
}

impl UpgradeState {
    pub fn new(config: &UpgradeConfig, nodes: Vec<UpgradeNode>) -> Self {
        let batches = Self::build_batches(config, &nodes);
        Self {
            phase: UpgradePhase::Pending,
            nodes,
            batches: VecDeque::from(batches),
            stats: UpgradeStats {
                total_nodes: config.total_nodes,
                total_batches: config.num_batches(),
                ..Default::default()
            },
            current_batch_index: 0,
            started_at: None,
            finished_at: None,
            failure_reason: None,
            rollback_version: None,
        }
    }

    fn build_batches(config: &UpgradeConfig, nodes: &[UpgradeNode]) -> Vec<UpgradeBatch> {
        let mut batches = Vec::new();
        let node_ids: Vec<String> = nodes.iter().map(|n| n.node_id.clone()).collect();
        for (i, chunk) in node_ids.chunks(config.batch_size).enumerate() {
            batches.push(UpgradeBatch::new(i as u32 + 1, chunk.to_vec()));
        }
        batches
    }

    pub fn current_batch(&self) -> Option<&UpgradeBatch> {
        self.batches.get(self.current_batch_index)
    }

    pub fn current_batch_mut(&mut self) -> Option<&mut UpgradeBatch> {
        self.batches.get_mut(self.current_batch_index)
    }
}

/// Rolling upgrade executor.
#[derive(Debug)]
pub struct RollingUpgrade {
    name: String,
    config: UpgradeConfig,
    state: UpgradeState,
}

impl RollingUpgrade {
    /// Create a new rolling upgrade.
    pub fn new(name: String, config: UpgradeConfig) -> Self {
        // Create nodes
        let nodes: Vec<UpgradeNode> = (0..config.total_nodes)
            .map(|i| UpgradeNode::new(&format!("node-{}", i), "v1", "v2"))
            .collect();

        let state = UpgradeState::new(&config, nodes);
        Self {
            name,
            config,
            state,
        }
    }

    /// Create with explicit nodes.
    pub fn with_nodes(name: String, config: UpgradeConfig, nodes: Vec<UpgradeNode>) -> Self {
        let state = UpgradeState::new(&config, nodes);
        Self {
            name,
            config,
            state,
        }
    }

    /// Get the upgrade name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current phase.
    pub fn phase(&self) -> UpgradePhase {
        self.state.phase
    }

    /// Get upgrade configuration.
    pub fn config(&self) -> &UpgradeConfig {
        &self.config
    }

    /// Get current stats.
    pub fn stats(&self) -> &UpgradeStats {
        &self.state.stats
    }

    /// Get upgrade statistics (mutable).
    pub fn stats_mut(&mut self) -> &mut UpgradeStats {
        &mut self.state.stats
    }

    /// Get all nodes.
    pub fn nodes(&self) -> &[UpgradeNode] {
        &self.state.nodes
    }

    /// Get all batches.
    pub fn batches(&self) -> &VecDeque<UpgradeBatch> {
        &self.state.batches
    }

    /// Start the upgrade.
    pub fn start(&mut self) -> bool {
        if self.state.phase == UpgradePhase::Pending {
            self.state.phase = UpgradePhase::PreUpgrade;
            self.state.started_at = Some(Instant::now());
            // Immediately transition to Upgrading for sync execution
            self.state.phase = UpgradePhase::Upgrading;
            return true;
        }
        false
    }

    /// Execute pre-upgrade hooks.
    pub fn run_pre_upgrade_hook(&mut self) -> bool {
        if self.config.pre_upgrade_hook.is_none() {
            return true;
        }
        // Simulate hook execution
        std::thread::sleep(Duration::from_millis(1));
        true
    }

    /// Execute post-upgrade hooks.
    pub fn run_post_upgrade_hook(&mut self) -> bool {
        if self.config.post_upgrade_hook.is_none() {
            return true;
        }
        // Simulate hook execution
        std::thread::sleep(Duration::from_millis(1));
        true
    }

    /// Begin draining connections for nodes in current batch.
    pub fn begin_drain(&mut self) -> bool {
        let node_ids: Vec<String> = {
            if let Some(batch) = self.state.current_batch_mut() {
                let ids = batch.node_ids.clone();
                batch.phase = UpgradePhase::Upgrading;
                batch.started_at = Some(chrono_lite_now());
                ids
            } else {
                return false;
            }
        };

        for node_id in &node_ids {
            if let Some(node) = self.state.nodes.iter_mut().find(|n| n.node_id == *node_id) {
                node.in_batch = true;
                node.drain_started_at = Some(chrono_lite_now());
            }
        }
        true
    }

    /// Complete upgrade for a node.
    pub fn complete_node_upgrade(&mut self, node_id: &str) {
        if let Some(node) = self.state.nodes.iter_mut().find(|n| n.node_id == node_id) {
            node.upgraded = true;
            node.in_batch = false;
            node.upgraded_at = Some(chrono_lite_now());
            self.state.stats.upgraded_nodes += 1;
        }
    }

    /// Record health check result for a node.
    pub fn record_health_check(&mut self, node_id: &str, result: &HealthResult) {
        if let Some(batch) = self.state.current_batch_mut() {
            batch.health_checks_total += 1;
            if result.healthy {
                batch.health_checks_passed += 1;
            }
        }

        if let Some(node) = self.state.nodes.iter_mut().find(|n| n.node_id == node_id) {
            node.health_check_count += 1;
            if result.healthy {
                node.consecutive_failures = 0;
            } else {
                node.consecutive_failures += 1;
            }
        }
    }

    /// Check if current batch is healthy enough to proceed.
    pub fn is_batch_healthy(&self) -> bool {
        if let Some(batch) = self.state.current_batch() {
            batch.health_checks_passed >= self.config.required_health_checks
        } else {
            false
        }
    }

    /// Check if all nodes in current batch have passed health checks.
    pub fn all_batch_nodes_healthy(&self) -> bool {
        if let Some(batch) = self.state.current_batch() {
            batch.node_ids.iter().all(|nid| {
                self.state
                    .nodes
                    .iter()
                    .any(|n| n.node_id == *nid && n.consecutive_failures == 0)
            })
        } else {
            false
        }
    }

    /// Complete the current batch.
    pub fn complete_batch(&mut self) {
        // Extract fields we need first, before mutating stats
        let (batch_id, node_ids): (u32, Vec<String>) = {
            if let Some(batch) = self.state.current_batch_mut() {
                batch.completed_at = Some(chrono_lite_now());
                batch.phase = UpgradePhase::Completed;
                let id = batch.batch_id;
                let nids = batch.node_ids.clone();
                self.state.stats.completed_batches += 1;
                self.state.stats.current_batch = id;
                (id, nids)
            } else {
                return;
            }
        };

        // Mark all nodes in this batch as completed
        for node_id in &node_ids {
            self.complete_node_upgrade(node_id);
        }

        // Move to next batch
        self.state.current_batch_index += 1;

        if self.state.current_batch_index >= self.state.batches.len() {
            self.state.phase = UpgradePhase::Finalizing;
        } else {
            self.state.phase = UpgradePhase::Upgrading;
        }
    }

    /// Transition to waiting for health.
    pub fn enter_health_wait(&mut self) {
        self.state.phase = UpgradePhase::WaitingForHealth;
    }

    /// Transition to finalization.
    pub fn finalize(&mut self) {
        self.state.phase = UpgradePhase::Completed;
        self.state.finished_at = Some(Instant::now());
        self.compute_stats();
    }

    /// Rollback the upgrade.
    pub fn rollback(&mut self, reason: &str) {
        self.state.phase = UpgradePhase::RolledBack;
        self.state.failure_reason = Some(reason.to_string());
        self.state.rollback_version = Some("v1".to_string());
        self.state.stats.rollback_count += 1;
        self.state.finished_at = Some(Instant::now());
    }

    /// Mark upgrade as failed.
    pub fn fail(&mut self, reason: &str) {
        self.state.phase = UpgradePhase::Failed;
        self.state.failure_reason = Some(reason.to_string());
        self.state.finished_at = Some(Instant::now());
    }

    /// Check if upgrade is complete.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.state.phase,
            UpgradePhase::Completed | UpgradePhase::RolledBack | UpgradePhase::Failed
        )
    }

    /// Get upgrade progress percentage (0-100).
    pub fn progress_percent(&self) -> f64 {
        if self.state.stats.total_nodes == 0 {
            return 0.0;
        }
        (self.state.stats.upgraded_nodes as f64 / self.state.stats.total_nodes as f64) * 100.0
    }

    /// Get backoff duration for current failure attempt.
    pub fn current_backoff(&self) -> Duration {
        let attempt = self
            .state
            .stats
            .failed_nodes
            .min(self.config.max_consecutive_failures);
        self.config.backoff_for_attempt(attempt)
    }

    fn compute_stats(&mut self) {
        if let Some(started) = self.state.started_at {
            if let Some(finished) = self.state.finished_at {
                self.state.stats.total_duration_ms =
                    finished.duration_since(started).as_millis() as u64;
            }
        }
    }

    /// Get failure reason if any.
    pub fn failure_reason(&self) -> Option<&str> {
        self.state.failure_reason.as_deref()
    }

    /// Check if we should auto-rollback.
    pub fn should_auto_rollback(&self) -> bool {
        self.config.auto_rollback
            && self.state.stats.failed_nodes >= self.config.max_consecutive_failures
    }
}

/// Lightweight timestamp helper.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> UpgradeConfig {
        UpgradeConfig::new(4).with_batch_size(2)
    }

    #[test]
    fn test_upgrade_initial_state() {
        let upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        assert_eq!(upgrade.phase(), UpgradePhase::Pending);
        assert_eq!(upgrade.stats().total_nodes, 4);
        assert_eq!(upgrade.stats().total_batches, 2);
    }

    #[test]
    fn test_upgrade_start() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        assert!(upgrade.start());
        assert_eq!(upgrade.phase(), UpgradePhase::Upgrading);
    }

    #[test]
    fn test_upgrade_batches() {
        let upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        let batches = upgrade.batches();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].node_ids, vec!["node-0", "node-1"]);
        assert_eq!(batches[1].node_ids, vec!["node-2", "node-3"]);
    }

    #[test]
    fn test_upgrade_drain() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        assert!(upgrade.begin_drain());
        let batch = upgrade.state.current_batch();
        assert!(batch.is_some());
        assert_eq!(batch.unwrap().phase, UpgradePhase::Upgrading);
    }

    #[test]
    fn test_upgrade_health_check_recording() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.begin_drain();

        upgrade.record_health_check("node-0", &HealthResult::healthy("node-0"));
        upgrade.record_health_check("node-1", &HealthResult::unhealthy("node-1", "timeout"));

        let batch = upgrade.state.current_batch();
        assert!(batch.is_some());
        assert_eq!(batch.unwrap().health_checks_total, 2);
        assert_eq!(batch.unwrap().health_checks_passed, 1);
    }

    #[test]
    fn test_upgrade_complete_node() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.begin_drain();
        upgrade.complete_node_upgrade("node-0");

        let node = upgrade
            .nodes()
            .iter()
            .find(|n| n.node_id == "node-0")
            .unwrap();
        assert!(node.upgraded);
        assert_eq!(upgrade.stats().upgraded_nodes, 1);
    }

    #[test]
    fn test_upgrade_complete_batch() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.begin_drain();
        upgrade.complete_batch();

        assert_eq!(upgrade.stats().completed_batches, 1);
        assert_eq!(upgrade.stats().upgraded_nodes, 2);
    }

    #[test]
    fn test_upgrade_progress() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.begin_drain();
        upgrade.complete_batch();
        // Now 50% complete (2/4 nodes)
        assert!((upgrade.progress_percent() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_upgrade_finalize() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();

        // Complete all batches
        while !upgrade.is_complete() {
            upgrade.begin_drain();
            upgrade.complete_batch();
        }

        upgrade.finalize();
        assert_eq!(upgrade.phase(), UpgradePhase::Completed);
        assert!(upgrade.state.finished_at.is_some());
    }

    #[test]
    fn test_upgrade_rollback() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.rollback("Test rollback");
        assert_eq!(upgrade.phase(), UpgradePhase::RolledBack);
        assert_eq!(upgrade.failure_reason(), Some("Test rollback"));
        assert_eq!(upgrade.stats().rollback_count, 1);
    }

    #[test]
    fn test_upgrade_fail() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.fail("Critical error");
        assert_eq!(upgrade.phase(), UpgradePhase::Failed);
    }

    #[test]
    fn test_upgrade_config_num_batches() {
        let config = UpgradeConfig::new(5).with_batch_size(2);
        assert_eq!(config.num_batches(), 3); // ceil(5/2) = 3
    }

    #[test]
    fn test_upgrade_config_backoff() {
        let config = UpgradeConfig::default();
        assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(10));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(20));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(40));
    }

    #[test]
    fn test_upgrade_config_backoff_capped() {
        let config = UpgradeConfig::default();
        // Very high attempt should still cap at 5 minutes
        let backoff = config.backoff_for_attempt(10);
        assert!(backoff <= Duration::from_secs(300));
    }

    #[test]
    fn test_health_result_helpers() {
        let healthy = HealthResult::healthy("node-1");
        assert!(healthy.healthy);
        assert_eq!(healthy.error_message, None);

        let unhealthy = HealthResult::unhealthy("node-2", "connection refused");
        assert!(!unhealthy.healthy);
        assert_eq!(
            unhealthy.error_message,
            Some("connection refused".to_string())
        );
    }

    #[test]
    fn test_upgrade_node_state() {
        let node = UpgradeNode::new("n1", "v1", "v2");
        assert_eq!(node.current_version, "v1");
        assert_eq!(node.target_version, "v2");
        assert!(!node.upgraded);
        assert!(!node.in_batch);
    }

    #[test]
    fn test_batch_health_checks() {
        let batch = UpgradeBatch::new(1, vec!["n1".to_string(), "n2".to_string()]);
        assert_eq!(batch.health_checks_passed, 0);
        assert_eq!(batch.health_checks_total, 0);
    }

    #[test]
    fn test_all_batch_nodes_healthy() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        upgrade.begin_drain();

        // Both nodes healthy
        upgrade.record_health_check("node-0", &HealthResult::healthy("node-0"));
        upgrade.record_health_check("node-1", &HealthResult::healthy("node-1"));

        assert!(upgrade.all_batch_nodes_healthy());
    }

    #[test]
    fn test_upgrade_is_complete() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        upgrade.start();
        assert!(!upgrade.is_complete());

        while !upgrade.is_complete() {
            upgrade.begin_drain();
            upgrade.complete_batch();
        }
        upgrade.finalize();
        assert!(upgrade.is_complete());
    }

    #[test]
    fn test_upgrade_phase_display() {
        assert_eq!(format!("{}", UpgradePhase::Upgrading), "Upgrading");
        assert_eq!(format!("{}", UpgradePhase::RolledBack), "RolledBack");
    }

    #[test]
    fn test_upgrade_with_nodes() {
        let config = UpgradeConfig::new(3);
        let nodes = vec![
            UpgradeNode::new("a", "v1", "v2"),
            UpgradeNode::new("b", "v1", "v2"),
            UpgradeNode::new("c", "v1", "v2"),
        ];
        let upgrade = RollingUpgrade::with_nodes("pool-x".to_string(), config, nodes);
        assert_eq!(upgrade.stats().total_nodes, 3);
        assert_eq!(upgrade.batches().len(), 3); // batch_size=1 default
    }

    #[test]
    fn test_shared_upgrade() {
        let config = UpgradeConfig::new(2);
        let upgrade = RollingUpgrade::new("pool-1".to_string(), config);
        let shared = Arc::new(StdRwLock::new(upgrade));
        assert_eq!(shared.read().unwrap().phase(), UpgradePhase::Pending);
    }

    #[test]
    fn test_should_auto_rollback() {
        let mut upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        assert!(!upgrade.should_auto_rollback()); // 0 failed < 3
    }

    #[test]
    fn test_current_backoff() {
        let upgrade = RollingUpgrade::new("pool-1".to_string(), make_config());
        let backoff = upgrade.current_backoff();
        assert_eq!(backoff, Duration::from_secs(10));
    }

    #[test]
    fn test_upgrade_stats_default() {
        let stats = UpgradeStats::default();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.upgraded_nodes, 0);
    }
}
