//! # A2A Protocol Standard Support
//!
//! Implements the Google A2A (Agent-to-Agent) protocol specification for
//! standardized inter-agent communication.
//!
//! ## A2A Protocol Overview
//!
//! The A2A protocol enables agents to communicate with each other through
//! a standardized task-based interface. Key concepts include:
//!
//! - **AgentCard**: Service discovery endpoint describing agent capabilities
//! - **Task**: The central unit of work with lifecycle states
//! - **Messages**: Conversations within a task
//! - **Artifacts**: Outputs produced by agent work
//!
//! ## A2A Endpoints (per spec)
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/.well-known/agent.json` | Agent Card (self-description) |
//! | GET | `/a2a/v1/agents` | List all registered agents |
//! | GET | `/a2a/v1/agents/:id` | Get specific agent card |
//! | POST | `/a2a/v1/tasks` | Send a task to an agent |
//! | GET | `/a2a/v1/tasks/:id` | Get task status and details |
//! | POST | `/a2a/v1/tasks/:id/cancel` | Cancel an active task |
//! | DELETE | `/a2a/v1/tasks/:id` | Delete a completed task |
//! | GET | `/a2a/v1/tasks/:id/stream` | SSE stream for task updates |
//!
//! ## Protocol Flow
//!
//! ```text
//! Client                    A2A Server                    Agent
//!   │                            │                           │
//!   ├─ GET /.well-known/agent.json ──▶│ (AgentCard discovery)  │
//!   │◀── AgentCard ──────────────┤                           │
//!   │                            │                           │
//!   ├─ POST /a2a/v1/tasks ──────▶│                           │
//!   │   (submit task)            ├─ route ──────────────────▶│
//!   │◀── Task(Working) ─────────┤                           │
//!   │                            │◀── status update ──────────┤
//!   ├─ GET /a2a/v1/tasks/:id ───▶│                           │
//!   │◀── Task(status) ───────────┤                           │
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Agent Card (A2A Service Discovery)
// ---------------------------------------------------------------------------

/// Agent Card — the self-description document every A2A agent must expose.
///
/// Per the A2A spec, clients discover agents by fetching their AgentCard at
/// `/.well-known/agent.json`. This describes what the agent can do,
/// how to authenticate, and what content types it supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique identifier for this agent
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this agent does
    pub description: String,
    /// A2A protocol version (e.g., "1.0")
    pub protocol_version: String,
    /// Implementation version of this agent
    pub version: String,
    /// Base URL for this agent's A2A endpoint
    pub url: String,
    /// Capabilities this agent supports
    pub capabilities: AgentCapabilities,
    /// Supported input content types
    pub input_modes: Vec<String>,
    /// Supported output content types
    pub output_modes: Vec<String>,
    /// Skills / capabilities exposed by this agent
    pub skills: Vec<AgentSkill>,
    /// Authentication requirements (None = no auth)
    pub authentication: Option<AuthenticationInfo>,
    /// Provider metadata (org that owns this agent)
    pub provider: Option<ProviderInfo>,
}

/// Capabilities supported by an agent
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    /// Supports streaming (SSE) responses
    pub streaming: bool,
    /// Supports push notifications (webhook)
    pub push_notifications: bool,
    /// Supports returning full state transition history
    pub state_transition_history: bool,
}

/// A named skill / capability exposed by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    /// Unique skill identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// What this skill does
    pub description: String,
    /// Example prompts that exercise this skill
    pub examples: Vec<String>,
    /// Tags for skill discovery and filtering
    pub tags: Vec<String>,
    /// Whether this skill requires a specific location
    pub location_bound: bool,
}

/// Authentication scheme description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationInfo {
    /// Supported auth schemes (e.g., ["bearer", "api_key"])
    pub schemes: Vec<String>,
    /// Whether authentication is required to call this agent
    pub required: bool,
}

/// Provider / vendor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Organization name
    pub organization: String,
    /// Optional URL to the provider's website
    pub url: Option<String>,
}

