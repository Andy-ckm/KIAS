//! # Risk Assessment — GAMP 5 & ISO 14971 for AI Agents
//!
//! Structured risk assessment for AI/ML systems in GxP environments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use super::electronic_signature::ElectronicSignature;
pub use super::gamp_classification::GampCategory;

/// AI Agent risk classification per ISO 14971 / GAMP 5
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AIAgentRiskLevel {
    /// Low risk: minimal patient/safety impact
    ClassI,
    /// Medium risk: indirect impact
    ClassII,
    /// High risk: direct patient/safety impact
    ClassIII,
    /// Critical risk: life-critical decisions
    ClassIV,
}

/// GxP regulatory authority context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GxPRegulatorContext {
    FDA,
    EMA,
    PMDA,
    MHRA,
    HealthCanada,
}

/// A single hazard scenario identified in risk analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardScenario {
    /// Unique hazard ID
    pub id: String,
    /// Description of the hazard
    pub description: String,
    /// Severity: 1 (negligible) to 5 (catastrophic)
    pub severity: u8,
    /// Probability of occurrence: 1 (rare) to 5 (certain)
    pub probability: u8,
    /// Detectability: 1 (always detectable) to 5 (undetectable)
    pub detectability: u8,
    /// Risk Priority Number = severity × probability × detectability
    pub rpn: u32,
    /// Mitigation measure applied
    pub mitigation: String,
    /// Residual RPN after mitigation
    pub residual_rpn: u32,
}

impl HazardScenario {
    /// Create a new hazard scenario and compute RPN.
    pub fn new(
        id: &str,
        description: &str,
        severity: u8,
        probability: u8,
        detectability: u8,
        mitigation: &str,
    ) -> Self {
        let rpn = (severity as u32) * (probability as u32) * (detectability as u32);
        Self {
            id: id.to_string(),
            description: description.to_string(),
            severity: severity.clamp(1, 5),
            probability: probability.clamp(1, 5),
            detectability: detectability.clamp(1, 5),
            rpn,
            mitigation: mitigation.to_string(),
            residual_rpn: rpn, // initially same, updated post-mitigation
        }
    }

    /// Update RPN after mitigation is applied.
    pub fn apply_mitigation(&mut self, residual_rpn: u32) {
        self.residual_rpn = residual_rpn;
    }
}

/// Full risk assessment for an AI agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Agent being assessed
    pub agent_id: String,
    /// GAMP 5 category
    pub gamp_category: GampCategory,
    /// Computed risk level
    pub risk_level: AIAgentRiskLevel,
    /// All identified hazards
    pub hazard_analysis: Vec<HazardScenario>,
    /// Overall risk score (sum of RPNs)
    pub risk_score: f64,
    /// Mitigations applied
    pub mitigation_applied: Vec<String>,
    /// Residual risk score after mitigations
    pub residual_risk: f64,
    /// When assessed
    pub assessment_date: DateTime<Utc>,
    /// Who performed the assessment
    pub assessor_id: String,
    /// Approval signatures
    pub approval_signatures: Vec<ElectronicSignature>,
    /// When next review is due
    pub review_due_date: DateTime<Utc>,
    /// Regulatory context
    pub regulator: GxPRegulatorContext,
    /// Overall acceptable risk threshold
    pub acceptable_risk_threshold: u32,
}

