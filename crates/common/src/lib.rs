//! # KIAS Common Library
//!
//! Shared utilities for the KIAS (Kubernetes-like Intelligent Agent Scheduling) system.
//! This crate provides foundational types used across all KIAS components:
//!
//! - **error** – Unified error type (`KiasError`)
//! - **types** – Core domain types (`Agent`, `Node`, `Resources`, `ScheduleResult`, …)
//! - **config** – Configuration loading and structures (`KiasConfig`)
//! - **logging** – Tracing initialisation helpers
//! - **metrics** – Prometheus metric definitions
//! - **utils** – Hashing, time, and general-purpose helpers

pub mod a2a;
pub mod audit;
pub mod config;
pub mod data_mask;
pub mod error;
pub mod graceful_shutdown;
pub mod gxp_audit;
pub mod gxp_auth;
pub mod hot_config;
pub mod logging;
pub mod messaging;
pub mod minimax_client;
pub mod metrics;
pub mod tls;
pub mod types;
pub mod unified_namespace;
pub mod utils;
pub mod vector;
pub mod vfs;
pub mod vq_codebook;

// Re-export the most commonly used items at crate root for convenience.
pub use config::KiasConfig;
pub use error::KiasError;
pub use types::*;

/// Result alias pinned to [`KiasError`].
pub type KiasResult<T> = Result<T, KiasError>;
