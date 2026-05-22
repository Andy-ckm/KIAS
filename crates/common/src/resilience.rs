//! Resilience primitives — TimeoutBudget, RetryBudget, BulkheadIsolation.
//!
//! ## TimeoutBudget
//! Tracks per-operation time remaining across a distributed call chain.
//!
//! ## RetryBudget
//! Limits retries by both max-attempts and time-window to prevent thundering-herd.
//!
//! ## BulkheadIsolation
//! Resource-pool isolation so that one misbehaving component cannot starve others.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// TimeoutBudget
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum allowed operation duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutBudget {
    pub max_duration_ms: u64,
    pub started_at: DateTime<Utc>,
}

impl TimeoutBudget {
    pub fn new(max_duration_ms: u64) -> Self {
        Self {
            max_duration_ms,
            started_at: Utc::now(),
        }
    }

    /// Remaining time in milliseconds.
    pub fn remaining_ms(&self) -> i64 {
        let elapsed = (Utc::now() - self.started_at).num_milliseconds();
        self.max_duration_ms as i64 - elapsed
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_ms() <= 0
    }

    /// Split this budget between N parallel sub-operations.
    pub fn split(&self, n: u32) -> Vec<TimeoutBudget> {
        let per_op_ms = self.max_duration_ms / n as u64;
        (0..n)
            .map(|_| Self {
                max_duration_ms: per_op_ms,
                started_at: self.started_at,
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RetryBudget
// ─────────────────────────────────────────────────────────────────────────────

/// Retry budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryBudgetConfig {
    pub max_attempts: u32,
    /// Time window in seconds for counting attempts.
    pub window_secs: i64,
    /// Minimum delay between retries (jitter applied on top of this).
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryBudgetConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            window_secs: 60,
            base_delay_ms: 100,
            max_delay_ms: 30_000,
        }
    }
}

/// Tracks retry eligibility within a time window.
#[derive(Debug, Clone)]
pub struct RetryBudget {
    config: RetryBudgetConfig,
    attempts: Vec<DateTime<Utc>>,
}

impl RetryBudget {
    pub fn new(config: RetryBudgetConfig) -> Self {
        Self {
            config,
            attempts: Vec::new(),
        }
    }

    /// Whether another retry attempt is allowed.
    pub fn can_retry(&self) -> bool {
        self.attempts.len() < self.config.max_attempts as usize
    }

    /// Record a new attempt.
    pub fn record_attempt(&mut self) {
        self.attempts.push(Utc::now());
    }

    /// Compute the back-off delay before the next retry (exponential with jitter).
    pub fn backoff_delay_ms(&self) -> u64 {
        let attempt = self.attempts.len();
        let exp = 2u64.saturating_pow(attempt.min(u32::MAX as usize) as u32);
        let delay = self.config.base_delay_ms * exp;
        delay.min(self.config.max_delay_ms)
    }

    /// Number of attempts in the current window (prunes expired first).
    pub fn attempt_count(&mut self) -> usize {
        self.prune();
        self.attempts.len()
    }

    fn prune(&mut self) {
        let cutoff = Utc::now() - Duration::seconds(self.config.window_secs);
        self.attempts.retain(|&t| t > cutoff);
    }

