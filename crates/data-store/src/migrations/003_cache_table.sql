-- Migration 003: Cache storage table
-- Version: 3
-- Description: Create cache storage table for persistent key-value cache

-- Cache entries: persistent KV cache with TTL support
-- Primary key is (key, namespace) to support namespace isolation
CREATE TABLE IF NOT EXISTS cache_entries (
    key TEXT NOT NULL,
    value BLOB NOT NULL,
    namespace TEXT NOT NULL DEFAULT 'default',
    ttl_seconds INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    access_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (key, namespace)
);

CREATE INDEX IF NOT EXISTS idx_cache_entries_namespace ON cache_entries(namespace);
CREATE INDEX IF NOT EXISTS idx_cache_entries_created_at ON cache_entries(created_at);
