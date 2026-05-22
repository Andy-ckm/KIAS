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
pub mod decision_record;
pub mod error;
pub mod fault_injection;
pub mod graceful_shutdown;
pub mod gxp_audit;
pub mod gxp_auth;
pub mod hot_config;
pub mod idempotency;
pub mod logging;
pub mod messaging;
pub mod metrics;
pub mod minimax_client;
pub mod resilience;
pub mod tls;
pub mod tracing;
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

pub mod change_impact;
pub mod circuit_breaker;
pub mod contract_test;
pub mod dependency_checker;
pub mod plugin_framework;
pub mod quality_scorer;
pub mod sandbox_config;
pub mod sdk_protocol;
// pub // mod concurrency_control; // TODO: fix compilation // TODO: fix compilation
pub mod consistency_matrix;
pub mod disaster_recovery;
pub mod manifest;
pub mod schema_validation;
