//! # Autonomy Certificate
//!
//! Issues cryptographic certificates that encode an agent's autonomy level
//! and capability permissions. These certificates can be presented to
//! external systems for trust verification.
//!
//! ## Certificate Structure
//!
//! - **Subject**: Agent identifier
//! - **Autonomy Level**: Suggest / AutoEdit / FullAuto
//! - **Granted Capabilities**: Set of permitted tools/actions
//! - **Expiry**: Validity period
//! - **Issuer**: CA that signed the certificate
//!
//! ## Design
//!
//! ```text
//! AutonomyCertificate ──► CertificateChain ──► TrustVerifier
//!        │                         │                  │
//!        ├── subject               │           ┌──────┴──────┐
//!        ├── level                 │           │ is_trusted()|
//!        ├── capabilities          │           │ can_do()    |
//!        ├── validity              │           └─────────────┘
//!        └── signature             └── issuer
//! ```

use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashSet;

/// Autonomy level encoded in the certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CertAutonomyLevel {
    Suggest,
    AutoEdit,
    FullAuto,
}

impl CertAutonomyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            CertAutonomyLevel::Suggest => "suggest",
            CertAutonomyLevel::AutoEdit => "auto_edit",
            CertAutonomyLevel::FullAuto => "full_auto",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "suggest" => Some(CertAutonomyLevel::Suggest),
            "auto_edit" | "autoedit" => Some(CertAutonomyLevel::AutoEdit),
            "full_auto" | "fullauto" => Some(CertAutonomyLevel::FullAuto),
            _ => None,
        }
    }

    /// Rank: higher values = more autonomous
    pub fn rank(&self) -> u8 {
        match self {
            CertAutonomyLevel::Suggest => 1,
            CertAutonomyLevel::AutoEdit => 2,
            CertAutonomyLevel::FullAuto => 3,
        }
    }

    /// True if this level permits automatic execution (commands, tool calls).
    pub fn permits_auto_execution(&self) -> bool {
        matches!(self, CertAutonomyLevel::FullAuto)
    }

    /// True if this level permits automatic file edits.
    pub fn permits_auto_edit(&self) -> bool {
        matches!(
            self,
            CertAutonomyLevel::AutoEdit | CertAutonomyLevel::FullAuto
        )
    }
}

impl std::fmt::Display for CertAutonomyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single capability granted to an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Capability {
    pub name: String,
    /// Optional constraints (e.g., max_rate, allowed_paths)
    pub constraints: HashSet<String>,
}

impl Capability {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            constraints: HashSet::new(),
        }
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraints.insert(constraint.into());
        self
    }
}

/// An autonomy certificate — encodes an agent's autonomy level and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyCertificate {
    /// Unique certificate serial number
    pub serial: String,
    /// Agent this certificate is issued to
    pub subject_id: String,
    /// Human-readable subject name
    pub subject_name: String,
    /// The autonomy level granted
    pub autonomy_level: CertAutonomyLevel,
    /// Capabilities granted by this certificate
    pub capabilities: Vec<Capability>,
    /// When the certificate becomes valid
    pub valid_from: DateTime<Utc>,
    /// When the certificate expires
    pub valid_until: DateTime<Utc>,
    /// The issuer (CA) that signed this certificate
    pub issuer_id: String,
    /// Digital signature over the certificate payload
    pub signature: String,
    /// Optional: certificate purpose / policy reference
    pub policy_ref: Option<String>,
}

impl AutonomyCertificate {
    /// Create a new self-signed certificate for testing.
    pub fn new_self_signed(
        subject_id: impl Into<String>,
        subject_name: impl Into<String>,
        level: CertAutonomyLevel,
        capabilities: Vec<Capability>,
        validity_days: i64,
    ) -> Self {
        let now = Utc::now();
        let serial = uuid::Uuid::new_v4().to_string();
        let subject_id_str: String = subject_id.into();
        let payload = format!("{}:{}:{}", serial, subject_id_str, level.as_str());

        Self {
            serial,
            subject_id: subject_id_str,
            subject_name: subject_name.into(),
            autonomy_level: level,
            capabilities,
            valid_from: now,
            valid_until: now + Duration::days(validity_days),
            issuer_id: "self".to_string(),
            signature: Self::sign_payload(&payload),
            policy_ref: None,
        }
    }

