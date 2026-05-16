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
}
