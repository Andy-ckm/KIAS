//! # Tenant Billing & Quota Management
//!
//! Provides QPS, Token, Storage, Tool Call, and Workflow quota management
//! with overage alerting and degradation strategies.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Quota Types ───────────────────────────────────────────────────────────────

/// Quota type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuotaType { Qps, Tokens, Storage, ToolCalls, Workflows, Custom }

impl std::fmt::Display for QuotaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaType::Qps => write!(f, "QPS"),
            QuotaType::Tokens => write!(f, "Tokens"),
            QuotaType::Storage => write!(f, "Storage"),
            QuotaType::ToolCalls => write!(f, "ToolCalls"),
            QuotaType::Workflows => write!(f, "Workflows"),
            QuotaType::Custom => write!(f, "Custom"),
        }
    }
}

/// A quota limit configuration
#[derive(Debug, Clone)]
pub struct QuotaLimit {
    pub quota_type: QuotaType,
    pub limit: u64,
    pub window_secs: u64,
    pub alert_threshold_pct: f64,
}

impl QuotaLimit {
    pub fn new(quota_type: QuotaType, limit: u64, window_secs: u64) -> Self {
        Self { quota_type, limit, window_secs, alert_threshold_pct: 0.8 }
    }
    pub fn with_alert_threshold(mut self, pct: f64) -> Self { self.alert_threshold_pct = pct; self }
}

/// Quota usage snapshot
#[derive(Debug, Clone)]
pub struct QuotaUsage {
    pub quota_type: QuotaType,
    pub current: u64,
    pub limit: u64,
    pub remaining: u64,
    pub usage_pct: f64,
    pub window_start: chrono::DateTime<chrono::Utc>,
    pub window_end: chrono::DateTime<chrono::Utc>,
    pub is_exceeded: bool,
}

impl QuotaUsage {
    pub fn new(limit: &QuotaLimit, current: u64) -> Self {
        let now = chrono::Utc::now();
        let remaining = limit.limit.saturating_sub(current);
        let usage_pct = if limit.limit == 0 { 0.0 } else { current as f64 / limit.limit as f64 };
        Self {
            quota_type: limit.quota_type, current, limit: limit.limit, remaining, usage_pct,
            window_start: now,
            window_end: now + chrono::Duration::seconds(limit.window_secs as i64),
            is_exceeded: current > limit.limit,
        }
    }
}

// ── Quota Manager ──────────────────────────────────────────────────────────────

pub struct QuotaManager {
    tenant_quotas: Arc<RwLock<HashMap<String, HashMap<QuotaType, QuotaLimit>>>>,
    usage_tracking: Arc<RwLock<HashMap<String, HashMap<QuotaType, UsageWindow>>>>,
    degradation_policies: Arc<RwLock<HashMap<String, DegradationPolicy>>>>,
}

impl Default for QuotaManager { fn default() -> Self { Self::new() } }

