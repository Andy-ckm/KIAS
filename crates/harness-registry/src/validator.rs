//! Harness Validator: Ensures artifacts comply with specifications.
//!
//! This module validates engineering artifacts against their specifications,
//! preventing non-compliant artifacts from entering production.

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::artifact::{ArtifactMetadata, ArtifactType};
use crate::error::{HarnessError, HarnessResult};

/// Result of validating a single artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the artifact passed validation.
    pub passed: bool,
    /// Artifact that was validated.
    pub artifact_id: String,
    /// Type of artifact.
    pub artifact_type: ArtifactType,
    /// Validation errors (if any).
    pub errors: Vec<ValidationError>,
    /// Validation warnings (non-blocking).
    pub warnings: Vec<ValidationWarning>,
    /// When validation was performed.
    pub validated_at: DateTime<Utc>,
}

/// A validation error that prevents artifact from being used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error code for programmatic handling.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Path within the artifact where error occurred.
    pub path: Option<String>,
}

/// A validation warning that doesn't block usage but should be addressed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Warning code for programmatic handling.
    pub code: String,
    /// Human-readable warning message.
    pub message: String,
    /// Path within the artifact where warning occurred.
    pub path: Option<String>,
}

/// Report containing validation results for multiple artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Overall validation status.
    pub passed: bool,
    /// Individual validation results.
    pub results: Vec<ValidationResult>,
    /// When the report was generated.
    pub generated_at: DateTime<Utc>,
    /// Total artifacts validated.
    pub total_artifacts: usize,
    /// Number of artifacts that passed.
    pub passed_count: usize,
    /// Number of artifacts that failed.
    pub failed_count: usize,
}

/// Trait for validating specific artifact types.
pub trait ArtifactValidator: Send + Sync {
    /// Validate an artifact and return validation result.
    fn validate(&self, metadata: &ArtifactMetadata, content: &str) -> HarnessResult<ValidationResult>;
    
    /// Get the artifact type this validator handles.
    fn artifact_type(&self) -> ArtifactType;
}

/// Validator for AGENTS.md files.
pub struct AgentsMdValidator;

impl ArtifactValidator for AgentsMdValidator {
    fn validate(&self, metadata: &ArtifactMetadata, content: &str) -> HarnessResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check if file starts with expected header
        if !content.contains("# AGENTS.md") {
            errors.push(ValidationError {
                code: "MISSING_HEADER".to_string(),
                message: "AGENTS.md must start with '# AGENTS.md' header".to_string(),
                path: None,
            });
        }

        // Check for required sections
        let required_sections = [
            "## 项目概述",
            "## 快速命令",
            "## 后端架构",
            "## 关键约定",
        ];

        for section in &required_sections {
            if !content.contains(section) {
                warnings.push(ValidationWarning {
                    code: "MISSING_SECTION".to_string(),
                    message: format!("Recommended section '{}' not found", section),
                    path: None,
                });
            }
        }

        // Check for prohibited patterns
        let prohibited_patterns = [
            "unwrap()",
            "println!",
            "dbg!",
        ];

        for pattern in &prohibited_patterns {
            if content.contains(pattern) {
                errors.push(ValidationError {
                    code: "PROHIBITED_PATTERN".to_string(),
                    message: format!("Prohibited pattern '{}' found in AGENTS.md", pattern),
                    path: None,
                });
            }
        }

        Ok(ValidationResult {
            passed: errors.is_empty(),
            artifact_id: metadata.id.clone(),
            artifact_type: ArtifactType::AgentsMd,
            errors,
            warnings,
            validated_at: Utc::now(),
        })
    }

    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::AgentsMd
    }
}

/// Validator for SKILL.md files.
pub struct SkillMdValidator;

impl ArtifactValidator for SkillMdValidator {
    fn validate(&self, metadata: &ArtifactMetadata, content: &str) -> HarnessResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check for YAML frontmatter
        if !content.starts_with("---") {
            errors.push(ValidationError {
                code: "MISSING_FRONTMATTER".to_string(),
                message: "SKILL.md must start with YAML frontmatter (---)".to_string(),
                path: None,
            });
        }

        // Check for required frontmatter fields
        let required_fields = ["name", "description", "triggers"];
        for field in &required_fields {
            if !content.contains(&format!("{}:", field)) {
                errors.push(ValidationError {
                    code: "MISSING_FIELD".to_string(),
                    message: format!("Required frontmatter field '{}' not found", field),
                    path: Some("frontmatter".to_string()),
                });
            }
        }

        // Check for steps section
        if !content.contains("## Steps") && !content.contains("## 步骤") {
            warnings.push(ValidationWarning {
                code: "MISSING_STEPS".to_string(),
                message: "SKILL.md should contain a Steps section".to_string(),
                path: None,
            });
        }

        Ok(ValidationResult {
            passed: errors.is_empty(),
            artifact_id: metadata.id.clone(),
            artifact_type: ArtifactType::Skills,
            errors,
            warnings,
            validated_at: Utc::now(),
        })
    }

    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Skills
    }
}

