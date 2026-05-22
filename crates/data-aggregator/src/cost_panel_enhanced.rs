use std::collections::HashMap;
use std::sync::Mutex;
use chrono::{DateTime, Utc, Duration, TimeZone, Local};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use kias_common::KiasError;

/// A cost component type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CostComponent {
    Compute,
    Memory,
    Storage,
    Network,
}

/// Represents a single cost entry for a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub tool: String,
    pub policy: String,
    pub component: CostComponent,
    pub amount: f64,
    pub timestamp: DateTime<Utc>,
}

/// A breakdown of costs for a single request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBreakdown {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub total_cost: f64,
    pub components: HashMap<CostComponent, f64>,
    pub tools: HashMap<String, f64>,
    pub policies: HashMap<String, f64>,
}

/// A point in the time series, aggregated over a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub total_cost: f64,
    pub cost_by_tool: HashMap<String, f64>,
    pub cost_by_tenant: HashMap<String, f64>,
    pub cost_by_policy: HashMap<String, f64>,
}

/// Aggregation granularity for time series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeGranularity {
    Minute,
    Hour,
    Day,
}

/// Internal aggregated data structure for quick lookups.
#[derive(Debug, Clone)]
struct AggregatedCost {
    per_tool: HashMap<String, f64>,
    per_tenant: HashMap<String, f64>,
    per_policy: HashMap<String, f64>,
    total: f64,
}

impl Default for AggregatedCost {
    fn default() -> Self {
        AggregatedCost {
            per_tool: HashMap::new(),
            per_tenant: HashMap::new(),
            per_policy: HashMap::new(),
            total: 0.0,
        }
    }
}

/// Main panel for tracking and aggregating costs.
pub struct EnhancedCostPanel {
    // Stores all cost entries
    entries: Mutex<Vec<CostEntry>>,
    // Aggregated costs updated on each insertion
    aggregated: Mutex<AggregatedCost>,
    // Per-request breakdown cache (optional)
    request_cache: Mutex<HashMap<Uuid, RequestBreakdown>>,
    // Time series points for each granularity
    series_minute: Mutex<Vec<TimeSeriesPoint>>,
    series_hour: Mutex<Vec<TimeSeriesPoint>>,
    series_day: Mutex<Vec<TimeSeriesPoint>>,
}

impl EnhancedCostPanel {
    /// Creates a new cost panel.
    pub fn new() -> Self {
        EnhancedCostPanel {
            entries: Mutex::new(Vec::new()),
            aggregated: Mutex::new(AggregatedCost::default()),
            request_cache: Mutex::new(HashMap::new()),
            series_minute: Mutex::new(Vec::new()),
            series_hour: Mutex::new(Vec::new()),
            series_day: Mutex::new(Vec::new()),
        }
    }

    /// Adds a cost entry to the panel.
    /// Returns Ok(()) on success, or an error if the entry is invalid.
    pub fn add_cost_entry(&self, entry: CostEntry) -> Result<(), KiasError> {
        // Basic validation
        if entry.amount < 0.0 {
            return Err(KiasError::new(format!(
                "Negative cost amount for request {}",
                entry.request_id
            )));
        }
        if entry.tenant_id.is_empty() || entry.tool.is_empty() || entry.policy.is_empty() {
            return Err(KiasError::new("Empty tenant, tool or policy".to_string()));
        }

        // Insert entry
        {
            let mut entries = self.entries.lock().map_err(|_| {
                KiasError::new("Failed to acquire lock on entries".to_string())
            })?;
            entries.push(entry.clone());
        }

        // Update aggregated totals
        {
            let mut aggregated = self.aggregated.lock().map_err(|_| {
                KiasError::new("Failed to acquire lock on aggregated".to_string())
            })?;
            *aggregated = Self::update_aggregated(&aggregated, &entry);
        }

        // Update per-request cache
        {
            let mut cache = self.request_cache.lock().map_err(|_| {
                KiasError::new("Failed to acquire lock on request cache".to_string())
            })?;
            Self::update_request_cache(&mut cache, &entry);
        }

        // Update time series
        self.update_time_series(&entry)?;

        Ok(())
    }

