//! # EU AI Act Compliance
//!
//! Implements risk classification, transparency obligations, and conformity
//! assessment checks as defined by the EU Artificial Intelligence Act (Regulation 2024/1689).
//!
//! Key provisions covered:
//! - **Article 6/Annex III**: Risk classification (Unacceptable/High/Limited/Minimal)
//! - **Article 13**: Transparency obligations
//! - **Article 14**: Human oversight requirements
//! - **Article 15**: Accuracy, robustness, cybersecurity
//! - **Article 52**: Obligations for deployers of high-risk AI systems
//! - **Annex IV**: Technical documentation requirements

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ── Risk Level ─────────────────────────────────────────────────────────

/// EU AI Act risk classification (Article 6 + Annex III).
/// Variants ordered from least to most severe so derived `Ord` matches risk severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Minimal or no risk.
    Minimal,
    /// Article 52: Limited risk (chatbots, deepfakes, emotion recognition).
    Limited,
    /// Annex III: High-risk AI systems (biometrics, critical infra, education, etc.).
    High,
    /// Article 5: Prohibited AI practices.
    Unacceptable,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unacceptable => write!(f, "unacceptable"),
            Self::High => write!(f, "high"),
            Self::Limited => write!(f, "limited"),
            Self::Minimal => write!(f, "minimal"),
        }
    }
}

// ── AI System ──────────────────────────────────────────────────────────

/// Description of an AI system for EU AI Act compliance assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSystem {
    /// Unique identifier.
    pub id: String,
    /// System name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Provider/developer name.
    pub provider: String,
    /// Intended purpose.
    pub intended_purpose: String,
    /// AI techniques used (ML, rule-based, etc.).
    pub techniques: Vec<String>,
    /// Domain/sector of deployment.
    pub domain: AiDomain,
    /// Whether the system uses biometric identification.
    pub uses_biometric_identification: bool,
    /// Whether the system is used in critical infrastructure.
    pub critical_infrastructure: bool,
    /// Whether the system affects access to education or vocational training.
    pub education_impact: bool,
    /// Whether the system affects employment/worker management.
    pub employment_impact: bool,
    /// Whether the system affects access to essential services.
    pub essential_services: bool,
    /// Whether the system performs profiling of natural persons.
    pub profiling: bool,
    /// Whether the system generates/manipulates content (deepfakes).
    pub content_generation: bool,
    /// Whether the system is an emotion recognition system.
    pub emotion_recognition: bool,
    /// Whether the system is a chatbot/conversational agent.
    pub chatbot: bool,
    /// Whether human oversight is built in.
    pub human_oversight: bool,
    /// Whether the system processes personal data.
    pub processes_personal_data: bool,
    /// Technical documentation available.
    pub has_technical_docs: bool,
    /// Risk management system in place.
    pub has_risk_management: bool,
    /// Data governance measures in place.
    pub has_data_governance: bool,
    /// Logging capability.
    pub has_logging: bool,
    /// Transparency to users.
    pub has_transparency: bool,
    /// Accuracy metrics documented.
    pub has_accuracy_metrics: bool,
    /// Cybersecurity measures.
    pub has_cybersecurity: bool,
}

/// Domain of AI deployment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AiDomain {
    Biometrics,
    CriticalInfrastructure,
    Education,
    Employment,
    EssentialServices,
    LawEnforcement,
    Migration,
    Justice,
    General,
}

// ── Conformity Report ──────────────────────────────────────────────────

/// Result of an EU AI Act conformity assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformityReport {
    /// System assessed.
    pub system_id: String,
    /// System name.
    pub system_name: String,
    /// When the assessment was performed.
    pub assessed_at: DateTime<Utc>,
    /// Overall risk classification.
    pub risk_level: RiskLevel,
    /// Individual compliance checks.
    pub checks: Vec<ComplianceCheck>,
    /// Overall compliance status.
    pub status: ComplianceStatus,
    /// Summary of findings.
    pub summary: String,
    /// Recommendations for remediation.
    pub recommendations: Vec<String>,
}

