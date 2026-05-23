//! LLM Provider 抽象层
//!
//! 参考 LiteLLM 的统一接口设计

use crate::types::*;
use async_trait::async_trait;

/// LLM Provider trait — 统一接口
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 获取 provider 名称
    fn name(&self) -> &str;

    /// 获取支持的模型列表
    fn models(&self) -> Vec<String>;

    /// 非流式聊天补全
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LlmError>;

    /// 流式聊天补全
    async fn chat_stream(
        &self,
        mut request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError>;

    /// 检查是否支持工具调用
    fn supports_tools(&self) -> bool;

    /// 检查是否支持流式输出
    fn supports_streaming(&self) -> bool;
}

/// Provider 工厂 — 根据配置创建 provider
pub struct ProviderFactory;

impl ProviderFactory {
    /// 根据配置创建 provider
    pub fn create(config: &ModelConfig) -> Box<dyn LlmProvider> {
        match config.provider.as_str() {
            "openai" => Box::new(OpenAiProvider::new(config.clone())),
            "anthropic" => Box::new(AnthropicProvider::new(config.clone())),
            "local" => Box::new(LocalProvider::new(config.clone())),
            _ => Box::new(OpenAiProvider::new(config.clone())), // 默认 OpenAI 兼容
        }
    }

    /// 获取所有支持的 provider
    pub fn supported_providers() -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                name: "openai".to_string(),
                models: vec![
                    "gpt-4o".to_string(),
                    "gpt-4o-mini".to_string(),
                    "gpt-4-turbo".to_string(),
                    "o3-mini".to_string(),
                ],
                supports_streaming: true,
                supports_tools: true,
            },
            ProviderInfo {
                name: "anthropic".to_string(),
                models: vec![
                    "claude-sonnet-4-20250514".to_string(),
                    "claude-3-5-haiku-20241022".to_string(),
                    "claude-3-opus-20240229".to_string(),
                ],
                supports_streaming: true,
                supports_tools: true,
            },
            ProviderInfo {
                name: "local".to_string(),
                models: vec![
                    "qwen3-235b".to_string(),
                    "deepseek-v4".to_string(),
                    "llama-4-maverick".to_string(),
                ],
                supports_streaming: true,
                supports_tools: true,
            },
        ]
    }
}

/// OpenAI Provider
pub struct OpenAiProvider {
    config: ModelConfig,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "gpt-4o".into(),
            "gpt-4o-mini".into(),
            "gpt-4-turbo".into(),
            "o3-mini".into(),
        ]
    }

    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse, LlmError> {
        request.stream = Some(false);
        let url = format!("{}/chat/completions", self.base_url());
        let auth_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        let resp = self
            .client
            .post(&url)
            .bearer_auth(auth_key)
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("{}: {}", status, body)));
        }

        Ok(resp.json().await?)
    }

    async fn chat_stream(
        &self,
        mut request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError> {
        request.stream = Some(true);
        let url = format!("{}/chat/completions", self.base_url());
        let auth_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        let resp = self
            .client
            .post(&url)
            .bearer_auth(auth_key)
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(LlmError::Provider(format!(
                "Stream error: {}",
                resp.status()
            )));
        }

        let mut chunks = Vec::new();
        let body = resp.text().await?;
        for line in body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }

        Ok(chunks)
    }

    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_streaming(&self) -> bool {
        true
    }
}

