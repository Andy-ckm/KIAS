//! Core types for the model router.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Routing Strategy
// ---------------------------------------------------------------------------

/// Load balancing strategy for routing requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Round-robin across providers.
    #[default]
    RoundRobin,
    /// Route to provider with lowest latency.
    LeastLatency,
    /// Route to provider with lowest cost per token.
    CostOptimized,
    /// Route based on model capability matching.
    CapabilityBased,
    /// Weighted random selection.
    WeightedRandom,
    /// Route to specific provider (pinned).
    Pinned(String),
    /// Least busy - route to provider with fewest active requests.
    LeastBusy,
    /// Usage-based routing - route based on TPM/RPM limits.
    UsageBased,
}

// Model Capability
// ---------------------------------------------------------------------------

/// Model capabilities for intelligent routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelCapability {
    /// Chat completion.
    Chat,
    /// Text completion.
    Completion,
    /// Text embeddings.
    Embedding,
    /// Image generation.
    ImageGeneration,
    /// Vision (image understanding).
    Vision,
    /// Function/tool calling.
    FunctionCalling,
    /// Streaming responses.
    Streaming,
    /// Long context (> 32k tokens).
    LongContext,
    /// Code generation.
    CodeGeneration,
    /// Reasoning/thinking.
    Reasoning,
}

// ---------------------------------------------------------------------------
// Model Info
// ---------------------------------------------------------------------------

/// Information about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "gpt-4", "claude-3-opus").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Provider that hosts this model.
    pub provider: String,
    /// Model capabilities.
    pub capabilities: Vec<ModelCapability>,
    /// Maximum context window (tokens).
    pub max_context_tokens: u32,
    /// Cost per 1M input tokens (USD).
    pub input_cost_per_million: f64,
    /// Cost per 1M output tokens (USD).
    pub output_cost_per_million: f64,
    /// Average latency (ms).
    pub avg_latency_ms: u64,
    /// Whether the model is currently available.
    pub available: bool,
}

// ---------------------------------------------------------------------------
// Chat Types
// ---------------------------------------------------------------------------

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: "system", "user", "assistant", "tool".
    pub role: String,
    /// Message content.
    pub content: String,
    /// Tool call ID (for tool responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool calls (for assistant messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
}

/// A chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Model to use (can be a model ID or alias).
    pub model: String,
    /// Messages for the conversation.
    pub messages: Vec<ChatMessage>,
    /// Sampling temperature (0-2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Top-p sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Whether to stream the response.
    #[serde(default)]
    pub stream: bool,
    /// Tool definitions for function calling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    /// User identifier for cost tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Routing preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<RoutingPreference>,
}

/// Routing preferences for a specific request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPreference {
    /// Preferred routing strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<RoutingStrategy>,
    /// Required capabilities.
    #[serde(default)]
    pub required_capabilities: Vec<ModelCapability>,
    /// Maximum cost per request (USD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost: Option<f64>,
    /// Maximum latency (ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_latency_ms: Option<u64>,
    /// Excluded providers.
    #[serde(default)]
    pub excluded_providers: Vec<String>,
}

/// A chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID.
    pub id: String,
    /// Model used.
    pub model: String,
    /// Provider that handled the request.
    pub provider: String,
    /// Generated choices.
    pub choices: Vec<ChatChoice>,
    /// Token usage.
    pub usage: Usage,
    /// Latency (ms).
    pub latency_ms: u64,
    /// Cost (USD).
    pub cost_usd: f64,
}

/// A single choice in a chat response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    /// Choice index.
    pub index: u32,
    /// Generated message.
    pub message: ChatMessage,
    /// Finish reason.
    pub finish_reason: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens.
    pub prompt_tokens: u32,
    /// Completion tokens.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Embedding Types
// ---------------------------------------------------------------------------

/// An embedding request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Model to use.
    pub model: String,
    /// Input text(s) to embed.
    pub input: Vec<String>,
    /// User identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// An embedding response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Model used.
    pub model: String,
    /// Provider that handled the request.
    pub provider: String,
    /// Generated embeddings.
    pub embeddings: Vec<Vec<f32>>,
    /// Token usage.
    pub usage: Usage,
    /// Cost (USD).
    pub cost_usd: f64,
}

