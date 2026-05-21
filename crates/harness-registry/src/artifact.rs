//! Artifact Registry: O(1) retrieval of engineering artifacts.
//!
//! This module provides a unified registry for all Harness Engineering artifacts:
//! - AGENTS.md: Global specifications and default behaviors
//! - skills/: On-demand loadable atomic capabilities
//! - agents/: Specialized roles and internal instructions
//! - .playground/commands/: Shared engineering actions
//! - .context/: Knowledge base for facts/decisions/domains
//! - service-matrix.md: Boundaries, owners, dependency graphs
//! - requirements/: Structured intents (PRD)
//! - scripts/: Reusable deterministic logic

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{HarnessError, HarnessResult};

/// Types of engineering artifacts in the Harness system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtifactType {
    /// AGENTS.md: Global specifications and default behaviors.
    AgentsMd,
    /// skills/: On-demand loadable atomic capabilities.
    Skills,
    /// agents/: Specialized roles and internal instructions.
    Agents,
    /// .playground/commands/: Shared engineering actions.
    Commands,
    /// .context/: Knowledge base for facts/decisions/domains.
    Context,
    /// service-matrix.md: Boundaries, owners, dependency graphs.
    ServiceMatrix,
    /// requirements/: Structured intents (PRD).
    Requirements,
    /// scripts/: Reusable deterministic logic.
    Scripts,
    /// Custom artifact type for extensibility.
    Custom(String),
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactType::AgentsMd => write!(f, "AGENTS.md"),
            ArtifactType::Skills => write!(f, "skills/"),
            ArtifactType::Agents => write!(f, "agents/"),
            ArtifactType::Commands => write!(f, ".playground/commands/"),
            ArtifactType::Context => write!(f, ".context/"),
            ArtifactType::ServiceMatrix => write!(f, "service-matrix.md"),
            ArtifactType::Requirements => write!(f, "requirements/"),
            ArtifactType::Scripts => write!(f, "scripts/"),
            ArtifactType::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Metadata for an engineering artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    /// Unique identifier for the artifact.
    pub id: String,
    /// Name of the artifact (filename or directory name).
    pub name: String,
    /// Type of the artifact.
    pub artifact_type: ArtifactType,
    /// Path to the artifact relative to project root.
    pub path: PathBuf,
    /// Version of the artifact (semantic versioning).
    pub version: String,
    /// Owner of the artifact (team or individual).
    pub owner: String,
    /// Dependencies on other artifacts.
    pub dependencies: Vec<String>,
    /// When the artifact was created.
    pub created_at: DateTime<Utc>,
    /// When the artifact was last modified.
    pub last_modified: DateTime<Utc>,
    /// Hash of the artifact content for integrity checking.
    pub content_hash: String,
    /// Additional custom metadata.
    pub custom_metadata: HashMap<String, serde_json::Value>,
}

/// Registry for managing engineering artifacts.
pub struct ArtifactRegistry {
    /// Internal storage for artifacts indexed by ID.
    artifacts: Arc<RwLock<HashMap<String, ArtifactMetadata>>>,
    /// Index by artifact type for fast type-based queries.
    type_index: Arc<RwLock<HashMap<ArtifactType, Vec<String>>>>,
    /// Index by name for fast name-based queries.
    name_index: Arc<RwLock<HashMap<String, String>>>,
    /// Base path for artifact storage.
    base_path: PathBuf,
}

impl ArtifactRegistry {
    /// Create a new ArtifactRegistry.
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            artifacts: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Register a new artifact in the registry.
    pub async fn register(&self, metadata: ArtifactMetadata) -> HarnessResult<()> {
        let mut artifacts = self.artifacts.write().await;
        
        // Check if artifact already exists
        if artifacts.contains_key(&metadata.id) {
            return Err(HarnessError::ArtifactAlreadyExists(metadata.id.clone()));
        }

        // Update type index
        let mut type_index = self.type_index.write().await;
        type_index
            .entry(metadata.artifact_type.clone())
            .or_insert_with(Vec::new)
            .push(metadata.id.clone());

        // Update name index
        let mut name_index = self.name_index.write().await;
        name_index.insert(metadata.name.clone(), metadata.id.clone());

        // Store artifact
        artifacts.insert(metadata.id.clone(), metadata);

        Ok(())
    }

    /// Get an artifact by ID.
    pub async fn get_by_id(&self, id: &str) -> HarnessResult<ArtifactMetadata> {
        let artifacts = self.artifacts.read().await;
        artifacts
            .get(id)
            .cloned()
            .ok_or_else(|| HarnessError::ArtifactNotFound(id.to_string()))
    }