impl RiskAssessment {
    /// Create a new assessment.
    pub fn new(
        agent_id: &str,
        gamp_category: GampCategory,
        assessor_id: &str,
        regulator: GxPRegulatorContext,
    ) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            gamp_category,
            risk_level: AIAgentRiskLevel::ClassI,
            hazard_analysis: Vec::new(),
            risk_score: 0.0,
            mitigation_applied: Vec::new(),
            residual_risk: 0.0,
            assessment_date: Utc::now(),
            assessor_id: assessor_id.to_string(),
            approval_signatures: Vec::new(),
            review_due_date: Utc::now(),
            regulator,
            acceptable_risk_threshold: 100,
        }
    }

    /// Add a hazard and recompute risk score.
    pub fn add_hazard(&mut self, hazard: HazardScenario) {
        self.risk_score += hazard.rpn as f64;
        self.hazard_analysis.push(hazard);
    }

    /// Determine risk level from total RPN.
    pub fn compute_risk_level(&self) -> AIAgentRiskLevel {
        let total_rpn: u32 = self.hazard_analysis.iter().map(|h| h.rpn).sum();
        RiskScorer::risk_level_from_score(total_rpn)
    }

    /// Check if residual risk is acceptable.
    pub fn is_residual_acceptable(&self) -> bool {
        let total_residual: u32 = self.hazard_analysis.iter().map(|h| h.residual_rpn).sum();
        total_residual <= self.acceptable_risk_threshold
    }

    /// Add approval signature.
    pub fn add_approval(&mut self, sig: ElectronicSignature) {
        self.approval_signatures.push(sig);
    }

    /// Years until review due.
    pub fn review_due_in_years(&self) -> i64 {
        (self.review_due_date - Utc::now()).num_days() / 365
    }
}

/// Risk scoring engine
pub struct RiskScorer {
    /// Historical risk data for reference
    historical: HashMap<String, Vec<u32>>,
}

impl RiskScorer {
    pub fn new() -> Self {
        Self {
            historical: HashMap::new(),
        }
    }

    /// Compute risk level from a single RPN value.
    pub fn risk_level_from_score(rpn: u32) -> AIAgentRiskLevel {
        match rpn {
            0..=20 => AIAgentRiskLevel::ClassI,
            21..=50 => AIAgentRiskLevel::ClassII,
            51..=100 => AIAgentRiskLevel::ClassIII,
            _ => AIAgentRiskLevel::ClassIV,
        }
    }

    /// Perform full risk assessment for an agent.
    pub fn assess(
        &self,
        agent_id: &str,
        category: GampCategory,
        hazards: Vec<HazardScenario>,
    ) -> RiskAssessment {
        let mut assessment = RiskAssessment::new(
            agent_id,
            category,
            "risk-assessor",
            GxPRegulatorContext::FDA,
        );

        for h in hazards {
            assessment.add_hazard(h);
        }

        assessment.risk_level = assessment.compute_risk_level();

        // Compute residual risk (assume mitigations reduce RPN by ~50% on average)
        let total_original: f64 = assessment
            .hazard_analysis
            .iter()
            .map(|h| h.rpn as f64)
            .sum();
        let mitigation_factor = match assessment.risk_level {
            AIAgentRiskLevel::ClassIV => 0.3,
            AIAgentRiskLevel::ClassIII => 0.4,
            AIAgentRiskLevel::ClassII => 0.5,
            AIAgentRiskLevel::ClassI => 0.6,
        };
        assessment.residual_risk = total_original * mitigation_factor;

        // Apply mitigations based on risk level
        match assessment.risk_level {
            AIAgentRiskLevel::ClassIV => {
                assessment
                    .mitigation_applied
                    .push("Continuous human oversight".to_string());
                assessment
                    .mitigation_applied
                    .push("Real-time monitoring".to_string());
                assessment
                    .mitigation_applied
                    .push("Fail-safe fallback".to_string());
            }
            AIAgentRiskLevel::ClassIII => {
                assessment
                    .mitigation_applied
                    .push("Human-in-the-loop review".to_string());
                assessment
                    .mitigation_applied
                    .push("Periodic validation".to_string());
            }
            AIAgentRiskLevel::ClassII => {
                assessment
                    .mitigation_applied
                    .push("Automated alerting".to_string());
            }
            AIAgentRiskLevel::ClassI => {
                assessment
                    .mitigation_applied
                    .push("Standard monitoring".to_string());
            }
        }

        assessment
    }

    /// Calculate residual risk score after mitigations.
    pub fn calculate_residual_risk(&self, original: &[HazardScenario]) -> f64 {
        let total: f64 = original.iter().map(|h| h.residual_rpn as f64).sum();
        // Assume mitigations reduce residual to 30-60% of original
        total * 0.45
    }

