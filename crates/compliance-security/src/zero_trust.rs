//! # Zero-Trust Architecture
//!
//! Implements continuous verification, mTLS enforcement, and least-privilege
//! access control. Every request is authenticated, authorized, and scored
//! based on device posture, network context, and behavioral signals.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ── Trust Score ────────────────────────────────────────────────────────

/// Composite trust score for an entity (agent, user, or device).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    /// Overall trust level 0.0 (untrusted) to 1.0 (fully trusted).
    pub value: f64,
    /// Breakdown of trust signals.
    pub components: Vec<TrustComponent>,
    /// When this score was computed.
    pub computed_at: DateTime<Utc>,
    /// How long this score is valid (seconds).
    pub validity_seconds: u64,
}

/// Individual trust signal contributing to the composite score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustComponent {
    /// Signal name (e.g. "device_posture", "network_location").
    pub name: String,
    /// Signal value 0.0 to 1.0.
    pub value: f64,
    /// Weight of this signal in the composite (0.0 to 1.0).
    pub weight: f64,
    /// Human-readable explanation.
    pub reason: String,
}

impl TrustScore {
    /// Compute weighted average of all components.
    pub fn compute(&mut self) {
        if self.components.is_empty() {
            self.value = 0.0;
            return;
        }
        let total_weight: f64 = self.components.iter().map(|c| c.weight).sum();
        if total_weight == 0.0 {
            self.value = 0.0;
            return;
        }
        self.value = self
            .components
            .iter()
            .map(|c| c.value * c.weight)
            .sum::<f64>()
            / total_weight;
    }

    /// Whether the score has expired.
    pub fn is_expired(&self) -> bool {
        let expires = self.computed_at + Duration::seconds(self.validity_seconds as i64);
        Utc::now() > expires
    }
}

// ── Trust Decision ─────────────────────────────────────────────────────

/// Decision from the zero-trust engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustDecision {
    /// Allow the request.
    Allow,
    /// Allow but with reduced privileges.
    AllowRestricted,
    /// Require step-up authentication (e.g. 2FA).
    RequireStepUp,
    /// Deny the request.
    Deny(String),
}

impl TrustDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow | Self::AllowRestricted)
    }
}

impl fmt::Display for TrustDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::AllowRestricted => write!(f, "allow_restricted"),
            Self::RequireStepUp => write!(f, "require_step_up"),
            Self::Deny(reason) => write!(f, "deny: {reason}"),
        }
    }
}

// ── Verification Context ───────────────────────────────────────────────

/// Context for a zero-trust verification request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationContext {
    /// Subject (user/agent ID) making the request.
    pub subject: String,
    /// Whether mTLS was used.
    pub mtls_used: bool,
    /// Client certificate fingerprint (if mTLS).
    pub cert_fingerprint: Option<String>,
    /// Source IP address.
    pub source_ip: Option<String>,
    /// Requested resource.
    pub resource: String,
    /// Requested action.
    pub action: String,
    /// Device posture score (0.0 = compromised, 1.0 = fully compliant).
    pub device_posture: f64,
    /// Whether the request is from a known-good network segment.
    pub trusted_network: bool,
    /// Time since last authentication (seconds).
    pub seconds_since_auth: u64,
    /// Additional context attributes.
    pub attributes: HashMap<String, String>,
}

// ── Zero-Trust Policy ──────────────────────────────────────────────────

/// A zero-trust policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroTrustPolicy {
    /// Unique policy ID.
    pub id: String,
    /// Minimum trust score required (0.0 to 1.0).
    pub min_trust_score: f64,
    /// Whether mTLS is mandatory.
    pub require_mtls: bool,
    /// Maximum seconds since last authentication.
    pub max_auth_age_seconds: u64,
    /// Minimum device posture score.
    pub min_device_posture: f64,
    /// Whether request must come from trusted network.
    pub require_trusted_network: bool,
    /// Resources this policy applies to ("*" for all).
    pub resource_pattern: String,
    /// Actions this policy applies to ("*" for all).
    pub action_pattern: String,
}

// ── Zero-Trust Engine ──────────────────────────────────────────────────