/// Individual compliance check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheck {
    /// Article or annex reference.
    pub article: String,
    /// Requirement description.
    pub requirement: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Finding detail.
    pub detail: String,
    /// Severity if failed.
    pub severity: CheckSeverity,
}

/// Severity of a compliance check failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CheckSeverity {
    /// Must fix before deployment.
    Critical,
    /// Should fix, may affect conformity.
    Major,
    /// Best practice recommendation.
    Minor,
    /// Informational.
    Info,
}

/// Overall compliance status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ComplianceStatus {
    /// All checks passed.
    Compliant,
    /// Some non-critical issues found.
    PartiallyCompliant,
    /// Critical issues found — cannot deploy.
    NonCompliant,
    /// System is prohibited under Article 5.
    Prohibited,
}

impl fmt::Display for ComplianceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compliant => write!(f, "compliant"),
            Self::PartiallyCompliant => write!(f, "partially_compliant"),
            Self::NonCompliant => write!(f, "non_compliant"),
            Self::Prohibited => write!(f, "prohibited"),
        }
    }
}

// ── AI Act Checker ─────────────────────────────────────────────────────

/// EU AI Act compliance checker.
pub struct AiActChecker {
    /// Custom requirements per domain (optional overrides).
    domain_requirements: HashMap<AiDomain, Vec<String>>,
}

impl AiActChecker {
    pub fn new() -> Self {
        Self {
            domain_requirements: HashMap::new(),
        }
    }

    /// Add domain-specific requirements.
    pub fn add_domain_requirements(&mut self, domain: AiDomain, requirements: Vec<String>) {
        self.domain_requirements.insert(domain, requirements);
    }

