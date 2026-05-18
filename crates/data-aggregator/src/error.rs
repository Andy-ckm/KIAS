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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_platform_display() {
        let err = AggregatorError::UnsupportedPlatform("TestPlatform".to_string());
        assert_eq!(err.to_string(), "Unsupported platform: TestPlatform");
    }

    #[test]
    fn test_http_request_display() {
        let err = AggregatorError::HttpRequest("connection refused".to_string());
        assert_eq!(err.to_string(), "HTTP request failed: connection refused");
    }

    #[test]
    fn test_parse_error_display() {
        let err = AggregatorError::ParseError("invalid JSON".to_string());
        assert_eq!(err.to_string(), "Response parse error: invalid JSON");
    }

    #[test]
    fn test_rate_limited_display() {
        let err = AggregatorError::RateLimited("Twitter".to_string(), 60);
        assert_eq!(err.to_string(), "Rate limited on Twitter, retry after 60s");
    }

    #[test]
    fn test_auth_error_display() {
        let err = AggregatorError::AuthError("invalid token".to_string());
        assert_eq!(err.to_string(), "Authentication error: invalid token");
    }

    #[test]
    fn test_config_error_display() {
        let err = AggregatorError::ConfigError("missing API key".to_string());
        assert_eq!(err.to_string(), "Provider config error: missing API key");
    }

    #[test]
    fn test_internal_display() {
        let err = AggregatorError::Internal("something went wrong".to_string());
        assert_eq!(err.to_string(), "Internal error: something went wrong");
    }

    #[test]
    fn test_from_aggregator_error_to_kias_error() {
        let err = AggregatorError::HttpRequest("timeout".to_string());
        let kias_err: KiasError = err.into();
        assert!(kias_err.to_string().contains("timeout"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let agg_err: AggregatorError = json_err.into();
        assert!(matches!(agg_err, AggregatorError::ParseError(_)));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AggregatorError>();
    }
}
