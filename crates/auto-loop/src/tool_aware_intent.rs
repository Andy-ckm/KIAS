//! Tool-aware Intent — 工具感知的意图识别
//!
//! 识别意图时考虑可用工具，参考 Toolformer 论文。
//!
//! # 论文支撑
//! - Toolformer (Schick et al., 2023): LLM自主学习何时调用工具
//! - HuggingGPT (Shen et al., 2023): LLM控制器→任务拆解→模型分配

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::intent_recognizer::{IntentType, RecognizedIntent};

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 工具能力
    pub capabilities: Vec<String>,
    /// 支持的意图类型
    pub supported_intents: Vec<IntentType>,
    /// 工具参数
    pub parameters: Vec<ToolParameter>,
}

/// 工具参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// 参数名
    pub name: String,
    /// 参数类型
    pub param_type: String,
    /// 是否必需
    pub required: bool,
    /// 参数描述
    pub description: String,
}

/// 工具感知的意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAwareIntent {
    /// 基础意图
    pub base_intent: RecognizedIntent,
    /// 推荐的工具
    pub recommended_tools: Vec<RecommendedTool>,
    /// 工具调用建议
    pub tool_calls: Vec<ToolCallSuggestion>,
    /// 是否需要工具
    pub needs_tools: bool,
}

/// 推荐的工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedTool {
    /// 工具名称
    pub name: String,
    /// 匹配分数 (0.0-1.0)
    pub score: f64,
    /// 匹配原因
    pub reason: String,
}

/// 工具调用建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSuggestion {
    /// 工具名称
    pub tool_name: String,
    /// 调用时机
    pub when: String,
    /// 预期参数
    pub expected_params: HashMap<String, String>,
}

/// 工具感知的意图识别器
pub struct ToolAwareRecognizer {
    /// 可用工具
    tools: Vec<ToolDefinition>,
    /// 关键词到工具的映射
    keyword_tool_map: HashMap<String, Vec<String>>,
    /// 意图到工具的映射
    intent_tool_map: HashMap<IntentType, Vec<String>>,
}

impl ToolAwareRecognizer {
    /// 创建新的识别器
    pub fn new() -> Self {
        let mut recognizer = Self {
            tools: Vec::new(),
            keyword_tool_map: HashMap::new(),
            intent_tool_map: HashMap::new(),
        };
        recognizer.register_default_tools();
        recognizer
    }

