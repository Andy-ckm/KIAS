//! # Cost Model - Cost Attribution & Analytics
//!
//! Provides per-request, per-tool, per-tenant, and per-strategy cost breakdown
//! with time-window aggregation and threshold alerting.

use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Cost Dimensions ──────────────────────────────────────────────────────────

/// Cost dimension identifiers
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum CostDimension {
    Request(String),
    Tool(String),
    Tenant(String),
    Strategy(String),
    Model(String),
    Node(String),
}

impl std::fmt::Display for CostDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostDimension::Request(id) => write!(f, "request:{}", id),
            CostDimension::Tool(name) => write!(f, "tool:{}", name),
            CostDimension::Tenant(id) => write!(f, "tenant:{}", id),
            CostDimension::Strategy(name) => write!(f, "strategy:{}", name),
            CostDimension::Model(name) => write!(f, "model:{}", name),
            CostDimension::Node(id) => write!(f, "node:{}", id),
        }
    }
}

/// A single cost record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    pub dimension: CostDimension,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub compute_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl CostRecord {
    pub fn new(
        dimension: CostDimension,
        cost_usd: f64,
        input_tokens: u64,
        output_tokens: u64,
        compute_ms: u64,
    ) -> Self {
        Self {
            dimension,
            cost_usd,
            input_tokens,
            output_tokens,
            compute_ms,
            timestamp: chrono::Utc::now(),
        }
    }
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Cost breakdown by multiple dimensions
#[derive(Debug, Default, Clone)]
pub struct CostBreakdown {
    pub by_request: HashMap<String, f64>,
    pub by_tool: HashMap<String, f64>,
    pub by_tenant: HashMap<String, f64>,
    pub by_strategy: HashMap<String, f64>,
    pub by_model: HashMap<String, f64>,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl CostBreakdown {
    pub fn add_record(&mut self, record: &CostRecord) {
        match &record.dimension {
            CostDimension::Request(id) => {
                *self.by_request.entry(id.clone()).or_insert(0.0) += record.cost_usd;
            }
            CostDimension::Tool(name) => {
                *self.by_tool.entry(name.clone()).or_insert(0.0) += record.cost_usd;
            }
            CostDimension::Tenant(id) => {
                *self.by_tenant.entry(id.clone()).or_insert(0.0) += record.cost_usd;
            }
            CostDimension::Strategy(name) => {
                *self.by_strategy.entry(name.clone()).or_insert(0.0) += record.cost_usd;
            }
            CostDimension::Model(name) => {
                *self.by_model.entry(name.clone()).or_insert(0.0) += record.cost_usd;
            }
            CostDimension::Node(_) => {}
        }
        self.total_cost_usd += record.cost_usd;
        self.total_input_tokens += record.input_tokens;
        self.total_output_tokens += record.output_tokens;
    }
    pub fn merge(&mut self, other: &CostBreakdown) {
        for (k, v) in &other.by_request {
            *self.by_request.entry(k.clone()).or_insert(0.0) += v;
        }
        for (k, v) in &other.by_tool {
            *self.by_tool.entry(k.clone()).or_insert(0.0) += v;
        }
        for (k, v) in &other.by_tenant {
            *self.by_tenant.entry(k.clone()).or_insert(0.0) += v;
        }
        for (k, v) in &other.by_strategy {
            *self.by_strategy.entry(k.clone()).or_insert(0.0) += v;
        }
        for (k, v) in &other.by_model {
            *self.by_model.entry(k.clone()).or_insert(0.0) += v;
        }
        self.total_cost_usd += other.total_cost_usd;
        self.total_input_tokens += other.total_input_tokens;
        self.total_output_tokens += other.total_output_tokens;
    }
}

/// Cost summary for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub top_tenants: Vec<(String, f64)>,
    pub top_tools: Vec<(String, f64)>,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TimeWindow {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl TimeWindow {
    pub fn as_secs(&self) -> i64 {
        match self {
            TimeWindow::Minute => 60,
            TimeWindow::Hour => 3600,
            TimeWindow::Day => 86400,
            TimeWindow::Week => 604800,
            TimeWindow::Month => 2592000,
        }
    }
}

// ── Cost Aggregator ──────────────────────────────────────────────────────────

/// Thread-safe cost aggregator with time-window support
pub struct CostAggregator {
    records: Arc<RwLock<Vec<CostRecord>>>,
    window_aggregates: Arc<RwLock<BTreeMap<TimeWindow, CostBreakdown>>>,
    tenant_snapshots: Arc<RwLock<HashMap<String, TenantCostSnapshot>>>,
}

impl Default for CostAggregator {
    fn default() -> Self {
        Self::new()
    }
}
impl CostAggregator {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            window_aggregates: Arc::new(RwLock::new(BTreeMap::new())),
            tenant_snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn record(&self, record: CostRecord) {
        self.records.write().await.push(record.clone());
        let mut aggregates = self.window_aggregates.write().await;
        for window in [
            TimeWindow::Minute,
            TimeWindow::Hour,
            TimeWindow::Day,
            TimeWindow::Week,
            TimeWindow::Month,
        ] {
            let breakdown = aggregates
                .entry(window)
                .or_insert_with(CostBreakdown::default);
            breakdown.add_record(&record);
        }
    }
    pub async fn get_aggregate(&self, window: TimeWindow) -> CostBreakdown {
        let aggregates = self.window_aggregates.read().await;
        aggregates.get(&window).cloned().unwrap_or_default()
    }
    pub async fn get_summary(&self, window: TimeWindow) -> CostSummary {
        let aggregate = self.get_aggregate(window).await;
        let mut top_tenants: Vec<_> = aggregate
            .by_tenant
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        top_tenants.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_tenants: Vec<_> = top_tenants.into_iter().take(5).collect();
        let mut top_tools: Vec<_> = aggregate
            .by_tool
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        top_tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_tools: Vec<_> = top_tools.into_iter().take(5).collect();
        let now = chrono::Utc::now();
        CostSummary {
            total_cost_usd: aggregate.total_cost_usd,
            total_input_tokens: aggregate.total_input_tokens,
            total_output_tokens: aggregate.total_output_tokens,
            top_tenants,
            top_tools,
            period_start: now - chrono::Duration::seconds(window.as_secs()),
            period_end: now,
        }
    }
    pub async fn get_tenant_breakdown(&self, tenant_id: &str) -> CostBreakdown {
        let aggregates = self.window_aggregates.read().await;
        let mut breakdown = CostBreakdown::default();
        for agg in aggregates.values() {
            if let Some(cost) = agg.by_tenant.get(tenant_id) {
                breakdown.total_cost_usd += cost;
            }
            for (k, v) in &agg.by_tool {
                *breakdown.by_tool.entry(k.clone()).or_insert(0.0) += v;
            }
        }
        breakdown
    }
    pub async fn prune(&self, window: TimeWindow) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(window.as_secs());
        let mut records = self.records.write().await;
        records.retain(|r| r.timestamp > cutoff);
    }
    pub async fn record_count(&self) -> usize {
        self.records.read().await.len()
    }
}

// ── Cost Alerting ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExceedanceTier {
    Warning,
    Critical,
    Excess,
}

