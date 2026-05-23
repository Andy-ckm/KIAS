//! PolicyEngine trait definition

use async_trait::async_trait;

use super::rule::PolicyRule;

/// Result of a policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvaluationResult {
    /// Whether access should be allowed
    pub allowed: bool,
    /// ID of the matched rule (if any)
    pub matched_rule_id: Option<String>,
    /// Reason for the decision
    pub reason: String,
}

/// PolicyEngine trait for evaluating policies
#[async_trait]
pub trait PolicyEngine: Send + Sync {
    /// Evaluates a request against all applicable policies
    async fn evaluate(
        &self,
        resource: &str,
        action: &str,
        context: std::collections::HashMap<String, String>,
    ) -> PolicyEvaluationResult;

    /// Adds a policy rule to the engine
    async fn add_rule(&self, rule: PolicyRule) -> Result<(), String>;

    /// Removes a policy rule by ID
    async fn remove_rule(&self, rule_id: &str) -> Result<(), String>;

    /// Lists all policy rules
    async fn list_rules(&self) -> Vec<PolicyRule>;
}