    /// 注册默认工具
    fn register_default_tools(&mut self) {
        // 代码生成工具
        self.register_tool(ToolDefinition {
            name: "codegen".into(),
            description: "代码生成工具，支持多种编程语言".into(),
            capabilities: vec![
                "generate_code".into(),
                "refactor".into(),
                "translate".into(),
            ],
            supported_intents: vec![IntentType::CodeGeneration],
            parameters: vec![
                ToolParameter {
                    name: "language".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "目标编程语言".into(),
                },
                ToolParameter {
                    name: "specification".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "代码规格说明".into(),
                },
            ],
        });

        // 测试工具
        self.register_tool(ToolDefinition {
            name: "testgen".into(),
            description: "测试生成工具，支持单元测试和集成测试".into(),
            capabilities: vec!["generate_tests".into(), "run_tests".into()],
            supported_intents: vec![IntentType::TestGeneration],
            parameters: vec![
                ToolParameter {
                    name: "test_type".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "测试类型（unit/integration/e2e）".into(),
                },
                ToolParameter {
                    name: "source_file".into(),
                    param_type: "string".into(),
                    required: false,
                    description: "源代码文件路径".into(),
                },
            ],
        });

        // 代码审查工具
        self.register_tool(ToolDefinition {
            name: "reviewer".into(),
            description: "代码审查工具，检查代码质量和规范".into(),
            capabilities: vec!["review_code".into(), "find_issues".into()],
            supported_intents: vec![IntentType::CodeReview],
            parameters: vec![
                ToolParameter {
                    name: "file_path".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "代码文件路径".into(),
                },
                ToolParameter {
                    name: "rules".into(),
                    param_type: "array".into(),
                    required: false,
                    description: "审查规则".into(),
                },
            ],
        });

        // 调试工具
        self.register_tool(ToolDefinition {
            name: "debugger".into(),
            description: "调试工具，帮助定位和修复问题".into(),
            capabilities: vec!["analyze_logs".into(), "find_root_cause".into()],
            supported_intents: vec![IntentType::BugFix],
            parameters: vec![
                ToolParameter {
                    name: "error_log".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "错误日志".into(),
                },
                ToolParameter {
                    name: "context".into(),
                    param_type: "string".into(),
                    required: false,
                    description: "上下文信息".into(),
                },
            ],
        });

        // 文档工具
        self.register_tool(ToolDefinition {
            name: "docgen".into(),
            description: "文档生成工具，支持多种文档格式".into(),
            capabilities: vec!["generate_docs".into(), "update_docs".into()],
            supported_intents: vec![IntentType::Documentation],
            parameters: vec![
                ToolParameter {
                    name: "format".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "文档格式（markdown/html/pdf）".into(),
                },
                ToolParameter {
                    name: "source".into(),
                    param_type: "string".into(),
                    required: false,
                    description: "源代码或配置文件".into(),
                },
            ],
        });

        // 安全审计工具
        self.register_tool(ToolDefinition {
            name: "security_scanner".into(),
            description: "安全扫描工具，检测漏洞和风险".into(),
            capabilities: vec!["scan_vulnerabilities".into(), "audit_code".into()],
            supported_intents: vec![IntentType::SecurityAudit],
            parameters: vec![
                ToolParameter {
                    name: "scan_type".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "扫描类型（full/quick/targeted）".into(),
                },
                ToolParameter {
                    name: "target".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "扫描目标".into(),
                },
            ],
        });

        // 性能分析工具
        self.register_tool(ToolDefinition {
            name: "profiler".into(),
            description: "性能分析工具，定位瓶颈".into(),
            capabilities: vec!["profile_cpu".into(), "profile_memory".into()],
            supported_intents: vec![IntentType::PerformanceOptimization],
            parameters: vec![
                ToolParameter {
                    name: "metric".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "性能指标（cpu/memory/network）".into(),
                },
                ToolParameter {
                    name: "duration".into(),
                    param_type: "number".into(),
                    required: false,
                    description: "分析时长（秒）".into(),
                },
            ],
        });

        // 知识检索工具
        self.register_tool(ToolDefinition {
            name: "knowledge_search".into(),
            description: "知识检索工具，支持向量和关键词搜索".into(),
            capabilities: vec!["search_knowledge".into(), "query_docs".into()],
            supported_intents: vec![IntentType::KnowledgeQuery],
            parameters: vec![
                ToolParameter {
                    name: "query".into(),
                    param_type: "string".into(),
                    required: true,
                    description: "搜索查询".into(),
                },
                ToolParameter {
                    name: "limit".into(),
                    param_type: "number".into(),
                    required: false,
                    description: "结果数量限制".into(),
                },
            ],
        });

        // 设置关键词到工具的映射
        self.keyword_tool_map
            .insert("代码".into(), vec!["codegen".into()]);
        self.keyword_tool_map
            .insert("code".into(), vec!["codegen".into()]);
        self.keyword_tool_map
            .insert("测试".into(), vec!["testgen".into()]);
        self.keyword_tool_map
            .insert("test".into(), vec!["testgen".into()]);
        self.keyword_tool_map
            .insert("审查".into(), vec!["reviewer".into()]);
        self.keyword_tool_map
            .insert("review".into(), vec!["reviewer".into()]);
        self.keyword_tool_map
            .insert("修复".into(), vec!["debugger".into()]);
        self.keyword_tool_map
            .insert("fix".into(), vec!["debugger".into()]);
        self.keyword_tool_map
            .insert("bug".into(), vec!["debugger".into()]);
        self.keyword_tool_map
            .insert("文档".into(), vec!["docgen".into()]);
        self.keyword_tool_map
            .insert("doc".into(), vec!["docgen".into()]);
        self.keyword_tool_map
            .insert("安全".into(), vec!["security_scanner".into()]);
        self.keyword_tool_map
            .insert("security".into(), vec!["security_scanner".into()]);
        self.keyword_tool_map
            .insert("性能".into(), vec!["profiler".into()]);
        self.keyword_tool_map
            .insert("performance".into(), vec!["profiler".into()]);
        self.keyword_tool_map
            .insert("搜索".into(), vec!["knowledge_search".into()]);
        self.keyword_tool_map
            .insert("search".into(), vec!["knowledge_search".into()]);

        // 设置意图到工具的映射
        self.intent_tool_map
            .insert(IntentType::CodeGeneration, vec!["codegen".into()]);
        self.intent_tool_map
            .insert(IntentType::TestGeneration, vec!["testgen".into()]);
        self.intent_tool_map
            .insert(IntentType::CodeReview, vec!["reviewer".into()]);
        self.intent_tool_map
            .insert(IntentType::BugFix, vec!["debugger".into()]);
        self.intent_tool_map
            .insert(IntentType::Documentation, vec!["docgen".into()]);
        self.intent_tool_map
            .insert(IntentType::SecurityAudit, vec!["security_scanner".into()]);
        self.intent_tool_map
            .insert(IntentType::PerformanceOptimization, vec!["profiler".into()]);
        self.intent_tool_map
            .insert(IntentType::KnowledgeQuery, vec!["knowledge_search".into()]);
    }

