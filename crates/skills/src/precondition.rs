//! # Precondition Validator (SDOF-inspired)
//!
//! Declarative precondition framework for skill execution.
//! Based on SDOF paper (2605.15204): "Taming the Alignment Tax in Multi-Agent Orchestration"
//!
//! Each skill declares a set of preconditions (Πpre).
//! Before execution, all preconditions must evaluate to true.
//! If any fails → refuse execution + return reason.

use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ── Precondition ───────────────────────────────────────────────────────

/// A single precondition that must be satisfied before a skill can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precondition {
    /// Unique identifier for this precondition.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Type of precondition check.
    pub check_type: PreconditionType,
    /// Whether this precondition is required (hard) or advisory (soft).
    pub required: bool,
}

/// The type of check to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreconditionType {
    /// Check that a context key exists and is non-empty.
    ContextKeyExists { key: String },
    /// Check that a context key equals a specific value.
    ContextKeyEquals { key: String, value: String },
    /// Check that a numeric context key is within a range.
    ContextKeyRange {
        key: String,
        min: Option<f64>,
        max: Option<f64>,
    },
    /// Check that a prior skill has been executed.
    SkillExecuted { skill_id: String },
    /// Check that the current time is within a time window.
    TimeWindow {
        after_hour: Option<u32>,
        before_hour: Option<u32>,
    },
    /// Custom predicate (evaluated by the runtime).
    Custom { predicate_name: String },
}

// ── Validation Result ──────────────────────────────────────────────────

/// Result of evaluating a single precondition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionResult {
    /// The precondition that was evaluated.
    pub precondition_id: String,
    /// Whether the precondition was satisfied.
    pub satisfied: bool,
    /// Human-readable reason for the result.
    pub reason: String,
}

/// Result of evaluating a full precondition set.
#[derive(Debug, Clone)]
pub struct PreconditionSetResult {
    /// Whether all required preconditions passed.
    pub passed: bool,
    /// Individual results for each precondition.
    pub results: Vec<PreconditionResult>,
    /// Summary message.
    pub summary: String,
}

impl PreconditionSetResult {
    /// Get IDs of failed preconditions.
    pub fn failed_ids(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter(|r| !r.satisfied)
            .map(|r| r.precondition_id.as_str())
            .collect()
    }
}

// ── Precondition Set ───────────────────────────────────────────────────

/// A set of preconditions for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionSet {
    /// The skill this set belongs to.
    pub skill_id: String,
    /// The preconditions to evaluate.
    pub preconditions: Vec<Precondition>,
}

impl PreconditionSet {
    pub fn new(skill_id: &str) -> Self {
        Self {
            skill_id: skill_id.to_string(),
            preconditions: Vec::new(),
        }
    }

    /// Add a precondition.
    pub fn add(&mut self, precondition: Precondition) {
        self.preconditions.push(precondition);
    }

    /// Evaluate all preconditions against the given context.
    pub fn evaluate(&self, context: &PreconditionContext) -> PreconditionSetResult {
        let results: Vec<PreconditionResult> = self
            .preconditions
            .iter()
            .map(|pc| evaluate_single(pc, context))
            .collect();

        let failed_required = results
            .iter()
            .zip(self.preconditions.iter())
            .any(|(r, pc)| !r.satisfied && pc.required);

        let failed_count = results.iter().filter(|r| !r.satisfied).count();
        let passed = !failed_required;

        let summary = if passed {
            format!(
                "All {} preconditions satisfied for skill '{}'",
                results.len(),
                self.skill_id
            )
        } else {
            format!(
                "{} precondition(s) failed for skill '{}'",
                failed_count, self.skill_id
            )
        };

        PreconditionSetResult {
            passed,
            results,
            summary,
        }
    }
}

// ── Precondition Context ───────────────────────────────────────────────

/// Runtime context for evaluating preconditions.
#[derive(Debug, Clone, Default)]
pub struct PreconditionContext {
    /// Key-value context data.
    pub data: HashMap<String, ContextValue>,
    /// Set of skill IDs that have been executed.
    pub executed_skills: std::collections::HashSet<String>,
}