// ---------------------------------------------------------------------------
// A2A Task State Machine
// ---------------------------------------------------------------------------

/// A2A Task State — the lifecycle states defined by the A2A specification.
///
/// ```text
///                    ┌───────────────┐
///                    │   Submitted   │ ← Initial state
///                    └───────┬───────┘
///                            │
///                   ┌────────▼────────┐
///                   │    Working      │ ← Agent actively processing
///                   └────────┬────────┘
///                            │
///              ┌─────────────┼─────────────┐
///              │             │             │
///     ┌─────────▼──────┐ ┌───▼────┐ ┌──────▼──────┐
///     │ InputRequired  │ │Completed│ │   Failed    │
///     └────────────────┘ └────────┘ └─────────────┘
///              │
///     ┌────────▼────────┐
///     │   Cancelled     │
///     └─────────────────┘
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    /// Task submitted, waiting to be picked up
    Submitted,
    /// Agent is actively working on this task
    Working,
    /// Agent requires additional input from the user (human-in-the-loop)
    InputRequired,
    /// Task completed successfully
    Completed,
    /// Task failed during processing
    Failed,
    /// Task was cancelled by the client
    Cancelled,
    /// Agent explicitly rejected the task
    Rejected,
}

impl TaskState {
    /// Returns true if this is a terminal (final) state.
    ///
    /// Terminal states cannot transition further without client intervention.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled | TaskState::Rejected
        )
    }

    /// Returns true if this is an active (non-terminal) state.
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// Returns the human-readable name of this state.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Submitted => "submitted",
            TaskState::Working => "working",
            TaskState::InputRequired => "input-required",
            TaskState::Completed => "completed",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
            TaskState::Rejected => "rejected",
        }
    }
}

/// Historical record of a state transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state
    pub from: TaskState,
    /// New state
    pub to: TaskState,
    /// When the transition occurred
    pub timestamp: DateTime<Utc>,
    /// Optional reason for the transition
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// A2A Task
// ---------------------------------------------------------------------------

/// A2A Task — the central object representing a unit of work in the A2A protocol.
///
/// A task groups a series of messages between a client and an agent, along with
/// any artifacts produced during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATask {
    /// Unique task identifier (UUID recommended)
    pub id: String,
    /// Current lifecycle state
    pub state: TaskState,
    /// All messages in this task's conversation history
    pub messages: Vec<TaskMessage>,
    /// Artifacts produced by the agent during this task
    pub artifacts: Vec<TaskArtifact>,
    /// Arbitrary key-value metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// State transition history (if capabilities.state_transition_history = true)
    #[serde(default)]
    pub state_history: Vec<StateTransition>,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task was last updated
    pub updated_at: DateTime<Utc>,
}

impl A2ATask {
    /// Create a new task in the `Submitted` state.
    pub fn new(id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            state: TaskState::Submitted,
            messages: Vec::new(),
            artifacts: Vec::new(),
            metadata: HashMap::new(),
            state_history: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Transition to a new state, recording the transition in history.
    pub fn transition_to(&mut self, new_state: TaskState, reason: Option<String>) {
        let transition = StateTransition {
            from: self.state.clone(),
            to: new_state.clone(),
            timestamp: Utc::now(),
            reason,
        };
        self.state_history.push(transition);
        self.state = new_state;
        self.updated_at = Utc::now();
    }

    /// Add a message to the task's conversation history.
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(TaskMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Add an artifact produced by the agent.
    pub fn add_artifact(&mut self, name: String, content: String) {
        self.artifacts.push(TaskArtifact {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            content,
            mime_type: "text/plain".to_string(),
        });
        self.updated_at = Utc::now();
    }
}

/// A single message in a task's conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    /// Unique message identifier
    pub id: String,
    /// Who authored this message
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// When the message was sent
    pub timestamp: DateTime<Utc>,
}

/// Who authored a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Agent,
    System,
}

/// An artifact produced during task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskArtifact {
    /// Unique artifact identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Artifact content
    pub content: String,
    /// MIME type of the content
    pub mime_type: String,
}

