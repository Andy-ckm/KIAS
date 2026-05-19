# AgentGuard Data Layer Architecture

## Overview

The AgentGuard data layer provides unified persistence for the entire system via the `kias-data-store` crate (L1). It follows the architecture principle of depending only on `kias-common` (L0), making it available to all L2+ crates.

```
┌──────────────────────────────────────────────────────────────────┐
│                    L2+ Crates (api-server, kias-main, etc.)      │
│                              ▲                                   │
│                              │ depends on                        │
│                    ┌─────────┴──────────┐                       │
│                    │   kias-data-store   │  ← L1                │
│                    │   (this crate)      │                       │
│                    └─────────┬──────────┘                       │
│                              │ depends on                        │
│                    ┌─────────┴──────────┐                       │
│                    │   kias-common       │  ← L0                │
│                    └────────────────────┘                       │
└──────────────────────────────────────────────────────────────────┘
```

## Components

### 1. SQLite Repository (Structured Data)

Generic `Repository<T>` trait with SQLite implementation for 6 domain models:

| Model | Table | Key Features |
|-------|-------|--------------|
| `AgentRow` | `agents` | Soft-delete, status/node queries |
| `TaskRow` | `tasks` | Agent/workflow/status queries |
| `WorkflowRow` | `workflows` | Soft-delete, status queries |
| `ConfigRow` | `configs` | Namespace+key upsert |
| `SkillRow` | `skills` | Name lookup, enabled filter |
| `ComponentRow` | `components` | Type queries |
| `ExperienceReplayRow` | `experience_replay` | Batch insert, episode/agent queries, random sampling |
| `PrefixCacheRow` | `prefix_cache` | Lookup with hit tracking, LRU eviction, model stats |

**Key abstractions:**
- `Repository<T>` trait — CRUD operations, swappable backend
- `SqliteRepository` — Unified facade holding all repositories
- `DatabaseHealth` / `PoolStats` — Observability types

### 2. Vector Store (Semantic Search)

`PersistentVectorStore` bridges in-memory HNSW index with SQLite persistence:

- **Write-through**: Every insert writes to both SQLite and DashMap
- **Read-through**: On startup, loads all vectors from SQLite
- **Crash recovery**: SQLite provides durability; DashMap provides speed
- **Cosine similarity**: In-memory KNN search

**Tables:** `vector_indices`, `vector_entries`

### 3. Cache Strategy (Persistent KV Cache)

`SqliteCacheStrategy` provides persistent key-value caching:

- **TTL-based expiration**: Lazy cleanup on read
- **Namespace isolation**: Multiple independent caches
- **Access counting**: Hit-rate monitoring
- **Compatible** with `CacheStrategy` trait from `kias-cache`

**Table:** `cache_entries`

### 4. Prefix Cache (DeepSeek-style KV Optimization)

`PrefixCacheRepository` implements DeepSeek-style KV prefix caching:

- **Prefix hashing**: Cache KV tensors by token prefix hash
- **Hit tracking**: Automatic hit count increment on lookup
- **LRU eviction**: Evict stale entries by hit count
- **Model stats**: Per-model cache statistics

**Table:** `prefix_cache`

### 5. Experience Replay (Agent Learning)

`ExperienceReplayRepository` supports RL-based agent training:

- **SARS transitions**: State-Action-Reward-NextState storage
- **Episode tracking**: Group transitions by episode
- **Random sampling**: Sample N random experiences for training
- **Batch insert**: High-throughput transactional inserts
- **Cleanup**: Delete old experiences by age

**Table:** `experience_replay`

## Migration System

Versioned schema management with atomic apply:

| Version | Description |
|---------|-------------|
| 1 | Core tables (agents, tasks, workflows, configs, skills, components) |
| 2 | Vector storage tables |
| 3 | Cache storage table |
| 4 | Experience replay + prefix cache tables |

Migrations run automatically on startup via `MigrationRunner::run_all()`.

## Integration

The data store is initialized in `KiasServiceManager::new()`:

```rust
// File-backed SQLite with fallback to in-memory
let data_store = SqliteRepository::open("kias.db").await
    .or_else(|_| SqliteRepository::in_memory().await)?;

let vector_store = PersistentVectorStore::new(data_store.pool.clone());
let cache_strategy = SqliteCacheStrategy::new(data_store.pool.clone());
```

Access via:
- `manager.data_store()` → `&SqliteRepository`
- `manager.vector_store()` → `&PersistentVectorStore`

## Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `AgentGuard_DB_PATH` | `kias.db` | SQLite database file path |

## Quality Standards

- **Zero clippy warnings** — `cargo clippy -- -D warnings`
- **Zero unwrap in production code** — All errors use `KiasError`
- **41+ tests** — CRUD, batch ops, cache, vector, health check, migrations
- **L1 architecture** — Depends only on `kias-common`

## Future Work

- [ ] PostgreSQL backend for `Repository<T>` trait
- [ ] SQLite-vss for vector similarity (native SQL)
- [ ] Connection pool tuning (idle timeout, max lifetime)
- [ ] WAL mode for concurrent read performance
- [ ] Backup/restore utilities
