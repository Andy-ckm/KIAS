//! # GAMP 5 Classification for AI/ML Systems
//!
//! GAMP 5 category determination and documentation requirements for AI agents.

use serde::{Deserialize, Serialize};

pub use super::agent_validation::ValidationStage;

/// AI system type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AIType {
    RuleBased,
    MLSupervised,
    MLUnsupervised,
    RL,
    Generative,
    Hybrid,
}

/// Data dependency criticality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDependency {
    Low,
    Medium,
    High,
    Critical,
}

/// Human oversight requirement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanOversightLevel {
    Full,    // Human reviews every decision
    Partial, // Human reviews high-risk decisions
    Minimal, // Human reviews only flagged decisions
}

/// Regulatory relevance of the AI system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegulatoryRelevance {
    Direct,   // Directly impacts patient safety
    Indirect, // Supports but does not drive decisions
    No,       // No regulatory impact
}

/// GAMP 5 category for AI systems (authoritative definition, shared)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GampCategory {
    StandardSoftware,
    ConfigurableSoftware,
    CustomSoftware,
    AIModel,
}

/// AI agent profile for GAMP 5 classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GampAIProfile {
    pub ai_type: AIType,
    pub data_dependency: DataDependency,
    pub model_retraining_frequency_months: u32,
    pub input_data_quality_requirements: Vec<String>,
    pub output_validation_required: bool,
    pub human_oversight_level: HumanOversightLevel,
    pub regulatory_relevance: RegulatoryRelevance,
    pub patient_safety_critical: bool,
    pub makes_decisions_autonomously: bool,
}

impl GampAIProfile {
    /// Create a standard AI profile with defaults.
    pub fn new(ai_type: AIType) -> Self {
        Self {
            ai_type,
            data_dependency: DataDependency::Medium,
            model_retraining_frequency_months: 6,
            input_data_quality_requirements: vec!["Data completeness check".to_string()],
            output_validation_required: true,
            human_oversight_level: HumanOversightLevel::Partial,
            regulatory_relevance: RegulatoryRelevance::Indirect,
            patient_safety_critical: false,
            makes_decisions_autonomously: false,
        }
    }

    pub fn with_data_dependency(mut self, dep: DataDependency) -> Self {
        self.data_dependency = dep;
        self
    }

    pub fn with_regulatory_relevance(mut self, rel: RegulatoryRelevance) -> Self {
        self.regulatory_relevance = rel;
        self
    }

    pub fn with_oversight(mut self, level: HumanOversightLevel) -> Self {
        self.human_oversight_level = level;
        self
    }

    pub fn with_retraining_frequency(mut self, months: u32) -> Self {
        self.model_retraining_frequency_months = months;
        self
    }

    pub fn patient_critical(mut self) -> Self {
        self.patient_safety_critical = true;
        self
    }
}

/// GAMP 5 classifier for AI systems
pub struct GampClassifier;

impl GampClassifier {
    /// Classify an AI agent into a GAMP 5 category.
    ///
    /// GAMP 5 categorization rules for AI/ML systems:
    /// - Generative AI (LLM) with direct regulatory impact → Custom Software
    /// - Supervised/Unsupervised ML with indirect impact → Configurable Software
    /// - Rule-based expert systems → Standard Software
    /// - Hybrid systems (ML + rules) → Custom Software
    pub fn classify(profile: &GampAIProfile) -> GampCategory {
        match profile.ai_type {
            AIType::Generative => {
                if profile.regulatory_relevance == RegulatoryRelevance::Direct
                    || profile.patient_safety_critical
                {
                    GampCategory::CustomSoftware
                } else {
                    GampCategory::CustomSoftware
                }
            }
            AIType::RuleBased => {
                if profile.data_dependency == DataDependency::Critical
                    || profile.patient_safety_critical
                {
                    GampCategory::ConfigurableSoftware
                } else {
                    GampCategory::StandardSoftware
                }
            }
            AIType::Hybrid => GampCategory::CustomSoftware,
            AIType::MLSupervised | AIType::MLUnsupervised => {
                if profile.regulatory_relevance == RegulatoryRelevance::Direct
                    || profile.patient_safety_critical
                {
                    GampCategory::CustomSoftware
                } else {
                    GampCategory::ConfigurableSoftware
                }
            }
            AIType::RL => {
                // Reinforcement learning is always custom due to unpredictable behavior
                GampCategory::CustomSoftware
            }
        }
    }

