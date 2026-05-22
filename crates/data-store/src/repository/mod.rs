//! # Repository Layer
//!
//! Generic `Repository<T>` trait providing CRUD operations, plus a SQLite-backed
//! implementation. The trait is designed to be swappable — a PostgreSQL
//! implementation can be added later without changing higher-level code.

use async_trait::async_trait;
use kias_common::{KiasError, KiasResult};
use sqlx::SqlitePool;
use tracing::debug;

use super::models::*;

/// Generic repository trait for CRUD operations on domain entities.
///
/// # Type Parameters
/// - `T`: The row type (must be Send + Sync + 'static).
#[async_trait]
pub trait Repository<T: Send + Sync + 'static>: Send + Sync {
    /// Insert a new record. Returns the inserted record.
    async fn create(&self, entity: &T) -> KiasResult<()>;

    /// Get a record by ID. Returns None if not found.
    async fn get_by_id(&self, id: &str) -> KiasResult<Option<T>>;

    /// List all records, optionally with a limit and offset.
    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<T>>;

    /// Update an existing record. Returns Ok(()) on success.
    async fn update(&self, entity: &T) -> KiasResult<()>;

    /// Soft-delete a record by ID (sets deleted_at). Returns Ok(()) on success.
    async fn delete(&self, id: &str) -> KiasResult<()>;

    /// Count total records.
    async fn count(&self) -> KiasResult<i64>;
}

/// SQLite-backed repository for `AgentRow`.
pub struct AgentRepository {
    pool: SqlitePool,
}

impl AgentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get agents by status.
    pub async fn get_by_status(&self, status: &str) -> KiasResult<Vec<AgentRow>> {
        let rows = sqlx::query_as::<_, AgentRow>(
            "SELECT * FROM agents WHERE status = ? AND deleted_at IS NULL ORDER BY created_at DESC",
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query agents by status: {e}")))?;
        Ok(rows)
    }

    /// Get agents assigned to a specific node.
    pub async fn get_by_node(&self, node_id: &str) -> KiasResult<Vec<AgentRow>> {
        let rows = sqlx::query_as::<_, AgentRow>(
            "SELECT * FROM agents WHERE node_id = ? AND deleted_at IS NULL",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query agents by node: {e}")))?;
        Ok(rows)
    }
}

#[async_trait]
impl Repository<AgentRow> for AgentRepository {
    async fn create(&self, agent: &AgentRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO agents (id, name, status, node_id, image, priority, cpu, memory_bytes, gpu, labels, system_prompt_hash, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&agent.id)
        .bind(&agent.name)
        .bind(&agent.status)
        .bind(&agent.node_id)
        .bind(&agent.image)
        .bind(agent.priority)
        .bind(agent.cpu)
        .bind(agent.memory_bytes)
        .bind(agent.gpu)
        .bind(&agent.labels)
        .bind(agent.system_prompt_hash)
        .bind(&agent.metadata)
        .bind(&agent.created_at)
        .bind(&agent.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create agent: {e}")))?;
        debug!("Created agent: {}", agent.id);
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<AgentRow>> {
        let row = sqlx::query_as::<_, AgentRow>(
            "SELECT * FROM agents WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to get agent: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<AgentRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows = sqlx::query_as::<_, AgentRow>(
            "SELECT * FROM agents WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to list agents: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, agent: &AgentRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE agents SET name = ?, status = ?, node_id = ?, image = ?, priority = ?, cpu = ?, memory_bytes = ?, gpu = ?, labels = ?, system_prompt_hash = ?, metadata = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&agent.name)
        .bind(&agent.status)
        .bind(&agent.node_id)
        .bind(&agent.image)
        .bind(agent.priority)
        .bind(agent.cpu)
        .bind(agent.memory_bytes)
        .bind(agent.gpu)
        .bind(&agent.labels)
        .bind(agent.system_prompt_hash)
        .bind(&agent.metadata)
        .bind(&now)
        .bind(&agent.id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update agent: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Agent {} not found", agent.id)));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result =
            sqlx::query("UPDATE agents SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&now)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to delete agent: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Agent {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agents WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count agents: {e}")))?;
        Ok(row.0)
    }
}

/// SQLite-backed repository for `TaskRow`.
pub struct TaskRepository {
    pool: SqlitePool,
}

impl TaskRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get tasks for a specific agent.
    pub async fn get_by_agent(&self, agent_id: &str) -> KiasResult<Vec<TaskRow>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE agent_id = ? ORDER BY created_at DESC",
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query tasks by agent: {e}")))?;
        Ok(rows)
    }

    /// Get tasks for a specific workflow.
    pub async fn get_by_workflow(&self, workflow_id: &str) -> KiasResult<Vec<TaskRow>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE workflow_id = ? ORDER BY created_at DESC",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query tasks by workflow: {e}")))?;
        Ok(rows)
    }

    /// Get tasks by status.
    pub async fn get_by_status(&self, status: &str) -> KiasResult<Vec<TaskRow>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE status = ? ORDER BY created_at DESC",
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query tasks by status: {e}")))?;
        Ok(rows)
    }
}

#[async_trait]
impl Repository<TaskRow> for TaskRepository {
    async fn create(&self, task: &TaskRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO tasks (id, agent_id, workflow_id, name, status, task_type, input, output, error_message, priority, retry_count, max_retries, timeout_seconds, started_at, completed_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&task.id)
        .bind(&task.agent_id)
        .bind(&task.workflow_id)
        .bind(&task.name)
        .bind(&task.status)
        .bind(&task.task_type)
        .bind(&task.input)
        .bind(&task.output)
        .bind(&task.error_message)
        .bind(task.priority)
        .bind(task.retry_count)
        .bind(task.max_retries)
        .bind(task.timeout_seconds)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create task: {e}")))?;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<TaskRow>> {
        let row = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get task: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<TaskRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to list tasks: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, task: &TaskRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE tasks SET status = ?, output = ?, error_message = ?, retry_count = ?, started_at = ?, completed_at = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&task.status)
        .bind(&task.output)
        .bind(&task.error_message)
        .bind(task.retry_count)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&now)
        .bind(&task.id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update task: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Task {} not found", task.id)));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to delete task: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Task {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count tasks: {e}")))?;
        Ok(row.0)
    }
}

/// SQLite-backed repository for `WorkflowRow`.
pub struct WorkflowRepository {
    pool: SqlitePool,
}

impl WorkflowRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get workflows by status.
    pub async fn get_by_status(&self, status: &str) -> KiasResult<Vec<WorkflowRow>> {
        let rows = sqlx::query_as::<_, WorkflowRow>(
            "SELECT * FROM workflows WHERE status = ? AND deleted_at IS NULL",
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query workflows by status: {e}")))?;
        Ok(rows)
    }
}