    /// Perform a full conformity assessment on an AI system.
    pub fn assess(&self, system: &AiSystem) -> ConformityReport {
        let mut checks = Vec::new();
        let mut recommendations = Vec::new();

        // Step 1: Risk classification
        let risk_level = self.classify_risk(system);

        // Step 2: Article 5 — Prohibited practices check
        if risk_level == RiskLevel::Unacceptable {
            checks.push(ComplianceCheck {
                article: "Article 5".to_string(),
                requirement: "AI system must not be a prohibited practice".to_string(),
                passed: false,
                detail: "System classified as unacceptable risk under Article 5".to_string(),
                severity: CheckSeverity::Critical,
            });
        }

        // Step 3: Article 13 — Transparency
        checks.push(ComplianceCheck {
            article: "Article 13".to_string(),
            requirement: "Transparency — users must know they are interacting with AI".to_string(),
            passed: system.has_transparency,
            detail: if system.has_transparency {
                "Transparency measures in place".to_string()
            } else {
                "No transparency measures documented".to_string()
            },
            severity: if system.has_transparency {
                CheckSeverity::Info
            } else {
                CheckSeverity::Major
            },
        });

        // Step 4: Article 14 — Human oversight (for high-risk)
        if risk_level == RiskLevel::High {
            checks.push(ComplianceCheck {
                article: "Article 14".to_string(),
                requirement: "Human oversight for high-risk AI systems".to_string(),
                passed: system.human_oversight,
                detail: if system.human_oversight {
                    "Human oversight mechanisms in place".to_string()
                } else {
                    "No human oversight for high-risk system".to_string()
                },
                severity: if system.human_oversight {
                    CheckSeverity::Info
                } else {
                    CheckSeverity::Critical
                },
            });
            if !system.human_oversight {
                recommendations.push(
                    "Implement human-in-the-loop oversight for high-risk operations".to_string(),
                );
            }
        }

        // Step 5: Article 15 — Accuracy, robustness, cybersecurity
        checks.push(ComplianceCheck {
            article: "Article 15".to_string(),
            requirement: "Accuracy metrics documented".to_string(),
            passed: system.has_accuracy_metrics,
            detail: if system.has_accuracy_metrics {
                "Accuracy metrics documented".to_string()
            } else {
                "No accuracy metrics documentation".to_string()
            },
            severity: if system.has_accuracy_metrics {
                CheckSeverity::Info
            } else {
                CheckSeverity::Major
            },
        });

        checks.push(ComplianceCheck {
            article: "Article 15".to_string(),
            requirement: "Cybersecurity measures".to_string(),
            passed: system.has_cybersecurity,
            detail: if system.has_cybersecurity {
                "Cybersecurity measures in place".to_string()
            } else {
                "No cybersecurity measures documented".to_string()
            },
            severity: if system.has_cybersecurity {
                CheckSeverity::Info
            } else {
                CheckSeverity::Critical
            },
        });

        // Step 6: Annex IV — Technical documentation
        if risk_level == RiskLevel::High {
            checks.push(ComplianceCheck {
                article: "Annex IV".to_string(),
                requirement: "Technical documentation for high-risk systems".to_string(),
                passed: system.has_technical_docs,
                detail: if system.has_technical_docs {
                    "Technical documentation available".to_string()
                } else {
                    "Missing technical documentation (Annex IV)".to_string()
                },
                severity: if system.has_technical_docs {
                    CheckSeverity::Info
                } else {
                    CheckSeverity::Critical
                },
            });
        }

        // Step 7: Data governance (Article 10)
        checks.push(ComplianceCheck {
            article: "Article 10".to_string(),
            requirement: "Data governance measures".to_string(),
            passed: system.has_data_governance,
            detail: if system.has_data_governance {
                "Data governance in place".to_string()
            } else {
                "No data governance measures".to_string()
            },
            severity: if system.has_data_governance {
                CheckSeverity::Info
            } else {
                CheckSeverity::Major
            },
        });

        // Step 8: Logging (Article 12)
        if risk_level == RiskLevel::High {
            checks.push(ComplianceCheck {
                article: "Article 12".to_string(),
                requirement: "Automatic logging for high-risk systems".to_string(),
                passed: system.has_logging,
                detail: if system.has_logging {
                    "Logging enabled".to_string()
                } else {
                    "No automatic logging for high-risk system".to_string()
                },
                severity: if system.has_logging {
                    CheckSeverity::Info
                } else {
                    CheckSeverity::Critical
                },
            });
        }

        // Step 9: Risk management (Article 9)
        if risk_level >= RiskLevel::High {
            checks.push(ComplianceCheck {
                article: "Article 9".to_string(),
                requirement: "Risk management system".to_string(),
                passed: system.has_risk_management,
                detail: if system.has_risk_management {
                    "Risk management system in place".to_string()
                } else {
                    "No risk management system".to_string()
                },
                severity: if system.has_risk_management {
                    CheckSeverity::Info
                } else {
                    CheckSeverity::Critical
                },
            });
        }

        // Step 10: Article 52 — Transparency for limited-risk
        if risk_level == RiskLevel::Limited {
            if system.chatbot {
                checks.push(ComplianceCheck {
                    article: "Article 52(1)".to_string(),
                    requirement: "Chatbot must disclose AI nature".to_string(),
                    passed: system.has_transparency,
                    detail: if system.has_transparency {
                        "Chatbot AI disclosure in place".to_string()
                    } else {
                        "Chatbot does not disclose AI nature".to_string()
                    },
                    severity: if system.has_transparency {
                        CheckSeverity::Info
                    } else {
                        CheckSeverity::Major
                    },
                });
            }

            if system.content_generation {
                checks.push(ComplianceCheck {
                    article: "Article 52(3)".to_string(),
                    requirement: "AI-generated content must be disclosed".to_string(),
                    passed: system.has_transparency,
                    detail: if system.has_transparency {
                        "Content generation disclosure in place".to_string()
                    } else {
                        "AI-generated content not properly disclosed".to_string()
                    },
                    severity: if system.has_transparency {
                        CheckSeverity::Info
                    } else {
                        CheckSeverity::Major
                    },
                });
            }
        }

        // Domain-specific checks
        if let Some(reqs) = self.domain_requirements.get(&system.domain) {
            for req in reqs {
                checks.push(ComplianceCheck {
                    article: format!("Domain: {:?}", system.domain),
                    requirement: req.clone(),
                    passed: true, // Custom requirements assumed met unless specified
                    detail: "Domain-specific requirement (manual review needed)".to_string(),
                    severity: CheckSeverity::Info,
                });
            }
        }

        // Determine overall status
        let has_critical = checks
            .iter()
            .any(|c| !c.passed && c.severity == CheckSeverity::Critical);
        let has_major = checks
            .iter()
            .any(|c| !c.passed && c.severity == CheckSeverity::Major);

        let status = if risk_level == RiskLevel::Unacceptable {
            ComplianceStatus::Prohibited
        } else if has_critical {
            ComplianceStatus::NonCompliant
        } else if has_major {
            ComplianceStatus::PartiallyCompliant
        } else {
            ComplianceStatus::Compliant
        };

        // Generate summary
        let passed_count = checks.iter().filter(|c| c.passed).count();
        let total = checks.len();
        let summary = format!(
            "Risk level: {risk_level}. {passed_count}/{total} checks passed. Status: {status}"
        );

        // Generate recommendations for failed checks
        for check in &checks {
            if !check.passed && check.severity == CheckSeverity::Critical {
                recommendations.push(format!(
                    "[CRITICAL] {}: {}",
                    check.article, check.requirement
                ));
            }
        }

        ConformityReport {
            system_id: system.id.clone(),
            system_name: system.name.clone(),
            assessed_at: Utc::now(),
            risk_level,
            checks,
            status,
            summary,
            recommendations,
        }
    }