    /// 注册新工具
    pub fn register_tool(&mut self, tool: ToolDefinition) {
        // 更新意图到工具的映射
        for intent in &tool.supported_intents {
            self.intent_tool_map
                .entry(intent.clone())
                .or_default()
                .push(tool.name.clone());
        }
        self.tools.push(tool);
    }

    /// 识别工具感知的意图
    pub fn recognize(&self, input: &str, base_intent: RecognizedIntent) -> ToolAwareIntent {
        let input_lower = input.to_lowercase();

        // 1. 基于关键词推荐工具
        let keyword_tools: Vec<String> = self
            .keyword_tool_map
            .iter()
            .filter(|(keyword, _)| input_lower.contains(&keyword.to_lowercase()))
            .flat_map(|(_, tools)| tools.clone())
            .collect();

        // 2. 基于意图类型推荐工具
        let intent_tools: Vec<String> = self
            .intent_tool_map
            .get(&base_intent.intent_type)
            .cloned()
            .unwrap_or_default();

        // 3. 合并推荐工具
        let mut all_tools: Vec<String> = keyword_tools;
        all_tools.extend(intent_tools);
        all_tools.sort();
        all_tools.dedup();

        // 4. 计算匹配分数
        let recommended_tools: Vec<RecommendedTool> = all_tools
            .iter()
            .filter_map(|tool_name| {
                self.tools
                    .iter()
                    .find(|t| t.name == *tool_name)
                    .map(|tool| {
                        let score = self.calculate_match_score(tool, input, &base_intent);
                        let reason = self.generate_match_reason(tool, input, &base_intent);
                        RecommendedTool {
                            name: tool.name.clone(),
                            score,
                            reason,
                        }
                    })
            })
            .collect();

        // 5. 生成工具调用建议
        let tool_calls: Vec<ToolCallSuggestion> = recommended_tools
            .iter()
            .filter(|r| r.score > 0.5)
            .filter_map(|r| {
                self.tools.iter().find(|t| t.name == r.name).map(|tool| {
                    let expected_params = self.infer_parameters(tool, input);
                    ToolCallSuggestion {
                        tool_name: tool.name.clone(),
                        when: format!("当需要{}时", tool.description),
                        expected_params,
                    }
                })
            })
            .collect();

        let needs_tools = !recommended_tools.is_empty();

        ToolAwareIntent {
            base_intent,
            recommended_tools,
            tool_calls,
            needs_tools,
        }
    }

    /// 计算匹配分数
    fn calculate_match_score(
        &self,
        tool: &ToolDefinition,
        input: &str,
        intent: &RecognizedIntent,
    ) -> f64 {
        let mut score = 0.0;

        // 基于意图类型匹配
        if tool.supported_intents.contains(&intent.intent_type) {
            score += 0.4;
        }

        // 基于关键词匹配
        let input_lower = input.to_lowercase();
        let keyword_matches = self
            .keyword_tool_map
            .iter()
            .filter(|(keyword, tools)| {
                input_lower.contains(&keyword.to_lowercase()) && tools.contains(&tool.name)
            })
            .count();
        score += (keyword_matches as f64 * 0.1).min(0.3);

        // 基于置信度
        score += intent.confidence * 0.3;

        score.min(1.0)
    }

