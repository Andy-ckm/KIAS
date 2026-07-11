//! Bounded intent classification utilities for the KIAS control plane.
//!
//! This crate classifies and decomposes text deterministically. It does not
//! execute tools, modify code, or run recursive improvement loops.

pub mod intent_recognizer {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub enum IntentType {
        CodeGeneration,
        CodeReview,
        BugFix,
        Documentation,
        TestGeneration,
        ArchitectureDesign,
        PerformanceOptimization,
        SecurityAudit,
        KnowledgeQuery,
        SystemAdmin,
        Unknown,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub enum Complexity {
        Simple,
        Medium,
        Complex,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Priority {
        Low,
        Medium,
        High,
        Critical,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RecognizedIntent {
        pub intent_type: IntentType,
        pub complexity: Complexity,
        pub priority: Priority,
        pub keywords: Vec<String>,
        pub raw_input: String,
        pub confidence: f64,
    }

    #[derive(Debug, Clone)]
    struct Rule {
        intent_type: IntentType,
        keywords: &'static [&'static str],
        base_confidence: f64,
    }

    pub struct IntentRecognizer {
        rules: Vec<Rule>,
    }

    impl IntentRecognizer {
        pub fn new() -> Self {
            Self {
                rules: vec![
                    Rule {
                        intent_type: IntentType::SecurityAudit,
                        keywords: &["security", "安全", "audit", "审计", "vulnerability"],
                        base_confidence: 0.82,
                    },
                    Rule {
                        intent_type: IntentType::BugFix,
                        keywords: &["fix", "修复", "bug", "错误", "异常", "failure"],
                        base_confidence: 0.78,
                    },
                    Rule {
                        intent_type: IntentType::CodeReview,
                        keywords: &["review", "审查", "代码检查", "code review"],
                        base_confidence: 0.8,
                    },
                    Rule {
                        intent_type: IntentType::TestGeneration,
                        keywords: &["test", "测试", "单元测试", "integration test"],
                        base_confidence: 0.76,
                    },
                    Rule {
                        intent_type: IntentType::ArchitectureDesign,
                        keywords: &["architecture", "架构", "design", "设计"],
                        base_confidence: 0.72,
                    },
                    Rule {
                        intent_type: IntentType::PerformanceOptimization,
                        keywords: &["performance", "性能", "optimize", "优化", "latency"],
                        base_confidence: 0.76,
                    },
                    Rule {
                        intent_type: IntentType::Documentation,
                        keywords: &["documentation", "文档", "readme", "说明"],
                        base_confidence: 0.7,
                    },
                    Rule {
                        intent_type: IntentType::SystemAdmin,
                        keywords: &["deploy", "部署", "config", "配置", "system", "系统"],
                        base_confidence: 0.66,
                    },
                    Rule {
                        intent_type: IntentType::KnowledgeQuery,
                        keywords: &["search", "搜索", "query", "查询", "knowledge", "知识"],
                        base_confidence: 0.64,
                    },
                    Rule {
                        intent_type: IntentType::CodeGeneration,
                        keywords: &["implement", "实现", "write", "编写", "create", "开发"],
                        base_confidence: 0.7,
                    },
                ],
            }
        }

        pub fn recognize(&self, input: &str) -> RecognizedIntent {
            const MAX_INPUT_CHARS: usize = 8_192;
            let bounded: String = input.chars().take(MAX_INPUT_CHARS).collect();
            let normalized = bounded.to_lowercase();

            let best = self
                .rules
                .iter()
                .filter_map(|rule| {
                    let matches: Vec<String> = rule
                        .keywords
                        .iter()
                        .filter(|keyword| normalized.contains(&keyword.to_lowercase()))
                        .map(|keyword| (*keyword).to_string())
                        .collect();
                    (!matches.is_empty()).then_some((rule, matches))
                })
                .max_by_key(|(_, matches)| matches.len());

            let (intent_type, keywords, confidence) = match best {
                Some((rule, keywords)) => {
                    let confidence =
                        (rule.base_confidence + keywords.len() as f64 * 0.04).min(0.95);
                    (rule.intent_type.clone(), keywords, confidence)
                }
                None => (IntentType::Unknown, Vec::new(), 0.3),
            };

            RecognizedIntent {
                intent_type,
                complexity: estimate_complexity(&bounded),
                priority: estimate_priority(&normalized),
                keywords,
                raw_input: bounded,
                confidence,
            }
        }
    }

    impl Default for IntentRecognizer {
        fn default() -> Self {
            Self::new()
        }
    }

    fn estimate_complexity(input: &str) -> Complexity {
        let word_count = input.split_whitespace().count();
        let multi_step = ["然后", "并且", "之后", "and then", "followed by"]
            .iter()
            .any(|marker| input.contains(marker));
        let dependency = ["依赖", "先", "before", "depends on"]
            .iter()
            .any(|marker| input.contains(marker));

        if dependency || (multi_step && word_count > 40) {
            Complexity::Complex
        } else if multi_step || word_count > 20 {
            Complexity::Medium
        } else {
            Complexity::Simple
        }
    }

    fn estimate_priority(normalized: &str) -> Priority {
        if ["紧急", "urgent", "immediately", "asap"]
            .iter()
            .any(|keyword| normalized.contains(keyword))
        {
            Priority::Critical
        } else if ["重要", "important", "critical", "优先"]
            .iter()
            .any(|keyword| normalized.contains(keyword))
        {
            Priority::High
        } else {
            Priority::Medium
        }
    }
}

pub mod tool_aware_intent {
    use super::intent_recognizer::{IntentType, RecognizedIntent};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RecommendedTool {
        pub name: String,
        pub score: f64,
        pub reason: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ToolAwareIntent {
        pub base_intent: RecognizedIntent,
        pub recommended_tools: Vec<RecommendedTool>,
    }

    pub struct ToolAwareRecognizer;

    impl ToolAwareRecognizer {
        pub fn new() -> Self {
            Self
        }

        pub fn recognize(&self, _input: &str, intent: RecognizedIntent) -> ToolAwareIntent {
            let tool = match intent.intent_type {
                IntentType::CodeGeneration => Some(("code_builder", "code-generation intent")),
                IntentType::CodeReview => Some(("code_reviewer", "code-review intent")),
                IntentType::BugFix => Some(("diagnostics", "bug-fix intent")),
                IntentType::Documentation => Some(("documentation", "documentation intent")),
                IntentType::TestGeneration => Some(("test_runner", "test intent")),
                IntentType::ArchitectureDesign => Some(("architecture_analysis", "design intent")),
                IntentType::PerformanceOptimization => Some(("profiler", "performance intent")),
                IntentType::SecurityAudit => Some(("security_scanner", "security intent")),
                IntentType::KnowledgeQuery => Some(("knowledge_search", "knowledge-query intent")),
                IntentType::SystemAdmin => Some(("operations", "system-administration intent")),
                IntentType::Unknown => None,
            };

            let recommended_tools = tool
                .map(|(name, reason)| {
                    vec![RecommendedTool {
                        name: name.to_string(),
                        score: intent.confidence,
                        reason: reason.to_string(),
                    }]
                })
                .unwrap_or_default();

            ToolAwareIntent {
                base_intent: intent,
                recommended_tools,
            }
        }
    }

    impl Default for ToolAwareRecognizer {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub mod task_decomposer {
    use super::intent_recognizer::{Complexity, IntentType, RecognizedIntent};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskNode {
        pub id: String,
        pub name: String,
        pub description: String,
        pub dependencies: Vec<String>,
        pub estimated_duration: u64,
        pub required_skills: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskGraph {
        pub nodes: HashMap<String, TaskNode>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DecompositionResult {
        pub task_graph: TaskGraph,
        pub total_estimated_duration: u64,
        pub task_count: usize,
        pub requires_multi_agent: bool,
    }

    pub struct TaskDecomposer;

    impl TaskDecomposer {
        pub fn new() -> Self {
            Self
        }

        pub fn decompose(&self, intent: &RecognizedIntent) -> DecompositionResult {
            let templates = templates_for(&intent.intent_type);
            let mut nodes = HashMap::new();
            let mut previous: Option<String> = None;
            let multiplier = match intent.complexity {
                Complexity::Simple => 1,
                Complexity::Medium => 2,
                Complexity::Complex => 3,
            };

            for (index, (name, description, duration, skills)) in templates.iter().enumerate() {
                let id = format!("task-{}", index + 1);
                let dependencies = previous.iter().cloned().collect();
                nodes.insert(
                    id.clone(),
                    TaskNode {
                        id: id.clone(),
                        name: (*name).to_string(),
                        description: (*description).to_string(),
                        dependencies,
                        estimated_duration: duration * multiplier,
                        required_skills: skills.iter().map(|skill| (*skill).to_string()).collect(),
                    },
                );
                previous = Some(id);
            }

            let total_estimated_duration = nodes.values().map(|task| task.estimated_duration).sum();
            let task_count = nodes.len();

            DecompositionResult {
                task_graph: TaskGraph { nodes },
                total_estimated_duration,
                task_count,
                requires_multi_agent: matches!(intent.complexity, Complexity::Complex),
            }
        }
    }

    impl Default for TaskDecomposer {
        fn default() -> Self {
            Self::new()
        }
    }

    type Template = (&'static str, &'static str, u64, &'static [&'static str]);

    fn templates_for(intent: &IntentType) -> &'static [Template] {
        const IMPLEMENT: &[Template] = &[
            (
                "Analyze requirements",
                "Define scope and interfaces",
                60,
                &["analysis"],
            ),
            (
                "Implement",
                "Create the bounded implementation",
                180,
                &["coding"],
            ),
            (
                "Verify",
                "Run deterministic tests and review",
                120,
                &["testing", "review"],
            ),
        ];
        const FIX: &[Template] = &[
            (
                "Reproduce",
                "Capture the failing behavior",
                60,
                &["debugging"],
            ),
            ("Diagnose", "Identify the root cause", 90, &["analysis"]),
            (
                "Repair",
                "Implement the smallest safe fix",
                120,
                &["coding"],
            ),
            (
                "Regress",
                "Run focused and regression tests",
                90,
                &["testing"],
            ),
        ];
        const REVIEW: &[Template] = &[
            (
                "Inspect",
                "Run static and structural analysis",
                60,
                &["analysis"],
            ),
            (
                "Review",
                "Evaluate correctness, security, and maintainability",
                120,
                &["review"],
            ),
            (
                "Report",
                "Produce prioritized findings",
                60,
                &["documentation"],
            ),
        ];
        const OPERATE: &[Template] = &[
            (
                "Assess",
                "Validate desired state and constraints",
                60,
                &["operations"],
            ),
            (
                "Plan",
                "Prepare a reversible execution plan",
                90,
                &["analysis"],
            ),
            (
                "Verify",
                "Define health and rollback checks",
                60,
                &["testing"],
            ),
        ];
        const GENERIC: &[Template] = &[
            (
                "Clarify",
                "Clarify the requested outcome and constraints",
                60,
                &["analysis"],
            ),
            ("Plan", "Create a bounded execution plan", 90, &["planning"]),
        ];

        match intent {
            IntentType::BugFix => FIX,
            IntentType::CodeReview | IntentType::SecurityAudit => REVIEW,
            IntentType::SystemAdmin => OPERATE,
            IntentType::CodeGeneration
            | IntentType::TestGeneration
            | IntentType::Documentation
            | IntentType::ArchitectureDesign
            | IntentType::PerformanceOptimization => IMPLEMENT,
            IntentType::KnowledgeQuery | IntentType::Unknown => GENERIC,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::intent_recognizer::{IntentRecognizer, IntentType};
    use super::task_decomposer::TaskDecomposer;

    #[test]
    fn classifier_is_bounded_and_deterministic() {
        let recognizer = IntentRecognizer::new();
        let first = recognizer.recognize("urgent security audit of the API");
        let second = recognizer.recognize("urgent security audit of the API");
        assert_eq!(first.intent_type, IntentType::SecurityAudit);
        assert_eq!(
            format!("{:?}", first.intent_type),
            format!("{:?}", second.intent_type)
        );
        assert_eq!(first.confidence, second.confidence);
    }

    #[test]
    fn decomposition_has_bounded_dependencies() {
        let recognizer = IntentRecognizer::new();
        let intent = recognizer.recognize("fix the API authentication bug");
        let result = TaskDecomposer::new().decompose(&intent);
        assert!((2..=6).contains(&result.task_count));
        assert_eq!(result.task_count, result.task_graph.nodes.len());
    }
}