impl QuotaManager {
    pub fn new() -> Self {
        Self {
            tenant_quotas: Arc::new(RwLock::new(HashMap::new())),
            usage_tracking: Arc::new(RwLock::new(HashMap::new())),
            degradation_policies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_tenant(&self, tenant_id: &str, quotas: Vec<QuotaLimit>) {
        let mut quotas_map = self.tenant_quotas.write().await;
        let mut tracking = self.usage_tracking.write().await;
        quotas_map.insert(tenant_id.to_string(), quotas.into_iter().map(|q| (q.quota_type, q)).collect());
        tracking.insert(tenant_id.to_string(), HashMap::new());
    }

    pub async fn check_quota(&self, tenant_id: &str, quota_type: QuotaType, amount: u64) -> QuotaCheckResult {
        let quotas = self.tenant_quotas.read().await;
        let mut tracking = self.usage_tracking.write().await;

        match quotas.get(tenant_id).and_then(|q| q.get(&quota_type)) {
            None => QuotaCheckResult::Allowed { remaining: u64::MAX },
            Some(limit) => {
                let tenant_usage = tracking.entry(tenant_id.to_string()).or_insert_with(HashMap::new);
                let window = tenant_usage.entry(quota_type).or_insert_with(|| UsageWindow::new(limit.window_secs));
                window.prune_expired();
                let current = window.current_usage();
                let would_exceed = current + amount > limit.limit;
                let remaining = limit.limit.saturating_sub(current);
                if would_exceed {
                    QuotaCheckResult::Denied { reason: format!("quota exceeded: {} > {}", current + amount, limit.limit), remaining, limit: limit.limit }
                } else {
                    QuotaCheckResult::Allowed { remaining }
                }
            }
        }
    }

    pub async fn record_usage(&self, tenant_id: &str, quota_type: QuotaType, amount: u64) {
        let quotas = self.tenant_quotas.read().await;
        let mut tracking = self.usage_tracking.write().await;
        if let Some(limit) = quotas.get(tenant_id).and_then(|q| q.get(&quota_type)) {
            let tenant_usage = tracking.entry(tenant_id.to_string()).or_insert_with(HashMap::new);
            let window = tenant_usage.entry(quota_type).or_insert_with(|| UsageWindow::new(limit.window_secs));
            window.add(amount);
        }
    }

    pub async fn get_usage(&self, tenant_id: &str) -> HashMap<QuotaType, QuotaUsage> {
        let quotas = self.tenant_quotas.read().await;
        let tracking = self.usage_tracking.read().await;
        let mut result = HashMap::new();
        if let Some(tenant_quotas) = quotas.get(tenant_id) {
            if let Some(tenant_usage) = tracking.get(tenant_id) {
                for (qt, limit) in tenant_quotas {
                    let current = tenant_usage.get(qt).map(|w| w.current_usage()).unwrap_or(0);
                    result.insert(*qt, QuotaUsage::new(limit, current));
                }
            }
        }
        result
    }

    pub async fn set_degradation_policy(&self, tenant_id: &str, policy: DegradationPolicy) {
        self.degradation_policies.write().await.insert(tenant_id.to_string(), policy);
    }

    pub async fn get_degradation(&self, tenant_id: &str, quota_type: QuotaType) -> Option<DegradationStrategy> {
        let policies = self.degradation_policies.read().await;
        policies.get(tenant_id).and_then(|p| p.strategies.get(&quota_type).cloned())
    }
}

#[derive(Debug)]
struct UsageWindow {
    entries: VecDeque<UsageEntry>,
    window_secs: u64,
}

#[derive(Debug, Clone)]
struct UsageEntry { amount: u64, timestamp: chrono::DateTime<chrono::Utc> }

impl UsageWindow {
    fn new(window_secs: u64) -> Self { Self { entries: VecDeque::new(), window_secs } }
    fn add(&mut self, amount: u64) { self.entries.push_back(UsageEntry { amount, timestamp: chrono::Utc::now() }); }
    fn prune_expired(&mut self) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(self.window_secs as i64);
        while self.entries.front().map(|e| e.timestamp < cutoff).unwrap_or(false) { self.entries.pop_front(); }
    }
    fn current_usage(&self) -> u64 { self.entries.iter().map(|e| e.amount).sum() }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaCheckResult { Allowed { remaining: u64 }, Denied { reason: String, remaining: u64, limit: u64 } }

impl QuotaCheckResult {
    pub fn is_allowed(&self) -> bool { matches!(self, QuotaCheckResult::Allowed { .. }) }
    pub fn is_denied(&self) -> bool { matches!(self, QuotaCheckResult::Denied { .. }) }
}

// ── Degradation Policy ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DegradationPolicy {
    pub tenant_id: String,
    pub strategies: HashMap<QuotaType, DegradationStrategy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradationStrategy { Reject, Queue, ReduceQuality, Enqueue, DegradeFeatures }

impl DegradationStrategy {
    pub fn is_blocking(&self) -> bool { matches!(self, DegradationStrategy::Reject) }
}

// ── Billing Record ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingRecord {
    pub record_id: String,
    pub tenant_id: String,
    pub quota_type: QuotaType,
    pub amount: u64,
    pub cost_usd: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
}

impl BillingRecord {
    pub fn new(tenant_id: &str, quota_type: QuotaType, amount: u64, cost_usd: f64, period_hours: i64) -> Self {
        let now = chrono::Utc::now();
        Self {
            record_id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            quota_type,
            amount,
            cost_usd,
            timestamp: now,
            period_start: now - chrono::Duration::hours(period_hours),
            period_end: now,
        }
    }
}

pub struct BillingTracker {
    records: Arc<RwLock<VecDeque<BillingRecord>>>,
    tenant_balances: Arc<RwLock<HashMap<String, f64>>>,
}

impl Default for BillingTracker { fn default() -> Self { Self::new() } }

impl BillingTracker {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(VecDeque::new())),
            tenant_balances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_record(&self, record: BillingRecord) {
        self.records.write().await.push_back(record.clone());
        *self.tenant_balances.write().await.entry(record.tenant_id.clone()).or_insert(0.0) += record.cost_usd;
    }

