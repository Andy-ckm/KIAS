-- Migration 008: Idempotency key store for API/task/retry end-to-end idempotency
-- Stores idempotency keys with TTL to prevent duplicate processing

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key              TEXT PRIMARY KEY,
    method           TEXT NOT NULL,
    path             TEXT NOT NULL,
    operation_hash   TEXT NOT NULL,  -- SHA256(method + path + body_hash)
    request_body     TEXT,
    response_status  INTEGER NOT NULL,
    response_body   BLOB,
    response_headers TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at       TEXT NOT NULL,
    hit_count        INTEGER NOT NULL DEFAULT 1,
    completed        INTEGER NOT NULL DEFAULT 0  -- 0=in_progress, 1=completed
);

CREATE INDEX IF NOT EXISTS idx_idempotency_expires ON idempotency_keys(expires_at);
CREATE INDEX IF NOT EXISTS idx_idempotency_operation ON idempotency_keys(operation_hash);
CREATE INDEX IF NOT EXISTS idx_idempotency_created ON idempotency_keys(created_at);