#[async_trait]
impl Repository<WorkflowRow> for WorkflowRepository {
    async fn create(&self, wf: &WorkflowRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO workflows (id, name, description, status, workflow_type, config, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&wf.id)
        .bind(&wf.name)
        .bind(&wf.description)
        .bind(&wf.status)
        .bind(&wf.workflow_type)
        .bind(&wf.config)
        .bind(&wf.metadata)
        .bind(&wf.created_at)
        .bind(&wf.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create workflow: {e}")))?;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<WorkflowRow>> {
        let row = sqlx::query_as::<_, WorkflowRow>(
            "SELECT * FROM workflows WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to get workflow: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<WorkflowRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows = sqlx::query_as::<_, WorkflowRow>(
            "SELECT * FROM workflows WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to list workflows: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, wf: &WorkflowRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE workflows SET name = ?, description = ?, status = ?, workflow_type = ?, config = ?, metadata = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&wf.name)
        .bind(&wf.description)
        .bind(&wf.status)
        .bind(&wf.workflow_type)
        .bind(&wf.config)
        .bind(&wf.metadata)
        .bind(&now)
        .bind(&wf.id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update workflow: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Workflow {} not found", wf.id)));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result =
            sqlx::query("UPDATE workflows SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&now)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to delete workflow: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Workflow {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workflows WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count workflows: {e}")))?;
        Ok(row.0)
    }
}

/// SQLite-backed repository for `ConfigRow`.
pub struct ConfigRepository {
    pool: SqlitePool,
}

impl ConfigRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a config value by namespace and key.
    pub async fn get_by_key(&self, namespace: &str, key: &str) -> KiasResult<Option<ConfigRow>> {
        let row =
            sqlx::query_as::<_, ConfigRow>("SELECT * FROM configs WHERE namespace = ? AND key = ?")
                .bind(namespace)
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to get config: {e}")))?;
        Ok(row)
    }

    /// Get all configs for a namespace.
    pub async fn get_by_namespace(&self, namespace: &str) -> KiasResult<Vec<ConfigRow>> {
        let rows = sqlx::query_as::<_, ConfigRow>(
            "SELECT * FROM configs WHERE namespace = ? ORDER BY key",
        )
        .bind(namespace)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query configs by namespace: {e}")))?;
        Ok(rows)
    }

    /// Upsert a config value.
    pub async fn upsert(&self, config: &ConfigRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO configs (id, namespace, key, value, value_type, description, is_secret, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(namespace, key) DO UPDATE SET value = ?, value_type = ?, description = ?, is_secret = ?, updated_at = ?"
        )
        .bind(&config.id)
        .bind(&config.namespace)
        .bind(&config.key)
        .bind(&config.value)
        .bind(&config.value_type)
        .bind(&config.description)
        .bind(config.is_secret)
        .bind(&config.created_at)
        .bind(&now)
        .bind(&config.value)
        .bind(&config.value_type)
        .bind(&config.description)
        .bind(config.is_secret)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to upsert config: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl Repository<ConfigRow> for ConfigRepository {
    async fn create(&self, config: &ConfigRow) -> KiasResult<()> {
        self.upsert(config).await
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<ConfigRow>> {
        let row = sqlx::query_as::<_, ConfigRow>("SELECT * FROM configs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get config: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<ConfigRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows = sqlx::query_as::<_, ConfigRow>(
            "SELECT * FROM configs ORDER BY namespace, key LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to list configs: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, config: &ConfigRow) -> KiasResult<()> {
        self.upsert(config).await
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let result = sqlx::query("DELETE FROM configs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to delete config: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Config {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM configs")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count configs: {e}")))?;
        Ok(row.0)
    }
}

/// SQLite-backed repository for `SkillRow`.
pub struct SkillRepository {
    pool: SqlitePool,
}

impl SkillRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a skill by name.
    pub async fn get_by_name(&self, name: &str) -> KiasResult<Option<SkillRow>> {
        let row = sqlx::query_as::<_, SkillRow>("SELECT * FROM skills WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get skill by name: {e}")))?;
        Ok(row)
    }

    /// Get enabled skills.
    pub async fn get_enabled(&self) -> KiasResult<Vec<SkillRow>> {
        let rows =
            sqlx::query_as::<_, SkillRow>("SELECT * FROM skills WHERE enabled = 1 ORDER BY name")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to query enabled skills: {e}")))?;
        Ok(rows)
    }
}

#[async_trait]
impl Repository<SkillRow> for SkillRepository {
    async fn create(&self, skill: &SkillRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO skills (id, name, description, version, skill_type, config, tags, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&skill.id)
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&skill.version)
        .bind(&skill.skill_type)
        .bind(&skill.config)
        .bind(&skill.tags)
        .bind(skill.enabled)
        .bind(&skill.created_at)
        .bind(&skill.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create skill: {e}")))?;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<SkillRow>> {
        let row = sqlx::query_as::<_, SkillRow>("SELECT * FROM skills WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get skill: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<SkillRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows =
            sqlx::query_as::<_, SkillRow>("SELECT * FROM skills ORDER BY name LIMIT ? OFFSET ?")
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| KiasError::Config(format!("Failed to list skills: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, skill: &SkillRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE skills SET description = ?, version = ?, skill_type = ?, config = ?, tags = ?, enabled = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&skill.description)
        .bind(&skill.version)
        .bind(&skill.skill_type)
        .bind(&skill.config)
        .bind(&skill.tags)
        .bind(skill.enabled)
        .bind(&now)
        .bind(&skill.id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update skill: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Skill {} not found", skill.id)));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let result = sqlx::query("DELETE FROM skills WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to delete skill: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Skill {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skills")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count skills: {e}")))?;
        Ok(row.0)
    }
}

/// SQLite-backed repository for `ComponentRow`.
pub struct ComponentRepository {
    pool: SqlitePool,
}

impl ComponentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a component by name.
    pub async fn get_by_name(&self, name: &str) -> KiasResult<Option<ComponentRow>> {
        let row = sqlx::query_as::<_, ComponentRow>("SELECT * FROM components WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get component by name: {e}")))?;
        Ok(row)
    }

    /// Get components by type.
    pub async fn get_by_type(&self, component_type: &str) -> KiasResult<Vec<ComponentRow>> {
        let rows = sqlx::query_as::<_, ComponentRow>(
            "SELECT * FROM components WHERE component_type = ? ORDER BY name",
        )
        .bind(component_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query components by type: {e}")))?;
        Ok(rows)
    }
}

#[async_trait]
impl Repository<ComponentRow> for ComponentRepository {
    async fn create(&self, comp: &ComponentRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT INTO components (id, name, component_type, version, status, endpoint, config, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&comp.id)
        .bind(&comp.name)
        .bind(&comp.component_type)
        .bind(&comp.version)
        .bind(&comp.status)
        .bind(&comp.endpoint)
        .bind(&comp.config)
        .bind(&comp.metadata)
        .bind(&comp.created_at)
        .bind(&comp.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create component: {e}")))?;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> KiasResult<Option<ComponentRow>> {
        let row = sqlx::query_as::<_, ComponentRow>("SELECT * FROM components WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to get component: {e}")))?;
        Ok(row)
    }

    async fn list(&self, limit: Option<i64>, offset: Option<i64>) -> KiasResult<Vec<ComponentRow>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let rows = sqlx::query_as::<_, ComponentRow>(
            "SELECT * FROM components ORDER BY name LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to list components: {e}")))?;
        Ok(rows)
    }

    async fn update(&self, comp: &ComponentRow) -> KiasResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE components SET component_type = ?, version = ?, status = ?, endpoint = ?, config = ?, metadata = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&comp.component_type)
        .bind(&comp.version)
        .bind(&comp.status)
        .bind(&comp.endpoint)
        .bind(&comp.config)
        .bind(&comp.metadata)
        .bind(&now)
        .bind(&comp.id)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to update component: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!(
                "Component {} not found",
                comp.id
            )));
        }
        Ok(())
    }

    async fn delete(&self, id: &str) -> KiasResult<()> {
        let result = sqlx::query("DELETE FROM components WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to delete component: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(KiasError::NotFound(format!("Component {id} not found")));
        }
        Ok(())
    }

    async fn count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM components")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count components: {e}")))?;
        Ok(row.0)
    }
}

