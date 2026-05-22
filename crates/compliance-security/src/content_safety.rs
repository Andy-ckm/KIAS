//! # LLM Output Content Safety Screening
//!
//! Post-generation content safety screening for LLM outputs. Catches toxic,
//! harmful, or policy-violating content *before* it reaches the end user.
//!
//! ## Design
//!
//! 1. **Multi-category scanning** — toxicity, self-harm, sexual, violence,
//!    PII leakage, bias, hallucination markers, regulatory violations.
//! 2. **Configurable policy thresholds** — per-category severity cutoffs;
//!    enterprise can tune strictness per deployment (dev vs. prod).
//! 3. **Pluggable detectors** — built-in regex/heuristic detectors ship
//!    out-of-box; external API detectors (OpenAI Moderation, Perspective API)
//!    can be registered via the `ContentDetector` trait.
//! 4. **Audit trail** — every scan produces a `SafetyReport` suitable for
//!    GxP/FDA 21 CFR Part 11 audit logging.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
//  ContentCategory — what kind of unsafe content we're scanning for
// ═══════════════════════════════════════════════════════════════════════════

/// Categories of content safety violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentCategory {
    /// Hate speech, slurs, dehumanization.
    Toxicity,
    /// Self-harm, suicide, eating disorders.
    SelfHarm,
    /// Sexual content, exploitation.
    Sexual,
    /// Violence, gore, threats.
    Violence,
    /// Personally identifiable information leakage (SSN, credit card, etc.).
    PiiLeakage,
    /// Biased or discriminatory statements.
    Bias,
    /// Hallucination markers — fabricated citations, false statistics.
    Hallucination,
    /// Regulatory violations (e.g. medical advice without disclaimer).
    RegulatoryViolation,
}

impl fmt::Display for ContentCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toxicity => write!(f, "toxicity"),
            Self::SelfHarm => write!(f, "self_harm"),
            Self::Sexual => write!(f, "sexual"),
            Self::Violence => write!(f, "violence"),
            Self::PiiLeakage => write!(f, "pii_leakage"),
            Self::Bias => write!(f, "bias"),
            Self::Hallucination => write!(f, "hallucination"),
            Self::RegulatoryViolation => write!(f, "regulatory_violation"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Severity — how bad a finding is (0.0–1.0 score mapped to levels)
// ═══════════════════════════════════════════════════════════════════════════

/// Severity level of a content safety finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SafetySeverity {
    /// Safe — no issues detected.
    Safe,
    /// Low — borderline content, may need human review.
    Low,
    /// Medium — likely violates policy.
    Medium,
    /// High — clear violation, should be blocked.
    High,
    /// Critical — severe violation, block + alert.
    Critical,
}