    /// Validate hazard inputs.
    pub fn validate_hazard(&self, severity: u8, probability: u8, detectability: u8) -> bool {
        severity >= 1
            && severity <= 5
            && probability >= 1
            && probability <= 5
            && detectability >= 1
            && detectability <= 5
    }
}

impl Default for RiskScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpn_calculation() {
        let h = HazardScenario::new(
            "H1",
            "AI misdiagnoses patient",
            5,
            3,
            2,
            "Require human review",
        );
        assert_eq!(h.rpn, 30);
    }

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(
            RiskScorer::risk_level_from_score(10),
            AIAgentRiskLevel::ClassI
        );
        assert_eq!(
            RiskScorer::risk_level_from_score(25),
            AIAgentRiskLevel::ClassII
        );
        assert_eq!(
            RiskScorer::risk_level_from_score(75),
            AIAgentRiskLevel::ClassIII
        );
        assert_eq!(
            RiskScorer::risk_level_from_score(150),
            AIAgentRiskLevel::ClassIV
        );
    }

    #[test]
    fn test_residual_risk_calculation() {
        let scorer = RiskScorer::new();
        let hazards = vec![
            HazardScenario::new("H1", "Hazard 1", 5, 4, 3, "Mitigation A"),
            HazardScenario::new("H2", "Hazard 2", 3, 2, 2, "Mitigation B"),
        ];
        let residual = scorer.calculate_residual_risk(&hazards);
        let expected_original: f64 = hazards.iter().map(|h| h.rpn as f64).sum();
        assert!(residual < expected_original);
    }

    #[test]
    fn test_hazard_clamping() {
        // Values > 5 should be clamped
        let h = HazardScenario::new("H1", "Test", 10, 10, 10, "Mitigation");
        assert_eq!(h.severity, 5);
        assert_eq!(h.probability, 5);
        assert_eq!(h.detectability, 5);
    }

    #[test]
    fn test_assessment_with_multiple_hazards() {
        let scorer = RiskScorer::new();
        let hazards = vec![
            HazardScenario::new("H1", "Hazard 1", 4, 3, 2, "Mitigation 1"),
            HazardScenario::new("H2", "Hazard 2", 3, 4, 3, "Mitigation 2"),
            HazardScenario::new("H3", "Hazard 3", 5, 2, 1, "Mitigation 3"),
        ];
        let assessment = scorer.assess("diagnostic-agent", GampCategory::AIModel, hazards);
        assert_eq!(assessment.hazard_analysis.len(), 3);
        assert!(assessment.risk_score > 0.0);
    }

    #[test]
    fn test_residual_acceptable() {
        let mut assessment = RiskAssessment::new(
            "agent-1",
            GampCategory::AIModel,
            "assessor",
            GxPRegulatorContext::FDA,
        );
        assessment.add_hazard(HazardScenario::new("H1", "Test", 2, 2, 2, "Mitigation"));
        assessment.acceptable_risk_threshold = 100;
        assert!(assessment.is_residual_acceptable());
    }

    #[test]
    fn test_validate_hazard_inputs() {
        let scorer = RiskScorer::new();
        assert!(scorer.validate_hazard(3, 3, 3));
        assert!(!scorer.validate_hazard(0, 3, 3)); // severity too low
        assert!(!scorer.validate_hazard(3, 10, 3)); // probability too high
    }

    #[test]
    fn test_multiple_regulators() {
        let regulators = [
            GxPRegulatorContext::FDA,
            GxPRegulatorContext::EMA,
            GxPRegulatorContext::PMDA,
            GxPRegulatorContext::MHRA,
            GxPRegulatorContext::HealthCanada,
        ];
        for reg in regulators {
            let assessment =
                RiskAssessment::new("agent-1", GampCategory::CustomSoftware, "assessor", reg);
            assert_eq!(assessment.regulator, reg);
        }
    }
}