    /// 生成匹配原因
    fn generate_match_reason(
        &self,
        tool: &ToolDefinition,
        input: &str,
        intent: &RecognizedIntent,
    ) -> String {
        let mut reasons = Vec::new();

        if tool.supported_intents.contains(&intent.intent_type) {
            reasons.push(format!("支持{}意图", intent.intent_type_name()));
        }

        let input_lower = input.to_lowercase();
        for (keyword, tools) in &self.keyword_tool_map {
            if input_lower.contains(&keyword.to_lowercase()) && tools.contains(&tool.name) {
                reasons.push(format!("包含关键词'{}'", keyword));
            }
        }

        if reasons.is_empty() {
            "通用匹配".to_string()
        } else {
            reasons.join("，")
        }
    }

    /// 推断参数
    fn infer_parameters(&self, tool: &ToolDefinition, input: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();

        for param in &tool.parameters {
            match param.name.as_str() {
                "language" => {
                    if input.to_lowercase().contains("rust") {
                        params.insert("language".into(), "rust".into());
                    } else if input.to_lowercase().contains("python") {
                        params.insert("language".into(), "python".into());
                    } else if input.to_lowercase().contains("javascript")
                        || input.to_lowercase().contains("js")
                    {
                        params.insert("language".into(), "javascript".into());
                    } else {
                        params.insert("language".into(), "auto".into());
                    }
                }
                "test_type" => {
                    if input.to_lowercase().contains("单元")
                        || input.to_lowercase().contains("unit")
                    {
                        params.insert("test_type".into(), "unit".into());
                    } else if input.to_lowercase().contains("集成")
                        || input.to_lowercase().contains("integration")
                    {
                        params.insert("test_type".into(), "integration".into());
                    } else {
                        params.insert("test_type".into(), "unit".into());
                    }
                }
                "format" => {
                    if input.to_lowercase().contains("markdown")
                        || input.to_lowercase().contains("md")
                    {
                        params.insert("format".into(), "markdown".into());
                    } else if input.to_lowercase().contains("html") {
                        params.insert("format".into(), "html".into());
                    } else {
                        params.insert("format".into(), "markdown".into());
                    }
                }
                "scan_type" => {
                    if input.to_lowercase().contains("全面")
                        || input.to_lowercase().contains("full")
                    {
                        params.insert("scan_type".into(), "full".into());
                    } else {
                        params.insert("scan_type".into(), "quick".into());
                    }
                }
                "metric" => {
                    if input.to_lowercase().contains("cpu") {
                        params.insert("metric".into(), "cpu".into());
                    } else if input.to_lowercase().contains("内存")
                        || input.to_lowercase().contains("memory")
                    {
                        params.insert("metric".into(), "memory".into());
                    } else {
                        params.insert("metric".into(), "cpu".into());
                    }
                }
                _ => {}
            }
        }

        params
    }
}

/// IntentType 的辅助方法
impl IntentType {
    /// 获取意图类型名称
    pub fn type_name(&self) -> &'static str {
        match self {
            IntentType::CodeGeneration => "CodeGeneration",
            IntentType::CodeReview => "CodeReview",
            IntentType::BugFix => "BugFix",
            IntentType::Documentation => "Documentation",
            IntentType::TestGeneration => "TestGeneration",
            IntentType::ArchitectureDesign => "ArchitectureDesign",
            IntentType::PerformanceOptimization => "PerformanceOptimization",
            IntentType::SecurityAudit => "SecurityAudit",
            IntentType::KnowledgeQuery => "KnowledgeQuery",
            IntentType::SystemAdmin => "SystemAdmin",
            IntentType::Unknown => "Unknown",
        }
    }
}

