//! Persistence: Save and load registry state to/from disk.
//!
//! Provides JSON-based persistence for the artifact registry,
//! allowing state to survive process restarts.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::artifact::{ArtifactMetadata, ArtifactRegistry};
use crate::error::HarnessResult;

/// Serializable snapshot of the registry state.
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    /// Version of the snapshot format.
    pub version: u32,
    /// When the snapshot was created.
    pub created_at: String,
    /// All registered artifacts.
    pub artifacts: Vec<ArtifactMetadata>,
}

impl RegistrySnapshot {
    /// Current snapshot format version.
    const CURRENT_VERSION: u32 = 1;

    /// Create a snapshot from the registry.
    pub async fn from_registry(registry: &ArtifactRegistry) -> HarnessResult<Self> {
        let artifacts = registry.get_all().await;

        Ok(Self {
            version: Self::CURRENT_VERSION,
            created_at: chrono::Utc::now().to_rfc3339(),
            artifacts,
        })
    }

    /// Load a snapshot from a JSON file.
    pub async fn load(path: impl AsRef<Path>) -> HarnessResult<Self> {
        let content = fs::read_to_string(path).await?;
        let snapshot: Self = serde_json::from_str(&content)?;

        if snapshot.version != Self::CURRENT_VERSION {
            return Err(crate::error::HarnessError::InvalidArtifactFormat(format!(
                "Snapshot version mismatch: expected {}, found {}",
                Self::CURRENT_VERSION,
                snapshot.version
            )));
        }

        Ok(snapshot)
    }

    /// Save the snapshot to a JSON file.
    pub async fn save(&self, path: impl AsRef<Path>) -> HarnessResult<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Restore the registry from this snapshot.
    pub async fn restore_to(&self, registry: &ArtifactRegistry) -> HarnessResult<()> {
        for artifact in &self.artifacts {
            // Use register, skip if already exists
            let _ = registry.register(artifact.clone()).await;
        }
        Ok(())
    }
}

/// Persistence manager for the artifact registry.
pub struct RegistryPersistence {
    /// Path to the snapshot file.
    snapshot_path: PathBuf,
    /// Auto-save interval in seconds (0 = disabled).
    auto_save_interval: u64,
}

impl RegistryPersistence {
    /// Create a new persistence manager.
    pub fn new(snapshot_path: impl AsRef<Path>) -> Self {
        Self {
            snapshot_path: snapshot_path.as_ref().to_path_buf(),
            auto_save_interval: 0,
        }
    }

    /// Set auto-save interval.
    pub fn with_auto_save(mut self, interval_seconds: u64) -> Self {
        self.auto_save_interval = interval_seconds;
        self
    }

    /// Save the registry state to disk.
    pub async fn save(&self, registry: &ArtifactRegistry) -> HarnessResult<()> {
        let snapshot = RegistrySnapshot::from_registry(registry).await?;

        // Ensure parent directory exists
        if let Some(parent) = self.snapshot_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        snapshot.save(&self.snapshot_path).await
    }

    /// Load the registry state from disk.
    pub async fn load(&self, registry: &ArtifactRegistry) -> HarnessResult<bool> {
        if !self.snapshot_path.exists() {
            return Ok(false);
        }

        let snapshot = RegistrySnapshot::load(&self.snapshot_path).await?;
        snapshot.restore_to(registry).await?;
        Ok(true)
    }

    /// Check if a snapshot file exists.
    pub fn snapshot_exists(&self) -> bool {
        self.snapshot_path.exists()
    }

    /// Get the snapshot file path.
    pub fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf as StdPathBuf;
    use tempfile::TempDir;

    fn create_test_artifact(id: &str) -> ArtifactMetadata {
        ArtifactMetadata {
            id: id.to_string(),
            name: format!("{}.md", id),
            artifact_type: crate::artifact::ArtifactType::AgentsMd,
            path: StdPathBuf::from(format!("{}.md", id)),
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
    async fn test_save_and_load_snapshot() {
        let tmp = TempDir::new().unwrap();
        let snapshot_path = tmp.path().join("registry.json");

        // Create registry and add artifacts
        let registry = ArtifactRegistry::new(tmp.path());
        registry.register(create_test_artifact("a1")).await.unwrap();
        registry.register(create_test_artifact("a2")).await.unwrap();

        // Save
        let persistence = RegistryPersistence::new(&snapshot_path);
        persistence.save(&registry).await.unwrap();

        assert!(snapshot_path.exists());

        // Load into new registry
        let registry2 = ArtifactRegistry::new(tmp.path());
        let loaded = persistence.load(&registry2).await.unwrap();
        assert!(loaded);

        // Verify artifacts were restored
        assert_eq!(registry2.count().await, 2);
        let a1 = registry2.get_by_id("a1").await.unwrap();
        assert_eq!(a1.name, "a1.md");
    }

    #[tokio::test]
    async fn test_load_nonexistent_snapshot() {
        let tmp = TempDir::new().unwrap();
        let snapshot_path = tmp.path().join("nonexistent.json");

        let registry = ArtifactRegistry::new(tmp.path());
        let persistence = RegistryPersistence::new(&snapshot_path);
        let loaded = persistence.load(&registry).await.unwrap();

        assert!(!loaded);
    }

    #[tokio::test]
    async fn test_snapshot_format_version() {
        let tmp = TempDir::new().unwrap();
        let snapshot_path = tmp.path().join("registry.json");

        // Write a snapshot with wrong version
        let bad_snapshot = RegistrySnapshot {
            version: 999,
            created_at: Utc::now().to_rfc3339(),
            artifacts: vec![],
        };
        let content = serde_json::to_string_pretty(&bad_snapshot).unwrap();
        fs::write(&snapshot_path, content).await.unwrap();

        let registry = ArtifactRegistry::new(tmp.path());
        let persistence = RegistryPersistence::new(&snapshot_path);
        let result = persistence.load(&registry).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_get_all() {
        let tmp = TempDir::new().unwrap();
        let registry = ArtifactRegistry::new(tmp.path());

        registry.register(create_test_artifact("a1")).await.unwrap();
        registry.register(create_test_artifact("a2")).await.unwrap();
        registry.register(create_test_artifact("a3")).await.unwrap();

        let all = registry.get_all().await;
        assert_eq!(all.len(), 3);
    }
}