/// Central zero-trust evaluation engine.
pub struct ZeroTrustEngine {
    policies: Vec<ZeroTrustPolicy>,
    /// Default minimum trust score if no policy matches.
    default_min_trust: f64,
}

impl ZeroTrustEngine {
    /// Create a new engine with default settings.
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
            default_min_trust: 0.5,
        }
    }

    /// Create with policies.
    pub fn with_policies(policies: Vec<ZeroTrustPolicy>) -> Self {
        Self {
            policies,
            default_min_trust: 0.5,
        }
    }

    /// Set the default minimum trust score.
    pub fn set_default_min_trust(&mut self, score: f64) {
        self.default_min_trust = score.clamp(0.0, 1.0);
    }

    /// Add a policy.
    pub fn add_policy(&mut self, policy: ZeroTrustPolicy) {
        self.policies.push(policy);
    }

    /// Evaluate a verification context and return a trust score + decision.
    pub fn evaluate(&self, ctx: &VerificationContext) -> (TrustScore, TrustDecision) {
        // Build trust score from signals
        let mut score = TrustScore {
            value: 0.0,
            components: Vec::new(),
            computed_at: Utc::now(),
            validity_seconds: 300,
        };

        // Signal 1: mTLS usage (high weight)
        score.components.push(TrustComponent {
            name: "mtls".to_string(),
            value: if ctx.mtls_used { 1.0 } else { 0.0 },
            weight: 0.3,
            reason: if ctx.mtls_used {
                "mTLS verified".to_string()
            } else {
                "No mTLS".to_string()
            },
        });

        // Signal 2: Device posture
        score.components.push(TrustComponent {
            name: "device_posture".to_string(),
            value: ctx.device_posture,
            weight: 0.25,
            reason: format!("Device posture: {:.2}", ctx.device_posture),
        });

        // Signal 3: Network trust
        score.components.push(TrustComponent {
            name: "network".to_string(),
            value: if ctx.trusted_network { 1.0 } else { 0.3 },
            weight: 0.2,
            reason: if ctx.trusted_network {
                "Trusted network segment".to_string()
            } else {
                "Untrusted network".to_string()
            },
        });

        // Signal 4: Authentication freshness
        let auth_freshness = if ctx.seconds_since_auth < 300 {
            1.0
        } else if ctx.seconds_since_auth < 3600 {
            0.7
        } else if ctx.seconds_since_auth < 86400 {
            0.4
        } else {
            0.1
        };
        score.components.push(TrustComponent {
            name: "auth_freshness".to_string(),
            value: auth_freshness,
            weight: 0.25,
            reason: format!("Auth {}s ago", ctx.seconds_since_auth),
        });

        score.compute();

        // Find matching policy
        let matching_policy = self.policies.iter().find(|p| {
            (p.resource_pattern == "*" || p.resource_pattern == ctx.resource)
                && (p.action_pattern == "*" || p.action_pattern == ctx.action)
        });

        let min_trust = matching_policy
            .as_ref()
            .map(|p| p.min_trust_score)
            .unwrap_or(self.default_min_trust);

        // Evaluate decision
        let decision = if let Some(policy) = matching_policy {
            // Check mTLS requirement
            if policy.require_mtls && !ctx.mtls_used {
                return (
                    score,
                    TrustDecision::Deny("mTLS required by policy".to_string()),
                );
            }

            // Check device posture
            if ctx.device_posture < policy.min_device_posture {
                return (
                    score,
                    TrustDecision::Deny(format!(
                        "Device posture {:.2} below minimum {:.2}",
                        ctx.device_posture, policy.min_device_posture
                    )),
                );
            }

            // Check network requirement
            if policy.require_trusted_network && !ctx.trusted_network {
                return (
                    score,
                    TrustDecision::Deny("Trusted network required".to_string()),
                );
            }

            // Check auth freshness
            if ctx.seconds_since_auth > policy.max_auth_age_seconds {
                return (score, TrustDecision::RequireStepUp);
            }

            // Check trust score
            if score.value >= min_trust {
                if score.value >= 0.8 {
                    TrustDecision::Allow
                } else {
                    TrustDecision::AllowRestricted
                }
            } else if score.value >= min_trust * 0.7 {
                TrustDecision::RequireStepUp
            } else {
                TrustDecision::Deny(format!(
                    "Trust score {:.2} below minimum {:.2}",
                    score.value, min_trust
                ))
            }
        } else {
            // No matching policy — use defaults
            if score.value >= min_trust {
                if score.value >= 0.8 {
                    TrustDecision::Allow
                } else {
                    TrustDecision::AllowRestricted
                }
            } else {
                TrustDecision::Deny(format!(
                    "Trust score {:.2} below default minimum {:.2}",
                    score.value, min_trust
                ))
            }
        };

        (score, decision)
    }

    /// Evaluate and return only the decision.
    pub fn decide(&self, ctx: &VerificationContext) -> TrustDecision {
        self.evaluate(ctx).1
    }

    /// Continuous re-evaluation: re-score an existing trust score with new signals.
    pub fn re_evaluate(
        &self,
        previous: &TrustScore,
        ctx: &VerificationContext,
    ) -> (TrustScore, TrustDecision) {
        let (mut new_score, decision) = self.evaluate(ctx);
        // Blend with previous score (exponential moving average)
        let alpha = 0.7; // Weight for new observation
        new_score.value = alpha * new_score.value + (1.0 - alpha) * previous.value;
        (new_score, decision)
    }
}

