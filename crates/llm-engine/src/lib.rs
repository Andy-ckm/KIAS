//! LLM Engine — 多提供商 LLM 集成引擎
//!
//! 参考 LiteLLM 的 provider 抽象层，支持:
//! - OpenAI, Anthropic, 本地模型
//! - 流式输出 (SSE)
//! - Token 计数和成本追踪
//! - 失败重试和降级

pub mod provider;
pub mod types;
pub mod streaming;
pub mod cost;

pub use provider::{LlmProvider, ProviderFactory};
pub use types::{
    ChatMessage, ChatRequest, ChatResponse, LlmError, TokenUsage,
    MessageRole, ToolCall, FunctionCall, ToolDefinition, FunctionDefinition,
    StreamChunk, StreamChoice, StreamDelta,
};
pub use streaming::StreamEvent;
pub use cost::CostTracker;