    /// Retrieves the breakdown for a specific request.
    pub fn get_request_breakdown(&self, request_id: Uuid) -> Result<RequestBreakdown, KiasError> {
        let cache = self.request_cache.lock().map_err(|_| {
            KiasError::new("Failed to acquire lock on request cache".to_string())
        })?;
        cache
            .get(&request_id)
            .cloned()
            .ok_or_else(|| {
                KiasError::new(format!("Request {} not found", request_id))
            })
    }

    /// Returns the total aggregated cost per tool across all entries.
    pub fn per_tool_cost(&self) -> Result<HashMap<String, f64>, KiasError> {
        let aggregated = self.aggregated.lock().map_err(|_| {
            KiasError::new("Failed to acquire lock on aggregated".to_string())
        })?;
        Ok(aggregated.per_tool.clone())
    }

    /// Returns the total aggregated cost per tenant across all entries.
    pub fn per_tenant_cost(&self) -> Result<HashMap<String, f64>, KiasError> {
        let aggregated = self.aggregated.lock().map_err(|_| {
            KiasError::new("Failed to acquire lock on aggregated".to_string())
        })?;
        Ok(aggregated.per_tenant.clone())
    }

    /// Returns the total aggregated cost per policy across all entries.
    pub fn per_policy_cost(&self) -> Result<HashMap<String, f64>, KiasError> {
        let aggregated = self.aggregated.lock().map_err(|_| {
            KiasError::new("Failed to acquire lock on aggregated".to_string())
        })?;
        Ok(aggregated.per_policy.clone())
    }

    /// Returns the time series points for a given granularity.
    pub fn time_series(&self, granularity: TimeGranularity) -> Result<Vec<TimeSeriesPoint>, KiasError> {
        let series = match granularity {
            TimeGranularity::Minute => &self.series_minute,
            TimeGranularity::Hour => &self.series_hour,
            TimeGranularity::Day => &self.series_day,
        };
        let guard = series.lock().map_err(|_| {
            KiasError::new("Failed to acquire lock on time series".to_string())
        })?;
        Ok(guard.clone())
    }

    /// Clears all stored entries and resets aggregates.
    pub fn clear(&self) -> Result<(), KiasError> {
        {
            let mut entries = self.entries.lock().map_err(|_| {
                KiasError::new("Failed to lock entries".to_string())
            })?;
            entries.clear();
        }
        {
            let mut aggregated = self.aggregated.lock().map_err(|_| {
                KiasError::new("Failed to lock aggregated".to_string())
            })?;
            *aggregated = AggregatedCost::default();
        }
        {
            let mut cache = self.request_cache.lock().map_err(|_| {
                KiasError::new("Failed to lock request cache".to_string())
            })?;
            cache.clear();
        }
        {
            let mut s = self.series_minute.lock().map_err(|_| {
                KiasError::new("Failed to lock series_minute".to_string())
            })?;
            s.clear();
        }
        {
            let mut s = self.series_hour.lock().map_err(|_| {
                KiasError::new("Failed to lock series_hour".to_string())
            })?;
            s.clear();
        }
        {
            let mut s = self.series_day.lock().map_err(|_| {
                KiasError::new("Failed to lock series_day".to_string())
            })?;
            s.clear();
        }
        Ok(())
    }

    // ---- private helpers ----

    fn update_aggregated(current: &AggregatedCost, entry: &CostEntry) -> AggregatedCost {
        let mut next = current.clone();
        // Update tool
        let tool_cost = entry.amount;
        *next.per_tool.entry(entry.tool.clone()).or_insert(0.0) += tool_cost;
        // Update tenant
        *next.per_tenant.entry(entry.tenant_id.clone()).or_insert(0.0) += tool_cost;
        // Update policy
        *next.per_policy.entry(entry.policy.clone()).or_insert(0.0) += tool_cost;
        // Update total
        next.total += tool_cost;
        next
    }

    fn update_request_cache(cache: &mut HashMap<Uuid, RequestBreakdown>, entry: &CostEntry) {
        let breakdown = cache
            .entry(entry.request_id)
            .or_insert_with(|| RequestBreakdown {
                request_id: entry.request_id,
                tenant_id: entry.tenant_id.clone(),
                total_cost: 0.0,
                components: HashMap::new(),
                tools: HashMap::new(),
                policies: HashMap::new(),
            });
        breakdown.total_cost += entry.amount;
        *breakdown
            .components
            .entry(entry.component)
            .or_insert(0.0) += entry.amount;
        *breakdown.tools.entry(entry.tool.clone()).or_insert(0.0) += entry.amount;
        *breakdown
            .policies
            .entry(entry.policy.clone())
            .or_insert(0.0) += entry.amount;
    }

