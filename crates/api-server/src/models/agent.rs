use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

/// Agent lifecycle status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub enum AgentStatus {
    #[default]
    Pending,
    Scheduled,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

/// Resource requests / limits for an agent
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceRequest {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub gpu: Option<String>,
}

/// Agent specification — what the user requests
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AgentSpec {
    #[validate(length(min = 1, max = 128))]
    pub name: String,
    #[serde(default = "default_image")]
    pub image: String,
    #[serde(default = "default_command")]
    pub command: Vec<String>,
    #[serde(default)]
    pub resource_request: Option<ResourceRequest>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_image() -> String {
    "python:3.11".to_string()
}
fn default_command() -> Vec<String> {
    vec!["python".to_string(), "app.py".to_string()]
}
fn default_priority() -> String {
    "medium".to_string()
}

/// Full agent object returned by the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub spec: AgentSpec,
    pub status: AgentStatus,
    pub node_id: Option<String>,
    pub resource_usage: ResourceRequest,
    pub created_at: String,
    pub updated_at: String,
    pub start_time: Option<String>,
    pub restart_count: u32,
}

impl Agent {
    /// Create a new Agent in Pending state from a spec
    pub fn from_spec(spec: AgentSpec) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            spec,
            status: AgentStatus::Pending,
            node_id: None,
            resource_usage: ResourceRequest::default(),
            created_at: now.clone(),
            updated_at: now,
            start_time: None,
            restart_count: 0,
        }
    }
}

/// Summary returned in list endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub node_id: Option<String>,
}

impl From<&Agent> for AgentSummary {
    fn from(a: &Agent) -> Self {
        Self {
            id: a.id.clone(),
            name: a.spec.name.clone(),
            status: a.status.clone(),
            node_id: a.node_id.clone(),
        }
    }
}
