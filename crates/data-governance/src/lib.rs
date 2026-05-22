//! # KIAS Data Governance Layer
//!
//! Implements the SAP Three Pillars for the KIAS agent scheduling system:
//!
//! 1. **Multi-Datasource Access** ([`datasource`]) — Abstract data source trait + registry
//!    supporting SQLite, PostgreSQL, and extensible backends.
//!
//! 2. **Resource-Level Permissions** ([`policy`]) — Fine-grained access control policies
//!    that extend the existing RBAC system with resource-type-level rules.
//!
//! 3. **Audit Trail** ([`audit_middleware`]) — Automatic capture of all data-mutating
//!    operations through Axum middleware, persisted via [`kias_data_store::SqliteAuditLog`].
//!
//! ## Architecture (L1)
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                 L2+ Crates (api-server, etc.)                │
//! │                              ▲                               │
//! │                              │ depends on                    │
//! │              ┌───────────────┴───────────────┐               │
//! │              │   kias-data-governance  (L1)   │               │
//! │              └───────────────┬───────────────┘               │
//! │                  ┌───────────┴───────────┐                   │
//! │          ┌───────┴───────┐       ┌───────┴───────┐          │
//! │          │ kias-data-store│       │  kias-common  │          │
//! │          │      (L1)     │       │     (L0)      │          │
//! │          └───────────────┘       └───────────────┘          │
//! └──────────────────────────────────────────────────────────────┘
//! ```

pub mod accountability;
pub mod audit_middleware;
// pub // mod cost_attribution; // TODO: fix compilation // TODO: fix compilation
pub mod data_bridge;
pub mod datasource;
pub mod evidence_chain;
pub mod governance;
pub mod handlers;
pub mod policy;

pub use datasource::{DataSource, DataSourceRegistry, DataSourceType};
pub use evidence_chain::{
    EvidenceChain, EvidenceError, EvidenceEvent, EvidenceEventType, EvidenceStore,
};
pub use policy::{AccessDecision, PolicyEngine, ResourcePolicy};

// pub // mod cost_attribution; // TODO: fix compilation // TODO: fix compilation
// pub // mod tenant_quota; // TODO: fix compilation // TODO: fix compilation
// pub // mod data_residency; // TODO: fix compilation // TODO: fix compilation
// pub // mod multi_tenant; // TODO: fix compilation // TODO: fix compilation
// pub // mod sla_product; // TODO: fix compilation // TODO: fix compilation
// pub // mod data_bridge_kafka; // TODO: fix compilation // TODO: fix compilation
// pub // mod data_bridge_db; // TODO: fix compilation // TODO: fix compilation
// pub // mod data_bridge_s3; // TODO: fix compilation // TODO: fix compilation