// ---------------------------------------------------------------------------
// A2A Endpoint Definitions
// ---------------------------------------------------------------------------

/// A2A Standard Endpoint Registry
///
/// These are the paths defined by the A2A protocol specification.
/// Agents SHOULD implement all endpoints relevant to their capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2AEndpoint {
    /// GET /.well-known/agent.json — Agent Card discovery
    WellKnownAgentCard,
    /// GET /a2a/v1/agents — List all registered agents
    ListAgents,
    /// GET /a2a/v1/agents/:id — Get specific agent card
    GetAgent,
    /// POST /a2a/v1/tasks — Submit a new task
    CreateTask,
    /// GET /a2a/v1/tasks/:id — Get task status
    GetTask,
    /// POST /a2a/v1/tasks/:id/cancel — Cancel a task
    CancelTask,
    /// DELETE /a2a/v1/tasks/:id — Delete a task
    DeleteTask,
    /// GET /a2a/v1/tasks/:id/stream — SSE stream for task updates
    StreamTask,
    /// POST /a2a/v1/fire — Synchronous fire-and-wait invocation
    FireAgent,
}

impl A2AEndpoint {
    /// Returns the HTTP method for this endpoint.
    pub fn method(&self) -> &'static str {
        match self {
            A2AEndpoint::WellKnownAgentCard => "GET",
            A2AEndpoint::ListAgents => "GET",
            A2AEndpoint::GetAgent => "GET",
            A2AEndpoint::CreateTask => "POST",
            A2AEndpoint::GetTask => "GET",
            A2AEndpoint::CancelTask => "POST",
            A2AEndpoint::DeleteTask => "DELETE",
            A2AEndpoint::StreamTask => "GET",
            A2AEndpoint::FireAgent => "POST",
        }
    }

    /// Returns the URL path for this endpoint.
    pub fn path(&self) -> &'static str {
        match self {
            A2AEndpoint::WellKnownAgentCard => "/.well-known/agent.json",
            A2AEndpoint::ListAgents => "/a2a/v1/agents",
            A2AEndpoint::GetAgent => "/a2a/v1/agents/{agent_id}",
            A2AEndpoint::CreateTask => "/a2a/v1/tasks",
            A2AEndpoint::GetTask => "/a2a/v1/tasks/{task_id}",
            A2AEndpoint::CancelTask => "/a2a/v1/tasks/{task_id}/cancel",
            A2AEndpoint::DeleteTask => "/a2a/v1/tasks/{task_id}",
            A2AEndpoint::StreamTask => "/a2a/v1/tasks/{task_id}/stream",
            A2AEndpoint::FireAgent => "/a2a/v1/fire",
        }
    }

    /// Returns the path with templated parameters substituted.
    ///
    /// # Arguments
    /// * `agent_id` - Substitute for `{agent_id}` (if present)
    /// * `task_id` - Substitute for `{task_id}` (if present)
    pub fn resolved_path(&self, agent_id: Option<&str>, task_id: Option<&str>) -> String {
        let path = self.path();
        if let Some(aid) = agent_id {
            return path.replace("{agent_id}", aid);
        }
        if let Some(tid) = task_id {
            return path.replace("{task_id}", tid);
        }
        path.to_string()
    }
}

/// A resolved endpoint with method + URL ready to use
#[derive(Debug, Clone)]
pub struct ResolvedEndpoint {
    /// HTTP method
    pub method: String,
    /// Resolved URL path
    pub path: String,
}

impl From<A2AEndpoint> for ResolvedEndpoint {
    fn from(ep: A2AEndpoint) -> Self {
        Self {
            method: ep.method().to_string(),
            path: ep.path().to_string(),
        }
    }
}

impl From<A2AEndpoint> for (&'static str, &'static str) {
    fn from(ep: A2AEndpoint) -> Self {
        (ep.method(), ep.path())
    }
}

