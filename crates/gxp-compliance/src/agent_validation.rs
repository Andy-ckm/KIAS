//! # AI Agent Validation — GAMP 5 IQ / OQ / PQ
//!
//! Installation, Operational, and Performance qualification protocols for
//! AI agents in GxP-regulated environments per GAMP 5 guidelines.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use super::electronic_signature::ElectronicSignature;

/// Validation protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    /// Installation Qualification: system installed correctly
    IQ,
    /// Operational Qualification: system operates as specified
    OOQ,
    /// Performance Qualification: system performs consistently
    PQ,
    /// V-Model validation (waterfall)
    VModel,
    /// Agile continuous validation
    AgileValidation,
}

/// Validation stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStage {
    InstallationQualification,
    OperationalQualification,
    PerformanceQualification,
}

/// Protocol status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolStatus {
    Draft,
    InReview,
    Approved,
    Executed,
    Rejected,
    Archived,
}

/// Acceptance criterion for a validation test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    /// Human-readable description of the criterion
    pub description: String,
    /// How to test this criterion
    pub method: String,
    /// Expected outcome
    pub expected_outcome: String,
    /// Minimum pass threshold (e.g., 95% for PQ)
    pub pass_threshold: f64,
    /// Actual measured result (filled after execution)
    pub actual_result: Option<f64>,
    /// Whether criterion passed
    pub passed: Option<bool>,
}

impl AcceptanceCriterion {
    /// Create a new acceptance criterion.
    pub fn new(
        id: &str,
        description: &str,
        method: &str,
        expected_outcome: &str,
        pass_threshold: f64,
    ) -> Self {
        Self {
            id: id.to_string(),
            description: description.to_string(),
            method: method.to_string(),
            expected_outcome: expected_outcome.to_string(),
            pass_threshold,
            actual_result: None,
            passed: None,
        }
    }

    /// Record a test result and determine pass/fail.
    pub fn record_result(&mut self, actual: f64) {
        self.actual_result = Some(actual);
        self.passed = Some(actual >= self.pass_threshold);
    }

    /// Whether this criterion has been evaluated.
    pub fn is_evaluated(&self) -> bool {
        self.passed.is_some()
    }
}

/// A single test execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRecord {
    /// Unique test record ID
    pub id: String,
    /// Which criterion this tests
    pub criterion_id: String,
    /// Who executed the test
    pub tester_id: String,
    /// When executed
    pub executed_at: DateTime<Utc>,
    /// Test result as JSON
    pub result_json: serde_json::Value,
    /// Whether test passed
    pub passed: bool,
    /// Notes on any deviations
    pub deviation_notes: Option<String>,
}

impl TestRecord {
    /// Create a new test record.
    pub fn new(
        criterion_id: &str,
        tester_id: &str,
        result_json: serde_json::Value,
        passed: bool,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            criterion_id: criterion_id.to_string(),
            tester_id: tester_id.to_string(),
            executed_at: Utc::now(),
            result_json,
            passed,
            deviation_notes: None,
        }
    }

    /// Add deviation notes.
    pub fn with_deviation(mut self, notes: &str) -> Self {
        self.deviation_notes = Some(notes.to_string());
        self
    }
}

/// Complete validation protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationProtocol {
    /// Protocol ID
    pub id: String,
    /// Protocol type
    pub protocol_type: ProtocolType,
    /// Agent being validated
    pub agent_id: String,
    /// Scope of validation
    pub scope: String,
    /// All acceptance criteria
    pub criteria: Vec<AcceptanceCriterion>,
    /// All test execution records
    pub test_records: Vec<TestRecord>,
    /// Current status
    pub status: ProtocolStatus,
    /// When created
    pub created_at: DateTime<Utc>,
    /// Who approved the protocol
    pub approved_by: Option<String>,
    /// When approved
    pub approved_at: Option<DateTime<Utc>>,
    /// Who executed the tests
    pub executed_by: Option<String>,
    /// Who reviewed the results
    pub reviewed_by: Option<String>,
}

