//! LLM 引擎核心类型定义

use serde::{Deserialize, Serialize};

/// LLM 错误类型
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Authentication failed")]
    AuthError,
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// 聊天请求
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// 聊天响应
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<TokenUsage>,
}

/// 选择
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Token 使用量
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// 流式响应块
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

/// 流式选择
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

/// 流式增量
#[derive(Debug, Clone, Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// 流式工具调用
#[derive(Debug, Clone, Deserialize)]
pub struct StreamToolCall {
    pub index: u32,
    pub id: Option<String>,
    pub function: Option<StreamFunctionCall>,
}

/// 流式函数调用
#[derive(Debug, Clone, Deserialize)]
pub struct StreamFunctionCall {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
}

/// Provider 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub models: Vec<String>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "hello".to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"user\""));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_chat_message_deserialization() {
        let json = r#"{"role":"assistant","content":"hi there"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content, "hi there");
        assert!(msg.name.is_none());
    }

    #[test]
    fn test_message_role_all_variants() {
        let roles = vec![
            MessageRole::System,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ];
        for role in roles {
            let json = serde_json::to_string(&role).unwrap();
            let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, deserialized);
        }
    }

    #[test]
    fn test_tool_call_serialization() {
        let tc = ToolCall {
            id: "call_123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "get_weather".to_string(),
                arguments: r#"{"city":"Beijing"}"#.to_string(),
            },
        };
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("\"type\""));
        assert!(json.contains("get_weather"));
    }

    #[test]
    fn test_chat_request_skip_none() {
        let req = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "test".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        // None fields should be skipped
        assert!(!json.contains("temperature"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("tools"));
        assert!(!json.contains("stream"));
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_model_config_serialization() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: Some("sk-xxx".to_string()),
            base_url: None,
            temperature: Some(0.7),
            max_tokens: Some(1000),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("openai"));
        assert!(json.contains("0.7"));
    }

    #[test]
    fn test_stream_chunk_deserialization() {
        let json = r#"{"id":"chatcmpl-123","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.choices[0].delta.content, Some("hello".to_string()));
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::Provider("test error".to_string());
        assert_eq!(format!("{err}"), "Provider error: test error");

        let err = LlmError::RateLimit;
        assert_eq!(format!("{err}"), "Rate limit exceeded");

        let err = LlmError::Timeout(30);
        assert_eq!(format!("{err}"), "Timeout after 30s");
    }

    #[test]
    fn test_chat_message_with_tool_calls() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "".to_string(),
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"city":"Beijing"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("get_weather"));
    }

    #[test]
    fn test_chat_message_with_tool_call_id() {
        let msg = ChatMessage {
            role: MessageRole::Tool,
            content: "Weather is sunny".to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_call_id"));
        assert!(json.contains("call_1"));
    }

    #[test]
    fn test_chat_request_with_all_fields() {
        let req = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "test".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            tools: Some(vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get weather info".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            }]),
            stream: Some(true),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("temperature"));
        assert!(json.contains("max_tokens"));
        assert!(json.contains("tools"));
        assert!(json.contains("stream"));
    }

    #[test]
    fn test_choice_deserialization() {
        let json = r#"{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}"#;
        let choice: Choice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.index, 0);
        assert_eq!(choice.finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{"id":"chatcmpl-123","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chatcmpl-123");
        assert!(resp.usage.is_some());
        assert_eq!(resp.usage.unwrap().total_tokens, 15);
    }

    #[test]
    fn test_provider_info_serialization() {
        let info = ProviderInfo {
            name: "openai".to_string(),
            models: vec!["gpt-4o".to_string()],
            supports_streaming: true,
            supports_tools: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("openai"));
        assert!(json.contains("gpt-4o"));
    }

    #[test]
    fn test_function_definition_serialization() {
        let fd = FunctionDefinition {
            name: "test_func".to_string(),
            description: "A test function".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_string(&fd).unwrap();
        assert!(json.contains("test_func"));
        assert!(json.contains("A test function"));
    }

    #[test]
    fn test_stream_delta_deserialization() {
        let json = r#"{"role":"assistant","content":"hello"}"#;
        let delta: StreamDelta = serde_json::from_str(json).unwrap();
        assert_eq!(delta.role, Some("assistant".to_string()));
        assert_eq!(delta.content, Some("hello".to_string()));
        assert!(delta.tool_calls.is_none());
    }

    #[test]
    fn test_llm_error_invalid_request_display() {
        let err = LlmError::InvalidRequest("missing model".to_string());
        assert_eq!(format!("{err}"), "Invalid request: missing model");
    }

    #[test]
    fn test_llm_error_auth_display() {
        let err = LlmError::AuthError;
        assert_eq!(format!("{err}"), "Authentication failed");
    }

    #[test]
    fn test_chat_message_with_name() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "hello".to_string(),
            name: Some("Alice".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Alice"));
        let roundtrip: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.name, Some("Alice".to_string()));
    }

    #[test]
    fn test_chat_response_without_usage() {
        let json = r#"{"id":"cmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}]}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(resp.usage.is_none());
    }

    #[test]
    fn test_stream_tool_call_deserialization() {
        let json = r#"{"index":0,"id":"call_1","function":{"name":"search","arguments":"{}"}}"#;
        let tc: StreamToolCall = serde_json::from_str(json).unwrap();
        assert_eq!(tc.index, 0);
        assert_eq!(tc.id, Some("call_1".to_string()));
        assert!(tc.function.is_some());
        let func = tc.function.unwrap();
        assert_eq!(func.name, Some("search".to_string()));
        assert_eq!(func.arguments, Some("{}".to_string()));
    }

    #[test]
    fn test_stream_function_call_deserialization() {
        let json = r#"{"name":"calc","arguments":"{\"x\":1}"}"#;
        let fc: StreamFunctionCall = serde_json::from_str(json).unwrap();
        assert_eq!(fc.name, Some("calc".to_string()));
    }

    #[test]
    fn test_stream_function_call_all_none() {
        let json = r#"{}"#;
        let fc: StreamFunctionCall = serde_json::from_str(json).unwrap();
        assert!(fc.name.is_none());
        assert!(fc.arguments.is_none());
    }

    #[test]
    fn test_tool_definition_serialization() {
        let td = ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "search".to_string(),
                description: "Search the web".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        };
        let json = serde_json::to_string(&td).unwrap();
        assert!(json.contains("\"type\":\"function\""));
        assert!(json.contains("search"));
    }

    #[test]
    fn test_function_call_serialization() {
        let fc = FunctionCall {
            name: "get_weather".to_string(),
            arguments: r#"{"city":"Beijing"}"#.to_string(),
        };
        let json = serde_json::to_string(&fc).unwrap();
        assert!(json.contains("get_weather"));
        assert!(json.contains("Beijing"));
    }

    #[test]
    fn test_chat_request_model_field_always_present() {
        let req = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            tools: None,
            stream: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
    }

    #[test]
    fn test_stream_choice_deserialization() {
        let json = r#"{"index":0,"delta":{"content":"hi"},"finish_reason":"stop"}"#;
        let sc: StreamChoice = serde_json::from_str(json).unwrap();
        assert_eq!(sc.index, 0);
        assert_eq!(sc.delta.content, Some("hi".to_string()));
        assert_eq!(sc.finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_provider_info_deserialization() {
        let json = r#"{"name":"openai","models":["gpt-4o"],"supports_streaming":true,"supports_tools":true}"#;
        let info: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "openai");
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_token_usage_serialization() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("150"));
        let roundtrip: TokenUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.total_tokens, 150);
    }

    #[test]
    fn test_chat_message_system_role() {
        let msg = ChatMessage {
            role: MessageRole::System,
            content: "You are a helpful assistant".to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("system"));
    }
}