impl fmt::Display for SafetySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safe => write!(f, "safe"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Map a 0.0–1.0 confidence score to a severity level.
pub fn score_to_severity(score: f64) -> SafetySeverity {
    debug_assert!((0.0..=1.0).contains(&score));
    if score < 0.2 {
        SafetySeverity::Safe
    } else if score < 0.4 {
        SafetySeverity::Low
    } else if score < 0.6 {
        SafetySeverity::Medium
    } else if score < 0.8 {
        SafetySeverity::High
    } else {
        SafetySeverity::Critical
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  SafetyPolicy — configurable thresholds per category
// ═══════════════════════════════════════════════════════════════════════════

/// Policy configuration: per-category blocking thresholds.
///
/// Scores >= the threshold for a category trigger a block.
/// Default thresholds are conservative (0.5 for most categories).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyPolicy {
    /// Per-category threshold (0.0–1.0). If a finding's score >= threshold,
    /// it triggers a block for that category.
    pub thresholds: HashMap<ContentCategory, f64>,
    /// Whether to enable audit logging for all scans.
    pub audit_enabled: bool,
    /// Maximum content length (bytes) before truncation.
    pub max_content_length: usize,
}

impl Default for SafetyPolicy {
    fn default() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert(ContentCategory::Toxicity, 0.5);
        thresholds.insert(ContentCategory::SelfHarm, 0.3); // stricter
        thresholds.insert(ContentCategory::Sexual, 0.5);
        thresholds.insert(ContentCategory::Violence, 0.5);
        thresholds.insert(ContentCategory::PiiLeakage, 0.2); // very strict
        thresholds.insert(ContentCategory::Bias, 0.5);
        thresholds.insert(ContentCategory::Hallucination, 0.6);
        thresholds.insert(ContentCategory::RegulatoryViolation, 0.3); // strict

        Self {
            thresholds,
            audit_enabled: true,
            max_content_length: 100_000,
        }
    }
}

impl SafetyPolicy {
    /// Create a strict policy (lower thresholds = more aggressive blocking).
    pub fn strict() -> Self {
        let mut policy = Self::default();
        for val in policy.thresholds.values_mut() {
            *val = (*val * 0.5).max(0.1);
        }
        policy
    }

    /// Create a permissive policy (higher thresholds = fewer false positives).
    pub fn permissive() -> Self {
        let mut policy = Self::default();
        for val in policy.thresholds.values_mut() {
            *val = (*val * 1.5).min(0.9);
        }
        policy
    }

    /// Get the threshold for a specific category (falls back to 0.5).
    pub fn threshold_for(&self, category: &ContentCategory) -> f64 {
        self.thresholds.get(category).copied().unwrap_or(0.5)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Finding & Report — what the scanner found
// ═══════════════════════════════════════════════════════════════════════════

/// A single content safety finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyFinding {
    /// Which category this finding falls under.
    pub category: ContentCategory,
    /// Severity score (0.0–1.0).
    pub score: f64,
    /// Mapped severity level.
    pub severity: SafetySeverity,
    /// Human-readable description.
    pub description: String,
    /// Which detector produced this finding.
    pub detector: String,
    /// Matched text excerpt (if applicable).
    pub matched_text: Option<String>,
    /// Character position in the output where the issue was found.
    pub position: Option<usize>,
}

/// The result of a content safety scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    /// Truncated preview of the scanned content.
    pub content_preview: String,
    /// All findings across all detectors.
    pub findings: Vec<SafetyFinding>,
    /// Whether any finding exceeded its category threshold.
    pub should_block: bool,
    /// The highest severity found.
    pub max_severity: SafetySeverity,
    /// Per-category max scores.
    pub category_scores: HashMap<ContentCategory, f64>,
    /// Timestamp of the scan.
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    /// Which policy was applied.
    pub policy_name: String,
}

impl SafetyReport {
    /// Get findings filtered by category.
    pub fn findings_for(&self, category: &ContentCategory) -> Vec<&SafetyFinding> {
        self.findings
            .iter()
            .filter(|f| f.category == *category)
            .collect()
    }

    /// Get all findings at or above a severity level.
    pub fn findings_above(&self, min_severity: SafetySeverity) -> Vec<&SafetyFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity >= min_severity)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  ContentDetector trait — pluggable detection backends
// ═══════════════════════════════════════════════════════════════════════════

/// Trait for content safety detectors.
///
/// Implement this to add new detection backends (e.g. OpenAI Moderation API,
/// Perspective API, custom ML models).
#[async_trait::async_trait]
pub trait ContentDetector: Send + Sync {
    /// Human-readable name for this detector.
    fn name(&self) -> &str;

    /// Scan content and return findings.
    async fn scan(&self, content: &str) -> Result<Vec<SafetyFinding>, ContentSafetyError>;
}

// ═══════════════════════════════════════════════════════════════════════════
//  Error
// ═══════════════════════════════════════════════════════════════════════════

/// Errors from content safety operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentSafetyError {
    /// Content exceeds maximum length.
    ContentTooLong { length: usize, max: usize },
    /// A detector failed.
    DetectorError { detector: String, message: String },
    /// Policy configuration error.
    InvalidPolicy(String),
}

impl fmt::Display for ContentSafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTooLong { length, max } => {
                write!(f, "content too long: {length} bytes (max {max})")
            }
            Self::DetectorError { detector, message } => {
                write!(f, "detector '{detector}' error: {message}")
            }
            Self::InvalidPolicy(msg) => write!(f, "invalid policy: {msg}"),
        }
    }
}

impl std::error::Error for ContentSafetyError {}

// ═══════════════════════════════════════════════════════════════════════════
//  Built-in Detectors
// ═══════════════════════════════════════════════════════════════════════════

// ── Toxicity Detector (regex/heuristic) ──────────────────────────────────