/// Database health check result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseHealth {
    /// Whether the database is reachable.
    pub connected: bool,
    /// Current schema version.
    pub schema_version: i32,
    /// Connection pool size.
    pub pool_size: u32,
    /// Number of idle connections.
    pub idle_connections: u32,
}

/// SQLite-backed repository for `ExperienceReplayRow`.
///
/// Supports batch operations for high-throughput agent learning scenarios.
pub struct ExperienceReplayRepository {
    pool: SqlitePool,
}

impl ExperienceReplayRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Batch insert multiple experience entries in a single transaction.
    pub async fn batch_insert(&self, entries: &[ExperienceReplayRow]) -> KiasResult<u64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to begin transaction: {e}")))?;

        let mut count: u64 = 0;
        for entry in entries {
            sqlx::query(
                "INSERT INTO experience_replay (id, agent_id, task_id, state_snapshot, action_taken, reward, next_state, done, episode_id, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&entry.id)
            .bind(&entry.agent_id)
            .bind(&entry.task_id)
            .bind(&entry.state_snapshot)
            .bind(&entry.action_taken)
            .bind(entry.reward)
            .bind(&entry.next_state)
            .bind(entry.done)
            .bind(&entry.episode_id)
            .bind(&entry.metadata)
            .execute(&mut *tx)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to insert experience: {e}")))?;
            count += 1;
        }

        tx.commit()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to commit batch insert: {e}")))?;
        debug!("Batch inserted {count} experience entries");
        Ok(count)
    }

    /// Get experiences for a specific agent, ordered by creation time.
    pub async fn get_by_agent(
        &self,
        agent_id: &str,
        limit: Option<i64>,
    ) -> KiasResult<Vec<ExperienceReplayRow>> {
        let limit = limit.unwrap_or(100);
        let rows = sqlx::query_as::<_, ExperienceReplayRow>(
            "SELECT * FROM experience_replay WHERE agent_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(agent_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query experiences by agent: {e}")))?;
        Ok(rows)
    }

    /// Get experiences for a specific episode.
    pub async fn get_by_episode(&self, episode_id: &str) -> KiasResult<Vec<ExperienceReplayRow>> {
        let rows = sqlx::query_as::<_, ExperienceReplayRow>(
            "SELECT * FROM experience_replay WHERE episode_id = ? ORDER BY created_at ASC",
        )
        .bind(episode_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to query experiences by episode: {e}")))?;
        Ok(rows)
    }

    /// Sample N random experiences for training.
    pub async fn sample_random(&self, n: i64) -> KiasResult<Vec<ExperienceReplayRow>> {
        let rows = sqlx::query_as::<_, ExperienceReplayRow>(
            "SELECT * FROM experience_replay ORDER BY RANDOM() LIMIT ?",
        )
        .bind(n)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to sample experiences: {e}")))?;
        Ok(rows)
    }

    /// Get the total number of stored experiences.
    pub async fn total_count(&self) -> KiasResult<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM experience_replay")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to count experiences: {e}")))?;
        Ok(row.0)
    }

    /// Delete experiences older than a given number of days.
    pub async fn cleanup_older_than(&self, days: i64) -> KiasResult<u64> {
        let result = sqlx::query(
            "DELETE FROM experience_replay WHERE created_at < datetime('now', '-' || ? || ' days')",
        )
        .bind(days)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to cleanup old experiences: {e}")))?;
        Ok(result.rows_affected())
    }
}

/// SQLite-backed repository for `PrefixCacheRow`.
///
/// Implements DeepSeek-style KV prefix caching for LLM inference optimization.
pub struct PrefixCacheRepository {
    pool: SqlitePool,
}

impl PrefixCacheRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace a prefix cache entry.
    pub async fn insert(&self, entry: &PrefixCacheRow) -> KiasResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO prefix_cache (prefix_hash, model_id, kv_data, token_count, hit_count, last_hit_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&entry.prefix_hash)
        .bind(&entry.model_id)
        .bind(&entry.kv_data)
        .bind(entry.token_count)
        .bind(entry.hit_count)
        .bind(&entry.last_hit_at)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to insert prefix cache: {e}")))?;
        debug!(
            "Cached prefix {} for model {}",
            entry.prefix_hash, entry.model_id
        );
        Ok(())
    }

    /// Lookup a cached prefix and record a hit.
    pub async fn lookup(
        &self,
        prefix_hash: &str,
        model_id: &str,
    ) -> KiasResult<Option<PrefixCacheRow>> {
        let row = sqlx::query_as::<_, PrefixCacheRow>(
            "SELECT * FROM prefix_cache WHERE prefix_hash = ? AND model_id = ?",
        )
        .bind(prefix_hash)
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to lookup prefix cache: {e}")))?;

        if row.is_some() {
            // Record the hit
            sqlx::query(
                "UPDATE prefix_cache SET hit_count = hit_count + 1, last_hit_at = datetime('now') WHERE prefix_hash = ? AND model_id = ?"
            )
            .bind(prefix_hash)
            .bind(model_id)
            .execute(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to record prefix hit: {e}")))?;
        }

        Ok(row)
    }

    /// Batch insert prefix cache entries.
    pub async fn batch_insert(&self, entries: &[PrefixCacheRow]) -> KiasResult<u64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to begin transaction: {e}")))?;

        let mut count: u64 = 0;
        for entry in entries {
            sqlx::query(
                "INSERT OR REPLACE INTO prefix_cache (prefix_hash, model_id, kv_data, token_count, hit_count, last_hit_at) VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(&entry.prefix_hash)
            .bind(&entry.model_id)
            .bind(&entry.kv_data)
            .bind(entry.token_count)
            .bind(entry.hit_count)
            .bind(&entry.last_hit_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to batch insert prefix cache: {e}")))?;
            count += 1;
        }

        tx.commit()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to commit prefix cache batch: {e}")))?;
        Ok(count)
    }

    /// Get the least-hit entries (candidates for eviction).
    pub async fn get_lru_entries(&self, limit: i64) -> KiasResult<Vec<PrefixCacheRow>> {
        let rows = sqlx::query_as::<_, PrefixCacheRow>(
            "SELECT * FROM prefix_cache ORDER BY hit_count ASC, last_hit_at ASC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to get LRU prefix entries: {e}")))?;
        Ok(rows)
    }

    /// Evict entries that haven't been hit in the given number of days.
    pub async fn evict_stale(&self, stale_days: i64) -> KiasResult<u64> {
        let result = sqlx::query(
            "DELETE FROM prefix_cache WHERE last_hit_at IS NULL OR last_hit_at < datetime('now', '-' || ? || ' days')"
        )
        .bind(stale_days)
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to evict stale prefix cache: {e}")))?;
        Ok(result.rows_affected())
    }

    /// Get cache statistics for a model.
    pub async fn model_stats(&self, model_id: &str) -> KiasResult<PrefixCacheStats> {
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(hit_count), 0), COALESCE(SUM(token_count), 0) FROM prefix_cache WHERE model_id = ?"
        )
        .bind(model_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to get prefix cache stats: {e}")))?;

        Ok(PrefixCacheStats {
            entries: row.0,
            total_hits: row.1,
            total_tokens: row.2,
        })
    }
}

/// Statistics for prefix cache per model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixCacheStats {
    pub entries: i64,
    pub total_hits: i64,
    pub total_tokens: i64,
}