// ---------------------------------------------------------------------------
// Provider Health
// ---------------------------------------------------------------------------

/// Provider health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// Whether the provider is healthy.
    pub healthy: bool,
    /// Success rate (0.0 - 1.0).
    pub success_rate: f64,
    /// Average latency (ms).
    pub avg_latency_ms: u64,
    /// Total requests.
    pub total_requests: u64,
    /// Failed requests.
    pub failed_requests: u64,
    /// Last error message.
    pub last_error: Option<String>,
    /// Last health check timestamp.
    pub last_check: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_strategy_default() {
        let strategy = RoutingStrategy::default();
        assert_eq!(strategy, RoutingStrategy::RoundRobin);
    }

    #[test]
    fn test_routing_strategy_serialization() {
        let strategies = vec![
            RoutingStrategy::RoundRobin,
            RoutingStrategy::LeastLatency,
            RoutingStrategy::CostOptimized,
            RoutingStrategy::CapabilityBased,
            RoutingStrategy::WeightedRandom,
            RoutingStrategy::Pinned("openai".to_string()),
            RoutingStrategy::LeastBusy,
            RoutingStrategy::UsageBased,
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let deserialized: RoutingStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_model_capability_serialization() {
        let caps = vec![
            ModelCapability::Chat,
            ModelCapability::Embedding,
            ModelCapability::Vision,
            ModelCapability::FunctionCalling,
            ModelCapability::Streaming,
            ModelCapability::LongContext,
            ModelCapability::CodeGeneration,
            ModelCapability::Reasoning,
        ];

        for cap in caps {
            let json = serde_json::to_string(&cap).unwrap();
            let deserialized: ModelCapability = serde_json::from_str(&json).unwrap();
            assert_eq!(cap, deserialized);
        }
    }

    #[test]
    fn test_chat_message_construction() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
        assert!(msg.tool_call_id.is_none());
        assert!(msg.tool_calls.is_none());
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "Hi there".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "assistant");
        assert_eq!(deserialized.content, "Hi there");
    }

    #[test]
    fn test_chat_request_construction() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: Some(0.9),
            stop: None,
            stream: false,
            tools: None,
            user: Some("user-1".to_string()),
            routing: None,
        };
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_chat_request_serialization_roundtrip() {
        let request = ChatRequest {
            model: "claude-3".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: Some(vec!["STOP".to_string()]),
            stream: true,
            tools: None,
            user: None,
            routing: Some(RoutingPreference {
                strategy: Some(RoutingStrategy::LeastLatency),
                required_capabilities: vec![ModelCapability::Chat],
                max_cost: None,
                max_latency_ms: Some(1000),
                excluded_providers: vec![],
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, "claude-3");
        assert_eq!(deserialized.messages.len(), 2);
        assert!(deserialized.stream);
        assert!(deserialized.routing.is_some());
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_chat_response_serialization() {
        let response = ChatResponse {
            id: "chatcmpl-123".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Hello!".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            latency_ms: 200,
            cost_usd: 0.001,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ChatResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "chatcmpl-123");
        assert_eq!(deserialized.choices.len(), 1);
        assert_eq!(deserialized.usage.total_tokens, 15);
    }

    #[test]
    fn test_embedding_request_serialization() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: vec!["Hello world".to_string(), "Test".to_string()],
            user: Some("user-1".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: EmbeddingRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, "text-embedding-3-small");
        assert_eq!(deserialized.input.len(), 2);
    }

    #[test]
    fn test_provider_health_serialization() {
        let health = ProviderHealth {
            healthy: true,
            success_rate: 0.95,
            avg_latency_ms: 150,
            total_requests: 1000,
            failed_requests: 50,
            last_error: None,
            last_check: std::time::SystemTime::now(),
        };
        let json = serde_json::to_string(&health).unwrap();
        let deserialized: ProviderHealth = serde_json::from_str(&json).unwrap();
        assert!(deserialized.healthy);
        assert!((deserialized.success_rate - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_routing_preference_optional_fields() {
        let pref = RoutingPreference {
            strategy: None,
            required_capabilities: vec![],
            max_cost: None,
            max_latency_ms: None,
            excluded_providers: vec![],
        };
        let json = serde_json::to_string(&pref).unwrap();
        assert!(json.contains("required_capabilities"));
    }
}