    /// Determine required validation stages for a GAMP 5 category.
    pub fn required_validation_approach(category: &GampCategory) -> Vec<ValidationStage> {
        match category {
            GampCategory::StandardSoftware => {
                vec![ValidationStage::InstallationQualification]
            }
            GampCategory::ConfigurableSoftware => {
                vec![
                    ValidationStage::InstallationQualification,
                    ValidationStage::OperationalQualification,
                ]
            }
            GampCategory::CustomSoftware | GampCategory::AIModel => {
                vec![
                    ValidationStage::InstallationQualification,
                    ValidationStage::OperationalQualification,
                    ValidationStage::PerformanceQualification,
                ]
            }
        }
    }

    /// Required documentation for a GAMP 5 category.
    pub fn documentation_requirements(category: &GampCategory) -> Vec<String> {
        match category {
            GampCategory::StandardSoftware => vec![
                "User Requirements Specification (URS)".to_string(),
                "Functional Specification (FS)".to_string(),
                "Installation Qualification Protocol (IQ)".to_string(),
                "Validation Summary Report".to_string(),
            ],
            GampCategory::ConfigurableSoftware => vec![
                "User Requirements Specification (URS)".to_string(),
                "Functional Specification (FS)".to_string(),
                "Configuration Specification".to_string(),
                "Installation Qualification (IQ)".to_string(),
                "Operational Qualification (OQ)".to_string(),
                "Validation Summary Report".to_string(),
            ],
            GampCategory::CustomSoftware | GampCategory::AIModel => vec![
                "User Requirements Specification (URS)".to_string(),
                "Functional Specification (FS)".to_string(),
                "Design Specification (DS)".to_string(),
                "Risk Assessment (ISO 14971)".to_string(),
                "Model Card / AI Bill of Materials".to_string(),
                "Installation Qualification (IQ)".to_string(),
                "Operational Qualification (OQ)".to_string(),
                "Performance Qualification (PQ)".to_string(),
                "Computerized System Validation (CSV) Report".to_string(),
                "Model Retraining Protocol".to_string(),
                "Change Control Record".to_string(),
                "GxP Audit Trail Review".to_string(),
            ],
        }
    }

    /// Whether continuous monitoring is required.
    pub fn requires_continuous_monitoring(category: &GampCategory) -> bool {
        matches!(category, GampCategory::AIModel | GampCategory::CustomSoftware)
    }

    /// SOP requirements for AI agents in GxP.
    pub fn sop_requirements() -> Vec<SOPRequirement> {
        vec![
            SOPRequirement::new(
                "AI-SOP-001",
                "AI Agent Decision Approval Procedure",
                "Critical",
                true,
                "FDA 21 CFR Part 11",
                12,
            ),
            SOPRequirement::new(
                "AI-SOP-002",
                "AI Model Retraining and Re-validation",
                "Critical",
                true,
                "GAMP 5",
                6,
            ),
            SOPRequirement::new(
                "AI-SOP-003",
                "AI Agent Audit Trail Review",
                "High",
                true,
                "EU Annex 11",
                12,
            ),
            SOPRequirement::new(
                "AI-SOP-004",
                "AI Agent Change Control",
                "Critical",
                true,
                "FDA 21 CFR Part 11",
                6,
            ),
            SOPRequirement::new(
                "AI-SOP-005",
                "AI Agent Incident Response",
                "High",
                true,
                "ISO 14971",
                3,
            ),
            SOPRequirement::new(
                "AI-SOP-006",
                "AI Model Data Quality Monitoring",
                "Medium",
                true,
                "GAMP 5",
                6,
            ),
            SOPRequirement::new(
                "AI-SOP-007",
                "AI Agent Electronic Signature Management",
                "Critical",
                true,
                "FDA 21 CFR Part 11",
                12,
            ),
            SOPRequirement::new(
                "AI-SOP-008",
                "AI System Periodic Review",
                "High",
                true,
                "EU AI Act",
                12,
            ),
        ]
    }
}

/// Standard Operating Procedure requirement for GxP AI systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SOPRequirement {
    pub sop_id: String,
    pub title: String,
    pub category: String,
    pub agent_applicable: bool,
    pub gxp_domain: String,
    pub review_frequency_months: u32,
}

