//! # Digital Signature PKI
//!
//! Public Key Infrastructure for AgentGuard: certificate management,
//! digital signing/verification, and certificate chain validation.
//! Supports X.509-style certificate structures with configurable algorithms.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

// ── Errors ─────────────────────────────────────────────────────────────

/// PKI operation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkiError {
    /// Certificate has expired.
    CertificateExpired,
    /// Certificate is not yet valid.
    CertificateNotYetValid,
    /// Certificate signature verification failed.
    SignatureInvalid,
    /// Certificate chain is broken.
    ChainBroken(String),
    /// Key pair generation failed.
    KeyGenerationFailed(String),
    /// Signing operation failed.
    SigningFailed(String),
    /// The requested entity was not found.
    NotFound(String),
    /// Configuration or input error.
    InvalidInput(String),
}

impl fmt::Display for PkiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertificateExpired => write!(f, "Certificate expired"),
            Self::CertificateNotYetValid => write!(f, "Certificate not yet valid"),
            Self::SignatureInvalid => write!(f, "Signature invalid"),
            Self::ChainBroken(e) => write!(f, "Chain broken: {e}"),
            Self::KeyGenerationFailed(e) => write!(f, "Key generation failed: {e}"),
            Self::SigningFailed(e) => write!(f, "Signing failed: {e}"),
            Self::NotFound(e) => write!(f, "Not found: {e}"),
            Self::InvalidInput(e) => write!(f, "Invalid input: {e}"),
        }
    }
}

impl std::error::Error for PkiError {}

// ── Signature Algorithm ────────────────────────────────────────────────

/// Supported signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    /// RSA with SHA-256.
    RsaSha256,
    /// ECDSA with P-256 curve.
    EcdsaP256,
    /// Ed25519.
    Ed25519,
}

impl fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RsaSha256 => write!(f, "RSA-SHA256"),
            Self::EcdsaP256 => write!(f, "ECDSA-P256"),
            Self::Ed25519 => write!(f, "Ed25519"),
        }
    }
}

// ── Distinguished Name ─────────────────────────────────────────────────

/// X.509-style Distinguished Name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistinguishedName {
    pub common_name: String,
    pub organization: Option<String>,
    pub organizational_unit: Option<String>,
    pub country: Option<String>,
    pub email: Option<String>,
}

impl DistinguishedName {
    pub fn new(common_name: &str) -> Self {
        Self {
            common_name: common_name.to_string(),
            organization: None,
            organizational_unit: None,
            country: None,
            email: None,
        }
    }

    /// Render as LDAP-style DN string.
    pub fn to_dn_string(&self) -> String {
        let mut parts = vec![format!("CN={}", self.common_name)];
        if let Some(ref org) = self.organization {
            parts.push(format!("O={org}"));
        }
        if let Some(ref ou) = self.organizational_unit {
            parts.push(format!("OU={ou}"));
        }
        if let Some(ref c) = self.country {
            parts.push(format!("C={c}"));
        }
        parts.join(", ")
    }
}

impl fmt::Display for DistinguishedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dn_string())
    }
}

// ── Key Pair ───────────────────────────────────────────────────────────

/// A public/private key pair (simplified for in-memory use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    /// Algorithm used.
    pub algorithm: SignatureAlgorithm,
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Private key bytes (should be stored securely in production).
    pub private_key: Vec<u8>,
    /// Key fingerprint (SHA-256 of public key).
    pub fingerprint: String,
    /// When this key was generated.
    pub created_at: DateTime<Utc>,
}

impl KeyPair {
    /// Generate a new key pair (simulated — production uses ring/ed25519-dalek).
    pub fn generate(algorithm: SignatureAlgorithm) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let mut pub_bytes = vec![0u8; 32];
        let mut priv_bytes = vec![0u8; 64];

        // Deterministic but unique key material (NOT cryptographically secure — simulation)
        for (i, item) in pub_bytes.iter_mut().enumerate().take(32) {
            *item = ((seed >> (i % 16 * 8)) & 0xFF) as u8 ^ (i as u8);
        }
        for (i, item) in priv_bytes.iter_mut().enumerate().take(64) {
            *item = ((seed >> ((i + 32) % 16 * 8)) & 0xFF) as u8 ^ (i as u8 + 0x80);
        }

        let fingerprint = sha256_hex(&pub_bytes);

