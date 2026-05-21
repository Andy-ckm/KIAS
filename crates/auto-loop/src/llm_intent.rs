//! LLM-based Intent Classifier — 基于LLM的意图分类器
//!
//! 将关键词匹配升级为LLM分类，支持复杂语义和零样本学习。
//!
//! # 论文支撑
//! - Self-Instruct (Wang et al., 2023): 自生成指令微调
//! - HuggingGPT (Shen et al., 2023): LLM作为控制器
//! - Toolformer (Schick et al., 2023): 自主学习工具调用

use serde::{Deserialize, Serialize};

use crate::intent_recognizer::{Complexity, IntentType, Priority, RecognizedIntent};

/// LLM 意图分类请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIntentRequest {
    /// 用户输入
    pub input: String,
    /// 可选上下文
    pub context: Option<String>,
    /// 可用工具列表
    pub available_tools: Vec<String>,
}

/// LLM 意图分类响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIntentResponse {
    /// 识别的意图类型
    pub intent_type: IntentType,
    /// 置信度
    pub confidence: f64,
    /// 推理过程
    pub reasoning: String,
    /// 建议的工具
    pub suggested_tools: Vec<String>,
    /// 子意图（复杂任务）
    pub sub_intents: Vec<SubIntent>,
}

/// 子意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubIntent {
    /// 意图类型
    pub intent_type: IntentType,
    /// 描述
    pub description: String,
    /// 优先级
    pub priority: Priority,
}

/// LLM 意图分类器
pub struct LlmIntentClassifier {
    /// LLM 客户端
    client: Option<Box<dyn LlmClient>>,
    /// 是否启用 LLM 分类
    enabled: bool,
    /// 回退到关键词匹配
    fallback: crate::intent_recognizer::IntentRecognizer,
}

/// LLM 客户端 trait
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// 发送分类请求
    async fn classify_intent(
        &self,
        request: &LlmIntentRequest,
    ) -> Result<LlmIntentResponse, String>;
}

impl LlmIntentClassifier {
    /// 创建新的分类器
    pub fn new() -> Self {
        Self {
            client: None,
            enabled: false,
            fallback: crate::intent_recognizer::IntentRecognizer::new(),
        }
    }

    /// 启用 LLM 分类
    pub fn with_llm(client: Box<dyn LlmClient>) -> Self {
        Self {
            client: Some(client),
            enabled: true,
            fallback: crate::intent_recognizer::IntentRecognizer::new(),
        }
    }

    /// 识别意图
    pub async fn recognize(&self, input: &str) -> RecognizedIntent {
        if self.enabled {
            if let Some(client) = self.client.as_ref() {
                let request = LlmIntentRequest {
                    input: input.to_string(),
                    context: None,
                    available_tools: vec![],
                };

                match client.classify_intent(&request).await {
                    Ok(response) => {
                        return RecognizedIntent {
                            intent_type: response.intent_type.clone(),
                            complexity: self.estimate_complexity(input, &response),
                            priority: self.estimate_priority(input, &response),
                            keywords: self.extract_keywords(input),
                            raw_input: input.to_string(),
                            confidence: response.confidence,
                        };
                    }
                    Err(e) => {
                        tracing::warn!("LLM classification failed: {}, falling back to keyword", e);
                    }
                }
            }
        }

        // 回退到关键词匹配
        self.fallback.recognize(input)
    }

    /// 估算复杂度
    fn estimate_complexity(&self, input: &str, response: &LlmIntentResponse) -> Complexity {
        if !response.sub_intents.is_empty() {
            return Complexity::Complex;
        }
        let word_count = input.split_whitespace().count();
        if word_count > 50 {
            Complexity::Complex
        } else if word_count > 20 {
            Complexity::Medium
        } else {
            Complexity::Simple
        }
    }

    /// 估算优先级
    fn estimate_priority(&self, input: &str, response: &LlmIntentResponse) -> Priority {
        let urgent_keywords = ["紧急", "urgent", "立即", "immediately", "ASAP", "马上"];
        if urgent_keywords
            .iter()
            .any(|kw| input.to_lowercase().contains(&kw.to_lowercase()))
        {
            Priority::Critical
        } else if response.confidence > 0.9 {
            Priority::High
        } else {
            Priority::Medium
        }
    }

    /// 提取关键词
    fn extract_keywords(&self, input: &str) -> Vec<String> {
        input
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect()
    }
}

