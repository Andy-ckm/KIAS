//! A2A Agent Registry
//!
//! Provides Agent registration, discovery, and lifecycle management
//! following the A2A (Agent-to-Agent) protocol pattern.
//!
//! Key capabilities:
//! - Agent Card registration with Schema validation
//! - Real-time discovery (event-driven, no polling)
//! - Online/offline/lwt status tracking
//! - Governance: every registration/discovery is audited

mod types;
mod registry;
mod error;

pub use types::*;
pub use registry::AgentRegistry;
pub use error::RegistryError;
