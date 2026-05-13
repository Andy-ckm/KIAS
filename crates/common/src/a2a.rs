//! # A2A Protocol (Agent-to-Agent Protocol)
//!
//! Inspired by Google's A2A protocol for standardized agent communication.
//! Provides capability discovery, task lifecycle management, and agent cards.
//!
//! Reference: https://github.com/google/A2A

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent Card - describes an agent's capabilities (A2A spec)
/// Clients discover agents via their AgentCard endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique agent identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Agent description
    pub description: String,
    /// Protocol version
    pub protocol_version: String,
    /// Agent version
    pub version: String,
    /// Base URL for the agent's A2A endpoint
    pub url: String,
    /// Supported capabilities
    pub capabilities: AgentCapabilities,
    /// Supported input/output content types
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
    /// Skills this agent provides
    pub skills: Vec<AgentSkill>,
    /// Authentication requirements
    pub authentication: Option<AuthInfo>,
    /// Provider metadata
    pub provider: Option<ProviderInfo>,
}

/// Agent capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    /// Supports streaming responses
    pub streaming: bool,
    /// Supports push notifications
    pub push_notifications: bool,
    /// Supports state transition history
    pub state_transition_history: bool,
}

/// A skill that an agent provides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    /// Skill identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// What this skill does
    pub description: String,
    /// Example prompts/inputs
    pub examples: Vec<String>,
    /// Tags for discovery
    pub tags: Vec<String>,
    /// Whether this skill is location-specific
    pub location_bound: bool,
}

/// Authentication information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInfo {
    /// Authentication schemes (e.g., "bearer", "api_key")
    pub schemes: Vec<String>,
    /// Whether credentials are required
    pub required: bool,
}

/// Provider metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub organization: String,
    pub url: Option<String>,
}

/// A2A Task - the central object for agent-to-agent task delegation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTask {
    /// Unique task identifier
    pub id: String,
    /// Current task status
    pub status: A2aTaskStatus,
    /// Task messages (conversation history)
    pub messages: Vec<A2aMessage>,
    /// Task metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Associated artifacts (files, outputs)
    pub artifacts: Vec<A2aArtifact>,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
}

/// A2A Task status (follows A2A spec lifecycle)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2aTaskStatus {
    /// Task submitted, waiting to be processed
    Submitted,
    /// Agent is working on the task
    Working,
    /// Task input required (human-in-the-loop)
    InputRequired,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task was rejected by the agent
    Rejected,
}

impl A2aTaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            A2aTaskStatus::Completed
                | A2aTaskStatus::Failed
                | A2aTaskStatus::Cancelled
                | A2aTaskStatus::Rejected
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            A2aTaskStatus::Submitted | A2aTaskStatus::Working | A2aTaskStatus::InputRequired
        )
    }
}

/// A2A Message - a single message in a task conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessage {
    /// Message role (user, agent, system)
    pub role: A2aRole,
    /// Message content parts
    pub parts: Vec<A2aPart>,
    /// Whether this message is the final one
    pub is_final: bool,
    /// Message metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Message role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2aRole {
    User,
    Agent,
    System,
}

/// A part of a message (text, file, data, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum A2aPart {
    /// Text content
    Text {
        text: String,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    /// File content (inline or by reference)
    File {
        name: Option<String>,
        mime_type: String,
        /// Inline base64-encoded data
        data: Option<String>,
        /// URL reference
        uri: Option<String>,
    },
    /// Structured data
    Data {
        data: serde_json::Value,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
}

/// A2A Artifact - output produced by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aArtifact {
    /// Artifact identifier
    pub id: String,
    /// Artifact name
    pub name: Option<String>,
    /// Content parts
    pub parts: Vec<A2aPart>,
    /// Artifact metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A2A Task send request (client → server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskSendRequest {
    /// Task ID (optional, server generates if omitted)
    pub id: Option<String>,
    /// Session ID for multi-turn conversations
    pub session_id: Option<String>,
    /// Message to send
    pub message: A2aMessage,
    /// Push notification configuration
    pub push_notification: Option<PushNotificationConfig>,
    /// Task history length to return
    pub history_length: Option<u32>,
    /// Task metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Push notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationConfig {
    /// Webhook URL
    pub url: String,
    /// Authentication token for the webhook
    pub token: Option<String>,
}

/// A2A Task response (server → client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskResponse {
    /// Response ID
    pub id: String,
    /// Task details
    pub task: A2aTask,
}

/// Agent-to-Agent handoff request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRequest {
    /// Source agent ID
    pub from_agent: String,
    /// Target agent ID
    pub to_agent: String,
    /// Task being handed off
    pub task_id: String,
    /// Reason for handoff
    pub reason: HandoffReason,
    /// Context to transfer
    pub context: serde_json::Value,
    /// Required capabilities for the target agent
    pub required_skills: Vec<String>,
}

/// Reason for agent handoff
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HandoffReason {
    /// Agent lacks the capability
    CapabilityGap,
    /// Load balancing
    LoadBalancing,
    /// Specialization (target agent is better suited)
    Specialization,
    /// Error recovery
    ErrorRecovery,
    /// Human-directed handoff
    HumanDirected,
    /// Cost optimization
    CostOptimization,
}

