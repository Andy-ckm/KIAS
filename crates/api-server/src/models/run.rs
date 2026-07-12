use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRunRequest {
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

impl Default for StartRunRequest {
    fn default() -> Self {
        Self {
            input: String::new(),
            timeout_seconds: Some(30),
            max_retries: Some(0),
        }
    }
}

/// Retry and recovery requests must resupply the exact original input. KIAS
/// persists only its SHA-256 digest and byte count, never the raw value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayRunRequest {
    #[serde(default)]
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunStatus {
    pub fn from_storage(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            _ => Self::Failed,
        }
    }

    pub fn as_storage(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPolicyDecision {
    pub allowed: bool,
    pub policy_version: String,
    pub reasons: Vec<String>,
    pub constraints: RunConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConstraints {
    pub image: String,
    pub network: String,
    pub root_filesystem: String,
    pub host_mounts: bool,
    pub no_new_privileges: bool,
    pub cpu_limit: f64,
    pub memory_limit_bytes: u64,
    pub pids_limit: u32,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunLineage {
    pub retry_of: Option<String>,
    pub recovery_of: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub agent_id: String,
    pub status: RunStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub input_sha256: String,
    pub input_bytes: usize,
    pub policy: RunPolicyDecision,
    pub lineage: RunLineage,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunLogs {
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvidence {
    pub run: RunRecord,
    pub attempts: Vec<serde_json::Value>,
    pub final_execution: Option<serde_json::Value>,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub run_id: String,
    pub agent_id: String,
    pub replayable: bool,
    pub status: RunStatus,
    pub input_sha256: String,
    pub agent_spec_sha256: String,
    pub created_at: String,
    pub note: String,
}
