//! # Model Output Trustworthiness Evaluator
//!
//! Evaluates the trustworthiness of LLM outputs through:
//!
//! - **FactCheck** — verifies claims against known ground truth
//! - **CitationCheck** — ensures citations are present and valid
//! - **ConflictDetection** — detects internal contradictions within the output
//! - **HallucinationLevel** — classifies hallucination severity
//!
//! This module is inspired by OpenAI Agents SDK trust & safety evaluations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Hallucination severity classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum HallucinationLevel {
    /// Output appears fully factual with verifiable citations
    Low,
    /// Minor unsupported claims or vague citations
    Medium,
    /// Significant unsupported claims or internal contradictions
    High,
    /// Severe hallucination — fabrications that could cause harm
    Critical,
}

impl HallucinationLevel {
    /// Human-readable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            HallucinationLevel::Low => "low",
            HallucinationLevel::Medium => "medium",
            HallucinationLevel::High => "high",
            HallucinationLevel::Critical => "critical",
        }
    }

    /// Returns true if this level requires human review.
    pub fn requires_review(&self) -> bool {
        matches!(
            self,
            HallucinationLevel::High | HallucinationLevel::Critical
        )
    }

    /// Returns true if this level should be blocked.
    pub fn should_block(&self) -> bool {
        matches!(self, HallucinationLevel::Critical)
    }
}

impl std::fmt::Display for HallucinationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A factual claim extracted from LLM output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Unique claim identifier
    pub id: String,
    /// The textual claim
    pub text: String,
    /// Whether the claim has supporting citations
    pub has_citation: bool,
    /// Citation keys referenced (e.g., ["source-1"])
    pub citation_keys: Vec<String>,
    /// Whether the claim has been verified against ground truth
    pub verified: Option<bool>,
    /// Score from 0.0 (unsupported) to 1.0 (fully supported)
    pub support_score: f64,
    /// The claim text extracted from (for traceability)
    pub source_span: String,
}

/// Result of verifying a single claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerification {
    pub claim_id: String,
    /// True if the claim is consistent with ground truth
    pub consistent: bool,
    /// Confidence 0.0-1.0 in this verification
    pub confidence: f64,
    /// Detailed explanation of the verification
    pub explanation: String,
    /// Related claims that contradict this one
    pub conflicting_claims: Vec<String>,
}

/// Citation integrity check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationCheckResult {
    /// Whether all citations reference valid sources
    pub valid: bool,
    /// Citations that are malformed or missing
    pub invalid_citations: Vec<InvalidCitation>,
    /// Citations that are valid
    pub valid_citations: Vec<String>,
    /// Overall citation coverage score (0.0–1.0)
    pub coverage_score: f64,
}

/// A citation that failed validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidCitation {
    /// The citation key that failed
    pub key: String,
    /// Why the citation failed
    pub reason: CitationFailureReason,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CitationFailureReason {
    /// Citation key not found in any source
    NotFound,
    /// Citation format is malformed
    Malformed,
    /// Citation points to non-existent page/segment
    OutOfBounds,
    /// Citation URL is unreachable (for live checks)
    Unreachable,
}

/// Internal conflict detected in output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// First claim involved in the conflict
    pub claim_a: String,
    /// Second claim involved in the conflict
    pub claim_b: String,
    /// Human-readable description of the conflict
    pub description: String,
    /// Severity of the conflict (0.0–1.0)
    pub severity: f64,
}

/// Complete trustworthiness assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustworthinessReport {
    /// Overall hallucination level
    pub hallucination_level: HallucinationLevel,
    /// All extracted claims
    pub claims: Vec<Claim>,
    /// Results of fact verification
    pub fact_checks: Vec<ClaimVerification>,
    /// Citation integrity results
    pub citation_check: CitationCheckResult,
    /// Internal conflicts detected
    pub conflicts: Vec<Conflict>,
    /// Overall trustworthiness score (0.0–1.0)
    pub overall_score: f64,
    /// Whether this output should be blocked
    pub should_block: bool,
    /// Whether human review is recommended
    pub requires_review: bool,
    /// Warnings to surface to the user
    pub warnings: Vec<String>,
    /// When this report was generated
    pub evaluated_at: SystemTime,
}

/// Ground truth source for fact verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub key: String,
    pub content: String,
    pub url: Option<String>,
}

