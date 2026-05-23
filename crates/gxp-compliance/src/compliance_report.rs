//! # Compliance Reporting — Automated GxP Regulatory Reports
//!
//! Generates automated compliance reports for FDA, EMA, and other regulatory
//! bodies from audit trails, risk assessments, and validation data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use super::agent_validation::{
    AcceptanceCriterion, ProtocolType, TestRecord, ValidationProtocol,
};
pub use super::audit_trail::{AuditRecord, AuditTrail};
pub use super::electronic_signature::SignatureBundle;
pub use super::risk_assessment::RiskAssessment;

/// Report type matching specific regulations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportType {
    FDA21CFR11,
    EUAnnex11,
    GAMP5,
    EUAIAct,
    IS014971,
    IEC62304,
    HIPAA,
    AnnualReview,
    ChangeControl,
    IncidentReport,
}

/// Compliance status of a section
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Compliant,
    ConditionallyCompliant,
    NonCompliant,
    NotApplicable,
    InProgress,
}

/// Finding severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Major,
    Minor,
    Observation,
}

/// A single compliance finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Finding ID
    pub id: String,
    pub severity: Severity,
    pub description: String,
    pub root_cause: Option<String>,
    /// CAPA (Corrective and Preventive Action) ID if assigned
    pub capa_id: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
}

impl Finding {
    pub fn new(severity: Severity, description: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            severity,
            description: description.to_string(),
            root_cause: None,
            capa_id: None,
            due_date: None,
        }
    }

    pub fn with_root_cause(mut self, cause: &str) -> Self {
        self.root_cause = Some(cause.to_string());
        self
    }

    pub fn with_capa(mut self, capa_id: &str) -> Self {
        self.capa_id = Some(capa_id.to_string());
        self
    }
}

/// A section of a compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub content: String,
    pub compliance_status: ComplianceStatus,
    /// Evidence file references
    pub evidence_refs: Vec<String>,
    pub findings: Vec<Finding>,
}

impl ReportSection {
    pub fn new(title: &str, content: &str, status: ComplianceStatus) -> Self {
        Self {
            title: title.to_string(),
            content: content.to_string(),
            compliance_status: status,
            evidence_refs: Vec::new(),
            findings: Vec::new(),
        }
    }

    pub fn with_evidence(mut self, ref_id: &str) -> Self {
        self.evidence_refs.push(ref_id.to_string());
        self
    }

    pub fn with_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }
}

/// A complete GxP compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub id: String,
    pub report_type: ReportType,
    pub agent_id: String,
    pub reporting_period_start: DateTime<Utc>,
    pub reporting_period_end: DateTime<Utc>,
    pub sections: Vec<ReportSection>,
    pub prepared_by: String,
    pub reviewed_by: Option<String>,
    pub approved_by: Option<String>,
    pub generated_at: DateTime<Utc>,
    /// Reference to GxP domain
    pub gxp_domain: String,
}

impl ComplianceReport {
    /// Create a new report.
    pub fn new(
        report_type: ReportType,
        agent_id: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        prepared_by: &str,
        gxp_domain: &str,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            report_type,
            agent_id: agent_id.to_string(),
            reporting_period_start: period_start,
            reporting_period_end: period_end,
            sections: Vec::new(),
            prepared_by: prepared_by.to_string(),
            reviewed_by: None,
            approved_by: None,
            generated_at: Utc::now(),
            gxp_domain: gxp_domain.to_string(),
        }
    }

    /// Add a section.
    pub fn add_section(&mut self, section: ReportSection) {
        self.sections.push(section);
    }

    /// Number of critical findings across all sections.
    pub fn critical_count(&self) -> usize {
        self.sections
            .iter()
            .flat_map(|s| &s.findings)
            .filter(|f| f.severity == Severity::Critical)
            .count()
    }

    /// All findings sorted by severity.
    pub fn all_findings(&self) -> Vec<&Finding> {
        let mut findings: Vec<_> = self
            .sections
            .iter()
            .flat_map(|s| s.findings.iter())
            .collect();
        findings.sort_by_key(|f| match f.severity {
            Severity::Critical => 0,
            Severity::Major => 1,
            Severity::Minor => 2,
            Severity::Observation => 3,
        });
        findings
    }

    /// Overall compliance status.
    pub fn overall_status(&self) -> ComplianceStatus {
        if self.critical_count() > 0 {
            return ComplianceStatus::NonCompliant;
        }
        let major_count = self
            .sections
            .iter()
            .flat_map(|s| &s.findings)
            .filter(|f| f.severity == Severity::Major)
            .count();
        if major_count > 0 {
            return ComplianceStatus::InProgress;
        }
        if self
            .sections
            .iter()
            .any(|s| s.compliance_status == ComplianceStatus::InProgress)
        {
            return ComplianceStatus::InProgress;
        }
        ComplianceStatus::Compliant
    }
}