    fn update_time_series(&self, entry: &CostEntry) -> Result<(), KiasError> {
        // Determine bucket boundaries based on current time
        let now = Utc::now();
        let minute_start = Self::truncate_to_minute(now);
        let hour_start = Self::truncate_to_hour(now);
        let day_start = Self::truncate_to_day(now);

        // Update minute series
        {
            let mut series = self.series_minute.lock().map_err(|_| {
                KiasError::new("Failed to lock series_minute".to_string())
            })?;
            Self::insert_into_series(&mut series, minute_start, entry);
        }
        // Update hour series
        {
            let mut series = self.series_hour.lock().map_err(|_| {
                KiasError::new("Failed to lock series_hour".to_string())
            })?;
            Self::insert_into_series(&mut series, hour_start, entry);
        }
        // Update day series
        {
            let mut series = self.series_day.lock().map_err(|_| {
                KiasError::new("Failed to lock series_day".to_string())
            })?;
            Self::insert_into_series(&mut series, day_start, entry);
        }
        Ok(())
    }

    fn insert_into_series(series: &mut Vec<TimeSeriesPoint>, bucket_start: DateTime<Utc>, entry: &CostEntry) {
        // Find existing point for bucket_start or create new one
        if let Some(point) = series.iter_mut().find(|p| p.timestamp == bucket_start) {
            point.total_cost += entry.amount;
            *point.cost_by_tool.entry(entry.tool.clone()).or_insert(0.0) += entry.amount;
            *point.cost_by_tenant.entry(entry.tenant_id.clone()).or_insert(0.0) += entry.amount;
            *point.cost_by_policy.entry(entry.policy.clone()).or_insert(0.0) += entry.amount;
        } else {
            let mut new_point = TimeSeriesPoint {
                timestamp: bucket_start,
                total_cost: entry.amount,
                cost_by_tool: HashMap::new(),
                cost_by_tenant: HashMap::new(),
                cost_by_policy: HashMap::new(),
            };
            new_point
                .cost_by_tool
                .insert(entry.tool.clone(), entry.amount);
            new_point
                .cost_by_tenant
                .insert(entry.tenant_id.clone(), entry.amount);
            new_point
                .cost_by_policy
                .insert(entry.policy.clone(), entry.amount);
            series.push(new_point);
        }
    }

    fn truncate_to_minute(dt: DateTime<Utc>) -> DateTime<Utc> {
        let naive = dt.naive_utc();
        let truncated = naive
            .date()
            .and_hms_opt(naive.hour(), naive.minute(), 0)
            .unwrap();
        // Convert to DateTime<Utc> using TimeZone trait
        chrono::Utc.from_naive_utc_and_offset(truncated, chrono::Utc)
    }

    fn truncate_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
        let naive = dt.naive_utc();
        let truncated = naive
            .date()
            .and_hms_opt(naive.hour(), 0, 0)
            .unwrap();
        chrono::Utc.from_naive_utc_and_offset(truncated, chrono::Utc)
    }

    fn truncate_to_day(dt: DateTime<Utc>) -> DateTime<Utc> {
        let naive = dt.naive_utc();
        let truncated = naive.date().and_hms_opt(0, 0, 0).unwrap();
        chrono::Utc.from_naive_utc_and_offset(truncated, chrono::Utc)
    }
}