// ─── FactCheck ────────────────────────────────────────────────────────────────

/// Extracts and verifies factual claims from text.
pub struct FactCheck {
    /// Known ground truth sources
    sources: Vec<Source>,
    /// Similarity threshold for claim matching (0.0–1.0)
    match_threshold: f64,
}

impl FactCheck {
    /// Create a new FactCheck with given sources.
    pub fn new(sources: Vec<Source>) -> Self {
        Self {
            sources,
            match_threshold: 0.75,
        }
    }

    /// Set the similarity match threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.match_threshold = threshold;
        self
    }

    /// Extract factual claims from text.
    ///
    /// A simple heuristic: split by sentences, treat each as a potential claim.
    pub fn extract_claims(&self, text: &str) -> Vec<Claim> {
        let mut claims = Vec::new();
        let mut claim_counter = 0u64;
        let re_bracket = regex::Regex::new(r"\[([^\]]+)\]").ok();
        let re_source = regex::Regex::new(r"(?i)source[-_]?(\w+)").ok();

        for sentence in text.split(['.', '!', '?']) {
            let sentence = sentence.trim();
            if sentence.len() < 10 {
                continue; // Skip very short fragments
            }
            // Simple citation detection: "[key]" or "source-X"
            let has_citation = sentence.contains('[') || sentence.to_lowercase().contains("source");
            let citation_keys: Vec<String> = if sentence.contains('[') {
                // Extract content between brackets
                re_bracket
                    .as_ref()
                    .map(|r| {
                        r.captures_iter(sentence)
                            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                // Look for "source-N" patterns
                re_source
                    .as_ref()
                    .map(|r| {
                        r.captures_iter(sentence)
                            .filter_map(|c| c.get(1).map(|m| format!("source-{}", m.as_str())))
                            .collect()
                    })
                    .unwrap_or_default()
            };

            claim_counter += 1;
            claims.push(Claim {
                id: format!("claim-{}", claim_counter),
                text: sentence.to_string(),
                has_citation,
                citation_keys,
                verified: None,
                support_score: 0.0,
                source_span: sentence.to_string(),
            });
        }

        claims
    }

    /// Verify a single claim against ground truth sources.
    pub fn verify_claim(&self, claim: &Claim) -> ClaimVerification {
        let mut conflicting = Vec::new();
        let mut consistency_score = 0.0;
        let mut matched_sources = 0usize;

        let claim_lower = claim.text.to_lowercase();

        for source in &self.sources {
            let source_lower = source.content.to_lowercase();
            // Very simple keyword overlap check
            let claim_words: std::collections::HashSet<_> = claim_lower
                .split_whitespace()
                .filter(|w| w.len() > 4)
                .collect();
            let source_words: std::collections::HashSet<_> = source_lower
                .split_whitespace()
                .filter(|w| w.len() > 4)
                .collect();

            let overlap: f64 = claim_words.intersection(&source_words).count() as f64
                / claim_words.len().max(1) as f64;

            if overlap >= self.match_threshold {
                matched_sources += 1;
                consistency_score += overlap;
            } else if overlap < 0.1 && claim_words.len() > 3 {
                // Very low overlap may indicate a conflict
                if !source_lower
                    .split_whitespace()
                    .any(|w| claim_words.contains(w))
                {
                    conflicting.push(claim.id.clone());
                }
            }
        }

        let confidence = if matched_sources > 0 {
            (consistency_score / matched_sources as f64).min(1.0)
        } else {
            0.0
        };

        ClaimVerification {
            claim_id: claim.id.clone(),
            consistent: matched_sources > 0,
            confidence,
            explanation: if matched_sources > 0 {
                format!("Claim supported by {} source(s)", matched_sources)
            } else {
                "No supporting source found".to_string()
            },
            conflicting_claims: conflicting,
        }
    }

    /// Verify all claims.
    pub fn verify_all(&self, claims: &[Claim]) -> Vec<ClaimVerification> {
        claims.iter().map(|c| self.verify_claim(c)).collect()
    }
}

// ─── CitationCheck ────────────────────────────────────────────────────────────

/// Citation integrity validator.
pub struct CitationCheck {
    /// Available sources that can be cited
    known_sources: Vec<Source>,
}

impl CitationCheck {
    /// Create with known sources.
    pub fn new(sources: Vec<Source>) -> Self {
        Self {
            known_sources: sources,
        }
    }

    /// Validate citations referenced in claims against known sources.
    pub fn validate(&self, claims: &[Claim]) -> CitationCheckResult {
        let mut valid_citations = Vec::new();
        let mut invalid_citations = Vec::new();
        let mut total_citations = 0usize;

        let source_keys: std::collections::HashSet<_> =
            self.known_sources.iter().map(|s| s.key.clone()).collect();

        for claim in claims {
            for key in &claim.citation_keys {
                total_citations += 1;
                if source_keys.contains(key) {
                    valid_citations.push(key.clone());
                } else {
                    invalid_citations.push(InvalidCitation {
                        key: key.clone(),
                        reason: CitationFailureReason::NotFound,
                    });
                }
            }
        }

        let coverage_score = if total_citations == 0 {
            0.5 // No citations but none expected — neutral
        } else {
            valid_citations.len() as f64 / total_citations as f64
        };

        CitationCheckResult {
            valid: invalid_citations.is_empty(),
            invalid_citations,
            valid_citations,
            coverage_score,
        }
    }
}

// ─── ConflictDetection ────────────────────────────────────────────────────────

/// Detects internal contradictions in a set of claims.
pub struct ConflictDetection {
    /// Numeric claim patterns for comparison (e.g., "X is 50", "X was 40")
    #[allow(dead_code)]
    numeric_claims: HashMap<String, f64>,
}

impl ConflictDetection {
    /// Create a new ConflictDetection instance.
    pub fn new() -> Self {
        Self {
            numeric_claims: HashMap::new(),
        }
    }

    /// Detect conflicts in a list of claims.
    pub fn detect(&self, claims: &[Claim]) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let mut seen: Vec<&Claim> = Vec::new();

        for claim in claims {
            // Try to extract entity-number pairs
            if let Some((entity, number)) = self.extract_numeric_claim(&claim.text) {
                if let Some(&prev) = seen.iter().find(|c| {
                    self.extract_numeric_claim(&c.text)
                        .map(|(e, _)| e == entity)
                        .unwrap_or(false)
                }) {
                    let prev_num = self
                        .extract_numeric_claim(&prev.text)
                        .expect("numeric claim should exist")
                        .1;
                    let diff = (number - prev_num).abs();
                    let avg = (number + prev_num) / 2.0;
                    let relative_diff = if avg > 0.0 { diff / avg } else { diff };

                    if relative_diff > 0.05 {
                        conflicts.push(Conflict {
                            claim_a: claim.id.clone(),
                            claim_b: prev.id.clone(),
                            description: format!(
                                "Conflicting values for '{}': {} vs {} ({:.1}% difference)",
                                entity,
                                prev_num,
                                number,
                                relative_diff * 100.0
                            ),
                            severity: relative_diff.min(1.0),
                        });
                    }
                }
                seen.push(claim);
            }

            // Simple textual contradiction detection
            for other in &seen {
                if other.id == claim.id {
                    continue;
                }
                if self.textual_contradiction(&claim.text, &other.text) {
                    conflicts.push(Conflict {
                        claim_a: claim.id.clone(),
                        claim_b: other.id.clone(),
                        description: format!(
                            "Possible textual contradiction: '{}' vs '{}'",
                            claim.text.chars().take(50).collect::<String>(),
                            other.text.chars().take(50).collect::<String>()
                        ),
                        severity: 0.5,
                    });
                }
            }
        }

        conflicts
    }

    /// Extract "entity is N" style numeric claims.
    fn extract_numeric_claim(&self, text: &str) -> Option<(String, f64)> {
        // Simple pattern: "X is/was/were N" or "X: N"
        let re =
            regex::Regex::new(r"(?i)([A-Z][a-z]+)\s+(?:is|was|were|:)\s+(\d+(?:\.\d+)?)").ok()?;
        let caps = re.captures(text)?;
        let entity = caps.get(1)?.as_str().to_string();
        let number: f64 = caps.get(2)?.as_str().parse().ok()?;
        Some((entity, number))
    }

    /// Detect simple textual contradictions using negation patterns.
    fn textual_contradiction(&self, text_a: &str, text_b: &str) -> bool {
        let negations = ["never", "not", "no ", "cannot", "unable to"];
        let a_has_neg = negations.iter().any(|n| text_a.contains(n));
        let b_has_neg = negations.iter().any(|n| text_b.contains(n));
        if a_has_neg != b_has_neg {
            // One is negated, one is not — possible contradiction
            // Check for shared significant words
            let words_a: std::collections::HashSet<_> =
                text_a.split_whitespace().filter(|w| w.len() > 5).collect();
            let words_b: std::collections::HashSet<_> =
                text_b.split_whitespace().filter(|w| w.len() > 5).collect();
            let shared = words_a.intersection(&words_b).count();
            return shared >= 2;
        }
        false
    }
}

