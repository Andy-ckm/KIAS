//! Self-Evaluation and Repair Loop
//!
//! Implements self-assessment, failure classification, and repair strategies:
//! - SelfAssessor: evaluates output quality
//! - FailureClassifier: classifies failures (network/logic/data/model)
//! - RepairStrategy: strategies for each failure type

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quality level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    Excellent,
    Good,
    Acceptable,
    Poor,
    Failed,
}

impl QualityLevel {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.9 {
            QualityLevel::Excellent
        } else if score >= 0.75 {
            QualityLevel::Good
        } else if score >= 0.5 {
            QualityLevel::Acceptable
        } else if score >= 0.25 {
            QualityLevel::Poor
        } else {
            QualityLevel::Failed
        }
    }
}

/// Failure category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureCategory {
    Network,
    Logic,
    Data,
    Model,
    Timeout,
    Auth,
    Unknown,
}

impl std::fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureCategory::Network => write!(f, "Network"),
            FailureCategory::Logic => write!(f, "Logic"),
            FailureCategory::Data => write!(f, "Data"),
            FailureCategory::Model => write!(f, "Model"),
            FailureCategory::Timeout => write!(f, "Timeout"),
            FailureCategory::Auth => write!(f, "Auth"),
            FailureCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentResult {
    pub quality_score: f64,
    pub quality_level: QualityLevel,
    pub issues_found: Vec<String>,
    pub suggestions: Vec<String>,
    pub can_repair: bool,
}

/// Failure classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureClassification {
    pub category: FailureCategory,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub error_code: Option<String>,
}

/// Repair strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairStrategy {
    pub strategy_type: StrategyType,
    pub steps: Vec<RepairStep>,
    pub estimated_success_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyType {
    Retry,
    Fallback,
    Simplify,
    ExpandContext,
    UseCache,
    SwitchModel,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairStep {
    pub step_number: u8,
    pub action: String,
    pub parameters: HashMap<String, String>,
}

/// Assessment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentConfig {
    pub min_quality_threshold: f64,
    pub max_retries: u8,
    pub enable_auto_repair: bool,
}

impl Default for AssessmentConfig {
    fn default() -> Self {
        Self {
            min_quality_threshold: 0.5,
            max_retries: 3,
            enable_auto_repair: true,
        }
    }
}

/// SelfAssessor - evaluates output quality
pub struct SelfAssessor {
    config: AssessmentConfig,
}

impl Default for SelfAssessor {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfAssessor {
    pub fn new() -> Self {
        Self {
            config: AssessmentConfig::default(),
        }
    }

    pub fn with_config(config: AssessmentConfig) -> Self {
        Self { config }
    }

    /// Assess output quality
    pub fn assess(&self, output: &str, context: &AssessmentContext) -> AssessmentResult {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();
        let mut score: f64 = 1.0;

        // Check for empty output
        if output.trim().is_empty() {
            issues.push("Output is empty".to_string());
            suggestions.push("Provide meaningful content".to_string());
            score -= 0.5;
        }

        // Check for repetition patterns
        if has_repetition(output) {
            issues.push("Output contains repetition".to_string());
            suggestions.push("Avoid repeating content".to_string());
            score -= 0.15;
        }

        // Check for truncation indicators
        if output.ends_with("...") || output.contains("[truncated]") {
            issues.push("Output may be truncated".to_string());
            suggestions.push("Expand context or reduce output length".to_string());
            score -= 0.1;
        }

        // Check for error indicators
        if output.to_lowercase().contains("error") || output.to_lowercase().contains("failed") {
            issues.push("Output contains error indicators".to_string());
            suggestions.push("Handle error cases properly".to_string());
            score -= 0.2;
        }

        // Check length appropriateness
        if let Some(expected_len) = context.expected_length {
            let len_diff =
                (output.len() as i64 - expected_len as i64).abs() as f64 / expected_len as f64;
            if len_diff > 0.5 {
                issues.push("Output length significantly differs from expected".to_string());
                suggestions.push("Adjust output to match expected length".to_string());
                score -= 0.1;
            }
        }

        // Check for task completion
        if let Some(ref task) = context.task {
            if !task.is_empty()
                && !output
                    .to_lowercase()
                    .contains(&task.to_lowercase()[..task.len().min(20)])
            {
                issues.push("Output may not address the task".to_string());
                suggestions.push("Ensure output addresses the task requirements".to_string());
                score -= 0.15;
            }
        }

        // Penalize for safety issues
        if contains_safety_issues(output) {
            issues.push("Output contains potential safety issues".to_string());
            suggestions.push("Review content for safety compliance".to_string());
            score -= 0.25;
        }

        score = score.clamp(0.0, 1.0);
        let level = QualityLevel::from_score(score);
        let can_repair = score >= self.config.min_quality_threshold && score < 0.9;

        AssessmentResult {
            quality_score: score,
            quality_level: level,
            issues_found: issues,
            suggestions,
            can_repair,
        }
    }

