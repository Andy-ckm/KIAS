//! Unified error types for the KIAS system.

use std::fmt;

/// The single error type used across every KIAS crate.
///
/// Each variant maps to a distinct failure domain so callers can match on
/// specific conditions while still propagoting unknown errors via
/// [`KiasError::Internal`].
#[derive(Debug, thiserror::Error)]
pub enum KiasError {
    // ── Resource look-up errors ──────────────────────────────────────
    /// An agent with the given identifier does not exist.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// A node with the given identifier does not exist.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// A generic not-found error for any resource type.
    #[error("Not found: {0}")]
    NotFound(String),

    /// No nodes are available for scheduling.
    #[error("No available nodes")]
    NoAvailableNodes,

    // ── Resource constraint errors ───────────────────────────────────
    /// A node does not have enough resources (CPU / memory / GPU).
    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),

    // ── Cache errors ─────────────────────────────────────────────────
    /// A cache lookup returned no hit.
    #[error("Cache miss: {0}")]
    CacheMiss(String),

    // ── Validation / configuration errors ────────────────────────────
    /// Input validation failed.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Configuration loading or parsing failed.
    #[error("Configuration error: {0}")]
    Config(String),

    /// The request is malformed or has invalid fields.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// The request conflicts with the current state.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Authentication failed (bad credentials).
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Authorization denied (insufficient permissions).
    #[error("Authorization denied: {0}")]
    AuthorizationDenied(String),

    /// The service is temporarily unavailable.
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    // ── I/O & networking ─────────────────────────────────────────────
    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization or deserialization operation failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    // ── External service errors ──────────────────────────────────────
    /// etcd interaction failed.
    #[error("Storage error: {0}")]
    Storage(String),

    /// A gRPC / HTTP request to an external service failed.
    #[error("External service error: {0}")]
    ExternalService(String),

    // ── Scheduler errors ─────────────────────────────────────────────
    /// A scheduling operation failed.
    #[error("Scheduler error: {0}")]
    Scheduler(String),

    /// Tenant quota exceeded.
    #[error("Tenant quota exceeded: {0}")]
    TenantQuotaExceeded(String),

    // ── Concurrency errors ────────────────────────────────────────
    /// A mutex / RwLock was poisoned.
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),

    // ── Catch-all ────────────────────────────────────────────────────
    /// An unexpected internal error; wraps any [`anyhow::Error`].
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

// ─── Manual From impls for commonly-wrapped error types ───────────────

impl From<serde_json::Error> for KiasError {
    fn from(err: serde_json::Error) -> Self {
        KiasError::Serialization(err.to_string())
    }
}

impl From<toml::de::Error> for KiasError {
    fn from(err: toml::de::Error) -> Self {
        KiasError::Config(err.to_string())
    }
}

impl From<config::ConfigError> for KiasError {
    fn from(err: config::ConfigError) -> Self {
        KiasError::Config(err.to_string())
    }
}

impl From<KiasError> for std::io::Error {
    fn from(err: KiasError) -> Self {
        std::io::Error::other(err.to_string())
    }
}

/// Helper: create a [`KiasError::Validation`] from a formatted message.
pub fn validation_error(msg: impl fmt::Display) -> KiasError {
    KiasError::Validation(msg.to_string())
}

/// Helper: create a [`KiasError::Config`] from a formatted message.
pub fn config_error(msg: impl fmt::Display) -> KiasError {
    KiasError::Config(msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = KiasError::AgentNotFound("agent-1".into());
        assert_eq!(err.to_string(), "Agent not found: agent-1");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let kias_err: KiasError = io_err.into();
        assert!(kias_err.to_string().contains("gone"));
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let kias_err: KiasError = json_err.into();
        assert!(matches!(kias_err, KiasError::Serialization(_)));
    }

    #[test]
    fn test_validation_helper() {
        let err = validation_error("name must not be empty");
        assert!(matches!(err, KiasError::Validation(_)));
    }
}