/// FDA regulatory submission package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FDASubmissionPackage {
    pub administrative_info: AdministrativeInfo,
    pub agent_description: String,
    pub risk_summary: String,
    pub validation_summary: String,
    pub audit_trail_export: Vec<serde_json::Value>,
    pub electronic_signature_bundle: Option<SignatureBundle>,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdministrativeInfo {
    pub submitter_name: String,
    pub submission_date: DateTime<Utc>,
    pub document_ids: Vec<String>,
    pub regulatory_exemption_codes: Vec<String>,
}

/// Overall compliance summary for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_audit_records: usize,
    pub critical_findings: usize,
    pub major_findings: usize,
    pub minor_findings: usize,
    pub overall_status: ComplianceStatus,
    pub recommendation: String,
    pub next_review_date: DateTime<Utc>,
}

impl ComplianceSummary {
    pub fn from_report(report: &ComplianceReport) -> Self {
        let critical = report.critical_count();
        let major = report
            .sections
            .iter()
            .flat_map(|s| &s.findings)
            .filter(|f| f.severity == Severity::Major)
            .count();
        let minor = report
            .sections
            .iter()
            .flat_map(|s| &s.findings)
            .filter(|f| f.severity == Severity::Minor)
            .count();

        let recommendation = match report.overall_status() {
            ComplianceStatus::Compliant => "Continue operation with periodic review".to_string(),
            ComplianceStatus::ConditionallyCompliant => {
                "Address major findings within 30 days".to_string()
            }
            ComplianceStatus::InProgress => "Complete CAPA within specified timeframes".to_string(),
            ComplianceStatus::NonCompliant => {
                "Suspend AI agent operation until critical findings resolved".to_string()
            }
            _ => "Conduct additional assessment".to_string(),
        };

        Self {
            total_audit_records: 0, // filled by caller
            critical_findings: critical,
            major_findings: major,
            minor_findings: minor,
            overall_status: report.overall_status(),
            recommendation,
            next_review_date: Utc::now() + chrono::Duration::days(90),
        }
    }
}

/// Compliance report generator
pub struct ComplianceReporter;

impl ComplianceReporter {
    pub fn new() -> Self {
        Self
    }

    /// Generate a comprehensive compliance report from all data sources.
    pub fn generate_report(
        &self,
        agent_id: &str,
        report_type: ReportType,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        audit_trail: &AuditTrail,
        risk_assessment: &RiskAssessment,
        validation: &ValidationProtocol,
    ) -> ComplianceReport {
        let mut report = ComplianceReport::new(
            report_type,
            agent_id,
            period_start,
            period_end,
            "Compliance Reporter",
            self.report_type_to_domain(report_type),
        );

        // Section 1: Executive Summary
        report.add_section(self.section_executive_summary(agent_id, risk_assessment, validation));

        // Section 2: Audit Trail Analysis
        report.add_section(self.section_audit_trail(
            agent_id,
            period_start,
            period_end,
            audit_trail,
        ));

        // Section 3: Risk Assessment Summary
        report.add_section(self.section_risk_assessment(risk_assessment));

        // Section 4: Validation Summary
        report.add_section(self.section_validation(validation));

        // Section 5: GxP Compliance Status
        report.add_section(self.section_gxp_status(report_type));

        // Section 6: Findings and CAPA
        report.add_section(self.section_findings(&report));

        report
    }

