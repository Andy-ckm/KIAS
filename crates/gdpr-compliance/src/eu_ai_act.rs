//! # EU AI Act Risk Classification Engine
//!
//! Implementation of EU AI Act (Regulation (EU) 2024/1689) compliance checking:
//! - Risk tier classification (Unacceptable/High/Limited/Minimal)
//! - Article 6 high-risk system categories
//! - Article 52 transparency obligations
//! - Conformity assessment requirements
//! - Prohibited practice detection
//! - Automated risk scoring with weighted criteria

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Risk tier classification per EU AI Act
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskTier {
    /// Prohibited AI practices (Article 5) - outright banned
    Unacceptable,
    /// High-risk AI systems (Article 6, Annex III) - strict requirements
    High,
    /// Limited risk AI systems (Article 52) - transparency obligations
    Limited,
    /// Minimal risk AI systems - no specific obligations
    Minimal,
}

impl RiskTier {
    /// Returns true if the tier requires conformity assessment
    pub fn requires_conformity_assessment(self) -> bool {
        matches!(self, RiskTier::Unacceptable | RiskTier::High)
    }

    /// Returns the regulatory article reference
    pub fn article_reference(self) -> &'static str {
        match self {
            RiskTier::Unacceptable => "Article 5",
            RiskTier::High => "Articles 6-7",
            RiskTier::Limited => "Article 52",
            RiskTier::Minimal => "No specific article",
        }
    }
}

/// Article 6 high-risk system categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HighRiskCategory {
    /// Biometric identification and categorisation (Annex III.1)
    BiometricBiometricCategorization,
    /// Critical infrastructure management (Annex III.2)
    CriticalInfrastructure,
    /// Education and vocational training (Annex III.3)
    EducationVocationalTraining,
    /// Employment, workers management and access to self-employment (Annex III.4)
    EmploymentSelfEmployment,
    /// Law enforcement (Annex III.5)
    LawEnforcement,
    /// Migration, asylum and border control (Annex III.6)
    MigrationAsylumBorder,
    /// Administration of justice and democratic processes (Annex III.7)
    JusticeDemocraticProcesses,
    /// Other high-risk category per Article 6(1)
    OtherHighRisk(String),
}

impl HighRiskCategory {
    /// Check if this category requires conformity assessment per Annex III
    pub fn requires_conformity_assessment(&self) -> bool {
        matches!(
            self,
            HighRiskCategory::BiometricBiometricCategorization
                | HighRiskCategory::CriticalInfrastructure
                | HighRiskCategory::EducationVocationalTraining
                | HighRiskCategory::EmploymentSelfEmployment
                | HighRiskCategory::LawEnforcement
                | HighRiskCategory::MigrationAsylumBorder
                | HighRiskCategory::JusticeDemocraticProcesses
                | HighRiskCategory::OtherHighRisk(_)
        )
    }

    /// Get the relevant EU AI Act Annex III reference
    pub fn annex_reference(&self) -> &'static str {
        match self {
            HighRiskCategory::BiometricBiometricCategorization => "Annex III.1",
            HighRiskCategory::CriticalInfrastructure => "Annex III.2",
            HighRiskCategory::EducationVocationalTraining => "Annex III.3",
            HighRiskCategory::EmploymentSelfEmployment => "Annex III.4",
            HighRiskCategory::LawEnforcement => "Annex III.5",
            HighRiskCategory::MigrationAsylumBorder => "Annex III.6",
            HighRiskCategory::JusticeDemocraticProcesses => "Annex III.7",
            HighRiskCategory::OtherHighRisk(_) => "Article 6(1)",
        }
    }
}

/// Article 52 transparency obligations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransparencyObligation {
    /// Chatbot disclosure - users must be informed they are interacting with AI (Article 52.1)
    ChatbotDisclosure,
    /// Deepfake labeling - AI-generated content must be labeled (Article 52.3)
    DeepfakeLabeling,
    /// Emotion recognition disclosure - users must be informed when emotion recognition is used (Article 52.1)
    EmotionRecognitionDisclosure,
    /// Biometric categorization disclosure - users must be informed when biometric categorization is used (Article 52.1)
    BiometricCategorizationDisclosure,
    /// Generated text disclosure - AI-generated text must be identified (Article 52.4)
    GeneratedTextDisclosure,
    /// High-risk transparency - high-risk systems must provide clear information to users (Article 52.2)
    HighRiskTransparency,
}

impl TransparencyObligation {
    /// Get the article reference
    pub fn article_reference(self) -> &'static str {
        match self {
            TransparencyObligation::ChatbotDisclosure => "Article 52.1",
            TransparencyObligation::DeepfakeLabeling => "Article 52.3",
            TransparencyObligation::EmotionRecognitionDisclosure => "Article 52.1",
            TransparencyObligation::BiometricCategorizationDisclosure => "Article 52.1",
            TransparencyObligation::GeneratedTextDisclosure => "Article 52.4",
            TransparencyObligation::HighRiskTransparency => "Article 52.2",
        }
    }
}

/// Prohibited AI practices under Article 5
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProhibitedPractice {
    /// Social scoring by public authorities (Article 5.1(a))
    SocialScoring,
    /// Real-time remote biometric identification in public spaces for law enforcement (Article 5.1(b))
    RealTimeBiometricIdentification,
    /// Subliminal/manipulative techniques beyond awareness (Article 5.1(c))
    SubliminalManipulation,
    /// Exploitation of vulnerabilities (Article 5.1(d))
    ExploitationOfVulnerabilities,
    /// AI-based assessment of risk in insurance/financial institutions (Article 5.1(e))
    InsuranceRiskAssessment,
    /// Creation of facial recognition databases by untargeted scraping (Article 5.1(f))
    UntargetedFacialRecognitionDatabase,
    /// Emotion recognition in workplace/education (Article 5.1(g))
    EmotionRecognitionWorkplaceEducation,
    /// Biometric categorisation for sensitive attributes (Article 5.1(h))
    BiometricCategorisationSensitive,
    /// Use of AI for influence in elections/public decision-making (Article 5.1(i))
    AiInfluenceInElections,
}

