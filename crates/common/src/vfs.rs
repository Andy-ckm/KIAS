//! # Virtual Filesystem Abstraction
//!
//! Pluggable filesystem backend for agent workspaces.
//! Supports local disk (production), in-memory (testing), and sandbox (isolated) backends.
//!
//! Inspired by AgentScope's pluggable filesystem design:
//! - **LocalFs**: Direct disk I/O for personal dev environments
//! - **MemoryFs**: In-memory for unit tests
//! - **SandboxFs**: Delegates to sandbox process (feature-gated)
//!
//! ## Design Principles
//!
//! 1. All I/O is async via tokio
//! 2. Path traversal attacks are prevented (no `..` escaping)
//! 3. Backends are object-safe (`dyn VirtualFs`)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

/// Directory entry metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Pluggable filesystem trait for agent workspaces.
///
/// All paths are workspace-relative (no leading `/`).
/// Implementations must reject path traversal (`..` components).
#[async_trait]
pub trait VirtualFs: Send + Sync {
    /// Read entire file contents
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;

    /// Write entire file contents (creates parent dirs automatically)
    async fn write(&self, path: &str, data: &[u8]) -> Result<(), VfsError>;

    /// Append to file (creates if not exists)
    async fn append(&self, path: &str, data: &[u8]) -> Result<(), VfsError>;

    /// List directory contents
    async fn list_dir(&self, dir: &str) -> Result<Vec<DirEntry>, VfsError>;

    /// Check if path exists
    async fn exists(&self, path: &str) -> Result<bool, VfsError>;

    /// Create directory (recursive, like mkdir -p)
    async fn mkdir(&self, dir: &str) -> Result<(), VfsError>;

    /// Remove file or empty directory
    async fn remove(&self, path: &str) -> Result<(), VfsError>;

    /// Remove directory and all contents (recursive)
    async fn remove_dir_all(&self, dir: &str) -> Result<(), VfsError>;

    /// Get file/dir metadata
    async fn stat(&self, path: &str) -> Result<DirEntry, VfsError>;
}

/// VFS error types
#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("file not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("path traversal rejected: {0}")]
    PathTraversal(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("other: {0}")]
    Other(String),
}

/// Validate path: reject `..` components and absolute paths
fn validate_path(path: &str) -> Result<PathBuf, VfsError> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(VfsError::PathTraversal(path.to_string()));
    }
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return Err(VfsError::PathTraversal(path.to_string()));
        }
    }
    Ok(p.to_path_buf())
}

// ─── LocalFs: Disk-based backend ───────────────────────────────────────

/// Local filesystem backend. Workspace is a directory on disk.
pub struct LocalFs {
    root: PathBuf,
}

impl LocalFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, VfsError> {
        let rel = validate_path(path)?;
        Ok(self.root.join(rel))
    }
}

#[async_trait]
impl VirtualFs for LocalFs {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        let full = self.resolve(path)?;
        tokio::fs::read(&full).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => VfsError::NotFound(path.to_string()),
            _ => VfsError::Io(e),
        })
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, data).await?;
        Ok(())
    }

    async fn append(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        use tokio::io::AsyncWriteExt;
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&full)
            .await?;
        file.write_all(data).await?;
        Ok(())
    }

    async fn list_dir(&self, dir: &str) -> Result<Vec<DirEntry>, VfsError> {
        let full = self.resolve(dir)?;
        let mut entries = Vec::new();
        let mut rd = tokio::fs::read_dir(&full).await?;
        while let Some(entry) = rd.next_entry().await? {
            let meta = entry.metadata().await?;
            entries.push(DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: meta.len(),
            });
        }
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool, VfsError> {
        let full = self.resolve(path)?;
        Ok(full.exists())
    }

    async fn mkdir(&self, dir: &str) -> Result<(), VfsError> {
        let full = self.resolve(dir)?;
        tokio::fs::create_dir_all(&full).await?;
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), VfsError> {
        let full = self.resolve(path)?;
        if full.is_dir() {
            tokio::fs::remove_dir(&full).await?;
        } else {
            tokio::fs::remove_file(&full).await?;
        }
        Ok(())
    }

    async fn remove_dir_all(&self, dir: &str) -> Result<(), VfsError> {
        let full = self.resolve(dir)?;
        tokio::fs::remove_dir_all(&full).await?;
        Ok(())
    }

    async fn stat(&self, path: &str) -> Result<DirEntry, VfsError> {
        let full = self.resolve(path)?;
        let meta = tokio::fs::metadata(&full).await?;
        Ok(DirEntry {
            name: full
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            is_dir: meta.is_dir(),
            size: meta.len(),
        })
    }
}

// ─── MemoryFs: In-memory backend for testing ───────────────────────────

/// In-memory filesystem backend. All data lives in a `HashMap`.
/// Useful for unit tests — no disk I/O required.
pub struct MemoryFs {
    files: RwLock<HashMap<String, Vec<u8>>>,
}

