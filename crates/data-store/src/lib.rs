//! # KIAS Data Store
//!
//! Unified data layer for the KIAS (Kubernetes-like Intelligent Agent Scheduling) system.
//!
//! This crate provides:
//! - **migrations** — Versioned schema management for SQLite
//! - **models** — Persistent domain models (Agent, Task, Workflow, Config, Skill, Component)
//! - **repository** — Generic `Repository<T>` trait + SQLite implementation
//! - **vector_persist** — Persistent vector storage backed by SQLite
//! - **cache_persist** — SQLite-backed `CacheStrategy` implementation
//!
//! ## Architecture (L1)
//!
//! Depends only on `kias-common` (L0). All L2+ crates can depend on this for persistence.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                    L2+ Crates (api-server, etc.)            │
//! │                              ▲                               │
//! │                              │ depends on                    │
//! │                    ┌─────────┴──────────┐                   │
//! │                    │   kias-data-store   │  ← L1            │
//! │                    │   (this crate)      │                   │
//! │                    └─────────┬──────────┘                   │
//! │                              │ depends on                    │
//! │                    ┌─────────┴──────────┐                   │
//! │                    │   kias-common       │  ← L0            │
//! │                    └────────────────────┘                   │
//! └──────────────────────────────────────────────────────────────┘
//! ```

pub mod cache_persist;
pub mod migrations;
pub mod models;
pub mod repository;
pub mod vector_persist;

pub use cache_persist::{CacheEntry, CacheStrategy, SqliteCacheStrategy};
pub use migrations::MigrationRunner;
pub use models::{
    AgentRow, ComponentRow, ConfigRow, ExperienceReplayRow, PrefixCacheRow, SkillRow, TaskRow,
    WorkflowRow,
};
pub use repository::{
    DatabaseHealth, ExperienceReplayRepository, PoolStats, PrefixCacheRepository, PrefixCacheStats,
    Repository, SqliteRepository,
};
pub use vector_persist::{PersistentVectorStore, VectorSearchResult};