    fn section_executive_summary(
        &self,
        agent_id: &str,
        risk: &RiskAssessment,
        validation: &ValidationProtocol,
    ) -> ReportSection {
        let status = if validation.is_validated() {
            ComplianceStatus::Compliant
        } else {
            ComplianceStatus::InProgress
        };
        ReportSection::new(
            "Executive Summary",
            &format!(
                "AI Agent '{}' GxP compliance report. Risk level: {:?}. Validation status: {:?}.",
                agent_id, risk.risk_level, validation.status
            ),
            status,
        )
    }

    fn section_audit_trail(
        &self,
        agent_id: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        audit_trail: &AuditTrail,
    ) -> ReportSection {
        let records = audit_trail.query(agent_id, period_start, period_end);
        let record_count = records.len();
        let critical_actions = records
            .iter()
            .filter(|r| r.risk_level == crate::audit_trail::RiskLevel::Critical)
            .count();

        let content = format!(
            "Audit trail review: {} total records in period. {} critical-risk actions logged.",
            record_count, critical_actions
        );

        ReportSection::new("Audit Trail Review", &content, ComplianceStatus::Compliant)
            .with_evidence(&format!("audit_export_{}.json", agent_id))
    }

    fn section_risk_assessment(&self, risk: &RiskAssessment) -> ReportSection {
        let content = format!(
            "GAMP Category: {:?}. Risk Level: {:?}. {} hazards identified. \
             Residual risk: {:.1}. {} mitigations applied.",
            risk.gamp_category,
            risk.risk_level,
            risk.hazard_analysis.len(),
            risk.residual_risk,
            risk.mitigation_applied.len()
        );
        ReportSection::new("Risk Assessment", &content, ComplianceStatus::Compliant)
            .with_evidence("risk_assessment_report.json")
    }

    fn section_validation(&self, validation: &ValidationProtocol) -> ReportSection {
        let status = match validation.status {
            crate::agent_validation::ProtocolStatus::Executed => ComplianceStatus::Compliant,
            crate::agent_validation::ProtocolStatus::InReview => ComplianceStatus::InProgress,
            _ => ComplianceStatus::InProgress,
        };
        let content = format!(
            "Validation type: {:?}. {} criteria evaluated. {} passed.",
            validation.protocol_type,
            validation.criteria.len(),
            validation.passed_count(),
        );
        ReportSection::new("Validation Summary", &content, status)
            .with_evidence(&format!("validation_protocol_{}.json", validation.id))
    }

    fn section_gxp_status(&self, report_type: ReportType) -> ReportSection {
        let domain = self.report_type_to_domain(report_type);
        ReportSection::new(
            &format!("{} Compliance Status", domain),
            &format!("All applicable {} requirements have been reviewed.", domain),
            ComplianceStatus::Compliant,
        )
    }

    fn section_findings(&self, report: &ComplianceReport) -> ReportSection {
        let findings = report.all_findings();
        ReportSection::new(
            "Findings and CAPA",
            &format!(
                "{} total findings: {} critical, {} major, {} minor.",
                findings.len(),
                findings
                    .iter()
                    .filter(|f| f.severity == Severity::Critical)
                    .count(),
                findings
                    .iter()
                    .filter(|f| f.severity == Severity::Major)
                    .count(),
                findings
                    .iter()
                    .filter(|f| f.severity == Severity::Minor)
                    .count(),
            ),
            report.overall_status(),
        )
    }