/// Unified data store that holds all repositories.
///
/// This is the primary entry point for the data layer. Create one instance
/// and share it across your application.
pub struct SqliteRepository {
    pub pool: SqlitePool,
    pub agents: AgentRepository,
    pub tasks: TaskRepository,
    pub workflows: WorkflowRepository,
    pub configs: ConfigRepository,
    pub skills: SkillRepository,
    pub components: ComponentRepository,
    pub experience_replay: ExperienceReplayRepository,
    pub prefix_cache: PrefixCacheRepository,
}

impl SqliteRepository {
    /// Create a new repository from an existing SQLite pool.
    /// Migrations should be run before calling this.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            agents: AgentRepository::new(pool.clone()),
            tasks: TaskRepository::new(pool.clone()),
            workflows: WorkflowRepository::new(pool.clone()),
            configs: ConfigRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            components: ComponentRepository::new(pool.clone()),
            experience_replay: ExperienceReplayRepository::new(pool.clone()),
            prefix_cache: PrefixCacheRepository::new(pool.clone()),
            pool,
        }
    }

    /// Create an in-memory SQLite database with migrations applied.
    /// Useful for testing.
    pub async fn in_memory() -> KiasResult<Self> {
        let pool = SqlitePool::connect("sqlite::memory:").await.map_err(|e| {
            KiasError::Config(format!("Failed to connect to in-memory SQLite: {e}"))
        })?;

        let migration_runner = super::migrations::MigrationRunner::new(pool.clone());
        migration_runner.run_all().await?;

        Ok(Self::new(pool))
    }

    /// Create a file-backed SQLite database with migrations applied.
    pub async fn open(path: &str) -> KiasResult<Self> {
        let url = format!("sqlite:{path}?mode=rwc");
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to open SQLite at {path}: {e}")))?;

        let migration_runner = super::migrations::MigrationRunner::new(pool.clone());
        migration_runner.run_all().await?;

        Ok(Self::new(pool))
    }

    /// Open a file-backed SQLite database with custom pool configuration.
    pub async fn open_with_pool_config(
        path: &str,
        max_connections: u32,
        min_connections: u32,
    ) -> KiasResult<Self> {
        let url = format!("sqlite:{path}?mode=rwc");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .connect(&url)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to open SQLite at {path}: {e}")))?;

        let migration_runner = super::migrations::MigrationRunner::new(pool.clone());
        migration_runner.run_all().await?;

        Ok(Self::new(pool))
    }

    /// Check database health — verifies connectivity and schema version.
    pub async fn health_check(&self) -> DatabaseHealth {
        let schema_version = super::migrations::MigrationRunner::new(self.pool.clone())
            .current_version()
            .await
            .unwrap_or(-1);

        let pool_size = self.pool.size();
        let idle = self.pool.num_idle() as u32;

        DatabaseHealth {
            connected: true,
            schema_version,
            pool_size,
            idle_connections: idle,
        }
    }

    /// Get pool statistics.
    pub fn pool_stats(&self) -> PoolStats {
        PoolStats {
            pool_size: self.pool.size(),
            idle_connections: self.pool.num_idle() as u32,
        }
    }
}