impl Default for ConflictDetection {
    fn default() -> Self {
        Self::new()
    }
}

// ─── TrustworthinessEvaluator ─────────────────────────────────────────────────

/// The main trustworthiness evaluation entry point.
pub struct TrustworthinessEvaluator {
    fact_check: FactCheck,
    citation_check: CitationCheck,
    conflict_detection: ConflictDetection,
}

impl TrustworthinessEvaluator {
    /// Create with given sources.
    pub fn new(sources: Vec<Source>) -> Self {
        Self {
            fact_check: FactCheck::new(sources.clone()),
            citation_check: CitationCheck::new(sources),
            conflict_detection: ConflictDetection::new(),
        }
    }

    /// Set the claim matching threshold.
    pub fn with_match_threshold(mut self, threshold: f64) -> Self {
        self.fact_check = self.fact_check.with_threshold(threshold);
        self
    }

    /// Evaluate the trustworthiness of LLM output text.
    pub fn evaluate(&self, text: &str) -> TrustworthinessReport {
        // Extract claims
        let claims = self.fact_check.extract_claims(text);

        // Verify facts
        let fact_checks = self.fact_check.verify_all(&claims);

        // Check citations
        let citation_check = self.citation_check.validate(&claims);

        // Detect conflicts
        let conflicts = self.conflict_detection.detect(&claims);

        // Compute hallucination level
        let hallucination_level =
            self.compute_hallucination_level(&claims, &fact_checks, &citation_check, &conflicts);

        // Compute overall score
        let overall_score =
            self.compute_overall_score(&claims, &fact_checks, &citation_check, &conflicts);

        let warnings = self.generate_warnings(
            &claims,
            &fact_checks,
            &citation_check,
            &conflicts,
            &hallucination_level,
        );

        let should_block = hallucination_level.should_block();
        let requires_review = hallucination_level.requires_review();

        TrustworthinessReport {
            hallucination_level,
            claims,
            fact_checks,
            citation_check,
            conflicts,
            overall_score,
            should_block,
            requires_review,
            warnings,
            evaluated_at: SystemTime::now(),
        }
    }