    /// Get an artifact by name.
    pub async fn get_by_name(&self, name: &str) -> HarnessResult<ArtifactMetadata> {
        let name_index = self.name_index.read().await;
        let id = name_index
            .get(name)
            .ok_or_else(|| HarnessError::ArtifactNotFound(name.to_string()))?;
        
        let artifacts = self.artifacts.read().await;
        artifacts
            .get(id)
            .cloned()
            .ok_or_else(|| HarnessError::ArtifactNotFound(id.clone()))
    }

    /// Get all artifacts of a specific type.
    pub async fn get_by_type(&self, artifact_type: &ArtifactType) -> Vec<ArtifactMetadata> {
        let type_index = self.type_index.read().await;
        let artifacts = self.artifacts.read().await;
        
        type_index
            .get(artifact_type)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| artifacts.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Update an existing artifact.
    pub async fn update(&self, metadata: ArtifactMetadata) -> HarnessResult<()> {
        let mut artifacts = self.artifacts.write().await;
        
        // Check if artifact exists
        if !artifacts.contains_key(&metadata.id) {
            return Err(HarnessError::ArtifactNotFound(metadata.id.clone()));
        }

        // Update artifact
        artifacts.insert(metadata.id.clone(), metadata);

        Ok(())
    }

    /// Remove an artifact from the registry.
    pub async fn remove(&self, id: &str) -> HarnessResult<()> {
        let mut artifacts = self.artifacts.write().await;
        
        // Get artifact before removing
        let metadata = artifacts
            .remove(id)
            .ok_or_else(|| HarnessError::ArtifactNotFound(id.to_string()))?;

        // Remove from type index
        let mut type_index = self.type_index.write().await;
        if let Some(ids) = type_index.get_mut(&metadata.artifact_type) {
            ids.retain(|x| x != id);
        }

        // Remove from name index
        let mut name_index = self.name_index.write().await;
        name_index.remove(&metadata.name);

        Ok(())
    }

    /// Get total number of registered artifacts.
    pub async fn count(&self) -> usize {
        let artifacts = self.artifacts.read().await;
        artifacts.len()
    }

    /// Check if an artifact exists.
    pub async fn exists(&self, id: &str) -> bool {
        let artifacts = self.artifacts.read().await;
        artifacts.contains_key(id)
    }

    /// Get all registered artifacts.
    pub async fn get_all(&self) -> Vec<ArtifactMetadata> {
        let artifacts = self.artifacts.read().await;
        artifacts.values().cloned().collect()
    }

    /// Get base path for artifact storage.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_register_and_get() {
        let dir = tempdir().unwrap();
        let registry = ArtifactRegistry::new(dir.path());

        let metadata = ArtifactMetadata {
            id: "test-1".to_string(),
            name: "AGENTS.md".to_string(),
            artifact_type: ArtifactType::AgentsMd,
            path: PathBuf::from("AGENTS.md"),
            version: "1.0.0".to_string(),
            owner: "platform-team".to_string(),
            dependencies: vec![],
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash: "abc123".to_string(),
            custom_metadata: HashMap::new(),
        };

        // Register artifact
        registry.register(metadata.clone()).await.unwrap();

        // Get by ID
        let retrieved = registry.get_by_id("test-1").await.unwrap();
        assert_eq!(retrieved.name, "AGENTS.md");

        // Get by name
        let retrieved = registry.get_by_name("AGENTS.md").await.unwrap();
        assert_eq!(retrieved.id, "test-1");

        // Get by type
        let artifacts = registry.get_by_type(&ArtifactType::AgentsMd).await;
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "AGENTS.md");
    }

    #[tokio::test]
    async fn test_duplicate_registration() {
        let dir = tempdir().unwrap();
        let registry = ArtifactRegistry::new(dir.path());

        let metadata = ArtifactMetadata {
            id: "test-1".to_string(),
            name: "AGENTS.md".to_string(),
            artifact_type: ArtifactType::AgentsMd,
            path: PathBuf::from("AGENTS.md"),
            version: "1.0.0".to_string(),
            owner: "platform-team".to_string(),
            dependencies: vec![],
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash: "abc123".to_string(),
            custom_metadata: HashMap::new(),
        };

        // First registration should succeed
        registry.register(metadata.clone()).await.unwrap();

        // Second registration should fail
        let result = registry.register(metadata).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = tempdir().unwrap();
        let registry = ArtifactRegistry::new(dir.path());

        let metadata = ArtifactMetadata {
            id: "test-1".to_string(),
            name: "AGENTS.md".to_string(),
            artifact_type: ArtifactType::AgentsMd,
            path: PathBuf::from("AGENTS.md"),
            version: "1.0.0".to_string(),
            owner: "platform-team".to_string(),
            dependencies: vec![],
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash: "abc123".to_string(),
            custom_metadata: HashMap::new(),
        };

        // Register and then remove
        registry.register(metadata).await.unwrap();
        assert!(registry.exists("test-1").await);

        registry.remove("test-1").await.unwrap();
        assert!(!registry.exists("test-1").await);
    }
}
