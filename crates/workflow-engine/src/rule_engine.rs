//! Rule Engine — declarative rule evaluation framework.
//!
//! Provides a flexible rule engine for:
//! - Event-condition-action (ECA) rules
//! - Threshold-based alerting
//! - Policy enforcement
//! - Agent behavior rules
//!
//! Inspired by:
//! - EMQX Rule Engine (SQL-like rules for message routing)
//! - Drools rule engine pattern
//! - AWS EventBridge rules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rule trigger type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerType {
    /// Triggered on every event.
    Event,
    /// Triggered on schedule (cron expression).
    Schedule(String),
    /// Triggered when threshold is exceeded.
    Threshold,
    /// Triggered manually.
    Manual,
}

/// Comparison operators for conditions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOp {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    Contains,
    NotContains,
    Regex,
    In,
}

/// A single condition in a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ComparisonOp,
    pub value: serde_json::Value,
}

/// Action to execute when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    pub action_type: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// A complete rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger: TriggerType,
    pub conditions: Vec<Condition>,
    /// All conditions must match (AND logic).
    pub actions: Vec<RuleAction>,
    /// Priority (higher = evaluated first).
    pub priority: i32,
    /// Tags for organizing rules.
    pub tags: Vec<String>,
    /// Number of times this rule has fired.
    pub fire_count: u64,
    /// Last fired timestamp (ms).
    pub last_fired_ms: Option<u64>,
}

/// Result of rule evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResult {
    pub rule_id: String,
    pub matched: bool,
    pub actions_executed: Vec<String>,
    pub error: Option<String>,
    pub evaluated_at_ms: u64,
}

/// Context for rule evaluation — provides the data to evaluate conditions against.
#[derive(Debug, Clone)]
pub struct RuleContext {
    pub data: HashMap<String, serde_json::Value>,
}

impl RuleContext {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn with(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    pub fn get(&self, field: &str) -> Option<&serde_json::Value> {
        self.data.get(field)
    }
}

impl Default for RuleContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Rule engine — evaluates rules against contexts.
pub struct RuleEngine {
    rules: Vec<Rule>,
    /// Total evaluations performed.
    total_evaluations: u64,
    /// Total rule fires.
    total_fires: u64,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            total_evaluations: 0,
            total_fires: 0,
        }
    }

    /// Add a rule to the engine.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        // Sort by priority (highest first)
        self.rules.sort_by_key(|b| std::cmp::Reverse(b.priority));
    }

    /// Remove a rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) -> Result<(), String> {
        let idx = self
            .rules
            .iter()
            .position(|r| r.rule_id == rule_id)
            .ok_or_else(|| format!("Rule '{}' not found", rule_id))?;
        self.rules.remove(idx);
        Ok(())
    }

    /// Enable/disable a rule.
    pub fn set_enabled(&mut self, rule_id: &str, enabled: bool) -> Result<(), String> {
        let rule = self
            .rules
            .iter_mut()
            .find(|r| r.rule_id == rule_id)
            .ok_or_else(|| format!("Rule '{}' not found", rule_id))?;
        rule.enabled = enabled;
        Ok(())
    }

    /// Evaluate all enabled rules against a context.
    pub fn evaluate(&mut self, ctx: &RuleContext) -> Vec<RuleResult> {
        self.total_evaluations += 1;
        let now = now_ms();
        let mut results = Vec::new();

        for rule in &mut self.rules {
            if !rule.enabled {
                continue;
            }

            if !matches!(rule.trigger, TriggerType::Event | TriggerType::Threshold) {
                continue;
            }

            let conditions = rule.conditions.clone();
            let matched = evaluate_conditions(&conditions, ctx);

            if matched {
                rule.fire_count += 1;
                rule.last_fired_ms = Some(now);
                self.total_fires += 1;

                let action_names: Vec<String> =
                    rule.actions.iter().map(|a| a.action_type.clone()).collect();

                results.push(RuleResult {
                    rule_id: rule.rule_id.clone(),
                    matched: true,
                    actions_executed: action_names,
                    error: None,
                    evaluated_at_ms: now,
                });
            }
        }

        results
    }

    /// Evaluate a single rule by ID.
    pub fn evaluate_rule(&self, rule_id: &str, ctx: &RuleContext) -> Result<RuleResult, String> {
        let rule = self
            .rules
            .iter()
            .find(|r| r.rule_id == rule_id)
            .ok_or_else(|| format!("Rule '{}' not found", rule_id))?;

        if !rule.enabled {
            return Ok(RuleResult {
                rule_id: rule_id.to_string(),
                matched: false,
                actions_executed: Vec::new(),
                error: Some("Rule is disabled".to_string()),
                evaluated_at_ms: now_ms(),
            });
        }

        let matched = evaluate_conditions(&rule.conditions, ctx);

        Ok(RuleResult {
            rule_id: rule_id.to_string(),
            matched,
            actions_executed: if matched {
                rule.actions.iter().map(|a| a.action_type.clone()).collect()
            } else {
                Vec::new()
            },
            error: None,
            evaluated_at_ms: now_ms(),
        })
    }

    /// Get all rules.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Get engine statistics.
    pub fn stats(&self) -> RuleEngineStats {
        RuleEngineStats {
            total_rules: self.rules.len(),
            enabled_rules: self.rules.iter().filter(|r| r.enabled).count(),
            total_evaluations: self.total_evaluations,
            total_fires: self.total_fires,
        }
    }
}

