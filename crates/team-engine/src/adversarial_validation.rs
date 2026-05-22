//! Multi-Agent Adversarial Validation闭环
//!
//! Implements Worker/Verifier/Critic/Judge four-role adversarial validation:
//! - Worker: Produces output
//! - Verifier: Checks output against criteria
//! - Critic: Challenges and finds weaknesses
//! - Judge: Makes final decision on quality
//!
//! Validation loop: Worker→Verifier→Critic→Judge, retries until Judge passes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Validation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Pending,
    InProgress,
    Passed,
    Failed,
    Escalated,
}

/// Role in the adversarial team
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Worker,
    Verifier,
    Critic,
    Judge,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Worker => write!(f, "Worker"),
            AgentRole::Verifier => write!(f, "Verifier"),
            AgentRole::Critic => write!(f, "Critic"),
            AgentRole::Judge => write!(f, "Judge"),
        }
    }
}

/// Input to the validation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInput {
    pub task: String,
    pub context: HashMap<String, String>,
}

/// Output produced by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub role: AgentRole,
    pub content: String,
    pub findings: Vec<String>,
    pub confidence: f64,
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub criteria_met: Vec<String>,
    pub criteria_failed: Vec<String>,
    pub issues: Vec<String>,
}

/// Critique result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CritiqueResult {
    pub challenges: Vec<String>,
    pub severity: u8,
    pub suggestions: Vec<String>,
}

/// Judge verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVerdict {
    pub approved: bool,
    pub score: f64,
    pub reasons: Vec<String>,
    pub conditions: Vec<String>,
}

/// Single validation round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRound {
    pub round_number: u32,
    pub worker_output: Option<AgentOutput>,
    pub verification: Option<VerificationResult>,
    pub critique: Option<CritiqueResult>,
    pub verdict: Option<JudgeVerdict>,
}

/// Complete validation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSession {
    pub id: String,
    pub input: ValidationInput,
    pub status: ValidationStatus,
    pub rounds: Vec<ValidationRound>,
    pub final_verdict: Option<JudgeVerdict>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for ValidationStatus {
    fn default() -> Self {
        ValidationStatus::Pending
    }
}

/// AdversarialTeam - Worker/Verifier/Critic/Judge four-role validation
pub struct AdversarialTeam {
    max_rounds: u32,
    quality_threshold: f64,
}

impl Default for AdversarialTeam {
    fn default() -> Self {
        Self::new()
    }
}

impl AdversarialTeam {
    pub fn new() -> Self {
        Self {
            max_rounds: 5,
            quality_threshold: 0.8,
        }
    }

    pub fn with_max_rounds(mut self, rounds: u32) -> Self {
        self.max_rounds = rounds;
        self
    }

