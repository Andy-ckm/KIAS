use crate::checkpoint::types::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use sha2::{Sha256, Digest};

/// Error types for serialization operations
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Bincode serialization error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

/// Supported serialization formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializationFormat {
    Json,
    Bincode,
    MessagePack,
}

/// Handles serialization and deserialization of workflow checkpoints
pub struct StateSerializer {
    format: SerializationFormat,
}

impl StateSerializer {
    /// Create a new StateSerializer with the specified format
    pub fn new(format: SerializationFormat) -> Self {
        Self { format }
    }
    
    /// Create a JSON serializer
    pub fn json() -> Self {
        Self::new(SerializationFormat::Json)
    }
    
    /// Create a Bincode serializer (more compact)
    pub fn bincode() -> Self {
        Self::new(SerializationFormat::Bincode)
    }
    
    /// Serialize a workflow state to bytes
    pub fn serialize_state(&self, state: &WorkflowState) -> Result<Vec<u8>, SerializationError> {
        match self.format {
            SerializationFormat::Json => {
                let json = serde_json::to_vec_pretty(state)?;
                Ok(json)
            }
            SerializationFormat::Bincode => {
                let bytes = bincode::serialize(state)?;
                Ok(bytes)
            }
            SerializationFormat::MessagePack => {
                // MessagePack implementation would go here
                // For now, fall back to JSON
                let json = serde_json::to_vec_pretty(state)?;
                Ok(json)
            }
        }
    }
    
    /// Deserialize bytes to a workflow state
    pub fn deserialize_state(&self, data: &[u8]) -> Result<WorkflowState, SerializationError> {
        match self.format {
            SerializationFormat::Json => {
                let state = serde_json::from_slice(data)?;
                Ok(state)
            }
            SerializationFormat::Bincode => {
                let state = bincode::deserialize(data)?;
                Ok(state)
            }
            SerializationFormat::MessagePack => {
                // MessagePack implementation would go here
                // For now, fall back to JSON
                let state = serde_json::from_slice(data)?;
                Ok(state)
            }
        }
    }
    
    /// Serialize a checkpoint to bytes
    pub fn serialize_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<Vec<u8>, SerializationError> {
        match self.format {
            SerializationFormat::Json => {
                let json = serde_json::to_vec_pretty(checkpoint)?;
                Ok(json)
            }
            SerializationFormat::Bincode => {
                let bytes = bincode::serialize(checkpoint)?;
                Ok(bytes)
            }
            SerializationFormat::MessagePack => {
                // MessagePack implementation would go here
                // For now, fall back to JSON
                let json = serde_json::to_vec_pretty(checkpoint)?;
                Ok(json)
            }
        }
    }
    
    /// Deserialize bytes to a checkpoint
    pub fn deserialize_checkpoint(&self, data: &[u8]) -> Result<WorkflowCheckpoint, SerializationError> {
        match self.format {
            SerializationFormat::Json => {
                let checkpoint = serde_json::from_slice(data)?;
                Ok(checkpoint)
            }
            SerializationFormat::Bincode => {
                let checkpoint = bincode::deserialize(data)?;
                Ok(checkpoint)
            }
            SerializationFormat::MessagePack => {
                // MessagePack implementation would go here
                // For now, fall back to JSON
                let checkpoint = serde_json::from_slice(data)?;
                Ok(checkpoint)
            }
        }
    }
    