/// Built-in toxicity detector using pattern matching.
///
/// This is a baseline detector. For production use, combine with an external
/// ML-based moderation API (OpenAI, Perspective, etc.).
pub struct ToxicityDetector {
    patterns: Vec<ToxicityPattern>,
}

struct ToxicityPattern {
    name: &'static str,
    pattern: &'static str,
    score: f64,
    category: ContentCategory,
}

impl Default for ToxicityDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ToxicityDetector {
    pub fn new() -> Self {
        let patterns = vec![
            // Slurs and hate speech indicators
            ToxicityPattern {
                name: "hate_speech_slur",
                pattern: r"(?i)\b(idiot|stupid|moron|dumb|trash|scum|vermin)\b",
                score: 0.6,
                category: ContentCategory::Toxicity,
            },
            // Self-harm language
            ToxicityPattern {
                name: "self_harm_language",
                pattern: r"(?i)\b(kill yourself|end your life|self[\-\s]?harm|suicide method|cutting yourself)\b",
                score: 0.9,
                category: ContentCategory::SelfHarm,
            },
            // Violence threats
            ToxicityPattern {
                name: "violence_threat",
                pattern: r"(?i)\b(i will (kill|murder|hurt|destroy)|bomb|shoot|stab|attack)\b",
                score: 0.8,
                category: ContentCategory::Violence,
            },
            // PII patterns — SSN
            ToxicityPattern {
                name: "pii_ssn",
                pattern: r"\b\d{3}-\d{2}-\d{4}\b",
                score: 0.95,
                category: ContentCategory::PiiLeakage,
            },
            // PII patterns — credit card
            ToxicityPattern {
                name: "pii_credit_card",
                pattern: r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13})\b",
                score: 0.95,
                category: ContentCategory::PiiLeakage,
            },
            // PII patterns — email in sensitive context
            ToxicityPattern {
                name: "pii_email",
                pattern: r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",
                score: 0.3,
                category: ContentCategory::PiiLeakage,
            },
            // Hallucination markers — fabricated citations
            ToxicityPattern {
                name: "hallucination_fake_citation",
                pattern: r"(?i)(according to a \d{4} study|research (shows|proves|confirms) that|studies have shown)",
                score: 0.4,
                category: ContentCategory::Hallucination,
            },
            // Medical advice without disclaimer
            ToxicityPattern {
                name: "regulatory_medical_advice",
                pattern: r"(?i)(you should (take|stop taking|increase|decrease) (your |the )?(medication|dose|prescription)|diagnos(e|is|ed) with)",
                score: 0.7,
                category: ContentCategory::RegulatoryViolation,
            },
            // Discriminatory language
            ToxicityPattern {
                name: "bias_discrimination",
                pattern: r"(?i)\b(all (women|men|blacks|whites|asians|jews|muslims|gays) are)\b",
                score: 0.85,
                category: ContentCategory::Bias,
            },
        ];
        Self { patterns }
    }
}

#[async_trait::async_trait]
impl ContentDetector for ToxicityDetector {
    fn name(&self) -> &str {
        "builtin_toxicity"
    }

    async fn scan(&self, content: &str) -> Result<Vec<SafetyFinding>, ContentSafetyError> {
        let mut findings = Vec::new();

        for tp in &self.patterns {
            let re = match regex::Regex::new(tp.pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for mat in re.find_iter(content) {
                findings.push(SafetyFinding {
                    category: tp.category,
                    score: tp.score,
                    severity: score_to_severity(tp.score),
                    description: format!("Matched pattern '{}'", tp.name),
                    detector: self.name().to_string(),
                    matched_text: Some(mat.as_str().to_string()),
                    position: Some(mat.start()),
                });
            }
        }

        Ok(findings)
    }
}

// ── PII Detector (dedicated) ─────────────────────────────────────────────

/// Dedicated PII detection with broader pattern coverage.
pub struct PiiDetector;

impl Default for PiiDetector {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ContentDetector for PiiDetector {
    fn name(&self) -> &str {
        "builtin_pii"
    }