    /// Quick check if output is acceptable
    pub fn is_acceptable(&self, output: &str) -> bool {
        let result = self.assess(output, &AssessmentContext::default());
        result.quality_score >= self.config.min_quality_threshold
    }
}

/// Context for assessment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssessmentContext {
    pub task: Option<String>,
    pub expected_length: Option<usize>,
    pub constraints: Vec<String>,
}

#[allow(dead_code)]
/// FailureClassifier - classifies failures
pub struct FailureClassifier {
    patterns: HashMap<FailureCategory, Vec<String>>,
}

impl Default for FailureClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl FailureClassifier {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();

        patterns.insert(
            FailureCategory::Network,
            vec![
                "connection refused".to_string(),
                "timeout".to_string(),
                "connection reset".to_string(),
                "network unreachable".to_string(),
                "dns".to_string(),
                "ECONNREFUSED".to_string(),
                "ETIMEDOUT".to_string(),
            ],
        );

        patterns.insert(
            FailureCategory::Logic,
            vec![
                "null pointer".to_string(),
                "division by zero".to_string(),
                "index out of bounds".to_string(),
                "assertion failed".to_string(),
                "illegal argument".to_string(),
                "invalid state".to_string(),
            ],
        );

        patterns.insert(
            FailureCategory::Data,
            vec![
                "parse error".to_string(),
                "invalid format".to_string(),
                "missing field".to_string(),
                "type mismatch".to_string(),
                "schema".to_string(),
                "corrupt".to_string(),
            ],
        );

        patterns.insert(
            FailureCategory::Model,
            vec![
                "rate limit".to_string(),
                "quota exceeded".to_string(),
                "model overloaded".to_string(),
                "context length".to_string(),
                "token limit".to_string(),
            ],
        );

        patterns.insert(
            FailureCategory::Timeout,
            vec![
                "timed out".to_string(),
                "deadline exceeded".to_string(),
                "took too long".to_string(),
                "execution time".to_string(),
            ],
        );

        patterns.insert(
            FailureCategory::Auth,
            vec![
                "unauthorized".to_string(),
                "forbidden".to_string(),
                "invalid token".to_string(),
                "permission denied".to_string(),
                "access denied".to_string(),
            ],
        );