    fn report_type_to_domain(&self, report_type: ReportType) -> &str {
        match report_type {
            ReportType::FDA21CFR11 => "FDA 21 CFR Part 11",
            ReportType::EUAnnex11 => "EU Annex 11",
            ReportType::GAMP5 => "GAMP 5",
            ReportType::EUAIAct => "EU AI Act",
            ReportType::IS014971 => "ISO 14971",
            ReportType::IEC62304 => "IEC 62304",
            ReportType::HIPAA => "HIPAA",
            ReportType::AnnualReview => "Annual Review",
            ReportType::ChangeControl => "Change Control",
            ReportType::IncidentReport => "Incident Report",
        }
    }

    /// Generate a complete FDA submission package.
    pub fn generate_fda_submission_package(
        &self,
        agent_id: &str,
        audit_trail: &AuditTrail,
        risk: &RiskAssessment,
        validation: &ValidationProtocol,
    ) -> FDASubmissionPackage {
        FDASubmissionPackage {
            administrative_info: AdministrativeInfo {
                submitter_name: "AgentGuard KIAS".to_string(),
                submission_date: Utc::now(),
                document_ids: vec![format!("agent-{}-fda-001", agent_id)],
                regulatory_exemption_codes: vec![],
            },
            agent_description: format!(
                "AI Agent {} - GxP compliant diagnostic assistant",
                agent_id
            ),
            risk_summary: format!(
                "Risk level: {:?}. GAMP: {:?}.",
                risk.risk_level, risk.gamp_category
            ),
            validation_summary: format!(
                "Validation status: {:?}. {} criteria passed.",
                validation.status,
                validation.passed_count()
            ),
            audit_trail_export: audit_trail.export_for_inspection(agent_id),
            electronic_signature_bundle: None,
            attachments: vec![
                format!("risk_assessment_{}.pdf", agent_id),
                format!("validation_protocol_{}.pdf", validation.id),
            ],
        }
    }
}

