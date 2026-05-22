//! # Whitepaper Data - Security & Compliance Evidence Collection
//!
//! Implements security evidence collection, compliance evidence gathering,
//! and automated technical documentation for whitepapers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Security evidence types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEvidenceType {
    AccessControl,
    Encryption,
    AuditLog,
    VulnerabilityAssessment,
    PenetrationTest,
    IncidentResponse,
    SecurityPolicy,
    ComplianceCertificate,
}

/// Security evidence record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvidence {
    pub id: String,
    pub evidence_type: SecurityEvidenceType,
    pub title: String,
    pub description: String,
    pub collected_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub data: serde_json::Value,
    pub certified: bool,
}

impl SecurityEvidence {
    pub fn new(evidence_type: SecurityEvidenceType, title: &str, description: &str) -> Self {
        Self {
            id: format!("ev-{}", uuid::Uuid::new_v4()),
            evidence_type,
            title: title.to_string(),
            description: description.to_string(),
            collected_at: Utc::now(),
            valid_until: None,
            data: serde_json::json!({}),
            certified: false,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    pub fn with_validity(mut self, valid_until: DateTime<Utc>) -> Self {
        self.valid_until = Some(valid_until);
        self
    }

    pub fn certify(&mut self) {
        self.certified = true;
    }

    pub fn is_expired(&self) -> bool {
        if let Some(valid_until) = self.valid_until {
            return valid_until < Utc::now();
        }
        false
    }
}

/// Compliance evidence types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceEvidenceType {
    PolicyDocument,
    AuditReport,
    TrainingRecord,
    RiskAssessment,
    IncidentLog,
    ChangeRecord,
    AccessReview,
    DataFlowDiagram,
}

impl ComplianceEvidenceType {
    pub fn name(&self) -> &'static str {
        match self {
            ComplianceEvidenceType::PolicyDocument => "Policy Document",
            ComplianceEvidenceType::AuditReport => "Audit Report",
            ComplianceEvidenceType::TrainingRecord => "Training Record",
            ComplianceEvidenceType::RiskAssessment => "Risk Assessment",
            ComplianceEvidenceType::IncidentLog => "Incident Log",
            ComplianceEvidenceType::ChangeRecord => "Change Record",
            ComplianceEvidenceType::AccessReview => "Access Review",
            ComplianceEvidenceType::DataFlowDiagram => "Data Flow Diagram",
        }
    }
}

/// Compliance evidence record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceEvidence {
    pub id: String,
    pub evidence_type: ComplianceEvidenceType,
    pub requirement: String,
    pub description: String,
    pub collected_at: DateTime<Utc>,
    pub attachments: Vec<String>,
    pub verified: bool,
}

impl ComplianceEvidence {
    pub fn new(
        evidence_type: ComplianceEvidenceType,
        requirement: &str,
        description: &str,
    ) -> Self {
        Self {
            id: format!("ce-{}", uuid::Uuid::new_v4()),
            evidence_type,
            requirement: requirement.to_string(),
            description: description.to_string(),
            collected_at: Utc::now(),
            attachments: Vec::new(),
            verified: false,
        }
    }

    pub fn with_attachments(mut self, attachments: Vec<&str>) -> Self {
        self.attachments = attachments.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn verify(&mut self) {
        self.verified = true;
    }
}

/// Technical documentation section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechDocSection {
    pub id: String,
    pub title: String,
    pub content: String,
    pub level: u8,
}

impl TechDocSection {
    pub fn new(title: &str, content: &str, level: u8) -> Self {
        Self {
            id: format!("sec-{}", uuid::Uuid::new_v4()),
            title: title.to_string(),
            content: content.to_string(),
            level,
        }
    }
}

/// Technical documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalDoc {
    pub title: String,
    pub version: String,
    pub generated_at: DateTime<Utc>,
    pub sections: Vec<TechDocSection>,
    pub security_evidence: Vec<SecurityEvidence>,
    pub compliance_evidence: Vec<ComplianceEvidence>,
}

