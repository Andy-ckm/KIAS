# Data Layer Architecture

> AgentGuard Data Store — Unified persistence layer for all domain entities

## Overview

The `kias-data-store` crate provides a complete data layer for AgentGuard, offering:

1. **Structured Database** — SQLite-backed CRUD for all domain entities
2. **Vector Storage** — Persistent vector embeddings with in-memory HNSW search
3. **Cache Layer** — SQLite-backed key-value cache with TTL support

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    L2+ Crates                                │
│  (api-server, scheduler, controller, workflow-engine, etc.)  │
│                              ▲                               │
│                              │ uses                          │
│              ┌───────────────┼───────────────┐               │
│              ▼               ▼               ▼               │
│    ┌─────────────┐  ┌──────────────┐  ┌──────────────┐      │
│    │ Repository  │  │ VectorStore  │  │ CacheStrategy│      │
│    │ (SQLite)    │  │ (HNSW+SQLite)│  │ (SQLite)     │      │
│    └─────────────┘  └──────────────┘  └──────────────┘      │
│                    kias-data-store  ← L1                     │
└──────────────────────────────────────────────────────────────┘
                              │
                              │ depends on
                    ┌─────────┴──────────┐
                    │   kias-common       │  ← L0
                    └────────────────────┘
```

## Components

### 1. Migration System (`migrations/`)

Versioned schema management with atomic migrations.

- **Migration tracking**: `_migrations` table records applied versions
- **Atomic execution**: Each migration runs in a transaction
- **SQL files**: Embedded via `include_str!()` for zero-copy at runtime
- **Current schema**: 3 migrations (core tables, vector tables, cache table)

```rust
use kias_data_store::MigrationRunner;
use sqlx::SqlitePool;

let pool = SqlitePool::connect("sqlite:kias.db?mode=rwc").await?;
let runner = MigrationRunner::new(pool);
let applied = runner.run_all().await?;
```

### 2. Repository Layer (`repository/`)

Generic `Repository<T>` trait with SQLite implementations for 6 domain entities:

| Entity     | Repository          | Table       | Soft Delete |
|------------|---------------------|-------------|-------------|
| Agent      | `AgentRepository`   | `agents`    | ✅          |
| Task       | `TaskRepository`    | `tasks`     | ❌          |
| Workflow   | `WorkflowRepository`| `workflows` | ✅          |
| Config     | `ConfigRepository`  | `configs`   | ❌          |
| Skill      | `SkillRepository`   | `skills`    | ❌          |
| Component  | `ComponentRepository`| `components`| ❌         |

The `Repository<T>` trait provides:
- `create(&self, entity: &T)` — Insert
- `get_by_id(&self, id: &str)` — Read by ID
- `list(&self, limit, offset)` — Paginated list
- `update(&self, entity: &T)` — Update
- `delete(&self, id: &str)` — Delete (soft or hard)
- `count(&self)` — Count records

```rust
use kias_data_store::{SqliteRepository, AgentRow, Repository};

let repo = SqliteRepository::in_memory().await?;
let agent = AgentRow::new("my-agent");
repo.agents.create(&agent).await?;

let fetched = repo.agents.get_by_id(&agent.id).await?;
let running = repo.agents.get_by_status("running").await?;
```

### 3. Vector Storage (`vector_persist/`)

Persistent vector store with in-memory HNSW search:

- **Write-through**: Every insert writes to both SQLite and in-memory index
- **Read-through**: Loads all vectors from SQLite on startup
- **Crash recovery**: SQLite provides durability; in-memory provides speed
- **Cosine similarity**: O(N) search with sorted results

```rust
use kias_data_store::PersistentVectorStore;

let store = PersistentVectorStore::new(pool);
store.create_index("embeddings", 128, "cosine").await?;
store.load_from_db().await?;

// Insert
store.insert("embeddings", "doc-1", &embedding_vec, json!({"source": "file.rs"})).await?;

// Search
let results = store.search("embeddings", &query_vec, 5)?;
for r in results {
    println!("{}: {:.4}", r.external_id, r.similarity);
}
```

### 4. Cache Layer (`cache_persist/`)

SQLite-backed key-value cache with TTL and namespace support:

- **TTL expiration**: Lazy cleanup on read
- **Namespace isolation**: Multiple independent caches in one DB
- **Access counting**: Hit-rate monitoring
- **Write-through**: Immediate persistence

```rust
use kias_data_store::{SqliteCacheStrategy, CacheEntry, CacheStrategy};

let cache = SqliteCacheStrategy::with_namespace(pool, "api-cache");
cache.set(CacheEntry::with_ttl("key1", data, Duration::from_secs(3600))).await?;
let entry = cache.get("key1").await?;
```

## Database Schema

### Core Tables (Migration 1)
- `agents` — Agent definitions with resources, labels, metadata
- `tasks` — Task execution records with retry tracking
- `workflows` — Workflow definitions (DAG, sequential)
- `workflow_steps` — Individual steps within workflows
- `configs` — Namespaced key-value configuration
- `skills` — Skill registry with versioning
- `components` — Registered system components

### Vector Tables (Migration 2)
- `vector_indices` — Index metadata (dimension, metric, HNSW params)
- `vector_entries` — Individual vectors with binary embedding storage

### Cache Table (Migration 3)
- `cache_entries` — Key-value cache with TTL and access tracking

## Integration Guide

### Adding to an existing crate

```toml
# Cargo.toml
[dependencies]
kias-data-store = { path = "../data-store" }
```

### Initialization

```rust
use kias_data_store::{SqliteRepository, PersistentVectorStore, SqliteCacheStrategy, MigrationRunner};
use sqlx::SqlitePool;

// Create pool
let pool = SqlitePool::connect("sqlite:kias.db?mode=rwc").await?;

// Run migrations
MigrationRunner::new(pool.clone()).run_all().await?;

// Create data store
let repo = SqliteRepository::new(pool.clone());
let vectors = PersistentVectorStore::new(pool.clone());
let cache = SqliteCacheStrategy::new(pool);
```

### PostgreSQL Migration Path

The `Repository<T>` trait is backend-agnostic. To add PostgreSQL support:

1. Create `PostgresRepository` implementing `Repository<T>`
2. Use the same domain models (different `FromRow` derive)
3. Swap at initialization — zero changes to business logic

## Performance Characteristics

| Operation | Expected Latency | Notes |
|-----------|-----------------|-------|
| Agent CRUD | < 1ms | SQLite WAL mode |
| Vector insert | < 2ms | Write-through (SQLite + memory) |
| Vector search (1K) | < 100µs | In-memory cosine similarity |
| Cache get | < 500µs | SQLite indexed lookup |
| Migration | < 100ms | One-time startup cost |

## Testing

```bash
# Run data-store tests only
cargo test -p kias-data-store

# Run with output
cargo test -p kias-data-store -- --nocapture
```

All tests use in-memory SQLite (`sqlite::memory:`) for zero I/O overhead.