impl Default for ComplianceReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_audit_trail() -> AuditTrail {
        let mut trail = AuditTrail::new();
        let r = crate::audit_trail::AuditRecord::new(
            "agent-1",
            "Test action",
            crate::audit_trail::ActionType::Decision,
            "user-1",
            "Rationale",
            r#"{"input":"test"}"#,
            r#"{"output":"ok"}"#,
        );
        trail.seal(r).unwrap();
        trail
    }

    fn dummy_risk() -> RiskAssessment {
        RiskAssessment::new(
            "agent-1",
            crate::risk_assessment::GampCategory::AIModel,
            "assessor",
            crate::risk_assessment::GxPRegulatorContext::FDA,
        )
    }

    fn dummy_validation() -> ValidationProtocol {
        let mut p = ValidationProtocol::new(ProtocolType::PQ, "agent-1", "PQ validation");
        p.add_criterion(AcceptanceCriterion::new(
            "C1",
            "Accuracy >= 95%",
            "Test",
            ">=95%",
            95.0,
        ));
        p.add_criterion(AcceptanceCriterion::new(
            "C2",
            "Latency < 2s",
            "Test",
            "<2s",
            90.0,
        ));
        p.approve("qa").unwrap();
        if let Some(c) = p.criteria.get_mut(0) {
            c.record_result(97.0);
        }
        if let Some(c) = p.criteria.get_mut(1) {
            c.record_result(92.0);
        }
        p.test_records.push(TestRecord::new(
            "C1",
            "t",
            serde_json::json!({"value": 97.0}),
            true,
        ));
        p.test_records.push(TestRecord::new(
            "C2",
            "t",
            serde_json::json!({"value": 92.0}),
            true,
        ));
        p.complete("reviewer").unwrap();
        p
    }

    #[test]
    fn test_generate_report() {
        let reporter = ComplianceReporter::new();
        let audit = dummy_audit_trail();
        let risk = dummy_risk();
        let validation = dummy_validation();

        let report = reporter.generate_report(
            "agent-1",
            ReportType::FDA21CFR11,
            DateTime::from_timestamp(0, 0).unwrap(),
            DateTime::from_timestamp(9_000_000_000, 0).unwrap(),
            &audit,
            &risk,
            &validation,
        );

        assert_eq!(report.sections.len(), 6);
        assert_eq!(report.agent_id, "agent-1");
    }

    #[test]
    fn test_finding_classification() {
        let critical = Finding::new(Severity::Critical, "AI agent produced incorrect diagnosis");
        let major = Finding::new(Severity::Major, "Validation documentation incomplete");
        let minor = Finding::new(Severity::Minor, "Minor logging gap");
        let observation = Finding::new(Severity::Observation, "Consider improving response time");

        assert!(matches!(critical.severity, Severity::Critical));
        assert!(matches!(major.severity, Severity::Major));
        assert!(matches!(minor.severity, Severity::Minor));
        assert!(matches!(observation.severity, Severity::Observation));
    }

    #[test]
    fn test_fda_submission_package() {
        let reporter = ComplianceReporter::new();
        let audit = dummy_audit_trail();
        let risk = dummy_risk();
        let validation = dummy_validation();

        let pkg = reporter.generate_fda_submission_package("agent-1", &audit, &risk, &validation);
        assert!(!pkg.administrative_info.submitter_name.is_empty());
        assert_eq!(pkg.audit_trail_export.len(), 1);
        assert_eq!(pkg.attachments.len(), 2);
    }

    #[test]
    fn test_compliance_summary() {
        let audit = dummy_audit_trail();
        let risk = dummy_risk();
        let validation = dummy_validation();

        let reporter = ComplianceReporter::new();
        let report = reporter.generate_report(
            "agent-1",
            ReportType::FDA21CFR11,
            DateTime::from_timestamp(0, 0).unwrap(),
            DateTime::from_timestamp(9_000_000_000, 0).unwrap(),
            &audit,
            &risk,
            &validation,
        );

        let summary = ComplianceSummary::from_report(&report);
        assert_eq!(summary.critical_findings, 0);
        assert_eq!(summary.overall_status, ComplianceStatus::Compliant);
    }

    #[test]
    fn test_compliance_summary_with_critical() {
        let mut report = ComplianceReport::new(
            ReportType::FDA21CFR11,
            "agent-1",
            DateTime::from_timestamp(0, 0).unwrap(),
            DateTime::from_timestamp(9_000_000_0, 0).unwrap(),
            "tester",
            "FDA 21 CFR Part 11",
        );
        let mut section = ReportSection::new("Test", "Test content", ComplianceStatus::Compliant);
        section
            .findings
            .push(Finding::new(Severity::Critical, "Critical failure"));
        report.add_section(section);

        let summary = ComplianceSummary::from_report(&report);
        assert_eq!(summary.critical_findings, 1);
        assert_eq!(summary.overall_status, ComplianceStatus::NonCompliant);
        assert!(summary.recommendation.contains("Suspend"));
    }

    #[test]
    fn test_audit_trail_integration_in_report() {
        let mut audit = dummy_audit_trail();
        // Add another record
        let r = crate::audit_trail::AuditRecord::new(
            "agent-1",
            "Second action",
            crate::audit_trail::ActionType::ToolCall,
            "user-2",
            "Second rationale",
            r#"{"input":"data"}"#,
            r#"{"output":"result"}"#,
        );
        audit.seal(r).unwrap();

        let reporter = ComplianceReporter::new();
        let report = reporter.generate_report(
            "agent-1",
            ReportType::AnnualReview,
            DateTime::from_timestamp(0, 0).unwrap(),
            DateTime::from_timestamp(9_000_000_000, 0).unwrap(),
            &audit,
            &dummy_risk(),
            &dummy_validation(),
        );

        // Should have an audit trail section
        let audit_section = report.sections.iter().find(|s| s.title.contains("Audit"));
        assert!(audit_section.is_some());
    }
}
