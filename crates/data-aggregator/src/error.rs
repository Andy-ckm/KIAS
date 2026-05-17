//! Unified error types for the data-aggregator crate.

use kias_common::error::KiasError;

/// Errors specific to the data aggregator.
#[derive(Debug, thiserror::Error)]
pub enum AggregatorError {
    /// The platform is not supported.
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// The HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    /// The response body could not be parsed.
    #[error("Response parse error: {0}")]
    ParseError(String),

    /// API rate limit has been hit.
    #[error("Rate limited on {0}, retry after {1}s")]
    RateLimited(String, u64),

    /// Authentication credentials are missing or invalid.
    #[error("Authentication error: {0}")]
    AuthError(String),

    /// The configuration for a provider is invalid.
    #[error("Provider config error: {0}")]
    ConfigError(String),

    /// An internal error occurred.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<AggregatorError> for KiasError {
    fn from(err: AggregatorError) -> Self {
        KiasError::ExternalService(err.to_string())
    }
}

impl From<reqwest::Error> for AggregatorError {
    fn from(err: reqwest::Error) -> Self {
        AggregatorError::HttpRequest(err.to_string())
    }
}

impl From<serde_json::Error> for AggregatorError {
    fn from(err: serde_json::Error) -> Self {
        AggregatorError::ParseError(err.to_string())
    }
}
