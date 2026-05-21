-- Migration 007: Token budget management tables
-- Version: 7
-- Description: Create token_budgets, token_usage, spend_alerts tables for budget tracking

CREATE TABLE IF NOT EXISTS token_budgets (
    agent_id TEXT PRIMARY KEY NOT NULL,
    agent_name TEXT NOT NULL DEFAULT '',
    daily_limit INTEGER NOT NULL DEFAULT 0,
    monthly_limit INTEGER NOT NULL DEFAULT 0,
    input_cost_per_1k REAL NOT NULL DEFAULT 0.0,
    output_cost_per_1k REAL NOT NULL DEFAULT 0.0,
    alert_threshold REAL NOT NULL DEFAULT 0.8,
    rollover_policy TEXT NOT NULL DEFAULT 'none',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_token_budgets_deleted_at ON token_budgets(deleted_at);

CREATE TABLE IF NOT EXISTS token_usage (
    id TEXT PRIMARY KEY NOT NULL,
    agent_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost REAL NOT NULL DEFAULT 0.0,
    period TEXT NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_token_usage_agent_id ON token_usage(agent_id);
CREATE INDEX IF NOT EXISTS idx_token_usage_task_id ON token_usage(task_id);
CREATE INDEX IF NOT EXISTS idx_token_usage_period ON token_usage(period);
CREATE INDEX IF NOT EXISTS idx_token_usage_agent_period ON token_usage(agent_id, period);
CREATE INDEX IF NOT EXISTS idx_token_usage_recorded_at ON token_usage(recorded_at);

CREATE TABLE IF NOT EXISTS spend_alerts (
    id TEXT PRIMARY KEY NOT NULL,
    agent_id TEXT NOT NULL,
    threshold REAL NOT NULL,
    delivery_method TEXT NOT NULL,
    target TEXT NOT NULL,
    triggered_at TEXT NOT NULL DEFAULT (datetime('now')),
    message TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_spend_alerts_agent_id ON spend_alerts(agent_id);
CREATE INDEX IF NOT EXISTS idx_spend_alerts_triggered_at ON spend_alerts(triggered_at);
