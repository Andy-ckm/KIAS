use async_trait::async_trait;
use kias_common::KiasResult;
use super::goal::{Goal, EvaluationResult, Constraint};
use chrono::Utc;
use std::collections::HashMap;

/// 目标评估器（裁判分离设计）
///
/// 核心设计：
/// - 干活的归干活，验收的归验收
/// - 独立小模型判断是否满足条件
/// - 未满足返回理由，作为下一轮方向指引
#[async_trait]
pub trait GoalEvaluator: Send + Sync {
    /// 评估目标是否达成
    async fn evaluate(&self, goal: &Goal, round_output: &str) -> KiasResult<EvaluationResult>;
}

/// Default evaluator with improved verification logic
pub struct DefaultEvaluator {
    /// Custom verification functions keyed by verification_method
    verifiers: HashMap<String, fn(&str, &str) -> bool>,
}

impl Default for DefaultEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultEvaluator {
    pub fn new() -> Self {
        let mut verifiers: HashMap<String, fn(&str, &str) -> bool> = HashMap::new();

        // Built-in verification methods
        verifiers.insert("contains".to_string(), |output, expected| {
            output.contains(expected)
        });
        verifiers.insert("exact".to_string(), |output, expected| {
            output.trim() == expected.trim()
        });
        verifiers.insert("starts_with".to_string(), |output, expected| {
            output.starts_with(expected)
        });
        verifiers.insert("ends_with".to_string(), |output, expected| {
            output.ends_with(expected)
        });
        verifiers.insert("not_contains".to_string(), |output, expected| {
            !output.contains(expected)
        });
        verifiers.insert("line_count_gt".to_string(), |output, expected| {
            let count = output.lines().count();
            let threshold: usize = expected.parse().unwrap_or(0);
            count > threshold
        });
        verifiers.insert("word_count_gt".to_string(), |output, expected| {
            let count = output.split_whitespace().count();
            let threshold: usize = expected.parse().unwrap_or(0);
            count > threshold
        });

        Self { verifiers }
    }

    /// Register a custom verification function
    pub fn with_verifier(mut self, method: &str, verifier: fn(&str, &str) -> bool) -> Self {
        self.verifiers.insert(method.to_string(), verifier);
        self
    }

    /// Check a constraint using shell command
    async fn check_constraint(&self, constraint: &Constraint, output: &str) -> ConstraintCheck {
        // Parse check_method as a simple verification method
        // Format: "method:expected_value" or just "method"
        let parts: Vec<&str> = constraint.check_method.splitn(2, ':').collect();
        let method = parts[0];
        let expected = parts.get(1).unwrap_or(&"");

        if let Some(verifier) = self.verifiers.get(method) {
            let passed = verifier(output, expected);
            ConstraintCheck {
                constraint_name: constraint.name.clone(),
                passed,
                reason: if passed {
                    "Constraint satisfied".to_string()
                } else {
                    format!("Constraint '{}' violated: {}", constraint.name, constraint.description)
                },
            }
        } else {
            // Default: treat as contains check
            let passed = output.contains(&constraint.check_method);
            ConstraintCheck {
                constraint_name: constraint.name.clone(),
                passed,
                reason: if passed {
                    "Constraint satisfied".to_string()
                } else {
                    format!("Constraint '{}' check failed: {}", constraint.name, constraint.description)
                },
            }
        }
    }
}

/// Result of a constraint check
#[allow(dead_code)]
struct ConstraintCheck {
    constraint_name: String,
    passed: bool,
    reason: String,
}