    /// Consume this budget (start retry cycle) — records attempt and returns delay.
    pub fn start_retry(&mut self) -> Option<RetryDelay> {
        if !self.can_retry() {
            return None;
        }
        let delay_ms = self.backoff_delay_ms();
        self.record_attempt();
        Some(RetryDelay {
            delay_ms,
            attempt_number: self.attempts.len() as u32,
            max_attempts: self.config.max_attempts,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryDelay {
    pub delay_ms: u64,
    pub attempt_number: u32,
    pub max_attempts: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// BulkheadIsolation
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a single bulkhead partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkheadConfig {
    pub name: String,
    pub max_concurrent: u32,
}

impl BulkheadConfig {
    pub fn new(name: &str, max_concurrent: u32) -> Self {
        Self {
            name: name.to_string(),
            max_concurrent,
        }
    }
}

/// Token bucket for a single bulkhead partition.
#[derive(Debug)]
struct Partition {
    config: BulkheadConfig,
    active: AtomicU64,
}

impl Partition {
    fn new(config: BulkheadConfig) -> Self {
        Self {
            config,
            active: AtomicU64::new(0),
        }
    }

    fn try_acquire(&self) -> bool {
        loop {
            let current = self.active.load(Ordering::Acquire);
            if current >= u64::from(self.config.max_concurrent) {
                return false;
            }
            if self
                .active
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    fn release(&self) {
        self.active.fetch_sub(1, Ordering::Release);
    }
}

/// BulkheadIsolation — resource pool isolation between named partitions.
#[derive(Debug)]
pub struct BulkheadIsolation {
    partitions: HashMap<String, Arc<Partition>>,
}

impl BulkheadIsolation {
    pub fn new(configs: Vec<BulkheadConfig>) -> Self {
        let partitions = configs
            .into_iter()
            .map(|c| (c.name.clone(), Arc::new(Partition::new(c))))
            .collect();
        Self { partitions }
    }

    /// Try to acquire a permit for the named partition.
    pub fn try_acquire(&self, name: &str) -> Option<BulkheadPermit> {
        let partition = self.partitions.get(name)?;
        if partition.try_acquire() {
            Some(BulkheadPermit {
                partition: partition.clone(),
            })
        } else {
            None
        }
    }

    /// Number of active calls in a partition.
    pub fn active_count(&self, name: &str) -> Option<u32> {
        self.partitions
            .get(name)
            .map(|p| p.active.load(Ordering::Acquire) as u32)
    }

    /// Names of all partitions.
    pub fn partition_names(&self) -> HashSet<&String> {
        self.partitions.keys().collect()
    }
}

/// RAII guard — released on drop.
#[derive(Debug)]
pub struct BulkheadPermit {
    partition: Arc<Partition>,
}

impl Drop for BulkheadPermit {
    fn drop(&mut self) {
        self.partition.release();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TimeoutBudget tests ─────────────────────────────────────────────────

    #[test]
    fn test_timeout_budget_not_expired_initially() {
        let b = TimeoutBudget::new(5000);
        assert!(!b.is_expired());
        assert!(b.remaining_ms() > 0);
    }

    #[test]
    fn test_timeout_budget_split_evenly() {
        let b = TimeoutBudget::new(300);
        let subs = b.split(3);
        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].max_duration_ms, 100);
        assert_eq!(subs[1].max_duration_ms, 100);
        assert_eq!(subs[2].max_duration_ms, 100);
    }

    // ── RetryBudget tests ──────────────────────────────────────────────────

    #[test]
    fn test_retry_budget_allows_within_limit() {
        let mut budget = RetryBudget::new(RetryBudgetConfig {
            max_attempts: 3,
            window_secs: 60,
            base_delay_ms: 10,
            max_delay_ms: 1000,
        });
        assert!(budget.can_retry());
        let delay = budget.start_retry();
        assert!(delay.is_some());
        assert_eq!(delay.unwrap().attempt_number, 1);
    }

    #[test]
    fn test_retry_budget_blocks_when_exhausted() {
        let config = RetryBudgetConfig {
            max_attempts: 2,
            window_secs: 60,
            base_delay_ms: 10,
            max_delay_ms: 1000,
        };
        let mut budget = RetryBudget::new(config.clone());
        budget.start_retry().unwrap();
        budget.start_retry().unwrap(); // second attempt
        assert!(!budget.can_retry());
        assert!(budget.start_retry().is_none());
    }

    #[test]
    fn test_retry_budget_exponential_backoff() {
        let mut budget = RetryBudget::new(RetryBudgetConfig {
            max_attempts: 5,
            window_secs: 60,
            base_delay_ms: 100,
            max_delay_ms: 10_000,
        });
        // After 0 attempts: next backoff = 100 * 2^0 = 100
        assert_eq!(budget.backoff_delay_ms(), 100);
        budget.record_attempt();
        // After 1 attempt: next backoff = 100 * 2^1 = 200
        assert_eq!(budget.backoff_delay_ms(), 200);
        budget.record_attempt();
        // After 2 attempts: next backoff = 100 * 2^2 = 400
        assert_eq!(budget.backoff_delay_ms(), 400);
        budget.record_attempt();
        // After 3 attempts: next backoff = 100 * 2^3 = 800
        assert_eq!(budget.backoff_delay_ms(), 800);
    }

    #[test]
    fn test_retry_budget_respects_max_delay() {
        let mut budget = RetryBudget::new(RetryBudgetConfig {
            max_attempts: 10,
            window_secs: 60,
            base_delay_ms: 10_000,
            max_delay_ms: 30_000,
        });
        for _ in 0..5 {
            budget.record_attempt();
        }
        // 10 * 2^5 = 320, exceeds max of 30
        assert!(budget.backoff_delay_ms() <= 30_000);
    }

    // ── BulkheadIsolation tests ─────────────────────────────────────────────

    #[test]
    fn test_bulkhead_acquire_release() {
        let bulkhead = BulkheadIsolation::new(vec![BulkheadConfig::new("io", 2)]);
        let permit = bulkhead.try_acquire("io");
        assert!(permit.is_some());
        drop(permit);
        assert_eq!(bulkhead.active_count("io"), Some(0));
    }

    #[test]
    fn test_bulkhead_blocks_when_full() {
        let bulkhead = BulkheadIsolation::new(vec![BulkheadConfig::new("cpu", 1)]);
        let _p1 = bulkhead.try_acquire("cpu");
        let p2 = bulkhead.try_acquire("cpu"); // second should be denied
        assert!(p2.is_none());
    }

    #[test]
    fn test_bulkhead_multiple_partitions_independent() {
        let bulkhead = BulkheadIsolation::new(vec![
            BulkheadConfig::new("a", 1),
            BulkheadConfig::new("b", 1),
        ]);
        let _pa = bulkhead.try_acquire("a");
        assert!(bulkhead.try_acquire("a").is_none()); // a is full
        assert!(bulkhead.try_acquire("b").is_some()); // b is independent
    }

    #[test]
    fn test_bulkhead_unknown_partition_returns_none() {
        let bulkhead = BulkheadIsolation::new(vec![BulkheadConfig::new("x", 5)]);
        assert!(bulkhead.try_acquire("unknown").is_none());
        assert!(bulkhead.active_count("unknown").is_none());
    }

    #[test]
    fn test_timeout_budget_serialization() {
        let b = TimeoutBudget::new(5000);
        let json = serde_json::to_string(&b).unwrap();
        let back: TimeoutBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(back.max_duration_ms, 5000);
    }
}