impl ProhibitedPractice {
    /// Get the specific article reference
    pub fn article_reference(&self) -> &'static str {
        match self {
            ProhibitedPractice::SocialScoring => "Article 5.1(a)",
            ProhibitedPractice::RealTimeBiometricIdentification => "Article 5.1(b)",
            ProhibitedPractice::SubliminalManipulation => "Article 5.1(c)",
            ProhibitedPractice::ExploitationOfVulnerabilities => "Article 5.1(d)",
            ProhibitedPractice::InsuranceRiskAssessment => "Article 5.1(e)",
            ProhibitedPractice::UntargetedFacialRecognitionDatabase => "Article 5.1(f)",
            ProhibitedPractice::EmotionRecognitionWorkplaceEducation => "Article 5.1(g)",
            ProhibitedPractice::BiometricCategorisationSensitive => "Article 5.1(h)",
            ProhibitedPractice::AiInfluenceInElections => "Article 5.1(i)",
        }
    }

    /// Description of the prohibited practice
    pub fn description(&self) -> &'static str {
        match self {
            ProhibitedPractice::SocialScoring => {
                "AI systems that score or classify individuals based on social behavior or personal characteristics"
            }
            ProhibitedPractice::RealTimeBiometricIdentification => {
                "Real-time remote biometric identification systems used in public spaces for law enforcement"
            }
            ProhibitedPractice::SubliminalManipulation => {
                "Subliminal techniques beyond awareness that can cause harm"
            }
            ProhibitedPractice::ExploitationOfVulnerabilities => {
                "AI systems that exploit vulnerabilities of specific groups"
            }
            ProhibitedPractice::InsuranceRiskAssessment => {
                "AI systems used for risk assessment in insurance and financial institutions"
            }
            ProhibitedPractice::UntargetedFacialRecognitionDatabase => {
                "Creation of facial recognition databases through untargeted scraping"
            }
            ProhibitedPractice::EmotionRecognitionWorkplaceEducation => {
                "Emotion recognition systems in workplace and educational institutions"
            }
            ProhibitedPractice::BiometricCategorisationSensitive => {
                "Biometric categorisation systems inferring sensitive attributes"
            }
            ProhibitedPractice::AiInfluenceInElections => {
                "AI systems used to influence elections and public decision-making"
            }
        }
    }
}

/// Conformity assessment requirement types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConformityAssessmentRequirement {
    /// Internal assessment for limited risk systems
    InternalAssessment,
    /// Third-party conformity assessment for high-risk systems
    ThirdPartyAssessment,
    /// No assessment required for minimal risk
    None,
    /// Prohibited - cannot be placed on market
    Prohibited,
}

impl ConformityAssessmentRequirement {
    /// Get the requirement for a given risk tier
    pub fn for_tier(tier: RiskTier) -> Self {
        match tier {
            RiskTier::Unacceptable => ConformityAssessmentRequirement::Prohibited,
            RiskTier::High => ConformityAssessmentRequirement::ThirdPartyAssessment,
            RiskTier::Limited => ConformityAssessmentRequirement::InternalAssessment,
            RiskTier::Minimal => ConformityAssessmentRequirement::None,
        }
    }
}

/// Criteria for automated risk scoring with weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScoringCriteria {
    /// Impact score (0.0-1.0) - severity of potential harm
    pub impact_score: f64,
    /// Likelihood score (0.0-1.0) - probability of harm occurring
    pub likelihood_score: f64,
    /// Autonomy score (0.0-1.0) - degree of AI autonomy in decision-making
    pub autonomy_score: f64,
    /// Transparency score (0.0-1.0) - how transparent the system is
    pub transparency_score: f64,
    /// Data sensitivity score (0.0-1.0) - sensitivity of data processed
    pub data_sensitivity_score: f64,
    /// Oversight score (0.0-1.0) - level of human oversight
    pub oversight_score: f64,
}

impl Default for RiskScoringCriteria {
    fn default() -> Self {
        Self {
            impact_score: 0.5,
            likelihood_score: 0.5,
            autonomy_score: 0.5,
            transparency_score: 0.5,
            data_sensitivity_score: 0.5,
            oversight_score: 0.5,
        }
    }
}

impl RiskScoringCriteria {
    /// Weight factors for the risk score calculation
    const IMPACT_WEIGHT: f64 = 0.30;
    const LIKELIHOOD_WEIGHT: f64 = 0.20;
    const AUTONOMY_WEIGHT: f64 = 0.15;
    const TRANSPARENCY_WEIGHT: f64 = 0.10;
    const DATA_SENSITIVITY_WEIGHT: f64 = 0.15;
    const OVERSIGHT_WEIGHT: f64 = 0.10;

    /// Calculate weighted risk score (0.0-1.0)
    pub fn calculate_risk_score(&self) -> f64 {
        let weighted_sum = (self.impact_score * Self::IMPACT_WEIGHT)
            + (self.likelihood_score * Self::LIKELIHOOD_WEIGHT)
            + (self.autonomy_score * Self::AUTONOMY_WEIGHT)
            + ((1.0 - self.transparency_score) * Self::TRANSPARENCY_WEIGHT)
            + (self.data_sensitivity_score * Self::DATA_SENSITIVITY_WEIGHT)
            + ((1.0 - self.oversight_score) * Self::OVERSIGHT_WEIGHT);
        weighted_sum.clamp(0.0, 1.0)
    }

    /// Convert risk score to risk tier
    pub fn score_to_tier(score: f64) -> RiskTier {
        if score >= 0.8 {
            RiskTier::Unacceptable
        } else if score >= 0.6 {
            RiskTier::High
        } else if score >= 0.3 {
            RiskTier::Limited
        } else {
            RiskTier::Minimal
        }
    }
}

/// AI system description for risk classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSystemDescription {
    /// System name
    pub name: String,
    /// System purpose description
    pub purpose: String,
    /// High-risk categories applicable
    pub high_risk_categories: Vec<HighRiskCategory>,
    /// Uses prohibited practices
    pub uses_prohibited_practices: Vec<ProhibitedPractice>,
    /// Transparency obligations applicable
    pub transparency_obligations: Vec<TransparencyObligation>,
    /// Risk scoring criteria
    pub risk_criteria: RiskScoringCriteria,
    /// Technical documentation available
    pub has_technical_documentation: bool,
    /// Quality management system in place
    pub has_quality_management: bool,
    /// Human oversight measures in place
    pub has_human_oversight: bool,
    /// Accuracy, robustness and cybersecurity measures
    pub has_accuracy_robustness: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Default for AiSystemDescription {
    fn default() -> Self {
        Self {
            name: String::new(),
            purpose: String::new(),
            high_risk_categories: Vec::new(),
            uses_prohibited_practices: Vec::new(),
            transparency_obligations: Vec::new(),
            risk_criteria: RiskScoringCriteria::default(),
            has_technical_documentation: false,
            has_quality_management: false,
            has_human_oversight: false,
            has_accuracy_robustness: false,
            created_at: Utc::now(),
        }
    }
}

