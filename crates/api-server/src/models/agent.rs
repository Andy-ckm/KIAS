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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_default() {
        assert_eq!(AgentStatus::default(), AgentStatus::Pending);
    }

    #[test]
    fn test_agent_status_serialization() {
        assert!(serde_json::to_string(&AgentStatus::Pending)
            .unwrap()
            .contains("Pending"));
        assert!(serde_json::to_string(&AgentStatus::Running)
            .unwrap()
            .contains("Running"));
        assert!(serde_json::to_string(&AgentStatus::Failed)
            .unwrap()
            .contains("Failed"));
    }

    #[test]
    fn test_agent_status_deserialization() {
        let status: AgentStatus = serde_json::from_str("\"Scheduled\"").unwrap();
        assert_eq!(status, AgentStatus::Scheduled);
        let status: AgentStatus = serde_json::from_str("\"Succeeded\"").unwrap();
        assert_eq!(status, AgentStatus::Succeeded);
    }

    #[test]
    fn test_resource_request_default() {
        let rr = ResourceRequest::default();
        assert!(rr.cpu.is_none());
        assert!(rr.memory.is_none());
        assert!(rr.gpu.is_none());
    }

    #[test]
    fn test_agent_spec_defaults() {
        let json = r#"{"name":"test-agent"}"#;
        let spec: AgentSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name, "test-agent");
        assert_eq!(spec.image, "python:3.11");
        assert_eq!(spec.command, vec!["python", "app.py"]);
        assert_eq!(spec.priority, "medium");
        assert!(spec.labels.is_empty());
        assert!(spec.env.is_empty());
    }

    #[test]
    fn test_agent_spec_custom_values() {
        let json = r#"{"name":"custom","image":"node:18","command":["node","server.js"],"priority":"high"}"#;
        let spec: AgentSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.image, "node:18");
        assert_eq!(spec.command, vec!["node", "server.js"]);
        assert_eq!(spec.priority, "high");
    }

    #[test]
    fn test_agent_from_spec() {
        let spec = AgentSpec {
            name: "test".to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string()],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let agent = Agent::from_spec(spec);
        assert!(!agent.id.is_empty());
        assert_eq!(agent.status, AgentStatus::Pending);
        assert!(agent.node_id.is_none());
        assert_eq!(agent.restart_count, 0);
        assert!(agent.start_time.is_none());
        assert!(!agent.created_at.is_empty());
        assert_eq!(agent.created_at, agent.updated_at);
    }

    #[test]
    fn test_agent_summary_from_agent() {
        let spec = AgentSpec {
            name: "my-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string()],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let agent = Agent::from_spec(spec);
        let summary = AgentSummary::from(&agent);
        assert_eq!(summary.id, agent.id);
        assert_eq!(summary.name, "my-agent");
        assert_eq!(summary.status, AgentStatus::Pending);
        assert!(summary.node_id.is_none());
    }

    #[test]
    fn test_agent_serialization_roundtrip() {
        let spec = AgentSpec {
            name: "roundtrip".to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string()],
            resource_request: None,
            labels: HashMap::new(),
            priority: "low".to_string(),
            env: HashMap::new(),
        };
        let agent = Agent::from_spec(spec);
        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: Agent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, agent.id);
        assert_eq!(deserialized.spec.name, "roundtrip");
        assert_eq!(deserialized.status, AgentStatus::Pending);
    }
}