/// Agent registry entry for A2A discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistryEntry {
    /// Agent card
    pub card: AgentCard,
    /// When the agent was last seen
    pub last_heartbeat: DateTime<Utc>,
    /// Current health status
    pub health: AgentHealth,
    /// Current load (0.0 - 1.0)
    pub load: f64,
    /// Active task count
    pub active_tasks: u32,
}

/// Agent health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_card_creation() {
        let card = AgentCard {
            id: "agent-1".to_string(),
            name: "Code Assistant".to_string(),
            description: "Helps with coding tasks".to_string(),
            protocol_version: "1.0".to_string(),
            version: "0.1.0".to_string(),
            url: "http://localhost:8080/a2a".to_string(),
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: false,
                state_transition_history: true,
            },
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![AgentSkill {
                id: "code-review".to_string(),
                name: "Code Review".to_string(),
                description: "Reviews code for issues".to_string(),
                examples: vec!["Review this PR".to_string()],
                tags: vec!["coding".to_string(), "review".to_string()],
                location_bound: false,
            }],
            authentication: None,
            provider: None,
        };

        assert_eq!(card.id, "agent-1");
        assert!(card.capabilities.streaming);
        assert_eq!(card.skills.len(), 1);
    }

    #[test]
    fn test_task_status_lifecycle() {
        let submitted = A2aTaskStatus::Submitted;
        assert!(submitted.is_active());
        assert!(!submitted.is_terminal());

        let working = A2aTaskStatus::Working;
        assert!(working.is_active());
        assert!(!working.is_terminal());

        let completed = A2aTaskStatus::Completed;
        assert!(!completed.is_active());
        assert!(completed.is_terminal());

        let failed = A2aTaskStatus::Failed;
        assert!(!failed.is_active());
        assert!(failed.is_terminal());

        let cancelled = A2aTaskStatus::Cancelled;
        assert!(!cancelled.is_active());
        assert!(cancelled.is_terminal());

        let rejected = A2aTaskStatus::Rejected;
        assert!(!rejected.is_active());
        assert!(rejected.is_terminal());
    }

    #[test]
    fn test_a2a_task_creation() {
        let task = A2aTask {
            id: "task-1".to_string(),
            status: A2aTaskStatus::Submitted,
            messages: vec![A2aMessage {
                role: A2aRole::User,
                parts: vec![A2aPart::Text {
                    text: "Hello".to_string(),
                    metadata: None,
                }],
                is_final: false,
                metadata: HashMap::new(),
            }],
            metadata: HashMap::new(),
            artifacts: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(task.messages.len(), 1);
        assert_eq!(task.status, A2aTaskStatus::Submitted);
    }

    #[test]
    fn test_a2a_message_parts() {
        let text_part = A2aPart::Text {
            text: "hello".to_string(),
            metadata: None,
        };
        let file_part = A2aPart::File {
            name: Some("test.rs".to_string()),
            mime_type: "text/x-rust".to_string(),
            data: Some("Zm4gbWFpbigpIHt9".to_string()),
            uri: None,
        };
        let data_part = A2aPart::Data {
            data: serde_json::json!({"key": "value"}),
            metadata: None,
        };

        match text_part {
            A2aPart::Text { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("Expected Text part"),
        }
        match file_part {
            A2aPart::File { name, .. } => assert_eq!(name, Some("test.rs".to_string())),
            _ => panic!("Expected File part"),
        }
        match data_part {
            A2aPart::Data { data, .. } => assert_eq!(data["key"], "value"),
            _ => panic!("Expected Data part"),
        }
    }

    #[test]
    fn test_handoff_request() {
        let handoff = HandoffRequest {
            from_agent: "agent-1".to_string(),
            to_agent: "agent-2".to_string(),
            task_id: "task-1".to_string(),
            reason: HandoffReason::Specialization,
            context: serde_json::json!({"progress": 50}),
            required_skills: vec!["code-review".to_string()],
        };

        assert_eq!(handoff.reason, HandoffReason::Specialization);
    }

    #[test]
    fn test_agent_health() {
        assert_ne!(AgentHealth::Healthy, AgentHealth::Unhealthy);
        assert_eq!(AgentHealth::Unknown, AgentHealth::Unknown);
    }

    #[test]
    fn test_agent_capabilities_default() {
        let caps = AgentCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.push_notifications);
        assert!(!caps.state_transition_history);
    }

    #[test]
    fn test_push_notification_config() {
        let config = PushNotificationConfig {
            url: "https://example.com/webhook".to_string(),
            token: Some("secret".to_string()),
        };
        assert!(config.token.is_some());
    }

    #[test]
    fn test_task_send_request() {
        let req = A2aTaskSendRequest {
            id: Some("task-1".to_string()),
            session_id: Some("session-1".to_string()),
            message: A2aMessage {
                role: A2aRole::User,
                parts: vec![A2aPart::Text {
                    text: "Do something".to_string(),
                    metadata: None,
                }],
                is_final: true,
                metadata: HashMap::new(),
            },
            push_notification: None,
            history_length: Some(10),
            metadata: HashMap::new(),
        };

        assert!(req.id.is_some());
        assert!(req.push_notification.is_none());
    }
}
