//! Smart Model Router
//!
//! Intelligent model selection based on task type, budget, and risk:
//! - ModelProfile: capability/cost/latency/quality ratings
//! - RoutingDecision: route + reasoning
//! - SmartRouter: automatic model selection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task category for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskCategory {
    CodeGeneration,
    CodeReview,
    Reasoning,
    Creative,
    Summarization,
    QnA,
    Translation,
    Classification,
    Unknown,
}

/// Model capability profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub model_id: String,
    pub provider: String,
    pub capabilities: Vec<TaskCategory>,
    /// Cost per 1K tokens (USD)
    pub cost_per_1k_tokens: f64,
    /// Latency estimate in ms
    pub latency_ms: u32,
    /// Quality score 0.0 - 1.0
    pub quality_score: f64,
    /// Max context length
    pub max_context: u32,
    /// Whether streaming is supported
    pub supports_streaming: bool,
    /// Risk level 0-10
    pub risk_level: u8,
}

impl ModelProfile {
    pub fn new(model_id: &str, provider: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            provider: provider.to_string(),
            capabilities: Vec::new(),
            cost_per_1k_tokens: 0.01,
            latency_ms: 1000,
            quality_score: 0.8,
            max_context: 4096,
            supports_streaming: true,
            risk_level: 3,
        }
    }

    pub fn with_capability(mut self, cap: TaskCategory) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_per_1k_tokens = cost;
        self
    }

    pub fn with_latency(mut self, latency: u32) -> Self {
        self.latency_ms = latency;
        self
    }

    pub fn with_quality(mut self, quality: f64) -> Self {
        self.quality_score = quality.clamp(0.0, 1.0);
        self
    }

    /// Calculate cost efficiency score
    pub fn cost_efficiency(&self) -> f64 {
        if self.cost_per_1k_tokens == 0.0 {
            return f64::MAX;
        }
        self.quality_score / self.cost_per_1k_tokens
    }

    /// Check if model supports a task
    pub fn supports_task(&self, task: TaskCategory) -> bool {
        self.capabilities.contains(&task) || self.capabilities.contains(&TaskCategory::Unknown)
    }
}

/// Budget constraints for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub max_cost_per_request: f64,
    pub max_latency_ms: u32,
    pub daily_budget: f64,
    pub daily_spent: f64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_cost_per_request: 0.10,
            max_latency_ms: 5000,
            daily_budget: 100.0,
            daily_spent: 0.0,
        }
    }
}

impl Budget {
    pub fn remaining(&self) -> f64 {
        self.daily_budget - self.daily_spent
    }

    pub fn can_afford(&self, cost: f64) -> bool {
        cost <= self.max_cost_per_request && cost <= self.remaining()
    }
}

/// Risk tolerance for routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RiskTolerance {
    Low, // Prefer safe, proven models
    #[default]
    Medium, // Balance cost/quality
    High, // Willing to try newer models
}

/// Routing decision with reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected_model: String,
    pub provider: String,
    pub reasoning: Vec<String>,
    pub estimated_cost: f64,
    pub estimated_latency_ms: u32,
    pub confidence: f64,
    pub alternatives: Vec<String>,
}

impl Default for RoutingDecision {
    fn default() -> Self {
        Self {
            selected_model: String::new(),
            provider: String::new(),
            reasoning: Vec::new(),
            estimated_cost: 0.0,
            estimated_latency_ms: 0,
            confidence: 0.0,
            alternatives: Vec::new(),
        }
    }
}

/// Routing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub task: TaskCategory,
    pub prompt_length: u32,
    pub requires_streaming: bool,
    pub budget: Budget,
    pub risk_tolerance: RiskTolerance,
    pub prefer_low_latency: bool,
}

impl Default for RoutingRequest {
    fn default() -> Self {
        Self {
            task: TaskCategory::Unknown,
            prompt_length: 100,
            requires_streaming: false,
            budget: Budget::default(),
            risk_tolerance: RiskTolerance::Medium,
            prefer_low_latency: false,
        }
    }
}

/// SmartRouter - intelligent model selection
pub struct SmartRouter {
    models: HashMap<String, ModelProfile>,
    fallback_chain: Vec<String>,
}

impl Default for SmartRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartRouter {
    pub fn new() -> Self {
        let mut router = Self {
            models: HashMap::new(),
            fallback_chain: Vec::new(),
        };
        router.register_defaults();
        router
    }

