//! Condition evaluators for GraphFlow conditional edges.
//!
//! Defines the `ConditionEvaluator` trait and built-in implementations
//! (RegexMatch, JsonPath, NumericCompare, CustomScript) that enable
//! declarative, composable conditional edge logic in graph execution.

use regex::Regex;
use std::fmt;

use crate::state::GraphState;

// ─── Comparison operators ────────────────────────────────────────────

/// Numeric comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl CompareOp {
    pub fn apply_f64(&self, lhs: f64, rhs: f64) -> bool {
        match self {
            CompareOp::Eq => (lhs - rhs).abs() < f64::EPSILON,
            CompareOp::Ne => (lhs - rhs).abs() >= f64::EPSILON,
            CompareOp::Gt => lhs > rhs,
            CompareOp::Ge => lhs >= rhs,
            CompareOp::Lt => lhs < rhs,
            CompareOp::Le => lhs <= rhs,
        }
    }
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CompareOp::Eq => "==",
            CompareOp::Ne => "!=",
            CompareOp::Gt => ">",
            CompareOp::Ge => ">=",
            CompareOp::Lt => "<",
            CompareOp::Le => "<=",
        };
        write!(f, "{}", s)
    }
}

// ─── ConditionEvaluator trait ────────────────────────────────────────

/// Trait for evaluating conditions against graph state.
///
/// Implementors must be `Send + Sync` for concurrent graph execution.
/// Evaluators are designed to be lightweight and composable.
pub trait ConditionEvaluator: Send + Sync {
    /// Human-readable name for this evaluator (used in logging/debugging).
    fn name(&self) -> &str;

    /// Evaluate the condition against the current graph state.
    /// Returns `true` if the condition is satisfied.
    fn evaluate(&self, state: &GraphState) -> bool;

    /// Box this evaluator for dynamic dispatch.
    fn into_boxed(self) -> Box<dyn ConditionEvaluator>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

impl fmt::Debug for dyn ConditionEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConditionEvaluator({})", self.name())
    }
}

// ─── Always / Never (combinators) ───────────────────────────────────

/// Always evaluates to `true`.
#[derive(Debug, Clone)]
pub struct Always;

impl ConditionEvaluator for Always {
    fn name(&self) -> &str {
        "Always"
    }
    fn evaluate(&self, _state: &GraphState) -> bool {
        true
    }
}

/// Always evaluates to `false`.
#[derive(Debug, Clone)]
pub struct Never;

impl ConditionEvaluator for Never {
    fn name(&self) -> &str {
        "Never"
    }
    fn evaluate(&self, _state: &GraphState) -> bool {
        false
    }
}

// ─── RegexMatch ──────────────────────────────────────────────────────

/// Evaluates to `true` when the string value in the specified channel
/// matches the given regular expression pattern.
///
/// If the channel is missing or cannot be deserialized to `String`,
/// the condition evaluates to `false`.
///
/// # Examples
/// ```
/// use kias_langgraph_engine::condition::{RegexMatch, ConditionEvaluator};
/// use kias_langgraph_engine::state::GraphState;
///
/// let cond = RegexMatch::new("status", r"^error|fail");
/// let mut state = GraphState::new();
/// state.set("status", "error_timeout");
/// assert!(cond.evaluate(&state));
/// ```
#[derive(Clone)]
pub struct RegexMatch {
    channel: String,
    pattern: String,
    regex: Regex,
}

