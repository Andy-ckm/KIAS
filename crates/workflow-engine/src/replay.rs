//! # Durable Execution with Replay
//!
//! Inspired by Chidori's deterministic replay system.
//! Enables zero-cost checkpointing: save a workflow's execution log,
//! replay it later for identical output with zero LLM calls.
//!
//! Key concepts:
//! - ExecutionLog: records every side effect during workflow execution
//! - ReplayMode: can replay from any checkpoint with cached results
//! - Deterministic execution: all side effects go through logged host functions

use crate::state::WorkflowState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A logged side effect during workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEntry {
    /// Unique entry ID
    pub id: String,
    /// Which node produced this entry
    pub node_id: String,
    /// Type of side effect
    pub effect_type: EffectType,
    /// Input that was provided
    pub input: serde_json::Value,
    /// Output that was produced
    pub output: serde_json::Value,
    /// When this happened
    pub timestamp: DateTime<Utc>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether this was a cached replay
    pub from_cache: bool,
}

/// Types of side effects that can be logged
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EffectType {
    /// LLM call
    LlmCall,
    /// Shell command execution
    ShellExec,
    /// HTTP request
    HttpCall,
    /// File read/write
    FileIO,
    /// Tool invocation
    ToolCall,
    /// Sub-workflow execution
    SubWorkflow,
    /// Human input
    HumanInput,
    /// State mutation
    StateMutation,
}

/// Execution log for a workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// Workflow ID
    pub workflow_id: String,
    /// Execution run ID (unique per execution)
    pub run_id: String,
    /// Ordered list of side effects
    pub entries: Vec<ExecutionEntry>,
    /// Final state after execution
    pub final_state: Option<WorkflowState>,
    /// When execution started
    pub started_at: DateTime<Utc>,
    /// When execution ended
    pub ended_at: Option<DateTime<Utc>>,
    /// Total execution time in milliseconds
    pub total_duration_ms: Option<u64>,
    /// Whether this execution was a replay
    pub is_replay: bool,
}

impl ExecutionLog {
    pub fn new(workflow_id: &str, run_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            run_id: run_id.to_string(),
            entries: Vec::new(),
            final_state: None,
            started_at: Utc::now(),
            ended_at: None,
            total_duration_ms: None,
            is_replay: false,
        }
    }

    /// Add an execution entry
    pub fn record(&mut self, entry: ExecutionEntry) {
        self.entries.push(entry);
    }

    /// Mark execution as complete
    pub fn complete(&mut self, final_state: WorkflowState) {
        self.final_state = Some(final_state);
        self.ended_at = Some(Utc::now());
        self.total_duration_ms = Some(
            self.ended_at
                .unwrap()
                .signed_duration_since(self.started_at)
                .num_milliseconds() as u64,
        );
    }

    /// Get entries for a specific node
    pub fn entries_for_node(&self, node_id: &str) -> Vec<&ExecutionEntry> {
        self.entries
            .iter()
            .filter(|e| e.node_id == node_id)
            .collect()
    }

    /// Get the last entry for a specific node and effect type
    pub fn last_entry(&self, node_id: &str, effect_type: &EffectType) -> Option<&ExecutionEntry> {
        self.entries
            .iter()
            .rfind(|e| e.node_id == node_id && e.effect_type == *effect_type)
    }
}

/// Replay store - stores execution logs for replay
pub struct ReplayStore {
    logs: dashmap::DashMap<String, ExecutionLog>,
}

impl Default for ReplayStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayStore {
    pub fn new() -> Self {
        Self {
            logs: dashmap::DashMap::new(),
        }
    }

    /// Store an execution log
    pub fn store(&self, log: ExecutionLog) {
        let key = format!("{}:{}", log.workflow_id, log.run_id);
        self.logs.insert(key, log);
    }

    /// Get an execution log
    pub fn get(&self, workflow_id: &str, run_id: &str) -> Option<ExecutionLog> {
        let key = format!("{}:{}", workflow_id, run_id);
        self.logs.get(&key).map(|v| v.clone())
    }

    /// Get the latest execution log for a workflow
    pub fn get_latest(&self, workflow_id: &str) -> Option<ExecutionLog> {
        self.logs
            .iter()
            .filter(|entry| entry.value().workflow_id == workflow_id)
            .max_by_key(|entry| entry.value().started_at)
            .map(|entry| entry.value().clone())
    }

    /// List all run IDs for a workflow
    pub fn list_runs(&self, workflow_id: &str) -> Vec<String> {
        self.logs
            .iter()
            .filter(|entry| entry.value().workflow_id == workflow_id)
            .map(|entry| entry.value().run_id.clone())
            .collect()
    }
}

/// Replay engine - replays a workflow from an execution log
pub struct ReplayEngine {
    store: ReplayStore,
}

impl ReplayEngine {
    pub fn new(store: ReplayStore) -> Self {
        Self { store }
    }

    /// Check if a workflow can be replayed (has a previous execution log)
    pub fn can_replay(&self, workflow_id: &str) -> bool {
        self.store.get_latest(workflow_id).is_some()
    }

    /// Get cached output for a node+effect combination during replay
    pub fn get_cached_output(
        &self,
        workflow_id: &str,
        run_id: &str,
        node_id: &str,
        effect_type: &EffectType,
        input: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let log = self.store.get(workflow_id, run_id)?;
        log.entries
            .iter()
            .find(|e| e.node_id == node_id && e.effect_type == *effect_type && e.input == *input)
            .map(|e| e.output.clone())
    }

    /// Create a replay execution log from an original log
    pub fn create_replay_log(&self, original: &ExecutionLog) -> ExecutionLog {
        let mut replay =
            ExecutionLog::new(&original.workflow_id, &uuid::Uuid::new_v4().to_string());
        replay.is_replay = true;
        replay
    }
}