#[async_trait]
impl GoalEvaluator for DefaultEvaluator {
    async fn evaluate(&self, goal: &Goal, round_output: &str) -> KiasResult<EvaluationResult> {
        tracing::info!(goal_id = %goal.id, "Evaluating goal");

        let mut all_conditions_met = true;
        let mut reasons = Vec::new();
        let mut suggestions = Vec::new();

        // Check each condition using the configured verification method
        for condition in &goal.conditions {
            let verifier = self.verifiers.get(&condition.verification_method)
                .copied()
                .unwrap_or_else(|| {
                    // Default: contains check
                    (|output: &str, expected: &str| output.contains(expected)) as fn(&str, &str) -> bool
                });

            let passed = verifier(round_output, &condition.expected_result);
            if !passed {
                all_conditions_met = false;
                reasons.push(format!(
                    "Condition '{}' not met: {} (expected '{}')",
                    condition.name, condition.description, condition.expected_result
                ));
                suggestions.push(format!("Try to achieve: {}", condition.expected_result));
            }
        }

        // Check constraints
        let mut all_constraints_met = true;
        for constraint in &goal.constraints {
            let check = self.check_constraint(constraint, round_output).await;
            if !check.passed {
                all_constraints_met = false;
                reasons.push(check.reason);
            }
        }

        let achieved = all_conditions_met && all_constraints_met;

        let result = EvaluationResult {
            round: 0, // 由调用者设置
            achieved,
            reason: if achieved {
                "All conditions and constraints met".to_string()
            } else {
                reasons.join("; ")
            },
            suggestions,
            evaluated_at: Utc::now(),
        };

        tracing::info!(achieved = %achieved, conditions_met = %all_conditions_met, constraints_met = %all_constraints_met, "Evaluation completed");
        Ok(result)
    }
}