fn evaluate_conditions(conditions: &[Condition], ctx: &RuleContext) -> bool {
    conditions.iter().all(|cond| evaluate_condition(cond, ctx))
}

fn evaluate_condition(cond: &Condition, ctx: &RuleContext) -> bool {
    let field_value = match ctx.get(&cond.field) {
        Some(v) => v,
        None => return false,
    };

    match &cond.operator {
        ComparisonOp::Equals => field_value == &cond.value,
        ComparisonOp::NotEquals => field_value != &cond.value,
        ComparisonOp::GreaterThan => compare_numeric(field_value, &cond.value, |a, b| a > b),
        ComparisonOp::LessThan => compare_numeric(field_value, &cond.value, |a, b| a < b),
        ComparisonOp::GreaterOrEqual => compare_numeric(field_value, &cond.value, |a, b| a >= b),
        ComparisonOp::LessOrEqual => compare_numeric(field_value, &cond.value, |a, b| a <= b),
        ComparisonOp::Contains => match (field_value.as_str(), cond.value.as_str()) {
            (Some(haystack), Some(needle)) => haystack.contains(needle),
            _ => false,
        },
        ComparisonOp::NotContains => match (field_value.as_str(), cond.value.as_str()) {
            (Some(haystack), Some(needle)) => !haystack.contains(needle),
            _ => true,
        },
        ComparisonOp::Regex => match (field_value.as_str(), cond.value.as_str()) {
            (Some(text), Some(pattern)) => regex_match(text, pattern),
            _ => false,
        },
        ComparisonOp::In => {
            if let Some(arr) = cond.value.as_array() {
                arr.contains(field_value)
            } else {
                false
            }
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn compare_numeric(
    a: &serde_json::Value,
    b: &serde_json::Value,
    op: impl Fn(f64, f64) -> bool,
) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => op(a, b),
        _ => false,
    }
}

fn regex_match(text: &str, pattern: &str) -> bool {
    // Simple substring match as regex fallback (avoids regex crate dependency)
    if pattern.starts_with("^") && pattern.ends_with("$") {
        let inner = &pattern[1..pattern.len() - 1];
        text == inner
    } else {
        text.contains(pattern)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEngineStats {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub total_evaluations: u64,
    pub total_fires: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rule() -> Rule {
        Rule {
            rule_id: "r1".to_string(),
            name: "High CPU Alert".to_string(),
            description: "Alert when CPU > 90%".to_string(),
            enabled: true,
            trigger: TriggerType::Event,
            conditions: vec![Condition {
                field: "cpu".to_string(),
                operator: ComparisonOp::GreaterThan,
                value: serde_json::json!(90),
            }],
            actions: vec![RuleAction {
                action_type: "alert".to_string(),
                params: HashMap::new(),
            }],
            priority: 10,
            tags: vec!["monitoring".to_string()],
            fire_count: 0,
            last_fired_ms: None,
        }
    }

    #[test]
    fn test_rule_matches() {
        let mut engine = RuleEngine::new();
        engine.add_rule(sample_rule());

        let ctx = RuleContext::new().with("cpu", serde_json::json!(95));
        let results = engine.evaluate(&ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
    }

    #[test]
    fn test_rule_no_match() {
        let mut engine = RuleEngine::new();
        engine.add_rule(sample_rule());

        let ctx = RuleContext::new().with("cpu", serde_json::json!(50));
        let results = engine.evaluate(&ctx);
        assert!(results.is_empty());
    }

    #[test]
    fn test_multiple_conditions_and() {
        let mut rule = sample_rule();
        rule.conditions.push(Condition {
            field: "region".to_string(),
            operator: ComparisonOp::Equals,
            value: serde_json::json!("us-east"),
        });
        let mut engine = RuleEngine::new();
        engine.add_rule(rule);

        // Both match
        let ctx = RuleContext::new()
            .with("cpu", serde_json::json!(95))
            .with("region", serde_json::json!("us-east"));
        assert_eq!(engine.evaluate(&ctx).len(), 1);

        // Only one matches
        let ctx = RuleContext::new()
            .with("cpu", serde_json::json!(95))
            .with("region", serde_json::json!("eu-west"));
        assert!(engine.evaluate(&ctx).is_empty());
    }

    #[test]
    fn test_contains_operator() {
        let mut rule = sample_rule();
        rule.conditions = vec![Condition {
            field: "message".to_string(),
            operator: ComparisonOp::Contains,
            value: serde_json::json!("error"),
        }];
        let mut engine = RuleEngine::new();
        engine.add_rule(rule);

        let ctx = RuleContext::new().with("message", serde_json::json!("connection error timeout"));
        assert_eq!(engine.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn test_in_operator() {
        let mut rule = sample_rule();
        rule.conditions = vec![Condition {
            field: "status".to_string(),
            operator: ComparisonOp::In,
            value: serde_json::json!(["critical", "high"]),
        }];
        let mut engine = RuleEngine::new();
        engine.add_rule(rule);

        let ctx = RuleContext::new().with("status", serde_json::json!("critical"));
        assert_eq!(engine.evaluate(&ctx).len(), 1);

        let ctx = RuleContext::new().with("status", serde_json::json!("low"));
        assert!(engine.evaluate(&ctx).is_empty());
    }

    #[test]
    fn test_disabled_rule_skipped() {
        let mut rule = sample_rule();
        rule.enabled = false;
        let mut engine = RuleEngine::new();
        engine.add_rule(rule);

        let ctx = RuleContext::new().with("cpu", serde_json::json!(95));
        assert!(engine.evaluate(&ctx).is_empty());
    }

    #[test]
    fn test_priority_order() {
        let mut engine = RuleEngine::new();
        let mut low = sample_rule();
        low.rule_id = "low".to_string();
        low.priority = 1;
        let mut high = sample_rule();
        high.rule_id = "high".to_string();
        high.priority = 100;
        engine.add_rule(low);
        engine.add_rule(high);

        assert_eq!(engine.rules()[0].rule_id, "high");
    }

    #[test]
    fn test_fire_count() {
        let mut engine = RuleEngine::new();
        engine.add_rule(sample_rule());

        let ctx = RuleContext::new().with("cpu", serde_json::json!(95));
        engine.evaluate(&ctx);
        engine.evaluate(&ctx);

        assert_eq!(engine.rules()[0].fire_count, 2);
        assert_eq!(engine.stats().total_fires, 2);
    }

    #[test]
    fn test_evaluate_single_rule() {
        let mut engine = RuleEngine::new();
        engine.add_rule(sample_rule());

        let ctx = RuleContext::new().with("cpu", serde_json::json!(95));
        let result = engine.evaluate_rule("r1", &ctx).unwrap();
        assert!(result.matched);
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = RuleEngine::new();
        engine.add_rule(sample_rule());
        engine.remove_rule("r1").unwrap();
        assert!(engine.rules().is_empty());
    }
}