/// Execution recorder - wraps side effects with logging
pub struct ExecutionRecorder {
    log: ExecutionLog,
}

impl ExecutionRecorder {
    pub fn new(workflow_id: &str, run_id: &str) -> Self {
        Self {
            log: ExecutionLog::new(workflow_id, run_id),
        }
    }

    /// Record a side effect
    pub fn record(
        &mut self,
        node_id: &str,
        effect_type: EffectType,
        input: serde_json::Value,
        output: serde_json::Value,
        duration_ms: u64,
    ) {
        let entry = ExecutionEntry {
            id: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.to_string(),
            effect_type,
            input,
            output,
            timestamp: Utc::now(),
            duration_ms,
            from_cache: false,
        };
        self.log.record(entry);
    }

    /// Record a cached replay result
    pub fn record_cached(
        &mut self,
        node_id: &str,
        effect_type: EffectType,
        input: serde_json::Value,
        output: serde_json::Value,
    ) {
        let entry = ExecutionEntry {
            id: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.to_string(),
            effect_type,
            input,
            output,
            timestamp: Utc::now(),
            duration_ms: 0,
            from_cache: true,
        };
        self.log.record(entry);
    }

    /// Finalize the log
    pub fn finalize(mut self, final_state: WorkflowState) -> ExecutionLog {
        self.log.complete(final_state);
        self.log
    }

    /// Get the current log (for inspection)
    pub fn log(&self) -> &ExecutionLog {
        &self.log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_log_lifecycle() {
        let mut log = ExecutionLog::new("wf-1", "run-1");
        assert_eq!(log.entries.len(), 0);
        assert!(log.ended_at.is_none());

        log.record(ExecutionEntry {
            id: "entry-1".to_string(),
            node_id: "node-1".to_string(),
            effect_type: EffectType::LlmCall,
            input: serde_json::json!({"prompt": "hello"}),
            output: serde_json::json!({"response": "hi"}),
            timestamp: Utc::now(),
            duration_ms: 100,
            from_cache: false,
        });

        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries_for_node("node-1").len(), 1);
        assert_eq!(log.entries_for_node("node-2").len(), 0);
    }

    #[test]
    fn test_replay_store() {
        let store = ReplayStore::new();
        let log = ExecutionLog::new("wf-1", "run-1");
        store.store(log);

        assert!(store.get("wf-1", "run-1").is_some());
        assert!(store.get("wf-1", "run-2").is_none());

        let latest = store.get_latest("wf-1");
        assert!(latest.is_some());

        let runs = store.list_runs("wf-1");
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn test_replay_engine() {
        let store = ReplayStore::new();
        let mut log = ExecutionLog::new("wf-1", "run-1");
        log.record(ExecutionEntry {
            id: "entry-1".to_string(),
            node_id: "node-1".to_string(),
            effect_type: EffectType::ShellExec,
            input: serde_json::json!({"command": "echo hello"}),
            output: serde_json::json!({"stdout": "hello\n"}),
            timestamp: Utc::now(),
            duration_ms: 50,
            from_cache: false,
        });
        store.store(log);

        let engine = ReplayEngine::new(store);
        assert!(engine.can_replay("wf-1"));
        assert!(!engine.can_replay("wf-2"));

        let cached = engine.get_cached_output(
            "wf-1",
            "run-1",
            "node-1",
            &EffectType::ShellExec,
            &serde_json::json!({"command": "echo hello"}),
        );
        assert!(cached.is_some());
        assert_eq!(cached.unwrap()["stdout"], "hello\n");
    }

    #[test]
    fn test_execution_recorder() {
        let mut recorder = ExecutionRecorder::new("wf-1", "run-1");

        recorder.record(
            "node-1",
            EffectType::LlmCall,
            serde_json::json!({"prompt": "summarize"}),
            serde_json::json!({"result": "summary"}),
            200,
        );

        recorder.record_cached(
            "node-2",
            EffectType::ShellExec,
            serde_json::json!({"cmd": "ls"}),
            serde_json::json!({"output": "file.txt"}),
        );

        assert_eq!(recorder.log().entries.len(), 2);
        assert!(recorder.log().entries[1].from_cache);
    }

    #[test]
    fn test_last_entry() {
        let mut log = ExecutionLog::new("wf-1", "run-1");

        for i in 0..3 {
            log.record(ExecutionEntry {
                id: format!("entry-{}", i),
                node_id: "node-1".to_string(),
                effect_type: EffectType::LlmCall,
                input: serde_json::json!({"attempt": i}),
                output: serde_json::json!({"result": i}),
                timestamp: Utc::now(),
                duration_ms: 100,
                from_cache: false,
            });
        }

        let last = log.last_entry("node-1", &EffectType::LlmCall);
        assert!(last.is_some());
        assert_eq!(last.unwrap().id, "entry-2");
    }

    #[test]
    fn test_replay_log_creation() {
        let store = ReplayStore::new();
        let original = ExecutionLog::new("wf-1", "run-1");
        store.store(original);

        let engine = ReplayEngine::new(store);
        let original = engine.store.get("wf-1", "run-1").unwrap();
        let replay = engine.create_replay_log(&original);

        assert!(replay.is_replay);
        assert_eq!(replay.workflow_id, "wf-1");
        assert_ne!(replay.run_id, "run-1");
    }

    #[test]
    fn test_effect_types() {
          let types = [
            EffectType::LlmCall,
            EffectType::ShellExec,
            EffectType::HttpCall,
            EffectType::FileIO,
            EffectType::ToolCall,
            EffectType::SubWorkflow,
            EffectType::HumanInput,
            EffectType::StateMutation,
        ];
        assert_eq!(types.len(), 8);
    }
}
