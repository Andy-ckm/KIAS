//! # Compliance as a Service - Automated Audit Report Generation
//!
//! Implements automated compliance report generation for multiple regulatory frameworks
//! including 21CFR Part 11, EU Annex 11, GAMP 5, EU AI Act, and ALCOA+ data integrity.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Report type for different regulatory frameworks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportType {
    /// FDA 21 CFR Part 11 - Electronic Records
    CFR21Part11,
    /// EU Annex 11 - Computerized Systems
    EUAnnex11,
    /// GAMP 5 - Good Automated Manufacturing Practice
    GAMP5,
    /// EU AI Act - Artificial Intelligence Act
    EUAIAct,
    /// ALCOA+ - Data Integrity Principles
    ALCOAPlus,
}

impl ReportType {
    pub fn name(&self) -> &'static str {
        match self {
            ReportType::CFR21Part11 => "21 CFR Part 11",
            ReportType::EUAnnex11 => "EU Annex 11",
            ReportType::GAMP5 => "GAMP 5",
            ReportType::EUAIAct => "EU AI Act",
            ReportType::ALCOAPlus => "ALCOA+",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ReportType::CFR21Part11 => "FDA regulation for electronic records and signatures",
            ReportType::EUAnnex11 => "EU GMP guidance for computerized systems",
            ReportType::GAMP5 => "Good Automated Manufacturing Practice guidelines",
            ReportType::EUAIAct => "EU regulation on artificial intelligence",
            ReportType::ALCOAPlus => {
                "Data integrity: Attributable, Legible, Contemporaneous, Original, Accurate+"
            }
        }
    }

    pub fn required_sections(&self) -> Vec<&'static str> {
        match self {
            ReportType::CFR21Part11 => vec![
                "System Description",
                "Validation Documentation",
                "Audit Trail",
                "Access Controls",
                "Data Integrity",
                "Electronic Signatures",
                "Security Measures",
            ],
            ReportType::EUAnnex11 => vec![
                "Risk Assessment",
                "System Validation",
                "Validation Documentation",
                "Data Integrity",
                "Audit Trail",
                "Change Control",
                "Security",
                "Incident Management",
            ],
            ReportType::GAMP5 => vec![
                "Category Assessment",
                "Risk Assessment",
                "Validation Planning",
                "Specification",
                "Verification",
                "Release",
            ],
            ReportType::EUAIAct => vec![
                "AI System Description",
                "Risk Classification",
                "Data Governance",
                "Transparency Measures",
                "Human Oversight",
                "Accuracy Robustness",
                "Conformity Assessment",
            ],
            ReportType::ALCOAPlus => vec![
                "Attributable",
                "Legible",
                "Contemporaneous",
                "Original",
                "Accurate",
                "Complete",
                "Consistent",
                "Enduring",
                "Available",
            ],
        }
    }
}

/// Compliance service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceServiceConfig {
    pub organization: String,
    pub system_name: String,
    pub system_version: String,
    pub assessment_date: DateTime<Utc>,
    pub assessor: String,
}

impl ComplianceServiceConfig {
    pub fn new(
        organization: &str,
        system_name: &str,
        system_version: &str,
        assessor: &str,
    ) -> Self {
        Self {
            organization: organization.to_string(),
            system_name: system_name.to_string(),
            system_version: system_version.to_string(),
            assessment_date: Utc::now(),
            assessor: assessor.to_string(),
        }
    }
}

/// A compliance finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFinding {
    pub requirement: String,
    pub status: FindingStatus,
    pub evidence: Vec<String>,
    pub gaps: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingStatus {
    Compliant,
    PartiallyCompliant,
    NonCompliant,
    NotApplicable,
}

impl FindingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingStatus::Compliant => "Compliant",
            FindingStatus::PartiallyCompliant => "Partially Compliant",
            FindingStatus::NonCompliant => "Non-Compliant",
            FindingStatus::NotApplicable => "Not Applicable",
        }
    }
}

/// Generated compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub config: ComplianceServiceConfig,
    pub report_type: ReportType,
    pub generated_at: DateTime<Utc>,
    pub findings: Vec<ComplianceFinding>,
    pub summary: ReportSummary,
    pub approval_info: Option<ApprovalInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_requirements: usize,
    pub compliant_count: usize,
    pub partially_compliant_count: usize,
    pub non_compliant_count: usize,
    pub not_applicable_count: usize,
    pub overall_status: OverallStatus,
    pub compliance_percentage: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverallStatus {
    FullCompliance,
    AcceptableCompliance,
    NeedsImprovement,
    CriticalNonCompliance,
}