    pub async fn get_tenant_balance(&self, tenant_id: &str) -> f64 {
        *self.tenant_balances.read().await.get(tenant_id).unwrap_or(&0.0)
    }

    pub async fn get_records(&self, tenant_id: &str) -> Vec<BillingRecord> {
        let records = self.records.read().await;
        records.iter().filter(|r| r.tenant_id == tenant_id).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quota_manager_register() {
        let manager = QuotaManager::new();
        manager.register_tenant("tenant1", vec![QuotaLimit::new(QuotaType::Qps, 100, 60), QuotaLimit::new(QuotaType::Tokens, 10000, 3600)]).await;
        let usage = manager.get_usage("tenant1").await;
        assert_eq!(usage.len(), 2);
    }

    #[tokio::test]
    async fn test_quota_check_allowed() {
        let manager = QuotaManager::new();
        manager.register_tenant("tenant1", vec![QuotaLimit::new(QuotaType::Qps, 100, 60)]).await;
        let result = manager.check_quota("tenant1", QuotaType::Qps, 50).await;
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_quota_check_denied() {
        let manager = QuotaManager::new();
        manager.register_tenant("tenant1", vec![QuotaLimit::new(QuotaType::Qps, 100, 60)]).await;
        manager.record_usage("tenant1", QuotaType::Qps, 90).await;
        let result = manager.check_quota("tenant1", QuotaType::Qps, 20).await;
        assert!(result.is_denied());
    }

    #[tokio::test]
    async fn test_quota_record_usage() {
        let manager = QuotaManager::new();
        manager.register_tenant("tenant1", vec![QuotaLimit::new(QuotaType::Tokens, 1000, 3600)]).await;
        manager.record_usage("tenant1", QuotaType::Tokens, 100).await;
        let usage = manager.get_usage("tenant1").await;
        assert_eq!(usage.get(&QuotaType::Tokens).unwrap().current, 100);
    }

    #[tokio::test]
    async fn test_billing_record_creation() {
        let record = BillingRecord::new("tenant1", QuotaType::Tokens, 1000, 0.05, 1);
        assert!(!record.record_id.is_empty());
        assert_eq!(record.tenant_id, "tenant1");
    }

    #[tokio::test]
    async fn test_billing_tracker_balance() {
        let tracker = BillingTracker::new();
        tracker.add_record(BillingRecord::new("tenant1", QuotaType::Tokens, 100, 0.05, 1)).await;
        tracker.add_record(BillingRecord::new("tenant1", QuotaType::Qps, 50, 0.02, 1)).await;
        let balance = tracker.get_tenant_balance("tenant1").await;
        assert!((balance - 0.07).abs() < 0.001);
    }

    #[test]
    fn test_degradation_strategy_blocking() {
        assert!(DegradationStrategy::Reject.is_blocking());
        assert!(!DegradationStrategy::Queue.is_blocking());
        assert!(!DegradationStrategy::ReduceQuality.is_blocking());
    }
}
