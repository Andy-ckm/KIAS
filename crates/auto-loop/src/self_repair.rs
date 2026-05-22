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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        let mut score = 1.0;

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
            let len_diff = (output.len() as i64 - expected_len as i64).abs() as f64 / expected_len as f64;
            if len_diff > 0.5 {
                issues.push("Output length significantly differs from expected".to_string());
                suggestions.push("Adjust output to match expected length".to_string());
                score -= 0.1;
            }
        }

        // Check for task completion
        if let Some(ref task) = context.task {
            if !task.is_empty() && !output.to_lowercase().contains(&task.to_lowercase()[..task.len().min(20)]) {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentContext {
    pub task: Option<String>,
    pub expected_length: Option<usize>,
    pub constraints: Vec<String>,
}

impl Default for AssessmentContext {
    fn default() -> Self {
        Self {
            task: None,
            expected_length: None,
            constraints: Vec::new(),
        }
    }
}

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
        
        patterns.insert(FailureCategory::Network, vec![
            "connection refused".to_string(),
            "timeout".to_string(),
            "connection reset".to_string(),
            "network unreachable".to_string(),
            "dns".to_string(),
            "ECONNREFUSED".to_string(),
            "ETIMEDOUT".to_string(),
        ]);
        
        patterns.insert(FailureCategory::Logic, vec![
            "null pointer".to_string(),
            "division by zero".to_string(),
            "index out of bounds".to_string(),
            "assertion failed".to_string(),
            "illegal argument".to_string(),
            "invalid state".to_string(),
        ]);
        
        patterns.insert(FailureCategory::Data, vec![
            "parse error".to_string(),
            "invalid format".to_string(),
            "missing field".to_string(),
            "type mismatch".to_string(),
            "schema".to_string(),
            "corrupt".to_string(),
        ]);
        
        patterns.insert(FailureCategory::Model, vec![
            "rate limit".to_string(),
            "quota exceeded".to_string(),
            "model overloaded".to_string(),
            "context length".to_string(),
            "token limit".to_string(),
        ]);
        
        patterns.insert(FailureCategory::Timeout, vec![
            "timed out".to_string(),
            "deadline exceeded".to_string(),
            "took too long".to_string(),
            "execution time".to_string(),
        ]);
        
        patterns.insert(FailureCategory::Auth, vec![
            "unauthorized".to_string(),
            "forbidden".to_string(),
            "invalid token".to_string(),
            "permission denied".to_string(),
            "access denied".to_string(),
        ]);

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
            400 => (FailureCategory::Data, "Bad Request - invalid input data".to_string()),
            401 | 403 => (FailureCategory::Auth, "Authentication/Authorization failure".to_string()),
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
    pub fn generate(
        &self,
        failure: &FailureClassification,
        attempt: u8,
    ) -> RepairStrategy {
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
                    parameters: [("delay_ms", (1000 * 2_u64)).to_string()].into(),
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
                    parameters: [("delay_ms", delay.to_string())].into(),
                },
                RepairStep {
                    step_number: 2,
                    action: "Retry with longer timeout".to_string(),
                    parameters: [("timeout_ms", (30000 * (attempt + 1) as u64)).to_string()].into(),
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
                    parameters: [("delay_ms", "5000".to_string())].into(),
                }],
                estimated_success_rate: 0.5,
            }
        } else {
            RepairStrategy {
                strategy_type: StrategyType::SwitchModel,
                steps: vec![
                    RepairStep {
                        step_number: 1,
                        action: "Switch to fallback model".to_string(),
                        parameters: [("model", "gpt-4o-mini".to_string())].into(),
                    },
                ],
                estimated_success_rate: 0.8,
            }
        }
    }

    fn logic_repair(&self, _failure: &FailureClassification) -> RepairStrategy {
        RepairStrategy {
            strategy_type: StrategyType::Simplify,
            steps: vec![
                RepairStep {
                    step_number: 1,
                    action: "Simplify the request".to_string(),
                    parameters: [("max_tokens", "500".to_string())].into(),
                },
            ],
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
            steps: vec![
                RepairStep {
                    step_number: 1,
                    action: "Refresh authentication token".to_string(),
                    parameters: HashMap::new(),
                },
            ],
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
    
    first.iter().zip(last.iter()).fold(0, |matches, (a, b)| {
        if a == b { matches + 1 } else { matches }
    }) >= first.len() / 2
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
        assert!(!assessor.is_acceptable("valid output"));
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
        assert!(result.issues_found.iter().any(|i| i.to_lowercase().contains("error")));
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
        let result = classifier.classify("NullPointerException at line 42");
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
        let generator = RepairStrategyGenerator::with_max_retries(3);
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
        assert!(has_repetition("foo foo foo foo foo bar baz"));
        assert!(!has_repetition("The quick brown fox jumps"));
    }
}
