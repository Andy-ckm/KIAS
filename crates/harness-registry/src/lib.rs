//! Harness Engineering: Artifact Registry, Validator, and Evolution Analyzer
//!
//! This crate implements the core Harness Engineering concepts for AI Agent governance:
//! - Artifact Registry: O(1) retrieval of engineering artifacts
//! - Validator: Ensures artifacts comply with specifications
//! - Evolution Analyzer: Tracks artifact changes and identifies patterns

pub mod artifact;
pub mod validator;
pub mod analytics;
pub mod scanner;
pub mod persistence;
pub mod error;

pub use artifact::{ArtifactRegistry, ArtifactType, ArtifactMetadata};
pub use validator::{HarnessValidator, ValidationReport, ValidationResult};
pub use analytics::{EvolutionAnalyzer, ChangePattern, OptimizationRecommendation};
pub use scanner::{ArtifactScanner, ScannerConfig, ScanResult};
pub use persistence::{RegistryPersistence, RegistrySnapshot};
pub use error::{HarnessError, HarnessResult};