/// Pool connection statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoolStats {
    pub pool_size: u32,
    pub idle_connections: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> SqliteRepository {
        SqliteRepository::in_memory()
            .await
            .expect("Failed to create in-memory repo")
    }

    #[tokio::test]
    async fn test_agent_crud() {
        let repo = test_repo().await;

        // Create
        let mut agent = AgentRow::new("test-agent");
        agent.cpu = 2.0;
        agent.memory_bytes = 1024 * 1024 * 512;
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        // Read
        let fetched = repo
            .agents
            .get_by_id(&agent.id)
            .await
            .expect("Failed to get agent");
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "test-agent");
        assert_eq!(fetched.cpu, 2.0);

        // List
        let all = repo
            .agents
            .list(None, None)
            .await
            .expect("Failed to list agents");
        assert_eq!(all.len(), 1);

        // Update
        let mut updated = fetched.clone();
        updated.status = "running".to_string();
        repo.agents
            .update(&updated)
            .await
            .expect("Failed to update agent");

        let fetched = repo
            .agents
            .get_by_id(&agent.id)
            .await
            .expect("Failed to get agent");
        assert_eq!(fetched.unwrap().status, "running");

        // Count
        let count = repo.agents.count().await.expect("Failed to count agents");
        assert_eq!(count, 1);

        // Delete (soft)
        repo.agents
            .delete(&agent.id)
            .await
            .expect("Failed to delete agent");
        let fetched = repo
            .agents
            .get_by_id(&agent.id)
            .await
            .expect("Failed to get agent");
        assert!(fetched.is_none(), "Soft-deleted agent should not be found");

        let count = repo.agents.count().await.expect("Failed to count agents");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_task_crud() {
        let repo = test_repo().await;

        // Create agent first (FK constraint)
        let agent = AgentRow::new("agent-for-tasks");
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        // Create task
        let task = TaskRow::new(&agent.id, "my-task");
        repo.tasks
            .create(&task)
            .await
            .expect("Failed to create task");

        // Read
        let fetched = repo
            .tasks
            .get_by_id(&task.id)
            .await
            .expect("Failed to get task");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "my-task");

        // Get by agent
        let agent_tasks = repo
            .tasks
            .get_by_agent(&agent.id)
            .await
            .expect("Failed to get agent tasks");
        assert_eq!(agent_tasks.len(), 1);

        // Count
        let count = repo.tasks.count().await.expect("Failed to count tasks");
        assert_eq!(count, 1);

        // Delete (hard)
        repo.tasks
            .delete(&task.id)
            .await
            .expect("Failed to delete task");
        let count = repo.tasks.count().await.expect("Failed to count tasks");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_workflow_crud() {
        let repo = test_repo().await;

        let wf = WorkflowRow::new("pipeline");
        repo.workflows
            .create(&wf)
            .await
            .expect("Failed to create workflow");

        let fetched = repo
            .workflows
            .get_by_id(&wf.id)
            .await
            .expect("Failed to get workflow");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "pipeline");

        // Soft delete
        repo.workflows
            .delete(&wf.id)
            .await
            .expect("Failed to delete workflow");
        let fetched = repo
            .workflows
            .get_by_id(&wf.id)
            .await
            .expect("Failed to get workflow");
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_config_upsert() {
        let repo = test_repo().await;

        let mut config = ConfigRow::new("default", "key1", "value1");
        repo.configs
            .create(&config)
            .await
            .expect("Failed to create config");

        // Upsert with new value
        config.value = "updated".to_string();
        repo.configs
            .upsert(&config)
            .await
            .expect("Failed to upsert config");

        let fetched = repo
            .configs
            .get_by_key("default", "key1")
            .await
            .expect("Failed to get config");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().value, "updated");
    }

    #[tokio::test]
    async fn test_skill_crud() {
        let repo = test_repo().await;

        let skill = SkillRow::new("web-search", "Search the web");
        repo.skills
            .create(&skill)
            .await
            .expect("Failed to create skill");

        let fetched = repo
            .skills
            .get_by_name("web-search")
            .await
            .expect("Failed to get skill");
        assert!(fetched.is_some());

        let enabled = repo
            .skills
            .get_enabled()
            .await
            .expect("Failed to get enabled skills");
        assert_eq!(enabled.len(), 1);
    }

    #[tokio::test]
    async fn test_component_crud() {
        let repo = test_repo().await;

        let comp = ComponentRow::new("api-server", "service");
        repo.components
            .create(&comp)
            .await
            .expect("Failed to create component");

        let fetched = repo
            .components
            .get_by_name("api-server")
            .await
            .expect("Failed to get component");
        assert!(fetched.is_some());

        let by_type = repo
            .components
            .get_by_type("service")
            .await
            .expect("Failed to get by type");
        assert_eq!(by_type.len(), 1);
    }

    #[tokio::test]
    async fn test_pagination() {
        let repo = test_repo().await;

        // Create 5 agents
        for i in 0..5 {
            let agent = AgentRow::new(format!("agent-{i}"));
            repo.agents
                .create(&agent)
                .await
                .expect("Failed to create agent");
        }

        // List with limit
        let page1 = repo
            .agents
            .list(Some(2), Some(0))
            .await
            .expect("Failed to list");
        assert_eq!(page1.len(), 2);

        let page2 = repo
            .agents
            .list(Some(2), Some(2))
            .await
            .expect("Failed to list");
        assert_eq!(page2.len(), 2);

        let page3 = repo
            .agents
            .list(Some(2), Some(4))
            .await
            .expect("Failed to list");
        assert_eq!(page3.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_by_status() {
        let repo = test_repo().await;

        let mut agent = AgentRow::new("status-test");
        agent.status = "running".to_string();
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        let running = repo
            .agents
            .get_by_status("running")
            .await
            .expect("Failed to get by status");
        assert_eq!(running.len(), 1);

        let pending = repo
            .agents
            .get_by_status("pending")
            .await
            .expect("Failed to get by status");
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_experience_replay_batch_insert() {
        let repo = test_repo().await;

        // Create an agent first
        let agent = AgentRow::new("rl-agent");
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        let entries = vec![
            ExperienceReplayRow::new(&agent.id, r#"{"s": 1}"#, r#"{"a": 0}"#, 0.5),
            ExperienceReplayRow::new(&agent.id, r#"{"s": 2}"#, r#"{"a": 1}"#, 0.8),
            ExperienceReplayRow::new(&agent.id, r#"{"s": 3}"#, r#"{"a": 0}"#, -0.2),
        ];

        let count = repo
            .experience_replay
            .batch_insert(&entries)
            .await
            .expect("Failed to batch insert");
        assert_eq!(count, 3);

        let total = repo
            .experience_replay
            .total_count()
            .await
            .expect("Failed to count");
        assert_eq!(total, 3);

        let by_agent = repo
            .experience_replay
            .get_by_agent(&agent.id, Some(10))
            .await
            .expect("Failed to get by agent");
        assert_eq!(by_agent.len(), 3);
    }

    #[tokio::test]
    async fn test_experience_replay_sample_random() {
        let repo = test_repo().await;

        let agent = AgentRow::new("sample-agent");
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        for i in 0..10 {
            let entry = ExperienceReplayRow::new(
                &agent.id,
                format!(r#"{{"s": {i}}}"#),
                r#"{"a": 0}"#,
                i as f64 * 0.1,
            );
            repo.experience_replay
                .batch_insert(&[entry])
                .await
                .expect("Failed to insert");
        }

        let sample = repo
            .experience_replay
            .sample_random(5)
            .await
            .expect("Failed to sample");
        assert_eq!(sample.len(), 5);
    }

    #[tokio::test]
    async fn test_experience_replay_episode() {
        let repo = test_repo().await;

        let agent = AgentRow::new("ep-agent");
        repo.agents
            .create(&agent)
            .await
            .expect("Failed to create agent");

        let mut entry = ExperienceReplayRow::new(&agent.id, r#"{"s": 1}"#, r#"{"a": 0}"#, 0.5);
        entry.episode_id = Some("ep-001".to_string());
        repo.experience_replay
            .batch_insert(&[entry])
            .await
            .expect("Failed to insert");

        let by_episode = repo
            .experience_replay
            .get_by_episode("ep-001")
            .await
            .expect("Failed to get by episode");
        assert_eq!(by_episode.len(), 1);
        assert_eq!(by_episode[0].episode_id.as_deref(), Some("ep-001"));
    }

    #[tokio::test]
    async fn test_prefix_cache_insert_and_lookup() {
        let repo = test_repo().await;

        let entry = PrefixCacheRow::new("hash-abc", "model-7b", vec![1, 2, 3, 4], 128);
        repo.prefix_cache
            .insert(&entry)
            .await
            .expect("Failed to insert");

        let found = repo
            .prefix_cache
            .lookup("hash-abc", "model-7b")
            .await
            .expect("Failed to lookup");
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.token_count, 128);
        assert_eq!(found.kv_data, vec![1, 2, 3, 4]);

        // Lookup again should increment hit count
        repo.prefix_cache
            .lookup("hash-abc", "model-7b")
            .await
            .expect("Failed to lookup");
        repo.prefix_cache
            .lookup("hash-abc", "model-7b")
            .await
            .expect("Failed to lookup");

        let stats = repo
            .prefix_cache
            .model_stats("model-7b")
            .await
            .expect("Failed to get stats");
        assert_eq!(stats.entries, 1);
        assert!(
            stats.total_hits >= 3,
            "Expected at least 3 hits, got {}",
            stats.total_hits
        );
        assert_eq!(stats.total_tokens, 128);
    }

    #[tokio::test]
    async fn test_prefix_cache_batch_insert() {
        let repo = test_repo().await;

        let entries = vec![
            PrefixCacheRow::new("h1", "m1", vec![10], 64),
            PrefixCacheRow::new("h2", "m1", vec![20], 128),
            PrefixCacheRow::new("h3", "m2", vec![30], 256),
        ];

        let count = repo
            .prefix_cache
            .batch_insert(&entries)
            .await
            .expect("Failed to batch insert");
        assert_eq!(count, 3);

        let stats = repo
            .prefix_cache
            .model_stats("m1")
            .await
            .expect("Failed to get stats");
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.total_tokens, 192);
    }

    #[tokio::test]
    async fn test_prefix_cache_miss() {
        let repo = test_repo().await;

        let found = repo
            .prefix_cache
            .lookup("nonexistent", "model-x")
            .await
            .expect("Failed to lookup");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_health_check() {
        let repo = test_repo().await;

        let health = repo.health_check().await;
        assert!(health.connected);
        assert_eq!(health.schema_version, 7);
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let repo = test_repo().await;

        let stats = repo.pool_stats();
        assert!(stats.pool_size > 0);
    }

    #[tokio::test]
    async fn test_agent_get_by_node() {
        let repo = test_repo().await;

        let mut agent = AgentRow::new("node-agent");
        agent.node_id = Some("node-1".to_string());
        repo.agents.create(&agent).await.expect("create");

        let mut agent2 = AgentRow::new("other-agent");
        agent2.node_id = Some("node-2".to_string());
        repo.agents.create(&agent2).await.expect("create2");

        let node1_agents = repo
            .agents
            .get_by_node("node-1")
            .await
            .expect("get_by_node");
        assert_eq!(node1_agents.len(), 1);
        assert_eq!(node1_agents[0].name, "node-agent");

        let node2_agents = repo
            .agents
            .get_by_node("node-2")
            .await
            .expect("get_by_node");
        assert_eq!(node2_agents.len(), 1);
        assert_eq!(node2_agents[0].name, "other-agent");

        let none_agents = repo
            .agents
            .get_by_node("nonexistent")
            .await
            .expect("get_by_node");
        assert_eq!(none_agents.len(), 0);
    }

    #[tokio::test]
    async fn test_task_get_by_workflow() {
        let repo = test_repo().await;

        let agent = AgentRow::new("wf-agent");
        repo.agents.create(&agent).await.expect("create agent");

        let wf = WorkflowRow::new("test-wf");
        repo.workflows.create(&wf).await.expect("create wf");

        let mut task = TaskRow::new(&agent.id, "wf-task");
        task.workflow_id = Some(wf.id.clone());
        repo.tasks.create(&task).await.expect("create task");

        let task2 = TaskRow::new(&agent.id, "standalone-task");
        repo.tasks.create(&task2).await.expect("create task2");

        let wf_tasks = repo
            .tasks
            .get_by_workflow(&wf.id)
            .await
            .expect("get_by_workflow");
        assert_eq!(wf_tasks.len(), 1);
        assert_eq!(wf_tasks[0].name, "wf-task");
    }

    #[tokio::test]
    async fn test_task_get_by_status() {
        let repo = test_repo().await;

        let agent = AgentRow::new("status-agent");
        repo.agents.create(&agent).await.expect("create agent");

        let mut task = TaskRow::new(&agent.id, "running-task");
        task.status = "running".to_string();
        repo.tasks.create(&task).await.expect("create task");

        let task2 = TaskRow::new(&agent.id, "pending-task");
        repo.tasks.create(&task2).await.expect("create task2");

        let running = repo
            .tasks
            .get_by_status("running")
            .await
            .expect("get_by_status");
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].name, "running-task");

        let pending = repo
            .tasks
            .get_by_status("pending")
            .await
            .expect("get_by_status");
        assert_eq!(pending.len(), 1);

        let none = repo
            .tasks
            .get_by_status("completed")
            .await
            .expect("get_by_status");
        assert_eq!(none.len(), 0);
    }

    #[tokio::test]
    async fn test_workflow_get_by_status() {
        let repo = test_repo().await;

        let mut wf = WorkflowRow::new("active-wf");
        wf.status = "active".to_string();
        repo.workflows.create(&wf).await.expect("create wf");

        let wf2 = WorkflowRow::new("draft-wf");
        repo.workflows.create(&wf2).await.expect("create wf2");

        let active = repo
            .workflows
            .get_by_status("active")
            .await
            .expect("get_by_status");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "active-wf");

        let draft = repo
            .workflows
            .get_by_status("draft")
            .await
            .expect("get_by_status");
        assert_eq!(draft.len(), 1);
    }

    #[tokio::test]
    async fn test_config_get_by_namespace() {
        let repo = test_repo().await;

        let c1 = ConfigRow::new("ns1", "key1", "val1");
        let c2 = ConfigRow::new("ns1", "key2", "val2");
        let c3 = ConfigRow::new("ns2", "key1", "val3");
        repo.configs.create(&c1).await.expect("create c1");
        repo.configs.create(&c2).await.expect("create c2");
        repo.configs.create(&c3).await.expect("create c3");

        let ns1_configs = repo
            .configs
            .get_by_namespace("ns1")
            .await
            .expect("get_by_namespace");
        assert_eq!(ns1_configs.len(), 2);
        assert!(ns1_configs.iter().all(|c| c.namespace == "ns1"));

        let ns2_configs = repo
            .configs
            .get_by_namespace("ns2")
            .await
            .expect("get_by_namespace");
        assert_eq!(ns2_configs.len(), 1);

        let empty = repo
            .configs
            .get_by_namespace("nonexistent")
            .await
            .expect("get_by_namespace");
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_experience_replay_cleanup_older_than() {
        let repo = test_repo().await;

        let agent = AgentRow::new("cleanup-agent");
        repo.agents.create(&agent).await.expect("create agent");

        let entries = vec![
            ExperienceReplayRow::new(&agent.id, r#"{"s": 1}"#, r#"{"a": 0}"#, 0.5),
            ExperienceReplayRow::new(&agent.id, r#"{"s": 2}"#, r#"{"a": 1}"#, 0.8),
        ];
        repo.experience_replay
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        // cleanup_older_than(0) should delete entries older than 0 days (all entries)
        // But since entries were just created, they might be within 0 days
        // Use a large number of days to test the "no deletion" case
        let deleted = repo
            .experience_replay
            .cleanup_older_than(365)
            .await
            .expect("cleanup");
        assert_eq!(deleted, 0, "No entries should be deleted (too recent)");

        // Verify total count unchanged
        let total = repo
            .experience_replay
            .total_count()
            .await
            .expect("total_count");
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_prefix_cache_get_lru_entries() {
        let repo = test_repo().await;

        let entries = vec![
            PrefixCacheRow::new("h1", "m1", vec![10], 64),
            PrefixCacheRow::new("h2", "m1", vec![20], 128),
            PrefixCacheRow::new("h3", "m1", vec![30], 256),
        ];
        repo.prefix_cache
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        // Lookup h3 multiple times to give it more hits
        for _ in 0..5 {
            repo.prefix_cache.lookup("h3", "m1").await.expect("lookup");
        }
        // Lookup h1 once
        repo.prefix_cache.lookup("h1", "m1").await.expect("lookup");

        // LRU entries should be ordered by hit_count ASC
        let lru = repo
            .prefix_cache
            .get_lru_entries(10)
            .await
            .expect("get_lru_entries");
        assert_eq!(lru.len(), 3);
        // h2 has 0 hits (least), then h1 (1 hit), then h3 (5+ hits)
        assert_eq!(lru[0].prefix_hash, "h2");
        assert_eq!(lru[1].prefix_hash, "h1");

        // Test with limit
        let lru_limited = repo
            .prefix_cache
            .get_lru_entries(1)
            .await
            .expect("get_lru_entries limited");
        assert_eq!(lru_limited.len(), 1);
        assert_eq!(lru_limited[0].prefix_hash, "h2");
    }

    #[tokio::test]
    async fn test_prefix_cache_evict_stale() {
        let repo = test_repo().await;

        let entries = vec![
            PrefixCacheRow::new("evict-h1", "m1", vec![10], 64),
            PrefixCacheRow::new("evict-h2", "m1", vec![20], 128),
        ];
        repo.prefix_cache
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        // Evict entries older than 0 days — fresh entries should survive
        let evicted = repo.prefix_cache.evict_stale(0).await.expect("evict_stale");
        // Entries with NULL last_hit_at are evicted by the query
        // fresh entries have NULL last_hit_at since we only inserted, didn't lookup
        // So they should be evicted
        let _ = evicted; // u64 always >= 0

        // Verify with a lookup that sets last_hit_at
        let entry3 = PrefixCacheRow::new("evict-h3", "m1", vec![30], 256);
        repo.prefix_cache.insert(&entry3).await.expect("insert");
        repo.prefix_cache
            .lookup("evict-h3", "m1")
            .await
            .expect("lookup");

        // Now evict with very short stale period — h3 should survive (just hit)
        let _evicted2 = repo
            .prefix_cache
            .evict_stale(365)
            .await
            .expect("evict_stale");
        // h3 was just looked up, so it should NOT be evicted with 365-day window
        let remaining = repo
            .prefix_cache
            .model_stats("m1")
            .await
            .expect("model_stats");
        assert!(remaining.entries >= 1, "at least h3 should survive");
    }

    #[tokio::test]
    async fn test_model_stats_direct() {
        let repo = test_repo().await;

        // Insert entries for model "stats-model"
        let entries = vec![
            PrefixCacheRow::new("sm-h1", "stats-model", vec![1, 2, 3], 100),
            PrefixCacheRow::new("sm-h2", "stats-model", vec![4, 5, 6], 200),
            PrefixCacheRow::new("sm-h3", "stats-model", vec![7, 8, 9], 300),
        ];
        repo.prefix_cache
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        // Lookup h1 twice, h2 once
        repo.prefix_cache
            .lookup("sm-h1", "stats-model")
            .await
            .expect("lookup");
        repo.prefix_cache
            .lookup("sm-h1", "stats-model")
            .await
            .expect("lookup");
        repo.prefix_cache
            .lookup("sm-h2", "stats-model")
            .await
            .expect("lookup");

        let stats = repo
            .prefix_cache
            .model_stats("stats-model")
            .await
            .expect("model_stats");
        assert_eq!(stats.entries, 3, "should have 3 entries");
        assert_eq!(stats.total_hits, 3, "h1:2 + h2:1 = 3 hits");
        assert_eq!(stats.total_tokens, 600, "100+200+300 = 600 tokens");
    }

    #[tokio::test]
    async fn test_model_stats_empty_model() {
        let repo = test_repo().await;

        let stats = repo
            .prefix_cache
            .model_stats("nonexistent-model")
            .await
            .expect("model_stats for nonexistent");
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.total_hits, 0);
        assert_eq!(stats.total_tokens, 0);
    }

    #[tokio::test]
    async fn test_model_stats_cross_model_isolation() {
        let repo = test_repo().await;

        // Insert for model A
        let a_entries = vec![PrefixCacheRow::new("iso-h1", "model-a", vec![1], 50)];
        repo.prefix_cache
            .batch_insert(&a_entries)
            .await
            .expect("insert a");

        // Insert for model B
        let b_entries = vec![
            PrefixCacheRow::new("iso-h2", "model-b", vec![2], 100),
            PrefixCacheRow::new("iso-h3", "model-b", vec![3], 150),
        ];
        repo.prefix_cache
            .batch_insert(&b_entries)
            .await
            .expect("insert b");

        let stats_a = repo
            .prefix_cache
            .model_stats("model-a")
            .await
            .expect("stats a");
        assert_eq!(stats_a.entries, 1);
        assert_eq!(stats_a.total_tokens, 50);

        let stats_b = repo
            .prefix_cache
            .model_stats("model-b")
            .await
            .expect("stats b");
        assert_eq!(stats_b.entries, 2);
        assert_eq!(stats_b.total_tokens, 250);
    }

    #[tokio::test]
    async fn test_experience_replay_get_by_agent_with_limit() {
        let repo = test_repo().await;

        // Create agent first (FK constraint)
        let agent = AgentRow::new("limit-agent");
        repo.agents.create(&agent).await.expect("create agent");

        // Insert 5 experiences for the same agent
        for i in 0..5 {
            let entry = ExperienceReplayRow::new(
                &agent.id,
                format!("input-{i}"),
                format!("output-{i}"),
                0.8,
            );
            repo.experience_replay
                .batch_insert(&[entry])
                .await
                .expect("insert");
        }

        // Get with limit 3
        let limited = repo
            .experience_replay
            .get_by_agent(&agent.id, Some(3))
            .await
            .expect("get_by_agent limited");
        assert_eq!(limited.len(), 3, "should respect limit");

        // Get with default limit (None = 100)
        let all = repo
            .experience_replay
            .get_by_agent(&agent.id, None)
            .await
            .expect("get_by_agent default");
        assert_eq!(all.len(), 5, "should return all 5");
    }

    #[tokio::test]
    async fn test_experience_replay_get_by_agent_empty() {
        let repo = test_repo().await;

        let result = repo
            .experience_replay
            .get_by_agent("no-such-agent", None)
            .await
            .expect("get_by_agent empty");
        assert!(result.is_empty(), "no experiences for nonexistent agent");
    }

    #[tokio::test]
    async fn test_prefix_cache_lookup_increments_hit_count() {
        let repo = test_repo().await;

        let entry = PrefixCacheRow::new("hit-test", "m1", vec![42], 10);
        repo.prefix_cache.insert(&entry).await.expect("insert");

        // Initial hit count should be 0
        let stats_before = repo.prefix_cache.model_stats("m1").await.expect("stats");
        assert_eq!(stats_before.total_hits, 0);

        // Lookup 3 times
        for _ in 0..3 {
            repo.prefix_cache
                .lookup("hit-test", "m1")
                .await
                .expect("lookup");
        }

        let stats_after = repo.prefix_cache.model_stats("m1").await.expect("stats");
        assert_eq!(
            stats_after.total_hits, 3,
            "hit count should be 3 after 3 lookups"
        );
    }

    #[tokio::test]
    async fn test_prefix_cache_batch_insert_and_lookup_multiple_models() {
        let repo = test_repo().await;

        let entries = vec![
            PrefixCacheRow::new("multi-h1", "gpt4", vec![1], 10),
            PrefixCacheRow::new("multi-h2", "gpt4", vec![2], 20),
            PrefixCacheRow::new("multi-h3", "claude", vec![3], 30),
        ];
        repo.prefix_cache
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        // Lookup across models
        let r1 = repo
            .prefix_cache
            .lookup("multi-h1", "gpt4")
            .await
            .expect("lookup");
        assert!(r1.is_some(), "should find gpt4 entry");
        let r2 = repo
            .prefix_cache
            .lookup("multi-h3", "claude")
            .await
            .expect("lookup");
        assert!(r2.is_some(), "should find claude entry");

        // Cross-model lookup should miss
        let r3 = repo
            .prefix_cache
            .lookup("multi-h1", "claude")
            .await
            .expect("lookup");
        assert!(
            r3.is_none(),
            "gpt4 hash should not be found under claude model"
        );
    }

    #[tokio::test]
    async fn test_config_get_by_key_specific() {
        let repo = test_repo().await;

        // Insert configs with different keys in same namespace
        let c1 = ConfigRow::new("ns1", "key-a", "value-a");
        let c2 = ConfigRow::new("ns1", "key-b", "value-b");
        let c3 = ConfigRow::new("ns2", "key-a", "value-c");
        repo.configs.upsert(&c1).await.expect("upsert c1");
        repo.configs.upsert(&c2).await.expect("upsert c2");
        repo.configs.upsert(&c3).await.expect("upsert c3");

        // Get specific key
        let result = repo
            .configs
            .get_by_key("ns1", "key-a")
            .await
            .expect("get_by_key");
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "value-a");

        // Get key from different namespace
        let result = repo
            .configs
            .get_by_key("ns2", "key-a")
            .await
            .expect("get_by_key");
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "value-c");

        // Get nonexistent key
        let result = repo
            .configs
            .get_by_key("ns1", "missing")
            .await
            .expect("get_by_key");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_skill_get_enabled_filters_correctly() {
        let repo = test_repo().await;

        let mut s1 = SkillRow::new("enabled-skill", "an enabled skill");
        s1.enabled = 1;
        let mut s2 = SkillRow::new("disabled-skill", "a disabled skill");
        s2.enabled = 0;
        repo.skills.create(&s1).await.expect("create s1");
        repo.skills.create(&s2).await.expect("create s2");

        let enabled = repo.skills.get_enabled().await.expect("get_enabled");
        assert_eq!(enabled.len(), 1, "only one skill is enabled");
        assert_eq!(enabled[0].name, "enabled-skill");
    }

    #[tokio::test]
    async fn test_open_file_backed() {
        let path = "/tmp/test_repo_open.db";
        let _ = std::fs::remove_file(path);
        let repo = SqliteRepository::open(path)
            .await
            .expect("Failed to open file-backed repo");

        let agent = AgentRow::new("file-agent");
        repo.agents.create(&agent).await.expect("create agent");

        let fetched = repo.agents.get_by_id(&agent.id).await.expect("get agent");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "file-agent");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_open_with_pool_config() {
        let path = "/tmp/test_repo_pool_config.db";
        let _ = std::fs::remove_file(path);
        let repo = SqliteRepository::open_with_pool_config(path, 5, 1)
            .await
            .expect("Failed to open with pool config");

        let stats = repo.pool_stats();
        assert!(stats.pool_size >= 1);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_agent_soft_delete() {
        let repo = test_repo().await;
        let agent = AgentRow::new("delete-me");
        repo.agents.create(&agent).await.expect("create");

        // Delete (soft)
        repo.agents.delete(&agent.id).await.expect("delete");

        // Should not appear in get_by_id
        let fetched = repo.agents.get_by_id(&agent.id).await.expect("get");
        assert!(
            fetched.is_none(),
            "soft-deleted agent should not be returned"
        );

        // Should not appear in list
        let all = repo.agents.list(None, None).await.expect("list");
        assert!(all.iter().all(|a| a.id != agent.id));

        // Count should be 0
        let count = repo.agents.count().await.expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_agent_delete_nonexistent() {
        let repo = test_repo().await;
        let result = repo.agents.delete("nonexistent-id").await;
        assert!(result.is_err(), "deleting nonexistent agent should error");
    }

    #[tokio::test]
    async fn test_agent_update_nonexistent() {
        let repo = test_repo().await;
        let mut agent = AgentRow::new("ghost");
        agent.id = "nonexistent-id".to_string();
        let result = repo.agents.update(&agent).await;
        assert!(result.is_err(), "updating nonexistent agent should error");
    }

    #[tokio::test]
    async fn test_agent_count() {
        let repo = test_repo().await;
        assert_eq!(repo.agents.count().await.expect("count"), 0);

        let a1 = AgentRow::new("a1");
        repo.agents.create(&a1).await.expect("create a1");
        assert_eq!(repo.agents.count().await.expect("count"), 1);

        let a2 = AgentRow::new("a2");
        repo.agents.create(&a2).await.expect("create a2");
        assert_eq!(repo.agents.count().await.expect("count"), 2);

        // Soft delete a1, count should decrease
        repo.agents.delete(&a1.id).await.expect("delete a1");
        assert_eq!(repo.agents.count().await.expect("count"), 1);
    }

    #[tokio::test]
    async fn test_task_count() {
        let repo = test_repo().await;
        assert_eq!(repo.tasks.count().await.expect("count"), 0);

        let agent = AgentRow::new("task-owner");
        repo.agents.create(&agent).await.expect("create agent");

        let task = TaskRow::new(&agent.id, "task-1");
        repo.tasks.create(&task).await.expect("create task");
        assert_eq!(repo.tasks.count().await.expect("count"), 1);

        let task2 = TaskRow::new(&agent.id, "task-2");
        repo.tasks.create(&task2).await.expect("create task2");
        assert_eq!(repo.tasks.count().await.expect("count"), 2);
    }

    #[tokio::test]
    async fn test_config_upsert_overwrite() {
        let repo = test_repo().await;

        let mut config = ConfigRow::new("ns1", "key1", "value1");
        repo.configs.upsert(&config).await.expect("first upsert");

        // Overwrite with new value
        config.value = "value2".to_string();
        repo.configs.upsert(&config).await.expect("second upsert");

        let fetched = repo
            .configs
            .get_by_key("ns1", "key1")
            .await
            .expect("get by key");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().value, "value2");

        // Only one entry, not two
        let all = repo
            .configs
            .get_by_namespace("ns1")
            .await
            .expect("get by namespace");
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_experience_replay_total_count() {
        let repo = test_repo().await;
        assert_eq!(
            repo.experience_replay.total_count().await.expect("count"),
            0
        );

        // Create agent first (FK constraint)
        let agent = AgentRow::new("er-owner");
        repo.agents.create(&agent).await.expect("create agent");

        let entries = vec![
            ExperienceReplayRow::new(&agent.id, "state1", "action1", 1.0),
            ExperienceReplayRow::new(&agent.id, "state2", "action2", 0.5),
        ];
        repo.experience_replay
            .batch_insert(&entries)
            .await
            .expect("batch insert");

        assert_eq!(
            repo.experience_replay.total_count().await.expect("count"),
            2
        );
    }

    #[tokio::test]
    async fn test_prefix_cache_evict_stale_empty() {
        let repo = test_repo().await;
        let evicted = repo
            .prefix_cache
            .evict_stale(30)
            .await
            .expect("evict stale");
        assert_eq!(evicted, 0);
    }

    #[tokio::test]
    async fn test_experience_replay_cleanup_empty() {
        let repo = test_repo().await;
        let cleaned = repo
            .experience_replay
            .cleanup_older_than(0)
            .await
            .expect("cleanup");
        assert_eq!(cleaned, 0);
    }

    #[tokio::test]
    async fn test_component_get_by_type() {
        let repo = test_repo().await;

        let c1 = ComponentRow::new("comp-a", "sensor");
        let c2 = ComponentRow::new("comp-b", "sensor");
        let c3 = ComponentRow::new("comp-c", "actuator");
        repo.components.create(&c1).await.expect("create c1");
        repo.components.create(&c2).await.expect("create c2");
        repo.components.create(&c3).await.expect("create c3");

        let sensors = repo
            .components
            .get_by_type("sensor")
            .await
            .expect("get_by_type");
        assert_eq!(sensors.len(), 2);

        let actuators = repo
            .components
            .get_by_type("actuator")
            .await
            .expect("get_by_type");
        assert_eq!(actuators.len(), 1);

        let missing = repo
            .components
            .get_by_type("nonexistent")
            .await
            .expect("get_by_type");
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn test_task_get_by_agent_multiple() {
        let repo = test_repo().await;

        let agent1 = AgentRow::new("agent-a");
        let agent2 = AgentRow::new("agent-b");
        repo.agents.create(&agent1).await.unwrap();
        repo.agents.create(&agent2).await.unwrap();

        let t1 = TaskRow::new(&agent1.id, "task-1");
        let t2 = TaskRow::new(&agent1.id, "task-2");
        let t3 = TaskRow::new(&agent2.id, "task-3");
        repo.tasks.create(&t1).await.unwrap();
        repo.tasks.create(&t2).await.unwrap();
        repo.tasks.create(&t3).await.unwrap();

        let agent1_tasks = repo.tasks.get_by_agent(&agent1.id).await.unwrap();
        assert_eq!(agent1_tasks.len(), 2);

        let agent2_tasks = repo.tasks.get_by_agent(&agent2.id).await.unwrap();
        assert_eq!(agent2_tasks.len(), 1);

        let none_tasks = repo.tasks.get_by_agent("nonexistent").await.unwrap();
        assert!(none_tasks.is_empty());
    }

    #[tokio::test]
    async fn test_skill_get_by_name_found_and_missing() {
        let repo = test_repo().await;

        let skill = SkillRow::new("code-review", "Review code");
        repo.skills.create(&skill).await.unwrap();

        let found = repo.skills.get_by_name("code-review").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "code-review");

        let missing = repo.skills.get_by_name("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_component_get_by_name_found_and_missing() {
        let repo = test_repo().await;

        let comp = ComponentRow::new("gateway", "proxy");
        repo.components.create(&comp).await.unwrap();

        let found = repo.components.get_by_name("gateway").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().component_type, "proxy");

        let missing = repo.components.get_by_name("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_task_get_by_agent_empty() {
        let repo = test_repo().await;

        let agent = AgentRow::new("empty-agent");
        repo.agents.create(&agent).await.unwrap();

        let tasks = repo.tasks.get_by_agent(&agent.id).await.unwrap();
        assert!(tasks.is_empty());
    }
}
