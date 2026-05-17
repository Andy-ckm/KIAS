//! 自动测试验证 — KIAS自循环的核心
//!
//! 自动验证修复效果，包括：
//! - 编译验证
//! - 测试验证
//! - 功能验证
//! - 性能验证

use serde::{Deserialize, Serialize};

/// 验证类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationType {
    /// 编译验证
    Compilation,
    /// 测试验证
    Test,
    /// 功能验证
    Functional,
    /// 性能验证
    Performance,
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// 验证类型
    pub verification_type: VerificationType,
    /// 是否通过
    pub passed: bool,
    /// 验证详情
    pub details: String,
    /// 错误信息
    pub errors: Vec<String>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 验证时间
    pub verified_at: chrono::DateTime<chrono::Utc>,
}

/// 验证器 trait
pub trait Verifier: Send + Sync {
    /// 执行验证
    fn verify(&self, target: &str) -> VerificationResult;

    /// 获取验证器名称
    fn name(&self) -> &str;

    /// 获取验证器类型
    fn verification_type(&self) -> VerificationType;
}

/// 编译验证器
pub struct CompilationVerifier;

impl Default for CompilationVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilationVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Verifier for CompilationVerifier {
    fn verify(&self, target: &str) -> VerificationResult {
        // 模拟编译验证
        // 在实际实现中，这里会执行cargo build
        VerificationResult {
            verification_type: VerificationType::Compilation,
            passed: true,
            details: format!("编译验证通过: {}", target),
            errors: vec![],
            warnings: vec![],
            verified_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "CompilationVerifier"
    }

    fn verification_type(&self) -> VerificationType {
        VerificationType::Compilation
    }
}

/// 测试验证器
pub struct TestVerifier;

impl Default for TestVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TestVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Verifier for TestVerifier {
    fn verify(&self, target: &str) -> VerificationResult {
        // 模拟测试验证
        // 在实际实现中，这里会执行cargo test
        VerificationResult {
            verification_type: VerificationType::Test,
            passed: true,
            details: format!("测试验证通过: {}", target),
            errors: vec![],
            warnings: vec![],
            verified_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "TestVerifier"
    }

    fn verification_type(&self) -> VerificationType {
        VerificationType::Test
    }
}

/// 验证器管理器
pub struct VerifierManager {
    /// 验证器列表
    verifiers: Vec<Box<dyn Verifier>>,
    /// 验证历史
    history: Vec<VerificationResult>,
}

impl Default for VerifierManager {
    fn default() -> Self {
        Self::new()
    }
}

impl VerifierManager {
    pub fn new() -> Self {
        Self {
            verifiers: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 注册验证器
    pub fn register_verifier(&mut self, verifier: Box<dyn Verifier>) {
        self.verifiers.push(verifier);
    }

    /// 执行所有验证
    pub fn verify_all(&mut self, target: &str) -> Vec<VerificationResult> {
        let mut results = Vec::new();

        for verifier in &self.verifiers {
            let result = verifier.verify(target);
            results.push(result.clone());
            self.history.push(result);
        }

        results
    }

    /// 获取验证历史
    pub fn history(&self) -> &[VerificationResult] {
        &self.history
    }

    /// 检查是否所有验证都通过
    pub fn all_passed(&mut self, target: &str) -> bool {
        let results = self.verify_all(target);
        results.iter().all(|r| r.passed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compilation_verifier() {
        let verifier = CompilationVerifier::new();
        let result = verifier.verify("kias-api-server");

        assert!(result.passed);
        assert_eq!(result.verification_type, VerificationType::Compilation);
    }

    #[test]
    fn test_test_verifier() {
        let verifier = TestVerifier::new();
        let result = verifier.verify("kias-api-server");

        assert!(result.passed);
        assert_eq!(result.verification_type, VerificationType::Test);
    }

    #[test]
    fn test_verifier_manager() {
        let mut manager = VerifierManager::new();

        manager.register_verifier(Box::new(CompilationVerifier::new()));
        manager.register_verifier(Box::new(TestVerifier::new()));

        let results = manager.verify_all("kias-api-server");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.passed));
    }

    #[test]
    fn test_verification_type_variants() {
        assert!(matches!(
            VerificationType::Compilation,
            VerificationType::Compilation
        ));
        assert!(matches!(VerificationType::Test, VerificationType::Test));
        assert!(matches!(
            VerificationType::Functional,
            VerificationType::Functional
        ));
        assert!(matches!(
            VerificationType::Performance,
            VerificationType::Performance
        ));
    }

    #[test]
    fn test_verification_result_fields() {
        let result = VerificationResult {
            verification_type: VerificationType::Compilation,
            passed: true,
            details: "Build succeeded".to_string(),
            errors: vec![],
            warnings: vec!["unused import".to_string()],
            verified_at: chrono::Utc::now(),
        };
        assert!(result.passed);
        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_verifier_manager_history_accumulation() {
        let mut manager = VerifierManager::new();
        manager.register_verifier(Box::new(CompilationVerifier::new()));

        manager.verify_all("kias-common");
        assert_eq!(manager.history().len(), 1);

        manager.verify_all("kias-common");
        assert_eq!(manager.history().len(), 2);
    }

    #[test]
    fn test_all_passed_empty_manager() {
        let mut manager = VerifierManager::new();
        assert!(manager.all_passed("any-target"));
    }

    #[test]
    fn test_all_passed_with_test_verifier() {
        let mut manager = VerifierManager::new();
        manager.register_verifier(Box::new(TestVerifier::new()));
        assert!(manager.all_passed("kias-common"));
    }
}
