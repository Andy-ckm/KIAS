//! Intent Recognizer — 自然语言意图识别
//!
//! 将用户自然语言输入转化为结构化 Intent。
//!
//! # 参考来源
//! - DeepResearchAgent: 分层意图分类
//! - Dify Agent 工作流: 意图路由
//! - AgentRouter: 意图分类状态机

use serde::{Deserialize, Serialize};

/// 意图类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IntentType {
    /// 代码生成
    CodeGeneration,
    /// 代码审查
    CodeReview,
    /// Bug 修复
    BugFix,
    /// 文档生成
    Documentation,
    /// 测试生成
    TestGeneration,
    /// 架构设计
    ArchitectureDesign,
    /// 性能优化
    PerformanceOptimization,
    /// 安全审计
    SecurityAudit,
    /// 知识查询
    KnowledgeQuery,
    /// 系统管理
    SystemAdmin,
    /// 未知意图
    Unknown,
}

/// 复杂度等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Complexity {
    /// 简单任务（单步）
    Simple,
    /// 中等任务（2-5步）
    Medium,
    /// 复杂任务（5+步，需多Agent协作）
    Complex,
}

/// 优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// 识别后的意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizedIntent {
    /// 意图类型
    pub intent_type: IntentType,
    /// 复杂度
    pub complexity: Complexity,
    /// 优先级
    pub priority: Priority,
    /// 关键词
    pub keywords: Vec<String>,
    /// 原始输入
    pub raw_input: String,
    /// 置信度 (0.0-1.0)
    pub confidence: f64,
}

/// 意图识别器
pub struct IntentRecognizer {
    /// 关键词映射
    keyword_rules: Vec<KeywordRule>,
}

/// 关键词规则
struct KeywordRule {
    keywords: Vec<String>,
    intent_type: IntentType,
    base_confidence: f64,
}

impl RecognizedIntent {
    /// 获取意图类型名称
    pub fn intent_type_name(&self) -> &'static str {
        self.intent_type.type_name()
    }
}

impl IntentRecognizer {
    /// 创建默认识别器
    pub fn new() -> Self {
        Self {
            keyword_rules: Self::default_rules(),
        }
    }

    /// 默认关键词规则
    fn default_rules() -> Vec<KeywordRule> {
        vec![
            KeywordRule {
                keywords: vec![
                    "写代码".into(),
                    "实现".into(),
                    "编写".into(),
                    "开发".into(),
                    "create".into(),
                    "implement".into(),
                    "write".into(),
                ],
                intent_type: IntentType::CodeGeneration,
                base_confidence: 0.7,
            },
            KeywordRule {
                keywords: vec![
                    "审查".into(),
                    "review".into(),
                    "检查代码".into(),
                    "code review".into(),
                ],
                intent_type: IntentType::CodeReview,
                base_confidence: 0.8,
            },
            KeywordRule {
                keywords: vec![
                    "修复".into(),
                    "fix".into(),
                    "bug".into(),
                    "错误".into(),
                    "异常".into(),
                ],
                intent_type: IntentType::BugFix,
                base_confidence: 0.75,
            },
            KeywordRule {
                keywords: vec!["文档".into(), "doc".into(), "readme".into(), "说明".into()],
                intent_type: IntentType::Documentation,
                base_confidence: 0.7,
            },
            KeywordRule {
                keywords: vec![
                    "测试".into(),
                    "test".into(),
                    "单元测试".into(),
                    "集成测试".into(),
                ],
                intent_type: IntentType::TestGeneration,
                base_confidence: 0.75,
            },
            KeywordRule {
                keywords: vec![
                    "架构".into(),
                    "设计".into(),
                    "architecture".into(),
                    "design".into(),
                ],
                intent_type: IntentType::ArchitectureDesign,
                base_confidence: 0.7,
            },
            KeywordRule {
                keywords: vec![
                    "性能".into(),
                    "优化".into(),
                    "performance".into(),
                    "optimize".into(),
                ],
                intent_type: IntentType::PerformanceOptimization,
                base_confidence: 0.75,
            },
            KeywordRule {
                keywords: vec![
                    "安全".into(),
                    "security".into(),
                    "审计".into(),
                    "audit".into(),
                ],
                intent_type: IntentType::SecurityAudit,
                base_confidence: 0.8,
            },
            KeywordRule {
                keywords: vec![
                    "查询".into(),
                    "搜索".into(),
                    "search".into(),
                    "query".into(),
                    "知识".into(),
                ],
                intent_type: IntentType::KnowledgeQuery,
                base_confidence: 0.6,
            },
            KeywordRule {
                keywords: vec![
                    "部署".into(),
                    "deploy".into(),
                    "配置".into(),
                    "config".into(),
                    "系统".into(),
                ],
                intent_type: IntentType::SystemAdmin,
                base_confidence: 0.65,
            },
        ]
    }