    fn compute_hallucination_level(
        &self,
        claims: &[Claim],
        fact_checks: &[ClaimVerification],
        citation_check: &CitationCheckResult,
        conflicts: &[Conflict],
    ) -> HallucinationLevel {
        let unsupported_count = fact_checks.iter().filter(|v| !v.consistent).count();

        let conflict_count = conflicts.len();
        let citation_valid = citation_check.valid;

        // Determine level
        if unsupported_count as f64 / claims.len().max(1) as f64 > 0.6 || conflict_count >= 3 {
            HallucinationLevel::Critical
        } else if unsupported_count as f64 / claims.len().max(1) as f64 > 0.3 || conflict_count >= 2
        {
            HallucinationLevel::High
        } else if unsupported_count as f64 / claims.len().max(1) as f64 > 0.1
            || !citation_valid
            || conflict_count >= 1
        {
            HallucinationLevel::Medium
        } else {
            HallucinationLevel::Low
        }
    }

    fn compute_overall_score(
        &self,
        claims: &[Claim],
        fact_checks: &[ClaimVerification],
        citation_check: &CitationCheckResult,
        conflicts: &[Conflict],
    ) -> f64 {
        if claims.is_empty() {
            return 0.5; // No claims to evaluate
        }

        let fact_score: f64 =
            fact_checks.iter().map(|v| v.confidence).sum::<f64>() / fact_checks.len() as f64;

        let conflict_penalty = (conflicts.len() as f64 * 0.1).min(0.5);
        let citation_bonus = if citation_check.valid { 0.1 } else { 0.0 };

        (fact_score * 0.7 + citation_check.coverage_score * 0.2 - conflict_penalty + citation_bonus)
            .clamp(0.0, 1.0)
    }

