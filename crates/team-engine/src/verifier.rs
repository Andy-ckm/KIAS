use super::state::Task;
use async_trait::async_trait;
use kias_common::KiasResult;

/// Verifier - 质量门禁（借鉴 MiniMax 设计）
///
/// 核心设计：Worker-Verifier 对抗机制
///
/// Worker 停止的条件是 Verifier 启动的原因，
/// Verifier 停止的条件是尽可能发现 Worker 的问题，
/// 发现的问题又成为 Worker 重新启动的原因。
#[async_trait]
pub trait Verifier: Send + Sync {
    /// 验证任务结果
    async fn verify(&self, task: &Task, result: &str) -> KiasResult<VerificationResult>;

    /// 获取验证者名称
    fn name(&self) -> &str;
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub passed: bool,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Verification rule that can be applied to task results
#[derive(Debug, Clone)]
pub enum VerificationRule {
    /// Check that output contains specific text
    Contains(String),
    /// Check that output does NOT contain specific text
    NotContains(String),
    /// Check minimum output length
    MinLength(usize),
    /// Check maximum output length
    MaxLength(usize),
    /// Check that output is valid JSON
    ValidJson,
    /// Check that output matches a simple pattern (substring match)
    Pattern(String),
    /// Check that a shell command succeeds
    ShellCheck(String),
}

/// Configurable verifier with rules
pub struct RuleBasedVerifier {
    name: String,
    rules: Vec<VerificationRule>,
}

impl RuleBasedVerifier {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            rules: Vec::new(),
        }
    }

    pub fn with_rule(mut self, rule: VerificationRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_rules(mut self, rules: Vec<VerificationRule>) -> Self {
        self.rules.extend(rules);
        self
    }
}

