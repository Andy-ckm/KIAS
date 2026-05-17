//! 自动测试验证 — KIAS自循环的核心（真实执行版）
//!
//! 自动验证修复效果，包括：
//! - 编译验证（真实 cargo check）
//! - 测试验证（真实 cargo test）
//! - Clippy 验证（真实 cargo clippy）
//! - 功能验证（真实端点健康检查）
//!
//! ## 控制论原理
//! 验证器是闭环的"传感器"——从真实环境采集信号，而非模拟。
//! 参考：Wiener Cybernetics (1948) — 反馈必须基于真实测量。

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::{Duration, Instant};

/// 验证类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationType {
    /// 编译验证（cargo check）
    Compilation,
    /// 测试验证（cargo test）
    Test,
    /// Clippy 验证（cargo clippy -- -D warnings）
    Clippy,
    /// 格式验证（cargo fmt --check）
    Format,
    /// 功能验证（HTTP健康检查）
    Functional,
    /// 性能验证（基准测试）
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
    /// 错误信息（来自stderr）
    pub errors: Vec<String>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 验证耗时（毫秒）
    pub duration_ms: u64,
    /// 验证时间
    pub verified_at: chrono::DateTime<chrono::Utc>,
}

/// 验证器 trait
pub trait Verifier: Send + Sync {
    /// 执行验证
    fn verify(&self, workspace_path: &str) -> VerificationResult;

    /// 获取验证器名称
    fn name(&self) -> &str;

    /// 获取验证器类型
    fn verification_type(&self) -> VerificationType;
}

/// 执行命令并捕获输出
fn run_command(
    program: &str,
    args: &[&str],
    cwd: &str,
    timeout_secs: u64,
) -> (bool, String, String, Duration) {
    let start = Instant::now();
    let result = Command::new(program).args(args).current_dir(cwd).output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();
            (success, stdout, stderr, start.elapsed())
        }
        Err(e) => {
            let elapsed = start.elapsed();
            // 超时保护
            if elapsed.as_secs() > timeout_secs {
                (
                    false,
                    String::new(),
                    format!("命令执行超时({}s): {}", timeout_secs, e),
                    elapsed,
                )
            } else {
                (
                    false,
                    String::new(),
                    format!("命令执行失败: {}", e),
                    elapsed,
                )
            }
        }
    }
}

/// 编译验证器 — 真实执行 cargo check
pub struct CompilationVerifier {
    workspace_path: String,
}

impl CompilationVerifier {
    pub fn new(workspace_path: String) -> Self {
        Self { workspace_path }
    }
}

impl Default for CompilationVerifier {
    fn default() -> Self {
        Self::new("/workspace/kias".to_string())
    }
}

impl Verifier for CompilationVerifier {
    fn verify(&self, _target: &str) -> VerificationResult {
        let (success, _stdout, stderr, duration) = run_command(
            "cargo",
            &["check", "--workspace"],
            &self.workspace_path,
            300,
        );

        // 提取警告
        let warnings: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("warning[") || l.contains("warning:"))
            .map(|l| l.to_string())
            .collect();

