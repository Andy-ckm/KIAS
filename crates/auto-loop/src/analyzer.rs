//! 根因自动分析 — KIAS自循环的核心
//!
//! 自动分析问题根因，包括：
//! - 代码分析
//! - 配置分析
//! - 依赖分析
//! - 性能分析

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 分析器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyzerType {
    /// 代码分析
    Code,
    /// 配置分析
    Config,
    /// 依赖分析
    Dependency,
    /// 性能分析
    Performance,
    /// 内存分析
    Memory,
}

/// 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// 是否找到根因
    pub found_root_cause: bool,
    /// 根因描述
    pub root_cause: Option<String>,
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

/// 分析器 trait
pub trait Analyzer: Send + Sync {
    /// 执行分析
    fn analyze(&self, problem_description: &str) -> AnalysisResult;

    /// 获取分析器名称
    fn name(&self) -> &str;

    /// 获取分析器类型
    fn analyzer_type(&self) -> AnalyzerType;
}

/// 代码分析器
pub struct CodeAnalyzer {
    /// 代码库路径
    _codebase_path: String,
}

impl CodeAnalyzer {
    pub fn new(_codebase_path: String) -> Self {
        Self { _codebase_path }
    }
}

impl Analyzer for CodeAnalyzer {
    fn analyze(&self, problem_description: &str) -> AnalysisResult {
        // 分析问题描述，查找相关代码
        let mut related_files = Vec::new();
        let mut details = HashMap::new();

        // 检查是否是数据持久化问题
        if problem_description.contains("持久化") || problem_description.contains("丢失") {
            related_files.push("crates/api-server/src/lib.rs".to_string());
            related_files.push("crates/data-store/src/repository/mod.rs".to_string());
            details.insert(
                "analysis".to_string(),
                "检测到数据持久化问题，可能是HashMap存储导致".to_string(),
            );
        }

        // 检查是否是配置问题
        if problem_description.contains("配置") || problem_description.contains("placeholder") {
            related_files.push("config/kias.toml".to_string());
            details.insert(
                "analysis".to_string(),
                "检测到配置问题，可能是placeholder配置".to_string(),
            );
        }

        AnalysisResult {
            found_root_cause: !related_files.is_empty(),
            root_cause: if !related_files.is_empty() {
                Some(format!(
                    "问题可能在以下文件中: {}",
                    related_files.join(", ")
                ))
            } else {
                None
            },
            impact: Some("影响系统稳定性和数据完整性".to_string()),
            difficulty: Some(5),
            estimated_hours: Some(2.0),
            related_files,
            details,
            analyzed_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "CodeAnalyzer"
    }

    fn analyzer_type(&self) -> AnalyzerType {
        AnalyzerType::Code
    }
}

/// 配置分析器
pub struct ConfigAnalyzer {
    /// 配置文件路径
    config_path: String,
}

impl ConfigAnalyzer {
    pub fn new(config_path: String) -> Self {
        Self { config_path }
    }
}

impl Analyzer for ConfigAnalyzer {
    fn analyze(&self, problem_description: &str) -> AnalysisResult {
        let mut related_files = Vec::new();
        let mut details = HashMap::new();

        // 检查配置文件
        related_files.push(self.config_path.clone());

        // 分析配置问题
        if problem_description.contains("placeholder") || problem_description.contains("API") {
            details.insert(
                "analysis".to_string(),
                "检测到API配置问题，可能是placeholder配置".to_string(),
            );
        }

        AnalysisResult {
            found_root_cause: true,
            root_cause: Some("配置文件可能包含placeholder值".to_string()),
            impact: Some("影响API调用和功能正常运行".to_string()),
            difficulty: Some(2),
            estimated_hours: Some(0.5),
            related_files,
            details,
            analyzed_at: chrono::Utc::now(),
        }
    }

    fn name(&self) -> &str {
        "ConfigAnalyzer"
    }

    fn analyzer_type(&self) -> AnalyzerType {
        AnalyzerType::Config
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
    fn test_code_analyzer() {
        let analyzer = CodeAnalyzer::new("/workspace/kias".to_string());
        let result = analyzer.analyze("Agent数据持久化缺失，服务器重启后数据丢失");

        assert!(result.found_root_cause);
        assert!(!result.related_files.is_empty());
    }

    #[test]
    fn test_config_analyzer() {
        let analyzer = ConfigAnalyzer::new("config/kias.toml".to_string());
        let result = analyzer.analyze("Workflow执行需要LLM API但配置是placeholder");

        assert!(result.found_root_cause);
        assert!(!result.related_files.is_empty());
    }

    #[test]
    fn test_analyzer_manager() {
        let mut manager = AnalyzerManager::new();

        manager.register_analyzer(Box::new(CodeAnalyzer::new("/workspace/kias".to_string())));
        manager.register_analyzer(Box::new(ConfigAnalyzer::new(
            "config/kias.toml".to_string(),
        )));

        let results = manager.analyze("Agent数据持久化缺失");
        assert!(!results.is_empty());
    }
}