#[async_trait]
impl Verifier for RuleBasedVerifier {
    async fn verify(&self, task: &Task, result: &str) -> KiasResult<VerificationResult> {
        tracing::info!(verifier = %self.name, task_id = %task.id, rules = self.rules.len(), "Running rule-based verification");

        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        for rule in &self.rules {
            match rule {
                VerificationRule::Contains(text) => {
                    if !result.contains(text) {
                        issues.push(format!("Output does not contain '{}'", text));
                        suggestions.push(format!("Ensure output includes '{}'", text));
                    }
                }
                VerificationRule::NotContains(text) => {
                    if result.contains(text) {
                        issues.push(format!("Output contains forbidden text '{}'", text));
                        suggestions.push(format!("Remove '{}' from output", text));
                    }
                }
                VerificationRule::MinLength(min) => {
                    if result.len() < *min {
                        issues.push(format!(
                            "Output too short: {} < {} chars",
                            result.len(),
                            min
                        ));
                        suggestions.push("Provide more detailed output".to_string());
                    }
                }
                VerificationRule::MaxLength(max) => {
                    if result.len() > *max {
                        issues.push(format!("Output too long: {} > {} chars", result.len(), max));
                        suggestions.push("Reduce output length".to_string());
                    }
                }
                VerificationRule::ValidJson => {
                    if serde_json::from_str::<serde_json::Value>(result).is_err() {
                        issues.push("Output is not valid JSON".to_string());
                        suggestions.push("Ensure output is valid JSON".to_string());
                    }
                }
                VerificationRule::Pattern(pattern) => {
                    if !result.contains(pattern) {
                        issues.push(format!("Output does not match pattern '{}'", pattern));
                        suggestions.push(format!("Include '{}' in output", pattern));
                    }
                }
                VerificationRule::ShellCheck(command) => {
                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .output()
                        .await;

                    match output {
                        Ok(out) => {
                            if !out.status.success() {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                issues
                                    .push(format!("Shell check '{}' failed: {}", command, stderr));
                                suggestions.push(format!("Fix issues detected by '{}'", command));
                            }
                        }
                        Err(e) => {
                            issues.push(format!("Shell check '{}' error: {}", command, e));
                        }
                    }
                }
            }
        }

        let passed = issues.is_empty();
        Ok(VerificationResult {
            passed,
            issues,
            suggestions,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct CodeVerifier {
    name: String,
    rules: RuleBasedVerifier,
}

impl CodeVerifier {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            rules: RuleBasedVerifier::new(name)
                .with_rule(VerificationRule::NotContains("ERROR".to_string()))
                .with_rule(VerificationRule::NotContains("panic".to_string()))
                .with_rule(VerificationRule::MinLength(1)),
        }
    }

    pub fn with_custom_rules(mut self, rules: Vec<VerificationRule>) -> Self {
        self.rules = self.rules.with_rules(rules);
        self
    }
}

#[async_trait]
impl Verifier for CodeVerifier {
    async fn verify(&self, task: &Task, result: &str) -> KiasResult<VerificationResult> {
        tracing::info!(verifier = %self.name, task_id = %task.id, "Verifying code output");

        // Run rule-based verification
        let mut base_result = self.rules.verify(task, result).await?;

        // Additional code-specific checks
        if result.contains("TODO") {
            base_result
                .issues
                .push("Output contains TODO comments".to_string());
            base_result
                .suggestions
                .push("Resolve all TODO items".to_string());
        }

        if result.contains("FIXME") {
            base_result
                .issues
                .push("Output contains FIXME comments".to_string());
            base_result
                .suggestions
                .push("Resolve all FIXME items".to_string());
        }

        base_result.passed = base_result.issues.is_empty();
        Ok(base_result)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct ResearchVerifier {
    name: String,
    rules: RuleBasedVerifier,
}

impl ResearchVerifier {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            rules: RuleBasedVerifier::new(name)
                .with_rule(VerificationRule::MinLength(50))
                .with_rule(VerificationRule::NotContains("I don't know".to_string())),
        }
    }
}

#[async_trait]
impl Verifier for ResearchVerifier {
    async fn verify(&self, task: &Task, result: &str) -> KiasResult<VerificationResult> {
        tracing::info!(verifier = %self.name, task_id = %task.id, "Verifying research output");

        let mut base_result = self.rules.verify(task, result).await?;

        // Check for source citations
        if !result.contains("source") && !result.contains("Source") && !result.contains("http") {
            base_result
                .issues
                .push("No sources cited in research output".to_string());
            base_result
                .suggestions
                .push("Include source references".to_string());
        }

        base_result.passed = base_result.issues.is_empty();
        Ok(base_result)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Quality gate - combines multiple verifiers
pub struct QualityGate {
    name: String,
    verifiers: Vec<Box<dyn Verifier>>,
    /// All verifiers must pass for the gate to pass
    require_all: bool,
}

impl QualityGate {
    pub fn new(name: &str, require_all: bool) -> Self {
        Self {
            name: name.to_string(),
            verifiers: Vec::new(),
            require_all,
        }
    }

    pub fn add_verifier(mut self, verifier: Box<dyn Verifier>) -> Self {
        self.verifiers.push(verifier);
        self
    }
}

#[async_trait]
impl Verifier for QualityGate {
    async fn verify(&self, task: &Task, result: &str) -> KiasResult<VerificationResult> {
        let mut all_issues = Vec::new();
        let mut all_suggestions = Vec::new();
        let mut any_passed = false;
        let mut all_passed = true;

        for verifier in &self.verifiers {
            let vr = verifier.verify(task, result).await?;
            if vr.passed {
                any_passed = true;
            } else {
                all_passed = false;
                all_issues.extend(vr.issues);
                all_suggestions.extend(vr.suggestions);
            }
        }

        let passed = if self.require_all {
            all_passed
        } else {
            any_passed
        };

        Ok(VerificationResult {
            passed,
            issues: all_issues,
            suggestions: all_suggestions,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            name: "test".to_string(),
            description: "test task".to_string(),
            assigned_to: None,
            verified_by: None,
            status: crate::state::TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            retry_count: 0,
            max_retries: 3,
            context: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_rule_based_verifier_contains() {
        let verifier = RuleBasedVerifier::new("test")
            .with_rule(VerificationRule::Contains("hello".to_string()));

        let task = make_task("t1");
        let result = verifier.verify(&task, "hello world").await.unwrap();
        assert!(result.passed);

        let result = verifier.verify(&task, "goodbye world").await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_rule_based_verifier_not_contains() {
        let verifier = RuleBasedVerifier::new("test")
            .with_rule(VerificationRule::NotContains("ERROR".to_string()));

        let task = make_task("t1");
        let result = verifier.verify(&task, "everything is fine").await.unwrap();
        assert!(result.passed);

        let result = verifier
            .verify(&task, "ERROR: something broke")
            .await
            .unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_rule_based_verifier_min_length() {
        let verifier = RuleBasedVerifier::new("test").with_rule(VerificationRule::MinLength(10));

        let task = make_task("t1");
        let result = verifier.verify(&task, "short").await.unwrap();
        assert!(!result.passed);

        let result = verifier
            .verify(&task, "this is a longer string")
            .await
            .unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_rule_based_verifier_max_length() {
        let verifier = RuleBasedVerifier::new("test").with_rule(VerificationRule::MaxLength(20));

        let task = make_task("t1");
        let result = verifier.verify(&task, "short").await.unwrap();
        assert!(result.passed);

        let result = verifier
            .verify(&task, "this is a very long string that exceeds the limit")
            .await
            .unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_rule_based_verifier_valid_json() {
        let verifier = RuleBasedVerifier::new("test").with_rule(VerificationRule::ValidJson);

        let task = make_task("t1");
        let result = verifier.verify(&task, r#"{"key": "value"}"#).await.unwrap();
        assert!(result.passed);

        let result = verifier.verify(&task, "not json").await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_rule_based_verifier_multiple_rules() {
        let verifier = RuleBasedVerifier::new("test")
            .with_rule(VerificationRule::Contains("result".to_string()))
            .with_rule(VerificationRule::NotContains("ERROR".to_string()))
            .with_rule(VerificationRule::MinLength(5));

        let task = make_task("t1");

        // All pass
        let result = verifier.verify(&task, "result: success").await.unwrap();
        assert!(result.passed);

        // First passes, second fails
        let result = verifier.verify(&task, "result: ERROR").await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.issues.len(), 1);
    }

    #[tokio::test]
    async fn test_code_verifier() {
        let verifier = CodeVerifier::new("code-checker");
        let task = make_task("t1");

        let result = verifier.verify(&task, "all good").await.unwrap();
        assert!(result.passed);

        let result = verifier
            .verify(&task, "ERROR: compilation failed")
            .await
            .unwrap();
        assert!(!result.passed);

        let result = verifier.verify(&task, "panic: something").await.unwrap();
        assert!(!result.passed);

        let result = verifier
            .verify(&task, "// TODO: implement this")
            .await
            .unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_research_verifier() {
        let verifier = ResearchVerifier::new("research-checker");
        let task = make_task("t1");

        // Good research with sources
        let result = verifier.verify(&task, "According to source: this is a long enough research output that meets minimum length requirements for verification").await.unwrap();
        assert!(result.passed);

        // Too short
        let result = verifier.verify(&task, "short").await.unwrap();
        assert!(!result.passed);

        // No references cited
        let result = verifier.verify(&task, "This is a long enough output but has no references or citations at all in the text body whatsoever").await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_quality_gate_all_required() {
        let gate = QualityGate::new("gate", true)
            .add_verifier(Box::new(
                RuleBasedVerifier::new("v1")
                    .with_rule(VerificationRule::Contains("ok".to_string())),
            ))
            .add_verifier(Box::new(
                RuleBasedVerifier::new("v2").with_rule(VerificationRule::MinLength(5)),
            ));

        let task = make_task("t1");

        // Both pass
        let result = gate.verify(&task, "ok result").await.unwrap();
        assert!(result.passed);

        // One fails
        let result = gate.verify(&task, "ok").await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_quality_gate_any_required() {
        let gate = QualityGate::new("gate", false)
            .add_verifier(Box::new(
                RuleBasedVerifier::new("v1")
                    .with_rule(VerificationRule::Contains("ok".to_string())),
            ))
            .add_verifier(Box::new(
                RuleBasedVerifier::new("v2")
                    .with_rule(VerificationRule::Contains("fail".to_string())),
            ));

        let task = make_task("t1");

        // First passes
        let result = gate.verify(&task, "ok result").await.unwrap();
        assert!(result.passed);

        // Second passes
        let result = gate.verify(&task, "fail result").await.unwrap();
        assert!(result.passed);

        // Neither passes
        let result = gate.verify(&task, "neither").await.unwrap();
        assert!(!result.passed);
    }
}
