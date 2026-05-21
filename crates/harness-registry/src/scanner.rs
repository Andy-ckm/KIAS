//! Artifact Scanner: Auto-discovery of engineering artifacts on disk.
//!
//! Walks the project directory tree and registers all recognized artifacts
//! (AGENTS.md, skills/, agents/, .context/, scripts/, requirements/, etc.)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use chrono::Utc;
use sha2::{Sha256, Digest};

use crate::artifact::{ArtifactMetadata, ArtifactType};
use crate::error::{HarnessResult};

/// Configuration for the artifact scanner.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Root directory to scan.
    pub root: PathBuf,
    /// Whether to scan recursively.
    pub recursive: bool,
    /// Maximum depth for recursive scanning.
    pub max_depth: usize,
    /// File patterns to ignore.
    pub ignore_patterns: Vec<String>,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            recursive: true,
            max_depth: 5,
            ignore_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
                ".DS_Store".to_string(),
            ],
        }
    }
}

/// Result of scanning a directory.
#[derive(Debug)]
pub struct ScanResult {
    /// Artifacts discovered.
    pub artifacts: Vec<ArtifactMetadata>,
    /// Errors encountered during scanning.
    pub errors: Vec<ScanError>,
    /// Total files scanned.
    pub files_scanned: usize,
    /// Time taken for the scan.
    pub duration_ms: u64,
}

/// Error encountered during scanning.
#[derive(Debug)]
pub struct ScanError {
    /// Path where the error occurred.
    pub path: PathBuf,
    /// Error message.
    pub message: String,
}

/// Scanner for discovering engineering artifacts on disk.
pub struct ArtifactScanner {
    config: ScannerConfig,
}

impl ArtifactScanner {
    /// Create a new scanner with the given configuration.
    pub fn new(config: ScannerConfig) -> Self {
        Self { config }
    }

    /// Create a scanner with default configuration for a given root.
    pub fn with_root(root: impl AsRef<Path>) -> Self {
        let config = ScannerConfig {
            root: root.as_ref().to_path_buf(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// Scan the directory and discover all artifacts.
    pub async fn scan(&self) -> HarnessResult<ScanResult> {
        let start = std::time::Instant::now();
        let mut artifacts = Vec::new();
        let mut errors = Vec::new();
        let mut files_scanned = 0usize;

        self.scan_directory(&self.config.root, 0, &mut artifacts, &mut errors, &mut files_scanned).await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScanResult {
            artifacts,
            errors,
            files_scanned,
            duration_ms,
        })
    }

    /// Recursively scan a directory.
    fn scan_directory<'a>(
        &'a self,
        dir: &'a Path,
        depth: usize,
        artifacts: &'a mut Vec<ArtifactMetadata>,
        errors: &'a mut Vec<ScanError>,
        files_scanned: &'a mut usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HarnessResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if depth > self.config.max_depth {
                return Ok(());
            }

            let mut entries = match fs::read_dir(dir).await {
                Ok(entries) => entries,
                Err(e) => {
                    errors.push(ScanError {
                        path: dir.to_path_buf(),
                        message: format!("Failed to read directory: {}", e),
                    });
                    return Ok(());
                }
            };

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                // Check ignore patterns
                if self.should_ignore(&path, &file_name) {
                    continue;
                }

                *files_scanned += 1;

                if path.is_dir() {
                    // Check if this directory is a known artifact type
                    if let Some(artifact_type) = self.detect_directory_type(&file_name) {
                        match self.scan_artifact_directory(&path, artifact_type).await {
                            Ok(artifact) => artifacts.push(artifact),
                            Err(e) => errors.push(ScanError {
                                path: path.clone(),
                                message: format!("Failed to scan artifact directory: {}", e),
                            }),
                        }
                    }

                    // Recurse if configured
                    if self.config.recursive && depth < self.config.max_depth {
                        self.scan_directory(&path, depth + 1, artifacts, errors, files_scanned).await?;
                    }
                } else if path.is_file() {
                    // Check if this file is a known artifact type
                    if let Some(artifact_type) = self.detect_file_type(&file_name) {
                        match self.create_artifact_metadata(&path, artifact_type).await {
                            Ok(artifact) => artifacts.push(artifact),
                            Err(e) => errors.push(ScanError {
                                path: path.clone(),
                                message: format!("Failed to create artifact metadata: {}", e),
                            }),
                        }
                    }
                }
            }