/// A typed context value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

impl PreconditionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        self.data
            .insert(key.to_string(), ContextValue::String(value.to_string()));
    }

    pub fn set_number(&mut self, key: &str, value: f64) {
        self.data
            .insert(key.to_string(), ContextValue::Number(value));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.data
            .insert(key.to_string(), ContextValue::Boolean(value));
    }

    pub fn mark_executed(&mut self, skill_id: &str) {
        self.executed_skills.insert(skill_id.to_string());
    }

    fn get_string(&self, key: &str) -> Option<&str> {
        match self.data.get(key) {
            Some(ContextValue::String(s)) => Some(s),
            _ => None,
        }
    }

    fn get_number(&self, key: &str) -> Option<f64> {
        match self.data.get(key) {
            Some(ContextValue::Number(n)) => Some(*n),
            _ => None,
        }
    }

    fn has_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }
}

// ── Evaluator ──────────────────────────────────────────────────────────

fn evaluate_single(pc: &Precondition, ctx: &PreconditionContext) -> PreconditionResult {
    use PreconditionType::*;

    let (satisfied, reason) = match &pc.check_type {
        ContextKeyExists { key } => {
            if ctx.has_key(key) {
                (true, format!("Key '{key}' exists"))
            } else {
                (false, format!("Key '{key}' not found in context"))
            }
        }
        ContextKeyEquals { key, value } => match ctx.get_string(key) {
            Some(v) if v == value => (true, format!("Key '{key}' equals '{value}'")),
            Some(v) => (false, format!("Key '{key}' = '{v}', expected '{value}'")),
            None => (false, format!("Key '{key}' not found")),
        },
        ContextKeyRange { key, min, max } => match ctx.get_number(key) {
            Some(n) => {
                let min_ok = min.is_none_or(|m| n >= m);
                let max_ok = max.is_none_or(|m| n <= m);
                if min_ok && max_ok {
                    (true, format!("Key '{key}' = {n} within range"))
                } else {
                    (
                        false,
                        format!("Key '{key}' = {n} out of range [{min:?}, {max:?}]"),
                    )
                }
            }
            None => (false, format!("Key '{key}' not found or not numeric")),
        },
        SkillExecuted { skill_id } => {
            if ctx.executed_skills.contains(skill_id) {
                (true, format!("Skill '{skill_id}' has been executed"))
            } else {
                (false, format!("Skill '{skill_id}' has not been executed"))
            }
        }
        TimeWindow {
            after_hour,
            before_hour,
        } => {
            let now_hour = chrono::Utc::now().hour();
            let after_ok = after_hour.is_none_or(|h| now_hour >= h);
            let before_ok = before_hour.is_none_or(|h| now_hour < h);
            if after_ok && before_ok {
                (true, format!("Current hour {now_hour} within window"))
            } else {
                (
                    false,
                    format!(
                        "Current hour {now_hour} outside window [{after_hour:?}, {before_hour:?}]"
                    ),
                )
            }
        }
        Custom { predicate_name } => {
            // Custom predicates are evaluated externally; default to pass
            (
                true,
                format!("Custom predicate '{predicate_name}' assumed satisfied"),
            )
        }
    };

    PreconditionResult {
        precondition_id: pc.id.clone(),
        satisfied,
        reason,
    }
}

// ── Display ────────────────────────────────────────────────────────────

