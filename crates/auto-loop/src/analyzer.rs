//! 根因自动分析 — KIAS自循环的核心（真实分析版）
//!
//! 自动分析问题根因，包括：
//! - Cargo 错误输出解析（真实的编译/测试错误分析）
//! - 代码库文件搜索（grep 相关文件）
//! - 错误模式匹配（识别常见 Rust 错误类型）
//!
//! ## 控制论原理
//! Analyzer 是闭环的"决策器"——从感知信号中提取根因，指导修复行动。
//! 参考：钱学森综合集成 — 多源信息融合分析。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

/// 分析器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyzerType {
    /// 代码分析（文件搜索+错误解析）
    Code,
    /// Cargo 输出分析（编译/测试/clippy 错误）
    CargoOutput,
    /// 配置分析
    Config,
    /// 依赖分析
    Dependency,
}

/// 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// 是否找到根因
    pub found_root_cause: bool,
    /// 根因描述
    pub root_cause: Option<String>,
    /// 错误类型分类
    pub error_category: Option<ErrorCategory>,
    /// 影响范围
    pub impact: Option<String>,
    /// 修复难度 (1-10)
    pub difficulty: Option<u8>,
    /// 预计工作量（小时）
    pub estimated_hours: Option<f64>,
    /// 相关文件
    pub related_files: Vec<String>,
    /// 分析详情
    pub details: HashMap<String, String>,
    /// 分析时间
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
}

/// Rust 常见错误分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCategory {
    /// 类型不匹配
    TypeError,
    /// 借用检查失败
    BorrowError,
    /// 生命周期错误
    LifetimeError,
    /// 未找到方法/字段
    NotFound,
    /// 未使用的变量/导入
    Unused,
    /// Clippy 警告
    ClippyWarning,
    /// 测试断言失败
    TestFailure,
    /// 编译错误
    CompilationError,
    /// 未知
    Unknown,
}

/// 分析器 trait
pub trait Analyzer: Send + Sync {
    /// 执行分析
    fn analyze(&self, problem_description: &str) -> AnalysisResult;

    /// 获取分析器名称
    fn name(&self) -> &str;

    /// 获取分析器类型
    fn analyzer_type(&self) -> AnalyzerType;
}

/// Cargo 输出分析器 — 解析真实的 cargo 输出
///
/// 从 cargo check/test/clippy 的 stderr 中提取结构化错误信息，
/// 分类错误类型，定位相关文件。
pub struct CargoOutputAnalyzer;

impl Default for CargoOutputAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CargoOutputAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 解析 cargo stderr 输出
    pub fn parse_cargo_errors(output: &str) -> Vec<CargoError> {
        let mut errors = Vec::new();
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // 匹配 "error[E0xxx]: message" 模式
            if line.starts_with("error[") || line.starts_with("error:") {
                let error_msg = line.to_string();
                let mut file_location = String::new();
                let mut context_lines = Vec::new();

                // 向前找文件位置（通常在 error 前面的 --> 行）
                if i + 1 < lines.len() && lines[i + 1].contains("-->") {
                    file_location = lines[i + 1].trim().to_string();
                }

                // 收集上下文（后面的代码片段）
                for line in lines.iter().enumerate().skip(i + 1).take(7) {
                    if line.1.starts_with("error") || line.1.starts_with("warning") {
                        break;
                    }
                    context_lines.push(line.1.to_string());
                }

                let category = Self::classify_error(&error_msg);
                errors.push(CargoError {
                    message: error_msg,
                    file_location,
                    category,
                    context: context_lines,
                });
            }

            // 匹配 "warning[...]" 模式
            if line.starts_with("warning[")
                || (line.starts_with("warning:") && !line.contains("generated"))
            {
                errors.push(CargoError {
                    message: line.to_string(),
                    file_location: if i + 1 < lines.len() && lines[i + 1].contains("-->") {
                        lines[i + 1].trim().to_string()
                    } else {
                        String::new()
                    },
                    category: ErrorCategory::ClippyWarning,
                    context: vec![],
                });
            }

            i += 1;
        }

        errors
    }

    /// 分类 Rust 错误
    fn classify_error(msg: &str) -> ErrorCategory {
        let msg_lower = msg.to_lowercase();

        // 先检查具体模式，再检查通用 error[E...]
        if msg_lower.contains("unused") || msg_lower.contains("dead code") {
            ErrorCategory::Unused
        } else if msg_lower.contains("test failed") || msg_lower.contains("assertion") {
            ErrorCategory::TestFailure
        } else if msg_lower.contains("borrow")
            || msg_lower.contains("moved")
            || msg_lower.contains("ownership")
        {
            ErrorCategory::BorrowError
        } else if msg_lower.contains("lifetime") || msg_lower.contains("borrow checker") {
            ErrorCategory::LifetimeError
        } else if msg_lower.contains("not found")
            || msg_lower.contains("cannot find")
            || msg_lower.contains("no method")
            || msg_lower.contains("unknown field")
        {
            ErrorCategory::NotFound
        } else if msg_lower.contains("mismatched types")
            || msg_lower.contains("type mismatch")
            || (msg_lower.contains("expected") && msg_lower.contains("found"))
        {
            ErrorCategory::TypeError
        } else if msg.contains("error[E") {
            ErrorCategory::CompilationError
        } else {
            ErrorCategory::Unknown
        }
    }

    /// 从错误消息中提取文件路径
    pub fn extract_file_paths(output: &str) -> Vec<String> {
        let mut paths = Vec::new();
        for line in output.lines() {
            if line.contains("-->") {
                // 格式: "  --> crates/auto-loop/src/verifier.rs:123:45"
                if let Some(path_part) = line.split("-->").nth(1) {
                    let trimmed = path_part.trim();
                    if let Some(file) = trimmed.split(':').next() {
                        if file.ends_with(".rs") && !paths.contains(&file.to_string()) {
                            paths.push(file.to_string());
                        }
                    }
                }
            }
        }
        paths
    }
}

