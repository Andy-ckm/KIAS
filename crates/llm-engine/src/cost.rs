//! 成本追踪器

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 成本追踪器
#[derive(Clone)]
pub struct CostTracker {
    /// 每日成本记录
    daily_costs: Arc<RwLock<HashMap<String, DailyCost>>>,
    /// Agent 成本记录
    agent_costs: Arc<RwLock<HashMap<String, AgentCostSummary>>>,
    /// 模型定价 (per 1M tokens)
    pricing: HashMap<String, ModelPricing>,
}

/// 每日成本
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyCost {
    pub date: String,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub requests: u64,
    pub by_model: HashMap<String, ModelCost>,
}

/// Agent 成本汇总
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCostSummary {
    /// Agent 标识
    pub agent_id: String,
    /// 总 token 消耗
    pub total_tokens: u64,
    /// 总成本 (USD)
    pub total_cost: f64,
    /// 总请求数
    pub total_requests: u64,
    /// 按模型分拆
    pub by_model: HashMap<String, ModelCost>,
    /// 按日期分拆
    pub by_date: HashMap<String, DailyCost>,
}

/// 模型成本
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCost {
    pub tokens: u64,
    pub cost: f64,
    pub requests: u64,
}

/// 模型定价
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    pub fn new() -> Self {
        let mut pricing = HashMap::new();

        // OpenAI 模型定价
        pricing.insert(
            "gpt-4o".to_string(),
            ModelPricing {
                input_cost_per_1m: 2.50,
                output_cost_per_1m: 10.00,
            },
        );
        pricing.insert(
            "gpt-4o-mini".to_string(),
            ModelPricing {
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
            },
        );
        pricing.insert(
            "gpt-4-turbo".to_string(),
            ModelPricing {
                input_cost_per_1m: 10.00,
                output_cost_per_1m: 30.00,
            },
        );

        // Anthropic 模型定价
        pricing.insert(
            "claude-sonnet-4-20250514".to_string(),
            ModelPricing {
                input_cost_per_1m: 3.00,
                output_cost_per_1m: 15.00,
            },
        );
        pricing.insert(
            "claude-3-5-haiku-20241022".to_string(),
            ModelPricing {
                input_cost_per_1m: 0.80,
                output_cost_per_1m: 4.00,
            },
        );

        Self {
            daily_costs: Arc::new(RwLock::new(HashMap::new())),
            agent_costs: Arc::new(RwLock::new(HashMap::new())),
            pricing,
        }
    }

    /// 记录使用量
    pub async fn record_usage(&self, model: &str, usage: &crate::types::TokenUsage) -> f64 {
        let cost = self.calculate_cost(model, usage);
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let mut costs = self.daily_costs.write().await;
        let daily = costs.entry(date.clone()).or_insert_with(|| DailyCost {
            date: date.clone(),
            ..Default::default()
        });

        daily.total_tokens += usage.total_tokens;
        daily.total_cost += cost;
        daily.requests += 1;

        let model_cost = daily.by_model.entry(model.to_string()).or_default();
        model_cost.tokens += usage.total_tokens;
        model_cost.cost += cost;
        model_cost.requests += 1;

        cost
    }

    /// 计算成本
    pub fn calculate_cost(&self, model: &str, usage: &crate::types::TokenUsage) -> f64 {
        if let Some(pricing) = self.pricing.get(model) {
            let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * pricing.input_cost_per_1m;
            let output_cost =
                (usage.completion_tokens as f64 / 1_000_000.0) * pricing.output_cost_per_1m;
            input_cost + output_cost
        } else {
            0.0
        }
    }

    /// 获取每日成本
    pub async fn get_daily_cost(&self, date: &str) -> Option<DailyCost> {
        let costs = self.daily_costs.read().await;
        costs.get(date).cloned()
    }

    /// 获取总成本
    pub async fn get_total_cost(&self) -> f64 {
        let costs = self.daily_costs.read().await;
        costs.values().map(|d| d.total_cost).sum()
    }

    /// 记录 Agent 使用量（同时更新每日成本和 Agent 成本）
    pub async fn record_agent_usage(
        &self,
        agent_id: &str,
        model: &str,
        usage: &crate::types::TokenUsage,
    ) -> f64 {
        let cost = self.calculate_cost(model, usage);
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

        // 更新每日成本
        {
            let mut costs = self.daily_costs.write().await;
            let daily = costs.entry(date.clone()).or_insert_with(|| DailyCost {
                date: date.clone(),
                ..Default::default()
            });
            daily.total_tokens += usage.total_tokens;
            daily.total_cost += cost;
            daily.requests += 1;
            let model_cost = daily.by_model.entry(model.to_string()).or_default();
            model_cost.tokens += usage.total_tokens;
            model_cost.cost += cost;
            model_cost.requests += 1;
        }

        // 更新 Agent 成本
        {
            let mut agent_costs = self.agent_costs.write().await;
            let agent =
                agent_costs
                    .entry(agent_id.to_string())
                    .or_insert_with(|| AgentCostSummary {
                        agent_id: agent_id.to_string(),
                        ..Default::default()
                    });
            agent.total_tokens += usage.total_tokens;
            agent.total_cost += cost;
            agent.total_requests += 1;

            let model_cost = agent.by_model.entry(model.to_string()).or_default();
            model_cost.tokens += usage.total_tokens;
            model_cost.cost += cost;
            model_cost.requests += 1;

            let daily = agent
                .by_date
                .entry(date.clone())
                .or_insert_with(|| DailyCost {
                    date,
                    ..Default::default()
                });
            daily.total_tokens += usage.total_tokens;
            daily.total_cost += cost;
            daily.requests += 1;
            let model_cost = daily.by_model.entry(model.to_string()).or_default();
            model_cost.tokens += usage.total_tokens;
            model_cost.cost += cost;
            model_cost.requests += 1;
        }

        cost
    }

    /// 获取指定 Agent 的成本汇总
    pub async fn get_agent_cost(&self, agent_id: &str) -> Option<AgentCostSummary> {
        let agent_costs = self.agent_costs.read().await;
        agent_costs.get(agent_id).cloned()
    }

    /// 获取所有 Agent 的成本汇总
    pub async fn get_all_agent_costs(&self) -> Vec<AgentCostSummary> {
        let agent_costs = self.agent_costs.read().await;
        agent_costs.values().cloned().collect()
    }

    /// 获取 Agent 数量
    pub async fn agent_count(&self) -> usize {
        let agent_costs = self.agent_costs.read().await;
        agent_costs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TokenUsage;

    #[tokio::test]
    async fn test_cost_tracker_new() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.get_total_cost().await, 0.0);
    }

    #[tokio::test]
    async fn test_calculate_cost_known_model() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 500_000,
            total_tokens: 1_500_000,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        // Input: 1M * $2.50/1M = $2.50, Output: 0.5M * $10.00/1M = $5.00
        assert!((cost - 7.50).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        let cost = tracker.calculate_cost("unknown-model", &usage);
        assert_eq!(cost, 0.0);
    }

    #[tokio::test]
    async fn test_record_usage() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        let cost = tracker.record_usage("gpt-4o", &usage).await;
        assert!(cost > 0.0);

        let total = tracker.get_total_cost().await;
        assert!((total - cost).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_daily_cost_tracking() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker.record_usage("gpt-4o", &usage).await;

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let daily = tracker.get_daily_cost(&date).await;
        assert!(daily.is_some());
        let daily = daily.unwrap();
        assert_eq!(daily.requests, 1);
        assert_eq!(daily.total_tokens, 1500);
    }

    #[tokio::test]
    async fn test_multiple_records_same_model() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker.record_usage("gpt-4o", &usage).await;
        tracker.record_usage("gpt-4o", &usage).await;

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let daily = tracker.get_daily_cost(&date).await.unwrap();
        assert_eq!(daily.requests, 2);
        assert_eq!(daily.total_tokens, 3000);
        assert_eq!(daily.by_model["gpt-4o"].requests, 2);
    }

    #[test]
    fn test_model_pricing_serialization() {
        let pricing = ModelPricing {
            input_cost_per_1m: 2.50,
            output_cost_per_1m: 10.00,
        };
        let json = serde_json::to_string(&pricing).unwrap();
        assert!(json.contains("2.5"));
        assert!(json.contains("10.0"));
    }

    #[test]
    fn test_daily_cost_default() {
        let dc = DailyCost::default();
        assert_eq!(dc.total_tokens, 0);
        assert_eq!(dc.total_cost, 0.0);
        assert_eq!(dc.requests, 0);
        assert!(dc.by_model.is_empty());
    }

    #[tokio::test]
    async fn test_record_agent_usage() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        let cost = tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;
        assert!(cost > 0.0);

        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        assert_eq!(agent.agent_id, "agent-1");
        assert_eq!(agent.total_tokens, 1500);
        assert_eq!(agent.total_requests, 1);
        assert!((agent.total_cost - cost).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_agent_cost_by_model() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;
        tracker
            .record_agent_usage("agent-1", "claude-sonnet-4-20250514", &usage)
            .await;

        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        assert_eq!(agent.total_requests, 2);
        assert_eq!(agent.total_tokens, 3000);
        assert!(agent.by_model.contains_key("gpt-4o"));
        assert!(agent.by_model.contains_key("claude-sonnet-4-20250514"));
    }

    #[tokio::test]
    async fn test_multiple_agents() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;
        tracker
            .record_agent_usage("agent-2", "gpt-4o", &usage)
            .await;

        assert_eq!(tracker.agent_count().await, 2);

        let all = tracker.get_all_agent_costs().await;
        assert_eq!(all.len(), 2);

        let agent1 = tracker.get_agent_cost("agent-1").await.unwrap();
        let agent2 = tracker.get_agent_cost("agent-2").await.unwrap();
        assert_eq!(agent1.total_tokens, 1500);
        assert_eq!(agent2.total_tokens, 1500);
    }

    #[tokio::test]
    async fn test_agent_cost_not_found() {
        let tracker = CostTracker::new();
        assert!(tracker.get_agent_cost("nonexistent").await.is_none());
        assert_eq!(tracker.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_record_agent_usage_updates_daily() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;

        // Daily cost should also be updated
        let total = tracker.get_total_cost().await;
        assert!(total > 0.0);
    }

    // ===== NEW TESTS: Edge cases, error paths, coverage =====

    #[test]
    fn test_calculate_cost_gpt4o_mini() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = tracker.calculate_cost("gpt-4o-mini", &usage);
        // Input: 1M * $0.15/1M = $0.15, Output: 1M * $0.60/1M = $0.60
        assert!((cost - 0.75).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_gpt4_turbo() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = tracker.calculate_cost("gpt-4-turbo", &usage);
        // Input: 1M * $10/1M = $10, Output: 1M * $30/1M = $30
        assert!((cost - 40.0).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_claude_haiku() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = tracker.calculate_cost("claude-3-5-haiku-20241022", &usage);
        // Input: 1M * $0.80/1M = $0.80, Output: 1M * $4.00/1M = $4.00
        assert!((cost - 4.80).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_claude_sonnet() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = tracker.calculate_cost("claude-sonnet-4-20250514", &usage);
        // Input: 1M * $3.00/1M = $3.00, Output: 1M * $15.00/1M = $15.00
        assert!((cost - 18.0).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_input_only() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 500_000,
            completion_tokens: 0,
            total_tokens: 500_000,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        // 0.5M * $2.50/1M = $1.25
        assert!((cost - 1.25).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_output_only() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 500_000,
            total_tokens: 500_000,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        // 0.5M * $10.00/1M = $5.00
        assert!((cost - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_small_token_counts() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        // Input: 1/1M * $2.50 = $0.0000025, Output: 1/1M * $10.00 = $0.00001
        let expected = 2.50 / 1_000_000.0 + 10.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-12);
    }

    #[test]
    fn test_calculate_cost_unknown_model_variants() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        // Various invalid model names should all return 0.0
        assert_eq!(tracker.calculate_cost("", &usage), 0.0);
        assert_eq!(tracker.calculate_cost("gpt-5", &usage), 0.0);
        assert_eq!(
            tracker.calculate_cost("GPT-4O", &usage),
            0.0
        ); // case-sensitive
        assert_eq!(
            tracker.calculate_cost("gpt-4o ", &usage),
            0.0
        ); // trailing space
    }

    #[test]
    fn test_calculate_cost_large_token_counts() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 100_000_000,
            completion_tokens: 50_000_000,
            total_tokens: 150_000_000,
        };
        let cost = tracker.calculate_cost("gpt-4o", &usage);
        // Input: 100M * $2.50/1M = $250, Output: 50M * $10/1M = $500
        assert!((cost - 750.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_record_usage_unknown_model_returns_zero() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        let cost = tracker.record_usage("nonexistent-model", &usage).await;
        assert_eq!(cost, 0.0);

        // Usage is still recorded even though cost is 0
        let total = tracker.get_total_cost().await;
        assert_eq!(total, 0.0);
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let daily = tracker.get_daily_cost(&date).await.unwrap();
        assert_eq!(daily.total_tokens, 1500);
        assert_eq!(daily.requests, 1);
        assert_eq!(daily.total_cost, 0.0);
    }

    #[tokio::test]
    async fn test_get_daily_cost_nonexistent_date() {
        let tracker = CostTracker::new();
        assert!(tracker.get_daily_cost("2099-01-01").await.is_none());
    }

    #[tokio::test]
    async fn test_get_total_cost_empty_tracker() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.get_total_cost().await, 0.0);
    }

    #[tokio::test]
    async fn test_get_all_agent_costs_empty() {
        let tracker = CostTracker::new();
        let all = tracker.get_all_agent_costs().await;
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_agent_cost_by_date_tracking() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;

        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert!(agent.by_date.contains_key(&date));
        let daily = &agent.by_date[&date];
        assert_eq!(daily.requests, 1);
        assert_eq!(daily.total_tokens, 1500);
        assert!(daily.total_cost > 0.0);
    }

    #[tokio::test]
    async fn test_agent_cost_multiple_records_same_agent() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        // Record 3 times for same agent with same model
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;
        tracker
            .record_agent_usage("agent-1", "gpt-4o", &usage)
            .await;

        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        assert_eq!(agent.total_requests, 3);
        assert_eq!(agent.total_tokens, 4500);
        assert_eq!(agent.by_model["gpt-4o"].requests, 3);
        assert_eq!(agent.by_model["gpt-4o"].tokens, 4500);
    }

    #[tokio::test]
    async fn test_multiple_records_different_models() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        tracker.record_usage("gpt-4o", &usage).await;
        tracker.record_usage("gpt-4o-mini", &usage).await;
        tracker.record_usage("claude-sonnet-4-20250514", &usage).await;

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let daily = tracker.get_daily_cost(&date).await.unwrap();
        assert_eq!(daily.requests, 3);
        assert_eq!(daily.total_tokens, 4500);
        assert!(daily.by_model.contains_key("gpt-4o"));
        assert!(daily.by_model.contains_key("gpt-4o-mini"));
        assert!(daily.by_model.contains_key("claude-sonnet-4-20250514"));
    }

    #[tokio::test]
    async fn test_cost_accumulation_accuracy() {
        let tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost1 = tracker.record_usage("gpt-4o", &usage).await;
        let cost2 = tracker.record_usage("gpt-4o", &usage).await;

        let total = tracker.get_total_cost().await;
        assert!((total - (cost1 + cost2)).abs() < 0.0001);
        assert!((cost1 - cost2).abs() < 1e-12); // Same usage = same cost
    }

    #[tokio::test]
    async fn test_default_cost_tracker() {
        let tracker = CostTracker::default();
        assert_eq!(tracker.get_total_cost().await, 0.0);
        assert_eq!(tracker.agent_count().await, 0);
    }

    #[test]
    fn test_model_cost_default() {
        let mc = ModelCost::default();
        assert_eq!(mc.tokens, 0);
        assert_eq!(mc.cost, 0.0);
        assert_eq!(mc.requests, 0);
    }

    #[test]
    fn test_agent_cost_summary_default() {
        let ac = AgentCostSummary::default();
        assert_eq!(ac.agent_id, "");
        assert_eq!(ac.total_tokens, 0);
        assert_eq!(ac.total_cost, 0.0);
        assert_eq!(ac.total_requests, 0);
        assert!(ac.by_model.is_empty());
        assert!(ac.by_date.is_empty());
    }

    #[test]
    fn test_model_pricing_deserialization() {
        let json = r#"{"input_cost_per_1m":2.5,"output_cost_per_1m":10.0}"#;
        let pricing: ModelPricing = serde_json::from_str(json).unwrap();
        assert_eq!(pricing.input_cost_per_1m, 2.5);
        assert_eq!(pricing.output_cost_per_1m, 10.0);
    }

    // ========== 额外测试 ==========

    #[tokio::test]
    async fn test_agent_count_after_registration() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.agent_count().await, 0);
        let usage = TokenUsage { prompt_tokens: 100, completion_tokens: 50, total_tokens: 150 };
        tracker.record_agent_usage("a1", "gpt-4o", &usage).await;
        assert_eq!(tracker.agent_count().await, 1);
        tracker.record_agent_usage("a2", "gpt-4o", &usage).await;
        assert_eq!(tracker.agent_count().await, 2);
        // Same agent doesn't increase count
        tracker.record_agent_usage("a1", "gpt-4o", &usage).await;
        assert_eq!(tracker.agent_count().await, 2);
    }

    #[tokio::test]
    async fn test_get_all_agent_costs_multiple() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 100, completion_tokens: 50, total_tokens: 150 };
        tracker.record_agent_usage("a1", "gpt-4o", &usage).await;
        tracker.record_agent_usage("a2", "gpt-4o-mini", &usage).await;
        let all = tracker.get_all_agent_costs().await;
        assert_eq!(all.len(), 2);
        let ids: Vec<&str> = all.iter().map(|a| a.agent_id.as_str()).collect();
        assert!(ids.contains(&"a1"));
        assert!(ids.contains(&"a2"));
    }

    #[tokio::test]
    async fn test_record_usage_updates_total_cost() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 1_000_000, completion_tokens: 0, total_tokens: 1_000_000 };
        let cost = tracker.record_usage("gpt-4o", &usage).await;
        assert!(cost > 0.0);
        let total = tracker.get_total_cost().await;
        assert!((total - cost).abs() < 1e-12);
    }

    #[tokio::test]
    async fn test_daily_cost_not_found() {
        let tracker = CostTracker::new();
        assert!(tracker.get_daily_cost("2020-01-01").await.is_none());
    }

    #[tokio::test]
    async fn test_agent_cost_date_tracking() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 1000, completion_tokens: 500, total_tokens: 1500 };
        tracker.record_agent_usage("agent-1", "gpt-4o", &usage).await;
        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert!(agent.by_date.contains_key(&today));
        assert_eq!(agent.by_date[&today].total_tokens, 1500);
    }

    #[test]
    fn test_calculate_cost_deepseek_model() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 1_000_000, completion_tokens: 1_000_000, total_tokens: 2_000_000 };
        // deepseek not in pricing table, should return 0
        let cost = tracker.calculate_cost("deepseek-chat", &usage);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_all_builtin_models() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 1000, completion_tokens: 1000, total_tokens: 2000 };
        for model in &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "claude-sonnet-4-20250514", "claude-3-5-haiku-20241022"] {
            let cost = tracker.calculate_cost(model, &usage);
            assert!(cost > 0.0, "Cost for {} should be > 0", model);
        }
    }

    #[tokio::test]
    async fn test_record_agent_usage_different_models() {
        let tracker = CostTracker::new();
        let usage = TokenUsage { prompt_tokens: 1000, completion_tokens: 500, total_tokens: 1500 };
        tracker.record_agent_usage("agent-1", "gpt-4o", &usage).await;
        tracker.record_agent_usage("agent-1", "claude-sonnet-4-20250514", &usage).await;
        let agent = tracker.get_agent_cost("agent-1").await.unwrap();
        assert_eq!(agent.by_model.len(), 2);
        assert!(agent.by_model.contains_key("gpt-4o"));
        assert!(agent.by_model.contains_key("claude-sonnet-4-20250514"));
    }

    #[tokio::test]
    async fn test_cost_accumulation_precision() {
        let tracker = CostTracker::new();
        // Very small token count
        let tiny = TokenUsage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 };
        let cost = tracker.record_usage("gpt-4o", &tiny).await;
        assert!(cost > 0.0);
        assert!(cost < 0.01); // Should be very small
    }

    #[test]
    fn test_daily_cost_serialization() {
        let dc = DailyCost {
            date: "2025-01-01".to_string(),
            total_tokens: 1000,
            total_cost: 0.05,
            requests: 5,
            by_model: HashMap::new(),
        };
        let json = serde_json::to_string(&dc).unwrap();
        assert!(json.contains("2025-01-01"));
        assert!(json.contains("1000"));
        let deserialized: DailyCost = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.date, dc.date);
    }

    #[test]
    fn test_model_cost_serialization() {
        let mc = ModelCost { tokens: 500, cost: 0.01, requests: 3 };
        let json = serde_json::to_string(&mc).unwrap();
        let deserialized: ModelCost = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tokens, 500);
        assert_eq!(deserialized.requests, 3);
    }
}