    /// Classify the risk level of an AI system.
    pub fn classify_risk(&self, system: &AiSystem) -> RiskLevel {
        // Article 5: Prohibited practices
        if self.is_prohibited(system) {
            return RiskLevel::Unacceptable;
        }

        // Annex III: High-risk
        if self.is_high_risk(system) {
            return RiskLevel::High;
        }

        // Article 52: Limited risk
        if self.is_limited_risk(system) {
            return RiskLevel::Limited;
        }

        RiskLevel::Minimal
    }

    fn is_prohibited(&self, system: &AiSystem) -> bool {
        // Article 5(1)(a): Subliminal manipulation
        // Article 5(1)(b): Exploitation of vulnerabilities
        // Article 5(1)(c): Social scoring by public authorities
        // Article 5(1)(d): Real-time remote biometric identification in public (law enforcement)
        // We flag if the system does social scoring or manipulative techniques
        system
            .intended_purpose
            .to_lowercase()
            .contains("social scoring")
            || system
                .intended_purpose
                .to_lowercase()
                .contains("subliminal manipulation")
    }

    fn is_high_risk(&self, system: &AiSystem) -> bool {
        // Annex III categories
        system.uses_biometric_identification
            || system.critical_infrastructure
            || system.education_impact
            || system.employment_impact
            || system.essential_services
            || system.profiling
            || system.domain == AiDomain::LawEnforcement
            || system.domain == AiDomain::Migration
            || system.domain == AiDomain::Justice
    }

    fn is_limited_risk(&self, system: &AiSystem) -> bool {
        system.chatbot || system.content_generation || system.emotion_recognition
    }
}
// ── Annex IV Technical Documentation ──────────────────────────────────
/// Annex IV technical documentation report for high-risk AI systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnexIVReport {
    /// System identifier.
    pub system_id: String,
    /// System name and version.
    pub system_name: String,
    /// Provider/developer information.
    pub provider: String,
    /// When the report was generated.
    pub generated_at: DateTime<Utc>,
    /// General description of the AI system.
    pub general_description: String,
    /// Detailed description of the AI system's elements.
    pub elements_description: ElementsDescription,
    /// Development process details.
    pub development_process: DevelopmentProcess,
    /// Risk management system details.
    pub risk_management: RiskManagementDetails,
    /// Performance metrics and validation.
    pub performance: PerformanceMetrics,
    /// Compliance status for each Annex IV section.
    pub compliance_sections: Vec<AnnexIVSection>,
}