    /// Create a certificate issued by a specific CA.
    pub fn new_issued(
        subject_id: impl Into<String>,
        subject_name: impl Into<String>,
        level: CertAutonomyLevel,
        capabilities: Vec<Capability>,
        validity_days: i64,
        issuer_id: impl Into<String>,
        ca_private_key: &str,
    ) -> Self {
        let now = Utc::now();
        let serial = uuid::Uuid::new_v4().to_string();
        let subject_id_str: String = subject_id.into();
        let payload = format!("{}:{}:{}", serial, subject_id_str, level.as_str());

        Self {
            serial,
            subject_id: subject_id_str,
            subject_name: subject_name.into(),
            autonomy_level: level,
            capabilities,
            valid_from: now,
            valid_until: now + Duration::days(validity_days),
            issuer_id: issuer_id.into(),
            signature: Self::sign_with_key(&payload, ca_private_key),
            policy_ref: None,
        }
    }

    /// Check if the certificate is currently valid.
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.valid_from && now <= self.valid_until
    }

    /// Check if the certificate has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.valid_until
    }

    /// Check if the certificate is not yet valid.
    pub fn is_not_yet_valid(&self) -> bool {
        Utc::now() < self.valid_from
    }

    /// Check if a specific capability is granted.
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == name)
    }

    /// Get remaining validity duration.
    pub fn remaining_validity(&self) -> Option<Duration> {
        let now = Utc::now();
        if now > self.valid_until {
            return None;
        }
        Some(self.valid_until.signed_duration_since(now))
    }

    /// Days until expiration (negative if expired).
    pub fn days_until_expiry(&self) -> i64 {
        let remaining = self.remaining_validity();
        remaining.map(|d| d.num_days()).unwrap_or(-1)
    }

    /// Whether the certificate is within the high-risk expiry window.
    pub fn is_near_expiry(&self, threshold_days: i64) -> bool {
        self.remaining_validity()
            .map(|d| d.num_days() <= threshold_days)
            .unwrap_or(false)
    }

    /// Verify the certificate's signature (simplified — uses HMAC-SHA256).
    fn sign_payload(payload: &str) -> String {
        Self::sign_with_key(payload, "default-ca-key")
    }

    fn sign_with_key(payload: &str, key: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts any key size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Verify the certificate's signature.
    pub fn verify_signature(&self, ca_public_key: &str) -> bool {
        let payload = format!(
            "{}:{}:{}",
            self.serial,
            self.subject_id,
            self.autonomy_level.as_str()
        );
        let expected = Self::sign_with_key(&payload, ca_public_key);
        self.signature == expected
    }

    /// Upgrade the certificate's autonomy level.
    pub fn with_upgraded_level(mut self, new_level: CertAutonomyLevel) -> Self {
        if new_level.rank() <= self.autonomy_level.rank() {
            panic!("Cannot upgrade to a lower or equal autonomy level");
        }
        self.autonomy_level = new_level;
        // Re-sign
        let payload = format!(
            "{}:{}:{}",
            self.serial,
            self.subject_id,
            self.autonomy_level.as_str()
        );
        self.signature = Self::sign_payload(&payload);
        self
    }

    /// Add a capability to the certificate.
    pub fn with_added_capability(mut self, cap: Capability) -> Self {
        self.capabilities.push(cap);
        let payload = format!(
            "{}:{}:{}",
            self.serial,
            self.subject_id,
            self.autonomy_level.as_str()
        );
        self.signature = Self::sign_payload(&payload);
        self
    }

    /// Revoke the certificate by setting expiry to now.
    pub fn revoke(mut self) -> Self {
        self.valid_until = Utc::now() - Duration::seconds(1);
        self
    }
}

/// A chain of certificates for trust verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateChain {
    /// The leaf certificate (agent's own certificate)
    pub leaf: AutonomyCertificate,
    /// Intermediate certificates (if any)
    pub intermediates: Vec<AutonomyCertificate>,
    /// The root (trust anchor) certificate
    pub root: Option<AutonomyCertificate>,
}

