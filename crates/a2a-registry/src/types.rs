use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent status in the registry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is connected and operational
    Online,
    /// Agent disconnected gracefully
    Offline,
    /// Agent disconnected unexpectedly (Last Will and Testament)
    Lwt,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Offline => write!(f, "offline"),
            AgentStatus::Lwt => write!(f, "lwt"),
        }
    }
}

/// Agent Card — the structured identity document for an A2A agent.
///
/// Registered as a retained message on discovery topic:
/// `$a2a/v1/discovery/{org_id}/{unit_id}/{agent_id}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique agent identifier
    pub agent_id: String,

    /// Organization ID (multi-tenant isolation)
    pub org_id: String,

    /// Unit/team ID within the organization
    pub unit_id: String,

    /// Human-readable agent name
    pub name: String,

    /// Agent description
    pub description: Option<String>,

    /// Agent capabilities (skills, tools)
    pub capabilities: Vec<AgentCapability>,

    /// Supported interaction modes
    pub interaction_modes: Vec<InteractionMode>,

    /// Agent endpoint URL (for HTTP-based communication)
    pub endpoint: Option<String>,

    /// Version string
    pub version: String,

    /// Custom metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A capability that an agent provides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    /// Capability name (e.g., "text-generation", "code-review")
    pub name: String,

    /// Capability description
    pub description: Option<String>,

    /// Input schema (JSON Schema)
    pub input_schema: Option<serde_json::Value>,

    /// Output schema (JSON Schema)
    pub output_schema: Option<serde_json::Value>,
}

/// Supported interaction modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionMode {
    /// Request/response (synchronous)
    RequestResponse,
    /// Server-sent events (streaming)
    StreamingResponse,
    /// Multi-turn conversation
    MultiTurn,
    /// Load-balanced pool
    LoadBalancedPool,
    /// Task handoff between agents
    TaskHandoff,
}

/// Registration record with lifecycle metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    /// The agent card
    pub card: AgentCard,

    /// Current status
    pub status: AgentStatus,

    /// When the agent was first registered
    pub registered_at: DateTime<Utc>,

    /// When the agent was last seen (heartbeat)
    pub last_seen: DateTime<Utc>,

    /// When the status last changed
    pub status_changed_at: DateTime<Utc>,
}

/// Discovery event — emitted when an agent registers, updates, or disconnects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEvent {
    /// Event type
    pub event_type: DiscoveryEventType,

    /// The agent registration
    pub registration: AgentRegistration,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

/// Types of discovery events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryEventType {
    /// New agent registered
    AgentRegistered,
    /// Agent card updated
    AgentUpdated,
    /// Agent status changed
    StatusChanged,
    /// Agent deregistered
    AgentDeregistered,
}

/// Schema validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: vec![],
        }
    }

    pub fn fail(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }
}
