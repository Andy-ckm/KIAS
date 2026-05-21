use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cost attribution engine — tracks LLM costs per Agent per Task.
///
/// Key capability that EMQ completely lacks: "Which Agent spent how much on what?"
/// This is the feature CFOs love.

/// Cost entry for a single LLM call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    /// Unique entry ID
    pub id: String,

    /// Agent that made the call
    pub agent_id: String,

    /// Task/workflow ID (optional)
    pub task_id: Option<String>,

    /// LLM provider (openai, anthropic, xiaomicoding, etc.)
    pub provider: String,

    /// Model name (gpt-4, claude-3, etc.)
    pub model: String,

    /// Input tokens
    pub input_tokens: u64,

    /// Output tokens
    pub output_tokens: u64,

    /// Cost in USD
    pub cost_usd: f64,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Whether the call was cached (no cost)
    pub cached: bool,
}

/// Cost summary for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCostSummary {
    pub agent_id: String,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub call_count: u64,
    pub cached_calls: u64,
    pub avg_cost_per_call: f64,
    pub by_model: HashMap<String, ModelCost>,
}

/// Cost breakdown by model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    pub model: String,
    pub cost_usd: f64,
    pub call_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Budget alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    /// Agent ID (or "*" for global)
    pub agent_id: String,

    /// Budget limit in USD per day
    pub daily_limit_usd: f64,

    /// Budget limit in USD per month
    pub monthly_limit_usd: f64,

    /// Alert threshold (0.0 - 1.0, e.g., 0.8 = alert at 80%)
    pub alert_threshold: f64,
}

/// Budget alert status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub agent_id: String,
    pub daily_spent: f64,
    pub daily_limit: f64,
    pub daily_usage_pct: f64,
    pub monthly_spent: f64,
    pub monthly_limit: f64,
    pub monthly_usage_pct: f64,
    pub alert_triggered: bool,
}

/// Cost attribution engine
#[derive(Debug)]
pub struct CostEngine {
    entries: Arc<RwLock<Vec<CostEntry>>>,
    alerts: Arc<RwLock<Vec<BudgetAlert>>>,
    max_entries: usize,
}

impl CostEngine {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            max_entries,
        }
    }

    /// Record a cost entry
    pub async fn record(&self, entry: CostEntry) {
        let mut entries = self.entries.write().await;

        // Evict old entries if at capacity
        if entries.len() >= self.max_entries {
            let drain = self.max_entries / 10;
            entries.drain(..drain);
        }

        entries.push(entry);
    }

    /// Set a budget alert
    pub async fn set_alert(&self, alert: BudgetAlert) {
        let mut alerts = self.alerts.write().await;
        // Replace existing alert for same agent
        alerts.retain(|a| a.agent_id != alert.agent_id);
        alerts.push(alert);
    }

    /// Get cost summary for a specific agent
    pub async fn agent_summary(&self, agent_id: &str) -> AgentCostSummary {
        let entries = self.entries.read().await;
        let agent_entries: Vec<&CostEntry> = entries
            .iter()
            .filter(|e| e.agent_id == agent_id)
            .collect();

        let total_cost: f64 = agent_entries.iter().map(|e| e.cost_usd).sum();
        let total_input: u64 = agent_entries.iter().map(|e| e.input_tokens).sum();
        let total_output: u64 = agent_entries.iter().map(|e| e.output_tokens).sum();
        let cached = agent_entries.iter().filter(|e| e.cached).count() as u64;
        let count = agent_entries.len() as u64;

        let mut by_model: HashMap<String, ModelCost> = HashMap::new();
        for entry in &agent_entries {
            let mc = by_model.entry(entry.model.clone()).or_insert(ModelCost {
                model: entry.model.clone(),
                cost_usd: 0.0,
                call_count: 0,
                input_tokens: 0,
                output_tokens: 0,
            });
            mc.cost_usd += entry.cost_usd;
            mc.call_count += 1;
            mc.input_tokens += entry.input_tokens;
            mc.output_tokens += entry.output_tokens;
        }

        AgentCostSummary {
            agent_id: agent_id.to_string(),
            total_cost_usd: total_cost,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            call_count: count,
            cached_calls: cached,
            avg_cost_per_call: if count > 0 { total_cost / count as f64 } else { 0.0 },
            by_model,
        }
    }

    /// Get global cost summary (all agents)
    pub async fn global_summary(&self) -> Vec<AgentCostSummary> {
        let entries = self.entries.read().await;
        let agent_ids: Vec<String> = entries
            .iter()
            .map(|e| e.agent_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        drop(entries);

        let mut summaries = Vec::new();
        for agent_id in agent_ids {
            summaries.push(self.agent_summary(&agent_id).await);
        }
        summaries.sort_by(|a, b| b.total_cost_usd.partial_cmp(&a.total_cost_usd).unwrap());
        summaries
    }

    /// Check budget status for an agent
    pub async fn check_budget(&self, agent_id: &str) -> Option<BudgetStatus> {
        let alerts = self.alerts.read().await;
        let alert = alerts.iter().find(|a| a.agent_id == agent_id || a.agent_id == "*")?;

        let entries = self.entries.read().await;
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let today_start = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);
        let month_start = now.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let month_start = DateTime::<Utc>::from_naive_utc_and_offset(month_start, Utc);

        let daily_spent: f64 = entries
            .iter()
            .filter(|e| (e.agent_id == agent_id || alert.agent_id == "*") && e.timestamp >= today_start)
            .map(|e| e.cost_usd)
            .sum();

        let monthly_spent: f64 = entries
            .iter()
            .filter(|e| (e.agent_id == agent_id || alert.agent_id == "*") && e.timestamp >= month_start)
            .map(|e| e.cost_usd)
            .sum();

        let daily_pct = daily_spent / alert.daily_limit_usd;
        let monthly_pct = monthly_spent / alert.monthly_limit_usd;

        Some(BudgetStatus {
            agent_id: agent_id.to_string(),
            daily_spent,
            daily_limit: alert.daily_limit_usd,
            daily_usage_pct: daily_pct,
            monthly_spent,
            monthly_limit: alert.monthly_limit_usd,
            monthly_usage_pct: monthly_pct,
            alert_triggered: daily_pct >= alert.alert_threshold
                || monthly_pct >= alert.alert_threshold,
        })
    }

    /// Get total cost across all agents
    pub async fn total_cost(&self) -> f64 {
        let entries = self.entries.read().await;
        entries.iter().map(|e| e.cost_usd).sum()
    }

    /// Get entry count
    pub async fn count(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// Get entries for a specific agent in time range
    pub async fn agent_entries_range(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<CostEntry> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| e.agent_id == agent_id && e.timestamp >= start && e.timestamp <= end)
            .cloned()
            .collect()
    }
}