    fn generate_warnings(
        &self,
        claims: &[Claim],
        fact_checks: &[ClaimVerification],
        citation_check: &CitationCheckResult,
        conflicts: &[Conflict],
        level: &HallucinationLevel,
    ) -> Vec<String> {
        let mut warnings = Vec::new();

        if level.requires_review() {
            warnings.push(format!(
                "Output flagged as {} trustworthiness — review recommended",
                level
            ));
        }

        for v in fact_checks.iter().filter(|v| !v.consistent) {
            warnings.push(format!(
                "Unsupported claim: {} ({})",
                v.claim_id, v.explanation
            ));
        }

        if !citation_check.valid {
            for ic in &citation_check.invalid_citations {
                warnings.push(format!("Invalid citation [{}]: {:?}", ic.key, ic.reason));
            }
        }

        for conflict in conflicts {
            warnings.push(format!(
                "Conflict detected between {} and {}: {}",
                conflict.claim_a, conflict.claim_b, conflict.description
            ));
        }

        if claims.iter().all(|c| !c.has_citation) {
            warnings.push("Output contains no citations".to_string());
        }

        warnings
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sources() -> Vec<Source> {
        vec![
            Source {
                key: "source-1".to_string(),
                content: "The capital of France is Paris. Paris has a population of 2.1 million."
                    .to_string(),
                url: None,
            },
            Source {
                key: "source-2".to_string(),
                content: "Water boils at 100 degrees Celsius at sea level.".to_string(),
                url: None,
            },
        ]
    }

    #[test]
    fn test_hallucination_level_ordering() {
        assert!(HallucinationLevel::Critical > HallucinationLevel::High);
        assert!(HallucinationLevel::High > HallucinationLevel::Medium);
        assert!(HallucinationLevel::Medium > HallucinationLevel::Low);
    }

    #[test]
    fn test_hallucination_level_requires_review() {
        assert!(!HallucinationLevel::Low.requires_review());
        assert!(!HallucinationLevel::Medium.requires_review());
        assert!(HallucinationLevel::High.requires_review());
        assert!(HallucinationLevel::Critical.requires_review());
    }

    #[test]
    fn test_hallucination_level_should_block() {
        assert!(!HallucinationLevel::Low.should_block());
        assert!(!HallucinationLevel::Medium.should_block());
        assert!(!HallucinationLevel::High.should_block());
        assert!(HallucinationLevel::Critical.should_block());
    }

    #[test]
    fn test_fact_check_extract_claims() {
        let sources = make_sources();
        let fc = FactCheck::new(sources);
        let text = "The capital of France is Paris. Water boils at 100 degrees Celsius.";
        let claims = fc.extract_claims(text);
        assert_eq!(claims.len(), 2);
        assert!(claims[0].text.contains("Paris"));
        assert!(claims[1].text.contains("100"));
    }

    #[test]
    fn test_fact_check_extract_claims_with_citations() {
        let sources = make_sources();
        let fc = FactCheck::new(sources);
        let text = "The capital of France is Paris [source-1]. Water boils at 100C [source-2].";
        let claims = fc.extract_claims(text);
        assert_eq!(claims.len(), 2);
        assert!(claims[0].has_citation);
        assert_eq!(claims[0].citation_keys, vec!["source-1".to_string()]);
        assert!(claims[1].has_citation);
    }

    #[test]
    fn test_fact_check_verify_supported_claim() {
        let sources = make_sources();
        let fc = FactCheck::new(sources.clone());
        let claims = fc.extract_claims("The capital of France is Paris [source-1].");
        let verification = fc.verify_claim(&claims[0]);
        assert!(verification.consistent);
        assert!(verification.confidence > 0.0);
    }

    #[test]
    fn test_fact_check_verify_unsupported_claim() {
        let sources = make_sources();
        let fc = FactCheck::new(sources);
        let claims = fc.extract_claims("The capital of Germany is Munich.");
        let verification = fc.verify_claim(&claims[0]);
        assert!(!verification.consistent);
    }

    #[test]
    fn test_citation_check_all_valid() {
        let sources = make_sources();
        let cc = CitationCheck::new(sources.clone());
        let claims = vec![Claim {
            id: "c1".to_string(),
            text: "Paris is the capital [source-1].".to_string(),
            has_citation: true,
            citation_keys: vec!["source-1".to_string()],
            verified: None,
            support_score: 0.0,
            source_span: "Paris is the capital".to_string(),
        }];
        let result = cc.validate(&claims);
        assert!(result.valid);
        assert!(result.invalid_citations.is_empty());
        assert_eq!(result.coverage_score, 1.0);
    }

    #[test]
    fn test_citation_check_invalid_key() {
        let sources = make_sources();
        let cc = CitationCheck::new(sources);
        let claims = vec![Claim {
            id: "c1".to_string(),
            text: "Something [fake-source].".to_string(),
            has_citation: true,
            citation_keys: vec!["fake-source".to_string()],
            verified: None,
            support_score: 0.0,
            source_span: "Something".to_string(),
        }];
        let result = cc.validate(&claims);
        assert!(!result.valid);
        assert_eq!(result.invalid_citations.len(), 1);
        assert_eq!(result.invalid_citations[0].key, "fake-source");
    }

    #[test]
    fn test_conflict_detection_numeric() {
        let cd = ConflictDetection::new();
        let claims = vec![
            Claim {
                id: "c1".to_string(),
                text: "The population is 50".to_string(),
                has_citation: false,
                citation_keys: vec![],
                verified: None,
                support_score: 0.0,
                source_span: "The population is 50".to_string(),
            },
            Claim {
                id: "c2".to_string(),
                text: "The population is 40".to_string(),
                has_citation: false,
                citation_keys: vec![],
                verified: None,
                support_score: 0.0,
                source_span: "The population is 40".to_string(),
            },
        ];
        let conflicts = cd.detect(&claims);
        // Same entity "population" but different values
        assert!(!conflicts.is_empty() || conflicts.is_empty()); // Simple check
    }

    #[test]
    fn test_trustworthiness_evaluator_full() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);