        Self {
            algorithm,
            public_key: pub_bytes,
            private_key: priv_bytes,
            fingerprint,
            created_at: Utc::now(),
        }
    }

    /// Sign data (simulated HMAC-based signature).
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        // In production: use ring or ed25519-dalek
        // Here: HMAC-SHA256(private_key, data) as signature
        hmac_sha256(&self.private_key, data)
    }

    /// Verify a signature against this key pair's public key.
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        // Simulate verification: re-sign with private key and compare
        let expected = self.sign(data);
        expected == signature
    }
}

// ── Certificate ────────────────────────────────────────────────────────

/// X.509-style certificate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    /// Serial number.
    pub serial: String,
    /// Subject (who this cert belongs to).
    pub subject: DistinguishedName,
    /// Issuer (who signed this cert).
    pub issuer: DistinguishedName,
    /// Signature algorithm.
    pub algorithm: SignatureAlgorithm,
    /// Subject's public key.
    pub public_key: Vec<u8>,
    /// Not valid before.
    pub not_before: DateTime<Utc>,
    /// Not valid after.
    pub not_after: DateTime<Utc>,
    /// Certificate signature (over the TBS bytes).
    pub signature: Vec<u8>,
    /// Whether this is a CA certificate.
    pub is_ca: bool,
    /// Key usage extensions.
    pub key_usage: Vec<KeyUsage>,
    /// Certificate fingerprint (SHA-256 of DER encoding).
    pub fingerprint: String,
}

/// X.509 Key Usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KeyUsage {
    DigitalSignature,
    KeyEncipherment,
    KeyCertSign,
    CrlSign,
    DataEncipherment,
}

impl Certificate {
    /// Check if the certificate is currently valid.
    pub fn is_valid(&self) -> Result<(), PkiError> {
        let now = Utc::now();
        if now < self.not_before {
            return Err(PkiError::CertificateNotYetValid);
        }
        if now > self.not_after {
            return Err(PkiError::CertificateExpired);
        }
        Ok(())
    }

    /// Check if this certificate is a self-signed root.
    pub fn is_self_signed(&self) -> bool {
        self.subject == self.issuer
    }

    /// Compute TBS (To-Be-Signed) bytes for verification.
    pub fn tbs_bytes(&self) -> Vec<u8> {
        // Simplified: hash of serial + subject + issuer + public_key + validity
        let mut hasher = Sha256::new();
        hasher.update(self.serial.as_bytes());
        hasher.update(self.subject.to_dn_string().as_bytes());
        hasher.update(self.issuer.to_dn_string().as_bytes());
        hasher.update(&self.public_key);
        hasher.update(self.not_before.to_rfc3339().as_bytes());
        hasher.update(self.not_after.to_rfc3339().as_bytes());
        hasher.finalize().to_vec()
    }

    /// Verify the certificate's signature against an issuer's public key.
    pub fn verify_signature(&self, issuer_public_key: &[u8]) -> bool {
        // Look up the key pair by public key to get the private key for verification
        // In this simplified HMAC scheme, signing uses private_key, so verify must too
        let tbs = self.tbs_bytes();
        // For certificate verification, we need the issuer's private key
        // This is called from verify_chain which has access to PkiManager
        // As a standalone method, we compare against the signature directly
        // The caller (verify_chain) handles the actual key lookup
        let expected_sig = hmac_sha256(issuer_public_key, &tbs);
        expected_sig == self.signature
    }
}

// ── PKI Manager ────────────────────────────────────────────────────────

/// Certificate Authority and certificate lifecycle manager.
pub struct PkiManager {
    /// Root CA certificates.
    root_cas: HashMap<String, Certificate>,
    /// Intermediate CA certificates.
    intermediate_cas: HashMap<String, Certificate>,
    /// End-entity certificates.
    certificates: HashMap<String, Certificate>,
    /// Key pairs (fingerprint -> key pair).
    key_pairs: HashMap<String, KeyPair>,
    /// Revoked certificate serials.
    revoked: std::collections::HashSet<String>,
}

impl PkiManager {
    pub fn new() -> Self {
        Self {
            root_cas: HashMap::new(),
            intermediate_cas: HashMap::new(),
            certificates: HashMap::new(),
            key_pairs: HashMap::new(),
            revoked: std::collections::HashSet::new(),
        }
    }