impl Default for CostEngine {
    fn default() -> Self {
        Self::new(100_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(agent_id: &str, cost: f64, model: &str) -> CostEntry {
        CostEntry {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            task_id: None,
            provider: "openai".to_string(),
            model: model.to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: cost,
            timestamp: Utc::now(),
            duration_ms: 500,
            cached: false,
        }
    }

    #[tokio::test]
    async fn test_record_and_summary() {
        let engine = CostEngine::default();

        engine.record(make_entry("agent-1", 0.05, "gpt-4")).await;
        engine.record(make_entry("agent-1", 0.03, "gpt-4")).await;
        engine.record(make_entry("agent-2", 0.10, "claude-3")).await;

        let summary = engine.agent_summary("agent-1").await;
        assert_eq!(summary.call_count, 2);
        assert!((summary.total_cost_usd - 0.08).abs() < 0.001);

        let total = engine.total_cost().await;
        assert!((total - 0.18).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_global_summary() {
        let engine = CostEngine::default();

        engine.record(make_entry("agent-1", 0.05, "gpt-4")).await;
        engine.record(make_entry("agent-2", 0.10, "claude-3")).await;
        engine.record(make_entry("agent-1", 0.03, "gpt-4")).await;

        let summaries = engine.global_summary().await;
        assert_eq!(summaries.len(), 2);
        // agent-2 should be first (higher total: 0.10)
        assert_eq!(summaries[0].agent_id, "agent-2");
    }

    #[tokio::test]
    async fn test_budget_alert() {
        let engine = CostEngine::default();

        engine.set_alert(BudgetAlert {
            agent_id: "agent-1".to_string(),
            daily_limit_usd: 1.0,
            monthly_limit_usd: 20.0,
            alert_threshold: 0.8,
        }).await;

        // Spend 0.9 (90% of daily limit)
        engine.record(make_entry("agent-1", 0.9, "gpt-4")).await;

        let status = engine.check_budget("agent-1").await.unwrap();
        assert!(status.alert_triggered);
        assert!((status.daily_spent - 0.9).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_model_breakdown() {
        let engine = CostEngine::default();

        engine.record(make_entry("agent-1", 0.05, "gpt-4")).await;
        engine.record(make_entry("agent-1", 0.02, "gpt-3.5-turbo")).await;
        engine.record(make_entry("agent-1", 0.08, "gpt-4")).await;

        let summary = engine.agent_summary("agent-1").await;
        assert_eq!(summary.by_model.len(), 2);

        let gpt4 = summary.by_model.get("gpt-4").unwrap();
        assert_eq!(gpt4.call_count, 2);
        assert!((gpt4.cost_usd - 0.13).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_budget_not_exceeded() {
        let engine = CostEngine::default();

        engine.set_alert(BudgetAlert {
            agent_id: "agent-1".to_string(),
            daily_limit_usd: 10.0,
            monthly_limit_usd: 200.0,
            alert_threshold: 0.8,
        }).await;

        engine.record(make_entry("agent-1", 0.05, "gpt-4")).await;

        let status = engine.check_budget("agent-1").await.unwrap();
        assert!(!status.alert_triggered);
    }

    #[tokio::test]
    async fn test_global_budget() {
        let engine = CostEngine::default();

        engine.set_alert(BudgetAlert {
            agent_id: "*".to_string(),
            daily_limit_usd: 5.0,
            monthly_limit_usd: 100.0,
            alert_threshold: 0.8,
        }).await;

        engine.record(make_entry("agent-1", 3.0, "gpt-4")).await;
        engine.record(make_entry("agent-2", 2.5, "claude-3")).await;

        let status = engine.check_budget("agent-1").await.unwrap();
        assert!(status.alert_triggered); // 5.5 > 5.0 * 0.8
    }
}