impl OverallStatus {
    pub fn from_percentage(pct: f64) -> Self {
        if pct >= 95.0 {
            OverallStatus::FullCompliance
        } else if pct >= 80.0 {
            OverallStatus::AcceptableCompliance
        } else if pct >= 50.0 {
            OverallStatus::NeedsImprovement
        } else {
            OverallStatus::CriticalNonCompliance
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalInfo {
    pub approved_by: String,
    pub approval_date: DateTime<Utc>,
    pub signature: String,
}

/// Compliance service for generating reports
#[derive(Debug, Clone)]
pub struct ComplianceService {
    config: ComplianceServiceConfig,
}

impl ComplianceService {
    pub fn new(config: ComplianceServiceConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults(organization: &str, system_name: &str) -> Self {
        Self {
            config: ComplianceServiceConfig::new(organization, system_name, "1.0.0", "System"),
        }
    }

    /// Generate a compliance report for the given report type
    pub fn generate_report(&self, report_type: ReportType) -> ComplianceReport {
        let findings = self.generate_findings(report_type);
        let summary = self.calculate_summary(&findings);

        ComplianceReport {
            config: self.config.clone(),
            report_type,
            generated_at: Utc::now(),
            findings,
            summary,
            approval_info: None,
        }
    }

    /// Generate findings for each requirement
    fn generate_findings(&self, report_type: ReportType) -> Vec<ComplianceFinding> {
        let requirements = self.get_requirements(report_type);
        requirements
            .into_iter()
            .map(|req| self.create_finding_for_requirement(&req, report_type))
            .collect()
    }

    /// Get requirements based on report type
    fn get_requirements(&self, report_type: ReportType) -> Vec<String> {
        match report_type {
            ReportType::CFR21Part11 => vec![
                "Audit Trail".to_string(),
                "Electronic Signatures".to_string(),
                "Access Controls".to_string(),
                "Data Validation".to_string(),
                "Authority Checks".to_string(),
                "Completeness Checks".to_string(),
                "Input Validation".to_string(),
                "Deviation Alerts".to_string(),
                "Change Documentation".to_string(),
            ],
            ReportType::EUAnnex11 => vec![
                "Risk Assessment".to_string(),
                "Validation Documentation".to_string(),
                "Data Integrity Controls".to_string(),
                "Audit Trail".to_string(),
                "Change Control".to_string(),
                "Security Measures".to_string(),
                "Incident Management".to_string(),
                "Business Continuity".to_string(),
            ],
            ReportType::GAMP5 => vec![
                "User Requirements".to_string(),
                "Functional Specification".to_string(),
                "Design Specification".to_string(),
                "Code Review".to_string(),
                "Unit Testing".to_string(),
                "Integration Testing".to_string(),
                "IQ/OQ/PQ".to_string(),
            ],
            ReportType::EUAIAct => vec![
                "Risk Classification".to_string(),
                "Data Governance".to_string(),
                "Technical Documentation".to_string(),
                "Transparency".to_string(),
                "Human Oversight".to_string(),
                "Accuracy Metrics".to_string(),
                "Robustness Metrics".to_string(),
            ],
            ReportType::ALCOAPlus => vec![
                "Attributable".to_string(),
                "Legible".to_string(),
                "Contemporaneous".to_string(),
                "Original".to_string(),
                "Accurate".to_string(),
                "Complete".to_string(),
                "Consistent".to_string(),
                "Enduring".to_string(),
                "Available".to_string(),
            ],
        }
    }

    /// Create a finding for a requirement
    fn create_finding_for_requirement(
        &self,
        requirement: &str,
        _report_type: ReportType,
    ) -> ComplianceFinding {
        // In a real implementation, this would check actual system state
        // For now, generate plausible mock findings
        ComplianceFinding {
            requirement: requirement.to_string(),
            status: FindingStatus::Compliant,
            evidence: vec![format!("Evidence for {}", requirement)],
            gaps: Vec::new(),
            recommendation: String::new(),
        }
    }

    /// Calculate summary statistics
    fn calculate_summary(&self, findings: &[ComplianceFinding]) -> ReportSummary {
        let total = findings.len();
        let compliant = findings
            .iter()
            .filter(|f| f.status == FindingStatus::Compliant)
            .count();
        let partial = findings
            .iter()
            .filter(|f| f.status == FindingStatus::PartiallyCompliant)
            .count();
        let non_compliant = findings
            .iter()
            .filter(|f| f.status == FindingStatus::NonCompliant)
            .count();
        let na = findings
            .iter()
            .filter(|f| f.status == FindingStatus::NotApplicable)
            .count();

        let compliance_pct = if total > na {
            (compliant as f64 / (total - na) as f64) * 100.0
        } else {
            100.0
        };

        ReportSummary {
            total_requirements: total,
            compliant_count: compliant,
            partially_compliant_count: partial,
            non_compliant_count: non_compliant,
            not_applicable_count: na,
            overall_status: OverallStatus::from_percentage(compliance_pct),
            compliance_percentage: compliance_pct,
        }
    }

    /// Approve a report
    pub fn approve_report(&self, report: &mut ComplianceReport, approver: &str) {
        report.approval_info = Some(ApprovalInfo {
            approved_by: approver.to_string(),
            approval_date: Utc::now(),
            signature: format!("SIG-{}", uuid::Uuid::new_v4()),
        });
    }

    /// Export report as structured data
    pub fn export_structured(&self, report: &ComplianceReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_default()
    }

    /// Check if a report meets regulatory submission requirements
    pub fn is_submittable(&self, report: &ComplianceReport) -> bool {
        report.approval_info.is_some()
            && report.summary.overall_status != OverallStatus::CriticalNonCompliance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_type_names() {
        assert_eq!(ReportType::CFR21Part11.name(), "21 CFR Part 11");
        assert_eq!(ReportType::EUAnnex11.name(), "EU Annex 11");
        assert_eq!(ReportType::GAMP5.name(), "GAMP 5");
        assert_eq!(ReportType::EUAIAct.name(), "EU AI Act");
        assert_eq!(ReportType::ALCOAPlus.name(), "ALCOA+");
    }

    #[test]
    fn test_report_type_required_sections() {
        let sections = ReportType::CFR21Part11.required_sections();
        assert!(sections.contains(&"Audit Trail"));
        assert!(sections.contains(&"Electronic Signatures"));

        let eu_sections = ReportType::EUAnnex11.required_sections();
        assert!(eu_sections.contains(&"Risk Assessment"));
        assert!(eu_sections.contains(&"Validation Documentation"));
    }

    #[test]
    fn test_compliance_service_new() {
        let config = ComplianceServiceConfig::new("Org", "System", "1.0", "Assessor");
        let service = ComplianceService::new(config);
        assert_eq!(service.config.organization, "Org");
    }

    #[test]
    fn test_compliance_service_defaults() {
        let service = ComplianceService::with_defaults("MyOrg", "MySystem");
        assert_eq!(service.config.organization, "MyOrg");
        assert_eq!(service.config.system_name, "MySystem");
    }

    #[test]
    fn test_generate_report() {
        let service = ComplianceService::with_defaults("Org", "System");
        let report = service.generate_report(ReportType::CFR21Part11);

        assert_eq!(report.report_type, ReportType::CFR21Part11);
        assert!(!report.findings.is_empty());
        assert!(report.approval_info.is_none());
    }

    #[test]
    fn test_report_summary_calculation() {
        let service = ComplianceService::with_defaults("Org", "System");
        let report = service.generate_report(ReportType::ALCOAPlus);

        assert!(report.summary.total_requirements > 0);
        let sum = report.summary.compliant_count
            + report.summary.partially_compliant_count
            + report.summary.non_compliant_count
            + report.summary.not_applicable_count;
        assert_eq!(sum, report.summary.total_requirements);
    }

    #[test]
    fn test_overall_status_from_percentage() {
        assert_eq!(
            OverallStatus::from_percentage(100.0),
            OverallStatus::FullCompliance
        );
        assert_eq!(
            OverallStatus::from_percentage(95.0),
            OverallStatus::FullCompliance
        );
        assert_eq!(
            OverallStatus::from_percentage(80.0),
            OverallStatus::AcceptableCompliance
        );
        assert_eq!(
            OverallStatus::from_percentage(50.0),
            OverallStatus::NeedsImprovement
        );
        assert_eq!(
            OverallStatus::from_percentage(49.0),
            OverallStatus::CriticalNonCompliance
        );
    }

    #[test]
    fn test_approve_report() {
        let service = ComplianceService::with_defaults("Org", "System");
        let mut report = service.generate_report(ReportType::GAMP5);
        assert!(report.approval_info.is_none());

        service.approve_report(&mut report, "Approver1");
        assert!(report.approval_info.is_some());
        assert_eq!(
            report.approval_info.as_ref().unwrap().approved_by,
            "Approver1"
        );
    }

    #[test]
    fn test_is_submittable() {
        let service = ComplianceService::with_defaults("Org", "System");
        let mut report = service.generate_report(ReportType::EUAIAct);

        // Not approved yet
        assert!(!service.is_submittable(&report));

        // Approve the report
        service.approve_report(&mut report, "Approver");
        assert!(service.is_submittable(&report));
    }

    #[test]
    fn test_export_structured() {
        let service = ComplianceService::with_defaults("Org", "System");
        let report = service.generate_report(ReportType::EUAnnex11);
        let json = service.export_structured(&report);

        assert!(json.contains("EUAnnex11"));
        assert!(json.contains("findings"));
    }

    #[test]
    fn test_finding_status() {
        assert_eq!(FindingStatus::Compliant.as_str(), "Compliant");
        assert_eq!(
            FindingStatus::PartiallyCompliant.as_str(),
            "Partially Compliant"
        );
        assert_eq!(FindingStatus::NonCompliant.as_str(), "Non-Compliant");
        assert_eq!(FindingStatus::NotApplicable.as_str(), "Not Applicable");
    }
}
