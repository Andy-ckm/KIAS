use thiserror::Error;

/// Errors from the A2A Registry
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Agent not found: {agent_id}")]
    NotFound { agent_id: String },

    #[error("Agent already registered: {agent_id}")]
    AlreadyRegistered { agent_id: String },

    #[error("Schema validation failed: {0:?}")]
    ValidationFailed(Vec<String>),

    #[error("Unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("Internal error: {0}")]
    Internal(String),
}