impl Default for ToolAwareRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_recognizer::{Complexity, Priority};

    #[test]
    fn test_tool_aware_recognizer_new() {
        let recognizer = ToolAwareRecognizer::new();
        assert!(!recognizer.tools.is_empty());
    }

    #[test]
    fn test_recognize_code_generation() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec!["代码".into()],
            raw_input: "请帮我写一个Rust的HTTP服务器".into(),
            confidence: 0.8,
        };
        let result = recognizer.recognize("请帮我写一个Rust的HTTP服务器", base_intent);
        assert!(result.needs_tools);
        assert!(!result.recommended_tools.is_empty());
    }

    #[test]
    fn test_recognize_bug_fix() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::BugFix,
            complexity: Complexity::Medium,
            priority: Priority::High,
            keywords: vec!["修复".into(), "bug".into()],
            raw_input: "修复这个空指针异常".into(),
            confidence: 0.9,
        };
        let result = recognizer.recognize("修复这个空指针异常", base_intent);
        assert!(result.needs_tools);
        assert!(result
            .recommended_tools
            .iter()
            .any(|t| t.name == "debugger"));
    }

    #[test]
    fn test_recognize_with_keyword() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::TestGeneration,
            complexity: Complexity::Simple,
            priority: Priority::Medium,
            keywords: vec!["测试".into()],
            raw_input: "为这个模块编写单元测试".into(),
            confidence: 0.7,
        };
        let result = recognizer.recognize("为这个模块编写单元测试", base_intent);
        assert!(result.recommended_tools.iter().any(|t| t.name == "testgen"));
    }

    #[test]
    fn test_parameter_inference() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "用Rust写一个HTTP服务器".into(),
            confidence: 0.8,
        };
        let result = recognizer.recognize("用Rust写一个HTTP服务器", base_intent);
        assert!(!result.tool_calls.is_empty());
    }

    #[test]
    fn test_register_custom_tool() {
        let mut recognizer = ToolAwareRecognizer::new();
        let tool = ToolDefinition {
            name: "custom_tool".into(),
            description: "自定义工具".into(),
            capabilities: vec!["custom_action".into()],
            supported_intents: vec![IntentType::SystemAdmin],
            parameters: vec![],
        };
        recognizer.register_tool(tool);
        assert!(recognizer.tools.iter().any(|t| t.name == "custom_tool"));
    }

    #[test]
    fn test_no_tools_needed() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::Unknown,
            complexity: Complexity::Simple,
            priority: Priority::Low,
            keywords: vec![],
            raw_input: "今天天气怎么样".into(),
            confidence: 0.3,
        };
        let result = recognizer.recognize("今天天气怎么样", base_intent);
        assert!(!result.needs_tools);
    }

    #[test]
    fn test_multiple_tools() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Complex,
            priority: Priority::High,
            keywords: vec!["代码".into(), "测试".into()],
            raw_input: "写代码并编写测试".into(),
            confidence: 0.9,
        };
        let result = recognizer.recognize("写代码并编写测试", base_intent);
        assert!(result.recommended_tools.len() > 1);
    }

    #[test]
    fn test_match_score() {
        let recognizer = ToolAwareRecognizer::new();
        let tool = recognizer
            .tools
            .iter()
            .find(|t| t.name == "codegen")
            .unwrap();
        let intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "test".into(),
            confidence: 0.8,
        };
        let score = recognizer.calculate_match_score(tool, "写代码", &intent);
        assert!(score > 0.0);
    }

    #[test]
    fn test_recognize_security_audit() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::SecurityAudit,
            complexity: Complexity::Complex,
            priority: Priority::High,
            keywords: vec!["安全".into()],
            raw_input: "对系统进行安全扫描".into(),
            confidence: 0.85,
        };
        let result = recognizer.recognize("对系统进行安全扫描", base_intent);
        assert!(result.needs_tools);
        assert!(result
            .recommended_tools
            .iter()
            .any(|t| t.name == "security_scanner"));
    }

    #[test]
    fn test_recognize_documentation() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::Documentation,
            complexity: Complexity::Simple,
            priority: Priority::Medium,
            keywords: vec!["文档".into()],
            raw_input: "生成API文档".into(),
            confidence: 0.7,
        };
        let result = recognizer.recognize("生成API文档", base_intent);
        assert!(result.recommended_tools.iter().any(|t| t.name == "docgen"));
    }

    #[test]
    fn test_recognize_performance_optimization() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::PerformanceOptimization,
            complexity: Complexity::Complex,
            priority: Priority::High,
            keywords: vec!["性能".into()],
            raw_input: "分析CPU性能瓶颈".into(),
            confidence: 0.8,
        };
        let result = recognizer.recognize("分析CPU性能瓶颈", base_intent);
        assert!(result.needs_tools);
        assert!(result
            .recommended_tools
            .iter()
            .any(|t| t.name == "profiler"));
    }

    #[test]
    fn test_parameter_inference_rust() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "用Rust写一个web服务器".into(),
            confidence: 0.8,
        };
        let result = recognizer.recognize("用Rust写一个web服务器", base_intent);
        assert!(!result.tool_calls.is_empty());
        let codegen_call = result.tool_calls.iter().find(|c| c.tool_name == "codegen");
        assert!(codegen_call.is_some());
        assert_eq!(
            codegen_call
                .unwrap()
                .expected_params
                .get("language")
                .unwrap(),
            "rust"
        );
    }

    #[test]
    fn test_parameter_inference_python() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::CodeGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "用python写一个脚本".into(),
            confidence: 0.8,
        };
        let result = recognizer.recognize("用python写一个脚本", base_intent);
        let codegen_call = result.tool_calls.iter().find(|c| c.tool_name == "codegen");
        assert!(codegen_call.is_some());
        assert_eq!(
            codegen_call
                .unwrap()
                .expected_params
                .get("language")
                .unwrap(),
            "python"
        );
    }

    #[test]
    fn test_parameter_inference_unit_test() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::TestGeneration,
            complexity: Complexity::Simple,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "编写单元测试".into(),
            confidence: 0.7,
        };
        let result = recognizer.recognize("编写单元测试", base_intent);
        let testgen_call = result.tool_calls.iter().find(|c| c.tool_name == "testgen");
        assert!(testgen_call.is_some());
        assert_eq!(
            testgen_call
                .unwrap()
                .expected_params
                .get("test_type")
                .unwrap(),
            "unit"
        );
    }

    #[test]
    fn test_parameter_inference_integration_test() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::TestGeneration,
            complexity: Complexity::Medium,
            priority: Priority::Medium,
            keywords: vec![],
            raw_input: "编写集成测试".into(),
            confidence: 0.7,
        };
        let result = recognizer.recognize("编写集成测试", base_intent);
        let testgen_call = result.tool_calls.iter().find(|c| c.tool_name == "testgen");
        assert!(testgen_call.is_some());
        assert_eq!(
            testgen_call
                .unwrap()
                .expected_params
                .get("test_type")
                .unwrap(),
            "integration"
        );
    }

    #[test]
    fn test_parameter_inference_full_scan() {
        let recognizer = ToolAwareRecognizer::new();
        let base_intent = RecognizedIntent {
            intent_type: IntentType::SecurityAudit,
            complexity: Complexity::Complex,
            priority: Priority::High,
            keywords: vec![],
            raw_input: "全面安全扫描".into(),
            confidence: 0.9,
        };
        let result = recognizer.recognize("全面安全扫描", base_intent);
        let scanner_call = result
            .tool_calls
            .iter()
            .find(|c| c.tool_name == "security_scanner");
        assert!(scanner_call.is_some());
        assert_eq!(
            scanner_call
                .unwrap()
                .expected_params
                .get("scan_type")
                .unwrap(),
            "full"
        );
    }

    #[test]
    fn test_type_name_all_variants() {
        assert_eq!(IntentType::CodeGeneration.type_name(), "CodeGeneration");
        assert_eq!(IntentType::CodeReview.type_name(), "CodeReview");
        assert_eq!(IntentType::BugFix.type_name(), "BugFix");
        assert_eq!(IntentType::Documentation.type_name(), "Documentation");
        assert_eq!(IntentType::TestGeneration.type_name(), "TestGeneration");
        assert_eq!(
            IntentType::ArchitectureDesign.type_name(),
            "ArchitectureDesign"
        );
        assert_eq!(
            IntentType::PerformanceOptimization.type_name(),
            "PerformanceOptimization"
        );
        assert_eq!(IntentType::SecurityAudit.type_name(), "SecurityAudit");
        assert_eq!(IntentType::KnowledgeQuery.type_name(), "KnowledgeQuery");
        assert_eq!(IntentType::SystemAdmin.type_name(), "SystemAdmin");
        assert_eq!(IntentType::Unknown.type_name(), "Unknown");
    }

    #[test]
    fn test_default_trait() {
        let recognizer = ToolAwareRecognizer::default();
        assert!(!recognizer.tools.is_empty());
    }

    #[test]
    fn test_tool_calls_threshold() {
        let recognizer = ToolAwareRecognizer::new();
        // Unknown intent with low confidence → score should be low → no tool_calls
        let base_intent = RecognizedIntent {
            intent_type: IntentType::Unknown,
            complexity: Complexity::Simple,
            priority: Priority::Low,
            keywords: vec![],
            raw_input: "hello".into(),
            confidence: 0.1,
        };
        let result = recognizer.recognize("hello", base_intent);
        // With Unknown intent and no keywords, score < 0.5 → no tool_calls
        assert!(result.tool_calls.is_empty());
    }
}
