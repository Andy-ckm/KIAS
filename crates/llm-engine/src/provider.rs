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
        let api_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        let resp = self
            .client
            .post(&url)
            .bearer_auth(api_key)
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
        let api_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

        let resp = self
            .client
            .post(&url)
            .bearer_auth(api_key)
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
        let api_key = self.config.api_key.as_ref().ok_or(LlmError::AuthError)?;

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
            .header("x-api-key", api_key)
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
        _request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError> {
        // Anthropic 流式实现类似，这里简化
        Err(LlmError::Provider(
            "Anthropic streaming not yet implemented".into(),
        ))
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
        _request: ChatRequest,
    ) -> Result<Vec<crate::types::StreamChunk>, LlmError> {
        // 本地模型流式实现
        Err(LlmError::Provider(
            "Local streaming not yet implemented".into(),
        ))
    }

    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_streaming(&self) -> bool {
        true
    }
}