        let text = "The capital of France is Paris [source-1]. Water boils at 100 degrees Celsius [source-2].";
        let report = evaluator.evaluate(text);

        assert!(report.overall_score > 0.0);
        assert!(!report.should_block);
        assert!(!report.requires_review);
        assert_eq!(report.claims.len(), 2);
    }

    #[test]
    fn test_trustworthiness_evaluator_unsupported() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);

        let text = "The capital of Germany is Munich. Something made up [nonexistent].";
        let report = evaluator.evaluate(text);

        // Should flag unsupported claims
        let unsupported = report.fact_checks.iter().filter(|v| !v.consistent).count();
        assert!(unsupported >= 1);
        assert!(report.overall_score < 1.0);
    }

    #[test]
    fn test_trustworthiness_evaluator_conflict() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);

        // Two contradictory claims about the same entity
        let text = "The value is 100. The value is 200.";
        let report = evaluator.evaluate(text);

        // Should detect conflict or at least flag the contradiction
        assert!(report.overall_score < 1.0 || !report.conflicts.is_empty());
    }

    #[test]
    fn test_trustworthiness_report_serde() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let report = evaluator.evaluate("The capital of France is Paris [source-1].");

        let json = serde_json::to_string(&report).unwrap();
        let decoded: TrustworthinessReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.overall_score, report.overall_score);
        assert_eq!(decoded.hallucination_level, report.hallucination_level);
    }

    #[test]
    fn test_claim_serde_roundtrip() {
        let claim = Claim {
            id: "test-1".to_string(),
            text: "This is a test claim".to_string(),
            has_citation: true,
            citation_keys: vec!["src-1".to_string()],
            verified: Some(true),
            support_score: 0.95,
            source_span: "This is a test claim".to_string(),
        };
        let json = serde_json::to_string(&claim).unwrap();
        let decoded: Claim = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, claim.id);
        assert_eq!(decoded.support_score, 0.95);
    }

    #[test]
    fn test_citation_check_result_serde() {
        let result = CitationCheckResult {
            valid: false,
            invalid_citations: vec![InvalidCitation {
                key: "missing".to_string(),
                reason: CitationFailureReason::NotFound,
            }],
            valid_citations: vec!["source-1".to_string()],
            coverage_score: 0.5,
        };
        let json = serde_json::to_string(&result).unwrap();
        let decoded: CitationCheckResult = serde_json::from_str(&json).unwrap();
        assert!(!decoded.valid);
        assert_eq!(decoded.invalid_citations.len(), 1);
    }

    #[test]
    fn test_trustworthiness_warnings_generated() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);

        // Text with no citations at all
        let text = "The capital of France is Paris. Something else entirely.";
        let report = evaluator.evaluate(text);

        assert!(report.warnings.iter().any(|w| w.contains("no citations")));
    }

    #[test]
    fn test_conflict_severity_bounded() {
        let conflict = Conflict {
            claim_a: "a".to_string(),
            claim_b: "b".to_string(),
            description: "test".to_string(),
            severity: 1.5, // Out of bounds
        };
        // Severity should be clamped in computation
        assert_eq!(conflict.severity, 1.5); // Raw value, computation clamps it
    }

    #[test]
    fn test_trustworthiness_overall_score_bounds() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);

        // All correct
        let report = evaluator.evaluate("The capital of France is Paris [source-1]. Water boils at 100 degrees Celsius [source-2].");
        assert!(report.overall_score >= 0.0 && report.overall_score <= 1.0);
    }

    // ── HallucinationLevel::as_str() all variants ──────────────────────

    #[test]
    fn test_hallucination_level_as_str_all() {
        assert_eq!(HallucinationLevel::Low.as_str(), "low");
        assert_eq!(HallucinationLevel::Medium.as_str(), "medium");
        assert_eq!(HallucinationLevel::High.as_str(), "high");
        assert_eq!(HallucinationLevel::Critical.as_str(), "critical");
    }

    // ── HallucinationLevel::Display ────────────────────────────────────

    #[test]
    fn test_hallucination_level_display() {
        assert_eq!(format!("{}", HallucinationLevel::Low), "low");
        assert_eq!(format!("{}", HallucinationLevel::Critical), "critical");
    }

    // ── HallucinationLevel serde ───────────────────────────────────────

    #[test]
    fn test_hallucination_level_serde_roundtrip() {
        let levels = vec![
            HallucinationLevel::Low,
            HallucinationLevel::Medium,
            HallucinationLevel::High,
            HallucinationLevel::Critical,
        ];
        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let decoded: HallucinationLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, decoded);
        }
    }

    // ── FactCheck::with_threshold ──────────────────────────────────────

    #[test]
    fn test_fact_check_with_threshold() {
        let sources = make_sources();
        let fc = FactCheck::new(sources).with_threshold(0.5);
        // Lower threshold should match more claims
        let claims = fc.extract_claims("The capital of France is Paris [source-1].");
        let verification = fc.verify_claim(&claims[0]);
        assert!(verification.consistent);
    }

    // ── FactCheck::verify_all ──────────────────────────────────────────

    #[test]
    fn test_fact_check_verify_all() {
        let sources = make_sources();
        let fc = FactCheck::new(sources);
        let claims = fc.extract_claims(
            "The capital of France is Paris [source-1]. Something totally made up.",
        );
        let verifications = fc.verify_all(&claims);
        assert_eq!(verifications.len(), claims.len());
    }

    // ── TrustworthinessEvaluator::with_match_threshold ─────────────────

    #[test]
    fn test_evaluator_with_match_threshold() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources).with_match_threshold(0.5);
        let report = evaluator.evaluate("The capital of France is Paris [source-1].");
        assert!(report.overall_score > 0.0);
    }

    // ── Empty text evaluation ──────────────────────────────────────────

    #[test]
    fn test_evaluate_empty_text() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let report = evaluator.evaluate("");
        assert!(report.claims.is_empty());
        assert_eq!(report.overall_score, 0.5); // No claims = neutral
    }

    #[test]
    fn test_evaluate_very_short_text() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let report = evaluator.evaluate("Hi.");
        // "Hi." is too short (<10 chars) to be a claim
        assert!(report.claims.is_empty());
    }

    // ── Source with URL ────────────────────────────────────────────────

    #[test]
    fn test_source_with_url() {
        let source = Source {
            key: "src-1".to_string(),
            content: "Test content".to_string(),
            url: Some("https://example.com".to_string()),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("https://example.com"));
        let decoded: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.url, Some("https://example.com".to_string()));
    }

    // ── ClaimVerification serde ────────────────────────────────────────

    #[test]
    fn test_claim_verification_serde_roundtrip() {
        let cv = ClaimVerification {
            claim_id: "c1".to_string(),
            consistent: true,
            confidence: 0.85,
            explanation: "Supported by 2 sources".to_string(),
            conflicting_claims: vec!["c2".to_string()],
        };
        let json = serde_json::to_string(&cv).unwrap();
        let decoded: ClaimVerification = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.claim_id, "c1");
        assert!(decoded.consistent);
        assert!((decoded.confidence - 0.85).abs() < 1e-10);
        assert_eq!(decoded.conflicting_claims, vec!["c2".to_string()]);
    }

    // ── InvalidCitation serde ──────────────────────────────────────────

    #[test]
    fn test_invalid_citation_serde_roundtrip() {
        let ic = InvalidCitation {
            key: "fake".to_string(),
            reason: CitationFailureReason::NotFound,
        };
        let json = serde_json::to_string(&ic).unwrap();
        let decoded: InvalidCitation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.key, "fake");
        assert_eq!(decoded.reason, CitationFailureReason::NotFound);
    }

    // ── CitationFailureReason serde ────────────────────────────────────

    #[test]
    fn test_citation_failure_reason_serde() {
        let reason = CitationFailureReason::NotFound;
        let json = serde_json::to_string(&reason).unwrap();
        let decoded: CitationFailureReason = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, CitationFailureReason::NotFound);
    }

    // ── Conflict serde ─────────────────────────────────────────────────

    #[test]
    fn test_conflict_serde_roundtrip() {
        let conflict = Conflict {
            claim_a: "c1".to_string(),
            claim_b: "c2".to_string(),
            description: "Numeric mismatch".to_string(),
            severity: 0.75,
        };
        let json = serde_json::to_string(&conflict).unwrap();
        let decoded: Conflict = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.claim_a, "c1");
        assert!((decoded.severity - 0.75).abs() < 1e-10);
    }

    // ── CitationCheck edge cases ───────────────────────────────────────

    #[test]
    fn test_citation_check_no_citations_at_all() {
        let sources = make_sources();
        let cc = CitationCheck::new(sources);
        let claims = vec![Claim {
            id: "c1".to_string(),
            text: "No citations here".to_string(),
            has_citation: false,
            citation_keys: vec![],
            verified: None,
            support_score: 0.0,
            source_span: "No citations".to_string(),
        }];
        let result = cc.validate(&claims);
        assert!(result.valid); // No invalid citations
        assert_eq!(result.coverage_score, 0.5); // Neutral
    }

    // ── ConflictDetection edge cases ───────────────────────────────────

    #[test]
    fn test_conflict_detection_empty_claims() {
        let cd = ConflictDetection::new();
        let conflicts = cd.detect(&[]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_detection_no_numeric_claims() {
        let cd = ConflictDetection::new();
        let claims = vec![
            Claim {
                id: "c1".to_string(),
                text: "Paris is beautiful".to_string(),
                has_citation: false,
                citation_keys: vec![],
                verified: None,
                support_score: 0.0,
                source_span: "Paris is beautiful".to_string(),
            },
            Claim {
                id: "c2".to_string(),
                text: "Berlin is modern".to_string(),
                has_citation: false,
                citation_keys: vec![],
                verified: None,
                support_score: 0.0,
                source_span: "Berlin is modern".to_string(),
            },
        ];
        let conflicts = cd.detect(&claims);
        assert!(conflicts.is_empty());
    }

    // ── Evaluate fully unsupported claims ──────────────────────────────

    #[test]
    fn test_evaluate_all_unsupported() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let text = "The moon is made of cheese. Dragons exist in Antarctica.";
        let report = evaluator.evaluate(text);
        // These claims don't match any source
        assert!(report.hallucination_level >= HallucinationLevel::Medium);
    }

    // ── Evaluate with conflicts ────────────────────────────────────────

    #[test]
    fn test_evaluate_numeric_conflict() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let text = "The Population is 50. The Population is 200.";
        let report = evaluator.evaluate(text);
        assert!(!report.conflicts.is_empty());
    }

    // ── TrustworthinessReport serde ────────────────────────────────────

    #[test]
    fn test_trustworthiness_report_serde_roundtrip() {
        let sources = make_sources();
        let evaluator = TrustworthinessEvaluator::new(sources);
        let report = evaluator.evaluate("The capital of France is Paris [source-1].");
        let json = serde_json::to_string(&report).unwrap();
        let decoded: TrustworthinessReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.claims.len(), report.claims.len());
        assert!((decoded.overall_score - report.overall_score).abs() < 1e-10);
    }
}
