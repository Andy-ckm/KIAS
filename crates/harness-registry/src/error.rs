//! Error types for Harness Engineering operations.

use thiserror::Error;

/// Result type for Harness operations.
pub type HarnessResult<T> = Result<T, HarnessError>;

/// Errors that can occur in Harness Engineering operations.
#[derive(Error, Debug)]
pub enum HarnessError {
    /// Artifact not found in registry.
    #[error("Artifact not found: {0}")]
    ArtifactNotFound(String),

    /// Artifact already exists in registry.
    #[error("Artifact already exists: {0}")]
    ArtifactAlreadyExists(String),

    /// Invalid artifact format or content.
    #[error("Invalid artifact format: {0}")]
    InvalidArtifactFormat(String),

    /// Validation failed with specific errors.
    #[error("Validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },

    /// IO error during artifact operations.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// YAML parsing error.
    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// Search index error.
    #[error("Search index error: {0}")]
    SearchError(String),

    /// Version conflict during update.
    #[error("Version conflict: expected {expected}, found {found}")]
    VersionConflict { expected: String, found: String },

    /// Dependency violation.
    #[error("Dependency violation: {0}")]
    DependencyViolation(String),

    /// Permission denied for operation.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}