    /// Create a self-signed root CA.
    pub fn create_root_ca(
        &mut self,
        subject: DistinguishedName,
        validity_days: i64,
    ) -> Result<(Certificate, &KeyPair), PkiError> {
        let kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        let serial = format!("CA-{}", &kp.fingerprint[..16]);
        let now = Utc::now();

        let mut cert = Certificate {
            serial: serial.clone(),
            subject: subject.clone(),
            issuer: subject,
            algorithm: SignatureAlgorithm::RsaSha256,
            public_key: kp.public_key.clone(),
            not_before: now,
            not_after: now + Duration::days(validity_days),
            signature: Vec::new(), // self-signed
            is_ca: true,
            key_usage: vec![KeyUsage::KeyCertSign, KeyUsage::CrlSign],
            fingerprint: String::new(),
        };

        // Self-sign
        let tbs = cert.tbs_bytes();
        cert.signature = kp.sign(&tbs);
        cert.fingerprint = sha256_hex(&cert.tbs_bytes());

        let fp = kp.fingerprint.clone();
        self.root_cas.insert(serial.clone(), cert.clone());
        self.key_pairs.insert(fp.clone(), kp);

        Ok((
            cert,
            self.key_pairs
                .get(&fp)
                .expect("key_pairs just inserted with same fingerprint"),
        ))
    }

    /// Issue a certificate signed by a CA.
    pub fn issue_certificate(
        &mut self,
        ca_serial: &str,
        subject: DistinguishedName,
        subject_public_key: Vec<u8>,
        validity_days: i64,
        is_ca: bool,
        key_usage: Vec<KeyUsage>,
    ) -> Result<Certificate, PkiError> {
        // Find the CA cert and key
        let ca_cert = self
            .root_cas
            .get(ca_serial)
            .or_else(|| self.intermediate_cas.get(ca_serial))
            .ok_or_else(|| PkiError::NotFound(format!("CA {ca_serial} not found")))?;

        ca_cert.is_valid()?;

        let ca_kp = self
            .key_pairs
            .values()
            .find(|kp| kp.public_key == ca_cert.public_key)
            .ok_or_else(|| PkiError::NotFound("CA key pair not found".to_string()))?;

        let serial = format!("CERT-{}", &sha256_hex(&subject_public_key)[..16]);
        let now = Utc::now();

        let mut cert = Certificate {
            serial: serial.clone(),
            subject,
            issuer: ca_cert.subject.clone(),
            algorithm: ca_cert.algorithm,
            public_key: subject_public_key,
            not_before: now,
            not_after: now + Duration::days(validity_days),
            signature: Vec::new(),
            is_ca,
            key_usage,
            fingerprint: String::new(),
        };

        let tbs = cert.tbs_bytes();
        cert.signature = ca_kp.sign(&tbs);
        cert.fingerprint = sha256_hex(&cert.tbs_bytes());

        if is_ca {
            self.intermediate_cas.insert(serial, cert.clone());
        } else {
            self.certificates.insert(serial, cert.clone());
        }

        Ok(cert)
    }

    /// Verify a certificate chain from leaf to root.
    pub fn verify_chain(&self, cert: &Certificate) -> Result<(), PkiError> {
        cert.is_valid()?;

        if cert.is_self_signed() {
            // Must be a trusted root CA
            if self.root_cas.values().any(|ca| ca.serial == cert.serial) {
                return Ok(());
            }
            return Err(PkiError::ChainBroken(
                "Self-signed cert not in trusted roots".to_string(),
            ));
        }

        // Find issuer
        let issuer_cert = self
            .root_cas
            .values()
            .chain(self.intermediate_cas.values())
            .find(|ca| ca.subject == cert.issuer)
            .ok_or_else(|| {
                PkiError::ChainBroken(format!("Issuer '{}' not found", cert.issuer.to_dn_string()))
            })?;

        // Verify signature using issuer's private key (HMAC scheme)
        let issuer_kp = self
            .key_pairs
            .values()
            .find(|kp| kp.public_key == issuer_cert.public_key)
            .ok_or_else(|| PkiError::NotFound("Issuer key pair not found".to_string()))?;
        let tbs = cert.tbs_bytes();
        let expected_sig = hmac_sha256(&issuer_kp.private_key, &tbs);
        if expected_sig != cert.signature {
            return Err(PkiError::SignatureInvalid);
        }

        // Recursively verify issuer chain
        self.verify_chain(issuer_cert)
    }

    /// Revoke a certificate.
    pub fn revoke(&mut self, serial: &str) {
        self.revoked.insert(serial.to_string());
    }

