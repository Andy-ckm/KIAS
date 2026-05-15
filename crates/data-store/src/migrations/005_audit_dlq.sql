-- KIAS Production Necessities: Audit Log + Dead Letter Queue
-- Migration 005: Audit log persistence + DLQ tables

-- Persistent audit log (replaces in-memory ring buffer for production)
CREATE TABLE IF NOT EXISTS audit_log (
    id              TEXT PRIMARY KEY,
    timestamp       TEXT NOT NULL DEFAULT (datetime('now')),
    actor           TEXT NOT NULL,
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    details         TEXT NOT NULL DEFAULT '',
    ip_address      TEXT,
    user_agent      TEXT,
    outcome         TEXT NOT NULL CHECK (outcome IN ('Success', 'Failure'))
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_log(actor);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_resource ON audit_log(resource_type, resource_id);

-- Dead Letter Queue for permanently failed tasks
CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL,
    agent_id        TEXT NOT NULL,
    workflow_id     TEXT,
    task_name       TEXT NOT NULL,
    task_type       TEXT NOT NULL,
    input           TEXT,
    last_error      TEXT NOT NULL,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    max_retries     INTEGER NOT NULL DEFAULT 3,
    failed_at       TEXT NOT NULL DEFAULT (datetime('now')),
    original_created_at TEXT,
    dead_letter_reason  TEXT NOT NULL DEFAULT 'max_retries_exceeded',
    can_retry       INTEGER NOT NULL DEFAULT 1,
    metadata        TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_dlq_task_id ON dead_letter_queue(task_id);
CREATE INDEX IF NOT EXISTS idx_dlq_agent_id ON dead_letter_queue(agent_id);
CREATE INDEX IF NOT EXISTS idx_dlq_failed_at ON dead_letter_queue(failed_at);
CREATE INDEX IF NOT EXISTS idx_dlq_can_retry ON dead_letter_queue(can_retry);