impl SOPRequirement {
    pub fn new(
        sop_id: &str,
        title: &str,
        category: &str,
        agent_applicable: bool,
        gxp_domain: &str,
        review_frequency_months: u32,
    ) -> Self {
        Self {
            sop_id: sop_id.to_string(),
            title: title.to_string(),
            category: category.to_string(),
            agent_applicable,
            gxp_domain: gxp_domain.to_string(),
            review_frequency_months,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_rule_based() {
        let profile = GampAIProfile::new(AIType::RuleBased);
        assert_eq!(GampClassifier::classify(&profile), GampCategory::StandardSoftware);
    }

    #[test]
    fn test_classify_generative_ai() {
        let profile = GampAIProfile::new(AIType::Generative);
        // Generative is always Custom
        assert_eq!(GampClassifier::classify(&profile), GampCategory::CustomSoftware);
    }

    #[test]
    fn test_classify_ml_supervised_direct() {
        let profile = GampAIProfile::new(AIType::MLSupervised)
            .with_regulatory_relevance(RegulatoryRelevance::Direct)
            .patient_critical();
        assert_eq!(GampClassifier::classify(&profile), GampCategory::CustomSoftware);
    }

    #[test]
    fn test_classify_ml_supervised_indirect() {
        let profile = GampAIProfile::new(AIType::MLSupervised)
            .with_regulatory_relevance(RegulatoryRelevance::Indirect);
        assert_eq!(GampClassifier::classify(&profile), GampCategory::ConfigurableSoftware);
    }

    #[test]
    fn test_classify_hybrid() {
        let profile = GampAIProfile::new(AIType::Hybrid);
        assert_eq!(GampClassifier::classify(&profile), GampCategory::CustomSoftware);
    }

    #[test]
    fn test_classify_rl_always_custom() {
        let profile = GampAIProfile::new(AIType::RL);
        assert_eq!(GampClassifier::classify(&profile), GampCategory::CustomSoftware);
    }

    #[test]
    fn test_validation_approach_standard() {
        let stages = GampClassifier::required_validation_approach(&GampCategory::StandardSoftware);
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0], ValidationStage::InstallationQualification);
    }

    #[test]
    fn test_validation_approach_configurable() {
        let stages = GampClassifier::required_validation_approach(&GampCategory::ConfigurableSoftware);
        assert_eq!(stages.len(), 2);
    }

    #[test]
    fn test_validation_approach_custom() {
        let stages = GampClassifier::required_validation_approach(&GampCategory::CustomSoftware);
        assert_eq!(stages.len(), 3);
        assert!(stages.contains(&ValidationStage::PerformanceQualification));
    }

    #[test]
    fn test_validation_approach_ai_model() {
        let stages = GampClassifier::required_validation_approach(&GampCategory::AIModel);
        assert_eq!(stages.len(), 3);
    }

    #[test]
    fn test_documentation_requirements() {
        let docs = GampClassifier::documentation_requirements(&GampCategory::CustomSoftware);
        assert!(docs.len() >= 10);
        assert!(docs.contains(&"Risk Assessment (ISO 14971)".to_string()));
        assert!(docs.contains(&"Model Card / AI Bill of Materials".to_string()));
    }

    #[test]
    fn test_continuous_monitoring() {
        assert!(!GampClassifier::requires_continuous_monitoring(&GampCategory::StandardSoftware));
        assert!(GampClassifier::requires_continuous_monitoring(&GampCategory::AIModel));
        assert!(GampClassifier::requires_continuous_monitoring(&GampCategory::CustomSoftware));
    }

    #[test]
    fn test_sop_requirements() {
        let sops = GampClassifier::sop_requirements();
        assert!(sops.len() >= 5);
        let decision_sop = sops.iter().find(|s| s.sop_id == "AI-SOP-001");
        assert!(decision_sop.is_some());
        assert!(decision_sop.unwrap().agent_applicable);
    }

    #[test]
    fn test_gamp_ai_profile_builder() {
        let profile = GampAIProfile::new(AIType::MLSupervised)
            .with_data_dependency(DataDependency::Critical)
            .with_regulatory_relevance(RegulatoryRelevance::Direct)
            .with_oversight(HumanOversightLevel::Full)
            .with_retraining_frequency(3)
            .patient_critical();

        assert_eq!(profile.ai_type, AIType::MLSupervised);
        assert_eq!(profile.data_dependency, DataDependency::Critical);
        assert_eq!(profile.regulatory_relevance, RegulatoryRelevance::Direct);
        assert_eq!(profile.human_oversight_level, HumanOversightLevel::Full);
        assert_eq!(profile.model_retraining_frequency_months, 3);
        assert!(profile.patient_safety_critical);
    }
}