            Ok(())
        })
    }

    /// Check if a path should be ignored.
    fn should_ignore(&self, path: &Path, file_name: &str) -> bool {
        // Check file name patterns
        for pattern in &self.config.ignore_patterns {
            if file_name.contains(pattern) || path.to_string_lossy().contains(pattern) {
                return true;
            }
        }

        // Skip hidden files/directories (starting with .)
        if file_name.starts_with('.') && file_name != ".context" && file_name != ".playground" {
            return true;
        }

        false
    }

    /// Detect artifact type from directory name.
    fn detect_directory_type(&self, name: &str) -> Option<ArtifactType> {
        match name {
            "skills" => Some(ArtifactType::Skills),
            "agents" => Some(ArtifactType::Agents),
            "requirements" => Some(ArtifactType::Requirements),
            "scripts" => Some(ArtifactType::Scripts),
            ".context" => Some(ArtifactType::Context),
            ".playground" => Some(ArtifactType::Commands),
            _ => None,
        }
    }

    /// Detect artifact type from file name.
    fn detect_file_type(&self, name: &str) -> Option<ArtifactType> {
        match name {
            "AGENTS.md" => Some(ArtifactType::AgentsMd),
            "service-matrix.md" => Some(ArtifactType::ServiceMatrix),
            _ => None,
        }
    }

    /// Scan an artifact directory and create metadata.
    async fn scan_artifact_directory(
        &self,
        dir: &Path,
        artifact_type: ArtifactType,
    ) -> HarnessResult<ArtifactMetadata> {
        let name = dir.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Count items in directory
        let item_count = self.count_items(dir).await?;

        // Compute content hash from directory listing
        let content_hash = self.compute_directory_hash(dir).await?;

        let relative_path = dir.strip_prefix(&self.config.root)
            .unwrap_or(dir)
            .to_path_buf();

        Ok(ArtifactMetadata {
            id: format!("{}-{}", artifact_type, name),
            name,
            artifact_type,
            path: relative_path,
            version: format!("items:{}", item_count),
            owner: "system".to_string(),
            dependencies: Vec::new(),
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash,
            custom_metadata: HashMap::from([
                ("item_count".to_string(), serde_json::Value::Number(item_count.into())),
            ]),
        })
    }

    /// Create metadata for a file artifact.
    async fn create_artifact_metadata(
        &self,
        path: &Path,
        artifact_type: ArtifactType,
    ) -> HarnessResult<ArtifactMetadata> {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Compute content hash
        let content = fs::read(path).await?;
        let content_hash = self.compute_hash(&content);

        // Get file size
        let file_size = content.len();

        let relative_path = path.strip_prefix(&self.config.root)
            .unwrap_or(path)
            .to_path_buf();

        Ok(ArtifactMetadata {
            id: format!("{}-{}", artifact_type, name),
            name,
            artifact_type,
            path: relative_path,
            version: "1.0.0".to_string(),
            owner: "system".to_string(),
            dependencies: Vec::new(),
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash,
            custom_metadata: HashMap::from([
                ("file_size".to_string(), serde_json::Value::Number(file_size.into())),
            ]),
        })
    }

    /// Count items in a directory.
    async fn count_items(&self, dir: &Path) -> HarnessResult<usize> {
        let mut count = 0usize;
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Compute hash of a directory's contents.
    async fn compute_directory_hash(&self, dir: &Path) -> HarnessResult<String> {
        let mut hasher = Sha256::new();
        let mut entries = fs::read_dir(dir).await?;

        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
        names.sort();

        for name in &names {
            hasher.update(name.as_bytes());
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Compute hash of file content.
    fn compute_hash(&self, content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_scan_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        assert_eq!(result.artifacts.len(), 0);
        assert_eq!(result.errors.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_agents_md() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# AGENTS.md\n\nTest").await.unwrap();

        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].name, "AGENTS.md");
        assert_eq!(result.artifacts[0].artifact_type, ArtifactType::AgentsMd);
    }

    #[tokio::test]
    async fn test_scan_skills_directory() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).await.unwrap();
        fs::write(skills_dir.join("test-skill.md"), "---\nname: test\n---").await.unwrap();

        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].artifact_type, ArtifactType::Skills);
    }

    #[tokio::test]
    async fn test_scan_multiple_artifacts() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# AGENTS.md").await.unwrap();
        fs::write(tmp.path().join("service-matrix.md"), "# Service Matrix").await.unwrap();

        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).await.unwrap();
        fs::write(skills_dir.join("s1.md"), "skill1").await.unwrap();

        let agents_dir = tmp.path().join("agents");
        fs::create_dir(&agents_dir).await.unwrap();
        fs::write(agents_dir.join("a1.md"), "agent1").await.unwrap();

        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        assert_eq!(result.artifacts.len(), 4);
    }

    #[tokio::test]
    async fn test_ignore_patterns() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# AGENTS.md").await.unwrap();

        // This should be ignored
        let target_dir = tmp.path().join("target");
        fs::create_dir(&target_dir).await.unwrap();
        fs::write(target_dir.join("AGENTS.md"), "ignored").await.unwrap();

        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        // Should only find the root AGENTS.md, not the one in target/
        assert_eq!(result.artifacts.len(), 1);
    }

    #[tokio::test]
    async fn test_content_hash() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# AGENTS.md\n\nContent").await.unwrap();

        let scanner = ArtifactScanner::with_root(tmp.path());
        let result = scanner.scan().await.unwrap();

        assert!(!result.artifacts[0].content_hash.is_empty());
        // Same content should produce same hash
        let result2 = scanner.scan().await.unwrap();
        assert_eq!(result.artifacts[0].content_hash, result2.artifacts[0].content_hash);
    }
}