/// Main Harness Validator that orchestrates validation of all artifact types.
pub struct HarnessValidator {
    /// Validators for specific artifact types.
    validators: HashMap<ArtifactType, Box<dyn ArtifactValidator>>,
}

impl HarnessValidator {
    /// Create a new HarnessValidator with default validators.
    pub fn new() -> Self {
        let mut validators: HashMap<ArtifactType, Box<dyn ArtifactValidator>> = HashMap::new();
        
        // Register default validators
        validators.insert(ArtifactType::AgentsMd, Box::new(AgentsMdValidator));
        validators.insert(ArtifactType::Skills, Box::new(SkillMdValidator));

        Self { validators }
    }

    /// Register a custom validator for an artifact type.
    pub fn register_validator(&mut self, validator: Box<dyn ArtifactValidator>) {
        self.validators.insert(validator.artifact_type(), validator);
    }

    /// Validate a single artifact.
    pub async fn validate_artifact(
        &self,
        metadata: &ArtifactMetadata,
        content: &str,
    ) -> HarnessResult<ValidationResult> {
        // Find validator for artifact type
        let validator = self.validators.get(&metadata.artifact_type);
        
        match validator {
            Some(validator) => validator.validate(metadata, content),
            None => {
                // If no specific validator, return a pass with warning
                Ok(ValidationResult {
                    passed: true,
                    artifact_id: metadata.id.clone(),
                    artifact_type: metadata.artifact_type.clone(),
                    errors: vec![],
                    warnings: vec![ValidationWarning {
                        code: "NO_VALIDATOR".to_string(),
                        message: format!(
                            "No validator registered for artifact type '{}'",
                            metadata.artifact_type
                        ),
                        path: None,
                    }],
                    validated_at: Utc::now(),
                })
            }
        }
    }

    /// Validate multiple artifacts and generate a report.
    pub async fn validate_all(
        &self,
        artifacts: &[(ArtifactMetadata, String)],
    ) -> HarnessResult<ValidationReport> {
        let mut results = Vec::new();
        let mut passed_count = 0;
        let mut failed_count = 0;

        for (metadata, content) in artifacts {
            let result = self.validate_artifact(metadata, content).await?;
            
            if result.passed {
                passed_count += 1;
            } else {
                failed_count += 1;
            }
            
            results.push(result);
        }

        Ok(ValidationReport {
            passed: failed_count == 0,
            results,
            generated_at: Utc::now(),
            total_artifacts: artifacts.len(),
            passed_count,
            failed_count,
        })
    }
}

impl Default for HarnessValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_metadata(artifact_type: ArtifactType) -> ArtifactMetadata {
        ArtifactMetadata {
            id: "test-1".to_string(),
            name: "test-artifact".to_string(),
            artifact_type,
            path: PathBuf::from("test-path"),
            version: "1.0.0".to_string(),
            owner: "test-owner".to_string(),
            dependencies: vec![],
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash: "abc123".to_string(),
            custom_metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_agents_md_validation() {
        let validator = HarnessValidator::new();
        let metadata = create_test_metadata(ArtifactType::AgentsMd);

        // Valid AGENTS.md
        let valid_content = "# AGENTS.md\n\n## 项目概述\n\nTest content";
        let result = validator.validate_artifact(&metadata, valid_content).await.unwrap();
        assert!(result.passed);

        // Invalid AGENTS.md (missing header)
        let invalid_content = "## 项目概述\n\nTest content";
        let result = validator.validate_artifact(&metadata, invalid_content).await.unwrap();
        assert!(!result.passed);
        assert!(!result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_skill_md_validation() {
        let validator = HarnessValidator::new();
        let metadata = create_test_metadata(ArtifactType::Skills);

        // Valid SKILL.md
        let valid_content = "---\nname: test-skill\ndescription: test\ntriggers: []\n---\n\n## Steps\n\n1. Do something";
        let result = validator.validate_artifact(&metadata, valid_content).await.unwrap();
        assert!(result.passed);

        // Invalid SKILL.md (missing frontmatter)
        let invalid_content = "# Test Skill\n\n## Steps\n\n1. Do something";
        let result = validator.validate_artifact(&metadata, invalid_content).await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_validate_all() {
        let validator = HarnessValidator::new();

        let artifacts = vec![
            (
                create_test_metadata(ArtifactType::AgentsMd),
                "# AGENTS.md\n\n## 项目概述\n\nTest".to_string(),
            ),
            (
                create_test_metadata(ArtifactType::Skills),
                "---\nname: test\ndescription: test\ntriggers: []\n---\n\n## Steps\n\n1. Test".to_string(),
            ),
        ];

        let report = validator.validate_all(&artifacts).await.unwrap();
        assert!(report.passed);
        assert_eq!(report.total_artifacts, 2);
        assert_eq!(report.passed_count, 2);
        assert_eq!(report.failed_count, 0);
    }
}
