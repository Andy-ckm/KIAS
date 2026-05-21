use crate::checkpoint::types::*;
use crate::checkpoint::state_serializer::{StateSerializer, SerializationError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Error types for storage operations
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Checkpoint not found: {0}")]
    NotFound(CheckpointId),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
}

/// Trait for checkpoint storage backends
pub trait CheckpointStorage: Send + Sync {
    /// Save a checkpoint
    fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<CheckpointId, StorageError>;
    
    /// Load a checkpoint by ID
    fn load_checkpoint(&self, id: &CheckpointId) -> Result<WorkflowCheckpoint, StorageError>;
    
    /// Delete a checkpoint
    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<(), StorageError>;
    
    /// List all checkpoints for a workflow
    fn list_checkpoints(&self, workflow_id: &str) -> Result<Vec<CheckpointId>, StorageError>;
    
    /// Get the latest checkpoint for a workflow
    fn get_latest_checkpoint(&self, workflow_id: &str) -> Result<Option<CheckpointId>, StorageError>;
    
    /// Check if a checkpoint exists
    fn checkpoint_exists(&self, id: &CheckpointId) -> Result<bool, StorageError>;
    
    /// Get storage statistics
    fn get_stats(&self) -> Result<StorageStats, StorageError>;
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_checkpoints: usize,
    pub total_size_bytes: usize,
    pub checkpoints_per_workflow: HashMap<String, usize>,
    pub oldest_checkpoint: Option<SystemTime>,
    pub newest_checkpoint: Option<SystemTime>,
}

/// In-memory checkpoint storage (for testing)
pub struct MemoryStorage {
    checkpoints: Arc<Mutex<HashMap<CheckpointId, WorkflowCheckpoint>>>,
    serializer: StateSerializer,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            checkpoints: Arc::new(Mutex::new(HashMap::new())),
            serializer: StateSerializer::json(),
        }
    }
    
    pub fn with_serializer(serializer: StateSerializer) -> Self {
        Self {
            checkpoints: Arc::new(Mutex::new(HashMap::new())),
            serializer,
        }
    }
}

impl CheckpointStorage for MemoryStorage {
    fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<CheckpointId, StorageError> {
        let mut checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        let id = checkpoint.id.clone();
        checkpoints.insert(id.clone(), checkpoint.clone());
        Ok(id)
    }
    
    fn load_checkpoint(&self, id: &CheckpointId) -> Result<WorkflowCheckpoint, StorageError> {
        let checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        checkpoints.get(id).cloned().ok_or_else(|| StorageError::NotFound(id.clone()))
    }
    
    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<(), StorageError> {
        let mut checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        checkpoints.remove(id);
        Ok(())
    }
    
    fn list_checkpoints(&self, workflow_id: &str) -> Result<Vec<CheckpointId>, StorageError> {
        let checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        let ids: Vec<CheckpointId> = checkpoints.values()
            .filter(|cp| cp.workflow_id == workflow_id)
            .map(|cp| cp.id.clone())
            .collect();
        Ok(ids)
    }
    
    fn get_latest_checkpoint(&self, workflow_id: &str) -> Result<Option<CheckpointId>, StorageError> {
        let checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        let latest = checkpoints.values()
            .filter(|cp| cp.workflow_id == workflow_id)
            .max_by_key(|cp| cp.metadata.created_at)
            .map(|cp| cp.id.clone());
        Ok(latest)
    }
    
    fn checkpoint_exists(&self, id: &CheckpointId) -> Result<bool, StorageError> {
        let checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        Ok(checkpoints.contains_key(id))
    }
    
    fn get_stats(&self) -> Result<StorageStats, StorageError> {
        let checkpoints = self.checkpoints.lock().map_err(|e| StorageError::Storage(e.to_string()))?;
        let total_checkpoints = checkpoints.len();
        
        let mut total_size = 0;
        let mut checkpoints_per_workflow = HashMap::new();
        let mut oldest = None;
        let mut newest = None;
        
        for checkpoint in checkpoints.values() {
            // Count per workflow
            *checkpoints_per_workflow.entry(checkpoint.workflow_id.clone()).or_insert(0) += 1;
            
            // Calculate size (approximate)
            if let Ok(data) = self.serializer.serialize_checkpoint(checkpoint) {
                total_size += data.len();
            }
            
            // Track oldest/newest
            let created = checkpoint.metadata.created_at;
            oldest = Some(oldest.map_or(created, |o: SystemTime| o.min(created)));
            newest = Some(newest.map_or(created, |n: SystemTime| n.max(created)));
        }
        
        Ok(StorageStats {
            total_checkpoints,
            total_size_bytes: total_size,
            checkpoints_per_workflow,
            oldest_checkpoint: oldest,
            newest_checkpoint: newest,
        })
    }
}

/// Filesystem-based checkpoint storage
pub struct FilesystemStorage {
    base_path: PathBuf,
    serializer: StateSerializer,
}