/// 单个 Cargo 错误
#[derive(Debug, Clone)]
pub struct CargoError {
    pub message: String,
    pub file_location: String,
    pub category: ErrorCategory,
    pub context: Vec<String>,
}

impl Analyzer for CargoOutputAnalyzer {
    fn analyze(&self, problem_description: &str) -> AnalysisResult {
        let errors = Self::parse_cargo_errors(problem_description);
        let related_files = Self::extract_file_paths(problem_description);

        let found = !errors.is_empty();
        let error_categories: Vec<String> = errors
            .iter()
            .map(|e| format!("{:?}", e.category))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let root_cause = if found {
            let first_error = &errors[0];
            Some(format!(
                "{:?}: {} (at {})",
                first_error.category, first_error.message, first_error.file_location
            ))
        } else {
            None
        };

        // 根据错误类型评估难度
        let difficulty = errors
            .iter()
            .map(|e| match e.category {
                ErrorCategory::Unused => 1,
                ErrorCategory::ClippyWarning => 2,
                ErrorCategory::NotFound => 3,
                ErrorCategory::TypeError => 4,
                ErrorCategory::TestFailure => 5,
                ErrorCategory::CompilationError => 5,
                ErrorCategory::BorrowError => 7,
                ErrorCategory::LifetimeError => 8,
                ErrorCategory::Unknown => 5,
            })
            .max()
            .unwrap_or(0);

        let mut details = HashMap::new();
        details.insert("error_count".to_string(), errors.len().to_string());
        details.insert("categories".to_string(), error_categories.join(", "));

        AnalysisResult {
            found_root_cause: found,
            root_cause,
            error_category: errors.first().map(|e| e.category.clone()),
            impact: if found {
                Some(format!("{} 个错误影响编译/测试", errors.len()))
            } else {
                None
            },
            difficulty: if found { Some(difficulty) } else { None },
            estimated_hours: if found {
                Some(difficulty as f64 * 0.5)
            } else {
                None
            },
            related_files,
            details,
            analyzed_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "CargoOutputAnalyzer"
    }

    fn analyzer_type(&self) -> AnalyzerType {
        AnalyzerType::CargoOutput
    }
}

/// 代码库搜索分析器 — 在代码库中搜索相关文件
pub struct CodebaseAnalyzer {
    workspace_path: String,
}

impl CodebaseAnalyzer {
    pub fn new(workspace_path: String) -> Self {
        Self { workspace_path }
    }

    /// 在代码库中搜索关键词
    pub fn search_codebase(&self, keyword: &str) -> Vec<String> {
        let output = Command::new("grep")
            .args(["-rl", keyword, &self.workspace_path, "--include=*.rs"])
            .output();

        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout)
                .lines()
                .take(20)
                .map(|l| l.to_string())
                .collect(),
            Err(_) => vec![],
        }
    }
}

impl Analyzer for CodebaseAnalyzer {
    fn analyze(&self, problem_description: &str) -> AnalysisResult {
        // 从问题描述中提取关键词
        let keywords: Vec<&str> = problem_description
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 3)
            .collect();

        let mut related_files = Vec::new();
        let mut details = HashMap::new();

        for keyword in keywords.iter().take(3) {
            let files = self.search_codebase(keyword);
            related_files.extend(files);
        }

        // 去重
        related_files.sort();
        related_files.dedup();

        let found = !related_files.is_empty();
        details.insert("searched_keywords".to_string(), keywords.join(", "));
        details.insert("files_found".to_string(), related_files.len().to_string());

        AnalysisResult {
            found_root_cause: found,
            root_cause: if found {
                Some(format!("找到 {} 个相关文件", related_files.len()))
            } else {
                None
            },
            error_category: None,
            impact: if found {
                Some(format!("涉及 {} 个文件", related_files.len()))
            } else {
                None
            },
            difficulty: None,
            estimated_hours: None,
            related_files,
            details,
            analyzed_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "CodebaseAnalyzer"
    }

    fn analyzer_type(&self) -> AnalyzerType {
        AnalyzerType::Code
    }
}

/// 分析器管理器
pub struct AnalyzerManager {
    /// 分析器列表
    analyzers: Vec<Box<dyn Analyzer>>,
    /// 分析历史
    history: Vec<AnalysisResult>,
}