    /// 识别意图
    pub fn recognize(&self, input: &str) -> RecognizedIntent {
        let input_lower = input.to_lowercase();
        let mut best_match: Option<(&KeywordRule, usize)> = None;

        for rule in &self.keyword_rules {
            let match_count = rule
                .keywords
                .iter()
                .filter(|kw| input_lower.contains(&kw.to_lowercase()))
                .count();

            if match_count > 0 && (best_match.is_none() || match_count > best_match.unwrap().1) {
                best_match = Some((rule, match_count));
            }
        }

        if let Some((rule, match_count)) = best_match {
            let confidence = (rule.base_confidence + (match_count as f64 * 0.05)).min(1.0);
            let keywords: Vec<String> = rule
                .keywords
                .iter()
                .filter(|kw| input_lower.contains(&kw.to_lowercase()))
                .cloned()
                .collect();

            RecognizedIntent {
                intent_type: rule.intent_type.clone(),
                complexity: self.estimate_complexity(input),
                priority: self.estimate_priority(input),
                keywords,
                raw_input: input.to_string(),
                confidence,
            }
        } else {
            RecognizedIntent {
                intent_type: IntentType::Unknown,
                complexity: self.estimate_complexity(input),
                priority: Priority::Medium,
                keywords: vec![],
                raw_input: input.to_string(),
                confidence: 0.3,
            }
        }
    }

    /// 估算复杂度
    fn estimate_complexity(&self, input: &str) -> Complexity {
        let word_count = input.split_whitespace().count();
        let has_multiple_tasks =
            input.contains("然后") || input.contains("并且") || input.contains("and then");
        let has_dependencies =
            input.contains("依赖") || input.contains("先") || input.contains("之后");

        if has_dependencies || (has_multiple_tasks && word_count > 50) {
            Complexity::Complex
        } else if has_multiple_tasks || word_count > 20 {
            Complexity::Medium
        } else {
            Complexity::Simple
        }
    }

    /// 估算优先级
    fn estimate_priority(&self, input: &str) -> Priority {
        let urgent_keywords = ["紧急", "urgent", "立即", "immediately", "ASAP", "马上"];
        let high_keywords = ["重要", "important", "关键", "critical", "优先"];

        if urgent_keywords
            .iter()
            .any(|kw| input.to_lowercase().contains(&kw.to_lowercase()))
        {
            Priority::Critical
        } else if high_keywords
            .iter()
            .any(|kw| input.to_lowercase().contains(&kw.to_lowercase()))
        {
            Priority::High
        } else {
            Priority::Medium
        }
    }
}

impl Default for IntentRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_generation_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("请帮我实现一个 Rust 的 HTTP 服务器");
        assert_eq!(intent.intent_type, IntentType::CodeGeneration);
        assert!(intent.confidence > 0.5);
    }

    #[test]
    fn test_bug_fix_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("修复这个 bug：空指针异常");
        assert_eq!(intent.intent_type, IntentType::BugFix);
    }

    #[test]
    fn test_code_review_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("请审查这段代码的质量");
        assert_eq!(intent.intent_type, IntentType::CodeReview);
    }

    #[test]
    fn test_test_generation_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("为这个模块编写单元测试");
        assert_eq!(intent.intent_type, IntentType::TestGeneration);
    }

    #[test]
    fn test_security_audit_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("对系统进行安全审计");
        assert_eq!(intent.intent_type, IntentType::SecurityAudit);
    }

    #[test]
    fn test_complex_task() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("先实现用户认证模块，然后添加权限管理，之后集成到主系统");
        assert_eq!(intent.complexity, Complexity::Complex);
    }

    #[test]
    fn test_urgent_priority() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("紧急修复生产环境的崩溃问题");
        assert_eq!(intent.priority, Priority::Critical);
    }

    #[test]
    fn test_unknown_intent() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("今天天气怎么样");
        assert_eq!(intent.intent_type, IntentType::Unknown);
        assert!(intent.confidence < 0.5);
    }

    #[test]
    fn test_keyword_extraction() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("实现一个缓存系统并编写测试");
        assert!(!intent.keywords.is_empty());
    }

    #[test]
    fn test_default_recognizer() {
        let recognizer = IntentRecognizer::default();
        let intent = recognizer.recognize("写代码");
        assert_eq!(intent.intent_type, IntentType::CodeGeneration);
    }
}