impl AiSystemDescription {
    /// Create a new AI system description
    pub fn new(name: String, purpose: String) -> Self {
        Self {
            name,
            purpose,
            ..Default::default()
        }
    }

    /// Add a high-risk category
    pub fn add_high_risk_category(mut self, category: HighRiskCategory) -> Self {
        self.high_risk_categories.push(category);
        self
    }

    /// Add a prohibited practice
    pub fn add_prohibited_practice(mut self, practice: ProhibitedPractice) -> Self {
        self.uses_prohibited_practices.push(practice);
        self
    }

    /// Add a transparency obligation
    pub fn add_transparency_obligation(mut self, obligation: TransparencyObligation) -> Self {
        self.transparency_obligations.push(obligation);
        self
    }

    /// Set risk scoring criteria
    pub fn set_risk_criteria(mut self, criteria: RiskScoringCriteria) -> Self {
        self.risk_criteria = criteria;
        self
    }
}

/// Result of EU AI Act risk classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskClassificationResult {
    /// System name
    pub system_name: String,
    /// Determined risk tier
    pub risk_tier: RiskTier,
    /// Calculated risk score (0.0-1.0)
    pub risk_score: f64,
    /// Applicable high-risk categories
    pub high_risk_categories: Vec<HighRiskCategory>,
    /// Detected prohibited practices
    pub prohibited_practices: Vec<ProhibitedPractice>,
    /// Required transparency obligations
    pub transparency_obligations: Vec<TransparencyObligation>,
    /// Conformity assessment requirement
    pub conformity_assessment: ConformityAssessmentRequirement,
    /// Compliance gaps identified
    pub compliance_gaps: Vec<ComplianceGap>,
    /// Recommendations
    pub recommendations: Vec<String>,
    /// Assessment timestamp
    pub assessed_at: DateTime<Utc>,
}

/// A compliance gap identified during assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGap {
    /// Gap identifier
    pub id: String,
    /// Description of the gap
    pub description: String,
    /// Severity level
    pub severity: GapSeverity,
    /// Relevant article reference
    pub article_reference: String,
    /// Recommended remediation
    pub remediation: String,
}

/// Gap severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum GapSeverity {
    Critical,
    Major,
    Minor,
    Info,
}

/// EU AI Act compliance engine
pub struct EuAiActClassifier {
    /// Classification results cache
    results: HashMap<String, RiskClassificationResult>,
}

impl Default for EuAiActClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl EuAiActClassifier {
    /// Create a new classifier instance
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    /// Classify an AI system against EU AI Act requirements
    pub fn classify_system(&mut self, system: &AiSystemDescription) -> RiskClassificationResult {
        let risk_score = system.risk_criteria.calculate_risk_score();

        // First check for prohibited practices - these are outright banned
        let has_prohibited = !system.uses_prohibited_practices.is_empty();

        // Determine risk tier
        let risk_tier = if has_prohibited {
            RiskTier::Unacceptable
        } else {
            RiskScoringCriteria::score_to_tier(risk_score)
        };

        // Collect high-risk categories from the system description
        let high_risk_categories = if system.high_risk_categories.is_empty() {
            // Auto-detect based on system purpose if not explicitly set
            self.detect_high_risk_categories(system)
        } else {
            system.high_risk_categories.clone()
        };

        // Determine transparency obligations
        let transparency_obligations = self.determine_transparency_obligations(system);

        // Determine conformity assessment requirement
        let conformity_assessment = ConformityAssessmentRequirement::for_tier(risk_tier);

        // Identify compliance gaps
        let compliance_gaps = self.identify_compliance_gaps(system, risk_tier);

        // Generate recommendations
        let recommendations = self.generate_recommendations(
            system,
            risk_tier,
            &compliance_gaps,
            &high_risk_categories,
        );

        let result = RiskClassificationResult {
            system_name: system.name.clone(),
            risk_tier,
            risk_score,
            high_risk_categories,
            prohibited_practices: system.uses_prohibited_practices.clone(),
            transparency_obligations,
            conformity_assessment,
            compliance_gaps,
            recommendations,
            assessed_at: Utc::now(),
        };

        self.results.insert(system.name.clone(), result.clone());
        result
    }

    /// Auto-detect high-risk categories based on system purpose
    fn detect_high_risk_categories(&self, system: &AiSystemDescription) -> Vec<HighRiskCategory> {
        let purpose_lower = system.purpose.to_lowercase();
        let mut categories = Vec::new();

        if purpose_lower.contains("biometric")
            || purpose_lower.contains("facial recognition")
            || purpose_lower.contains("fingerprint")
            || purpose_lower.contains("iris")
        {
            categories.push(HighRiskCategory::BiometricBiometricCategorization);
        }

        if purpose_lower.contains("critical infrastructure")
            || purpose_lower.contains("energy grid")
            || purpose_lower.contains("water supply")
            || purpose_lower.contains("transportation")
        {
            categories.push(HighRiskCategory::CriticalInfrastructure);
        }

        if purpose_lower.contains("education")
            || purpose_lower.contains("training")
            || purpose_lower.contains("student")
            || purpose_lower.contains("learning")
        {
            categories.push(HighRiskCategory::EducationVocationalTraining);
        }

        if purpose_lower.contains("employment")
            || purpose_lower.contains("hiring")
            || purpose_lower.contains("recruitment")
            || purpose_lower.contains("worker")
        {
            categories.push(HighRiskCategory::EmploymentSelfEmployment);
        }

        if purpose_lower.contains("law enforcement")
            || purpose_lower.contains("police")
            || purpose_lower.contains("investigation")
            || purpose_lower.contains("judicial")
        {
            categories.push(HighRiskCategory::LawEnforcement);
        }

        if purpose_lower.contains("migration")
            || purpose_lower.contains("asylum")
            || purpose_lower.contains("border")
            || purpose_lower.contains("immigration")
        {
            categories.push(HighRiskCategory::MigrationAsylumBorder);
        }

        if purpose_lower.contains("justice")
            || purpose_lower.contains("court")
            || purpose_lower.contains("democratic")
            || purpose_lower.contains("election")
            || purpose_lower.contains("voting")
        {
            categories.push(HighRiskCategory::JusticeDemocraticProcesses);
        }

        categories
    }