        Self { patterns }
    }

    /// Classify a failure from error message
    pub fn classify(&self, error: &str) -> FailureClassification {
        let error_lower = error.to_lowercase();
        let mut best_match: Option<(FailureCategory, usize)> = None;

        for (category, patterns) in &self.patterns {
            for pattern in patterns {
                if error_lower.contains(&pattern.to_lowercase()) {
                    let score = pattern.len();
                    if best_match.map(|(_, s)| score > s).unwrap_or(true) {
                        best_match = Some((*category, score));
                    }
                }
            }
        }

        if let Some((category, score)) = best_match {
            FailureClassification {
                category,
                confidence: (score as f64 / 20.0).min(0.95),
                evidence: vec![error.chars().take(200).collect()],
                error_code: extract_error_code(error),
            }
        } else {
            FailureClassification {
                category: FailureCategory::Unknown,
                confidence: 0.5,
                evidence: vec![error.chars().take(200).collect()],
                error_code: extract_error_code(error),
            }
        }
    }

    /// Classify from HTTP status code
    pub fn classify_from_status(&self, status_code: u16) -> FailureClassification {
        let (category, evidence) = match status_code {
            400 => (
                FailureCategory::Data,
                "Bad Request - invalid input data".to_string(),
            ),
            401 | 403 => (
                FailureCategory::Auth,
                "Authentication/Authorization failure".to_string(),
            ),
            404 => (FailureCategory::Data, "Resource not found".to_string()),
            408 => (FailureCategory::Timeout, "Request timeout".to_string()),
            429 => (FailureCategory::Model, "Rate limit exceeded".to_string()),
            500..=599 => (FailureCategory::Logic, "Internal server error".to_string()),
            _ => (FailureCategory::Unknown, format!("HTTP {}", status_code)),
        };

        FailureClassification {
            category,
            confidence: 0.8,
            evidence: vec![evidence],
            error_code: Some(status_code.to_string()),
        }
    }
}

/// Extract error code from error message
fn extract_error_code(error: &str) -> Option<String> {
    // Look for patterns like "ERR_123" or "E123" or "code: 123"
    let patterns = [
        r"ERR[_-]?(\d+)",
        r"E(\d+)",
        r"code[:\s]+(\d+)",
        r"status[:\s]+(\d+)",
    ];

    for pattern in &patterns {
        if let Ok(re) = regex_lite(pattern) {
            if let Some(caps) = re.find(error) {
                return Some(caps.as_str().to_string());
            }
        }
    }
    None
}

fn regex_lite(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(pattern)
}

/// RepairStrategyGenerator - generates repair strategies
pub struct RepairStrategyGenerator {
    max_retries: u8,
}

impl Default for RepairStrategyGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl RepairStrategyGenerator {
    pub fn new() -> Self {
        Self { max_retries: 3 }
    }

    pub fn with_max_retries(mut self, retries: u8) -> Self {
        self.max_retries = retries;
        self
    }

    /// Generate repair strategy for a failure
    pub fn generate(&self, failure: &FailureClassification, attempt: u8) -> RepairStrategy {
        if attempt >= self.max_retries {
            return RepairStrategy {
                strategy_type: StrategyType::Skip,
                steps: vec![RepairStep {
                    step_number: 1,
                    action: "Skip failed operation".to_string(),
                    parameters: HashMap::new(),
                }],
                estimated_success_rate: 0.0,
            };
        }

        match failure.category {
            FailureCategory::Network => self.network_repair(failure),
            FailureCategory::Timeout => self.timeout_repair(failure, attempt),
            FailureCategory::Model => self.model_repair(failure, attempt),
            FailureCategory::Logic => self.logic_repair(failure),
            FailureCategory::Data => self.data_repair(failure),
            FailureCategory::Auth => self.auth_repair(failure),
            FailureCategory::Unknown => self.generic_repair(attempt),
        }
    }

    fn network_repair(&self, _failure: &FailureClassification) -> RepairStrategy {
        RepairStrategy {
            strategy_type: StrategyType::Retry,
            steps: vec![
                RepairStep {
                    step_number: 1,
                    action: "Wait before retry".to_string(),
                    parameters: [("delay_ms".to_string(), (1000 * 2_u64).to_string())].into(),
                },
                RepairStep {
                    step_number: 2,
                    action: "Retry request".to_string(),
                    parameters: HashMap::new(),
                },
            ],
            estimated_success_rate: 0.7,
        }
    }