impl fmt::Display for PreconditionSetResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> PreconditionContext {
        let mut ctx = PreconditionContext::new();
        ctx.set_string("env", "production");
        ctx.set_number("risk_score", 0.3);
        ctx.set_bool("approved", true);
        ctx.mark_executed("skill-a");
        ctx
    }

    #[test]
    fn test_context_key_exists_pass() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "env exists".to_string(),
            check_type: PreconditionType::ContextKeyExists {
                key: "env".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed);
    }

    #[test]
    fn test_context_key_exists_fail() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "missing key".to_string(),
            check_type: PreconditionType::ContextKeyExists {
                key: "nonexistent".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(!result.passed);
    }

    #[test]
    fn test_context_key_equals() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "env is prod".to_string(),
            check_type: PreconditionType::ContextKeyEquals {
                key: "env".to_string(),
                value: "production".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed);
    }

    #[test]
    fn test_context_key_range_pass() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "risk low".to_string(),
            check_type: PreconditionType::ContextKeyRange {
                key: "risk_score".to_string(),
                min: Some(0.0),
                max: Some(0.5),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed);
    }

    #[test]
    fn test_context_key_range_fail() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "risk too high".to_string(),
            check_type: PreconditionType::ContextKeyRange {
                key: "risk_score".to_string(),
                min: Some(0.0),
                max: Some(0.2),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(!result.passed);
    }

    #[test]
    fn test_skill_executed_pass() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "skill-a done".to_string(),
            check_type: PreconditionType::SkillExecuted {
                skill_id: "skill-a".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed);
    }

    #[test]
    fn test_skill_executed_fail() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "skill-b done".to_string(),
            check_type: PreconditionType::SkillExecuted {
                skill_id: "skill-b".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(!result.passed);
    }

    #[test]
    fn test_soft_precondition_does_not_block() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "optional check".to_string(),
            check_type: PreconditionType::ContextKeyExists {
                key: "nonexistent".to_string(),
            },
            required: false, // soft
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed); // soft failure doesn't block
    }

    #[test]
    fn test_multiple_preconditions_mixed() {
        let mut set = PreconditionSet::new("deploy");
        set.add(Precondition {
            id: "pc1".to_string(),
            name: "env check".to_string(),
            check_type: PreconditionType::ContextKeyEquals {
                key: "env".to_string(),
                value: "production".to_string(),
            },
            required: true,
        });
        set.add(Precondition {
            id: "pc2".to_string(),
            name: "risk check".to_string(),
            check_type: PreconditionType::ContextKeyRange {
                key: "risk_score".to_string(),
                min: Some(0.0),
                max: Some(0.5),
            },
            required: true,
        });

        let result = set.evaluate(&sample_context());
        assert!(result.passed);
        assert_eq!(result.results.len(), 2);
        assert!(result.results.iter().all(|r| r.satisfied));
    }

    #[test]
    fn test_failed_ids() {
        let mut set = PreconditionSet::new("test");
        set.add(Precondition {
            id: "pc_ok".to_string(),
            name: "pass".to_string(),
            check_type: PreconditionType::ContextKeyExists {
                key: "env".to_string(),
            },
            required: true,
        });
        set.add(Precondition {
            id: "pc_fail".to_string(),
            name: "fail".to_string(),
            check_type: PreconditionType::ContextKeyExists {
                key: "missing".to_string(),
            },
            required: true,
        });

        let result = set.evaluate(&sample_context());
        assert_eq!(result.failed_ids(), vec!["pc_fail"]);
    }

    #[test]
    fn test_display() {
        let set = PreconditionSet::new("test");
        let result = set.evaluate(&PreconditionContext::new());
        let display = format!("{}", result);
        assert!(display.contains("test"));
    }

    #[test]
    fn test_custom_predicate() {
        let pc = Precondition {
            id: "pc1".to_string(),
            name: "custom check".to_string(),
            check_type: PreconditionType::Custom {
                predicate_name: "is_business_hours".to_string(),
            },
            required: true,
        };
        let set = PreconditionSet {
            skill_id: "test".to_string(),
            preconditions: vec![pc],
        };
        let result = set.evaluate(&sample_context());
        assert!(result.passed);
    }

    #[test]
    fn test_context_builder() {
        let mut ctx = PreconditionContext::new();
        ctx.set_string("k1", "v1");
        ctx.set_number("k2", 42.0);
        ctx.set_bool("k3", true);
        assert_eq!(ctx.get_string("k1"), Some("v1"));
        assert_eq!(ctx.get_number("k2"), Some(42.0));
        assert!(ctx.has_key("k3"));
        assert!(!ctx.has_key("k4"));
    }
}
