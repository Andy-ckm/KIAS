//! Condition matching for policy rules

use serde::{Deserialize, Serialize};

/// Comparison operators for conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    GreaterThan,
    LessThan,
    In,
    NotIn,
}

/// A condition that can be evaluated against a context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Field name to check
    pub field: String,
    /// Operator to use for comparison
    pub operator: ConditionOperator,
    /// Expected value (for single-value operators)
    pub value: String,
    /// Additional values (for In/NotIn operators)
    #[serde(default)]
    pub values: Vec<String>,
}

impl Condition {
    /// Creates a new string condition
    pub fn new_string(field: impl Into<String>, operator: ConditionOperator, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            operator,
            value: value.into(),
            values: Vec::new(),
        }
    }

    /// Creates a new condition with multiple values (for In/NotIn)
    pub fn new_multiple(field: impl Into<String>, operator: ConditionOperator, values: Vec<String>) -> Self {
        Self {
            field: field.into(),
            operator: operator,
            value: String::new(),
            values,
        }
    }

    /// Evaluates the condition against the given context
    pub fn evaluate(&self, context: &std::collections::HashMap<String, String>) -> bool {
        let Some(actual) = context.get(&self.field) else {
            return false;
        };

        match self.operator {
            ConditionOperator::Equals => actual == &self.value,
            ConditionOperator::NotEquals => actual != &self.value,
            ConditionOperator::Contains => actual.contains(&self.value),
            ConditionOperator::StartsWith => actual.starts_with(&self.value),
            ConditionOperator::EndsWith => actual.ends_with(&self.value),
            ConditionOperator::GreaterThan => {
                if let (Ok(a), Ok(v)) = (actual.parse::<i64>(), self.value.parse::<i64>()) {
                    a > v
                } else {
                    false
                }
            }
            ConditionOperator::LessThan => {
                if let (Ok(a), Ok(v)) = (actual.parse::<i64>(), self.value.parse::<i64>()) {
                    a < v
                } else {
                    false
                }
            }
            ConditionOperator::In => self.values.contains(actual),
            ConditionOperator::NotIn => !self.values.contains(actual),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_context() -> std::collections::HashMap<String, String> {
        let mut ctx = std::collections::HashMap::new();
        ctx.insert("role".to_string(), "admin".to_string());
        ctx.insert("level".to_string(), "5".to_string());
        ctx.insert("tags".to_string(), "prod,critical".to_string());
        ctx
    }

    #[test]
    fn test_equals_operator() {
        let ctx = create_context();
        let cond = Condition::new_string("role", ConditionOperator::Equals, "admin");
        assert!(cond.evaluate(&ctx));

        let cond2 = Condition::new_string("role", ConditionOperator::Equals, "user");
        assert!(!cond2.evaluate(&ctx));
    }

    #[test]
    fn test_not_equals_operator() {
        let ctx = create_context();
        let cond = Condition::new_string("role", ConditionOperator::NotEquals, "user");
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn test_contains_operator() {
        let ctx = create_context();
        let cond = Condition::new_string("tags", ConditionOperator::Contains, "prod");
        assert!(cond.evaluate(&ctx));

        let cond2 = Condition::new_string("tags", ConditionOperator::Contains, "dev");
        assert!(!cond2.evaluate(&ctx));
    }

    #[test]
    fn test_greater_than_operator() {
        let ctx = create_context();
        let cond = Condition::new_string("level", ConditionOperator::GreaterThan, "3");
        assert!(cond.evaluate(&ctx));

        let cond2 = Condition::new_string("level", ConditionOperator::GreaterThan, "5");
        assert!(!cond2.evaluate(&ctx));
    }

    #[test]
    fn test_in_operator() {
        let ctx = create_context();
        let cond = Condition::new_multiple("role", ConditionOperator::In, vec!["admin".to_string(), "superuser".to_string()]);
        assert!(cond.evaluate(&ctx));

        let cond2 = Condition::new_multiple("role", ConditionOperator::In, vec!["user".to_string(), "guest".to_string()]);
        assert!(!cond2.evaluate(&ctx));
    }

    #[test]
    fn test_missing_field() {
        let ctx = create_context();
        let cond = Condition::new_string("missing", ConditionOperator::Equals, "value");
        assert!(!cond.evaluate(&ctx));
    }
}