    /// Calculate checksum for data integrity verification
    pub fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        hex::encode(result)
    }
    
    /// Verify data integrity using checksum
    pub fn verify_checksum(data: &[u8], expected_checksum: &str) -> Result<bool, SerializationError> {
        let actual_checksum = Self::calculate_checksum(data);
        if actual_checksum == expected_checksum {
            Ok(true)
        } else {
            Err(SerializationError::ChecksumMismatch {
                expected: expected_checksum.to_string(),
                actual: actual_checksum,
            })
        }
    }
    
    /// Save checkpoint to file
    pub fn save_checkpoint_to_file(
        &self,
        checkpoint: &WorkflowCheckpoint,
        path: &Path,
    ) -> Result<(), SerializationError> {
        let data = self.serialize_checkpoint(checkpoint)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }
    
    /// Load checkpoint from file
    pub fn load_checkpoint_from_file(&self, path: &Path) -> Result<WorkflowCheckpoint, SerializationError> {
        let mut file = std::fs::File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        self.deserialize_checkpoint(&data)
    }
    
    /// Save state to file
    pub fn save_state_to_file(
        &self,
        state: &WorkflowState,
        path: &Path,
    ) -> Result<(), SerializationError> {
        let data = self.serialize_state(state)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }
    
    /// Load state from file
    pub fn load_state_from_file(&self, path: &Path) -> Result<WorkflowState, SerializationError> {
        let mut file = std::fs::File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        self.deserialize_state(&data)
    }
    
    /// Get the format used by this serializer
    pub fn format(&self) -> &SerializationFormat {
        &self.format
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;
    use tempfile::tempdir;
    
    fn create_test_state() -> WorkflowState {
        WorkflowState {
            current_step: 3,
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("key1".to_string(), serde_json::json!("value1"));
                ctx.insert("key2".to_string(), serde_json::json!(42));
                ctx
            },
            execution_history: vec![
                StepExecution {
                    step_index: 0,
                    step_name: "init".to_string(),
                    input: serde_json::json!({}),
                    output: serde_json::json!({"status": "ok"}),
                    duration: Duration::from_millis(100),
                    success: true,
                    error: None,
                },
                StepExecution {
                    step_index: 1,
                    step_name: "process".to_string(),
                    input: serde_json::json!({"data": "test"}),
                    output: serde_json::json!({"result": "processed"}),
                    duration: Duration::from_millis(250),
                    success: true,
                    error: None,
                },
            ],
            timestamp: SystemTime::now(),
            execution_duration: Duration::from_millis(350),
            error: None,
        }
    }
    
    fn create_test_checkpoint() -> WorkflowCheckpoint {
        WorkflowCheckpoint {
            id: CheckpointId::new("test-checkpoint-1"),
            workflow_id: "test-workflow".to_string(),
            workflow_version: "1.0.0".to_string(),
            state: create_test_state(),
            metadata: CheckpointMetadata {
                created_at: SystemTime::now(),
                created_by: "test".to_string(),
                description: Some("Test checkpoint".to_string()),
                tags: vec!["test".to_string()],
                size_bytes: 1024,
                checksum: "initial".to_string(),
            },
            is_auto_checkpoint: false,
            parent_checkpoint_id: None,
        }
    }
    
    #[test]
    fn test_json_serialization_roundtrip() {
        let serializer = StateSerializer::json();
        let state = create_test_state();
        
        let serialized = serializer.serialize_state(&state).unwrap();
        let deserialized = serializer.deserialize_state(&serialized).unwrap();
        
        assert_eq!(state.current_step, deserialized.current_step);
        assert_eq!(state.execution_history.len(), deserialized.execution_history.len());
    }
    
    #[test]
    fn test_bincode_serialization_roundtrip() {
        let serializer = StateSerializer::bincode();
        let state = create_test_state();
        
        let serialized = serializer.serialize_state(&state).unwrap();
        let deserialized = serializer.deserialize_state(&serialized).unwrap();
        
        assert_eq!(state.current_step, deserialized.current_step);
        assert_eq!(state.execution_history.len(), deserialized.execution_history.len());
    }
    
    #[test]
    fn test_checkpoint_serialization_roundtrip() {
        let serializer = StateSerializer::json();
        let checkpoint = create_test_checkpoint();
        
        let serialized = serializer.serialize_checkpoint(&checkpoint).unwrap();
        let deserialized = serializer.deserialize_checkpoint(&serialized).unwrap();
        
        assert_eq!(checkpoint.id, deserialized.id);
        assert_eq!(checkpoint.workflow_id, deserialized.workflow_id);
    }
    
    #[test]
    fn test_checksum_calculation() {
        let data = b"test data for checksum";
        let checksum1 = StateSerializer::calculate_checksum(data);
        let checksum2 = StateSerializer::calculate_checksum(data);
        
        assert_eq!(checksum1, checksum2);
        assert!(!checksum1.is_empty());
    }
    
    #[test]
    fn test_checksum_verification() {
        let data = b"test data for verification";
        let checksum = StateSerializer::calculate_checksum(data);
        
        // Should succeed with correct checksum
        assert!(StateSerializer::verify_checksum(data, &checksum).unwrap());
        
        // Should fail with incorrect checksum
        assert!(StateSerializer::verify_checksum(data, "wrong_checksum").is_err());
    }
    
    #[test]
    fn test_file_serialization() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_checkpoint.json");
        
        let serializer = StateSerializer::json();
        let checkpoint = create_test_checkpoint();
        
        // Save to file
        serializer.save_checkpoint_to_file(&checkpoint, &file_path).unwrap();
        
        // Load from file
        let loaded = serializer.load_checkpoint_from_file(&file_path).unwrap();
        
        assert_eq!(checkpoint.id, loaded.id);
        assert_eq!(checkpoint.workflow_id, loaded.workflow_id);
    }
    
    #[test]
    fn test_state_file_serialization() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_state.json");
        
        let serializer = StateSerializer::json();
        let state = create_test_state();
        
        // Save to file
        serializer.save_state_to_file(&state, &file_path).unwrap();
        
        // Load from file
        let loaded = serializer.load_state_from_file(&file_path).unwrap();
        
        assert_eq!(state.current_step, loaded.current_step);
        assert_eq!(state.execution_history.len(), loaded.execution_history.len());
    }
    
    #[test]
    fn test_different_formats() {
        let json_serializer = StateSerializer::json();
        let bincode_serializer = StateSerializer::bincode();
        
        let state = create_test_state();
        
        let json_data = json_serializer.serialize_state(&state).unwrap();
        let bincode_data = bincode_serializer.serialize_state(&state).unwrap();
        
        // Bincode should be more compact
        assert!(bincode_data.len() < json_data.len());
        
        // Both should deserialize correctly
        let json_state = json_serializer.deserialize_state(&json_data).unwrap();
        let bincode_state = bincode_serializer.deserialize_state(&bincode_data).unwrap();
        
        assert_eq!(json_state.current_step, bincode_state.current_step);
    }
}