impl RegexMatch {
    pub fn new(channel: &str, pattern: &str) -> Self {
        Self {
            channel: channel.to_string(),
            pattern: pattern.to_string(),
            regex: Regex::new(pattern).expect("Invalid regex pattern"),
        }
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

impl ConditionEvaluator for RegexMatch {
    fn name(&self) -> &str {
        "RegexMatch"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        match state.get::<String>(&self.channel) {
            Some(value) => self.regex.is_match(&value),
            None => false,
        }
    }
}

// ─── JsonPath ────────────────────────────────────────────────────────

/// Evaluates a simple JSON path expression against a channel value.
///
/// Supports:
/// - Top-level field: `"name"` → `value["name"]`
/// - Nested field: `"a.b.c"` → `value["a"]["b"]["c"]`
/// - Array index: `"items.0"` → `value["items"][0]`
///
/// The condition evaluates to `true` when the resolved value exists
/// and equals the expected `serde_json::Value`, OR when only checking
/// existence (`expected = None`).
///
/// # Examples
/// ```
/// use kias_langgraph_engine::condition::{JsonPath, ConditionEvaluator};
/// use kias_langgraph_engine::state::GraphState;
/// use serde_json::json;
///
/// let cond = JsonPath::new("data", "status", Some(json!("ready")));
/// let mut state = GraphState::new();
/// state.set("data", serde_json::json!({"status": "ready"}));
/// assert!(cond.evaluate(&state));
/// ```
#[derive(Clone)]
pub struct JsonPath {
    channel: String,
    path: String,
    expected: Option<serde_json::Value>,
}

impl JsonPath {
    /// Create a JsonPath evaluator that checks for a specific value.
    pub fn new(channel: &str, path: &str, expected: Option<serde_json::Value>) -> Self {
        Self {
            channel: channel.to_string(),
            path: path.to_string(),
            expected,
        }
    }

    /// Create a JsonPath evaluator that only checks existence (the path resolves to a non-null value).
    pub fn exists(channel: &str, path: &str) -> Self {
        Self::new(channel, path, None)
    }

    /// Resolve a dotted path against a JSON value.
    fn resolve<'a>(&self, value: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        let mut current = value;
        for segment in self.path.split('.') {
            current = if let Ok(index) = segment.parse::<usize>() {
                current.get(index)?
            } else {
                current.get(segment)?
            };
        }
        Some(current)
    }
}

impl ConditionEvaluator for JsonPath {
    fn name(&self) -> &str {
        "JsonPath"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        let value = match state.channels.get(&self.channel) {
            Some(v) => v,
            None => return false,
        };

        match self.resolve(value) {
            Some(resolved) => match &self.expected {
                Some(expected) => resolved == expected,
                None => !resolved.is_null(),
            },
            None => false,
        }
    }
}

// ─── NumericCompare ──────────────────────────────────────────────────

/// Compares a numeric channel value against a threshold.
///
/// Attempts to deserialize the channel as `f64`. If that fails,
/// falls back to `i64` → `f64` conversion. If both fail,
/// the condition evaluates to `false`.
///
/// # Examples
/// ```
/// use kias_langgraph_engine::condition::{NumericCompare, CompareOp, ConditionEvaluator};
/// use kias_langgraph_engine::state::GraphState;
///
/// let cond = NumericCompare::new("score", CompareOp::Ge, 80.0);
/// let mut state = GraphState::new();
/// state.set("score", 95i32);
/// assert!(cond.evaluate(&state));
/// ```
#[derive(Debug, Clone)]
pub struct NumericCompare {
    channel: String,
    op: CompareOp,
    threshold: f64,
}

impl NumericCompare {
    pub fn new(channel: &str, op: CompareOp, threshold: f64) -> Self {
        Self {
            channel: channel.to_string(),
            op,
            threshold,
        }
    }

