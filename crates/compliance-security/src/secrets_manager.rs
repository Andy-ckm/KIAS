//! Secrets Zero明文策略
//!
//! Implements runtime secret injection, secret references, and automatic rotation:
//! - SecretProvider: runtime secret injection
//! - SecretRef: reference instead of plaintext
//! - SecretRotation: automatic rotation interface
//! - Detection of plaintext secrets in code

use crate::error::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secret value type
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub enum SecretValue {
    Text(String),
    Binary(Vec<u8>),
    Opaque(String),
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Text(_) => "Text",
            Self::Binary(_) => "Binary",
            Self::Opaque(_) => "Opaque",
        };
        f.debug_tuple("SecretValue").field(&kind).field(&"[REDACTED]").finish()
    }
}

/// Secret reference (not the actual secret)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    pub name: String,
    pub version: u32,
    pub hint: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Secret metadata (without the actual value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub name: String,
    pub version: u32,
    pub hint: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub rotated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rotation_days: Option<u32>,
}

impl SecretMetadata {
    pub fn new(name: &str, hint: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            name: name.to_string(),
            version: 1,
            hint: hint.to_string(),
            created_at: now,
            rotated_at: now,
            expires_at: None,
            rotation_days: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|e| chrono::Utc::now() > e)
            .unwrap_or(false)
    }

    pub fn should_rotate(&self) -> bool {
        if let Some(days) = self.rotation_days {
            let age = chrono::Utc::now() - self.rotated_at;
            age.num_days() >= days as i64
        } else {
            false
        }
    }
}

/// Secret rotation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationStatus {
    pub secret_name: String,
    pub from_version: u32,
    pub to_version: u32,
    pub rotated_at: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub error: Option<String>,
}

/// Secrets provider for runtime injection
pub struct SecretProvider {
    secrets: RwLock<HashMap<String, SecretValue>>,
    refs: RwLock<HashMap<String, SecretMetadata>>,
    audit_log: RwLock<Vec<SecretAccessRecord>>,
    #[allow(dead_code)]
    hash_function: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SecretAccessRecord {
    pub secret_name: String,
    pub accessor: String,
    pub access_time: chrono::DateTime<chrono::Utc>,
    pub success: bool,
}

impl std::fmt::Debug for SecretAccessRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretAccessRecord")
            .field("secret_name", &"[REDACTED]")
            .field("accessor", &"[PSEUDONYMOUS]")
            .field("access_time", &self.access_time)
            .field("success", &self.success)
            .finish()
    }
}

