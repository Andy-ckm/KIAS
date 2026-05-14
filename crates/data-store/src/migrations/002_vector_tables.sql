-- Migration 002: Vector storage tables
-- Version: 2
-- Description: Create vector storage tables for HNSW index persistence

-- Vector indices: metadata for each vector index
CREATE TABLE IF NOT EXISTS vector_indices (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    dimension INTEGER NOT NULL DEFAULT 128,
    metric TEXT NOT NULL DEFAULT 'cosine',
    hnsw_m INTEGER NOT NULL DEFAULT 16,
    hnsw_ef_construction INTEGER NOT NULL DEFAULT 200,
    entry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_vector_indices_name ON vector_indices(name);

-- Vector entries: individual vectors stored in an index
CREATE TABLE IF NOT EXISTS vector_entries (
    id TEXT PRIMARY KEY NOT NULL,
    index_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    embedding BLOB NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (index_id) REFERENCES vector_indices(id)
);

CREATE INDEX IF NOT EXISTS idx_vector_entries_index_id ON vector_entries(index_id);
CREATE INDEX IF NOT EXISTS idx_vector_entries_external_id ON vector_entries(external_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_vector_entries_unique ON vector_entries(index_id, external_id);