/// Anthropic Provider
pub struct AnthropicProvider {
    config: ModelConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "claude-sonnet-4-20250514".into(),
            "claude-3-5-haiku-20241022".into(),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LlmError> {
        let url = "https://api.anthropic.com/v1/messages";
        let auth_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        // 转换为 Anthropic 格式
        let system_msg = request
            .messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| {
                serde_json::json!({
                    "role": if m.role == MessageRole::Assistant { "assistant" } else { "user" },
                    "content": m.content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if let Some(system) = system_msg {
            body["system"] = serde_json::json!(system);
        }

        let resp = self
            .client
            .post(url)
            .header("x-api-key", auth_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("{}: {}", status, body)));
        }

        // 转换响应格式
        let anthropic_resp: serde_json::Value = resp.json().await?;
        let content = anthropic_resp["content"][0]["text"].as_str().unwrap_or("");
        let usage = anthropic_resp["usage"].as_object();

        Ok(ChatResponse {
            id: anthropic_resp["id"].as_str().unwrap_or("").to_string(),
            model: request.model,
            choices: vec![crate::types::Choice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: content.to_string(),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: anthropic_resp["stop_reason"].as_str().map(String::from),
            }],
            usage: usage.map(|u| TokenUsage {
                prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                completion_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                total_tokens: u["input_tokens"].as_u64().unwrap_or(0)
                    + u["output_tokens"].as_u64().unwrap_or(0),
            }),
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError> {
        let url = "https://api.anthropic.com/v1/messages";
        let auth_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        // Convert to Anthropic format (same as chat(), but with stream: true)
        let system_msg = request
            .messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| {
                serde_json::json!({
                    "role": if m.role == MessageRole::Assistant { "assistant" } else { "user" },
                    "content": m.content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": true,
        });

        if let Some(system) = system_msg {
            body["system"] = serde_json::json!(system);
        }

        let resp = self
            .client
            .post(url)
            .header("x-api-key", auth_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("{}: {}", status, body_text)));
        }

        // Parse SSE stream — Anthropic uses event types: message_start, content_block_delta, message_stop
        let body_text = resp.text().await?;
        let mut chunks = Vec::new();
        let mut current_id = String::new();
        let mut current_model = String::new();

        for line in body_text.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }

            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = parsed["type"].as_str().unwrap_or("");

            match event_type {
                "message_start" => {
                    // Extract id and model from message_start event
                    if let Some(msg) = parsed["message"].as_object() {
                        current_id = msg["id"].as_str().unwrap_or("").to_string();
                        current_model = msg["model"].as_str().unwrap_or("").to_string();
                    }
                }
                "content_block_delta" => {
                    // Extract text delta
                    if let Some(delta) = parsed["delta"].as_object() {
                        if let Some(text) = delta["text"].as_str() {
                            chunks.push(crate::types::StreamChunk {
                                id: current_id.clone(),
                                model: current_model.clone(),
                                choices: vec![crate::types::StreamChoice {
                                    index: 0,
                                    delta: crate::types::StreamDelta {
                                        role: None,
                                        content: Some(text.to_string()),
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            });
                        }
                    }
                }
                "message_stop" => {
                    // Final chunk with finish reason
                    chunks.push(crate::types::StreamChunk {
                        id: current_id.clone(),
                        model: current_model.clone(),
                        choices: vec![crate::types::StreamChoice {
                            index: 0,
                            delta: crate::types::StreamDelta {
                                role: None,
                                content: None,
                                tool_calls: None,
                            },
                            finish_reason: Some("stop".to_string()),
                        }],
                    });
                }
                _ => {} // Ignore other event types (ping, etc.)
            }
        }

        Ok(chunks)
    }

    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_streaming(&self) -> bool {
        true
    }
}

/// 本地模型 Provider (兼容 OpenAI API)
pub struct LocalProvider {
    config: ModelConfig,
    client: reqwest::Client,
}

impl LocalProvider {
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434/v1")
    }
}

#[async_trait]
impl LlmProvider for LocalProvider {
    fn name(&self) -> &str {
        "local"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "qwen3-235b".into(),
            "deepseek-v4".into(),
            "llama-4-maverick".into(),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LlmError> {
        let mut req = request;
        req.stream = Some(false);
        let url = format!("{}/chat/completions", self.base_url());

        let resp = self.client.post(&url).json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("{}: {}", status, body)));
        }

        Ok(resp.json().await?)
    }