impl TechnicalDoc {
    pub fn new(title: &str, version: &str) -> Self {
        Self {
            title: title.to_string(),
            version: version.to_string(),
            generated_at: Utc::now(),
            sections: Vec::new(),
            security_evidence: Vec::new(),
            compliance_evidence: Vec::new(),
        }
    }

    pub fn add_section(&mut self, section: TechDocSection) {
        self.sections.push(section);
    }

    pub fn add_security_evidence(&mut self, evidence: SecurityEvidence) {
        self.security_evidence.push(evidence);
    }

    pub fn add_compliance_evidence(&mut self, evidence: ComplianceEvidence) {
        self.compliance_evidence.push(evidence);
    }

    pub fn to_markdown(&self) -> String {
        let mut md = format!(
            "# {}\n\nVersion: {} | Generated: {}\n\n",
            self.title,
            self.version,
            self.generated_at.format("%Y-%m-%d %H:%M:%S")
        );

        for section in &self.sections {
            let heading = "#".repeat(section.level as usize);
            md.push_str(&format!(
                "\n{} {}\n\n{}\n",
                heading, section.title, section.content
            ));
        }

        if !self.security_evidence.is_empty() {
            md.push_str("\n## Security Evidence\n\n");
            for ev in &self.security_evidence {
                md.push_str(&format!(
                    "- **{}**: {} (collected: {})\n",
                    ev.title,
                    ev.description,
                    ev.collected_at.format("%Y-%m-%d")
                ));
            }
        }

        if !self.compliance_evidence.is_empty() {
            md.push_str("\n## Compliance Evidence\n\n");
            for ev in &self.compliance_evidence {
                md.push_str(&format!(
                    "- **{}** [{}]: {} (verified: {})\n",
                    ev.requirement,
                    ev.evidence_type.name(),
                    ev.description,
                    ev.verified
                ));
            }
        }

        md
    }
}

/// Evidence collector
#[derive(Debug, Clone)]
pub struct EvidenceCollector {
    security_evidence: Vec<SecurityEvidence>,
    compliance_evidence: Vec<ComplianceEvidence>,
}

impl Default for EvidenceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceCollector {
    pub fn new() -> Self {
        Self {
            security_evidence: Vec::new(),
            compliance_evidence: Vec::new(),
        }
    }

    pub fn collect_security(&mut self, evidence: SecurityEvidence) {
        self.security_evidence.push(evidence);
    }

    pub fn collect_compliance(&mut self, evidence: ComplianceEvidence) {
        self.compliance_evidence.push(evidence);
    }

    pub fn get_security_evidence(&self) -> &[SecurityEvidence] {
        &self.security_evidence
    }

    pub fn get_compliance_evidence(&self) -> &[ComplianceEvidence] {
        &self.compliance_evidence
    }

    pub fn generate_technical_doc(&self, title: &str, version: &str) -> TechnicalDoc {
        let mut doc = TechnicalDoc::new(title, version);

        doc.add_section(TechDocSection::new(
            "Security Overview",
            &format!(
                "This document contains {} security evidence items.",
                self.security_evidence.len()
            ),
            1,
        ));

        doc.add_section(TechDocSection::new(
            "Compliance Overview",
            &format!(
                "This document contains {} compliance evidence items.",
                self.compliance_evidence.len()
            ),
            1,
        ));

        for ev in &self.security_evidence {
            doc.add_security_evidence(ev.clone());
        }
        for ev in &self.compliance_evidence {
            doc.add_compliance_evidence(ev.clone());
        }

        doc
    }