/// Detailed description of AI system elements (Annex IV Section 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementsDescription {
    /// General logic and algorithms used.
    pub logic_and_algorithms: String,
    /// Data requirements and sources.
    pub data_requirements: String,
    /// Input/output specifications.
    pub io_specifications: String,
    /// Hardware and software requirements.
    pub hardware_software: String,
    /// Human oversight mechanisms.
    pub human_oversight_mechanisms: String,
}

/// Development process details (Annex IV Section 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentProcess {
    /// Methodologies used for development.
    pub methodologies: Vec<String>,
    /// Design choices and rationale.
    pub design_choices: String,
    /// Data preparation and processing.
    pub data_preparation: String,
    /// Training and testing procedures.
    pub training_testing: String,
    /// Validation procedures.
    pub validation_procedures: String,
    /// Known limitations and constraints.
    pub known_limitations: Vec<String>,
}

/// Risk management system details (Annex IV Section 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementDetails {
    /// Risk identification methodology.
    pub risk_identification: String,
    /// Risk assessment procedures.
    pub risk_assessment: String,
    /// Risk mitigation measures.
    pub risk_mitigation: Vec<String>,
    /// Residual risk evaluation.
    pub residual_risk: String,
    /// Testing and validation of risk controls.
    pub testing_validation: String,
}

/// Performance metrics (Annex IV Section 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Accuracy metrics.
    pub accuracy: HashMap<String, f64>,
    /// Robustness metrics.
    pub robustness: String,
    /// Cybersecurity measures.
    pub cybersecurity: String,
    /// Bias and fairness metrics.
    pub bias_fairness: HashMap<String, f64>,
    /// Test results summary.
    pub test_results: String,
}

/// Compliance status for an Annex IV section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnexIVSection {
    /// Section number (1-11).
    pub section: u8,
    /// Section title.
    pub title: String,
    /// Whether this section is compliant.
    pub compliant: bool,
    /// Details about compliance status.
    pub details: String,
    /// Missing items (if non-compliant).
    pub missing_items: Vec<String>,
}

/// Batch assessor for evaluating multiple AI systems.
pub struct BatchAssessor {
    checker: AiActChecker,
}

impl BatchAssessor {
    pub fn new() -> Self {
        Self {
            checker: AiActChecker::new(),
        }
    }

    /// Assess multiple AI systems and generate reports.
    pub fn assess_batch(&self, systems: &[AiSystem]) -> Vec<ConformityReport> {
        systems.iter().map(|s| self.checker.assess(s)).collect()
    }

    /// Generate summary statistics for a batch of assessments.
    pub fn batch_summary(&self, reports: &[ConformityReport]) -> BatchSummary {
        let total = reports.len();
        let compliant = reports
            .iter()
            .filter(|r| r.status == ComplianceStatus::Compliant)
            .count();
        let partially_compliant = reports
            .iter()
            .filter(|r| r.status == ComplianceStatus::PartiallyCompliant)
            .count();
        let non_compliant = reports
            .iter()
            .filter(|r| r.status == ComplianceStatus::NonCompliant)
            .count();
        let prohibited = reports
            .iter()
            .filter(|r| r.status == ComplianceStatus::Prohibited)
            .count();

        let high_risk = reports
            .iter()
            .filter(|r| r.risk_level == RiskLevel::High)
            .count();
        let limited_risk = reports
            .iter()
            .filter(|r| r.risk_level == RiskLevel::Limited)
            .count();
        let minimal_risk = reports
            .iter()
            .filter(|r| r.risk_level == RiskLevel::Minimal)
            .count();

        let total_checks: usize = reports.iter().map(|r| r.checks.len()).sum();
        let passed_checks: usize = reports
            .iter()
            .map(|r| r.checks.iter().filter(|c| c.passed).count())
            .sum();
        let failed_checks = total_checks - passed_checks;

        BatchSummary {
            total_systems: total,
            compliant,
            partially_compliant,
            non_compliant,
            prohibited,
            high_risk_count: high_risk,
            limited_risk_count: limited_risk,
            minimal_risk_count: minimal_risk,
            total_checks,
            passed_checks,
            failed_checks,
        }
    }
}