impl ValidationProtocol {
    /// Create a new validation protocol.
    pub fn new(protocol_type: ProtocolType, agent_id: &str, scope: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            protocol_type,
            agent_id: agent_id.to_string(),
            scope: scope.to_string(),
            criteria: Vec::new(),
            test_records: Vec::new(),
            status: ProtocolStatus::Draft,
            created_at: Utc::now(),
            approved_by: None,
            approved_at: None,
            executed_by: None,
            reviewed_by: None,
        }
    }

    /// Add an acceptance criterion.
    pub fn add_criterion(&mut self, criterion: AcceptanceCriterion) {
        self.criteria.push(criterion);
    }

    /// Approve the protocol (requires approver ID).
    pub fn approve(&mut self, approver_id: &str) -> Result<(), ValidationError> {
        if self.status != ProtocolStatus::Draft && self.status != ProtocolStatus::InReview {
            return Err(ValidationError::InvalidStateTransition(
                format!("Cannot approve protocol in {:?} state", self.status),
            ));
        }
        if self.criteria.is_empty() {
            return Err(ValidationError::NoCriteria);
        }
        self.status = ProtocolStatus::Approved;
        self.approved_by = Some(approver_id.to_string());
        let now = Utc::now();
        self.approved_at = Some(now);
        Ok(())
    }

    /// Execute a test record against a criterion.
    pub fn execute_test(&mut self, record: TestRecord) -> Result<(), ValidationError> {
        if self.status != ProtocolStatus::Approved {
            return Err(ValidationError::InvalidStateTransition(
                format!("Cannot execute test on protocol in {:?} state", self.status),
            ));
        }

        // Verify criterion exists
        let criterion_exists = self.criteria.iter().any(|c| c.id == record.criterion_id);
        if !criterion_exists {
            return Err(ValidationError::CriterionNotFound(record.criterion_id.clone()));
        }

        self.test_records.push(record);

        // Update criterion result
        if let Some(record) = self.test_records.last() {
            if let Some(criterion) = self.criteria.iter_mut().find(|c| c.id == record.criterion_id) {
                if let Some(result) = record.result_json.get("value").and_then(|v| v.as_f64()) {
                    criterion.record_result(result);
                }
            }
        }

        Ok(())
    }

    /// Complete the protocol after all tests executed.
    pub fn complete(&mut self, reviewer_id: &str) -> Result<(), ValidationError> {
        if self.status != ProtocolStatus::Approved {
            return Err(ValidationError::InvalidStateTransition(
                "Cannot complete protocol that is not approved".to_string(),
            ));
        }

        // Check all criteria are evaluated
        let all_evaluated = self.criteria.iter().all(|c| c.is_evaluated());
        if !all_evaluated {
            return Err(ValidationError::IncompleteCriteria);
        }

        self.reviewed_by = Some(reviewer_id.to_string());
        self.status = ProtocolStatus::Executed;
        Ok(())
    }

    /// Whether all acceptance criteria passed.
    pub fn is_validated(&self) -> bool {
        self.status == ProtocolStatus::Executed
            && !self.criteria.is_empty()
            && self.criteria.iter().all(|c| c.passed == Some(true))
    }

    /// Number of criteria passed.
    pub fn passed_count(&self) -> usize {
        self.criteria.iter().filter(|c| c.passed == Some(true)).count()
    }
}

/// Validation engine for managing protocols
pub struct ValidationEngine;

impl ValidationEngine {
    pub fn new() -> Self {
        Self
    }

    /// Create a new IQ/OQ/PQ protocol.
    pub fn create_protocol(
        &self,
        agent_id: &str,
        protocol_type: ProtocolType,
    ) -> ValidationProtocol {
        let scope = match protocol_type {
            ProtocolType::IQ => "Verify AI agent is installed correctly per specification",
            ProtocolType::OOQ => "Verify AI agent operates per functional specification",
            ProtocolType::PQ => "Verify AI agent performs consistently under production conditions",
            ProtocolType::VModel => "Full V-Model lifecycle validation",
            ProtocolType::AgileValidation => "Continuous agile validation approach",
        };
        ValidationProtocol::new(protocol_type, agent_id, scope)
    }

    /// Compute overall pass rate.
    pub fn calculate_pass_rate(&self, protocol: &ValidationProtocol) -> f64 {
        if protocol.criteria.is_empty() {
            return 0.0;
        }
        let passed = protocol.criteria.iter().filter(|c| c.passed == Some(true)).count();
        (passed as f64) / (protocol.criteria.len() as f64) * 100.0
    }