impl Default for ZeroTrustEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn high_trust_context() -> VerificationContext {
        VerificationContext {
            subject: "agent-1".to_string(),
            mtls_used: true,
            cert_fingerprint: Some("abc123".to_string()),
            source_ip: Some("10.0.0.1".to_string()),
            resource: "agent".to_string(),
            action: "Read".to_string(),
            device_posture: 0.95,
            trusted_network: true,
            seconds_since_auth: 60,
            attributes: HashMap::new(),
        }
    }

    fn low_trust_context() -> VerificationContext {
        VerificationContext {
            subject: "unknown".to_string(),
            mtls_used: false,
            cert_fingerprint: None,
            source_ip: Some("203.0.113.1".to_string()),
            resource: "agent".to_string(),
            action: "Delete".to_string(),
            device_posture: 0.2,
            trusted_network: false,
            seconds_since_auth: 7200,
            attributes: HashMap::new(),
        }
    }

    #[test]
    fn test_high_trust_context_allows() {
        let engine = ZeroTrustEngine::new();
        let ctx = high_trust_context();
        let (score, decision) = engine.evaluate(&ctx);
        assert!(score.value > 0.8, "Score was {}", score.value);
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_low_trust_context_denied() {
        let engine = ZeroTrustEngine::new();
        let ctx = low_trust_context();
        let (score, decision) = engine.evaluate(&ctx);
        assert!(score.value < 0.5, "Score was {}", score.value);
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_mtls_required_policy() {
        let policy = ZeroTrustPolicy {
            id: "p1".to_string(),
            min_trust_score: 0.3,
            require_mtls: true,
            max_auth_age_seconds: 3600,
            min_device_posture: 0.0,
            require_trusted_network: false,
            resource_pattern: "*".to_string(),
            action_pattern: "*".to_string(),
        };
        let engine = ZeroTrustEngine::with_policies(vec![policy]);

        let mut ctx = high_trust_context();
        ctx.mtls_used = false;
        let decision = engine.decide(&ctx);
        assert_eq!(
            decision,
            TrustDecision::Deny("mTLS required by policy".to_string())
        );
    }

    #[test]
    fn test_trusted_network_required() {
        let policy = ZeroTrustPolicy {
            id: "p2".to_string(),
            min_trust_score: 0.3,
            require_mtls: false,
            max_auth_age_seconds: 86400,
            min_device_posture: 0.0,
            require_trusted_network: true,
            resource_pattern: "*".to_string(),
            action_pattern: "*".to_string(),
        };
        let engine = ZeroTrustEngine::with_policies(vec![policy]);

        let mut ctx = high_trust_context();
        ctx.trusted_network = false;
        let decision = engine.decide(&ctx);
        assert_eq!(
            decision,
            TrustDecision::Deny("Trusted network required".to_string())
        );
    }

    #[test]
    fn test_step_up_for_old_auth() {
        let policy = ZeroTrustPolicy {
            id: "p3".to_string(),
            min_trust_score: 0.3,
            require_mtls: false,
            max_auth_age_seconds: 600,
            min_device_posture: 0.0,
            require_trusted_network: false,
            resource_pattern: "*".to_string(),
            action_pattern: "*".to_string(),
        };
        let engine = ZeroTrustEngine::with_policies(vec![policy]);

        let mut ctx = high_trust_context();
        ctx.seconds_since_auth = 1200; // 20 minutes > 10 minute max
        let decision = engine.decide(&ctx);
        assert_eq!(decision, TrustDecision::RequireStepUp);
    }

    #[test]
    fn test_device_posture_gate() {
        let policy = ZeroTrustPolicy {
            id: "p4".to_string(),
            min_trust_score: 0.0,
            require_mtls: false,
            max_auth_age_seconds: 86400,
            min_device_posture: 0.8,
            require_trusted_network: false,
            resource_pattern: "*".to_string(),
            action_pattern: "*".to_string(),
        };
        let engine = ZeroTrustEngine::with_policies(vec![policy]);

        let mut ctx = high_trust_context();
        ctx.device_posture = 0.5;
        let decision = engine.decide(&ctx);
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_trust_score_compute() {
        let mut score = TrustScore {
            value: 0.0,
            components: vec![
                TrustComponent {
                    name: "a".to_string(),
                    value: 1.0,
                    weight: 0.6,
                    reason: "test".to_string(),
                },
                TrustComponent {
                    name: "b".to_string(),
                    value: 0.5,
                    weight: 0.4,
                    reason: "test".to_string(),
                },
            ],
            computed_at: Utc::now(),
            validity_seconds: 300,
        };
        score.compute();
        // (1.0*0.6 + 0.5*0.4) / (0.6+0.4) = (0.6+0.2)/1.0 = 0.8
        assert!((score.value - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_trust_score_expired() {
        let score = TrustScore {
            value: 0.5,
            components: vec![],
            computed_at: Utc::now() - Duration::seconds(600),
            validity_seconds: 300,
        };
        assert!(score.is_expired());

        let score = TrustScore {
            value: 0.5,
            components: vec![],
            computed_at: Utc::now(),
            validity_seconds: 300,
        };
        assert!(!score.is_expired());
    }

    #[test]
    fn test_trust_decision_display() {
        assert_eq!(TrustDecision::Allow.to_string(), "allow");
        assert_eq!(
            TrustDecision::AllowRestricted.to_string(),
            "allow_restricted"
        );
        assert_eq!(TrustDecision::RequireStepUp.to_string(), "require_step_up");
        assert_eq!(
            TrustDecision::Deny("reason".to_string()).to_string(),
            "deny: reason"
        );
    }

    #[test]
    fn test_re_evaluate_blending() {
        let engine = ZeroTrustEngine::new();
        let prev = TrustScore {
            value: 0.9,
            components: vec![],
            computed_at: Utc::now(),
            validity_seconds: 300,
        };
        let ctx = low_trust_context();
        let (new_score, _) = engine.re_evaluate(&prev, &ctx);
        // Should be blended: 0.7 * low + 0.3 * 0.9
        assert!(new_score.value < 0.9);
        assert!(new_score.value > 0.0);
    }

    #[test]
    fn test_resource_pattern_matching() {
        let policy = ZeroTrustPolicy {
            id: "p1".to_string(),
            min_trust_score: 0.0,
            require_mtls: false,
            max_auth_age_seconds: 86400,
            min_device_posture: 0.0,
            require_trusted_network: false,
            resource_pattern: "workflow".to_string(),
            action_pattern: "Execute".to_string(),
        };
        let engine = ZeroTrustEngine::with_policies(vec![policy]);

        // Matching resource+action
        let mut ctx = high_trust_context();
        ctx.resource = "workflow".to_string();
        ctx.action = "Execute".to_string();
        assert!(engine.decide(&ctx).is_allowed());

        // Non-matching resource — falls through to default policy
        ctx.resource = "agent".to_string();
        // High trust context should still be allowed by default
        assert!(engine.decide(&ctx).is_allowed());
    }
}
