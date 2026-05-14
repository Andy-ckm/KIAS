-- Migration 004: Agent experience replay + prefix cache tables
-- Version: 4
-- Description: Create experience replay table for agent learning and prefix cache for KV optimization

-- Agent experience replay: stores state-action-reward transitions for RL-based agent learning
CREATE TABLE IF NOT EXISTS experience_replay (
    id TEXT PRIMARY KEY NOT NULL,
    agent_id TEXT NOT NULL,
    task_id TEXT,
    state_snapshot TEXT NOT NULL,
    action_taken TEXT NOT NULL,
    reward REAL NOT NULL DEFAULT 0.0,
    next_state TEXT,
    done INTEGER NOT NULL DEFAULT 0,
    episode_id TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

CREATE INDEX IF NOT EXISTS idx_experience_replay_agent ON experience_replay(agent_id);
CREATE INDEX IF NOT EXISTS idx_experience_replay_episode ON experience_replay(episode_id);
CREATE INDEX IF NOT EXISTS idx_experience_replay_reward ON experience_replay(reward);

-- Prefix cache: DeepSeek-style KV prefix caching for LLM inference optimization
-- Stores cached KV tensors keyed by token prefix hash, enabling reuse across
-- requests that share a common prefix (e.g., system prompt).
CREATE TABLE IF NOT EXISTS prefix_cache (
    prefix_hash TEXT NOT NULL,
    model_id TEXT NOT NULL,
    kv_data BLOB NOT NULL,
    token_count INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    last_hit_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (prefix_hash, model_id)
);

CREATE INDEX IF NOT EXISTS idx_prefix_cache_model ON prefix_cache(model_id);
CREATE INDEX IF NOT EXISTS idx_prefix_cache_hits ON prefix_cache(hit_count DESC);