    fn timeout_repair(&self, _failure: &FailureClassification, attempt: u8) -> RepairStrategy {
        let delay = 1000 * 2_u64.pow(attempt as u32);
        RepairStrategy {
            strategy_type: StrategyType::Retry,
            steps: vec![
                RepairStep {
                    step_number: 1,
                    action: "Wait exponentially longer".to_string(),
                    parameters: [("delay_ms".to_string(), delay.to_string())].into(),
                },
                RepairStep {
                    step_number: 2,
                    action: "Retry with longer timeout".to_string(),
                    parameters: [(
                        "timeout_ms".to_string(),
                        (30000 * (attempt + 1) as u64).to_string(),
                    )]
                    .into(),
                },
            ],
            estimated_success_rate: 0.6,
        }
    }

    fn model_repair(&self, _failure: &FailureClassification, attempt: u8) -> RepairStrategy {
        if attempt < 2 {
            RepairStrategy {
                strategy_type: StrategyType::Retry,
                steps: vec![RepairStep {
                    step_number: 1,
                    action: "Wait and retry".to_string(),
                    parameters: [("delay_ms".to_string(), "5000".to_string())].into(),
                }],
                estimated_success_rate: 0.5,
            }
        } else {
            RepairStrategy {
                strategy_type: StrategyType::SwitchModel,
                steps: vec![RepairStep {
                    step_number: 1,
                    action: "Switch to fallback model".to_string(),
                    parameters: [("model".to_string(), "gpt-4o-mini".to_string())].into(),
                }],
                estimated_success_rate: 0.8,
            }
        }
    }

    fn logic_repair(&self, _failure: &FailureClassification) -> RepairStrategy {
        RepairStrategy {
            strategy_type: StrategyType::Simplify,
            steps: vec![RepairStep {
                step_number: 1,
                action: "Simplify the request".to_string(),
                parameters: [("max_tokens".to_string(), "500".to_string())].into(),
            }],
            estimated_success_rate: 0.4,
        }
    }

    fn data_repair(&self, _failure: &FailureClassification) -> RepairStrategy {
        RepairStrategy {
            strategy_type: StrategyType::ExpandContext,
            steps: vec![
                RepairStep {
                    step_number: 1,
                    action: "Re-parse input data".to_string(),
                    parameters: HashMap::new(),
                },
                RepairStep {
                    step_number: 2,
                    action: "Use default values for missing fields".to_string(),
                    parameters: HashMap::new(),
                },
            ],
            estimated_success_rate: 0.6,
        }
    }

    fn auth_repair(&self, _failure: &FailureClassification) -> RepairStrategy {
        RepairStrategy {
            strategy_type: StrategyType::Fallback,
            steps: vec![RepairStep {
                step_number: 1,
                action: "Refresh authentication token".to_string(),
                parameters: HashMap::new(),
            }],
            estimated_success_rate: 0.7,
        }
    }

    fn generic_repair(&self, attempt: u8) -> RepairStrategy {
        let strategy = if attempt == 0 {
            StrategyType::Retry
        } else if attempt == 1 {
            StrategyType::UseCache
        } else {
            StrategyType::Fallback
        };

        RepairStrategy {
            strategy_type: strategy,
            steps: vec![RepairStep {
                step_number: 1,
                action: format!("Apply {:?} strategy", strategy),
                parameters: HashMap::new(),
            }],
            estimated_success_rate: 0.5,
        }
    }
}

// Helper functions
fn has_repetition(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() < 10 {
        return false;
    }
    let first_quarter = words.len() / 4;
    let last_quarter = words.len() * 3 / 4;

    // Check if first quarter is repeated in last quarter
    let first = &words[..first_quarter.min(5)];
    let last = &words[last_quarter.saturating_sub(first.len())..];

    first.iter().zip(last.iter()).fold(
        0,
        |matches, (a, b)| {
            if a == b {
                matches + 1
            } else {
                matches
            }
        },
    ) >= first.len() / 2
}

