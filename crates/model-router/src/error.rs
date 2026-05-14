//! Unified error types for the model router.

use thiserror::Error;

/// Errors that can occur in the model router.
#[derive(Debug, Error)]
pub enum RouterError {
    /// No provider is available to handle the request.
    #[error("No available provider for model: {0}")]
    NoAvailableProvider(String),

    /// All providers failed for the request.
    #[error("All providers failed: {0}")]
    AllProvidersFailed(String),

    /// A specific provider returned an error.
    #[error("Provider error ({provider}): {message}")]
    ProviderError { provider: String, message: String },

    /// The circuit breaker is open for this provider.
    #[error("Circuit breaker open for provider: {0}")]
    CircuitBreakerOpen(String),

    /// The request timed out.
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// The model is not supported.
    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded for provider: {0}")]
    RateLimitExceeded(String),

    /// Invalid request configuration.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Budget exceeded.
    #[error("Budget exceeded: spent {spent}, limit {limit}")]
    BudgetExceeded { spent: f64, limit: f64 },

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result alias for router operations.
pub type RouterResult<T> = Result<T, RouterError>;