    /// Try to extract a f64 from a serde_json::Value.
    fn extract_f64(value: &serde_json::Value) -> Option<f64> {
        if let Some(f) = value.as_f64() {
            return Some(f);
        }
        if let Some(i) = value.as_i64() {
            return Some(i as f64);
        }
        if let Some(u) = value.as_u64() {
            return Some(u as f64);
        }
        // Try deserializing as f64
        value.as_str().and_then(|s| s.parse::<f64>().ok())
    }
}

impl ConditionEvaluator for NumericCompare {
    fn name(&self) -> &str {
        "NumericCompare"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        let value = match state.channels.get(&self.channel) {
            Some(v) => v,
            None => return false,
        };

        match Self::extract_f64(value) {
            Some(numeric) => self.op.apply_f64(numeric, self.threshold),
            None => false,
        }
    }
}

// ─── HasChannel ──────────────────────────────────────────────────────

/// Evaluates to `true` when the specified channel exists in state.
#[derive(Debug, Clone)]
pub struct HasChannel {
    channel: String,
}

impl HasChannel {
    pub fn new(channel: &str) -> Self {
        Self {
            channel: channel.to_string(),
        }
    }
}

impl ConditionEvaluator for HasChannel {
    fn name(&self) -> &str {
        "HasChannel"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        state.has(&self.channel)
    }
}

// ─── ChannelEquals ───────────────────────────────────────────────────

/// Evaluates to `true` when the channel value equals the expected value.
#[derive(Clone)]
pub struct ChannelEquals {
    channel: String,
    expected: serde_json::Value,
}

impl ChannelEquals {
    pub fn new<T: serde::Serialize>(channel: &str, expected: T) -> Self {
        Self {
            channel: channel.to_string(),
            expected: serde_json::to_value(expected).expect("Failed to serialize expected value"),
        }
    }
}

impl ConditionEvaluator for ChannelEquals {
    fn name(&self) -> &str {
        "ChannelEquals"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        match state.channels.get(&self.channel) {
            Some(v) => v == &self.expected,
            None => false,
        }
    }
}

// ─── CustomScript ────────────────────────────────────────────────────

/// Wraps an arbitrary closure as a condition evaluator.
///
/// This is the escape hatch for conditions that don't fit into
/// the declarative evaluators above.
///
/// # Examples
/// ```
/// use kias_langgraph_engine::condition::{CustomScript, ConditionEvaluator};
/// use kias_langgraph_engine::state::GraphState;
///
/// let cond = CustomScript::new("multi_check", |state| {
///     state.has("input") && state.get::<i32>("retries").unwrap_or(0) < 3
/// });
/// ```
pub struct CustomScript {
    label: String,
    func: Box<dyn Fn(&GraphState) -> bool + Send + Sync>,
}

impl CustomScript {
    pub fn new<F>(label: &str, func: F) -> Self
    where
        F: Fn(&GraphState) -> bool + Send + Sync + 'static,
    {
        Self {
            label: label.to_string(),
            func: Box::new(func),
        }
    }
}

impl ConditionEvaluator for CustomScript {
    fn name(&self) -> &str {
        &self.label
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        (self.func)(state)
    }
}

// ─── Logical combinators ────────────────────────────────────────────

/// Logical AND of two evaluators.
pub struct AllOf {
    label: String,
    evaluators: Vec<Box<dyn ConditionEvaluator>>,
}

impl AllOf {
    pub fn new(label: &str, evaluators: Vec<Box<dyn ConditionEvaluator>>) -> Self {
        Self {
            label: label.to_string(),
            evaluators,
        }
    }
}

impl ConditionEvaluator for AllOf {
    fn name(&self) -> &str {
        &self.label
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        self.evaluators.iter().all(|e| e.evaluate(state))
    }
}

/// Logical OR of two evaluators.
pub struct AnyOf {
    label: String,
    evaluators: Vec<Box<dyn ConditionEvaluator>>,
}

impl AnyOf {
    pub fn new(label: &str, evaluators: Vec<Box<dyn ConditionEvaluator>>) -> Self {
        Self {
            label: label.to_string(),
            evaluators,
        }
    }
}

impl ConditionEvaluator for AnyOf {
    fn name(&self) -> &str {
        &self.label
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        self.evaluators.iter().any(|e| e.evaluate(state))
    }
}

/// Logical NOT of an evaluator.
pub struct Not {
    inner: Box<dyn ConditionEvaluator>,
}

impl Not {
    pub fn new(inner: Box<dyn ConditionEvaluator>) -> Self {
        Self { inner }
    }
}

impl ConditionEvaluator for Not {
    fn name(&self) -> &str {
        "Not"
    }