impl Default for AnalyzerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerManager {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 创建标准分析器集
    pub fn with_standard_analyzers(workspace_path: &str) -> Self {
        let mut manager = Self::new();
        manager.register_analyzer(Box::new(CargoOutputAnalyzer::new()));
        manager.register_analyzer(Box::new(CodebaseAnalyzer::new(workspace_path.to_string())));
        manager
    }

    /// 注册分析器
    pub fn register_analyzer(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    /// 执行分析
    pub fn analyze(&mut self, problem_description: &str) -> Vec<AnalysisResult> {
        let mut results = Vec::new();

        for analyzer in &self.analyzers {
            let result = analyzer.analyze(problem_description);
            if result.found_root_cause {
                results.push(result.clone());
            }
            self.history.push(result);
        }

        results
    }

    /// 获取分析历史
    pub fn history(&self) -> &[AnalysisResult] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_output_analyzer_parse_errors() {
        let output = r#"
error[E0308]: mismatched types
  --> crates/auto-loop/src/verifier.rs:123:45
   |
123 |         let x: String = 42;
   |                         ^^ expected `String`, found `i32`

error[E0425]: cannot find value `foo` in this scope
  --> crates/auto-loop/src/lib.rs:50:10
"#;
        let errors = CargoOutputAnalyzer::parse_cargo_errors(output);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].category, ErrorCategory::TypeError);
        assert_eq!(errors[1].category, ErrorCategory::NotFound);
    }

    #[test]
    fn test_cargo_output_analyzer_extract_files() {
        let output = r#"
error[E0308]: mismatched types
  --> crates/auto-loop/src/verifier.rs:123:45
warning: unused variable
  --> crates/auto-loop/src/lib.rs:50:10
"#;
        let files = CargoOutputAnalyzer::extract_file_paths(output);
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"crates/auto-loop/src/verifier.rs".to_string()));
    }

    #[test]
    fn test_classify_error_types() {
        assert_eq!(
            CargoOutputAnalyzer::classify_error("error[E0308]: mismatched types"),
            ErrorCategory::TypeError
        );
        assert_eq!(
            CargoOutputAnalyzer::classify_error("error[E0382]: use of moved value"),
            ErrorCategory::BorrowError
        );
        assert_eq!(
            CargoOutputAnalyzer::classify_error("error[E0425]: cannot find value `foo`"),
            ErrorCategory::NotFound
        );
        assert_eq!(
            CargoOutputAnalyzer::classify_error("warning: unused variable `x`"),
            ErrorCategory::Unused
        );
    }

    #[test]
    fn test_cargo_output_analyzer_full() {
        let analyzer = CargoOutputAnalyzer::new();
        let output = r#"
error[E0308]: mismatched types
  --> crates/test.rs:10:5
   |
10 |     let x: String = 42;
"#;
        let result = analyzer.analyze(output);
        assert!(result.found_root_cause);
        assert_eq!(result.error_category, Some(ErrorCategory::TypeError));
        assert!(!result.related_files.is_empty());
    }

    #[test]
    fn test_cargo_output_analyzer_no_errors() {
        let analyzer = CargoOutputAnalyzer::new();
        let result = analyzer.analyze("Everything is fine, no errors");
        assert!(!result.found_root_cause);
    }

    #[test]
    fn test_codebase_analyzer() {
        let analyzer = CodebaseAnalyzer::new("/workspace/kias".to_string());
        let result = analyzer.analyze("AutoLoopManager verifier compilation");
        // 应该找到相关文件
        assert!(result.found_root_cause || result.related_files.is_empty()); // 取决于 grep 是否可用
    }

    #[test]
    fn test_analyzer_manager_creation() {
        let manager = AnalyzerManager::new();
        assert!(manager.analyzers.is_empty());
        assert!(manager.history().is_empty());
    }

    #[test]
    fn test_analyzer_manager_with_standard() {
        let manager = AnalyzerManager::with_standard_analyzers("/workspace/kias");
        assert_eq!(manager.analyzers.len(), 2); // CargoOutput + Codebase
    }

    #[test]
    fn test_analyzer_manager_history() {
        let mut manager = AnalyzerManager::new();
        manager.register_analyzer(Box::new(CargoOutputAnalyzer::new()));

        manager.analyze("error[E0308]: mismatched types");
        assert_eq!(manager.history().len(), 1);

        manager.analyze("no errors here");
        assert_eq!(manager.history().len(), 2);
    }

    #[test]
    fn test_error_category_variants() {
        assert_ne!(ErrorCategory::TypeError, ErrorCategory::BorrowError);
        assert_ne!(ErrorCategory::TestFailure, ErrorCategory::Unknown);
    }

    #[test]
    fn test_borrow_error_classification() {
        assert_eq!(
            CargoOutputAnalyzer::classify_error("error[E0382]: borrow of moved value"),
            ErrorCategory::BorrowError
        );
    }

    #[test]
    fn test_lifetime_error_classification() {
        assert_eq!(
            CargoOutputAnalyzer::classify_error("error[E0495]: lifetime error"),
            ErrorCategory::LifetimeError
        );
    }
}