fn pseudonymize_accessor(accessor: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(accessor.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("accessor:{}", &digest[..16])
}

impl Default for SecretProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretProvider {
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            refs: RwLock::new(HashMap::new()),
            audit_log: RwLock::new(Vec::new()),
            hash_function: "SHA-256".to_string(),
        }
    }

    pub fn store(&self, name: &str, value: SecretValue) -> KiasResult<SecretRef> {
        let now = chrono::Utc::now();

        let mut secrets = self
            .secrets
            .write()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        let mut refs = self
            .refs
            .write()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;

        let version = refs.get(name).map(|r| r.version + 1).unwrap_or(1);

        secrets.insert(name.to_string(), value);

        let metadata = SecretMetadata {
            name: name.to_string(),
            version,
            hint: "Stored via SecretProvider".to_string(),
            created_at: now,
            rotated_at: now,
            expires_at: None,
            rotation_days: None,
        };
        refs.insert(name.to_string(), metadata.clone());

        Ok(SecretRef {
            name: name.to_string(),
            version,
            hint: metadata.hint.clone(),
            created_at: now,
        })
    }

    pub fn get(&self, name: &str, accessor: &str) -> KiasResult<SecretValue> {
        let now = chrono::Utc::now();

        let secrets = self
            .secrets
            .read()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        let mut audit = self
            .audit_log
            .write()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;

        match secrets.get(name) {
            Some(value) => {
                audit.push(SecretAccessRecord {
                    secret_name: name.to_string(),
                    accessor: pseudonymize_accessor(accessor),
                    access_time: now,
                    success: true,
                });
                Ok(value.clone())
            }
            None => {
                audit.push(SecretAccessRecord {
                    secret_name: name.to_string(),
                    accessor: pseudonymize_accessor(accessor),
                    access_time: now,
                    success: false,
                });
                Err(KiasError::Secrets("Secret not found".to_string()))
            }
        }
    }

    pub fn get_ref(&self, name: &str) -> KiasResult<SecretRef> {
        let refs = self
            .refs
            .read()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        refs.get(name)
            .map(|m| SecretRef {
                name: m.name.clone(),
                version: m.version,
                hint: m.hint.clone(),
                created_at: m.created_at,
            })
            .ok_or_else(|| KiasError::Secrets("Secret reference not found".to_string()))
    }

    pub fn rotate(&self, name: &str, new_value: SecretValue) -> KiasResult<RotationStatus> {
        let mut secrets = self
            .secrets
            .write()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        let mut refs = self
            .refs
            .write()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;

        let now = chrono::Utc::now();

        if let Some(metadata) = refs.get_mut(name) {
            let from_version = metadata.version;
            metadata.version += 1;
            metadata.rotated_at = now;

            secrets.insert(name.to_string(), new_value);

            Ok(RotationStatus {
                secret_name: name.to_string(),
                from_version,
                to_version: metadata.version,
                rotated_at: now,
                success: true,
                error: None,
            })
        } else {
            Err(KiasError::Secrets("Secret not found".to_string()))
        }
    }

    pub fn list_refs(&self) -> KiasResult<Vec<SecretMetadata>> {
        let refs = self
            .refs
            .read()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        Ok(refs.values().cloned().collect())
    }

    pub fn get_audit_log(&self) -> KiasResult<Vec<SecretAccessRecord>> {
        let audit = self
            .audit_log
            .read()
            .map_err(|_| KiasError::Secrets("Lock poisoned".to_string()))?;
        Ok(audit.clone())
    }

    pub fn hash_secret(&self, value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Plaintext secret patterns for detection
pub struct SecretPatternDetector {
    patterns: Vec<(String, regex::Regex)>,
}

impl Default for SecretPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretPatternDetector {
    pub fn new() -> Self {
        // These regex patterns are hardcoded and known to be valid
        let patterns = vec![
            (
                "AWS Access Key".to_string(),
                regex::Regex::new(r"AKIA[0-9A-Z]{16}").expect("invalid AWS access key pattern"),
            ),
            (
                "GitHub Token".to_string(),
                regex::Regex::new(r"ghp_[0-9a-zA-Z]{36}").expect("invalid GitHub token pattern"),
            ),
            (
                "Generic API Key".to_string(),
                regex::Regex::new(r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['"]?[0-9a-zA-Z_-]{20,}"#)
                    .expect("invalid generic API key pattern"),
            ),
            (
                "Generic Secret".to_string(),
                regex::Regex::new(r#"(?i)(secret|password|passwd|pwd)\s*[:=]\s*['"]?[^\s'"]{8,}"#)
                    .expect("invalid generic secret pattern"),
            ),
            (
                "Private Key Header".to_string(),
                regex::Regex::new(r"-----BEGIN (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----")
                    .expect("invalid private key pattern"),
            ),
            (
                "JWT Token".to_string(),
                regex::Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*")
                    .expect("invalid JWT pattern"),
            ),
            (
                "Slack Token".to_string(),
                regex::Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*")
                    .expect("invalid Slack token pattern"),
            ),
            (
                "Stripe Key".to_string(),
                regex::Regex::new(r"sk_live_[0-9a-zA-Z]{24,}").expect("invalid Stripe key pattern"),
            ),
        ];
        Self { patterns }
    }

    pub fn detect(&self, text: &str) -> Vec<DetectedSecret> {
        let mut findings = Vec::new();
        for (pattern_name, re) in &self.patterns {
            for mat in re.find_iter(text) {
                findings.push(DetectedSecret {
                    secret_type: pattern_name.clone(),
                    masked_text: mask_detected_secret(mat.as_str()),
                    fingerprint: fingerprint_detected_secret(mat.as_str()),
                    start: mat.start(),
                    end: mat.end(),
                    line_number: text[..mat.start()].chars().filter(|c| *c == '\n').count() + 1,
                });
            }
        }
        findings
    }

    pub fn contains_secrets(&self, text: &str) -> bool {
        !self.detect(text).is_empty()
    }
}

fn mask_detected_secret(value: &str) -> String {
    if value.len() <= 6 {
        return "*".repeat(value.len());
    }
    format!("{}…{}", &value[..3], &value[value.len() - 3..])
}

fn fingerprint_detected_secret(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest[..16].to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedSecret {
    pub secret_type: String,
    pub masked_text: String,
    pub fingerprint: String,
    pub start: usize,
    pub end: usize,
    pub line_number: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let provider = SecretProvider::new();
        let secret = SecretValue::Text("my-secret-key".to_string());
        let reference = provider.store("test-key", secret.clone()).unwrap();

        assert_eq!(reference.name, "test-key");
        assert_eq!(reference.version, 1);

        let retrieved = provider.get("test-key", "test_accessor").unwrap();
        assert_eq!(retrieved, secret);
    }

    #[test]
    fn test_get_nonexistent() {
        let provider = SecretProvider::new();
        let result = provider.get("nonexistent", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_ref() {
        let provider = SecretProvider::new();
        provider
            .store("my-key", SecretValue::Text("value".to_string()))
            .unwrap();

        let reference = provider.get_ref("my-key").unwrap();
        assert_eq!(reference.name, "my-key");
    }

    #[test]
    fn test_rotate() {
        let provider = SecretProvider::new();
        provider
            .store("key", SecretValue::Text("v1".to_string()))
            .unwrap();

        let status = provider
            .rotate("key", SecretValue::Text("v2".to_string()))
            .unwrap();
        assert!(status.success);
        assert_eq!(status.from_version, 1);
        assert_eq!(status.to_version, 2);

        let retrieved = provider.get("key", "test").unwrap();
        assert_eq!(retrieved, SecretValue::Text("v2".to_string()));
    }

    #[test]
    fn test_audit_log() {
        let provider = SecretProvider::new();
        provider
            .store("key", SecretValue::Text("v".to_string()))
            .unwrap();
        provider.get("key", "user1").unwrap();
        provider.get("key", "user2").unwrap();
        let _ = provider.get("nonexistent", "user1"); // Expected to fail, ignore result

        let log = provider.get_audit_log().unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].accessor, "user1");
        assert!(log[0].success);
        assert!(!log[2].success);
    }

    #[test]
    fn test_list_refs() {
        let provider = SecretProvider::new();
        provider
            .store("key1", SecretValue::Text("v1".to_string()))
            .unwrap();
        provider
            .store("key2", SecretValue::Text("v2".to_string()))
            .unwrap();

        let refs = provider.list_refs().unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_hash_secret() {
        let provider = SecretProvider::new();
        let hash = provider.hash_secret("my-secret");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_detect_aws_key() {
        let detector = SecretPatternDetector::new();
        let text = "AWS_ACCESS_KEY=AKIAIOSFODNN7EXAMPLE";
        let findings = detector.detect(text);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].secret_type, "AWS Access Key");
    }

    #[test]
    fn test_detect_github_token() {
        let detector = SecretPatternDetector::new();
        let text = "GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let findings = detector.detect(text);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_multiple_secrets() {
        let detector = SecretPatternDetector::new();
        let text = "api_key=abc123secretkeyXXXXX\npassword=mysecretpassXXXX";
        let findings = detector.detect(text);
        assert!(findings.len() >= 2);
    }

    #[test]
    fn test_detect_no_secrets() {
        let detector = SecretPatternDetector::new();
        let text = "This is a normal text without any secrets";
        let findings = detector.detect(text);
        assert!(findings.is_empty());
        assert!(!detector.contains_secrets(text));
    }

    #[test]
    fn test_detect_jwt() {
        let detector = SecretPatternDetector::new();
        let text = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let findings = detector.detect(text);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_metadata_expired() {
        let mut metadata = SecretMetadata::new("test", "hint");
        metadata.expires_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        assert!(metadata.is_expired());

        metadata.expires_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        assert!(!metadata.is_expired());
    }

    #[test]
    fn test_metadata_should_rotate() {
        let mut metadata = SecretMetadata::new("test", "hint");
        metadata.rotation_days = Some(30);

        assert!(!metadata.should_rotate());

        metadata.rotated_at = chrono::Utc::now() - chrono::Duration::days(31);
        assert!(metadata.should_rotate());
    }
}