    /// Register default model profiles
    fn register_defaults(&mut self) {
        // GPT-4 class models
        self.register(
            ModelProfile::new("gpt-4o", "openai")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::CodeReview)
                .with_capability(TaskCategory::Reasoning)
                .with_capability(TaskCategory::Creative)
                .with_capability(TaskCategory::Summarization)
                .with_cost(0.015)
                .with_latency(2000)
                .with_quality(0.95)
                .with_quality(0.9),
        );

        self.register(
            ModelProfile::new("gpt-4o-mini", "openai")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::CodeReview)
                .with_capability(TaskCategory::Reasoning)
                .with_capability(TaskCategory::Summarization)
                .with_cost(0.0015)
                .with_latency(800)
                .with_quality(0.85),
        );

        // Claude class models
        self.register(
            ModelProfile::new("claude-sonnet-4", "anthropic")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::CodeReview)
                .with_capability(TaskCategory::Reasoning)
                .with_capability(TaskCategory::Creative)
                .with_capability(TaskCategory::Summarization)
                .with_cost(0.009)
                .with_latency(1800)
                .with_quality(0.93),
        );

        self.register(
            ModelProfile::new("claude-haiku-4", "anthropic")
                .with_capability(TaskCategory::Summarization)
                .with_capability(TaskCategory::QnA)
                .with_capability(TaskCategory::Classification)
                .with_cost(0.0008)
                .with_latency(500)
                .with_quality(0.82),
        );

        // DeepSeek models
        self.register(
            ModelProfile::new("deepseek-coder", "deepseek")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::CodeReview)
                .with_cost(0.001)
                .with_latency(1200)
                .with_quality(0.88),
        );

        // Qwen models
        self.register(
            ModelProfile::new("qwen2.5-coder", "alibaba")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::CodeReview)
                .with_cost(0.0008)
                .with_latency(1000)
                .with_quality(0.86),
        );

        // Local/Ollama models
        self.register(
            ModelProfile::new("llama3.1-8b", "ollama")
                .with_capability(TaskCategory::CodeGeneration)
                .with_capability(TaskCategory::Reasoning)
                .with_capability(TaskCategory::QnA)
                .with_cost(0.0) // Local, no API cost
                .with_latency(3000)
                .with_quality(0.75)
                .with_quality(0.78),
        );

        self.register(
            ModelProfile::new("codellama-7b", "ollama")
                .with_capability(TaskCategory::CodeGeneration)
                .with_cost(0.0)
                .with_latency(2500)
                .with_quality(0.72),
        );

        self.fallback_chain = vec![
            "gpt-4o-mini".to_string(),
            "claude-haiku-4".to_string(),
            "qwen2.5-coder".to_string(),
            "llama3.1-8b".to_string(),
        ];
    }

    /// Register a model profile
    pub fn register(&mut self, profile: ModelProfile) {
        self.models.insert(profile.model_id.clone(), profile);
    }

    /// Route a request to the best model
    pub fn route(&self, request: &RoutingRequest) -> RoutingDecision {
        let mut candidates: Vec<&ModelProfile> = self
            .models
            .values()
            .filter(|m| {
                // Filter by capability
                m.supports_task(request.task)
                // Filter by streaming requirement
                && (!request.requires_streaming || m.supports_streaming)
                // Filter by latency
                && m.latency_ms <= request.budget.max_latency_ms
            })
            .collect();

        if candidates.is_empty() {
            // Use fallback chain
            let default_fb = "gpt-4o-mini".to_string();
            let fallback_id = self.fallback_chain.first().unwrap_or(&default_fb);
            let fallback = self
                .models
                .get(fallback_id)
                .cloned()
                .unwrap_or_else(|| ModelProfile::new("unknown", "unknown"));
            return RoutingDecision {
                selected_model: fallback.model_id,
                provider: fallback.provider,
                reasoning: vec!["No matching model found, using fallback".to_string()],
                estimated_cost: fallback.cost_per_1k_tokens * request.prompt_length as f64 / 1000.0,
                estimated_latency_ms: fallback.latency_ms,
                confidence: 0.3,
                alternatives: self.fallback_chain.clone(),
            };
        }

        // Score and rank candidates
        candidates.sort_by(|a, b| {
            let score_a = self.score_model(a, request);
            let score_b = self.score_model(b, request);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let selected = candidates.first().expect("candidates should not be empty");
        let mut reasoning = Vec::new();

        // Generate reasoning
        if selected.quality_score > 0.9 {
            reasoning.push("High quality model selected".to_string());
        }
        if selected.cost_per_1k_tokens < 0.005 {
            reasoning.push("Cost-effective choice".to_string());
        }
        if selected.latency_ms < 1500 {
            reasoning.push("Low latency model".to_string());
        }
        if selected.risk_level < 3 {
            reasoning.push("Conservative, low-risk model".to_string());
        }
        reasoning.push(format!("Model supports {:?} tasks", request.task));

        let alternatives: Vec<String> = candidates
            .iter()
            .skip(1)
            .take(3)
            .map(|m| m.model_id.clone())
            .collect();

        let confidence = self.compute_confidence(selected, request, &candidates);

        RoutingDecision {
            selected_model: selected.model_id.clone(),
            provider: selected.provider.clone(),
            reasoning,
            estimated_cost: selected.cost_per_1k_tokens * request.prompt_length as f64 / 1000.0,
            estimated_latency_ms: selected.latency_ms,
            confidence,
            alternatives,
        }
    }

    /// Score a model for a request
    fn score_model(&self, model: &ModelProfile, request: &RoutingRequest) -> f64 {
        let mut score = 0.0;

        // Quality weight (higher for reasoning/coding tasks)
        let quality_weight = match request.task {
            TaskCategory::CodeGeneration | TaskCategory::CodeReview | TaskCategory::Reasoning => {
                0.5
            }
            TaskCategory::Creative => 0.4,
            _ => 0.3,
        };
        score += model.quality_score * quality_weight;

        // Cost weight (inverse - lower cost is better)
        let cost_weight = 0.25;
        if model.cost_per_1k_tokens > 0.0 {
            let cost_score = (0.02 / model.cost_per_1k_tokens).min(2.0);
            score += cost_score * cost_weight;
        } else {
            score += 0.5; // Free local models get a bonus
        }

        // Latency weight
        let latency_weight = if request.prefer_low_latency {
            0.35
        } else {
            0.15
        };
        let latency_score = if model.latency_ms < 1000 {
            1.0
        } else if model.latency_ms < 2000 {
            0.7
        } else {
            0.4
        };
        score += latency_score * latency_weight;

        // Risk adjustment
        let risk_weight = 0.1;
        let risk_score = match request.risk_tolerance {
            RiskTolerance::Low => {
                if model.risk_level <= 3 {
                    1.0
                } else {
                    0.5
                }
            }
            RiskTolerance::Medium => {
                if model.risk_level <= 5 {
                    1.0
                } else {
                    0.6
                }
            }
            RiskTolerance::High => 1.0,
        };
        score += risk_score * risk_weight;

        score
    }

    /// Compute confidence in routing decision
    fn compute_confidence(
        &self,
        selected: &ModelProfile,
        request: &RoutingRequest,
        candidates: &[&ModelProfile],
    ) -> f64 {
        if candidates.len() == 1 {
            return 0.5; // No alternatives, low confidence
        }

        let selected_score = self.score_model(selected, request);
        let second_best = candidates
            .get(1)
            .map(|m| self.score_model(m, request))
            .unwrap_or(0.0);

        let gap = selected_score - second_best;
        let confidence = if gap > 0.3 {
            0.9
        } else if gap > 0.1 {
            0.7
        } else {
            0.5
        };

        // Boost confidence if model has high quality for this specific task
        if selected.supports_task(request.task) && selected.quality_score > 0.85 {
            (confidence + 0.1_f64).min(1.0_f64)
        } else {
            confidence
        }
    }

    /// Get model profile by ID
    pub fn get_model(&self, model_id: &str) -> Option<&ModelProfile> {
        self.models.get(model_id)
    }

    /// List all registered models
    pub fn list_models(&self) -> Vec<&ModelProfile> {
        self.models.values().collect()
    }

    /// Estimate cost for a request with a specific model
    pub fn estimate_cost(
        &self,
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<f64> {
        self.models.get(model_id).map(|m| {
            let total_tokens = prompt_tokens + completion_tokens;
            m.cost_per_1k_tokens * (total_tokens as f64 / 1000.0)
        })
    }

    /// Get cheapest model for a task
    pub fn cheapest(&self, task: TaskCategory) -> Option<&ModelProfile> {
        self.models
            .values()
            .filter(|m| m.supports_task(task))
            .min_by(|a, b| {
                a.cost_per_1k_tokens
                    .partial_cmp(&b.cost_per_1k_tokens)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get highest quality model for a task
    pub fn best_quality(&self, task: TaskCategory) -> Option<&ModelProfile> {
        self.models
            .values()
            .filter(|m| m.supports_task(task))
            .max_by(|a, b| {
                a.quality_score
                    .partial_cmp(&b.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get fastest model for a task
    pub fn fastest(&self, task: TaskCategory) -> Option<&ModelProfile> {
        self.models
            .values()
            .filter(|m| m.supports_task(task))
            .min_by_key(|m| m.latency_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_router_new() {
        let router = SmartRouter::new();
        assert!(!router.list_models().is_empty());
    }

    #[test]
    fn test_route_code_generation() {
        let router = SmartRouter::new();
        let request = RoutingRequest {
            task: TaskCategory::CodeGeneration,
            prompt_length: 200,
            requires_streaming: false,
            budget: Budget::default(),
            risk_tolerance: RiskTolerance::Medium,
            prefer_low_latency: false,
        };

        let decision = router.route(&request);
        assert!(!decision.selected_model.is_empty());
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn test_route_with_budget_constraint() {
        let router = SmartRouter::new();
        let mut budget = Budget::default();
        budget.max_cost_per_request = 0.001;

        let request = RoutingRequest {
            task: TaskCategory::CodeGeneration,
            prompt_length: 200,
            budget,
            ..Default::default()
        };

        let decision = router.route(&request);
        // Should select a cheap model
        assert!(decision.estimated_cost <= 0.001);
    }

    #[test]
    fn test_route_prefers_low_latency() {
        let router = SmartRouter::new();
        let request = RoutingRequest {
            task: TaskCategory::QnA,
            prompt_length: 100,
            prefer_low_latency: true,
            ..Default::default()
        };

        let decision = router.route(&request);
        // Should route to a fast model
        assert!(decision.estimated_latency_ms < 2000);
    }

    #[test]
    fn test_model_profile_capabilities() {
        let profile = ModelProfile::new("test", "provider")
            .with_capability(TaskCategory::CodeGeneration)
            .with_capability(TaskCategory::Reasoning);

        assert!(profile.supports_task(TaskCategory::CodeGeneration));
        assert!(profile.supports_task(TaskCategory::Reasoning));
        assert!(!profile.supports_task(TaskCategory::Creative));
    }

    #[test]
    fn test_cost_efficiency() {
        let cheap = ModelProfile::new("cheap", "p")
            .with_cost(0.001)
            .with_quality(0.8);
        let expensive = ModelProfile::new("exp", "p")
            .with_cost(0.01)
            .with_quality(0.9);

        assert!(cheap.cost_efficiency() > expensive.cost_efficiency());
    }

    #[test]
    fn test_budget_remaining() {
        let budget = Budget {
            daily_budget: 100.0,
            daily_spent: 30.0,
            ..Default::default()
        };
        assert_eq!(budget.remaining(), 70.0);
    }

    #[test]
    fn test_budget_can_afford() {
        let budget = Budget {
            max_cost_per_request: 0.05,
            daily_budget: 100.0,
            daily_spent: 99.0,
            ..Default::default()
        };
        assert!(budget.can_afford(0.04));
        assert!(!budget.can_afford(0.06)); // exceeds max per request
        assert!(!budget.can_afford(2.0)); // exceeds remaining
    }

    #[test]
    fn test_cheapest_model() {
        let router = SmartRouter::new();
        let cheapest = router.cheapest(TaskCategory::CodeGeneration);
        assert!(cheapest.is_some());
        if let Some(c) = cheapest {
            assert!(c.cost_per_1k_tokens <= 0.002);
        }
    }

    #[test]
    fn test_best_quality_model() {
        let router = SmartRouter::new();
        let best = router.best_quality(TaskCategory::CodeGeneration);
        assert!(best.is_some());
        // GPT-4o should be among the best for code generation
        assert!(best
            .map(|m| m.model_id.contains("gpt-4") || m.model_id.contains("claude"))
            .unwrap_or(false));
    }

    #[test]
    fn test_estimate_cost() {
        let router = SmartRouter::new();
        let cost = router.estimate_cost("gpt-4o", 1000, 500);
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    #[test]
    fn test_routing_decision_has_alternatives() {
        let router = SmartRouter::new();
        let request = RoutingRequest::default();
        let decision = router.route(&request);
        // Should have alternatives unless only one model matches
        let model_count = router.list_models().len();
        if model_count > 1 {
            assert!(!decision.alternatives.is_empty() || decision.confidence > 0.8);
        }
    }
}
