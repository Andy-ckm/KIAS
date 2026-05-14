//! Core types for the model router.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Routing Strategy
// ---------------------------------------------------------------------------

/// Load balancing strategy for routing requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Round-robin across providers.
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

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
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
