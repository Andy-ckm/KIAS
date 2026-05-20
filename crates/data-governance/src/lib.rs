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
pub mod datasource;
pub mod governance;
pub mod handlers;
pub mod policy;

pub use datasource::{DataSource, DataSourceRegistry, DataSourceType};
pub use policy::{AccessDecision, PolicyEngine, ResourcePolicy};