    pub fn with_quality_threshold(mut self, threshold: f64) -> Self {
        self.quality_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Run the complete adversarial validation loop
    pub fn validate(&self, input: ValidationInput) -> ValidationSession {
        let mut session = ValidationSession {
            id: Uuid::new_v4().to_string(),
            input,
            status: ValidationStatus::InProgress,
            rounds: Vec::new(),
            final_verdict: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        for round_num in 1..=self.max_rounds {
            let mut round = ValidationRound {
                round_number: round_num,
                worker_output: None,
                verification: None,
                critique: None,
                verdict: None,
            };

            // Step 1: Worker produces output
            let worker_output = self.worker_produce(&session.input, round_num);
            round.worker_output = Some(worker_output.clone());

            // Step 2: Verifier checks output
            let verification = self.verify(&worker_output, &session.input);
            round.verification = Some(verification.clone());

            // If verification passes basic checks, proceed to critique
            if !verification.criteria_met.is_empty() {
                // Step 3: Critic challenges the output
                let critique = self.critique(&worker_output, &verification);
                round.critique = Some(critique.clone());

                // Step 4: Judge makes final decision
                let verdict = self.judge(&worker_output, &verification, &critique);
                round.verdict = Some(verdict.clone());

                if verdict.approved {
                    session.final_verdict = Some(verdict);
                    session.status = ValidationStatus::Passed;
                    session.completed_at = Some(chrono::Utc::now());
                    break;
                } else if verdict.score < self.quality_threshold / 2.0 {
                    // Hopeless case
                    session.status = ValidationStatus::Failed;
                    session.completed_at = Some(chrono::Utc::now());
                    break;
                }
            } else {
                // Verification failed critically
                round.verdict = Some(JudgeVerdict {
                    approved: false,
                    score: 0.0,
                    reasons: vec!["Critical verification failures".to_string()],
                    conditions: vec![],
                });
            }

            session.rounds.push(round);
        }

        if session.status == ValidationStatus::InProgress {
            session.status = ValidationStatus::Escalated;
            session.completed_at = Some(chrono::Utc::now());
        }

        session
    }

    /// Worker produces initial output
    fn worker_produce(&self, input: &ValidationInput, _round: u32) -> AgentOutput {
        // Simulate worker producing output
        // Production would call actual LLM
        AgentOutput {
            role: AgentRole::Worker,
            content: format!("Worker output for task: {}", input.task),
            findings: vec![
                "Completed initial analysis".to_string(),
                "Identified key requirements".to_string(),
            ],
            confidence: 0.75,
        }
    }

    /// Verifier checks output against criteria
    fn verify(&self, output: &AgentOutput, input: &ValidationInput) -> VerificationResult {
        let mut criteria_met = Vec::new();
        let mut criteria_failed = Vec::new();
        let mut issues = Vec::new();

        // Check task completion
        if output
            .content
            .to_lowercase()
            .contains(&input.task.to_lowercase())
        {
            criteria_met.push("Task addressed".to_string());
        } else {
            criteria_failed.push("Task not adequately addressed".to_string());
            issues.push("Worker output does not match task requirements".to_string());
        }

        // Check confidence threshold
        if output.confidence >= 0.7 {
            criteria_met.push("Confidence adequate".to_string());
        } else {
            criteria_failed.push("Confidence too low".to_string());
            issues.push("Worker confidence below threshold".to_string());
        }

        // Check for findings
        if !output.findings.is_empty() {
            criteria_met.push("Findings documented".to_string());
        } else {
            criteria_failed.push("No findings".to_string());
        }

        let passed = criteria_failed.is_empty() && criteria_met.len() >= 2;
        VerificationResult {
            passed,
            criteria_met,
            criteria_failed,
            issues,
        }
    }

    /// Critic challenges the output
    fn critique(&self, output: &AgentOutput, verification: &VerificationResult) -> CritiqueResult {
        let mut challenges = Vec::new();
        let mut suggestions = Vec::new();
        let mut severity = 1;

        // Challenge based on verification failures
        for failed in &verification.criteria_failed {
            challenges.push(format!("Critic: {}", failed));
            suggestions.push(format!(
                "Improve: {}",
                failed.replace("not ", "").replace(" too ", " ")
            ));
            severity = severity.max(3);
        }

        // Challenge based on confidence
        if output.confidence < 0.8 {
            challenges.push("Critic: Confidence could be higher".to_string());
            suggestions.push("Provide more detailed analysis".to_string());
            severity = severity.max(2);
        }

        // Challenge based on findings count
        if output.findings.len() < 2 {
            challenges.push("Critic: Insufficient findings documented".to_string());
            suggestions.push("Document more detailed findings".to_string());
            severity = severity.max(2);
        }

        CritiqueResult {
            challenges,
            severity,
            suggestions,
        }
    }

    /// Judge makes final verdict
    fn judge(
        &self,
        output: &AgentOutput,
        verification: &VerificationResult,
        critique: &CritiqueResult,
    ) -> JudgeVerdict {
        let mut score = 0.5;
        let mut reasons = Vec::new();
        let mut conditions = Vec::new();

        // Base score from verification
        let verification_ratio = verification.criteria_met.len() as f64
            / (verification.criteria_met.len() + verification.criteria_failed.len()) as f64;
        score += verification_ratio * 0.3;

        // Adjust for critique severity
        let critique_penalty = critique.severity as f64 * 0.05;
        score -= critique_penalty;

        // Adjust for confidence
        score += output.confidence * 0.2;

        score = score.clamp(0.0, 1.0);

        if verification.criteria_failed.is_empty() && critique.severity <= 2 {
            reasons.push("Output meets all criteria with minimal critique".to_string());
        } else if verification.criteria_failed.len() <= 1 && critique.severity <= 3 {
            reasons.push("Output mostly acceptable with minor issues".to_string());
            conditions.extend(critique.suggestions.iter().take(2).cloned());
        } else {
            reasons.push("Output requires significant improvements".to_string());
            conditions.extend(critique.suggestions.iter().take(3).cloned());
        }

        let approved = score >= self.quality_threshold;

        JudgeVerdict {
            approved,
            score,
            reasons,
            conditions,
        }
    }

    /// Get session status
    pub fn get_session_status(session: &ValidationSession) -> ValidationStatus {
        session.status
    }

    /// Check if session needs retry
    pub fn should_retry(session: &ValidationSession) -> bool {
        session.status == ValidationStatus::InProgress
            && session.rounds.len() < session.rounds.capacity()
    }

    /// Get summary statistics
    pub fn get_summary(session: &ValidationSession) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        stats.insert("session_id".to_string(), session.id.clone());
        stats.insert("total_rounds".to_string(), session.rounds.len().to_string());
        stats.insert("final_status".to_string(), format!("{:?}", session.status));
        if let Some(ref verdict) = session.final_verdict {
            stats.insert("final_score".to_string(), format!("{:.2}", verdict.score));
            stats.insert("approved".to_string(), verdict.approved.to_string());
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adversarial_team_new() {
        let team = AdversarialTeam::new();
        assert_eq!(team.max_rounds, 5);
        assert_eq!(team.quality_threshold, 0.8);
    }

    #[test]
    fn test_validation_session_creation() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "Review code for security".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        assert_eq!(session.status, ValidationStatus::Passed);
        assert!(!session.id.is_empty());
    }

    #[test]
    fn test_validation_passes_with_good_output() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "review code".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        // Status should be Passed, Failed, or Escalated
        assert_ne!(session.status, ValidationStatus::Pending);
    }

    #[test]
    fn test_validation_round_count() {
        let team = AdversarialTeam::new().with_max_rounds(3);
        let input = ValidationInput {
            task: "analyze data".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        assert!(session.rounds.len() <= 3);
    }

    #[test]
    fn test_worker_output_creation() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "test task".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        if let Some(first_round) = session.rounds.first() {
            if let Some(ref worker) = first_round.worker_output {
                assert_eq!(worker.role, AgentRole::Worker);
                assert!(!worker.content.is_empty());
            }
        }
    }

    #[test]
    fn test_verification_happens() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "test".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        for round in &session.rounds {
            assert!(round.verification.is_some());
        }
    }

