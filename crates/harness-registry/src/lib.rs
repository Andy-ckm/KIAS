//! Harness Engineering: Artifact Registry, Validator, and Evolution Analyzer
//!
//! This crate implements the core Harness Engineering concepts for AI Agent governance:
//! - Artifact Registry: O(1) retrieval of engineering artifacts
//! - Validator: Ensures artifacts comply with specifications
//! - Evolution Analyzer: Tracks artifact changes and identifies patterns

pub mod analytics;
pub mod artifact;
pub mod error;
pub mod persistence;
pub mod scanner;
pub mod validator;

pub use analytics::{ChangePattern, EvolutionAnalyzer, OptimizationRecommendation};
pub use artifact::{ArtifactMetadata, ArtifactRegistry, ArtifactType};
pub use error::{HarnessError, HarnessResult};
pub use persistence::{RegistryPersistence, RegistrySnapshot};
pub use scanner::{ArtifactScanner, ScanResult, ScannerConfig};
pub use validator::{HarnessValidator, ValidationReport, ValidationResult};
