//! PolicyRule structure

use serde::{Deserialize, Serialize};

use super::condition::Condition;

/// Effect of a policy rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Allow,
    Deny,
}

/// PolicyRule defines a single policy rule with id, name, resource, effect, conditions, and version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Unique identifier for the rule
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Resource pattern this rule applies to (e.g., "agent:*", "node:worker-1")
    pub resource: String,
    /// Effect: Allow or Deny
    pub effect: Effect,
    /// Conditions that must be satisfied for the rule to apply
    pub conditions: Vec<Condition>,
    /// Version for optimistic concurrency control
    pub version: u64,
}

impl PolicyRule {
    /// Creates a new PolicyRule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        resource: impl Into<String>,
        effect: Effect,
        conditions: Vec<Condition>,
        version: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            resource: resource.into(),
            effect,
            conditions,
            version,
        }
    }

    /// Returns true if all conditions are satisfied
    pub fn evaluate_conditions(&self, context: &std::collections::HashMap<String, String>) -> bool {
        self.conditions.iter().all(|c| c.evaluate(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_rule() -> PolicyRule {
        PolicyRule::new(
            "rule-001",
            "Deny dangerous tools",
            "tool:*",
            Effect::Deny,
            vec![Condition::new_string("tool_name", ConditionOperator::NotEquals, "safe_tool".to_string())],
            1,
        )
    }

    #[test]
    fn test_policy_rule_creation() {
        let rule = create_test_rule();
        assert_eq!(rule.id, "rule-001");
        assert_eq!(rule.name, "Deny dangerous tools");
        assert_eq!(rule.resource, "tool:*");
        assert_eq!(rule.effect, Effect::Deny);
        assert_eq!(rule.version, 1);
    }

    #[test]
    fn test_evaluate_conditions_match() {
        let rule = create_test_rule();
        let mut ctx = HashMap::new();
        ctx.insert("tool_name".to_string(), "dangerous_tool".to_string());

        // condition: tool_name != "safe_tool" -> true (dangerous_tool != safe_tool)
        assert!(rule.evaluate_conditions(&ctx));
    }

    #[test]
    fn test_evaluate_conditions_no_match() {
        let rule = create_test_rule();
        let mut ctx = HashMap::new();
        ctx.insert("tool_name".to_string(), "safe_tool".to_string());

        // condition: tool_name != "safe_tool" -> false (safe_tool == safe_tool)
        assert!(!rule.evaluate_conditions(&ctx));
    }
}
