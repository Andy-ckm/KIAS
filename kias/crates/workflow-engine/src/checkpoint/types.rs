use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Unique identifier for a workflow checkpoint
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(String);

impl CheckpointId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents the state of a workflow at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Current step index in the workflow
    pub current_step: usize,
    /// Variables and context data
    pub context: HashMap<String, serde_json::Value>,
    /// Execution history (step index -> result)
    pub execution_history: Vec<StepExecution>,
    /// Timestamp when this state was captured
    pub timestamp: SystemTime,
    /// Duration of execution up to this point
    pub execution_duration: Duration,
    /// Any error that occurred at this point
    pub error: Option<String>,
}

/// Record of a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecution {
    /// Step index
    pub step_index: usize,
    /// Step name or identifier
    pub step_name: String,
    /// Input data for this step
    pub input: serde_json::Value,
    /// Output data from this step
    pub output: serde_json::Value,
    /// Duration of this step
    pub duration: Duration,
    /// Whether this step completed successfully
    pub success: bool,
    /// Error message if step failed
    pub error: Option<String>,
}

/// A checkpoint that captures the complete state of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Unique identifier for this checkpoint
    pub id: CheckpointId,
    /// Workflow identifier
    pub workflow_id: String,
    /// Version of the workflow definition
    pub workflow_version: String,
    /// The captured workflow state
    pub state: WorkflowState,
    /// Metadata about the checkpoint
    pub metadata: CheckpointMetadata,
    /// Whether this checkpoint was created automatically (crash recovery)
    pub is_auto_checkpoint: bool,
    /// Parent checkpoint ID (for checkpoint chains)
    pub parent_checkpoint_id: Option<CheckpointId>,
}

/// Metadata about a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// When the checkpoint was created
    pub created_at: SystemTime,
    /// Who or what created the checkpoint
    pub created_by: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Size of the checkpoint data in bytes
    pub size_bytes: usize,
    /// Checksum for integrity verification
    pub checksum: String,
}

/// Result of a checkpoint operation
#[derive(Debug, Clone)]
pub enum CheckpointResult {
    /// Checkpoint was created successfully
    Created(CheckpointId),
    /// Checkpoint was restored successfully
    Restored(WorkflowState),
    /// Checkpoint was deleted
    Deleted,
    /// Checkpoint not found
    NotFound,
    /// Error occurred
    Error(String),
}

/// Configuration for checkpoint behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Maximum number of checkpoints to keep per workflow
    pub max_checkpoints_per_workflow: usize,
    /// Whether to enable automatic checkpoints on errors
    pub auto_checkpoint_on_error: bool,
    /// Whether to enable periodic checkpoints during long-running steps
    pub enable_periodic_checkpoints: bool,
    /// Interval for periodic checkpoints (in seconds)
    pub periodic_checkpoint_interval: Duration,
    /// Storage backend configuration
    pub storage_backend: StorageBackendConfig,
}

/// Storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackendConfig {
    /// Local filesystem storage
    Local { path: std::path::PathBuf },
    /// In-memory storage (for testing)
    Memory,
    /// Database storage
    Database { connection_string: String },
    /// Cloud storage
    Cloud { bucket: String, region: String },
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            max_checkpoints_per_workflow: 10,
            auto_checkpoint_on_error: true,
            enable_periodic_checkpoints: false,
            periodic_checkpoint_interval: Duration::from_secs(300), // 5 minutes
            storage_backend: StorageBackendConfig::Local {
                path: std::path::PathBuf::from(".workflow-checkpoints"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_checkpoint_id_creation() {
        let id = CheckpointId::new("test-123");
        assert_eq!(id.as_str(), "test-123");
        assert_eq!(id.to_string(), "test-123");
    }

    #[test]
    fn test_workflow_state_creation() {
        let state = WorkflowState {
            current_step: 5,
            context: HashMap::new(),
            execution_history: Vec::new(),
            timestamp: SystemTime::now(),
            execution_duration: Duration::from_secs(10),
            error: None,
        };
        
        assert_eq!(state.current_step, 5);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = WorkflowCheckpoint {
            id: CheckpointId::new("checkpoint-1"),
            workflow_id: "workflow-1".to_string(),
            workflow_version: "1.0.0".to_string(),
            state: WorkflowState {
                current_step: 3,
                context: HashMap::new(),
                execution_history: Vec::new(),
                timestamp: SystemTime::now(),
                execution_duration: Duration::from_secs(5),
                error: None,
            },
            metadata: CheckpointMetadata {
                created_at: SystemTime::now(),
                created_by: "test".to_string(),
                description: Some("Test checkpoint".to_string()),
                tags: vec!["test".to_string()],
                size_bytes: 1024,
                checksum: "abc123".to_string(),
            },
            is_auto_checkpoint: false,
            parent_checkpoint_id: None,
        };
        
        assert_eq!(checkpoint.workflow_id, "workflow-1");
        assert!(!checkpoint.is_auto_checkpoint);
    }

    #[test]
    fn test_default_checkpoint_config() {
        let config = CheckpointConfig::default();
        
        assert_eq!(config.max_checkpoints_per_workflow, 10);
        assert!(config.auto_checkpoint_on_error);
        assert!(!config.enable_periodic_checkpoints);
        assert_eq!(config.periodic_checkpoint_interval, Duration::from_secs(300));
    }

    #[test]
    fn test_step_execution_creation() {
        let step = StepExecution {
            step_index: 1,
            step_name: "process_data".to_string(),
            input: serde_json::json!({"data": "test"}),
            output: serde_json::json!({"result": "success"}),
            duration: Duration::from_millis(100),
            success: true,
            error: None,
        };
        
        assert_eq!(step.step_index, 1);
        assert!(step.success);
        assert!(step.error.is_none());
    }
}