    pub fn clear(&mut self) {
        self.security_evidence.clear();
        self.compliance_evidence.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_evidence_creation() {
        let ev = SecurityEvidence::new(
            SecurityEvidenceType::AccessControl,
            "Access Control Audit",
            "Quarterly review",
        );
        assert_eq!(ev.title, "Access Control Audit");
        assert!(!ev.certified);
    }

    #[test]
    fn test_security_evidence_certify() {
        let mut ev = SecurityEvidence::new(
            SecurityEvidenceType::Encryption,
            "Encryption Status",
            "TLS 1.3",
        );
        ev.certify();
        assert!(ev.certified);
    }

    #[test]
    fn test_security_evidence_with_data() {
        let ev = SecurityEvidence::new(
            SecurityEvidenceType::AuditLog,
            "Audit Log Sample",
            "Recent entries",
        )
        .with_data(serde_json::json!({"entries": 100}));
        assert!(!ev.data.is_null());
    }

    #[test]
    fn test_security_evidence_expiry() {
        let past = Utc::now() - chrono::Duration::days(1);
        let ev = SecurityEvidence::new(SecurityEvidenceType::PenetrationTest, "Pen Test", "Report")
            .with_validity(past);
        assert!(ev.is_expired());

        let future = Utc::now() + chrono::Duration::days(365);
        let ev2 = SecurityEvidence::new(
            SecurityEvidenceType::PenetrationTest,
            "Pen Test 2",
            "Report",
        )
        .with_validity(future);
        assert!(!ev2.is_expired());
    }

    #[test]
    fn test_compliance_evidence_creation() {
        let ev = ComplianceEvidence::new(
            ComplianceEvidenceType::AuditReport,
            "SOC2 Audit",
            "Annual audit",
        );
        assert_eq!(ev.requirement, "SOC2 Audit");
        assert!(!ev.verified);
    }

    #[test]
    fn test_compliance_evidence_with_attachments() {
        let ev = ComplianceEvidence::new(
            ComplianceEvidenceType::PolicyDocument,
            "Policy",
            "Company policy",
        )
        .with_attachments(vec!["policy.pdf", "signatures.pdf"]);
        assert_eq!(ev.attachments.len(), 2);
    }

    #[test]
    fn test_compliance_evidence_verify() {
        let mut ev = ComplianceEvidence::new(
            ComplianceEvidenceType::TrainingRecord,
            "Training",
            "Employee training",
        );
        ev.verify();
        assert!(ev.verified);
    }

    #[test]
    fn test_technical_doc_creation() {
        let doc = TechnicalDoc::new("Security Whitepaper", "1.0.0");
        assert_eq!(doc.title, "Security Whitepaper");
    }

    #[test]
    fn test_technical_doc_markdown() {
        let mut doc = TechnicalDoc::new("Test Doc", "1.0");
        doc.add_section(TechDocSection::new("Introduction", "Welcome", 1));
        let md = doc.to_markdown();
        assert!(md.contains("Test Doc"));
        assert!(md.contains("Introduction"));
    }

    #[test]
    fn test_evidence_collector_collect() {
        let mut collector = EvidenceCollector::new();
        collector.collect_security(SecurityEvidence::new(
            SecurityEvidenceType::AccessControl,
            "ACME Corp Access Review",
            "Annual review",
        ));
        collector.collect_compliance(ComplianceEvidence::new(
            ComplianceEvidenceType::AuditReport,
            "Annual SOC2 Audit",
            "SOC2 report",
        ));
        assert_eq!(collector.get_security_evidence().len(), 1);
        assert_eq!(collector.get_compliance_evidence().len(), 1);
    }

    #[test]
    fn test_evidence_collector_generate_doc() {
        let mut collector = EvidenceCollector::new();
        collector.collect_security(SecurityEvidence::new(
            SecurityEvidenceType::Encryption,
            "Encryption Status",
            "TLS 1.3 confirmed",
        ));
        let doc = collector.generate_technical_doc("Security Whitepaper", "1.0");
        assert_eq!(doc.title, "Security Whitepaper");
        assert!(!doc.security_evidence.is_empty());
    }
}
