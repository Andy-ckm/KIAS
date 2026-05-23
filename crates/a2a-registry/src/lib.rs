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

mod error;
mod registry;
mod types;
pub mod a2a_enhanced;

pub use error::RegistryError;
pub use registry::AgentRegistry;
pub use types::*;