impl FilesystemStorage {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            serializer: StateSerializer::json(),
        }
    }
    
    pub fn with_serializer(base_path: impl AsRef<Path>, serializer: StateSerializer) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            serializer,
        }
    }
    
    fn checkpoint_path(&self, id: &CheckpointId) -> PathBuf {
        self.base_path.join(format!("{}.json", id.as_str()))
    }
    
    fn workflow_dir(&self, workflow_id: &str) -> PathBuf {
        self.base_path.join(workflow_id)
    }
    
    fn ensure_dir(&self, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

impl CheckpointStorage for FilesystemStorage {
    fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<CheckpointId, StorageError> {
        let path = self.checkpoint_path(&checkpoint.id);
        self.ensure_dir(&path)?;
        
        let data = self.serializer.serialize_checkpoint(checkpoint)?;
        std::fs::write(&path, data)?;
        
        Ok(checkpoint.id.clone())
    }
    
    fn load_checkpoint(&self, id: &CheckpointId) -> Result<WorkflowCheckpoint, StorageError> {
        let path = self.checkpoint_path(id);
        if !path.exists() {
            return Err(StorageError::NotFound(id.clone()));
        }
        
        let data = std::fs::read(&path)?;
        let checkpoint = self.serializer.deserialize_checkpoint(&data)?;
        Ok(checkpoint)
    }
    
    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<(), StorageError> {
        let path = self.checkpoint_path(id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
    
    fn list_checkpoints(&self, workflow_id: &str) -> Result<Vec<CheckpointId>, StorageError> {
        let workflow_dir = self.workflow_dir(workflow_id);
        if !workflow_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut checkpoint_ids = Vec::new();
        for entry in std::fs::read_dir(&workflow_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(stem) = path.file_stem() {
                    if let Some(id_str) = stem.to_str() {
                        checkpoint_ids.push(CheckpointId::new(id_str));
                    }
                }
            }
        }
        
        Ok(checkpoint_ids)
    }
    
    fn get_latest_checkpoint(&self, workflow_id: &str) -> Result<Option<CheckpointId>, StorageError> {
        let checkpoint_ids = self.list_checkpoints(workflow_id)?;
        if checkpoint_ids.is_empty() {
            return Ok(None);
        }
        
        // Load all checkpoints and find the latest
        let mut latest = None;
        let mut latest_time = None;
        
        for id in checkpoint_ids {
            if let Ok(checkpoint) = self.load_checkpoint(&id) {
                let created = checkpoint.metadata.created_at;
                if latest_time.is_none() || latest_time.map_or(false, |t: SystemTime| created > t) {
                    latest = Some(id);
                    latest_time = Some(created);
                }
            }
        }
        
        Ok(latest)
    }
    
    fn checkpoint_exists(&self, id: &CheckpointId) -> Result<bool, StorageError> {
        let path = self.checkpoint_path(id);
        Ok(path.exists())
    }
    
    fn get_stats(&self) -> Result<StorageStats, StorageError> {
        let mut total_checkpoints = 0;
        let mut total_size = 0;
        let mut checkpoints_per_workflow = HashMap::new();
        let mut oldest = None;
        let mut newest = None;
        
        // Walk through all workflow directories
        if self.base_path.exists() {
            for entry in std::fs::read_dir(&self.base_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    if let Some(workflow_id) = path.file_name().and_then(|n| n.to_str()) {
                        let checkpoint_ids = self.list_checkpoints(workflow_id)?;
                        *checkpoints_per_workflow.entry(workflow_id.to_string()).or_insert(0) += checkpoint_ids.len();
                        
                        for id in checkpoint_ids {
                            total_checkpoints += 1;
                            if let Ok(checkpoint) = self.load_checkpoint(&id) {
                                let created = checkpoint.metadata.created_at;
                                oldest = Some(oldest.map_or(created, |o: SystemTime| o.min(created)));
                                newest = Some(newest.map_or(created, |n: SystemTime| n.max(created)));
                                
                                if let Ok(data) = self.serializer.serialize_checkpoint(&checkpoint) {
                                    total_size += data.len();
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(StorageStats {
            total_checkpoints,
            total_size_bytes: total_size,
            checkpoints_per_workflow,
            oldest_checkpoint: oldest,
            newest_checkpoint: newest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    fn create_test_checkpoint(id: &str, workflow_id: &str) -> WorkflowCheckpoint {
        WorkflowCheckpoint {
            id: CheckpointId::new(id),
            workflow_id: workflow_id.to_string(),
            workflow_version: "1.0.0".to_string(),
            state: WorkflowState {
                current_step: 1,
                context: HashMap::new(),
                execution_history: Vec::new(),
                timestamp: SystemTime::now(),
                execution_duration: std::time::Duration::from_secs(1),
                error: None,
            },
            metadata: CheckpointMetadata {
                created_at: SystemTime::now(),
                created_by: "test".to_string(),
                description: Some("Test checkpoint".to_string()),
                tags: vec!["test".to_string()],
                size_bytes: 100,
                checksum: "test".to_string(),
            },
            is_auto_checkpoint: false,
            parent_checkpoint_id: None,
        }
    }
    
    #[test]
    fn test_memory_storage_basic_operations() {
        let storage = MemoryStorage::new();
        let checkpoint = create_test_checkpoint("cp1", "workflow1");
        
        // Save
        let id = storage.save_checkpoint(&checkpoint).unwrap();
        assert_eq!(id.as_str(), "cp1");
        
        // Load
        let loaded = storage.load_checkpoint(&id).unwrap();
        assert_eq!(loaded.id.as_str(), "cp1");
        
        // Exists
        assert!(storage.checkpoint_exists(&id).unwrap());
        
        // List
        let ids = storage.list_checkpoints("workflow1").unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "cp1");
        
        // Delete
        storage.delete_checkpoint(&id).unwrap();
        assert!(!storage.checkpoint_exists(&id).unwrap());
    }
    
    #[test]
    fn test_memory_storage_multiple_checkpoints() {
        let storage = MemoryStorage::new();
        
        // Save multiple checkpoints for same workflow
        storage.save_checkpoint(&create_test_checkpoint("cp1", "workflow1")).unwrap();
        storage.save_checkpoint(&create_test_checkpoint("cp2", "workflow1")).unwrap();
        storage.save_checkpoint(&create_test_checkpoint("cp3", "workflow2")).unwrap();
        
        // List by workflow
        let ids1 = storage.list_checkpoints("workflow1").unwrap();
        assert_eq!(ids1.len(), 2);
        
        let ids2 = storage.list_checkpoints("workflow2").unwrap();
        assert_eq!(ids2.len(), 1);
        
        // Stats
        let stats = storage.get_stats().unwrap();
        assert_eq!(stats.total_checkpoints, 3);
        assert_eq!(stats.checkpoints_per_workflow.get("workflow1"), Some(&2));
        assert_eq!(stats.checkpoints_per_workflow.get("workflow2"), Some(&1));
    }
    
    #[test]
    fn test_filesystem_storage_basic_operations() {
        let dir = tempdir().unwrap();
        let storage = FilesystemStorage::new(dir.path());
        
        let checkpoint = create_test_checkpoint("cp1", "workflow1");
        
        // Save
        let id = storage.save_checkpoint(&checkpoint).unwrap();
        
        // Load
        let loaded = storage.load_checkpoint(&id).unwrap();
        assert_eq!(loaded.id.as_str(), "cp1");
        
        // Exists
        assert!(storage.checkpoint_exists(&id).unwrap());
        
        // Delete
        storage.delete_checkpoint(&id).unwrap();
        assert!(!storage.checkpoint_exists(&id).unwrap());
    }
    
    #[test]
    fn test_filesystem_storage_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        
        // Save checkpoint
        {
            let storage = FilesystemStorage::new(&path);
            let checkpoint = create_test_checkpoint("cp1", "workflow1");
            storage.save_checkpoint(&checkpoint).unwrap();
        }
        
        // Load checkpoint in new storage instance
        {
            let storage = FilesystemStorage::new(&path);
            let loaded = storage.load_checkpoint(&CheckpointId::new("cp1")).unwrap();
            assert_eq!(loaded.id.as_str(), "cp1");
        }
    }
    
    #[test]
    fn test_storage_stats() {
        let dir = tempdir().unwrap();
        let storage = FilesystemStorage::new(dir.path());
        
        // Save some checkpoints
        storage.save_checkpoint(&create_test_checkpoint("cp1", "workflow1")).unwrap();
        storage.save_checkpoint(&create_test_checkpoint("cp2", "workflow1")).unwrap();
        storage.save_checkpoint(&create_test_checkpoint("cp3", "workflow2")).unwrap();
        
        let stats = storage.get_stats().unwrap();
        assert_eq!(stats.total_checkpoints, 3);
        assert!(stats.total_size_bytes > 0);
    }
    
    #[test]
    fn test_latest_checkpoint() {
        let storage = MemoryStorage::new();
        
        // Save checkpoints with different timestamps
        let mut cp1 = create_test_checkpoint("cp1", "workflow1");
        cp1.metadata.created_at = SystemTime::now() - std::time::Duration::from_secs(10);
        
        let mut cp2 = create_test_checkpoint("cp2", "workflow1");
        cp2.metadata.created_at = SystemTime::now();
        
        storage.save_checkpoint(&cp1).unwrap();
        storage.save_checkpoint(&cp2).unwrap();
        
        let latest = storage.get_latest_checkpoint("workflow1").unwrap();
        assert_eq!(latest.unwrap().as_str(), "cp2");
    }
}