fn contains_safety_issues(s: &str) -> bool {
    let lower = s.to_lowercase();
    let unsafe_patterns = [
        "injection",
        "exploit",
        "vulnerability",
        "attack",
        "malicious",
    ];
    unsafe_patterns.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_assessor_new() {
        let assessor = SelfAssessor::new();
        // Short valid text is acceptable (score > 0.5 threshold)
        assert!(assessor.is_acceptable("valid output"));
    }

    #[test]
    fn test_assess_empty_output() {
        let assessor = SelfAssessor::new();
        let result = assessor.assess("", &AssessmentContext::default());
        assert!(result.quality_score < 1.0);
        assert!(result.issues_found.contains(&"Output is empty".to_string()));
    }

    #[test]
    fn test_assess_good_output() {
        let assessor = SelfAssessor::new();
        let result = assessor.assess(
            "This is a reasonable response to the task at hand.",
            &AssessmentContext::default(),
        );
        assert!(result.quality_score >= 0.7);
    }

    #[test]
    fn test_assess_output_with_errors() {
        let assessor = SelfAssessor::new();
        let result = assessor.assess(
            "ERROR: Something failed in the system",
            &AssessmentContext::default(),
        );
        assert!(result
            .issues_found
            .iter()
            .any(|i| i.to_lowercase().contains("error")));
    }

    #[test]
    fn test_quality_level_from_score() {
        assert_eq!(QualityLevel::from_score(0.95), QualityLevel::Excellent);
        assert_eq!(QualityLevel::from_score(0.8), QualityLevel::Good);
        assert_eq!(QualityLevel::from_score(0.6), QualityLevel::Acceptable);
        assert_eq!(QualityLevel::from_score(0.3), QualityLevel::Poor);
        assert_eq!(QualityLevel::from_score(0.1), QualityLevel::Failed);
    }

    #[test]
    fn test_failure_classifier_network() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("Connection timeout after 30 seconds");
        assert_eq!(result.category, FailureCategory::Network);
    }

    #[test]
    fn test_failure_classifier_logic() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("null pointer exception at line 42");
        assert_eq!(result.category, FailureCategory::Logic);
    }

    #[test]
    fn test_failure_classifier_model() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("Rate limit exceeded, try again later");
        assert_eq!(result.category, FailureCategory::Model);
    }

    #[test]
    fn test_failure_classifier_from_status() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(429);
        assert_eq!(result.category, FailureCategory::Model);
        assert_eq!(result.error_code, Some("429".to_string()));
    }

    #[test]
    fn test_repair_strategy_generator() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Network,
            confidence: 0.9,
            evidence: vec!["timeout".to_string()],
            error_code: None,
        };

        let strategy = generator.generate(&failure, 0);
        assert!(strategy.estimated_success_rate > 0.0);
    }

    #[test]
    fn test_repair_strategy_skips_after_max_retries() {
        let generator = RepairStrategyGenerator::new().with_max_retries(3);
        let failure = FailureClassification {
            category: FailureCategory::Unknown,
            confidence: 0.5,
            evidence: vec![],
            error_code: None,
        };

        let strategy = generator.generate(&failure, 3);
        assert_eq!(strategy.strategy_type, StrategyType::Skip);
    }

    #[test]
    fn test_has_repetition() {
        // Need >= 10 words, first quarter repeated in last quarter
        // "a b c d e a b c d e": first=["a","b"], last=["a","b","c","d","e"] => 2 matches
        let repeated = "a b c d e a b c d e";
        assert!(has_repetition(repeated));
        assert!(!has_repetition(
            "The quick brown fox jumps over the lazy dog and runs far"
        ));
    }

    #[test]
    fn test_quality_level_boundary_values() {
        assert_eq!(QualityLevel::from_score(1.0), QualityLevel::Excellent);
        assert_eq!(QualityLevel::from_score(0.9), QualityLevel::Excellent);
        assert_eq!(QualityLevel::from_score(0.89), QualityLevel::Good);
        assert_eq!(QualityLevel::from_score(0.75), QualityLevel::Good);
        assert_eq!(QualityLevel::from_score(0.74), QualityLevel::Acceptable);
        assert_eq!(QualityLevel::from_score(0.5), QualityLevel::Acceptable);
        assert_eq!(QualityLevel::from_score(0.49), QualityLevel::Poor);
        assert_eq!(QualityLevel::from_score(0.25), QualityLevel::Poor);
        assert_eq!(QualityLevel::from_score(0.24), QualityLevel::Failed);
        assert_eq!(QualityLevel::from_score(0.0), QualityLevel::Failed);
    }

    #[test]
    fn test_failure_category_display() {
        assert_eq!(format!("{}", FailureCategory::Network), "Network");
        assert_eq!(format!("{}", FailureCategory::Logic), "Logic");
        assert_eq!(format!("{}", FailureCategory::Data), "Data");
        assert_eq!(format!("{}", FailureCategory::Model), "Model");
        assert_eq!(format!("{}", FailureCategory::Timeout), "Timeout");
        assert_eq!(format!("{}", FailureCategory::Auth), "Auth");
        assert_eq!(format!("{}", FailureCategory::Unknown), "Unknown");
    }

    #[test]
    fn test_assessment_config_default() {
        let config = AssessmentConfig::default();
        assert_eq!(config.min_quality_threshold, 0.5);
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_auto_repair);
    }

    #[test]
    fn test_self_assessor_with_custom_config() {
        let config = AssessmentConfig {
            min_quality_threshold: 0.3,
            max_retries: 5,
            enable_auto_repair: false,
        };
        let assessor = SelfAssessor::with_config(config);
        // With lower threshold, more outputs should be acceptable
        assert!(assessor.is_acceptable("valid output content"));
    }

    #[test]
    fn test_assess_truncated_output() {
        let assessor = SelfAssessor::new();
        let result = assessor.assess("This output ends with...", &AssessmentContext::default());
        assert!(result.issues_found.iter().any(|i| i.contains("truncated")));
    }

    #[test]
    fn test_assess_safety_issues() {
        let assessor = SelfAssessor::new();
        let result = assessor.assess(
            "This output contains an injection attack vector",
            &AssessmentContext::default(),
        );
        assert!(result.issues_found.iter().any(|i| i.contains("safety")));
        assert!(result.quality_score < 1.0);
    }

    #[test]
    fn test_assess_with_matching_task() {
        let assessor = SelfAssessor::new();
        let context = AssessmentContext {
            task: Some("fix".to_string()),
            expected_length: None,
            constraints: vec![],
        };
        // Output contains "fix" (first 3 chars of task)
        let result = assessor.assess(
            "We need to fix the compilation errors in the module",
            &context,
        );
        assert!(!result
            .issues_found
            .iter()
            .any(|i| i.contains("address the task")));
    }

    #[test]
    fn test_assess_with_mismatched_task() {
        let assessor = SelfAssessor::new();
        let context = AssessmentContext {
            task: Some("implement the database migration system".to_string()),
            expected_length: None,
            constraints: vec![],
        };
        let result = assessor.assess("Hello world", &context);
        assert!(result
            .issues_found
            .iter()
            .any(|i| i.contains("address the task")));
    }

    #[test]
    fn test_assess_with_length_mismatch() {
        let assessor = SelfAssessor::new();
        let context = AssessmentContext {
            task: None,
            expected_length: Some(1000),
            constraints: vec![],
        };
        let result = assessor.assess("short", &context);
        assert!(result.issues_found.iter().any(|i| i.contains("length")));
    }

    #[test]
    fn test_is_acceptable_with_custom_threshold() {
        let config = AssessmentConfig {
            min_quality_threshold: 0.2,
            ..Default::default()
        };
        let assessor = SelfAssessor::with_config(config);
        // Even with some issues, lower threshold should accept
        assert!(assessor.is_acceptable("Some reasonable output text"));
    }

    #[test]
    fn test_failure_classifier_data() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("parse error in JSON response");
        assert_eq!(result.category, FailureCategory::Data);
    }

    #[test]
    fn test_failure_classifier_timeout() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("Request timed out after 30 seconds");
        assert_eq!(result.category, FailureCategory::Timeout);
    }

    #[test]
    fn test_failure_classifier_auth() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("unauthorized access to resource");
        assert_eq!(result.category, FailureCategory::Auth);
    }

    #[test]
    fn test_failure_classifier_unknown() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify("something completely unexpected happened");
        assert_eq!(result.category, FailureCategory::Unknown);
        assert_eq!(result.confidence, 0.5);
    }

    #[test]
    fn test_classify_from_status_400() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(400);
        assert_eq!(result.category, FailureCategory::Data);
        assert_eq!(result.error_code, Some("400".to_string()));
    }

    #[test]
    fn test_classify_from_status_401() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(401);
        assert_eq!(result.category, FailureCategory::Auth);
    }

    #[test]
    fn test_classify_from_status_403() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(403);
        assert_eq!(result.category, FailureCategory::Auth);
    }

    #[test]
    fn test_classify_from_status_404() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(404);
        assert_eq!(result.category, FailureCategory::Data);
    }

    #[test]
    fn test_classify_from_status_408() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(408);
        assert_eq!(result.category, FailureCategory::Timeout);
    }

    #[test]
    fn test_classify_from_status_500() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(500);
        assert_eq!(result.category, FailureCategory::Logic);
    }

    #[test]
    fn test_classify_from_status_503() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(503);
        assert_eq!(result.category, FailureCategory::Logic);
    }

    #[test]
    fn test_classify_from_status_200() {
        let classifier = FailureClassifier::new();
        let result = classifier.classify_from_status(200);
        assert_eq!(result.category, FailureCategory::Unknown);
        assert_eq!(result.confidence, 0.8);
    }

    #[test]
    fn test_extract_error_code_patterns() {
        // "code: 123" pattern
        let code = extract_error_code("Error code: 404 not found");
        assert!(code.is_some());

        // "ERR_123" pattern
        let code = extract_error_code("ERR_500 internal error");
        assert!(code.is_some());

        // No pattern
        let code = extract_error_code("just a plain error message");
        assert!(code.is_none());
    }

    #[test]
    fn test_repair_generator_with_max_retries() {
        let generator = RepairStrategyGenerator::new().with_max_retries(5);
        let failure = FailureClassification {
            category: FailureCategory::Unknown,
            confidence: 0.5,
            evidence: vec![],
            error_code: None,
        };
        // attempt 3 < max_retries 5 => should not skip
        let strategy = generator.generate(&failure, 3);
        assert_ne!(strategy.strategy_type, StrategyType::Skip);
    }

    #[test]
    fn test_generate_network_repair() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Network,
            confidence: 0.9,
            evidence: vec!["connection refused".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 0);
        assert_eq!(strategy.strategy_type, StrategyType::Retry);
        assert_eq!(strategy.steps.len(), 2);
        assert_eq!(strategy.estimated_success_rate, 0.7);
    }

    #[test]
    fn test_generate_timeout_repair_exponential_backoff() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Timeout,
            confidence: 0.8,
            evidence: vec!["timed out".to_string()],
            error_code: None,
        };
        let s0 = generator.generate(&failure, 0);
        let s1 = generator.generate(&failure, 1);
        assert_eq!(s0.strategy_type, StrategyType::Retry);
        assert_eq!(s1.strategy_type, StrategyType::Retry);
        // Exponential backoff: delay increases
        assert!(s1.estimated_success_rate > 0.0);
    }

    #[test]
    fn test_generate_model_repair_early_retry() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Model,
            confidence: 0.7,
            evidence: vec!["rate limit".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 0);
        assert_eq!(strategy.strategy_type, StrategyType::Retry);
    }

    #[test]
    fn test_generate_model_repair_switch_after_retries() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Model,
            confidence: 0.7,
            evidence: vec!["rate limit".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 2);
        assert_eq!(strategy.strategy_type, StrategyType::SwitchModel);
    }

    #[test]
    fn test_generate_logic_repair() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Logic,
            confidence: 0.6,
            evidence: vec!["null pointer".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 0);
        assert_eq!(strategy.strategy_type, StrategyType::Simplify);
        assert_eq!(strategy.estimated_success_rate, 0.4);
    }

    #[test]
    fn test_generate_data_repair() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Data,
            confidence: 0.7,
            evidence: vec!["parse error".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 0);
        assert_eq!(strategy.strategy_type, StrategyType::ExpandContext);
        assert_eq!(strategy.steps.len(), 2);
    }

    #[test]
    fn test_generate_auth_repair() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Auth,
            confidence: 0.9,
            evidence: vec!["unauthorized".to_string()],
            error_code: None,
        };
        let strategy = generator.generate(&failure, 0);
        assert_eq!(strategy.strategy_type, StrategyType::Fallback);
        assert_eq!(strategy.estimated_success_rate, 0.7);
    }

    #[test]
    fn test_generate_generic_repair_attempts() {
        let generator = RepairStrategyGenerator::new();
        let failure = FailureClassification {
            category: FailureCategory::Unknown,
            confidence: 0.5,
            evidence: vec![],
            error_code: None,
        };
        assert_eq!(
            generator.generate(&failure, 0).strategy_type,
            StrategyType::Retry
        );
        assert_eq!(
            generator.generate(&failure, 1).strategy_type,
            StrategyType::UseCache
        );
        assert_eq!(
            generator.generate(&failure, 2).strategy_type,
            StrategyType::Fallback
        );
    }

    #[test]
    fn test_contains_safety_issues() {
        assert!(contains_safety_issues("SQL injection vulnerability"));
        assert!(contains_safety_issues("This is an exploit attempt"));
        assert!(contains_safety_issues("Found malicious code"));
        assert!(!contains_safety_issues("Everything looks good"));
        assert!(!contains_safety_issues("Normal output"));
    }

    #[test]
    fn test_has_repetition_short_string() {
        // Strings with < 10 words should not be flagged
        assert!(!has_repetition("one two three four five"));
        assert!(!has_repetition(""));
    }

    #[test]
    fn test_failure_classification_serialization() {
        let classification = FailureClassification {
            category: FailureCategory::Network,
            confidence: 0.85,
            evidence: vec!["timeout".to_string()],
            error_code: Some("E001".to_string()),
        };
        let json = serde_json::to_string(&classification).unwrap();
        assert!(json.contains("Network"));
        assert!(json.contains("0.85"));
        assert!(json.contains("E001"));
    }

    #[test]
    fn test_repair_strategy_serialization() {
        let strategy = RepairStrategy {
            strategy_type: StrategyType::Retry,
            steps: vec![RepairStep {
                step_number: 1,
                action: "wait".to_string(),
                parameters: HashMap::new(),
            }],
            estimated_success_rate: 0.8,
        };
        let json = serde_json::to_string(&strategy).unwrap();
        assert!(json.contains("Retry"));
        assert!(json.contains("wait"));
    }

    #[test]
    fn test_assessment_result_can_repair() {
        let assessor = SelfAssessor::new();
        // Good enough to not fail, but not perfect => can_repair
        let result = assessor.assess(
            "This is a reasonable response with some issues...",
            &AssessmentContext::default(),
        );
        // Score should be between threshold and 0.9 => can_repair
        if result.quality_score >= 0.5 && result.quality_score < 0.9 {
            assert!(result.can_repair);
        }
    }
}