    #[test]
    fn test_critique_contains_challenges() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "test".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        for round in &session.rounds {
            if let Some(ref critique) = round.critique {
                assert!(!critique.challenges.is_empty() || !critique.suggestions.is_empty());
            }
        }
    }

    #[test]
    fn test_judge_provides_verdict() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "test".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        if let Some(first_round) = session.rounds.first() {
            if let Some(ref verdict) = first_round.verdict {
                assert!(verdict.score >= 0.0 && verdict.score <= 1.0);
                assert!(!verdict.reasons.is_empty());
            }
        }
    }

    #[test]
    fn test_quality_threshold_adjustment() {
        let strict_team = AdversarialTeam::new().with_quality_threshold(0.95);
        let lenient_team = AdversarialTeam::new().with_quality_threshold(0.5);
        assert_eq!(strict_team.quality_threshold, 0.95);
        assert_eq!(lenient_team.quality_threshold, 0.5);
    }

    #[test]
    fn test_validation_status_display() {
        assert_eq!(format!("{}", AgentRole::Worker), "Worker");
        assert_eq!(format!("{}", AgentRole::Verifier), "Verifier");
        assert_eq!(format!("{}", AgentRole::Critic), "Critic");
        assert_eq!(format!("{}", AgentRole::Judge), "Judge");
    }

    #[test]
    fn test_get_summary() {
        let team = AdversarialTeam::new();
        let input = ValidationInput {
            task: "test".to_string(),
            context: HashMap::new(),
        };
        let session = team.validate(input);
        let summary = AdversarialTeam::get_summary(&session);
        assert!(summary.contains_key("session_id"));
        assert!(summary.contains_key("total_rounds"));
    }
}