    /// Determine applicable transparency obligations
    fn determine_transparency_obligations(
        &self,
        system: &AiSystemDescription,
    ) -> Vec<TransparencyObligation> {
        let mut obligations = Vec::new();
        let purpose_lower = system.purpose.to_lowercase();

        // Check for chatbot/interactive AI
        if purpose_lower.contains("chatbot")
            || purpose_lower.contains("conversational")
            || purpose_lower.contains("virtual assistant")
        {
            obligations.push(TransparencyObligation::ChatbotDisclosure);
        }

        // Check for deepfake/generative content
        if purpose_lower.contains("deepfake")
            || purpose_lower.contains("synthetic media")
            || purpose_lower.contains("generated video")
            || purpose_lower.contains("generated image")
        {
            obligations.push(TransparencyObligation::DeepfakeLabeling);
        }

        // Check for emotion recognition
        if purpose_lower.contains("emotion recognition")
            || purpose_lower.contains("affect detection")
            || purpose_lower.contains("sentiment analysis")
        {
            obligations.push(TransparencyObligation::EmotionRecognitionDisclosure);
        }

        // Check for biometric categorization
        if purpose_lower.contains("biometric categorization")
            || purpose_lower.contains("biometric classification")
        {
            obligations.push(TransparencyObligation::BiometricCategorizationDisclosure);
        }

        // High-risk systems always have high-risk transparency obligation
        if !system.high_risk_categories.is_empty() {
            obligations.push(TransparencyObligation::HighRiskTransparency);
        }

        obligations
    }

    /// Identify compliance gaps
    fn identify_compliance_gaps(
        &self,
        system: &AiSystemDescription,
        tier: RiskTier,
    ) -> Vec<ComplianceGap> {
        let mut gaps = Vec::new();

        // Check for prohibited practices
        for practice in &system.uses_prohibited_practices {
            gaps.push(ComplianceGap {
                id: format!("PROHIBITED_{:?}", practice),
                description: format!("Prohibited practice detected: {}", practice.description()),
                severity: GapSeverity::Critical,
                article_reference: practice.article_reference().to_string(),
                remediation: "Remove or redesign the system to avoid prohibited practice"
                    .to_string(),
            });
        }

        // High-risk system requirements
        if tier == RiskTier::High {
            if !system.has_technical_documentation {
                gaps.push(ComplianceGap {
                    id: "TECH_DOC".to_string(),
                    description: "Technical documentation not available".to_string(),
                    severity: GapSeverity::Major,
                    article_reference: "Article 11".to_string(),
                    remediation: "Prepare comprehensive technical documentation per Annex IV"
                        .to_string(),
                });
            }

            if !system.has_quality_management {
                gaps.push(ComplianceGap {
                    id: "QMS".to_string(),
                    description: "Quality management system not in place".to_string(),
                    severity: GapSeverity::Major,
                    article_reference: "Article 9".to_string(),
                    remediation: "Implement a quality management system".to_string(),
                });
            }

            if !system.has_human_oversight {
                gaps.push(ComplianceGap {
                    id: "OVERSIGHT".to_string(),
                    description: "Human oversight measures not defined".to_string(),
                    severity: GapSeverity::Major,
                    article_reference: "Article 14".to_string(),
                    remediation: "Define and implement human oversight measures".to_string(),
                });
            }

            if !system.has_accuracy_robustness {
                gaps.push(ComplianceGap {
                    id: "ACCURACY".to_string(),
                    description: "Accuracy, robustness and cybersecurity measures not documented"
                        .to_string(),
                    severity: GapSeverity::Major,
                    article_reference: "Article 15".to_string(),
                    remediation: "Document accuracy, robustness and cybersecurity measures"
                        .to_string(),
                });
            }
        }

        // Transparency obligations check
        for obligation in &system.transparency_obligations {
            match obligation {
                TransparencyObligation::ChatbotDisclosure
                    if !system.purpose.to_lowercase().contains("disclosure") =>
                {
                    gaps.push(ComplianceGap {
                        id: "CHATBOT_DISCLOSURE".to_string(),
                        description: "Chatbot disclosure mechanism not implemented".to_string(),
                        severity: GapSeverity::Minor,
                        article_reference: "Article 52.1".to_string(),
                        remediation:
                            "Implement clear disclosure that users are interacting with AI"
                                .to_string(),
                    });
                }
                TransparencyObligation::ChatbotDisclosure => {}
                TransparencyObligation::DeepfakeLabeling => {
                    gaps.push(ComplianceGap {
                        id: "DEEPFAKE".to_string(),
                        description: "Deepfake labeling not implemented".to_string(),
                        severity: GapSeverity::Major,
                        article_reference: "Article 52.3".to_string(),
                        remediation: "Implement visible labels for AI-generated content"
                            .to_string(),
                    });
                }
                _ => {}
            }
        }

        gaps
    }

    /// Generate compliance recommendations
    fn generate_recommendations(
        &self,
        _system: &AiSystemDescription,
        tier: RiskTier,
        gaps: &[ComplianceGap],
        categories: &[HighRiskCategory],
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match tier {
            RiskTier::Unacceptable => {
                recommendations.push(
                    "IMMEDIATE ACTION REQUIRED: This system uses prohibited AI practices and cannot be deployed in the EU market.".to_string(),
                );
                recommendations.push(
                    "Consider redesigning the system to remove prohibited components.".to_string(),
                );
            }
            RiskTier::High => {
                recommendations.push(
                    "High-risk AI system: Ensure full compliance with Annex II (quality management) and Annex III (technical documentation).".to_string(),
                );
                if !categories.is_empty() {
                    let category_list: Vec<String> = categories
                        .iter()
                        .map(|c| c.annex_reference().to_string())
                        .collect();
                    recommendations.push(format!(
                        "Relevant Annex III categories: {}",
                        category_list.join(", ")
                    ));
                }
                recommendations
                    .push("Register the system in the EU database before deployment.".to_string());
            }
            RiskTier::Limited => {
                recommendations.push(
                    "Limited risk: Implement required transparency measures per Article 52."
                        .to_string(),
                );
            }
            RiskTier::Minimal => {
                recommendations.push(
                    "Minimal risk: No specific obligations, but best practices recommended."
                        .to_string(),
                );
            }
        }

        // Add gap-specific recommendations
        for gap in gaps {
            if gap.severity == GapSeverity::Critical || gap.severity == GapSeverity::Major {
                recommendations.push(format!("[{}] {}", gap.article_reference, gap.remediation));
            }
        }

        if recommendations.is_empty() {
            recommendations
                .push("System appears compliant with EU AI Act requirements.".to_string());
        }

        recommendations
    }