impl Default for BatchAssessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for a batch of conformity assessments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total_systems: usize,
    pub compliant: usize,
    pub partially_compliant: usize,
    pub non_compliant: usize,
    pub prohibited: usize,
    pub high_risk_count: usize,
    pub limited_risk_count: usize,
    pub minimal_risk_count: usize,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
}

impl Default for AiActChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_system() -> AiSystem {
        AiSystem {
            id: "sys-1".to_string(),
            name: "Simple Chatbot".to_string(),
            version: "1.0.0".to_string(),
            provider: "AgentGuard".to_string(),
            intended_purpose: "Answer customer questions".to_string(),
            techniques: vec!["LLM".to_string()],
            domain: AiDomain::General,
            uses_biometric_identification: false,
            critical_infrastructure: false,
            education_impact: false,
            employment_impact: false,
            essential_services: false,
            profiling: false,
            content_generation: false,
            emotion_recognition: false,
            chatbot: true,
            human_oversight: false,
            processes_personal_data: false,
            has_technical_docs: true,
            has_risk_management: false,
            has_data_governance: false,
            has_logging: true,
            has_transparency: true,
            has_accuracy_metrics: false,
            has_cybersecurity: true,
        }
    }

    fn high_risk_system() -> AiSystem {
        AiSystem {
            id: "sys-2".to_string(),
            name: "Biometric Access Control".to_string(),
            version: "2.0.0".to_string(),
            provider: "AgentGuard".to_string(),
            intended_purpose: "Facial recognition for building access".to_string(),
            techniques: vec!["CNN".to_string(), "FaceNet".to_string()],
            domain: AiDomain::Biometrics,
            uses_biometric_identification: true,
            critical_infrastructure: false,
            education_impact: false,
            employment_impact: false,
            essential_services: false,
            profiling: false,
            content_generation: false,
            emotion_recognition: false,
            chatbot: false,
            human_oversight: true,
            processes_personal_data: true,
            has_technical_docs: true,
            has_risk_management: true,
            has_data_governance: true,
            has_logging: true,
            has_transparency: true,
            has_accuracy_metrics: true,
            has_cybersecurity: true,
        }
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Unacceptable > RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Limited);
        assert!(RiskLevel::Limited > RiskLevel::Minimal);
    }

    #[test]
    fn test_risk_level_display() {
        assert_eq!(RiskLevel::Unacceptable.to_string(), "unacceptable");
        assert_eq!(RiskLevel::Minimal.to_string(), "minimal");
    }

    #[test]
    fn test_minimal_risk_chatbot() {
        let checker = AiActChecker::new();
        let system = minimal_system();
        assert_eq!(checker.classify_risk(&system), RiskLevel::Limited);
    }

    #[test]
    fn test_high_risk_biometric() {
        let checker = AiActChecker::new();
        let system = high_risk_system();
        assert_eq!(checker.classify_risk(&system), RiskLevel::High);
    }

    #[test]
    fn test_prohibited_system() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.intended_purpose = "Social scoring of citizens".to_string();
        assert_eq!(checker.classify_risk(&system), RiskLevel::Unacceptable);
    }

    #[test]
    fn test_conformity_report_limited_risk() {
        let checker = AiActChecker::new();
        let system = minimal_system();
        let report = checker.assess(&system);

        assert_eq!(report.risk_level, RiskLevel::Limited);
        assert_eq!(report.status, ComplianceStatus::PartiallyCompliant);
        // Should have Article 52 check for chatbot
        assert!(report.checks.iter().any(|c| c.article.contains("52")));
    }

    #[test]
    fn test_conformity_report_high_risk_compliant() {
        let checker = AiActChecker::new();
        let system = high_risk_system();
        let report = checker.assess(&system);

        assert_eq!(report.risk_level, RiskLevel::High);
        assert_eq!(report.status, ComplianceStatus::Compliant);
        // Should have Annex IV check
        assert!(report.checks.iter().any(|c| c.article == "Annex IV"));
        // Should have Article 14 human oversight check
        assert!(report.checks.iter().any(|c| c.article == "Article 14"));
    }

    #[test]
    fn test_conformity_report_prohibited() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.intended_purpose = "Subliminal manipulation of consumers".to_string();
        let report = checker.assess(&system);

        assert_eq!(report.status, ComplianceStatus::Prohibited);
    }

    #[test]
    fn test_high_risk_non_compliant() {
        let checker = AiActChecker::new();
        let mut system = high_risk_system();
        system.human_oversight = false;
        system.has_cybersecurity = false;
        system.has_logging = false;
        let report = checker.assess(&system);

        assert_eq!(report.status, ComplianceStatus::NonCompliant);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_compliance_status_display() {
        assert_eq!(ComplianceStatus::Compliant.to_string(), "compliant");
        assert_eq!(
            ComplianceStatus::PartiallyCompliant.to_string(),
            "partially_compliant"
        );
        assert_eq!(ComplianceStatus::NonCompliant.to_string(), "non_compliant");
        assert_eq!(ComplianceStatus::Prohibited.to_string(), "prohibited");
    }

    #[test]
    fn test_domain_specific_requirements() {
        let mut checker = AiActChecker::new();
        checker.add_domain_requirements(
            AiDomain::Biometrics,
            vec!["Must have bias audit every 6 months".to_string()],
        );

        let system = high_risk_system();
        let report = checker.assess(&system);
        assert!(report.checks.iter().any(|c| c.article.contains("Domain")));
    }

    #[test]
    fn test_employment_impact_high_risk() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.chatbot = false;
        system.employment_impact = true;
        assert_eq!(checker.classify_risk(&system), RiskLevel::High);
    }

    #[test]
    fn test_content_generation_limited_risk() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.chatbot = false;
        system.content_generation = true;
        assert_eq!(checker.classify_risk(&system), RiskLevel::Limited);
    }

    #[test]
    fn test_minimal_risk_system() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.chatbot = false;
        system.content_generation = false;
        system.emotion_recognition = false;
        assert_eq!(checker.classify_risk(&system), RiskLevel::Minimal);
    }

    #[test]
    fn test_report_summary_format() {
        let checker = AiActChecker::new();
        let system = high_risk_system();
        let report = checker.assess(&system);
        assert!(report.summary.contains("Risk level: high"));
        assert!(report.summary.contains("compliant"));
    }

    #[test]
    fn test_critical_infrastructure_high_risk() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.chatbot = false;
        system.critical_infrastructure = true;
        assert_eq!(checker.classify_risk(&system), RiskLevel::High);
    }

    #[test]
    fn test_law_enforcement_domain_high_risk() {
        let checker = AiActChecker::new();
        let mut system = minimal_system();
        system.chatbot = false;
        system.domain = AiDomain::LawEnforcement;
        assert_eq!(checker.classify_risk(&system), RiskLevel::High);
    }

    #[test]
    fn test_check_severity_levels() {
        // Ensure all severity levels are distinct
        let levels = [
            CheckSeverity::Critical,
            CheckSeverity::Major,
            CheckSeverity::Minor,
            CheckSeverity::Info,
        ];
        let unique: std::collections::HashSet<_> = levels.iter().collect();
        assert_eq!(unique.len(), 4);
    }
}