    /// Check if a certificate is revoked.
    pub fn is_revoked(&self, serial: &str) -> bool {
        self.revoked.contains(serial)
    }

    /// Get certificate by serial.
    pub fn get_certificate(&self, serial: &str) -> Option<&Certificate> {
        self.certificates
            .get(serial)
            .or_else(|| self.intermediate_cas.get(serial))
            .or_else(|| self.root_cas.get(serial))
    }

    /// Sign arbitrary data with a key identified by fingerprint.
    pub fn sign_data(&self, key_fingerprint: &str, data: &[u8]) -> Result<Vec<u8>, PkiError> {
        let kp = self
            .key_pairs
            .get(key_fingerprint)
            .ok_or_else(|| PkiError::NotFound(format!("Key {key_fingerprint} not found")))?;
        Ok(kp.sign(data))
    }

    /// Verify a signature against a certificate's public key.
    pub fn verify_data(
        &self,
        cert_serial: &str,
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, PkiError> {
        let cert = self
            .get_certificate(cert_serial)
            .ok_or_else(|| PkiError::NotFound(format!("Certificate {cert_serial} not found")))?;

        // Find the key pair matching this cert's public key, verify with private key
        let kp = self
            .key_pairs
            .values()
            .find(|kp| kp.public_key == cert.public_key)
            .ok_or_else(|| PkiError::NotFound("Certificate key pair not found".to_string()))?;
        let expected = hmac_sha256(&kp.private_key, data);
        Ok(expected == signature)
    }

    /// List all certificates.
    pub fn list_certificates(&self) -> Vec<&Certificate> {
        self.certificates.values().collect()
    }

    /// Register an externally-generated key pair so it can be found by sign_data.
    pub fn register_key_pair(&mut self, kp: KeyPair) {
        self.key_pairs.insert(kp.fingerprint.clone(), kp);
    }
}

impl Default for PkiManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA256 accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distinguished_name() {
        let dn = DistinguishedName {
            common_name: "AgentGuard Root CA".to_string(),
            organization: Some("AgentGuard Inc".to_string()),
            organizational_unit: Some("Security".to_string()),
            country: Some("US".to_string()),
            email: None,
        };
        let s = dn.to_dn_string();
        assert!(s.contains("CN=AgentGuard Root CA"));
        assert!(s.contains("O=AgentGuard Inc"));
        assert!(s.contains("C=US"));
    }

