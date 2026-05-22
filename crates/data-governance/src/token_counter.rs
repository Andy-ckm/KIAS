
// token_counter.rs

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Kias common errors (stub; replace with actual crate import in production)
// ---------------------------------------------------------------------------
pub mod kias_common {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum KiasError {
        /// Budget exceeded for the request.
        InsufficientBudget {
            requested: f64,
            available: f64,
        },
        /// Token count exceeds allowed limit.
        TokenLimitExceeded {
            limit: u64,
            actual: u64,
        },
        /// Unknown or unsupported model identifier.
        UnknownModel(String),
        /// General internal error.
        Internal(String),
    }

    impl Display for KiasError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                KiasError::InsufficientBudget { requested, available } => {
                    write!(
                        f,
                        "Budget insufficient: requested {:.4}, available {:.4}",
                        requested, available
                    )
                }
                KiasError::TokenLimitExceeded { limit, actual } => {
                    write!(
                        f,
                        "Token limit exceeded: limit {} tokens, actual {} tokens",
                        limit, actual
                    )
                }
                KiasError::UnknownModel(name) => {
                    write!(f, "Unknown model: {}", name)
                }
                KiasError::Internal(msg) => {
                    write!(f, "Internal error: {}", msg)
                }
            }
        }
    }

    impl std::error::Error for KiasError {}
}

// ---------------------------------------------------------------------------
// Model pricing
// ---------------------------------------------------------------------------
/// Pricing information for a specific model.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Human‑readable model identifier (e.g. "gpt-4", "claude-3").
    pub model_name: String,
    /// Cost in USD (or arbitrary units) per 1,000 tokens.
    pub cost_per_thousand_tokens: f64,
}

impl ModelPricing {
    /// Build a new `ModelPricing`.
    pub fn new(model_name: impl Into<String>, cost_per_thousand_tokens: f64) -> Self {
        Self {
            model_name: model_name.into(),
            cost_per_thousand_tokens,
        }
    }

    /// Cost for a given number of tokens (rounded up to the nearest token).
    pub fn cost_for_tokens(&self, tokens: u64) -> f64 {
        let token_cost = self.cost_per_thousand_tokens / 1000.0;
        token_cost * tokens as f64
    }
}

// ---------------------------------------------------------------------------
// Budget policy
// ---------------------------------------------------------------------------
/// Policy that defines various budget constraints.
#[derive(Debug, Clone)]
pub struct BudgetPolicy {
    /// Maximum total amount of money (e.g., USD) that can be spent.
    pub max_total_cost: f64,
    /// Maximum number of tokens that may be sent in a single request.
    pub max_tokens_per_request: u64,
    /// Maximum number of tokens that may be received in a single response.
    pub max_tokens_per_response: u64,
    /// Optional cap on total tokens across all calls.
    pub max_total_tokens: Option<u64>,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            max_total_cost: f64::INFINITY,
            max_tokens_per_request: u64::MAX,
            max_tokens_per_response: u64::MAX,
            max_total_tokens: None,
        }
    }
}

/// Convenience builder for `BudgetPolicy`.
pub struct BudgetPolicyBuilder(BudgetPolicy);

impl BudgetPolicyBuilder {
    pub fn new() -> Self {
        Self(BudgetPolicy::default())
    }

    pub fn max_total_cost(mut self, cost: f64) -> Self {
        self.0.max_total_cost = cost;
        self
    }

    pub fn max_tokens_per_request(mut self, tokens: u64) -> Self {
        self.0.max_tokens_per_request = tokens;
        self
    }

    pub fn max_tokens_per_response(mut self, tokens: u64) -> Self {
        self.0.max_tokens_per_response = tokens;
        self
    }

    pub fn max_total_tokens(mut self, tokens: u64) -> Self {
        self.0.max_total_tokens = Some(tokens);
        self
    }

