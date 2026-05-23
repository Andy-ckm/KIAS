//! # KIAS GxP Compliance Module
//!
//! Comprehensive GxP (Good Practice) compliance framework for AI agents in
//! medical/pharmaceutical environments.
//!
//! ## Supported Regulations
//!
//! - **FDA 21 CFR Part 11**: Electronic Records and Electronic Signatures
//! - **EU Annex 11**: Computerised Systems in Pharmaceutical Quality Systems
//! - **GAMP 5**: Good Automated Manufacturing Practice (AI/ML specific)
//! - **EU AI Act**: Risk-based AI system classification
//! - **ISO 14971**: Risk Management for Medical Devices
//! - **IEC 62304**: Software Lifecycle Processes for Medical Device Software
//! - **HIPAA**: Health Insurance Portability and Accountability Act
//!
//! ## Core Components
//!
//! - [`audit_trail`] — Immutable, hash-chained audit trail for 21 CFR Part 11
//! - [`electronic_signature`] — Non-repudiable electronic signatures
//! - [`risk_assessment`] — GAMP 5 / ISO 14971 risk assessment for AI agents
//! - [`agent_validation`] — IQ / OQ / PQ validation protocols per GAMP 5
//! - [`compliance_report`] — Automated regulatory compliance reports
//! - [`gamp_classification`] — GAMP 5 category classification for AI systems

pub mod agent_validation;
pub mod audit_trail;
pub mod compliance_report;
pub mod electronic_signature;
pub mod gamp_classification;
pub mod risk_assessment;

pub use agent_validation::{
    AcceptanceCriterion, ProtocolType, TestRecord, ValidationEngine, ValidationProtocol,
    ValidationStage,
};
pub use audit_trail::{
    ActionType, AuditRecord, AuditTrail, CorrelationId, GxPDomain, MerkleProof, MerkleTree,
    RiskLevel, WormStore, WormStoreError,
};
pub use compliance_report::{
    ComplianceReport, ComplianceReporter, ComplianceStatus, ComplianceSummary,
    FDASubmissionPackage, Finding, ReportSection, ReportType, Severity,
};
pub use electronic_signature::{
    ElectronicSignature, OperationType, SignatureBundle, SignatureManager, SignatureType,
};
pub use gamp_classification::{
    AIType, DataDependency, GampAIProfile, GampCategory, GampClassifier, HumanOversightLevel,
    RegulatoryRelevance, SOPRequirement, ValidationStage as GampValidationStage,
};
pub use risk_assessment::{
    AIAgentRiskLevel, GxPRegulatorContext, HazardScenario, RiskAssessment, RiskScorer,
};