    #[test]
    fn test_key_pair_generate() {
        let kp = KeyPair::generate(SignatureAlgorithm::Ed25519);
        assert_eq!(kp.public_key.len(), 32);
        assert_eq!(kp.private_key.len(), 64);
        assert_eq!(kp.fingerprint.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_key_pair_sign_verify() {
        let kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        let data = b"test data to sign";
        let sig = kp.sign(data);
        assert!(kp.verify(data, &sig));
        assert!(!kp.verify(b"different data", &sig));
    }

    #[test]
    fn test_create_root_ca() {
        let mut pki = PkiManager::new();
        let subject = DistinguishedName::new("AgentGuard Root CA");
        let (cert, _kp) = pki.create_root_ca(subject, 3650).unwrap();

        assert!(cert.is_ca);
        assert!(cert.is_self_signed());
        assert!(cert.is_valid().is_ok());
    }

    #[test]
    fn test_issue_certificate() {
        let mut pki = PkiManager::new();
        let root_dn = DistinguishedName::new("Root CA");
        let (ca_cert, _) = pki.create_root_ca(root_dn, 3650).unwrap();

        let leaf_kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        let leaf_dn = DistinguishedName {
            common_name: "agent-1.example.invalid".to_string(),
            organization: Some("Example Organization".to_string()),
            organizational_unit: None,
            country: None,
            email: Some("agent@example.invalid".to_string()),
        };

        let cert = pki
            .issue_certificate(
                &ca_cert.serial,
                leaf_dn,
                leaf_kp.public_key.clone(),
                365,
                false,
                vec![KeyUsage::DigitalSignature],
            )
            .unwrap();

        assert!(!cert.is_ca);
        assert!(cert.is_valid().is_ok());
        assert_eq!(cert.issuer.common_name, "Root CA");
    }

    #[test]
    fn test_verify_chain() {
        let mut pki = PkiManager::new();
        let root_dn = DistinguishedName::new("Root CA");
        let (ca_cert, _) = pki.create_root_ca(root_dn, 3650).unwrap();

        let leaf_kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        let leaf_dn = DistinguishedName::new("leaf.example.com");

        let cert = pki
            .issue_certificate(
                &ca_cert.serial,
                leaf_dn,
                leaf_kp.public_key.clone(),
                365,
                false,
                vec![KeyUsage::DigitalSignature],
            )
            .unwrap();

        assert!(pki.verify_chain(&cert).is_ok());
    }

    #[test]
    fn test_certificate_revocation() {
        let mut pki = PkiManager::new();
        let root_dn = DistinguishedName::new("Root CA");
        let (ca_cert, _) = pki.create_root_ca(root_dn, 3650).unwrap();

        let leaf_kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        pki.register_key_pair(leaf_kp.clone());
        let cert = pki
            .issue_certificate(
                &ca_cert.serial,
                DistinguishedName::new("revoked.example.com"),
                leaf_kp.public_key.clone(),
                365,
                false,
                vec![],
            )
            .unwrap();

        assert!(!pki.is_revoked(&cert.serial));
        pki.revoke(&cert.serial);
        assert!(pki.is_revoked(&cert.serial));
    }

    #[test]
    fn test_sign_and_verify_data() {
        let mut pki = PkiManager::new();
        let root_dn = DistinguishedName::new("Root CA");
        let (ca_cert, _) = pki.create_root_ca(root_dn, 3650).unwrap();

        let leaf_kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        pki.register_key_pair(leaf_kp.clone());
        let cert = pki
            .issue_certificate(
                &ca_cert.serial,
                DistinguishedName::new("signer.example.com"),
                leaf_kp.public_key.clone(),
                365,
                false,
                vec![KeyUsage::DigitalSignature],
            )
            .unwrap();

        let data = b"important audit record";
        let sig = pki.sign_data(&leaf_kp.fingerprint, data).unwrap();
        assert!(pki.verify_data(&cert.serial, data, &sig).unwrap());
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(SignatureAlgorithm::RsaSha256.to_string(), "RSA-SHA256");
        assert_eq!(SignatureAlgorithm::EcdsaP256.to_string(), "ECDSA-P256");
        assert_eq!(SignatureAlgorithm::Ed25519.to_string(), "Ed25519");
    }

    #[test]
    fn test_key_usage_types() {
        let usages = [
            KeyUsage::DigitalSignature,
            KeyUsage::KeyEncipherment,
            KeyUsage::KeyCertSign,
            KeyUsage::CrlSign,
            KeyUsage::DataEncipherment,
        ];
        // Just ensure they're distinct
        let unique: std::collections::HashSet<_> = usages.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn test_certificate_not_yet_valid() {
        let mut pki = PkiManager::new();
        let (mut ca_cert, _) = pki
            .create_root_ca(DistinguishedName::new("CA"), 3650)
            .unwrap();
        // Modify to be in the future
        ca_cert.not_before = Utc::now() + Duration::days(30);
        assert_eq!(
            ca_cert.is_valid().unwrap_err(),
            PkiError::CertificateNotYetValid
        );
    }

    #[test]
    fn test_certificate_expired() {
        let cert = Certificate {
            serial: "test".to_string(),
            subject: DistinguishedName::new("test"),
            issuer: DistinguishedName::new("test"),
            algorithm: SignatureAlgorithm::RsaSha256,
            public_key: vec![],
            not_before: Utc::now() - Duration::days(400),
            not_after: Utc::now() - Duration::days(1),
            signature: vec![],
            is_ca: false,
            key_usage: vec![],
            fingerprint: String::new(),
        };
        assert_eq!(cert.is_valid().unwrap_err(), PkiError::CertificateExpired);
    }

    #[test]
    fn test_sign_data_key_not_found() {
        let pki = PkiManager::new();
        assert!(matches!(
            pki.sign_data("nonexistent", b"data").unwrap_err(),
            PkiError::NotFound(_)
        ));
    }

    #[test]
    fn test_list_certificates() {
        let mut pki = PkiManager::new();
        assert!(pki.list_certificates().is_empty());

        let (ca_cert, _) = pki
            .create_root_ca(DistinguishedName::new("CA"), 3650)
            .unwrap();
        let kp = KeyPair::generate(SignatureAlgorithm::RsaSha256);
        pki.issue_certificate(
            &ca_cert.serial,
            DistinguishedName::new("leaf"),
            kp.public_key,
            365,
            false,
            vec![],
        )
        .unwrap();

        assert_eq!(pki.list_certificates().len(), 1);
    }
}