    fn evaluate(&self, state: &GraphState) -> bool {
        !self.inner.evaluate(state)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state_with(key: &str, value: impl serde::Serialize) -> GraphState {
        let mut s = GraphState::new();
        s.set(key, value);
        s
    }

    // ---- RegexMatch ----

    #[test]
    fn test_regex_match_basic() {
        let cond = RegexMatch::new("msg", r"^hello");
        assert!(cond.evaluate(&state_with("msg", "hello world")));
        assert!(!cond.evaluate(&state_with("msg", "goodbye")));
    }

    #[test]
    fn test_regex_match_missing_channel() {
        let cond = RegexMatch::new("missing", r".*");
        assert!(!cond.evaluate(&GraphState::new()));
    }

    #[test]
    fn test_regex_match_non_string_channel() {
        let cond = RegexMatch::new("num", r"42");
        // channel holds i32, not String → should return false
        assert!(!cond.evaluate(&state_with("num", 42i32)));
    }

    // ---- JsonPath ----

    #[test]
    fn test_json_path_nested_field() {
        let cond = JsonPath::new("data", "user.role", Some(json!("admin")));
        let state = state_with("data", json!({"user": {"role": "admin"}}));
        assert!(cond.evaluate(&state));
    }

    #[test]
    fn test_json_path_array_index() {
        let cond = JsonPath::new("data", "items.0", Some(json!("first")));
        let state = state_with("data", json!({"items": ["first", "second"]}));
        assert!(cond.evaluate(&state));
    }

    #[test]
    fn test_json_path_exists() {
        let cond = JsonPath::exists("data", "name");
        let state = state_with("data", json!({"name": "Alice"}));
        assert!(cond.evaluate(&state));

        let state2 = state_with("data", json!({"other": 1}));
        assert!(!cond.evaluate(&state2));
    }

    #[test]
    fn test_json_path_null_value_not_exists() {
        let cond = JsonPath::exists("data", "name");
        let state = state_with("data", json!({"name": null}));
        assert!(!cond.evaluate(&state));
    }

    // ---- NumericCompare ----

    #[test]
    fn test_numeric_compare_gt() {
        let cond = NumericCompare::new("score", CompareOp::Gt, 80.0);
        assert!(cond.evaluate(&state_with("score", 95i32)));
        assert!(!cond.evaluate(&state_with("score", 70i32)));
    }

    #[test]
    fn test_numeric_compare_eq() {
        let cond = NumericCompare::new("val", CompareOp::Eq, 42.0);
        assert!(cond.evaluate(&state_with("val", 42.0f64)));
        assert!(!cond.evaluate(&state_with("val", 43.0f64)));
    }

    #[test]
    fn test_numeric_compare_f64_channel() {
        let cond = NumericCompare::new("temp", CompareOp::Ge, 100.0);
        assert!(cond.evaluate(&state_with("temp", 100.5f64)));
    }

    #[test]
    fn test_numeric_compare_missing_channel() {
        let cond = NumericCompare::new("nope", CompareOp::Lt, 1.0);
        assert!(!cond.evaluate(&GraphState::new()));
    }

    // ---- HasChannel ----

    #[test]
    fn test_has_channel() {
        let cond = HasChannel::new("input");
        assert!(!cond.evaluate(&GraphState::new()));
        assert!(cond.evaluate(&state_with("input", "data")));
    }

    // ---- ChannelEquals ----

    #[test]
    fn test_channel_equals() {
        let cond = ChannelEquals::new("status", "ready");
        assert!(cond.evaluate(&state_with("status", "ready")));
        assert!(!cond.evaluate(&state_with("status", "busy")));
    }

    // ---- CustomScript ----

    #[test]
    fn test_custom_script() {
        let cond = CustomScript::new("complex", |state| state.has("a") && state.has("b"));
        let mut state = GraphState::new();
        state.set("a", 1);
        assert!(!cond.evaluate(&state));
        state.set("b", 2);
        assert!(cond.evaluate(&state));
    }

    // ---- Combinators ----

    #[test]
    fn test_all_of_combinator() {
        let cond = AllOf::new(
            "both",
            vec![
                HasChannel::new("x").into_boxed(),
                NumericCompare::new("x", CompareOp::Gt, 0.0).into_boxed(),
            ],
        );
        assert!(!cond.evaluate(&GraphState::new()));
        assert!(!cond.evaluate(&state_with("x", -1i32)));
        assert!(cond.evaluate(&state_with("x", 5i32)));
    }

    #[test]
    fn test_any_of_combinator() {
        let cond = AnyOf::new(
            "either",
            vec![
                ChannelEquals::new("status", "error").into_boxed(),
                ChannelEquals::new("status", "timeout").into_boxed(),
            ],
        );
        assert!(cond.evaluate(&state_with("status", "error")));
        assert!(cond.evaluate(&state_with("status", "timeout")));
        assert!(!cond.evaluate(&state_with("status", "ok")));
    }

    #[test]
    fn test_not_combinator() {
        let cond = Not::new(HasChannel::new("blocked").into_boxed());
        assert!(cond.evaluate(&GraphState::new()));
        assert!(!cond.evaluate(&state_with("blocked", true)));
    }

    // ---- CompareOp ----

    #[test]
    fn test_compare_op_all_variants() {
        assert!(CompareOp::Eq.apply_f64(1.0, 1.0));
        assert!(!CompareOp::Eq.apply_f64(1.0, 2.0));
        assert!(CompareOp::Ne.apply_f64(1.0, 2.0));
        assert!(!CompareOp::Ne.apply_f64(1.0, 1.0));
        assert!(CompareOp::Gt.apply_f64(2.0, 1.0));
        assert!(!CompareOp::Gt.apply_f64(1.0, 2.0));
        assert!(CompareOp::Ge.apply_f64(2.0, 1.0));
        assert!(CompareOp::Ge.apply_f64(1.0, 1.0));
        assert!(CompareOp::Lt.apply_f64(1.0, 2.0));
        assert!(!CompareOp::Lt.apply_f64(2.0, 1.0));
        assert!(CompareOp::Le.apply_f64(1.0, 2.0));
        assert!(CompareOp::Le.apply_f64(1.0, 1.0));
        assert!(!CompareOp::Le.apply_f64(2.0, 1.0));
    }

    #[test]
    fn test_always_never() {
        let state = GraphState::new();
        assert!(Always.evaluate(&state));
        assert!(!Never.evaluate(&state));
    }
}