impl MemoryFs {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VirtualFs for MemoryFs {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        validate_path(path)?;
        let files = self.files.read().await;
        files
            .get(path)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(path.to_string()))
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        validate_path(path)?;
        let mut files = self.files.write().await;
        files.insert(path.to_string(), data.to_vec());
        Ok(())
    }

    async fn append(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        validate_path(path)?;
        let mut files = self.files.write().await;
        files
            .entry(path.to_string())
            .or_default()
            .extend_from_slice(data);
        Ok(())
    }

    async fn list_dir(&self, dir: &str) -> Result<Vec<DirEntry>, VfsError> {
        validate_path(dir)?;
        let files = self.files.read().await;
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{}/", dir.trim_end_matches('/'))
        };
        let mut seen = std::collections::HashSet::new();
        let mut entries = Vec::new();
        for key in files.keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                if let Some(first) = rest.split('/').next() {
                    if seen.insert(first.to_string()) {
                        let is_dir = rest.contains('/');
                        entries.push(DirEntry {
                            name: first.to_string(),
                            is_dir,
                            size: if is_dir { 0 } else { files[key].len() as u64 },
                        });
                    }
                }
            }
        }
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool, VfsError> {
        validate_path(path)?;
        let files = self.files.read().await;
        Ok(files.contains_key(path))
    }

    async fn mkdir(&self, _dir: &str) -> Result<(), VfsError> {
        // No-op for in-memory (implicit dirs)
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), VfsError> {
        validate_path(path)?;
        let mut files = self.files.write().await;
        files.remove(path);
        Ok(())
    }

    async fn remove_dir_all(&self, dir: &str) -> Result<(), VfsError> {
        validate_path(dir)?;
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        let mut files = self.files.write().await;
        files.retain(|k, _| !k.starts_with(&prefix) && k != dir);
        Ok(())
    }

    async fn stat(&self, path: &str) -> Result<DirEntry, VfsError> {
        validate_path(path)?;
        let files = self.files.read().await;
        if let Some(data) = files.get(path) {
            Ok(DirEntry {
                name: Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                is_dir: false,
                size: data.len() as u64,
            })
        } else {
            Err(VfsError::NotFound(path.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_write_read_roundtrip() {
        let fs = MemoryFs::new();
        fs.write("hello.txt", b"world").await.unwrap();
        let data = fs.read("hello.txt").await.unwrap();
        assert_eq!(data, b"world");
    }

    #[tokio::test]
    async fn test_memory_append() {
        let fs = MemoryFs::new();
        fs.write("log.txt", b"line1\n").await.unwrap();
        fs.append("log.txt", b"line2\n").await.unwrap();
        let data = fs.read("log.txt").await.unwrap();
        assert_eq!(data, b"line1\nline2\n");
    }

    #[tokio::test]
    async fn test_memory_not_found() {
        let fs = MemoryFs::new();
        let err = fs.read("missing.txt").await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_memory_exists() {
        let fs = MemoryFs::new();
        assert!(!fs.exists("x.txt").await.unwrap());
        fs.write("x.txt", b"y").await.unwrap();
        assert!(fs.exists("x.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_list_dir() {
        let fs = MemoryFs::new();
        fs.write("docs/a.md", b"A").await.unwrap();
        fs.write("docs/b.md", b"B").await.unwrap();
        fs.write("root.txt", b"R").await.unwrap();
        let entries = fs.list_dir("docs").await.unwrap();
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.md"));
        assert!(names.contains(&"b.md"));
    }

    #[tokio::test]
    async fn test_path_traversal_rejected() {
        let fs = MemoryFs::new();
        let err = fs.read("../etc/passwd").await.unwrap_err();
        assert!(matches!(err, VfsError::PathTraversal(_)));
    }

    #[tokio::test]
    async fn test_absolute_path_rejected() {
        let fs = MemoryFs::new();
        let err = fs.write("/etc/evil", b"x").await.unwrap_err();
        assert!(matches!(err, VfsError::PathTraversal(_)));
    }

    #[tokio::test]
    async fn test_remove() {
        let fs = MemoryFs::new();
        fs.write("tmp.txt", b"bye").await.unwrap();
        assert!(fs.exists("tmp.txt").await.unwrap());
        fs.remove("tmp.txt").await.unwrap();
        assert!(!fs.exists("tmp.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_local_fs_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write("test.txt", b"hello").await.unwrap();
        let data = fs.read("test.txt").await.unwrap();
        assert_eq!(data, b"hello");
    }

    #[tokio::test]
    async fn test_local_fs_nested_write() {
        let dir = tempfile::tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write("deep/nested/file.txt", b"data").await.unwrap();
        let data = fs.read("deep/nested/file.txt").await.unwrap();
        assert_eq!(data, b"data");
    }

    #[tokio::test]
    async fn test_local_fs_stat() {
        let dir = tempfile::tempdir().unwrap();
        let fs = LocalFs::new(dir.path());
        fs.write("info.txt", b"12345").await.unwrap();
        let entry = fs.stat("info.txt").await.unwrap();
        assert_eq!(entry.size, 5);
        assert!(!entry.is_dir);
    }
}