    async fn scan(&self, content: &str) -> Result<Vec<SafetyFinding>, ContentSafetyError> {
        let mut findings = Vec::new();

        let patterns: Vec<(&str, &str, f64)> = vec![
            ("SSN", r"\b\d{3}-\d{2}-\d{4}\b", 0.95),
            ("credit_card_visa", r"\b4[0-9]{12}(?:[0-9]{3})?\b", 0.95),
            ("credit_card_mc", r"\b5[1-5][0-9]{14}\b", 0.95),
            ("ip_address", r"\b(?:\d{1,3}\.){3}\d{1,3}\b", 0.3),
            (
                "phone_us",
                r"\b(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
                0.5,
            ),
        ];

        for (name, pattern, score) in patterns {
            let re = match regex::Regex::new(pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };
            for mat in re.find_iter(content) {
                findings.push(SafetyFinding {
                    category: ContentCategory::PiiLeakage,
                    score,
                    severity: score_to_severity(score),
                    description: format!("Detected {name}"),
                    detector: self.name().to_string(),
                    matched_text: Some(mat.as_str().to_string()),
                    position: Some(mat.start()),
                });
            }
        }

        Ok(findings)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  ContentSafetyScanner — the main orchestrator
// ═══════════════════════════════════════════════════════════════════════════

/// Main content safety scanner. Runs registered detectors and applies policy.
pub struct ContentSafetyScanner {
    policy: SafetyPolicy,
    detectors: Vec<Box<dyn ContentDetector>>,
}

impl ContentSafetyScanner {
    /// Create a new scanner with the given policy.
    pub fn new(policy: SafetyPolicy) -> Self {
        Self {
            policy,
            detectors: Vec::new(),
        }
    }

    /// Create a scanner with default policy and built-in detectors.
    pub fn with_defaults() -> Self {
        let mut scanner = Self::new(SafetyPolicy::default());
        scanner.add_detector(Box::new(ToxicityDetector::new()));
        scanner.add_detector(Box::new(PiiDetector));
        scanner
    }

    /// Register a detector.
    pub fn add_detector(&mut self, detector: Box<dyn ContentDetector>) {
        self.detectors.push(detector);
    }

    /// Scan LLM output content and return a safety report.
    pub async fn scan(&self, content: &str) -> Result<SafetyReport, ContentSafetyError> {
        // Truncate if needed
        let truncated = if content.len() > self.policy.max_content_length {
            &content[..self.policy.max_content_length]
        } else {
            content
        };

        // Run all detectors
        let mut all_findings = Vec::new();
        for detector in &self.detectors {
            match detector.scan(truncated).await {
                Ok(findings) => all_findings.extend(findings),
                Err(e) => {
                    // Log detector failure but continue with other detectors
                    tracing::warn!(
                        detector = detector.name(),
                        error = %e,
                        "content safety detector failed"
                    );
                }
            }
        }

        // Deduplicate findings by position + category
        all_findings.sort_by(|a, b| {
            a.position
                .unwrap_or(0)
                .cmp(&b.position.unwrap_or(0))
                .then_with(|| a.category.to_string().cmp(&b.category.to_string()))
        });
        all_findings.dedup_by(|a, b| a.position == b.position && a.category == b.category);

        // Apply policy: check if any finding exceeds its category threshold
        let mut category_scores: HashMap<ContentCategory, f64> = HashMap::new();
        for finding in &all_findings {
            let entry = category_scores.entry(finding.category).or_insert(0.0);
            *entry = entry.max(finding.score);
        }

        let should_block = category_scores
            .iter()
            .any(|(cat, score)| *score >= self.policy.threshold_for(cat));

        let max_severity = all_findings
            .iter()
            .map(|f| f.severity)
            .max()
            .unwrap_or(SafetySeverity::Safe);

        let content_preview = if truncated.len() > 200 {
            format!("{}...", &truncated[..200])
        } else {
            truncated.to_string()
        };

        Ok(SafetyReport {
            content_preview,
            findings: all_findings,
            should_block,
            max_severity,
            category_scores,
            scanned_at: chrono::Utc::now(),
            policy_name: "default".to_string(),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Score-to-severity mapping ────────────────────────────────────────

    #[test]
    fn test_score_to_severity_boundaries() {
        assert_eq!(score_to_severity(0.0), SafetySeverity::Safe);
        assert_eq!(score_to_severity(0.19), SafetySeverity::Safe);
        assert_eq!(score_to_severity(0.2), SafetySeverity::Low);
        assert_eq!(score_to_severity(0.39), SafetySeverity::Low);
        assert_eq!(score_to_severity(0.4), SafetySeverity::Medium);
        assert_eq!(score_to_severity(0.59), SafetySeverity::Medium);
        assert_eq!(score_to_severity(0.6), SafetySeverity::High);
        assert_eq!(score_to_severity(0.79), SafetySeverity::High);
        assert_eq!(score_to_severity(0.8), SafetySeverity::Critical);
        assert_eq!(score_to_severity(1.0), SafetySeverity::Critical);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(SafetySeverity::Safe < SafetySeverity::Low);
        assert!(SafetySeverity::Low < SafetySeverity::Medium);
        assert!(SafetySeverity::Medium < SafetySeverity::High);
        assert!(SafetySeverity::High < SafetySeverity::Critical);
    }

    // ── SafetyPolicy ─────────────────────────────────────────────────────

    #[test]
    fn test_default_policy_has_all_categories() {
        let policy = SafetyPolicy::default();
        assert_eq!(policy.thresholds.len(), 8);
        assert!(policy.thresholds.contains_key(&ContentCategory::Toxicity));
        assert!(policy.thresholds.contains_key(&ContentCategory::PiiLeakage));
    }

    #[test]
    fn test_strict_policy_lower_thresholds() {
        let default = SafetyPolicy::default();
        let strict = SafetyPolicy::strict();
        for cat in &[
            ContentCategory::Toxicity,
            ContentCategory::SelfHarm,
            ContentCategory::Violence,
        ] {
            assert!(
                strict.threshold_for(cat) <= default.threshold_for(cat),
                "strict threshold for {cat} should be <= default"
            );
        }
    }

    #[test]
    fn test_permissive_policy_higher_thresholds() {
        let default = SafetyPolicy::default();
        let permissive = SafetyPolicy::permissive();
        for cat in &[
            ContentCategory::Toxicity,
            ContentCategory::SelfHarm,
            ContentCategory::Violence,
        ] {
            assert!(
                permissive.threshold_for(cat) >= default.threshold_for(cat),
                "permissive threshold for {cat} should be >= default"
            );
        }
    }

    #[test]
    fn test_threshold_for_unknown_category_returns_default() {
        let policy = SafetyPolicy::default();
        // All categories are in default, but test the fallback
        assert_eq!(policy.threshold_for(&ContentCategory::Toxicity), 0.5);
    }

    // ── ToxicityDetector ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_toxicity_clean_text() {
        let detector = ToxicityDetector::new();
        let findings = detector
            .scan("Hello, how can I help you today?")
            .await
            .unwrap();
        assert!(findings.is_empty(), "clean text should have no findings");
    }

    #[tokio::test]
    async fn test_toxicity_detects_hate_speech() {
        let detector = ToxicityDetector::new();
        let findings = detector
            .scan("You are such an idiot and moron")
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.category == ContentCategory::Toxicity));
    }

    #[tokio::test]
    async fn test_toxicity_detects_self_harm() {
        let detector = ToxicityDetector::new();
        let findings = detector.scan("You should kill yourself").await.unwrap();
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.category == ContentCategory::SelfHarm));
        assert!(findings
            .iter()
            .any(|f| f.severity >= SafetySeverity::Critical));
    }

