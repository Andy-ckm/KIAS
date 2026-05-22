//! MiniMax API client for chat completions.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::KiasError;

// ─── Request / Response types ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: Option<u32>,
    pub message: Option<ChatMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ─── Client ─────────────────────────────────────────────────────────────────

/// MiniMax API client.
///
/// Configuration is read from environment variables:
/// - `MINIMAX_API_URL` – base URL (default: `https://api.minimaxi.com/v1`)
/// - `MINIMAX_API_KEY` – API key (required)
/// - `MINIMAX_MODEL` – model name (default: `MiniMax-M2.7`)
#[derive(Debug, Clone)]
pub struct MiniMaxClient {
    client: Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl MiniMaxClient {
    /// Create a new client from environment variables.
    pub fn from_env() -> Result<Self, KiasError> {
        let api_key = std::env::var("MINIMAX_API_KEY")
            .map_err(|_| KiasError::Config("MINIMAX_API_KEY not set".into()))?;
        let api_url = std::env::var("MINIMAX_API_URL")
            .unwrap_or_else(|_| "https://api.minimaxi.com/v1".to_string());
        let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());

        Ok(Self {
            client: Client::new(),
            api_url,
            api_key,
            model,
        })
    }

    /// Create a client with explicit parameters (useful for testing).
    pub fn new(
        api_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Send a chat completion request.
    pub async fn chat_completions(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatCompletionResponse, KiasError> {
        let url = format!("{}/chat/completions", self.api_url);
        let body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| KiasError::ExternalService(format!("MiniMax request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(KiasError::ExternalService(format!(
                "MiniMax API returned {status}: {text}"
            )));
        }

        resp.json::<ChatCompletionResponse>()
            .await
            .map_err(|e| KiasError::Serialization(format!("Failed to parse MiniMax response: {e}")))
    }

    /// Get the configured model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the configured API URL.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new_stores_config() {
        let client = MiniMaxClient::new("https://example.com/v1", "sk-test", "test-model");
        assert_eq!(client.api_url(), "https://example.com/v1");
        assert_eq!(client.model(), "test-model");
    }

    #[test]
    fn test_client_new_default_model() {
        let client = MiniMaxClient::new("https://example.com/v1", "sk-test", "MiniMax-M2.7");
        assert_eq!(client.model(), "MiniMax-M2.7");
    }

    #[test]
    fn test_from_env_missing_key_errors() {
        // Ensure the key is not set
        std::env::remove_var("MINIMAX_API_KEY");
        let result = MiniMaxClient::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("MINIMAX_API_KEY"));
    }

    #[test]
    fn test_from_env_with_key_succeeds() {
        std::env::set_var("MINIMAX_API_KEY", "sk-test-key");
        std::env::set_var("MINIMAX_API_URL", "https://custom.api.com/v1");
        std::env::set_var("MINIMAX_MODEL", "CustomModel");

        let client = MiniMaxClient::from_env().unwrap();
        assert_eq!(client.api_url(), "https://custom.api.com/v1");
        assert_eq!(client.model(), "CustomModel");

        // cleanup
        std::env::remove_var("MINIMAX_API_KEY");
        std::env::remove_var("MINIMAX_API_URL");
        std::env::remove_var("MINIMAX_MODEL");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi there!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id.as_deref(), Some("chatcmpl-123"));
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.as_ref().unwrap().content,
            "Hi there!"
        );
        assert_eq!(resp.usage.as_ref().unwrap().total_tokens, Some(15));
    }

    #[test]
    fn test_request_body_structure() {
        let body = ChatCompletionRequest {
            model: "MiniMax-M2.7".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "MiniMax-M2.7");
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "test");
    }
}
