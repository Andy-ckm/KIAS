//! Compliance Security Error Types
use thiserror::Error;

/// Result alias for compliance-security operations
pub type KiasResult<T> = Result<T, KiasError>;

/// Unified error type for compliance-security module
#[derive(Debug, Error)]
pub enum KiasError {
    #[error("secrets error: {0}")]
    Secrets(String),

    #[error("supply chain error: {0}")]
    SupplyChain(String),

    #[error("runtime protection error: {0}")]
    RuntimeProtection(String),

    #[error("data masking error: {0}")]
    DataMasking(String),

    #[error("security drill error: {0}")]
    SecurityDrill(String),

    #[error("vault error: {0}")]
    Vault(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("internal error: {0}")]
    Internal(String),
}
