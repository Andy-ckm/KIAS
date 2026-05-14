//! # KIAS Model Router
//!
//! Intelligent multi-model router with:
//! - Multiple LLM provider support (OpenAI, Anthropic, DeepSeek, Qwen, Ollama)
//! - Load balancing strategies (round-robin, least-latency, cost-based)
//! - Circuit breaker and failover
//! - Request caching
//! - Cost tracking and budget enforcement
//! - Streaming support

pub mod error;
pub mod provider;
pub mod router;
pub mod types;

pub use error::{RouterError, RouterResult};
pub use provider::{Provider, ProviderConfig};
pub use types::ProviderHealth;
pub use router::{ModelRouter, RouterConfig};
pub use types::{
    ChatMessage, ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse,
    ModelCapability, ModelInfo, RoutingStrategy, Usage,
};