/// LLM-backed evaluator that calls an external model for evaluation
pub struct LlmEvaluator {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl LlmEvaluator {
    pub fn new(api_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl GoalEvaluator for LlmEvaluator {
    async fn evaluate(&self, goal: &Goal, round_output: &str) -> KiasResult<EvaluationResult> {
        let conditions_desc: Vec<String> = goal.conditions.iter()
            .map(|c| format!("- {}: {} (expected: {})", c.name, c.description, c.expected_result))
            .collect();

        let constraints_desc: Vec<String> = goal.constraints.iter()
            .map(|c| format!("- {}: {} (check: {})", c.name, c.description, c.check_method))
            .collect();

        let prompt = format!(
            "You are a goal evaluator. Determine if the following output meets the goal.\n\n\
             Goal: {}\n\n\
             Conditions:\n{}\n\n\
             Constraints:\n{}\n\n\
             Output to evaluate:\n{}\n\n\
             Respond with JSON: {{\"achieved\": bool, \"reason\": string, \"suggestions\": [string]}}",
            goal.description,
            conditions_desc.join("\n"),
            constraints_desc.join("\n"),
            round_output
        );

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 512,
        });

        let response = self.client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(content) = parsed["choices"].as_array()
                            .and_then(|c| c.first())
                            .and_then(|c| c["message"]["content"].as_str())
                        {
                            // Try to parse JSON from the response
                            if let Ok(eval) = serde_json::from_str::<serde_json::Value>(content) {
                                let achieved = eval["achieved"].as_bool().unwrap_or(false);
                                let reason = eval["reason"].as_str().unwrap_or("No reason provided").to_string();
                                let suggestions: Vec<String> = eval["suggestions"].as_array()
                                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                    .unwrap_or_default();

                                return Ok(EvaluationResult {
                                    round: 0,
                                    achieved,
                                    reason,
                                    suggestions,
                                    evaluated_at: Utc::now(),
                                });
                            }
                        }
                    }
                }
                // Fallback to default evaluation
                Ok(EvaluationResult {
                    round: 0,
                    achieved: false,
                    reason: "LLM evaluation failed, falling back to default".to_string(),
                    suggestions: vec!["Retry evaluation".to_string()],
                    evaluated_at: Utc::now(),
                })
            }
            Err(e) => Ok(EvaluationResult {
                round: 0,
                achieved: false,
                reason: format!("LLM request failed: {}", e),
                suggestions: vec!["Check LLM API availability".to_string()],
                evaluated_at: Utc::now(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal::GoalStatus;

    #[test]
    fn test_goal_creation() {
        let goal = Goal::new("Write a hello world program");
        assert_eq!(goal.description, "Write a hello world program");
        assert!(goal.conditions.is_empty());
        assert!(goal.constraints.is_empty());
        assert_eq!(goal.max_rounds, Some(20));
        assert!(!goal.id.is_empty());
    }

    #[test]
    fn test_goal_add_condition() {
        let mut goal = Goal::new("test");
        goal.add_condition("compiles", "Code compiles", "contains", "exit code 0");
        assert_eq!(goal.conditions.len(), 1);
        assert_eq!(goal.conditions[0].name, "compiles");
    }

    #[test]
    fn test_goal_add_constraint() {
        let mut goal = Goal::new("test");
        goal.add_constraint("no-unsafe", "No unsafe code", "not_contains:unsafe");
        assert_eq!(goal.constraints.len(), 1);
    }

    #[test]
    fn test_goal_set_max_rounds() {
        let mut goal = Goal::new("test");
        goal.set_max_rounds(50);
        assert_eq!(goal.max_rounds, Some(50));
    }

    #[tokio::test]
    async fn test_default_evaluator_achieved() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("done", "it works", "contains", "works");

        let result = evaluator.evaluate(&goal, "it works and is done").await.unwrap();
        assert!(result.achieved);
    }

    #[tokio::test]
    async fn test_default_evaluator_not_achieved() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("done", "it works", "contains", "EXACT_EXPECTED");

        let result = evaluator.evaluate(&goal, "something else").await.unwrap();
        assert!(!result.achieved);
        assert!(!result.suggestions.is_empty());
    }

    #[test]
    fn test_goal_status_enum() {
        assert_ne!(GoalStatus::Pending, GoalStatus::InProgress);
        assert_ne!(GoalStatus::Achieved, GoalStatus::Failed);
    }

    #[tokio::test]
    async fn test_evaluator_exact_match() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("exact", "exact output", "exact", "hello world");

        let result = evaluator.evaluate(&goal, "hello world").await.unwrap();
        assert!(result.achieved);

        let result = evaluator.evaluate(&goal, "hello world!").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_starts_with() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("prefix", "starts with hello", "starts_with", "hello");

        let result = evaluator.evaluate(&goal, "hello world").await.unwrap();
        assert!(result.achieved);

        let result = evaluator.evaluate(&goal, "world hello").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_ends_with() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("suffix", "ends with done", "ends_with", "done");

        let result = evaluator.evaluate(&goal, "work is done").await.unwrap();
        assert!(result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_not_contains() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("no-error", "no errors", "not_contains", "ERROR");

        let result = evaluator.evaluate(&goal, "everything is fine").await.unwrap();
        assert!(result.achieved);

        let result = evaluator.evaluate(&goal, "ERROR: something broke").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_line_count() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("multiline", "more than 3 lines", "line_count_gt", "3");

        let result = evaluator.evaluate(&goal, "line1\nline2\nline3\nline4").await.unwrap();
        assert!(result.achieved);

        let result = evaluator.evaluate(&goal, "line1\nline2").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_word_count() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("verbose", "more than 5 words", "word_count_gt", "5");

        let result = evaluator.evaluate(&goal, "one two three four five six").await.unwrap();
        assert!(result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_constraint_check() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("done", "output present", "contains", "result");
        goal.add_constraint("no-error", "no errors allowed", "not_contains:ERROR");

        // Both condition and constraint met
        let result = evaluator.evaluate(&goal, "result: success").await.unwrap();
        assert!(result.achieved);

        // Condition met but constraint violated
        let result = evaluator.evaluate(&goal, "result: ERROR occurred").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_multiple_conditions() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("has-output", "has output", "contains", "output");
        goal.add_condition("no-error", "no error", "not_contains", "ERROR");

        // Both met
        let result = evaluator.evaluate(&goal, "output: ok").await.unwrap();
        assert!(result.achieved);

        // Only first met
        let result = evaluator.evaluate(&goal, "output: ERROR").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_custom_verifier() {
        let evaluator = DefaultEvaluator::new()
            .with_verifier("is_numeric", |output, _| {
                output.trim().parse::<f64>().is_ok()
            });

        let mut goal = Goal::new("test");
        goal.add_condition("number", "output is a number", "is_numeric", "");

        let result = evaluator.evaluate(&goal, "42.5").await.unwrap();
        assert!(result.achieved);

        let result = evaluator.evaluate(&goal, "not a number").await.unwrap();
        assert!(!result.achieved);
    }

    #[tokio::test]
    async fn test_evaluator_unknown_method_defaults_to_contains() {
        let evaluator = DefaultEvaluator::new();
        let mut goal = Goal::new("test");
        goal.add_condition("test", "test condition", "unknown_method", "expected");

        let result = evaluator.evaluate(&goal, "contains expected text").await.unwrap();
        assert!(result.achieved);
    }
}
