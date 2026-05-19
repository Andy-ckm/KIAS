//! 文档存储 - 文件系统存储

use crate::error::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct DocumentStorage {
    base_path: PathBuf,
}

impl DocumentStorage {
    pub fn new(base_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(base_path)?;
        Ok(Self {
            base_path: base_path.to_path_buf(),
        })
    }

    pub fn store(&self, doc_id: &str, content: &str) -> Result<String> {
        let file_path = self.get_file_path(doc_id);
        std::fs::write(&file_path, content)?;

        // 计算校验和
        let checksum = self.calculate_checksum(content);
        Ok(checksum)
    }

    pub fn retrieve(&self, doc_id: &str) -> Result<String> {
        let file_path = self.get_file_path(doc_id);
        let content = std::fs::read_to_string(file_path)?;
        Ok(content)
    }

    pub fn delete(&self, doc_id: &str) -> Result<()> {
        let file_path = self.get_file_path(doc_id);
        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }
        Ok(())
    }

    pub fn calculate_checksum(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn get_file_path(&self, doc_id: &str) -> PathBuf {
        self.base_path.join(format!("{}.txt", doc_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_and_retrieve() {
        let tmp = TempDir::new().unwrap();
        let storage = DocumentStorage::new(tmp.path()).unwrap();

        let doc_id = "test-doc-1";
        let content = "这是测试内容";

        storage.store(doc_id, content).unwrap();
        let retrieved = storage.retrieve(doc_id).unwrap();

        assert_eq!(retrieved, content);
    }

    #[test]
    fn test_checksum() {
        let tmp = TempDir::new().unwrap();
        let storage = DocumentStorage::new(tmp.path()).unwrap();

        let checksum = storage.calculate_checksum("test content");
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA256 hex length
    }
}