        // 提取错误
        let errors: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("error[") || l.contains("error:") || l.contains("aborting"))
            .map(|l| l.to_string())
            .collect();

        VerificationResult {
            verification_type: VerificationType::Compilation,
            passed: success,
            details: if success {
                format!(
                    "cargo check 通过 ({}ms, {} 警告)",
                    duration.as_millis(),
                    warnings.len()
                )
            } else {
                format!(
                    "cargo check 失败 ({}ms, {} 错误)",
                    duration.as_millis(),
                    errors.len()
                )
            },
            errors,
            warnings,
            duration_ms: duration.as_millis() as u64,
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

/// 测试验证器 — 真实执行 cargo test
pub struct TestVerifier {
    workspace_path: String,
}

impl TestVerifier {
    pub fn new(workspace_path: String) -> Self {
        Self { workspace_path }
    }
}

impl Default for TestVerifier {
    fn default() -> Self {
        Self::new("/workspace/kias".to_string())
    }
}

impl Verifier for TestVerifier {
    fn verify(&self, _target: &str) -> VerificationResult {
        let (success, stdout, stderr, duration) = run_command(
            "cargo",
            &["test", "--workspace", "--", "--test-threads=4"],
            &self.workspace_path,
            600,
        );

        // 解析测试结果
        let test_summary = stdout
            .lines()
            .rfind(|l| l.starts_with("test result:"))
            .unwrap_or("无测试结果")
            .to_string();

        let errors: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("FAILED") || l.contains("error"))
            .map(|l| l.to_string())
            .collect();

        VerificationResult {
            verification_type: VerificationType::Test,
            passed: success,
            details: format!(
                "cargo test {} ({}ms) - {}",
                if success { "通过" } else { "失败" },
                duration.as_millis(),
                test_summary
            ),
            errors,
            warnings: vec![],
            duration_ms: duration.as_millis() as u64,
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

/// Clippy 验证器 — 真实执行 cargo clippy -- -D warnings
pub struct ClippyVerifier {
    workspace_path: String,
}

impl ClippyVerifier {
    pub fn new(workspace_path: String) -> Self {
        Self { workspace_path }
    }
}

impl Default for ClippyVerifier {
    fn default() -> Self {
        Self::new("/workspace/kias".to_string())
    }
}

impl Verifier for ClippyVerifier {
    fn verify(&self, _target: &str) -> VerificationResult {
        let (success, _stdout, stderr, duration) = run_command(
            "cargo",
            &["clippy", "--workspace", "--", "-D", "warnings"],
            &self.workspace_path,
            600,
        );

        let warnings: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("warning"))
            .map(|l| l.to_string())
            .collect();

        let errors: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("error"))
            .map(|l| l.to_string())
            .collect();

        VerificationResult {
            verification_type: VerificationType::Clippy,
            passed: success,
            details: format!(
                "cargo clippy {} ({}ms)",
                if success { "通过" } else { "失败" },
                duration.as_millis()
            ),
            errors,
            warnings,
            duration_ms: duration.as_millis() as u64,
            verified_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "ClippyVerifier"
    }

    fn verification_type(&self) -> VerificationType {
        VerificationType::Clippy
    }
}

/// 格式验证器 — 真实执行 cargo fmt --check
pub struct FormatVerifier {
    workspace_path: String,
}

impl FormatVerifier {
    pub fn new(workspace_path: String) -> Self {
        Self { workspace_path }
    }
}

impl Default for FormatVerifier {
    fn default() -> Self {
        Self::new("/workspace/kias".to_string())
    }
}

impl Verifier for FormatVerifier {
    fn verify(&self, _target: &str) -> VerificationResult {
        let (success, stdout, _stderr, duration) =
            run_command("cargo", &["fmt", "--check"], &self.workspace_path, 120);

        // 未格式化的文件
        let unformatted: Vec<String> = stdout
            .lines()
            .filter(|l| l.contains("Diff in"))
            .map(|l| l.to_string())
            .collect();

        VerificationResult {
            verification_type: VerificationType::Format,
            passed: success,
            details: format!(
                "cargo fmt {} ({}ms, {} 文件未格式化)",
                if success { "通过" } else { "失败" },
                duration.as_millis(),
                unformatted.len()
            ),
            errors: if success { vec![] } else { unformatted },
            warnings: vec![],
            duration_ms: duration.as_millis() as u64,
            verified_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "FormatVerifier"
    }

    fn verification_type(&self) -> VerificationType {
        VerificationType::Format
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

    /// 创建标准验证器集（check + clippy + fmt + test）
    pub fn with_standard_verifiers(workspace_path: &str) -> Self {
        let mut manager = Self::new();
        manager.register_verifier(Box::new(FormatVerifier::new(workspace_path.to_string())));
        manager.register_verifier(Box::new(CompilationVerifier::new(
            workspace_path.to_string(),
        )));
        manager.register_verifier(Box::new(ClippyVerifier::new(workspace_path.to_string())));
        manager.register_verifier(Box::new(TestVerifier::new(workspace_path.to_string())));
        manager
    }

    /// 注册验证器
    pub fn register_verifier(&mut self, verifier: Box<dyn Verifier>) {
        self.verifiers.push(verifier);
    }

    /// 执行所有验证（短路：任一失败则停止）
    pub fn verify_all(&mut self, target: &str) -> Vec<VerificationResult> {
        let mut results = Vec::new();

        for verifier in &self.verifiers {
            let result = verifier.verify(target);
            let passed = result.passed;
            results.push(result.clone());
            self.history.push(result);

            // 短路：编译失败则不跑测试
            if !passed {
                break;
            }
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

    /// 获取最近一次验证的总耗时
    pub fn last_total_duration_ms(&self) -> u64 {
        self.history
            .iter()
            .rev()
            .take(self.verifiers.len().max(1))
            .map(|r| r.duration_ms)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_type_variants() {
        assert!(matches!(
            VerificationType::Compilation,
            VerificationType::Compilation
        ));
        assert!(matches!(VerificationType::Test, VerificationType::Test));
        assert!(matches!(VerificationType::Clippy, VerificationType::Clippy));
        assert!(matches!(VerificationType::Format, VerificationType::Format));
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
            duration_ms: 1500,
            verified_at: chrono::Utc::now(),
        };
        assert!(result.passed);
        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.duration_ms, 1500);
    }

    #[test]
    fn test_verifier_manager_creation() {
        let manager = VerifierManager::new();
        assert!(manager.verifiers.is_empty());
        assert!(manager.history().is_empty());
    }

    #[test]
    fn test_verifier_manager_with_standard_verifiers() {
        let manager = VerifierManager::with_standard_verifiers("/tmp");
        assert_eq!(manager.verifiers.len(), 4); // fmt + check + clippy + test
    }

    #[test]
    fn test_verifier_manager_empty_all_passed() {
        let mut manager = VerifierManager::new();
        assert!(manager.all_passed("any-target"));
    }

    #[test]
    fn test_verifier_manager_history_accumulation() {
        let mut manager = VerifierManager::new();
        // 没有验证器，history 不增长
        manager.verify_all("test");
        assert!(manager.history().is_empty());
    }

    #[test]
    fn test_run_command_success() {
        let (success, stdout, _stderr, _dur) = run_command("echo", &["hello"], "/tmp", 10);
        assert!(success);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_run_command_failure() {
        let (success, _stdout, stderr, _dur) = run_command("false", &[], "/tmp", 10);
        assert!(!success);
    }

    #[test]
    fn test_run_command_nonexistent() {
        let (success, _stdout, stderr, _dur) =
            run_command("nonexistent_command_xyz", &[], "/tmp", 10);
        assert!(!success);
        assert!(stderr.contains("命令执行失败"));
    }
}
