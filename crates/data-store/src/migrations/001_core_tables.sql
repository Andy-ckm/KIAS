-- Migration 001: Core tables for KIAS data store
-- Version: 1
-- Description: Create core tables: agents, tasks, workflows, configs, skills, components

-- Agents: persistent storage for agent definitions and state
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    node_id TEXT,
    image TEXT,
    priority INTEGER NOT NULL DEFAULT 50,
    cpu REAL NOT NULL DEFAULT 0.0,
    memory_bytes INTEGER NOT NULL DEFAULT 0,
    gpu INTEGER NOT NULL DEFAULT 0,
    labels TEXT NOT NULL DEFAULT '{}',
    system_prompt_hash INTEGER,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
CREATE INDEX IF NOT EXISTS idx_agents_node_id ON agents(node_id);
CREATE INDEX IF NOT EXISTS idx_agents_deleted_at ON agents(deleted_at);

-- Tasks: persistent storage for task definitions and execution history
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    agent_id TEXT NOT NULL,
    workflow_id TEXT,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    task_type TEXT NOT NULL DEFAULT 'generic',
    input TEXT NOT NULL DEFAULT '{}',
    output TEXT,
    error_message TEXT,
    priority INTEGER NOT NULL DEFAULT 50,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    timeout_seconds INTEGER,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

CREATE INDEX IF NOT EXISTS idx_tasks_agent_id ON tasks(agent_id);
CREATE INDEX IF NOT EXISTS idx_tasks_workflow_id ON tasks(workflow_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

-- Workflows: persistent storage for workflow definitions
CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft',
    workflow_type TEXT NOT NULL DEFAULT 'dag',
    config TEXT NOT NULL DEFAULT '{}',
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflows_status ON workflows(status);
CREATE INDEX IF NOT EXISTS idx_workflows_deleted_at ON workflows(deleted_at);

-- Workflow steps: individual steps within a workflow
CREATE TABLE IF NOT EXISTS workflow_steps (
    id TEXT PRIMARY KEY NOT NULL,
    workflow_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    name TEXT NOT NULL,
    step_type TEXT NOT NULL DEFAULT 'shell',
    config TEXT NOT NULL DEFAULT '{}',
    depends_on TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_steps_workflow_id ON workflow_steps(workflow_id);

-- Configs: key-value configuration store
CREATE TABLE IF NOT EXISTS configs (
    id TEXT PRIMARY KEY NOT NULL,
    namespace TEXT NOT NULL DEFAULT 'default',
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL DEFAULT 'string',
    description TEXT NOT NULL DEFAULT '',
    is_secret INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(namespace, key)
);

CREATE INDEX IF NOT EXISTS idx_configs_namespace ON configs(namespace);

-- Skills: skill definitions and registry
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '0.1.0',
    skill_type TEXT NOT NULL DEFAULT 'builtin',
    config TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
CREATE INDEX IF NOT EXISTS idx_skills_type ON skills(skill_type);

-- Components: registered components in the system
CREATE TABLE IF NOT EXISTS components (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    component_type TEXT NOT NULL DEFAULT 'service',
    version TEXT NOT NULL DEFAULT '0.1.0',
    status TEXT NOT NULL DEFAULT 'registered',
    endpoint TEXT,
    config TEXT NOT NULL DEFAULT '{}',
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_components_name ON components(name);
CREATE INDEX IF NOT EXISTS idx_components_type ON components(component_type);