    /// Check if a specific prohibited practice is present
    pub fn check_prohibited_practice(
        system: &AiSystemDescription,
        practice: ProhibitedPractice,
    ) -> bool {
        system.uses_prohibited_practices.contains(&practice)
    }

    /// Get a previously stored classification result
    pub fn get_result(&self, system_name: &str) -> Option<&RiskClassificationResult> {
        self.results.get(system_name)
    }

    /// List all classification results
    pub fn list_results(&self) -> Vec<&RiskClassificationResult> {
        self.results.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_system(name: &str) -> AiSystemDescription {
        AiSystemDescription::new(name.to_string(), "Test AI system".to_string())
    }

    #[test]
    fn test_risk_tier_classification_unacceptable() {
        let mut system = create_test_system("prohibited_system");
        system = system.add_prohibited_practice(ProhibitedPractice::SocialScoring);

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert_eq!(result.risk_tier, RiskTier::Unacceptable);
        assert!(!result.prohibited_practices.is_empty());
    }

    #[test]
    fn test_risk_tier_classification_high() {
        let mut system = create_test_system("high_risk_system");
        system = system.add_high_risk_category(HighRiskCategory::BiometricBiometricCategorization);
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.8,
            likelihood_score: 0.7,
            autonomy_score: 0.6,
            transparency_score: 0.3,
            data_sensitivity_score: 0.9,
            oversight_score: 0.2,
        });

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert_eq!(result.risk_tier, RiskTier::High);
        assert!(!result.high_risk_categories.is_empty());
    }

    #[test]
    fn test_risk_tier_classification_limited() {
        let system = create_test_system("limited_risk_system");

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        // Without prohibited practices and medium risk score, should be Limited
        assert!(matches!(
            result.risk_tier,
            RiskTier::Limited | RiskTier::Minimal
        ));
    }

    #[test]
    fn test_risk_scoring_criteria_calculation() {
        let criteria = RiskScoringCriteria {
            impact_score: 0.8,
            likelihood_score: 0.6,
            autonomy_score: 0.7,
            transparency_score: 0.2,
            data_sensitivity_score: 0.9,
            oversight_score: 0.1,
        };

        let score = criteria.calculate_risk_score();
        assert!(score > 0.5); // Should be high risk due to high impact and low transparency
    }

    #[test]
    fn test_risk_score_to_tier_thresholds() {
        assert_eq!(
            RiskScoringCriteria::score_to_tier(0.85),
            RiskTier::Unacceptable
        );
        assert_eq!(RiskScoringCriteria::score_to_tier(0.70), RiskTier::High);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.45), RiskTier::Limited);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.15), RiskTier::Minimal);
    }

    #[test]
    fn test_conformity_assessment_requirements() {
        assert_eq!(
            ConformityAssessmentRequirement::for_tier(RiskTier::Unacceptable),
            ConformityAssessmentRequirement::Prohibited
        );
        assert_eq!(
            ConformityAssessmentRequirement::for_tier(RiskTier::High),
            ConformityAssessmentRequirement::ThirdPartyAssessment
        );
        assert_eq!(
            ConformityAssessmentRequirement::for_tier(RiskTier::Limited),
            ConformityAssessmentRequirement::InternalAssessment
        );
        assert_eq!(
            ConformityAssessmentRequirement::for_tier(RiskTier::Minimal),
            ConformityAssessmentRequirement::None
        );
    }

    #[test]
    fn test_high_risk_category_conformity() {
        assert!(HighRiskCategory::BiometricBiometricCategorization.requires_conformity_assessment());
        assert!(HighRiskCategory::LawEnforcement.requires_conformity_assessment());
        assert!(HighRiskCategory::JusticeDemocraticProcesses.requires_conformity_assessment());
        assert!(
            HighRiskCategory::OtherHighRisk("custom".to_string()).requires_conformity_assessment()
        );
    }

    #[test]
    fn test_transparency_obligation_article_references() {
        assert_eq!(
            TransparencyObligation::ChatbotDisclosure.article_reference(),
            "Article 52.1"
        );
        assert_eq!(
            TransparencyObligation::DeepfakeLabeling.article_reference(),
            "Article 52.3"
        );
        assert_eq!(
            TransparencyObligation::EmotionRecognitionDisclosure.article_reference(),
            "Article 52.1"
        );
    }

    #[test]
    fn test_prohibited_practice_detection() {
        let system = AiSystemDescription::new(
            "social_scoring_system".to_string(),
            "Government social scoring system".to_string(),
        )
        .add_prohibited_practice(ProhibitedPractice::SocialScoring);

        assert!(EuAiActClassifier::check_prohibited_practice(
            &system,
            ProhibitedPractice::SocialScoring
        ));
        assert!(!EuAiActClassifier::check_prohibited_practice(
            &system,
            ProhibitedPractice::RealTimeBiometricIdentification
        ));
    }

    #[test]
    fn test_prohibited_practice_descriptions() {
        assert!(!ProhibitedPractice::SocialScoring.description().is_empty());
        assert!(!ProhibitedPractice::RealTimeBiometricIdentification
            .description()
            .is_empty());
        assert!(!ProhibitedPractice::SubliminalManipulation
            .description()
            .is_empty());
        assert!(!ProhibitedPractice::ExploitationOfVulnerabilities
            .description()
            .is_empty());
    }

    #[test]
    fn test_high_risk_category_annex_references() {
        assert_eq!(
            HighRiskCategory::BiometricBiometricCategorization.annex_reference(),
            "Annex III.1"
        );
        assert_eq!(
            HighRiskCategory::CriticalInfrastructure.annex_reference(),
            "Annex III.2"
        );
        assert_eq!(
            HighRiskCategory::EducationVocationalTraining.annex_reference(),
            "Annex III.3"
        );
    }

    #[test]
    fn test_auto_detection_biometric() {
        let system = AiSystemDescription::new(
            "facial_recognition".to_string(),
            "Facial recognition system for security".to_string(),
        );

        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);

        assert!(categories.contains(&HighRiskCategory::BiometricBiometricCategorization));
    }

    #[test]
    fn test_auto_detection_law_enforcement() {
        let system = AiSystemDescription::new(
            "police_assistant".to_string(),
            "AI system for law enforcement investigation assistance".to_string(),
        );

        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);

        assert!(categories.contains(&HighRiskCategory::LawEnforcement));
    }

    #[test]
    fn test_auto_detection_chatbot_transparency() {
        let system = AiSystemDescription::new(
            "customer_chatbot".to_string(),
            "Conversational AI chatbot for customer support".to_string(),
        );

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert!(result
            .transparency_obligations
            .contains(&TransparencyObligation::ChatbotDisclosure));
    }

    #[test]
    fn test_chatbot_disclosure_obligation() {
        let system = AiSystemDescription::new(
            "support_chatbot".to_string(),
            "Virtual assistant chatbot".to_string(),
        )
        .add_transparency_obligation(TransparencyObligation::ChatbotDisclosure);

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert!(result
            .transparency_obligations
            .contains(&TransparencyObligation::ChatbotDisclosure));
    }

    #[test]
    fn test_classification_result_storage() {
        let system = create_test_system("stored_system");

        let mut classifier = EuAiActClassifier::new();
        classifier.classify_system(&system);

        let result = classifier.get_result("stored_system");
        assert!(result.is_some());
        assert_eq!(result.unwrap().system_name, "stored_system");
    }

    #[test]
    fn test_risk_tier_article_references() {
        assert_eq!(RiskTier::Unacceptable.article_reference(), "Article 5");
        assert_eq!(RiskTier::High.article_reference(), "Articles 6-7");
        assert_eq!(RiskTier::Limited.article_reference(), "Article 52");
        assert_eq!(RiskTier::Minimal.article_reference(), "No specific article");
    }

    #[test]
    fn test_risk_score_bounds() {
        // Test maximum risk scenario
        let max_risk = RiskScoringCriteria {
            impact_score: 1.0,
            likelihood_score: 1.0,
            autonomy_score: 1.0,
            transparency_score: 0.0,
            data_sensitivity_score: 1.0,
            oversight_score: 0.0,
        };
        let max_score = max_risk.calculate_risk_score();
        assert!(max_score <= 1.0);
        assert!(max_score >= 0.8); // Should be classified as Unacceptable

        // Test minimum risk scenario
        let min_risk = RiskScoringCriteria {
            impact_score: 0.0,
            likelihood_score: 0.0,
            autonomy_score: 0.0,
            transparency_score: 1.0,
            data_sensitivity_score: 0.0,
            oversight_score: 1.0,
        };
        let min_score = min_risk.calculate_risk_score();
        assert!(min_score >= 0.0);
        assert!(min_score <= 0.2); // Should be classified as Minimal
    }

    #[test]
    fn test_all_prohibited_practices_have_articles() {
        let practices = [
            ProhibitedPractice::SocialScoring,
            ProhibitedPractice::RealTimeBiometricIdentification,
            ProhibitedPractice::SubliminalManipulation,
            ProhibitedPractice::ExploitationOfVulnerabilities,
            ProhibitedPractice::InsuranceRiskAssessment,
            ProhibitedPractice::UntargetedFacialRecognitionDatabase,
            ProhibitedPractice::EmotionRecognitionWorkplaceEducation,
            ProhibitedPractice::BiometricCategorisationSensitive,
            ProhibitedPractice::AiInfluenceInElections,
        ];

        for practice in &practices {
            let article = practice.article_reference();
            assert!(article.starts_with("Article 5"));
        }
    }

    // ── RiskTier::requires_conformity_assessment ─────────────────────────

    #[test]
    fn test_risk_tier_requires_conformity_assessment() {
        assert!(RiskTier::Unacceptable.requires_conformity_assessment());
        assert!(RiskTier::High.requires_conformity_assessment());
        assert!(!RiskTier::Limited.requires_conformity_assessment());
        assert!(!RiskTier::Minimal.requires_conformity_assessment());
    }

    // ── RiskScoringCriteria::score_to_tier boundary values ───────────────

    #[test]
    fn test_score_to_tier_exact_boundaries() {
        assert_eq!(RiskScoringCriteria::score_to_tier(0.8), RiskTier::Unacceptable);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.6), RiskTier::High);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.3), RiskTier::Limited);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.29), RiskTier::Minimal);
    }

    #[test]
    fn test_score_to_tier_just_below_boundary() {
        assert_eq!(RiskScoringCriteria::score_to_tier(0.79), RiskTier::High);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.59), RiskTier::Limited);
        assert_eq!(RiskScoringCriteria::score_to_tier(0.29), RiskTier::Minimal);
    }

    // ── RiskScoringCriteria::Default ─────────────────────────────────────

    #[test]
    fn test_risk_scoring_criteria_default() {
        let criteria = RiskScoringCriteria::default();
        assert_eq!(criteria.impact_score, 0.5);
        assert_eq!(criteria.likelihood_score, 0.5);
        assert_eq!(criteria.autonomy_score, 0.5);
        assert_eq!(criteria.transparency_score, 0.5);
        assert_eq!(criteria.data_sensitivity_score, 0.5);
        assert_eq!(criteria.oversight_score, 0.5);
        // Default should produce a moderate score
        let score = criteria.calculate_risk_score();
        assert!(score > 0.3 && score < 0.6);
    }

    // ── AiSystemDescription builder methods ──────────────────────────────

    #[test]
    fn test_ai_system_description_new() {
        let system = AiSystemDescription::new("test".to_string(), "purpose".to_string());
        assert_eq!(system.name, "test");
        assert_eq!(system.purpose, "purpose");
        assert!(system.high_risk_categories.is_empty());
        assert!(system.uses_prohibited_practices.is_empty());
        assert!(system.transparency_obligations.is_empty());
        assert!(!system.has_technical_documentation);
        assert!(!system.has_quality_management);
        assert!(!system.has_human_oversight);
        assert!(!system.has_accuracy_robustness);
    }

    #[test]
    fn test_ai_system_description_builder_chain() {
        let system = AiSystemDescription::new("chain".to_string(), "test".to_string())
            .add_high_risk_category(HighRiskCategory::CriticalInfrastructure)
            .add_prohibited_practice(ProhibitedPractice::SocialScoring)
            .add_transparency_obligation(TransparencyObligation::ChatbotDisclosure)
            .set_risk_criteria(RiskScoringCriteria {
                impact_score: 0.9,
                ..Default::default()
            });

        assert_eq!(system.high_risk_categories.len(), 1);
        assert_eq!(system.uses_prohibited_practices.len(), 1);
        assert_eq!(system.transparency_obligations.len(), 1);
        assert_eq!(system.risk_criteria.impact_score, 0.9);
    }

    // ── EuAiActClassifier ────────────────────────────────────────────────

    #[test]
    fn test_classifier_default() {
        let classifier = EuAiActClassifier::default();
        assert!(classifier.list_results().is_empty());
    }

    #[test]
    fn test_classifier_get_result_nonexistent() {
        let classifier = EuAiActClassifier::new();
        assert!(classifier.get_result("nonexistent").is_none());
    }

    #[test]
    fn test_classifier_list_results_multiple() {
        let mut classifier = EuAiActClassifier::new();
        classifier.classify_system(&create_test_system("sys1"));
        classifier.classify_system(&create_test_system("sys2"));
        assert_eq!(classifier.list_results().len(), 2);
    }

    // ── Auto-detection: all categories ───────────────────────────────────

    #[test]
    fn test_auto_detection_critical_infrastructure() {
        let system = AiSystemDescription::new(
            "power_grid".to_string(),
            "AI for critical infrastructure energy grid management".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.contains(&HighRiskCategory::CriticalInfrastructure));
    }

    #[test]
    fn test_auto_detection_education() {
        let system = AiSystemDescription::new(
            "grading".to_string(),
            "AI system for student education assessment".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.contains(&HighRiskCategory::EducationVocationalTraining));
    }

    #[test]
    fn test_auto_detection_employment() {
        let system = AiSystemDescription::new(
            "hiring".to_string(),
            "AI for employment hiring and recruitment".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.contains(&HighRiskCategory::EmploymentSelfEmployment));
    }

    #[test]
    fn test_auto_detection_migration() {
        let system = AiSystemDescription::new(
            "border".to_string(),
            "AI system for migration asylum border control".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.contains(&HighRiskCategory::MigrationAsylumBorder));
    }

    #[test]
    fn test_auto_detection_justice() {
        let system = AiSystemDescription::new(
            "court".to_string(),
            "AI for judicial court decision support and democratic election".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.contains(&HighRiskCategory::JusticeDemocraticProcesses));
    }

    #[test]
    fn test_auto_detection_no_match() {
        let system = AiSystemDescription::new(
            "weather".to_string(),
            "Weather forecasting system".to_string(),
        );
        let classifier = EuAiActClassifier::new();
        let categories = classifier.detect_high_risk_categories(&system);
        assert!(categories.is_empty());
    }

    // ── Transparency auto-detection ──────────────────────────────────────

    #[test]
    fn test_transparency_detection_deepfake() {
        let system = AiSystemDescription::new(
            "deepfake".to_string(),
            "Deepfake synthetic media generation system".to_string(),
        );
        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);
        assert!(result
            .transparency_obligations
            .contains(&TransparencyObligation::DeepfakeLabeling));
    }

    #[test]
    fn test_transparency_detection_emotion_recognition() {
        let system = AiSystemDescription::new(
            "emotion".to_string(),
            "Emotion recognition and affect detection system".to_string(),
        );
        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);
        assert!(result
            .transparency_obligations
            .contains(&TransparencyObligation::EmotionRecognitionDisclosure));
    }

    #[test]
    fn test_transparency_detection_biometric_categorization() {
        let system = AiSystemDescription::new(
            "biocat".to_string(),
            "Biometric categorization and classification system".to_string(),
        );
        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);
        assert!(result
            .transparency_obligations
            .contains(&TransparencyObligation::BiometricCategorizationDisclosure));
    }

    // ── Compliance gaps for high-risk systems ────────────────────────────

    #[test]
    fn test_compliance_gaps_high_risk_no_documentation() {
        let mut system = create_test_system("high_no_docs");
        system = system.add_high_risk_category(HighRiskCategory::LawEnforcement);
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.7,
            likelihood_score: 0.6,
            autonomy_score: 0.5,
            transparency_score: 0.4,
            data_sensitivity_score: 0.7,
            oversight_score: 0.3,
        });
        // has_technical_documentation = false by default

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert_eq!(result.risk_tier, RiskTier::High);
        assert!(result.compliance_gaps.iter().any(|g| g.id == "TECH_DOC"));
        assert!(result.compliance_gaps.iter().any(|g| g.id == "QMS"));
        assert!(result.compliance_gaps.iter().any(|g| g.id == "OVERSIGHT"));
        assert!(result.compliance_gaps.iter().any(|g| g.id == "ACCURACY"));
    }

    #[test]
    fn test_compliance_gaps_high_risk_with_documentation() {
        let mut system = create_test_system("high_with_docs");
        system = system.add_high_risk_category(HighRiskCategory::LawEnforcement);
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.9,
            likelihood_score: 0.8,
            autonomy_score: 0.7,
            transparency_score: 0.2,
            data_sensitivity_score: 0.9,
            oversight_score: 0.1,
        });
        system.has_technical_documentation = true;
        system.has_quality_management = true;
        system.has_human_oversight = true;
        system.has_accuracy_robustness = true;

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert!(!result.compliance_gaps.iter().any(|g| g.id == "TECH_DOC"));
        assert!(!result.compliance_gaps.iter().any(|g| g.id == "QMS"));
        assert!(!result.compliance_gaps.iter().any(|g| g.id == "OVERSIGHT"));
        assert!(!result.compliance_gaps.iter().any(|g| g.id == "ACCURACY"));
    }

    #[test]
    fn test_compliance_gaps_prohibited_practice() {
        let system = create_test_system("prohibited_gaps")
            .add_prohibited_practice(ProhibitedPractice::SocialScoring);

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert!(result
            .compliance_gaps
            .iter()
            .any(|g| g.severity == GapSeverity::Critical));
    }

    // ── Recommendations per tier ─────────────────────────────────────────

    #[test]
    fn test_recommendations_unacceptable() {
        let system = create_test_system("unrec")
            .add_prohibited_practice(ProhibitedPractice::SocialScoring);

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert!(result
            .recommendations
            .iter()
            .any(|r| r.contains("IMMEDIATE ACTION")));
    }

    #[test]
    fn test_recommendations_high_risk_with_categories() {
        let mut system = create_test_system("high_rec");
        system = system.add_high_risk_category(HighRiskCategory::LawEnforcement);
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.7,
            likelihood_score: 0.6,
            autonomy_score: 0.5,
            transparency_score: 0.4,
            data_sensitivity_score: 0.7,
            oversight_score: 0.3,
        });

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        assert_eq!(result.risk_tier, RiskTier::High);
        assert!(result
            .recommendations
            .iter()
            .any(|r| r.contains("High-risk")));
        assert!(result
            .recommendations
            .iter()
            .any(|r| r.contains("EU database")));
    }

    #[test]
    fn test_recommendations_limited() {
        let mut system = create_test_system("limited_rec");
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.4,
            likelihood_score: 0.3,
            autonomy_score: 0.3,
            transparency_score: 0.7,
            data_sensitivity_score: 0.3,
            oversight_score: 0.6,
        });

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        if result.risk_tier == RiskTier::Limited {
            assert!(result
                .recommendations
                .iter()
                .any(|r| r.contains("Article 52")));
        }
    }

    #[test]
    fn test_recommendations_minimal() {
        let mut system = create_test_system("minimal_rec");
        system = system.set_risk_criteria(RiskScoringCriteria {
            impact_score: 0.1,
            likelihood_score: 0.1,
            autonomy_score: 0.1,
            transparency_score: 0.9,
            data_sensitivity_score: 0.1,
            oversight_score: 0.9,
        });

        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        if result.risk_tier == RiskTier::Minimal {
            assert!(result
                .recommendations
                .iter()
                .any(|r| r.contains("Minimal risk")));
        }
    }

    // ── GapSeverity ordering ─────────────────────────────────────────────

    #[test]
    fn test_gap_severity_ordering() {
        // Declaration order: Critical=0, Major=1, Minor=2, Info=3
        // So Critical < Major in Ord
        assert!(GapSeverity::Critical < GapSeverity::Major);
        assert!(GapSeverity::Major < GapSeverity::Minor);
        assert!(GapSeverity::Minor < GapSeverity::Info);
    }

    // ── Serde roundtrips ─────────────────────────────────────────────────

    #[test]
    fn test_risk_classification_result_serde() {
        let system = create_test_system("serde_test");
        let mut classifier = EuAiActClassifier::new();
        let result = classifier.classify_system(&system);

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RiskClassificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.system_name, "serde_test");
        assert_eq!(deserialized.risk_tier, result.risk_tier);
    }

    #[test]
    fn test_compliance_gap_serde() {
        let gap = ComplianceGap {
            id: "TEST_GAP".to_string(),
            description: "Test gap".to_string(),
            severity: GapSeverity::Major,
            article_reference: "Article 11".to_string(),
            remediation: "Fix it".to_string(),
        };
        let json = serde_json::to_string(&gap).unwrap();
        let deserialized: ComplianceGap = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "TEST_GAP");
        assert_eq!(deserialized.severity, GapSeverity::Major);
    }

    #[test]
    fn test_conformity_assessment_requirement_serde() {
        let requirements = vec![
            ConformityAssessmentRequirement::InternalAssessment,
            ConformityAssessmentRequirement::ThirdPartyAssessment,
            ConformityAssessmentRequirement::None,
            ConformityAssessmentRequirement::Prohibited,
        ];
        for req in requirements {
            let json = serde_json::to_string(&req).unwrap();
            let deserialized: ConformityAssessmentRequirement =
                serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", deserialized), format!("{:?}", req));
        }
    }

    #[test]
    fn test_high_risk_category_other_serde() {
        let category = HighRiskCategory::OtherHighRisk("custom category".to_string());
        assert_eq!(category.annex_reference(), "Article 6(1)");
        assert!(category.requires_conformity_assessment());

        let json = serde_json::to_string(&category).unwrap();
        let deserialized: HighRiskCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.annex_reference(), "Article 6(1)");
    }

    #[test]
    fn test_all_transparency_obligation_articles() {
        let obligations = vec![
            TransparencyObligation::ChatbotDisclosure,
            TransparencyObligation::DeepfakeLabeling,
            TransparencyObligation::EmotionRecognitionDisclosure,
            TransparencyObligation::BiometricCategorizationDisclosure,
            TransparencyObligation::GeneratedTextDisclosure,
            TransparencyObligation::HighRiskTransparency,
        ];
        for ob in obligations {
            assert!(!ob.article_reference().is_empty());
        }
    }

    // ── classify_system stores result ────────────────────────────────────

    #[test]
    fn test_classify_system_overwrites_previous() {
        let mut classifier = EuAiActClassifier::new();
        let system = create_test_system("overwrite_test");

        classifier.classify_system(&system);
        classifier.classify_system(&system);

        // Should still have exactly 1 result (overwritten)
        assert_eq!(classifier.list_results().len(), 1);
    }

    // ── RiskScoringCriteria: transparency/oversight inverse ──────────────

    #[test]
    fn test_risk_score_transparency_inverse() {
        // Higher transparency should LOWER risk
        let low_transparency = RiskScoringCriteria {
            transparency_score: 0.0,
            ..Default::default()
        };
        let high_transparency = RiskScoringCriteria {
            transparency_score: 1.0,
            ..Default::default()
        };
        assert!(low_transparency.calculate_risk_score() > high_transparency.calculate_risk_score());
    }

    #[test]
    fn test_risk_score_oversight_inverse() {
        // Higher oversight should LOWER risk
        let low_oversight = RiskScoringCriteria {
            oversight_score: 0.0,
            ..Default::default()
        };
        let high_oversight = RiskScoringCriteria {
            oversight_score: 1.0,
            ..Default::default()
        };
        assert!(low_oversight.calculate_risk_score() > high_oversight.calculate_risk_score());
    }
}