impl ExceedanceTier {
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio > 1.5 {
            ExceedanceTier::Excess
        } else if ratio > 1.0 {
            ExceedanceTier::Critical
        } else {
            ExceedanceTier::Warning
        }
    }
}

#[derive(Debug, Clone)]
pub struct CostAlert {
    pub alert_id: String,
    pub tenant_id: Option<String>,
    pub threshold_usd: f64,
    pub window: TimeWindow,
    pub severity: AlertSeverity,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl CostAlert {
    pub fn new(alert_id: &str, threshold_usd: f64, window: TimeWindow) -> Self {
        Self {
            alert_id: alert_id.to_string(),
            tenant_id: None,
            threshold_usd,
            window,
            severity: AlertSeverity::Warning,
            enabled: true,
        }
    }
    pub fn for_tenant(mut self, tenant_id: &str) -> Self {
        self.tenant_id = Some(tenant_id.to_string());
        self
    }
    pub fn with_severity(mut self, severity: AlertSeverity) -> Self {
        self.severity = severity;
        self
    }
}

#[derive(Debug, Clone)]
pub struct CostAlertInstance {
    pub alert: CostAlert,
    pub current_cost: f64,
    pub ratio: f64,
    pub tier: ExceedanceTier,
    pub triggered_at: chrono::DateTime<chrono::Utc>,
}

impl CostAlertInstance {
    pub fn new(alert: &CostAlert, current_cost: f64) -> Self {
        let ratio = current_cost / alert.threshold_usd;
        Self {
            alert: alert.clone(),
            current_cost,
            ratio,
            tier: ExceedanceTier::from_ratio(ratio),
            triggered_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TenantCostSnapshot {
    pub tenant_id: String,
    pub current_cost_usd: f64,
    pub threshold_usd: f64,
    pub window: TimeWindow,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl TenantCostSnapshot {
    pub fn new(tenant_id: &str, threshold_usd: f64, window: TimeWindow) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            current_cost_usd: 0.0,
            threshold_usd,
            window,
            last_updated: chrono::Utc::now(),
        }
    }
    pub fn ratio(&self) -> f64 {
        if self.threshold_usd == 0.0 {
            0.0
        } else {
            self.current_cost_usd / self.threshold_usd
        }
    }
    pub fn tier(&self) -> ExceedanceTier {
        ExceedanceTier::from_ratio(self.ratio())
    }
}

pub struct CostAlertManager {
    alerts: Arc<RwLock<Vec<CostAlert>>>,
    active_alerts: Arc<RwLock<HashMap<String, CostAlertInstance>>>,
    aggregator: Arc<CostAggregator>,
}

impl Default for CostAlertManager {
    fn default() -> Self {
        Self::new()
    }
}
impl CostAlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Arc::new(RwLock::new(Vec::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            aggregator: Arc::new(CostAggregator::new()),
        }
    }
    pub fn with_aggregator(aggregator: Arc<CostAggregator>) -> Self {
        Self {
            alerts: Arc::new(RwLock::new(Vec::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            aggregator,
        }
    }
    pub async fn register_alert(&self, alert: CostAlert) {
        self.alerts.write().await.push(alert);
    }
    pub async fn check_alerts(&self) -> Vec<CostAlertInstance> {
        let alerts = self.alerts.read().await;
        let mut triggered = Vec::new();
        for alert in alerts.iter() {
            if !alert.enabled {
                continue;
            }
            let aggregate = self.aggregator.get_aggregate(alert.window).await;
            let cost = match &alert.tenant_id {
                Some(tenant_id) => aggregate.by_tenant.get(tenant_id).copied().unwrap_or(0.0),
                None => aggregate.total_cost_usd,
            };
            if cost > alert.threshold_usd {
                triggered.push(CostAlertInstance::new(alert, cost));
            }
        }
        triggered
    }
    pub async fn get_active_alerts(&self) -> Vec<CostAlertInstance> {
        self.active_alerts.read().await.values().cloned().collect()
    }
    pub async fn clear_resolved(&self) {
        let mut active = self.active_alerts.write().await;
        let triggered = self.check_alerts().await;
        let triggered_ids: std::collections::HashSet<_> =
            triggered.iter().map(|t| t.alert.alert_id.clone()).collect();
        active.retain(|id, _| triggered_ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_record_creation() {
        let record = CostRecord::new(
            CostDimension::Tenant("tenant1".to_string()),
            0.05,
            1000,
            500,
            50,
        );
        assert_eq!(record.total_tokens(), 1500);
    }

    #[test]
    fn test_cost_breakdown_add_record() {
        let mut breakdown = CostBreakdown::default();
        let record = CostRecord::new(
            CostDimension::Tenant("tenant1".to_string()),
            10.0,
            1000,
            500,
            50,
        );
        breakdown.add_record(&record);
        assert_eq!(breakdown.by_tenant.get("tenant1"), Some(&10.0));
        assert_eq!(breakdown.total_cost_usd, 10.0);
    }

    #[tokio::test]
    async fn test_cost_aggregator_record_and_get() {
        let aggregator = CostAggregator::new();
        aggregator
            .record(CostRecord::new(
                CostDimension::Tenant("t1".to_string()),
                5.0,
                100,
                50,
                10,
            ))
            .await;
        let aggregate = aggregator.get_aggregate(TimeWindow::Hour).await;
        assert_eq!(aggregate.total_cost_usd, 5.0);
    }

    #[tokio::test]
    async fn test_cost_aggregator_multiple_records() {
        let aggregator = CostAggregator::new();
        for i in 0..5 {
            aggregator
                .record(CostRecord::new(
                    CostDimension::Tool(format!("tool{}", i)),
                    i as f64,
                    100,
                    50,
                    10,
                ))
                .await;
        }
        let aggregate = aggregator.get_aggregate(TimeWindow::Hour).await;
        assert_eq!(aggregate.total_cost_usd, 10.0);
    }

    #[tokio::test]
    async fn test_cost_aggregator_summary() {
        let aggregator = CostAggregator::new();
        aggregator
            .record(CostRecord::new(
                CostDimension::Tenant("t1".to_string()),
                100.0,
                1000,
                500,
                50,
            ))
            .await;
        let summary = aggregator.get_summary(TimeWindow::Hour).await;
        assert_eq!(summary.total_cost_usd, 100.0);
        assert!(!summary.top_tenants.is_empty());
    }

    #[tokio::test]
    async fn test_cost_alert_triggered() {
        let manager = CostAlertManager::new();
        manager
            .register_alert(CostAlert::new("alert1", 10.0, TimeWindow::Hour))
            .await;
        manager
            .aggregator
            .record(CostRecord::new(
                CostDimension::Tenant("t1".to_string()),
                15.0,
                100,
                50,
                10,
            ))
            .await;
        let triggered = manager.check_alerts().await;
        assert_eq!(triggered.len(), 1);
        assert!((triggered[0].ratio - 1.5).abs() < 0.01);
        assert_eq!(triggered[0].tier, ExceedanceTier::Critical);
    }

    #[test]
    fn test_exceedance_tier() {
        assert_eq!(ExceedanceTier::from_ratio(0.5), ExceedanceTier::Warning);
        assert_eq!(ExceedanceTier::from_ratio(0.9), ExceedanceTier::Warning);
        assert_eq!(ExceedanceTier::from_ratio(1.0), ExceedanceTier::Warning); // 1.0 is not > 1.0
        assert_eq!(ExceedanceTier::from_ratio(1.1), ExceedanceTier::Critical);
        assert_eq!(ExceedanceTier::from_ratio(1.3), ExceedanceTier::Critical);
        assert_eq!(ExceedanceTier::from_ratio(1.6), ExceedanceTier::Excess);
    }
}