impl CertificateChain {
    /// Build a chain from a leaf certificate up to a root.
    pub fn new(leaf: AutonomyCertificate) -> Self {
        Self {
            leaf,
            intermediates: Vec::new(),
            root: None,
        }
    }

    pub fn with_intermediates(mut self, intermediates: Vec<AutonomyCertificate>) -> Self {
        self.intermediates = intermediates;
        self
    }

    pub fn with_root(mut self, root: AutonomyCertificate) -> Self {
        self.root = Some(root);
        self
    }

    /// Verify the entire chain is valid and not expired.
    pub fn is_valid_chain(&self) -> bool {
        if !self.leaf.is_valid() {
            return false;
        }
        for ic in &self.intermediates {
            if !ic.is_valid() {
                return false;
            }
        }
        if let Some(ref root) = self.root {
            if !root.is_valid() {
                return false;
            }
        }
        true
    }
}

/// Trust verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustVerificationResult {
    /// Whether the certificate is trusted
    pub trusted: bool,
    /// The autonomy level that was verified
    pub verified_level: Option<CertAutonomyLevel>,
    /// Whether the specific action was permitted
    pub action_permitted: bool,
    /// Explanation of the trust decision
    pub reason: String,
    /// Warnings (e.g., near expiry)
    pub warnings: Vec<String>,
}

/// Verifies trust based on an autonomy certificate.
pub struct TrustVerifier {
    trusted_issuers: HashSet<String>,
    min_required_level: CertAutonomyLevel,
    expiry_warning_threshold_days: i64,
}

impl TrustVerifier {
    pub fn new() -> Self {
        Self {
            trusted_issuers: HashSet::new(),
            min_required_level: CertAutonomyLevel::Suggest,
            expiry_warning_threshold_days: 7,
        }
    }

    pub fn with_trusted_issuer(mut self, issuer_id: impl Into<String>) -> Self {
        self.trusted_issuers.insert(issuer_id.into());
        self
    }

    pub fn with_min_required_level(mut self, level: CertAutonomyLevel) -> Self {
        self.min_required_level = level;
        self
    }

    pub fn with_expiry_warning_threshold(mut self, days: i64) -> Self {
        self.expiry_warning_threshold_days = days;
        self
    }

    /// Verify whether an agent with the given certificate can perform an action.
    pub fn verify_action(
        &self,
        cert: &AutonomyCertificate,
        action: &str,
    ) -> TrustVerificationResult {
        let mut warnings = Vec::new();

        // Check validity
        if cert.is_expired() {
            return TrustVerificationResult {
                trusted: false,
                verified_level: None,
                action_permitted: false,
                reason: "Certificate has expired".to_string(),
                warnings: vec!["Certificate expired".to_string()],
            };
        }

        if cert.is_not_yet_valid() {
            return TrustVerificationResult {
                trusted: false,
                verified_level: None,
                action_permitted: false,
                reason: "Certificate is not yet valid".to_string(),
                warnings: vec!["Certificate not yet valid".to_string()],
            };
        }

        // Check issuer trust
        let issuer_trusted = self.trusted_issuers.is_empty()
            || self.trusted_issuers.contains(&cert.issuer_id)
            || cert.issuer_id == "self";
        if !issuer_trusted {
            return TrustVerificationResult {
                trusted: false,
                verified_level: Some(cert.autonomy_level.clone()),
                action_permitted: false,
                reason: format!("Issuer '{}' is not trusted", cert.issuer_id),
                warnings: vec![],
            };
        }

        // Check autonomy level
        if cert.autonomy_level.rank() < self.min_required_level.rank() {
            return TrustVerificationResult {
                trusted: false,
                verified_level: Some(cert.autonomy_level.clone()),
                action_permitted: false,
                reason: format!(
                    "Certificate autonomy level '{}' is below minimum '{}'",
                    cert.autonomy_level, self.min_required_level
                ),
                warnings: vec![],
            };
        }

        // Check expiry warning
        if cert.is_near_expiry(self.expiry_warning_threshold_days) {
            warnings.push(format!(
                "Certificate expires in {} days",
                cert.days_until_expiry()
            ));
        }

        // Determine if action is permitted
        let action_permitted = self.check_action_permission(cert, action);

        TrustVerificationResult {
            trusted: true,
            verified_level: Some(cert.autonomy_level.clone()),
            action_permitted,
            reason: if action_permitted {
                format!("Action '{}' permitted by certificate", action)
            } else {
                format!(
                    "Action '{}' not permitted by certificate capabilities",
                    action
                )
            },
            warnings,
        }
    }

