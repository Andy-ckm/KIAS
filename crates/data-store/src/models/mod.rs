//! # Domain Models
//!
//! Persistent row types for the SQLite data store. Each model maps 1:1 to a
//! database table and implements conversion traits for easy serialization.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Persistent agent row in the `agents` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub node_id: Option<String>,
    pub image: Option<String>,
    pub priority: i32,
    pub cpu: f64,
    pub memory_bytes: i64,
    pub gpu: i32,
    pub labels: String, // JSON
    pub system_prompt_hash: Option<i64>,
    pub metadata: String, // JSON
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl AgentRow {
    /// Create a new agent row with generated UUID.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            status: "pending".to_string(),
            node_id: None,
            image: None,
            priority: 50,
            cpu: 0.0,
            memory_bytes: 0,
            gpu: 0,
            labels: "{}".to_string(),
            system_prompt_hash: None,
            metadata: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        }
    }
}

/// Persistent task row in the `tasks` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskRow {
    pub id: String,
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub name: String,
    pub status: String,
    pub task_type: String,
    pub input: String,  // JSON
    pub output: Option<String>, // JSON
    pub error_message: Option<String>,
    pub priority: i32,
    pub retry_count: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TaskRow {
    /// Create a new task row.
    pub fn new(agent_id: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            workflow_id: None,
            name: name.into(),
            status: "pending".to_string(),
            task_type: "generic".to_string(),
            input: "{}".to_string(),
            output: None,
            error_message: None,
            priority: 50,
            retry_count: 0,
            max_retries: 3,
            timeout_seconds: None,
            started_at: None,
            completed_at: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Persistent workflow row in the `workflows` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub workflow_type: String,
    pub config: String,   // JSON
    pub metadata: String, // JSON
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl WorkflowRow {
    /// Create a new workflow row.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            status: "draft".to_string(),
            workflow_type: "dag".to_string(),
            config: "{}".to_string(),
            metadata: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        }
    }
}

/// Persistent configuration row in the `configs` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfigRow {
    pub id: String,
    pub namespace: String,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub description: String,
    pub is_secret: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl ConfigRow {
    /// Create a new config row.
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            namespace: namespace.into(),
            key: key.into(),
            value: value.into(),
            value_type: "string".to_string(),
            description: String::new(),
            is_secret: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Persistent skill row in the `skills` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub skill_type: String,
    pub config: String, // JSON
    pub tags: String,   // JSON array
    pub enabled: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl SkillRow {
    /// Create a new skill row.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            version: "0.1.0".to_string(),
            skill_type: "builtin".to_string(),
            config: "{}".to_string(),
            tags: "[]".to_string(),
            enabled: 1,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// Persistent component row in the `components` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ComponentRow {
    pub id: String,
    pub name: String,
    pub component_type: String,
    pub version: String,
    pub status: String,
    pub endpoint: Option<String>,
    pub config: String,   // JSON
    pub metadata: String, // JSON
    pub created_at: String,
    pub updated_at: String,
}

impl ComponentRow {
    /// Create a new component row.
    pub fn new(name: impl Into<String>, component_type: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            component_type: component_type.into(),
            version: "0.1.0".to_string(),
            status: "registered".to_string(),
            endpoint: None,
            config: "{}".to_string(),
            metadata: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}


/// Persistent experience replay entry for agent learning.
///
/// Stores state-action-reward-next_state (SARS) transitions used in
/// reinforcement learning-based agent training.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExperienceReplayRow {
    pub id: String,
    pub agent_id: String,
    pub task_id: Option<String>,
    pub state_snapshot: String,  // JSON
    pub action_taken: String,    // JSON
    pub reward: f64,
    pub next_state: Option<String>, // JSON
    pub done: i32,
    pub episode_id: Option<String>,
    pub metadata: String, // JSON
    pub created_at: String,
}

impl ExperienceReplayRow {
    /// Create a new experience replay entry.
    pub fn new(
        agent_id: impl Into<String>,
        state_snapshot: impl Into<String>,
        action_taken: impl Into<String>,
        reward: f64,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            task_id: None,
            state_snapshot: state_snapshot.into(),
            action_taken: action_taken.into(),
            reward,
            next_state: None,
            done: 0,
            episode_id: None,
            metadata: "{}".to_string(),
            created_at: now,
        }
    }
}

