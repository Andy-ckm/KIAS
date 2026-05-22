//! # Compliance Security Crate
//!
//! Enterprise-grade security and compliance module for AgentGuard.
//! Exceeds EMQ's 11-auth/TLS/RBAC/ACL security model with:
//!
//! 1. **Multi-Auth Providers** — LDAP, JWT, OAuth2.0, SCRAM, API-Key, mTLS cert auth
//! 2. **Zero-Trust Architecture** — Continuous verification, mTLS enforcement, least-privilege
//! 3. **Prompt Injection Defense** — Pattern-based + structural detection for LLM inputs
//! 4. **Agent Sandbox Isolation** — Resource limits, filesystem/network policies, syscall filtering
//! 5. **Digital Signature PKI** — X.509 CA chain, signing/verification, certificate lifecycle
//! 6. **EU AI Act Compliance** — Risk classification, transparency obligations, conformity checks

pub mod auth_providers;
pub mod bias_detection;
pub mod compliance_report;
// pub // mod data_masking; // TODO: fix compilation // TODO: fix compilation
pub mod error;
pub mod eu_ai_act;
pub mod gxp_audit;
pub mod pki;
pub mod policy_as_code;
pub mod policy_simulator;
pub mod prompt_defense;
pub mod red_team;
pub mod sandbox;
pub mod sandbox_enforcer;
pub mod secrets_manager;
// pub // mod security_drill; // TODO: fix compilation // TODO: fix compilation
// pub // mod supply_chain; // TODO: fix compilation // TODO: fix compilation
// pub // mod runtime_protection; // TODO: fix compilation // TODO: fix compilation
pub mod audit_console;
pub mod behavior_risk;
pub mod change_audit;
pub mod compliance_as_service;
pub mod whitepaper_data;
pub mod zero_trust;

// Re-export key types for convenience
pub use auth_providers::{
    AuthCredential, AuthProvider, AuthProviderType, AuthResult, MultiAuthProvider,
};
pub use bias_detection::{BiasDetector, BiasReport};
pub use compliance_report::{ComplianceReport, ComplianceReportGenerator, ReportType};
pub use eu_ai_act::{AiActChecker, AiSystem, ConformityReport, RiskLevel};
pub use pki::{Certificate, DistinguishedName, KeyPair, PkiManager, SignatureAlgorithm};
pub use prompt_defense::{InjectionDetector, InjectionSeverity, PromptAnalysis};
pub use sandbox::{ResourceLimits, SandboxConfig, SandboxManager};
pub use zero_trust::{TrustDecision, TrustScore, ZeroTrustEngine};