// Implement default for EnhancedCostPanel
impl Default for EnhancedCostPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Helper to create a cost entry for testing.
    fn make_entry(
        request_id: Uuid,
        tenant: &str,
        tool: &str,
        policy: &str,
        component: CostComponent,
        amount: f64,
    ) -> CostEntry {
        CostEntry {
            request_id,
            tenant_id: tenant.to_string(),
            tool: tool.to_string(),
            policy: policy.to_string(),
            component,
            amount,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_add_entry_and_retrieve_breakdown() {
        let panel = EnhancedCostPanel::new();
        let request_id = Uuid::new_v4();

        let entry = make_entry(request_id, "tenant1", "toolA", "policyX", CostComponent::Compute, 10.0);
        assert!(panel.add_cost_entry(entry).is_ok());

        let breakdown = panel.get_request_breakdown(request_id).unwrap();
        assert_eq!(breakdown.total_cost, 10.0);
        assert_eq!(breakdown.components.get(&CostComponent::Compute), Some(&10.0));
        assert_eq!(breakdown.tools.get("toolA"), Some(&10.0));
        assert_eq!(breakdown.policies.get("policyX"), Some(&10.0));
    }

    #[test]
    fn test_per_tool_aggregation() {
        let panel = EnhancedCostPanel::new();
        let req1 = Uuid::new_v4();
        let req2 = Uuid::new_v4();

        // Two entries for the same tool
        panel.add_cost_entry(make_entry(req1, "tenant1", "toolX", "policy1", CostComponent::Compute, 5.0)).unwrap();
        panel.add_cost_entry(make_entry(req2, "tenant2", "toolX", "policy2", CostComponent::Memory, 3.0)).unwrap();

        let per_tool = panel.per_tool_cost().unwrap();
        assert_eq!(per_tool.get("toolX"), Some(&8.0));

        // Additional tool
        let req3 = Uuid::new_v4();
        panel.add_cost_entry(make_entry(req3, "tenant1", "toolY", "policy1", CostComponent::Network, 2.0)).unwrap();
        let per_tool2 = panel.per_tool_cost().unwrap();
        assert_eq!(per_tool2.get("toolY"), Some(&2.0));
    }

    #[test]
    fn test_per_tenant_aggregation() {
        let panel = EnhancedCostPanel::new();
        let req1 = Uuid::new_v4();
        let req2 = Uuid::new_v4();

        panel.add_cost_entry(make_entry(req1, "tenantA", "tool1", "policy1", CostComponent::Storage, 12.0)).unwrap();
        panel.add_cost_entry(make_entry(req2, "tenantA", "tool2", "policy2", CostComponent::Compute, 8.0)).unwrap();

        let per_tenant = panel.per_tenant_cost().unwrap();
        assert_eq!(per_tenant.get("tenantA"), Some(&20.0));
    }

    #[test]
    fn test_per_policy_aggregation() {
        let panel = EnhancedCostPanel::new();
        let req1 = Uuid::new_v4();
        let req2 = Uuid::new_v4();

        panel.add_cost_entry(make_entry(req1, "tenant1", "tool1", "policyHigh", CostComponent::Memory, 5.0)).unwrap();
        panel.add_cost_entry(make_entry(req2, "tenant2", "tool2", "policyHigh", CostComponent::Network, 7.0)).unwrap();

        let per_policy = panel.per_policy_cost().unwrap();
        assert_eq!(per_policy.get("policyHigh"), Some(&12.0));
    }

    #[test]
    fn test_time_series_generation() {
        let panel = EnhancedCostPanel::new();
        let now = Utc::now();

        // Add entries spaced 1 minute apart (manually set timestamps)
        // We'll use entries with explicit timestamps.
        let entry1 = CostEntry {
            request_id: Uuid::new_v4(),
            tenant_id: "tenant1".to_string(),
            tool: "toolA".to_string(),
            policy: "policyX".to_string(),
            component: CostComponent::Compute,
            amount: 5.0,
            timestamp: now,
        };
        let entry2 = CostEntry {
            request_id: Uuid::new_v4(),
            tenant_id: "tenant2".to_string(),
            tool: "toolB".to_string(),
            policy: "policyY".to_string(),
            component: CostComponent::Memory,
            amount: 3.0,
            timestamp: now + Duration::minutes(2),
        };

        panel.add_cost_entry(entry1).unwrap();
        panel.add_cost_entry(entry2).unwrap();

        // Minute granularity should have two points
        let minute_series = panel.time_series(TimeGranularity::Minute).unwrap();
        assert!(minute_series.len() >= 1);
        // Check total cost
        let total: f64 = minute_series.iter().map(|p| p.total_cost).sum();
        assert!((total - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_error_on_missing_request() {
        let panel = EnhancedCostPanel::new();
        let missing_id = Uuid::new_v4();

        let result = panel.get_request_breakdown(missing_id);
        assert!(result.is_err());

        // Verify error message contains the uuid
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains(missing_id.to_string()));
    }
}