/// Persistent prefix cache entry for DeepSeek-style KV prefix caching.
///
/// Caches KV tensors by token prefix hash, enabling reuse across
/// requests sharing a common prefix (e.g., system prompt).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrefixCacheRow {
    pub prefix_hash: String,
    pub model_id: String,
    pub kv_data: Vec<u8>,
    pub token_count: i64,
    pub hit_count: i64,
    pub last_hit_at: Option<String>,
    pub created_at: String,
}

impl PrefixCacheRow {
    /// Create a new prefix cache entry.
    pub fn new(
        prefix_hash: impl Into<String>,
        model_id: impl Into<String>,
        kv_data: Vec<u8>,
        token_count: i64,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            prefix_hash: prefix_hash.into(),
            model_id: model_id.into(),
            kv_data,
            token_count,
            hit_count: 0,
            last_hit_at: None,
            created_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_row_new() {
        let agent = AgentRow::new("test-agent");
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.status, "pending");
        assert_eq!(agent.priority, 50);
        assert!(!agent.id.is_empty());
        assert!(agent.deleted_at.is_none());
    }

    #[test]
    fn test_task_row_new() {
        let task = TaskRow::new("agent-1", "my-task");
        assert_eq!(task.agent_id, "agent-1");
        assert_eq!(task.name, "my-task");
        assert_eq!(task.status, "pending");
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_retries, 3);
    }

    #[test]
    fn test_workflow_row_new() {
        let wf = WorkflowRow::new("pipeline");
        assert_eq!(wf.name, "pipeline");
        assert_eq!(wf.status, "draft");
        assert_eq!(wf.workflow_type, "dag");
    }

    #[test]
    fn test_config_row_new() {
        let cfg = ConfigRow::new("default", "key1", "value1");
        assert_eq!(cfg.namespace, "default");
        assert_eq!(cfg.key, "key1");
        assert_eq!(cfg.value, "value1");
        assert_eq!(cfg.is_secret, 0);
    }

    #[test]
    fn test_skill_row_new() {
        let skill = SkillRow::new("web-search", "Search the web");
        assert_eq!(skill.name, "web-search");
        assert_eq!(skill.description, "Search the web");
        assert_eq!(skill.skill_type, "builtin");
        assert_eq!(skill.enabled, 1);
    }

    #[test]
    fn test_component_row_new() {
        let comp = ComponentRow::new("api-server", "service");
        assert_eq!(comp.name, "api-server");
        assert_eq!(comp.component_type, "service");
        assert_eq!(comp.status, "registered");
    }

    #[test]
    fn test_experience_replay_row_new() {
        let exp = ExperienceReplayRow::new("agent-1", r#"{"obs": 1}"#, r#"{"act": "run"}"#, 0.85);
        assert_eq!(exp.agent_id, "agent-1");
        assert_eq!(exp.reward, 0.85);
        assert_eq!(exp.done, 0);
        assert!(exp.episode_id.is_none());
        assert!(!exp.id.is_empty());
    }

    #[test]
    fn test_prefix_cache_row_new() {
        let cache = PrefixCacheRow::new("abc123", "model-7b", vec![1, 2, 3, 4], 128);
        assert_eq!(cache.prefix_hash, "abc123");
        assert_eq!(cache.model_id, "model-7b");
        assert_eq!(cache.token_count, 128);
        assert_eq!(cache.hit_count, 0);
        assert!(cache.last_hit_at.is_none());
    }
}