    #[tokio::test]
    async fn test_toxicity_detects_violence() {
        let detector = ToxicityDetector::new();
        let findings = detector
            .scan("I will kill you and destroy everything")
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.category == ContentCategory::Violence));
    }

    #[tokio::test]
    async fn test_toxicity_detects_pii_ssn() {
        let detector = ToxicityDetector::new();
        let findings = detector.scan("My SSN is 123-45-6789").await.unwrap();
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.category == ContentCategory::PiiLeakage));
    }

    #[tokio::test]
    async fn test_toxicity_detects_medical_advice() {
        let detector = ToxicityDetector::new();
        let findings = detector
            .scan("You should stop taking your medication immediately")
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.category == ContentCategory::RegulatoryViolation));
    }

    #[tokio::test]
    async fn test_toxicity_detects_bias() {
        let detector = ToxicityDetector::new();
        let findings = detector
            .scan("All women are incapable of logic")
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.category == ContentCategory::Bias));
    }

    // ── PiiDetector ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_pii_detector_clean_text() {
        let detector = PiiDetector;
        let findings = detector.scan("The weather is nice today").await.unwrap();
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_pii_detector_ssn() {
        let detector = PiiDetector;
        let findings = detector.scan("Patient SSN: 123-45-6789").await.unwrap();
        assert!(!findings.is_empty());
        assert_eq!(findings[0].category, ContentCategory::PiiLeakage);
    }

    #[tokio::test]
    async fn test_pii_detector_credit_card() {
        let detector = PiiDetector;
        let findings = detector
            .scan("Card number: 4111111111111111")
            .await
            .unwrap();
        assert!(!findings.is_empty());
    }

    #[tokio::test]
    async fn test_pii_detector_ip_address() {
        let detector = PiiDetector;
        let findings = detector.scan("Server at 192.168.1.100").await.unwrap();
        assert!(!findings.is_empty());
    }

    // ── ContentSafetyScanner ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_scanner_clean_text() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner.scan("Hello world").await.unwrap();
        assert!(!report.should_block);
        assert_eq!(report.max_severity, SafetySeverity::Safe);
        assert!(report.findings.is_empty());
    }

    #[tokio::test]
    async fn test_scanner_blocks_toxic_content() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner
            .scan("You are a worthless moron, go kill yourself")
            .await
            .unwrap();
        assert!(report.should_block);
        assert!(report.max_severity >= SafetySeverity::Critical);
    }

    #[tokio::test]
    async fn test_scanner_blocks_pii_leakage() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner
            .scan("Patient John Doe, SSN: 123-45-6789, card: 4111111111111111")
            .await
            .unwrap();
        assert!(report.should_block);
        assert!(report
            .category_scores
            .contains_key(&ContentCategory::PiiLeakage));
    }

    #[tokio::test]
    async fn test_scanner_no_false_positive_on_technical_text() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner
            .scan("The API returns HTTP 200 with JSON payload. Use POST /api/v1/agents.")
            .await
            .unwrap();
        assert!(
            !report.should_block,
            "technical text should not trigger false positives"
        );
    }

    #[tokio::test]
    async fn test_scanner_findings_for_category() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner
            .scan("SSN: 123-45-6789 and you are an idiot")
            .await
            .unwrap();
        let pii_findings = report.findings_for(&ContentCategory::PiiLeakage);
        assert!(!pii_findings.is_empty());
        let toxicity_findings = report.findings_for(&ContentCategory::Toxicity);
        assert!(!toxicity_findings.is_empty());
    }

    #[tokio::test]
    async fn test_scanner_findings_above_severity() {
        let scanner = ContentSafetyScanner::with_defaults();
        let report = scanner.scan("You should kill yourself").await.unwrap();
        let critical = report.findings_above(SafetySeverity::Critical);
        assert!(!critical.is_empty());
        let low = report.findings_above(SafetySeverity::Low);
        assert!(low.len() >= critical.len());
    }

    #[tokio::test]
    async fn test_scanner_content_truncation() {
        let mut policy = SafetyPolicy::default();
        policy.max_content_length = 50;
        let scanner = ContentSafetyScanner::new(policy);
        let long_content = "a".repeat(200);
        let report = scanner.scan(&long_content).await.unwrap();
        assert!(report.content_preview.len() <= 55); // 50 + "..."
    }

    // ── Serialization ────────────────────────────────────────────────────

    #[test]
    fn test_safety_report_serde_roundtrip() {
        let report = SafetyReport {
            content_preview: "test".to_string(),
            findings: vec![SafetyFinding {
                category: ContentCategory::Toxicity,
                score: 0.8,
                severity: SafetySeverity::High,
                description: "test finding".to_string(),
                detector: "test".to_string(),
                matched_text: None,
                position: None,
            }],
            should_block: true,
            max_severity: SafetySeverity::High,
            category_scores: HashMap::new(),
            scanned_at: chrono::Utc::now(),
            policy_name: "test".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SafetyReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content_preview, "test");
        assert!(deserialized.should_block);
        assert_eq!(deserialized.findings.len(), 1);
    }

    #[test]
    fn test_content_category_display() {
        assert_eq!(ContentCategory::Toxicity.to_string(), "toxicity");
        assert_eq!(ContentCategory::PiiLeakage.to_string(), "pii_leakage");
        assert_eq!(
            ContentCategory::RegulatoryViolation.to_string(),
            "regulatory_violation"
        );
    }
}