    /// Summary of validation status.
    pub fn status_summary(&self, protocol: &ValidationProtocol) -> ValidationStatusSummary {
        ValidationStatusSummary {
            protocol_id: protocol.id.clone(),
            status: protocol.status,
            total_criteria: protocol.criteria.len(),
            passed: protocol.passed_count(),
            failed: protocol.criteria.iter().filter(|c| c.passed == Some(false)).count(),
            pending: protocol.criteria.iter().filter(|c| c.passed.is_none()).count(),
            pass_rate: self.calculate_pass_rate(protocol),
            is_validated: protocol.is_validated(),
        }
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary snapshot of validation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStatusSummary {
    pub protocol_id: String,
    pub status: ProtocolStatus,
    pub total_criteria: usize,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    pub pass_rate: f64,
    pub is_validated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("protocol has no criteria")]
    NoCriteria,

    #[error("criterion {0} not found in protocol")]
    CriterionNotFound(String),

    #[error("not all criteria have been evaluated")]
    IncompleteCriteria,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_iq_protocol() {
        let engine = ValidationEngine::new();
        let protocol = engine.create_protocol("diagnostic-agent", ProtocolType::IQ);
        assert_eq!(protocol.protocol_type, ProtocolType::IQ);
        assert_eq!(protocol.status, ProtocolStatus::Draft);
        assert_eq!(protocol.agent_id, "diagnostic-agent");
    }

    #[test]
    fn test_add_criterion() {
        let engine = ValidationEngine::new();
        let mut protocol = engine.create_protocol("agent-1", ProtocolType::OOQ);
        protocol.add_criterion(AcceptanceCriterion::new(
            "C1",
            "Agent responds within 2 seconds",
            "Measure response time over 100 requests",
            "< 2000ms",
            95.0,
        ));
        assert_eq!(protocol.criteria.len(), 1);
    }

    #[test]
    fn test_approve_protocol() {
        let mut protocol = ValidationProtocol::new(
            ProtocolType::PQ,
            "agent-1",
            "Performance qualification",
        );
        protocol.add_criterion(AcceptanceCriterion::new(
            "C1", "Test", "Method", "Outcome", 90.0,
        ));
        protocol.add_criterion(AcceptanceCriterion::new(
            "C2", "Test 2", "Method 2", "Outcome 2", 85.0,
        ));

        protocol.approve("qa-manager").unwrap();
        assert_eq!(protocol.status, ProtocolStatus::Approved);
        assert_eq!(protocol.approved_by.as_deref(), Some("qa-manager"));
    }

    #[test]
    fn test_execute_test() {
        let mut protocol = ValidationProtocol::new(ProtocolType::PQ, "agent-1", "PQ");
        protocol.add_criterion(AcceptanceCriterion::new("C1", "Test", "Method", "Outcome", 90.0));
        protocol.approve("qa").unwrap();

        let record = TestRecord::new(
            "C1",
            "tester-1",
            serde_json::json!({"value": 95.0}),
            true,
        );
        protocol.execute_test(record).unwrap();
        assert_eq!(protocol.test_records.len(), 1);
    }

    #[test]
    fn test_pass_rate() {
        let engine = ValidationEngine::new();
        let mut protocol = ValidationProtocol::new(ProtocolType::PQ, "agent-1", "PQ");
        protocol.add_criterion(AcceptanceCriterion::new("C1", "T", "M", "O", 90.0));
        protocol.add_criterion(AcceptanceCriterion::new("C2", "T", "M", "O", 90.0));

        let mut c1 = protocol.criteria.get_mut(0).unwrap();
        c1.record_result(95.0);
        let mut c2 = protocol.criteria.get_mut(1).unwrap();
        c2.record_result(85.0);

        assert_eq!(engine.calculate_pass_rate(&protocol), 50.0);
    }

    #[test]
    fn test_validation_status() {
        let engine = ValidationEngine::new();
        let mut protocol = ValidationProtocol::new(ProtocolType::IQ, "agent-1", "IQ");
        protocol.add_criterion(AcceptanceCriterion::new("C1", "T", "M", "O", 90.0));
        protocol.approve("qa").unwrap();

        let mut c = protocol.criteria.get_mut(0).unwrap();
        c.record_result(100.0);
        protocol.test_records.push(TestRecord::new("C1", "t", serde_json::json!({"value": 100.0}), true));

        protocol.complete("reviewer").unwrap();
        assert!(protocol.is_validated());

        let summary = engine.status_summary(&protocol);
        assert_eq!(summary.total_criteria, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.is_validated, true);
    }

    #[test]
    fn test_cannot_execute_unapproved_protocol() {
        let mut protocol = ValidationProtocol::new(ProtocolType::OOQ, "agent-1", "OOQ");
        protocol.add_criterion(AcceptanceCriterion::new("C1", "T", "M", "O", 90.0));
        let record = TestRecord::new("C1", "t", serde_json::json!({}), true);
        let err = protocol.execute_test(record).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidStateTransition(_)));
    }
}
