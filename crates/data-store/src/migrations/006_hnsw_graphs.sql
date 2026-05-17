-- HNSW graph snapshots for fast restart
-- Stores serialized graph topology (layers + connections) as JSON blobs.
-- On startup, load_graph() restores O(N) instead of re-inserting O(N·M·logN).
CREATE TABLE IF NOT EXISTS hnsw_graphs (
    id TEXT PRIMARY KEY,
    index_name TEXT NOT NULL UNIQUE,
    snapshot_json TEXT NOT NULL,
    vector_count INTEGER NOT NULL DEFAULT 0,
    layer_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_hnsw_graphs_index_name ON hnsw_graphs(index_name);