impl Default for LlmIntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Few-shot prompt 模板
pub fn build_classification_prompt(input: &str, available_tools: &[String]) -> String {
    let tools_str = if available_tools.is_empty() {
        "None".to_string()
    } else {
        available_tools.join(", ")
    };

    format!(
        r#"You are an intent classifier for an AI agent system. Classify the user's intent.

Available tools: {tools_str}

Intent types:
- CodeGeneration: Writing new code
- CodeReview: Reviewing existing code
- BugFix: Fixing bugs or errors
- Documentation: Writing or updating docs
- TestGeneration: Writing tests
- ArchitectureDesign: Designing system architecture
- PerformanceOptimization: Improving performance
- SecurityAudit: Security review
- KnowledgeQuery: Searching for information
- SystemAdmin: System administration tasks

User input: "{input}"

Respond in JSON:
{{
  "intent_type": "CodeGeneration",
  "confidence": 0.95,
  "reasoning": "The user wants to create new code",
  "suggested_tools": ["codegen"],
  "sub_intents": []
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classifier_default() {
        let classifier = LlmIntentClassifier::new();
        assert!(!classifier.enabled);
        assert!(classifier.client.is_none());
    }

    #[test]
    fn test_fallback_to_keyword() {
        let classifier = LlmIntentClassifier::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let intent = rt.block_on(classifier.recognize("请帮我实现一个缓存系统"));
        assert_eq!(intent.intent_type, IntentType::CodeGeneration);
    }

    #[test]
    fn test_build_prompt() {
        let tools = vec!["codegen".to_string(), "test".to_string()];
        let prompt = build_classification_prompt("写代码", &tools);
        assert!(prompt.contains("写代码"));
        assert!(prompt.contains("codegen, test"));
    }

    #[test]
    fn test_build_prompt_no_tools() {
        let prompt = build_classification_prompt("测试", &[]);
        assert!(prompt.contains("None"));
    }

    #[test]
    fn test_extract_keywords() {
        let classifier = LlmIntentClassifier::new();
        let keywords = classifier.extract_keywords("请帮我实现一个缓存系统");
        assert!(!keywords.is_empty());
    }

    #[test]
    fn test_sub_intent_complexity() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::CodeGeneration,
            confidence: 0.9,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![SubIntent {
                intent_type: IntentType::CodeGeneration,
                description: "test".to_string(),
                priority: Priority::Medium,
            }],
        };
        let complexity = classifier.estimate_complexity("test", &response);
        assert_eq!(complexity, Complexity::Complex);
    }

    #[test]
    fn test_urgent_priority() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::BugFix,
            confidence: 0.9,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        let priority = classifier.estimate_priority("紧急修复生产环境问题", &response);
        assert_eq!(priority, Priority::Critical);
    }

    #[test]
    fn test_high_confidence_priority() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::CodeGeneration,
            confidence: 0.95,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        let priority = classifier.estimate_priority("写代码", &response);
        assert_eq!(priority, Priority::High);
    }

    #[test]
    fn test_medium_confidence_priority() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::CodeGeneration,
            confidence: 0.7,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        let priority = classifier.estimate_priority("普通任务", &response);
        assert_eq!(priority, Priority::Medium);
    }

    #[test]
    fn test_complexity_long_input() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::CodeGeneration,
            confidence: 0.9,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        let long_input = "word ".repeat(60);
        let complexity = classifier.estimate_complexity(&long_input, &response);
        assert_eq!(complexity, Complexity::Complex);
    }

    #[test]
    fn test_complexity_medium_input() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::CodeGeneration,
            confidence: 0.9,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        let medium_input = "word ".repeat(25);
        let complexity = classifier.estimate_complexity(&medium_input, &response);
        assert_eq!(complexity, Complexity::Medium);
    }

    #[test]
    fn test_extract_keywords_filters_short() {
        let classifier = LlmIntentClassifier::new();
        let keywords = classifier.extract_keywords("I am a test");
        // "I" and "am" should be filtered (len <= 2)
        assert!(!keywords.contains(&"I".to_string()));
        assert!(!keywords.contains(&"am".to_string()));
        assert!(keywords.contains(&"test".to_string()));
    }

    #[test]
    fn test_urgent_keywords_variants() {
        let classifier = LlmIntentClassifier::new();
        let response = LlmIntentResponse {
            intent_type: IntentType::BugFix,
            confidence: 0.9,
            reasoning: "test".to_string(),
            suggested_tools: vec![],
            sub_intents: vec![],
        };
        assert_eq!(
            classifier.estimate_priority("urgent fix", &response),
            Priority::Critical
        );
        assert_eq!(
            classifier.estimate_priority("立即修复", &response),
            Priority::Critical
        );
        assert_eq!(
            classifier.estimate_priority("ASAP task", &response),
            Priority::Critical
        );
        assert_eq!(
            classifier.estimate_priority("马上处理", &response),
            Priority::Critical
        );
    }

    #[test]
    fn test_build_prompt_with_multiple_tools() {
        let tools = vec![
            "codegen".to_string(),
            "test".to_string(),
            "lint".to_string(),
        ];
        let prompt = build_classification_prompt("写代码", &tools);
        assert!(prompt.contains("codegen, test, lint"));
    }

    #[test]
    fn test_llm_intent_request_serialization() {
        let request = LlmIntentRequest {
            input: "test input".to_string(),
            context: Some("test context".to_string()),
            available_tools: vec!["tool1".to_string()],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test input"));
        assert!(json.contains("test context"));
    }
}
