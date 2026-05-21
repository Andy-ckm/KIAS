//! Workflow checkpoint and restore module.
//! 
//! This module provides functionality to pause long-running workflows
//! and resume from the last checkpoint after a crash or interruption.

pub mod checkpoint_manager;
pub mod crash_recovery;
pub mod state_serializer;
pub mod storage;
pub mod types;

pub use checkpoint_manager::CheckpointManager;
pub use crash_recovery::CrashRecovery;
pub use state_serializer::StateSerializer;
pub use storage::CheckpointStorage;
pub use types::{WorkflowCheckpoint, CheckpointId, WorkflowState};