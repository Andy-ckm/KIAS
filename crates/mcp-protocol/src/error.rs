use thiserror::Error;

/// Errors that can occur in the MCP protocol layer.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("request timed out: {0}")]
    Timeout(String),

    #[error("server error {code}: {message}")]
    ServerError { code: i64, message: String },

    #[error("transport error: {0}")]
    Transport(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("method not found: {0}")]
    MethodNotFound(String),

    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("resource not found: {0}")]
    ResourceNotFound(String),

    #[error("prompt not found: {0}")]
    PromptNotFound(String),

    #[error("authentication error: {0}")]
    Authentication(String),

    #[error("authorization error: {0}")]
    Authorization(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("rate limit exceeded")]
    RateLimitExceeded,

    #[error("circuit breaker open")]
    CircuitBreakerOpen,
}