    /// Verify a complete certificate chain.
    pub fn verify_chain(&self, chain: &CertificateChain) -> TrustVerificationResult {
        if !chain.is_valid_chain() {
            return TrustVerificationResult {
                trusted: false,
                verified_level: None,
                action_permitted: false,
                reason: "Certificate chain is invalid or contains expired certificates".to_string(),
                warnings: vec![],
            };
        }

        // Use leaf certificate for verification
        self.verify_action(&chain.leaf, "*")
    }

    fn check_action_permission(&self, cert: &AutonomyCertificate, action: &str) -> bool {
        // Wildcard always permitted if cert is valid
        if action == "*" {
            return true;
        }

        // Check capability match
        let action_base = action.split('.').next().unwrap_or(action);
        cert.has_capability(action) || cert.has_capability(action_base) || cert.has_capability("*")
    }
}

impl Default for TrustVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cert(level: CertAutonomyLevel) -> AutonomyCertificate {
        AutonomyCertificate::new_self_signed(
            "agent-test-001",
            "Test Agent",
            level,
            vec![
                Capability::new("tool.call"),
                Capability::new("data.read"),
                Capability::new("file.edit"),
            ],
            30,
        )
    }

    // --- CertAutonomyLevel tests ---

    #[test]
    fn test_autonomy_level_from_str() {
        assert_eq!(
            CertAutonomyLevel::from_str("suggest"),
            Some(CertAutonomyLevel::Suggest)
        );
        assert_eq!(
            CertAutonomyLevel::from_str("auto_edit"),
            Some(CertAutonomyLevel::AutoEdit)
        );
        assert_eq!(
            CertAutonomyLevel::from_str("full_auto"),
            Some(CertAutonomyLevel::FullAuto)
        );
        assert_eq!(CertAutonomyLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_autonomy_level_rank() {
        assert_eq!(CertAutonomyLevel::Suggest.rank(), 1);
        assert_eq!(CertAutonomyLevel::AutoEdit.rank(), 2);
        assert_eq!(CertAutonomyLevel::FullAuto.rank(), 3);
        assert!(CertAutonomyLevel::FullAuto.rank() > CertAutonomyLevel::AutoEdit.rank());
    }

    #[test]
    fn test_autonomy_level_permits() {
        assert!(!CertAutonomyLevel::Suggest.permits_auto_execution());
        assert!(CertAutonomyLevel::FullAuto.permits_auto_execution());

        assert!(!CertAutonomyLevel::Suggest.permits_auto_edit());
        assert!(CertAutonomyLevel::AutoEdit.permits_auto_edit());
        assert!(CertAutonomyLevel::FullAuto.permits_auto_edit());
    }

    // --- AutonomyCertificate tests ---

    #[test]
    fn test_new_self_signed_certificate() {
        let cert = make_test_cert(CertAutonomyLevel::AutoEdit);
        assert_eq!(cert.subject_id, "agent-test-001");
        assert_eq!(cert.autonomy_level, CertAutonomyLevel::AutoEdit);
        assert_eq!(cert.issuer_id, "self");
        assert!(cert.is_valid());
        assert!(!cert.is_expired());
    }

    #[test]
    fn test_certificate_validity_bounds() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let now = Utc::now();
        assert!(cert.valid_from <= now);
        assert!(cert.valid_until > now);
    }

    #[test]
    fn test_certificate_is_valid() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        assert!(cert.is_valid());
        assert!(!cert.is_expired());
        assert!(!cert.is_not_yet_valid());
    }

    #[test]
    fn test_certificate_has_capability() {
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        assert!(cert.has_capability("tool.call"));
        assert!(cert.has_capability("data.read"));
        assert!(!cert.has_capability("admin.delete"));
    }

    #[test]
    fn test_certificate_days_until_expiry() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let days = cert.days_until_expiry();
        assert!(days >= 29 && days <= 30);
    }

    #[test]
    fn test_certificate_is_near_expiry() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        assert!(!cert.is_near_expiry(30)); // 30 days cert, threshold 30
        assert!(cert.is_near_expiry(31)); // Should be near expiry when threshold > validity
    }

    #[test]
    fn test_certificate_remaining_validity() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let remaining = cert.remaining_validity();
        assert!(remaining.is_some());
        let days = remaining.unwrap().num_days();
        assert!(days >= 29 && days <= 30);
    }

    #[test]
    fn test_certificate_remaining_validity_expired() {
        let cert = AutonomyCertificate::new_self_signed(
            "agent-expired",
            "Expired Agent",
            CertAutonomyLevel::Suggest,
            vec![],
            0, // Expires today
        );
        assert!(cert.remaining_validity().is_none());
    }

    #[test]
    fn test_certificate_revoke() {
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        let revoked = cert.clone().revoke();
        assert!(revoked.is_expired());
        assert!(!revoked.is_valid());
    }

    #[test]
    fn test_certificate_upgrade_level() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let upgraded = cert.with_upgraded_level(CertAutonomyLevel::FullAuto);
        assert_eq!(upgraded.autonomy_level, CertAutonomyLevel::FullAuto);
        assert!(upgraded.is_valid());
    }

    #[test]
    fn test_certificate_add_capability() {
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let original_len = cert.capabilities.len();
        let extended = cert.with_added_capability(Capability::new("new.capability"));
        assert!(extended.has_capability("new.capability"));
        assert!(extended.capabilities.len() > original_len);
    }

    #[test]
    fn test_certificate_signature_verification() {
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        // Self-signed: use "self" as public key equivalent
        assert!(cert.verify_signature("default-ca-key"));
        assert!(!cert.verify_signature("wrong-key"));
    }

    #[test]
    fn test_certificate_serde_roundtrip() {
        let cert = make_test_cert(CertAutonomyLevel::AutoEdit);
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: AutonomyCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.subject_id, cert.subject_id);
        assert_eq!(decoded.autonomy_level, cert.autonomy_level);
        assert_eq!(decoded.capabilities.len(), cert.capabilities.len());
    }

    // --- Capability tests ---

    #[test]
    fn test_capability_builder() {
        let cap = Capability::new("data.write")
            .with_constraint("max_size_mb=100")
            .with_constraint("allowed_paths=/data/*");

        assert_eq!(cap.name, "data.write");
        assert_eq!(cap.constraints.len(), 2);
    }

    #[test]
    fn test_capability_equality() {
        let a = Capability::new("tool.call");
        let b = Capability::new("tool.call");
        let c = Capability::new("tool.delete");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // --- CertificateChain tests ---

    #[test]
    fn test_certificate_chain_valid() {
        let leaf = make_test_cert(CertAutonomyLevel::FullAuto);
        let chain = CertificateChain::new(leaf);
        assert!(chain.is_valid_chain());
    }

    #[test]
    fn test_certificate_chain_with_intermediates() {
        let leaf = make_test_cert(CertAutonomyLevel::FullAuto);
        let chain = CertificateChain::new(leaf).with_intermediates(vec![]);
        assert!(chain.is_valid_chain());
    }

    #[test]
    fn test_certificate_chain_with_root() {
        let leaf = make_test_cert(CertAutonomyLevel::FullAuto);
        let root = AutonomyCertificate::new_self_signed(
            "root-ca",
            "Root CA",
            CertAutonomyLevel::FullAuto,
            vec![],
            365,
        );
        let chain = CertificateChain::new(leaf).with_root(root);
        assert!(chain.is_valid_chain());
    }

    #[test]
    fn test_certificate_chain_is_valid_chain_false_on_expired_leaf() {
        let expired = AutonomyCertificate::new_self_signed(
            "agent-expired",
            "Expired",
            CertAutonomyLevel::Suggest,
            vec![],
            0,
        );
        let chain = CertificateChain::new(expired);
        assert!(!chain.is_valid_chain());
    }

    // --- TrustVerifier tests ---

    #[test]
    fn test_verifier_trusts_valid_cert() {
        let verifier = TrustVerifier::new();
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(result.trusted);
        assert!(result.action_permitted);
    }

    #[test]
    fn test_verifier_rejects_expired_cert() {
        let verifier = TrustVerifier::new();
        let expired = AutonomyCertificate::new_self_signed(
            "agent-expired",
            "Expired",
            CertAutonomyLevel::FullAuto,
            vec![],
            0,
        );
        let result = verifier.verify_action(&expired, "tool.call");
        assert!(!result.trusted);
        assert!(!result.action_permitted);
        assert!(result.reason.contains("expired"));
    }

    #[test]
    fn test_verifier_rejects_low_autonomy_level() {
        let verifier = TrustVerifier::new().with_min_required_level(CertAutonomyLevel::AutoEdit);
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(!result.trusted);
        assert!(result.reason.contains("below minimum"));
    }

    #[test]
    fn test_verifier_accepts_sufficient_autonomy_level() {
        let verifier = TrustVerifier::new().with_min_required_level(CertAutonomyLevel::Suggest);
        let cert = make_test_cert(CertAutonomyLevel::Suggest);
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(result.trusted);
    }

    #[test]
    fn test_verifier_wildcard_action() {
        let verifier = TrustVerifier::new();
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        let result = verifier.verify_action(&cert, "*");
        assert!(result.action_permitted);
    }

    #[test]
    fn test_verifier_checks_capability() {
        let verifier = TrustVerifier::new();
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        let result = verifier.verify_action(&cert, "admin.delete");
        assert!(result.trusted);
        assert!(!result.action_permitted); // Not in capabilities
    }

    #[test]
    fn test_verifier_near_expiry_warning() {
        let verifier = TrustVerifier::new().with_expiry_warning_threshold(30);

        // Create a cert that expires in 5 days
        let cert = AutonomyCertificate::new_self_signed(
            "agent-5days",
            "5 Day Agent",
            CertAutonomyLevel::FullAuto,
            vec![],
            5,
        );
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(result.trusted);
        assert!(!result.warnings.is_empty());
        assert!(result.warnings.iter().any(|w| w.contains("expires in")));
    }

    #[test]
    fn test_verifier_chain_verification() {
        let verifier = TrustVerifier::new();
        let leaf = make_test_cert(CertAutonomyLevel::FullAuto);
        let chain = CertificateChain::new(leaf);
        let result = verifier.verify_chain(&chain);
        assert!(result.trusted);
    }

    #[test]
    fn test_verifier_untrusted_issuer() {
        let verifier = TrustVerifier::new().with_trusted_issuer("trusted-ca");
        let cert = AutonomyCertificate::new_issued(
            "agent-1",
            "Agent One",
            CertAutonomyLevel::FullAuto,
            vec![],
            30,
            "untrusted-ca",
            "ca-secret",
        );
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(!result.trusted);
        assert!(result.reason.contains("not trusted"));
    }

    #[test]
    fn test_verifier_self_issuer_always_trusted() {
        let verifier = TrustVerifier::new().with_trusted_issuer("trusted-ca");
        let cert = AutonomyCertificate::new_self_signed(
            "agent-self",
            "Self Agent",
            CertAutonomyLevel::FullAuto,
            vec![],
            30,
        );
        let result = verifier.verify_action(&cert, "tool.call");
        assert!(result.trusted); // self is always trusted
    }

    // --- Serde tests ---

    #[test]
    fn test_trust_verification_result_serde() {
        let verifier = TrustVerifier::new();
        let cert = make_test_cert(CertAutonomyLevel::FullAuto);
        let result = verifier.verify_action(&cert, "tool.call");
        let json = serde_json::to_string(&result).unwrap();
        let decoded: TrustVerificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.trusted, true);
    }

    #[test]
    fn test_certificate_chain_serde() {
        let leaf = make_test_cert(CertAutonomyLevel::FullAuto);
        let chain = CertificateChain::new(leaf);
        let json = serde_json::to_string(&chain).unwrap();
        let decoded: CertificateChain = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.leaf.subject_id, "agent-test-001");
    }
}