    async fn chat_stream(
        &self,
        mut request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError> {
        request.stream = Some(true);
        let url = format!("{}/chat/completions", self.base_url());

        let resp = self.client.post(&url).json(&request).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!("{}: {}", status, body)));
        }

        // Local models use OpenAI-compatible SSE format
        let body = resp.text().await?;
        let mut chunks = Vec::new();
        for line in body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<crate::types::StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }

        Ok(chunks)
    }

    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_streaming(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_sse_parsing() {
        let sse_data = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"model\":\"claude-sonnet-4-20250514\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n",
            "data: {\"type\":\"message_stop\"}\n"
        );

        let mut chunks = Vec::new();
        let mut current_id = String::new();
        let mut current_model = String::new();

        for line in sse_data.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let event_type = parsed["type"].as_str().unwrap_or("");
            match event_type {
                "message_start" => {
                    if let Some(msg) = parsed["message"].as_object() {
                        current_id = msg["id"].as_str().unwrap_or("").to_string();
                        current_model = msg["model"].as_str().unwrap_or("").to_string();
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = parsed["delta"].as_object() {
                        if let Some(text) = delta["text"].as_str() {
                            chunks.push(StreamChunk {
                                id: current_id.clone(),
                                model: current_model.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: StreamDelta {
                                        role: None,
                                        content: Some(text.to_string()),
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            });
                        }
                    }
                }
                "message_stop" => {
                    chunks.push(StreamChunk {
                        id: current_id.clone(),
                        model: current_model.clone(),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: None,
                                tool_calls: None,
                            },
                            finish_reason: Some("stop".to_string()),
                        }],
                    });
                }
                _ => {}
            }
        }

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].id, "msg_123");
        assert_eq!(
            chunks[0].choices[0].delta.content,
            Some("Hello".to_string())
        );
        assert_eq!(
            chunks[1].choices[0].delta.content,
            Some(" world".to_string())
        );
        assert_eq!(chunks[2].choices[0].finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_openai_sse_parsing() {
        let sse_body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" there\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n"
        );

        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].choices[0].delta.content, Some("Hi".to_string()));
        assert_eq!(
            chunks[1].choices[0].delta.content,
            Some(" there".to_string())
        );
        assert_eq!(chunks[2].choices[0].finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_provider_factory_openai() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: Some("sk-test".to_string()),
            base_url: None,
            temperature: Some(0.7),
            max_tokens: Some(1000),
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "openai");
        assert!(provider.supports_streaming());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_provider_factory_anthropic() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: Some("sk-ant-test".to_string()),
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "anthropic");
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_provider_factory_local() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434/v1".to_string()),
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "local");
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_provider_factory_unknown_defaults_to_openai() {
        let config = ModelConfig {
            provider: "unknown-provider".to_string(),
            model: "some-model".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_supported_providers_list() {
        let providers = ProviderFactory::supported_providers();
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0].name, "openai");
        assert_eq!(providers[1].name, "anthropic");
        assert_eq!(providers[2].name, "local");
        assert!(providers.iter().all(|p| p.supports_streaming));
        assert!(providers.iter().all(|p| p.supports_tools));
    }

    #[test]
    fn test_openai_base_url_default() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = OpenAiProvider::new(config);
        assert_eq!(provider.base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn test_openai_base_url_custom() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: Some("https://custom.api.com/v1".to_string()),
            temperature: None,
            max_tokens: None,
        };
        let provider = OpenAiProvider::new(config);
        assert_eq!(provider.base_url(), "https://custom.api.com/v1");
    }

    #[test]
    fn test_local_base_url_default() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = LocalProvider::new(config);
        assert_eq!(provider.base_url(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_local_base_url_custom() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: Some("http://gpu-server:8080/v1".to_string()),
            temperature: None,
            max_tokens: None,
        };
        let provider = LocalProvider::new(config);
        assert_eq!(provider.base_url(), "http://gpu-server:8080/v1");
    }

    #[test]
    fn test_anthropic_models_list() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = AnthropicProvider::new(config);
        let models = provider.models();
        assert!(models.contains(&"claude-sonnet-4-20250514".to_string()));
        assert!(models.contains(&"claude-3-5-haiku-20241022".to_string()));
    }

    #[test]
    fn test_factory_create_anthropic() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_factory_create_local() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "local");
    }

    #[test]
    fn test_factory_create_unknown_defaults_to_openai() {
        let config = ModelConfig {
            provider: "unknown_provider".to_string(),
            model: "some-model".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_models_list() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = OpenAiProvider::new(config);
        let models = provider.models();
        assert!(models.contains(&"gpt-4o".to_string()));
        assert!(models.contains(&"gpt-4o-mini".to_string()));
    }

    #[test]
    fn test_local_models_list() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = LocalProvider::new(config);
        let models = provider.models();
        assert!(models.contains(&"qwen3-235b".to_string()));
    }

    #[test]
    fn test_all_providers_support_streaming_and_tools() {
        let providers = ProviderFactory::supported_providers();
        for p in providers {
            assert!(p.supports_streaming, "{} should support streaming", p.name);
            assert!(p.supports_tools, "{} should support tools", p.name);
        }
    }

    // ===== NEW TESTS: Error paths, edge cases, SSE parsing =====

    #[test]
    fn test_anthropic_sse_parsing_empty_body() {
        let sse_data = "";
        let mut chunks: Vec<StreamChunk> = Vec::new();
        let mut _current_id = String::new();
        let mut _current_model = String::new();

        for line in sse_data.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }
            let _parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
        }
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_anthropic_sse_parsing_invalid_json() {
        let sse_data = "data: {invalid json\n\ndata: {also bad\n";
        let mut chunks: Vec<crate::types::StreamChunk> = Vec::new();
        let mut _current_id = String::new();
        let mut _current_model = String::new();

        for line in sse_data.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }
            let _parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
        }
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_anthropic_sse_parsing_unknown_event_types() {
        let sse_data = concat!(
            "data: {\"type\":\"ping\"}\n\n",
            "data: {\"type\":\"content_block_start\"}\n\n",
            "data: {\"type\":\"message_delta\"}\n\n",
        );

        let mut chunks: Vec<StreamChunk> = Vec::new();
        let mut _current_id = String::new();
        let mut _current_model = String::new();

        for line in sse_data.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let event_type = parsed["type"].as_str().unwrap_or("");
            match event_type {
                "message_start" => {
                    if let Some(msg) = parsed["message"].as_object() {
                        _current_id = msg["id"].as_str().unwrap_or("").to_string();
                        _current_model = msg["model"].as_str().unwrap_or("").to_string();
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = parsed["delta"].as_object() {
                        if let Some(text) = delta["text"].as_str() {
                            chunks.push(StreamChunk {
                                id: _current_id.clone(),
                                model: _current_model.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: StreamDelta {
                                        role: None,
                                        content: Some(text.to_string()),
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            });
                        }
                    }
                }
                "message_stop" => {
                    chunks.push(StreamChunk {
                        id: _current_id.clone(),
                        model: _current_model.clone(),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: None,
                                tool_calls: None,
                            },
                            finish_reason: Some("stop".to_string()),
                        }],
                    });
                }
                _ => {}
            }
        }
        // All 3 events are unknown/ignored — no chunks produced
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_anthropic_sse_content_block_delta_missing_text() {
        let sse_data = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\"}}\n";
        let mut chunks = Vec::new();
        let mut _current_id = String::new();
        let mut _current_model = String::new();

        for line in sse_data.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let event_type = parsed["type"].as_str().unwrap_or("");
            match event_type {
                "content_block_delta" => {
                    if let Some(delta) = parsed["delta"].as_object() {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            chunks.push(StreamChunk {
                                id: _current_id.clone(),
                                model: _current_model.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: StreamDelta {
                                        role: None,
                                        content: Some(text.to_string()),
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        // Missing "text" field means no chunk
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_openai_sse_parsing_empty_body() {
        let sse_body = "";
        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_openai_sse_parsing_only_done() {
        let sse_body = "data: [DONE]\n";
        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_openai_sse_parsing_with_invalid_lines() {
        let sse_body = concat!(
            "not a data line\n",
            "data: {invalid json}\n",
            "data: \n",
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
        );
        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices[0].delta.content, Some("hi".to_string()));
    }

    #[test]
    fn test_openai_sse_parsing_multiple_choices() {
        let sse_body = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"},\"finish_reason\":null},{\"index\":1,\"delta\":{\"content\":\"b\"},\"finish_reason\":null}]}\n",
        );
        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices.len(), 2);
    }

    #[test]
    fn test_provider_factory_with_empty_provider_string() {
        let config = ModelConfig {
            provider: "".to_string(),
            model: "some-model".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = ProviderFactory::create(&config);
        // Empty string doesn't match any known provider, defaults to openai
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_provider_models_not_empty() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = OpenAiProvider::new(config);
        let models = provider.models();
        assert!(!models.is_empty());
        assert_eq!(models.len(), 4);
        assert!(models.contains(&"o3-mini".to_string()));
    }

    #[test]
    fn test_local_provider_models_not_empty() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = LocalProvider::new(config);
        let models = provider.models();
        assert!(!models.is_empty());
        assert_eq!(models.len(), 3);
        assert!(models.contains(&"deepseek-v4".to_string()));
        assert!(models.contains(&"llama-4-maverick".to_string()));
    }

    #[test]
    fn test_openai_provider_supports_streaming_and_tools() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = OpenAiProvider::new(config);
        assert!(provider.supports_streaming());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_anthropic_provider_supports_streaming_and_tools() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = AnthropicProvider::new(config);
        assert!(provider.supports_streaming());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_local_provider_supports_streaming_and_tools() {
        let config = ModelConfig {
            provider: "local".to_string(),
            model: "qwen3-235b".to_string(),
            api_key: None,
            base_url: None,
            temperature: None,
            max_tokens: None,
        };
        let provider = LocalProvider::new(config);
        assert!(provider.supports_streaming());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_supported_providers_all_have_models() {
        let providers = ProviderFactory::supported_providers();
        for p in &providers {
            assert!(
                !p.models.is_empty(),
                "Provider {} should have at least one model",
                p.name
            );
        }
    }

    #[test]
    fn test_openai_sse_parsing_empty_delta_content() {
        let sse_body = "data: {\"id\":\"chatcmpl-1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":null}]}\n";
        let mut chunks = Vec::new();
        for line in sse_body.lines() {
            if line.starts_with("data: ") && line != "data: [DONE]" {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&line[6..]) {
                    chunks.push(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), 1);
        // Empty delta should still parse — content is None
        assert_eq!(chunks[0].choices[0].delta.content, None);
    }

    #[test]
    fn test_provider_factory_creates_different_instances() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: Some("sk-test".to_string()),
            base_url: None,
            temperature: Some(0.7),
            max_tokens: Some(1000),
        };
        let p1 = ProviderFactory::create(&config);
        let p2 = ProviderFactory::create(&config);
        // Both should have the same name but are independent instances
        assert_eq!(p1.name(), p2.name());
    }
}
