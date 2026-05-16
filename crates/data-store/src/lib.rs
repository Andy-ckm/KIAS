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

pub mod audit_persist;
pub mod cache_persist;
pub mod dlq;
pub mod migrations;
pub mod models;
pub mod repository;
pub mod vector_persist;

pub use audit_persist::SqliteAuditLog;
pub use cache_persist::{CacheEntry, CacheStrategy, SqliteCacheStrategy};
pub use dlq::{DeadLetterEntry, DeadLetterQueue, DeadLetterReason, DlqStats};
pub use migrations::MigrationRunner;
pub use models::{
    AgentRow, ComponentRow, ConfigRow, ExperienceReplayRow, PrefixCacheRow, SkillRow, TaskRow,
    WorkflowRow,
};
pub use repository::{
    AgentRepository, ComponentRepository, ConfigRepository, DatabaseHealth,
    ExperienceReplayRepository, PoolStats, PrefixCacheRepository, PrefixCacheStats,
    Repository, SkillRepository, SqliteRepository, TaskRepository, WorkflowRepository,
};
pub use vector_persist::{PersistentVectorStore, VectorSearchResult};