    pub fn build(self) -> BudgetPolicy {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Usage report
// ---------------------------------------------------------------------------
/// Summary of the current token usage and budget status.
#[derive(Debug, Clone, Default)]
pub struct UsageReport {
    pub request_tokens: u64,
    pub response_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub remaining_budget: f64,
    pub remaining_token_quota: Option<u64>,
    pub current_model: Option<String>,
    pub current_model_cost: f64,
}

impl Display for UsageReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Usage Report ===")?;
        writeln!(f, "Request tokens: {}", self.request_tokens)?;
        writeln!(f, "Response tokens: {}", self.response_tokens)?;
        writeln!(f, "Total tokens:   {}", self.total_tokens)?;
        writeln!(f, "Total cost:     {:.4}", self.total_cost)?;
        writeln!(f, "Remaining budget: {:.4}", self.remaining_budget)?;
        if let Some(q) = self.remaining_token_quota {
            writeln!(f, "Remaining token quota: {}", q)?;
        }
        if let Some(ref m) = self.current_model {
            writeln!(f, "Current model: {}", m)?;
            writeln!(f, "Current model cost: {:.4}", self.current_model_cost)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Token Counter
// ---------------------------------------------------------------------------
/// Central token counter that tracks usage per request/response,
/// computes cost based on model pricing, and enforces budget limits.
#[derive(Debug, Clone)]
pub struct TokenCounter {
    request_tokens: u64,
    response_tokens: u64,
    total_tokens: u64,
    total_cost: f64,
    budgets: BudgetPolicy,
    // Pricing map keyed by model name.
    pricing: HashMap<String, ModelPricing>,
    // Track the last used model for reporting.
    current_model: Option<String>,
    // Current cost of the last request+response pair.
    current_model_cost: f64,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self {
            request_tokens: 0,
            response_tokens: 0,
            total_tokens: 0,
            total_cost: 0.0,
            budgets: BudgetPolicy::default(),
            pricing: HashMap::new(),
            current_model: None,
            current_model_cost: 0.0,
        }
    }
}

impl TokenCounter {
    // -------------------------
    // Constructor / Builder
    // -------------------------

    /// Create a new `TokenCounter` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `TokenCounter` with an initial budget policy.
    pub fn with_budget_policy(budgets: BudgetPolicy) -> Self {
        Self {
            budgets,
            ..Default::default()
        }
    }

    /// Add a model pricing entry.
    pub fn add_model_pricing(&mut self, pricing: ModelPricing) -> &mut Self {
        self.pricing.insert(pricing.model_name.clone(), pricing);
        self
    }

    /// Chain‑friendly version of `add_model_pricing`.
    pub fn with_model_pricing(mut self, pricing: ModelPricing) -> Self {
        self.add_model_pricing(pricing);
        self
    }

    // -------------------------
    // Core counting methods
    // -------------------------

    /// Record a request with a given token count and model.
    ///
    /// # Errors
    ///
    /// Returns [`KiasError::TokenLimitExceeded`] if `tokens` exceed the
    /// per‑request limit defined in the budget policy.
    pub fn add_request(&mut self, model: &str, tokens: u64) -> Result<(), kias_common::KiasError> {
        // Validate that the model is known.
        if !self.pricing.contains_key(model) {
            return Err(kias_common::KiasError::UnknownModel(model.to_string()));
        }

        // Enforce per‑request token limit.
        if tokens > self.budgets.max_tokens_per_request {
            return Err(kias_common::KiasError::TokenLimitExceeded {
                limit: self.budgets.max_tokens_per_request,
                actual: tokens,
            });
        }

        self.request_tokens = tokens;
        self.current_model = Some(model.to_string());

        // Update totals.
        self.total_tokens = self.total_tokens.saturating_add(tokens);

        // Check total token quota, if any.
        if let Some(max) = self.budgets.max_total_tokens {
            if self.total_tokens > max {
                return Err(kias_common::KiasError::TokenLimitExceeded {
                    limit: max,
                    actual: self.total_tokens,
                });
            }
        }

        // Compute cost for this request portion and add to total.
        if let Some(pricing) = self.pricing.get(model) {
            let cost = pricing.cost_for_tokens(tokens);
            self.total_cost += cost;
            self.current_model_cost = cost; // only request part for now
        }

        // Final budget check after request cost is added.
        if self.total_cost > self.budgets.max_total_cost {
            return Err(kias_common::KiasError::InsufficientBudget {
                requested: self.total_cost,
                available: self.budgets.max_total_cost,
            });
        }

        Ok(())
    }

    /// Record a response with a given token count and the previously selected model.
    ///
    /// # Errors
    ///
    /// Returns [`KiasError::TokenLimitExceeded`] if `tokens` exceed the
    /// per‑response limit defined in the budget policy.
    pub fn add_response(&mut self, tokens: u64) -> Result<(), kias_common::KiasError> {
        // Validate that a model has been set via `add_request`.
        let model = self.current_model.clone().ok_or_else(|| {
            kias_common::KiasError::Internal("No model set for response".to_string())
        })?;

        // Enforce per‑response token limit.
        if tokens > self.budgets.max_tokens_per_response {
            return Err(kias_common::KiasError::TokenLimitExceeded {
                limit: self.budgets.max_tokens_per_response,
                actual: tokens,
            });
        }

        self.response_tokens = tokens;
        self.total_tokens = self.total_tokens.saturating_add(tokens);

        // Check total token quota.
        if let Some(max) = self.budgets.max_total_tokens {
            if self.total_tokens > max {
                return Err(kias_common::KiasError::TokenLimitExceeded {
                    limit: max,
                    actual: self.total_tokens,
                });
            }
        }

        // Add response cost.
        if let Some(pricing) = self.pricing.get(&model) {
            let cost = pricing.cost_for_tokens(tokens);
            self.total_cost += cost;
            self.current_model_cost += cost;
        }

        // Final budget check.
        if self.total_cost > self.budgets.max_total_cost {
            return Err(kias_common::KiasError::InsufficientBudget {
                requested: self.total_cost,
                available: self.budgets.max_total_cost,
            });
        }

        Ok(())
    }

    /// Convenience method to add a request and response at once.
    ///
    /// # Errors
    ///
    /// See [`TokenCounter::add_request`] and [`TokenCounter::add_response`].
    pub fn add_request_response(
        &mut self,
        model: &str,
        request_tokens: u64,
        response_tokens: u64,
    ) -> Result<(), kias_common::KiasError> {
        self.add_request(model, request_tokens)?;
        self.add_response(response_tokens)?;
        Ok(())
    }

    // -------------------------
    // Cost & budget queries
    // -------------------------

    /// Compute the cost for a given model and token count **without**
    /// altering the internal state.
    pub fn compute_cost(&self, model: &str, tokens: u64) -> Result<f64, kias_common::KiasError> {
        self.pricing
            .get(model)
            .map(|p| p.cost_for_tokens(tokens))
            .ok_or_else(|| kias_common::KiasError::UnknownModel(model.to_string()))
    }

    /// Remaining budget (max_total_cost - total_cost).
    pub fn remaining_budget(&self) -> f64 {
        self.budgets
            .max_total_cost
            .saturating_sub(self.total_cost)
    }

    /// Remaining token quota (max_total_tokens - total_tokens), if defined.
    pub fn remaining_token_quota(&self) -> Option<u64> {
        self.budgets
            .max_total_tokens
            .map(|max| max.saturating_sub(self.total_tokens))
    }

    /// Generate a full usage report.
    pub fn usage_report(&self) -> UsageReport {
        UsageReport {
            request_tokens: self.request_tokens,
            response_tokens: self.response_tokens,
            total_tokens: self.total_tokens,
            total_cost: self.total_cost,
            remaining_budget: self.remaining_budget(),
            remaining_token_quota: self.remaining_token_quota(),
            current_model: self.current_model.clone(),
            current_model_cost: self.current_model_cost,
        }
    }

    // -------------------------
    // Reset
    // -------------------------

    /// Reset the internal counters and cost, preserving budget policy
    /// and pricing entries.
    pub fn reset(&mut self) {
        self.request_tokens = 0;
        self.response_tokens = 0;
        self.total_tokens = 0;
        self.total_cost = 0.0;
        self.current_model = None;
        self.current_model_cost = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a simple pricing map.
    fn standard_pricing() -> HashMap<String, ModelPricing> {
        let mut map = HashMap::new();
        map.insert(
            "gpt-4".to_string(),
            ModelPricing::new("gpt-4", 0.03), // $0.03 per 1k tokens
        );
        map.insert(
            "gpt-3.5".to_string(),
            ModelPricing::new("gpt-3.5", 0.002), // $0.002 per 1k tokens
        );
        map
    }

    // Helper to create a TokenCounter with some budget.
    fn configured_counter() -> TokenCounter {
        let policy = BudgetPolicyBuilder::new()
            .max_total_cost(1.0) // $1 total budget
            .max_tokens_per_request(4096)
            .max_tokens_per_response(8192)
            .max_total_tokens(100_000)
            .build();

        let mut counter = TokenCounter::with_budget_policy(policy);
        for (_, p) in standard_pricing() {
            counter.add_model_pricing(p);
        }
        counter
    }

    // -----------------------------
    // Test 1: Basic creation
    // -----------------------------
    #[test]
    fn test_counter_creation() {
        let counter = TokenCounter::new();
        assert_eq!(counter.total_tokens, 0);
        assert_eq!(counter.total_cost, 0.0);
        assert!(counter.usage_report().current_model.is_none());
    }

    // -----------------------------
    // Test 2: Adding a request
    // -----------------------------
    #[test]
    fn test_add_request() {
        let mut counter = configured_counter();
        let result = counter.add_request("gpt-4", 1000);
        assert!(result.is_ok());
        assert_eq!(counter.request_tokens, 1000);
        // GPT-4 costs $0.03 per 1k tokens, so 1000 tokens = $0.03
        assert!((counter.total_cost - 0.03).abs() < 1e-9);
    }

    // -----------------------------
    // Test 3: Adding a response
    // -----------------------------
    #[test]
    fn test_add_response() {
        let mut counter = configured_counter();
        // Simulate a request then response.
        counter.add_request("gpt-4", 500).unwrap();
        let resp_result = counter.add_response(1500);
        assert!(resp_result.is_ok());
        assert_eq!(counter.response_tokens, 1500);
        // 500 + 1500 = 2000 tokens total at $0.03/1k = $0.06
        assert!((counter.total_cost - 0.06).abs() < 1e-9);
    }

    // -----------------------------
    // Test 4: Cost calculation for different models
    // -----------------------------
    #[test]
    fn test_cost_calculation() {
        let mut counter = configured_counter();
        // Compute cost without adding to state.
        let cost_gpt4 = counter.compute_cost("gpt-4", 2000).unwrap();
        assert!((cost_gpt4 - 0.06).abs() < 1e-9);

        let cost_gpt35 = counter.compute_cost("gpt-3.5", 2000).unwrap();
        assert!((cost_gpt35 - 0.004).abs() < 1e-9);

        // Unknown model should fail.
        let unknown = counter.compute_cost("unknown-model", 100);
        assert!(unknown.is_err());
    }

    // -----------------------------
    // Test 5: Budget enforcement (exceed total cost)
    // -----------------------------
    #[test]
    fn test_budget_exceeded() {
        let policy = BudgetPolicyBuilder::new()
            .max_total_cost(0.05) // $0.05 max
            .max_tokens_per_request(u64::MAX)
            .max_tokens_per_response(u64::MAX)
            .build();

        let mut counter = TokenCounter::with_budget_policy(policy);
        counter.add_model_pricing(ModelPricing::new("gpt-4", 0.03));

        // First request: 1000 tokens = $0.03 -> ok
        let r1 = counter.add_request("gpt-4", 1000);
        assert!(r1.is_ok());

        // Second request: another 1000 tokens would add $0.03 -> total $0.06 > $0.05 => Err
        let r2 = counter.add_request("gpt-4", 1000);
        assert!(r2.is_err());
        assert!(matches!(
            r2.unwrap_err(),
            kias_common::KiasError::InsufficientBudget { .. }
        ));
    }

    // -----------------------------
    // Test 6: Token limit per request exceeded
    // -----------------------------
    #[test]
    fn test_request_token_limit_exceeded() {
        let mut counter = configured_counter();
        let result = counter.add_request("gpt-4", 10_000); // limit is 4096
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            kias_common::KiasError::TokenLimitExceeded { limit: 4096, actual: 10_000 }
        ));
    }

    // -----------------------------
    // Test 7: Token limit per response exceeded
    // -----------------------------
    #[test]
    fn test_response_token_limit_exceeded() {
        let mut counter = configured_counter();
        // Must set a request first.
        counter.add_request("gpt-4", 100).unwrap();
        let result = counter.add_response(20_000); // limit is 8192
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            kias_common::KiasError::TokenLimitExceeded { limit: 8192, actual: 20_000 }
        ));
    }

    // -----------------------------
    // Test 8: Resetting the counter
    // -----------------------------
    #[test]
    fn test_reset_counter() {
        let mut counter = configured_counter();
        counter.add_request("gpt-4", 1000).unwrap();
        counter.add_response(500).unwrap();
        assert_eq!(counter.total_tokens, 1500);
        assert!(counter.total_cost > 0.0);

        counter.reset();
        assert_eq!(counter.total_tokens, 0);
        assert_eq!(counter.total_cost, 0.0);
        assert!(counter.usage_report().current_model.is_none());
    }

    // -----------------------------
    // Test 9: Adding request & response in one call
    // -----------------------------
    #[test]
    fn test_request_response_combined() {
        let mut counter = configured_counter();
        let result = counter.add_request_response("gpt-4", 200, 800);
        assert!(result.is_ok());
        assert_eq!(counter.request_tokens, 200);
        assert_eq!(counter.response_tokens, 800);
        // Total 1000 tokens at $0.03/1k = $0.03
        assert!((counter.total_cost - 0.03).abs() < 1e-9);
    }

    // -----------------------------
    // Test 10: Remaining budget & token quota queries
    // -----------------------------
    #[test]
    fn test_remaining_queries() {
        let mut counter = configured_counter();
        // Initially remaining budget = 1.0 - 0.0 = 1.0
        assert!((counter.remaining_budget() - 1.0).abs() < 1e-9);
        // Remaining token quota = 100_000 - 0 = 100_000
        assert_eq!(counter.remaining_token_quota(), Some(100_000));

        counter.add_request("gpt-4", 5000).unwrap(); // cost = $0.15
        assert!((counter.remaining_budget() - 0.85).abs() < 1e-9);
        // Tokens added = 5000, so remaining = 100_000 - 5000 = 95_000
        assert_eq!(counter.remaining_token_quota(), Some(95_000));
    }
}