// ---------------------------------------------------------------------------
// Default AgentCard builder
// ---------------------------------------------------------------------------

/// Builder for creating a standard AgentCard
#[derive(Debug, Clone, Default)]
pub struct AgentCardBuilder {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    url: Option<String>,
    version: Option<String>,
    streaming: bool,
    push_notifications: bool,
    state_transition_history: bool,
    input_modes: Vec<String>,
    output_modes: Vec<String>,
    skills: Vec<AgentSkill>,
    auth_required: bool,
    organization: Option<String>,
}

impl AgentCardBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn streaming(mut self, enabled: bool) -> Self {
        self.streaming = enabled;
        self
    }

    pub fn push_notifications(mut self, enabled: bool) -> Self {
        self.push_notifications = enabled;
        self
    }

    pub fn state_transition_history(mut self, enabled: bool) -> Self {
        self.state_transition_history = enabled;
        self
    }

    pub fn input_modes(mut self, modes: Vec<String>) -> Self {
        self.input_modes = modes;
        self
    }

    pub fn output_modes(mut self, modes: Vec<String>) -> Self {
        self.output_modes = modes;
        self
    }

    pub fn add_skill(mut self, skill: AgentSkill) -> Self {
        self.skills.push(skill);
        self
    }

    pub fn auth_required(mut self, required: bool) -> Self {
        self.auth_required = required;
        self
    }

    pub fn organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }

    pub fn build(self) -> AgentCard {
        AgentCard {
            id: self.id.unwrap_or_else(|| "unknown".to_string()),
            name: self.name.unwrap_or_else(|| "Unknown Agent".to_string()),
            description: self
                .description
                .unwrap_or_else(|| "No description provided".to_string()),
            protocol_version: "1.0".to_string(),
            version: self.version.unwrap_or_else(|| "0.1.0".to_string()),
            url: self
                .url
                .unwrap_or_else(|| "http://localhost:8080/a2a/v1".to_string()),
            capabilities: AgentCapabilities {
                streaming: self.streaming,
                push_notifications: self.push_notifications,
                state_transition_history: self.state_transition_history,
            },
            input_modes: self.input_modes,
            output_modes: self.output_modes,
            skills: self.skills,
            authentication: Some(AuthenticationInfo {
                schemes: vec!["bearer".to_string()],
                required: self.auth_required,
            }),
            provider: self.organization.map(|org| ProviderInfo {
                organization: org,
                url: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // AgentCard Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_agent_card_builder_minimal() {
        let card = AgentCardBuilder::new()
            .id("test-agent")
            .name("Test Agent")
            .description("A test agent")
            .url("http://localhost:8080/a2a/v1")
            .build();

        assert_eq!(card.id, "test-agent");
        assert_eq!(card.name, "Test Agent");
        assert_eq!(card.protocol_version, "1.0");
        assert!(card.authentication.is_some());
    }

    #[test]
    fn test_agent_card_builder_with_skills() {
        let card = AgentCardBuilder::new()
            .id("code-agent")
            .name("Code Agent")
            .description("Helps with coding")
            .add_skill(AgentSkill {
                id: "code-review".to_string(),
                name: "Code Review".to_string(),
                description: "Reviews code".to_string(),
                examples: vec!["review this PR".to_string()],
                tags: vec!["coding".to_string(), "review".to_string()],
                location_bound: false,
            })
            .add_skill(AgentSkill {
                id: "refactor".to_string(),
                name: "Refactor".to_string(),
                description: "Refactors code".to_string(),
                examples: vec!["clean up this module".to_string()],
                tags: vec!["coding".to_string(), "refactor".to_string()],
                location_bound: false,
            })
            .build();

        assert_eq!(card.skills.len(), 2);
        assert_eq!(card.skills[0].id, "code-review");
        assert_eq!(card.skills[1].id, "refactor");
    }

    #[test]
    fn test_agent_capabilities_default() {
        let caps = AgentCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.push_notifications);
        assert!(!caps.state_transition_history);
    }

    #[test]
    fn test_agent_capabilities_all_enabled() {
        let caps = AgentCapabilities {
            streaming: true,
            push_notifications: true,
            state_transition_history: true,
        };
        assert!(caps.streaming);
        assert!(caps.push_notifications);
        assert!(caps.state_transition_history);
    }

    // -------------------------------------------------------------------------
    // TaskState Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_task_state_is_terminal() {
        assert!(!TaskState::Submitted.is_terminal());
        assert!(!TaskState::Working.is_terminal());
        assert!(!TaskState::InputRequired.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(TaskState::Rejected.is_terminal());
    }

    #[test]
    fn test_task_state_is_active() {
        assert!(TaskState::Submitted.is_active());
        assert!(TaskState::Working.is_active());
        assert!(TaskState::InputRequired.is_active());
        assert!(!TaskState::Completed.is_active());
        assert!(!TaskState::Failed.is_active());
    }

    #[test]
    fn test_task_state_as_str() {
        assert_eq!(TaskState::Submitted.as_str(), "submitted");
        assert_eq!(TaskState::Working.as_str(), "working");
        assert_eq!(TaskState::InputRequired.as_str(), "input-required");
        assert_eq!(TaskState::Completed.as_str(), "completed");
        assert_eq!(TaskState::Failed.as_str(), "failed");
        assert_eq!(TaskState::Cancelled.as_str(), "cancelled");
        assert_eq!(TaskState::Rejected.as_str(), "rejected");
    }

    #[test]
    fn test_task_state_serde_roundtrip() {
        let states = vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Cancelled,
            TaskState::Rejected,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let decoded: TaskState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, decoded);
        }
    }

    // -------------------------------------------------------------------------
    // A2ATask Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_a2a_task_new_is_submitted() {
        let task = A2ATask::new("task-123".to_string());
        assert_eq!(task.id, "task-123");
        assert_eq!(task.state, TaskState::Submitted);
        assert!(task.messages.is_empty());
        assert!(task.artifacts.is_empty());
        assert!(task.state_history.is_empty());
    }

    #[test]
    fn test_a2a_task_transition_to() {
        let mut task = A2ATask::new("task-123".to_string());
        assert_eq!(task.state, TaskState::Submitted);

        task.transition_to(TaskState::Working, None);
        assert_eq!(task.state, TaskState::Working);
        assert_eq!(task.state_history.len(), 1);
        assert_eq!(task.state_history[0].from, TaskState::Submitted);
        assert_eq!(task.state_history[0].to, TaskState::Working);

        task.transition_to(TaskState::Completed, Some("All done".to_string()));
        assert_eq!(task.state, TaskState::Completed);
        assert_eq!(task.state_history.len(), 2);
        assert_eq!(task.state_history[1].reason, Some("All done".to_string()));
    }

    #[test]
    fn test_a2a_task_add_message() {
        let mut task = A2ATask::new("task-123".to_string());
        task.add_message(MessageRole::User, "Hello agent".to_string());
        task.add_message(MessageRole::Agent, "Hello user".to_string());

        assert_eq!(task.messages.len(), 2);
        assert_eq!(task.messages[0].role, MessageRole::User);
        assert_eq!(task.messages[0].content, "Hello agent");
        assert_eq!(task.messages[1].role, MessageRole::Agent);
    }

    #[test]
    fn test_a2a_task_add_artifact() {
        let mut task = A2ATask::new("task-123".to_string());
        task.add_artifact("report".to_string(), "Report content".to_string());

        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(task.artifacts[0].name, "report");
        assert_eq!(task.artifacts[0].content, "Report content");
        assert_eq!(task.artifacts[0].mime_type, "text/plain");
    }

    #[test]
    fn test_a2a_task_full_lifecycle() {
        let mut task = A2ATask::new("task-full".to_string());
        assert_eq!(task.state, TaskState::Submitted);

        task.transition_to(TaskState::Working, None);
        task.add_message(MessageRole::User, "Process this".to_string());
        task.add_message(MessageRole::Agent, "Working on it".to_string());
        task.add_artifact("result".to_string(), "output data".to_string());

        task.transition_to(TaskState::Completed, None);

        assert!(task.state.is_terminal());
        assert_eq!(task.messages.len(), 2);
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(task.state_history.len(), 2); // Submitted→Working, Working→Completed
    }

    // -------------------------------------------------------------------------
    // Endpoint Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_a2a_endpoint_paths() {
        assert_eq!(
            A2AEndpoint::WellKnownAgentCard.path(),
            "/.well-known/agent.json"
        );
        assert_eq!(A2AEndpoint::ListAgents.path(), "/a2a/v1/agents");
        assert_eq!(A2AEndpoint::GetAgent.path(), "/a2a/v1/agents/{agent_id}");
        assert_eq!(A2AEndpoint::CreateTask.path(), "/a2a/v1/tasks");
        assert_eq!(A2AEndpoint::GetTask.path(), "/a2a/v1/tasks/{task_id}");
        assert_eq!(
            A2AEndpoint::CancelTask.path(),
            "/a2a/v1/tasks/{task_id}/cancel"
        );
        assert_eq!(A2AEndpoint::DeleteTask.path(), "/a2a/v1/tasks/{task_id}");
        assert_eq!(
            A2AEndpoint::StreamTask.path(),
            "/a2a/v1/tasks/{task_id}/stream"
        );
        assert_eq!(A2AEndpoint::FireAgent.path(), "/a2a/v1/fire");
    }

    #[test]
    fn test_a2a_endpoint_methods() {
        assert_eq!(A2AEndpoint::WellKnownAgentCard.method(), "GET");
        assert_eq!(A2AEndpoint::ListAgents.method(), "GET");
        assert_eq!(A2AEndpoint::GetAgent.method(), "GET");
        assert_eq!(A2AEndpoint::CreateTask.method(), "POST");
        assert_eq!(A2AEndpoint::CancelTask.method(), "POST");
        assert_eq!(A2AEndpoint::DeleteTask.method(), "DELETE");
        assert_eq!(A2AEndpoint::StreamTask.method(), "GET");
        assert_eq!(A2AEndpoint::FireAgent.method(), "POST");
    }

    #[test]
    fn test_resolved_endpoint_task_id() {
        let ep = A2AEndpoint::GetTask;
        let resolved = ep.resolved_path(None, Some("abc-123"));
        assert_eq!(resolved, "/a2a/v1/tasks/abc-123");
    }

    #[test]
    fn test_resolved_endpoint_agent_id() {
        let ep = A2AEndpoint::GetAgent;
        let resolved = ep.resolved_path(Some("my-agent"), None);
        assert_eq!(resolved, "/a2a/v1/agents/my-agent");
    }

    #[test]
    fn test_resolved_endpoint_no_substitution() {
        let ep = A2AEndpoint::CreateTask;
        let resolved = ep.resolved_path(None, None);
        assert_eq!(resolved, "/a2a/v1/tasks");
    }

    #[test]
    fn test_resolved_endpoint_from_trait() {
        let ep: ResolvedEndpoint = A2AEndpoint::DeleteTask.into();
        assert_eq!(ep.method, "DELETE");
        assert_eq!(ep.path, "/a2a/v1/tasks/{task_id}");
    }

    #[test]
    fn test_all_endpoints_have_unique_method_path_pairs() {
        let endpoints = vec![
            A2AEndpoint::WellKnownAgentCard,
            A2AEndpoint::ListAgents,
            A2AEndpoint::GetAgent,
            A2AEndpoint::CreateTask,
            A2AEndpoint::GetTask,
            A2AEndpoint::CancelTask,
            A2AEndpoint::DeleteTask,
            A2AEndpoint::StreamTask,
            A2AEndpoint::FireAgent,
        ];
        let mut seen = std::collections::HashSet::new();
        for ep in endpoints {
            let key = (ep.method(), ep.path());
            assert!(
                seen.insert(key),
                "Duplicate method+path: {} {}",
                ep.method(),
                ep.path()
            );
        }
    }

    // -------------------------------------------------------------------------
    // TaskMessage / TaskArtifact Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_task_message_roles() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_eq!(MessageRole::Agent, MessageRole::Agent);
        assert_ne!(MessageRole::User, MessageRole::Agent);
    }

    #[test]
    fn test_task_artifact_mime_type_default() {
        let artifact = TaskArtifact {
            id: "a1".to_string(),
            name: "result.json".to_string(),
            content: "{}".to_string(),
            mime_type: "application/json".to_string(),
        };
        assert_eq!(artifact.mime_type, "application/json");
    }

    // -------------------------------------------------------------------------
    // StateTransition Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_state_transition_recorded() {
        let transition = StateTransition {
            from: TaskState::Working,
            to: TaskState::Failed,
            timestamp: Utc::now(),
            reason: Some("Exception during processing".to_string()),
        };
        assert_eq!(transition.from, TaskState::Working);
        assert_eq!(transition.to, TaskState::Failed);
        assert!(transition.reason.is_some());
    }

    // -------------------------------------------------------------------------
    // AuthenticationInfo Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_authentication_info_required() {
        let auth = AuthenticationInfo {
            schemes: vec!["bearer".to_string(), "api_key".to_string()],
            required: true,
        };
        assert!(auth.required);
        assert_eq!(auth.schemes.len(), 2);
    }

    #[test]
    fn test_authentication_info_optional() {
        let auth = AuthenticationInfo {
            schemes: vec!["bearer".to_string()],
            required: false,
        };
        assert!(!auth.required);
    }

    // -------------------------------------------------------------------------
    // AgentSkill Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_agent_skill_location_bound() {
        let skill = AgentSkill {
            id: "geo-task".to_string(),
            name: "Geo Task".to_string(),
            description: "Must run in a specific region".to_string(),
            examples: vec![],
            tags: vec!["geo".to_string()],
            location_bound: true,
        };
        assert!(skill.location_bound);
    }

    // ── New enhanced A2A protocol tests ─────────────────────────────────────────

    #[test]
    fn test_agent_card_full_serialization() {
        let card = AgentCard {
            id: "agent-full".into(),
            name: "Full Agent".into(),
            description: "A fully featured agent".into(),
            protocol_version: "1.0".into(),
            version: "2.0.0".into(),
            url: "https://agent.example.com/a2a/v1".into(),
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            input_modes: vec!["text".into(), "json".into()],
            output_modes: vec!["text".into(), "json".into()],
            skills: vec![AgentSkill {
                id: "skill1".into(),
                name: "Skill One".into(),
                description: "Does thing one".into(),
                examples: vec!["do one".into()],
                tags: vec!["one".into()],
                location_bound: false,
            }],
            authentication: Some(AuthenticationInfo {
                schemes: vec!["bearer".into()],
                required: true,
            }),
            provider: Some(ProviderInfo {
                organization: "Example Corp".into(),
                url: Some("https://example.com".into()),
            }),
        };

        let json = serde_json::to_string(&card).unwrap();
        let decoded: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "agent-full");
        assert_eq!(decoded.version, "2.0.0");
        assert!(decoded.capabilities.streaming);
        assert_eq!(decoded.skills.len(), 1);
        assert!(decoded.authentication.unwrap().required);
        assert_eq!(decoded.provider.unwrap().organization, "Example Corp");
    }

    #[test]
    fn test_agent_card_builder_all_options() {
        let card = AgentCardBuilder::new()
            .id("my-agent")
            .name("My Agent")
            .description("My agent description")
            .url("http://localhost:9000/a2a/v1")
            .version("3.0.0")
            .streaming(true)
            .push_notifications(true)
            .state_transition_history(true)
            .input_modes(vec!["text".into(), "voice".into()])
            .output_modes(vec!["text".into(), "json".into()])
            .auth_required(true)
            .organization("Acme Corp")
            .build();

        assert_eq!(card.id, "my-agent");
        assert_eq!(card.version, "3.0.0");
        assert!(card.capabilities.streaming);
        assert!(card.capabilities.push_notifications);
        assert_eq!(card.input_modes, vec!["text", "voice"]);
        assert_eq!(card.output_modes, vec!["text", "json"]);
        assert!(card.authentication.unwrap().required);
        assert_eq!(card.provider.unwrap().organization, "Acme Corp");
    }

    #[test]
    fn test_agent_card_builder_defaults() {
        let card = AgentCardBuilder::new()
            .id("minimal-agent")
            .name("Minimal Agent")
            .description("Minimal")
            .build();

        assert_eq!(card.id, "minimal-agent");
        assert_eq!(card.protocol_version, "1.0");
        assert_eq!(card.version, "0.1.0");
        assert!(!card.capabilities.streaming);
        assert!(!card.capabilities.push_notifications);
        assert!(card.authentication.is_some());
    }

    #[test]
    fn test_task_message_serde_roundtrip() {
        let msg = TaskMessage {
            id: "msg-123".into(),
            role: MessageRole::Agent,
            content: "Hello world".into(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: TaskMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "msg-123");
        assert_eq!(decoded.role, MessageRole::Agent);
        assert_eq!(decoded.content, "Hello world");
    }

    #[test]
    fn test_task_artifact_custom_mime_type() {
        let artifact = TaskArtifact {
            id: "art-1".into(),
            name: "data.json".into(),
            content: "{\"key\": \"value\"}".into(),
            mime_type: "application/json".into(),
        };
        assert_eq!(artifact.mime_type, "application/json");
        let json = serde_json::to_string(&artifact).unwrap();
        let decoded: TaskArtifact = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mime_type, "application/json");
    }

    #[test]
    fn test_a2a_task_message_roles() {
        let mut task = A2ATask::new("task-roles".into());
        task.add_message(MessageRole::User, "User message".into());
        task.add_message(MessageRole::Agent, "Agent response".into());
        task.add_message(MessageRole::System, "System note".into());

        assert_eq!(task.messages[0].role, MessageRole::User);
        assert_eq!(task.messages[1].role, MessageRole::Agent);
        assert_eq!(task.messages[2].role, MessageRole::System);
    }

    #[test]
    fn test_a2a_task_multiple_artifacts() {
        let mut task = A2ATask::new("task-artifacts".into());
        task.add_artifact("output1".into(), "First output".into());
        task.add_artifact("output2".into(), "Second output".into());

        assert_eq!(task.artifacts.len(), 2);
        assert_eq!(task.artifacts[0].name, "output1");
        assert_eq!(task.artifacts[1].name, "output2");
    }

    #[test]
    fn test_a2a_task_metadata() {
        let mut task = A2ATask::new("task-meta".into());
        task.metadata
            .insert("priority".into(), serde_json::json!("high"));
        task.metadata.insert("retries".into(), serde_json::json!(3));

        assert_eq!(
            task.metadata.get("priority").unwrap(),
            &serde_json::json!("high")
        );
        assert_eq!(task.metadata.get("retries").unwrap(), &serde_json::json!(3));
    }

    #[test]
    fn test_a2a_endpoint_copy_trait() {
        // A2AEndpoint should implement Copy
        fn assert_copy<T: Copy>() {}
        assert_copy::<A2AEndpoint>();
    }

    #[test]
    fn test_resolved_endpoint_tuple_conversion() {
        let (method, path): (&str, &str) = A2AEndpoint::CreateTask.into();
        assert_eq!(method, "POST");
        assert_eq!(path, "/a2a/v1/tasks");
    }

    #[test]
    fn test_provider_info() {
        let provider = ProviderInfo {
            organization: "Test Org".into(),
            url: None,
        };
        assert_eq!(provider.organization, "Test Org");
        assert!(provider.url.is_none());

        let json = serde_json::to_string(&provider).unwrap();
        let decoded: ProviderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.organization, "Test Org");
    }
}
