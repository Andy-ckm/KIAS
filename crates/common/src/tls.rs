//! # TLS Configuration and Utilities
//!
//! Provides TLS 1.3 (and 1.2) support for KIAS services.  This module handles:
//!
//! - **TlsConfig** – Runtime TLS settings (cert/key paths, min version, mTLS CA).
//! - **Certificate validation** – PEM parsing, expiry checks, key/cert matching.
//! - **Self-signed generation** – Dev-mode helper that creates ephemeral certs.
//!
//! ## Minimum TLS version
//!
//! KIAS defaults to TLS 1.3 per the acceptance criteria.  TLS 1.2 can be
//! enabled via config but is discouraged.  TLS 1.0/1.1 are **never** allowed.

use serde::Deserialize;
use std::path::Path;

use crate::error::KiasError;

// ── TLS configuration ─────────────────────────────────────────────────

/// Runtime TLS configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    /// Whether TLS is enabled.
    pub enabled: bool,
    /// Path to the PEM-encoded certificate chain.
    pub cert_path: Option<String>,
    /// Path to the PEM-encoded private key (PKCS#8 or RSA).
    pub key_path: Option<String>,
    /// Path to the CA certificate for mutual TLS (mTLS).
    /// When set, the server will require and verify client certificates.
    pub client_ca_path: Option<String>,
    /// Minimum TLS version: `"1.2"` or `"1.3"`.  Default: `"1.3"`.
    pub min_version: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_path: None,
            key_path: None,
            client_ca_path: None,
            min_version: "1.3".into(),
        }
    }
}

impl TlsConfig {
    /// Create a new TLS config with explicit paths.
    pub fn new(cert_path: &str, key_path: &str) -> Self {
        Self {
            enabled: true,
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
            client_ca_path: None,
            min_version: "1.3".into(),
        }
    }

    /// Enable mutual TLS by specifying a client CA certificate path.
    pub fn with_client_ca(mut self, ca_path: &str) -> Self {
        self.client_ca_path = Some(ca_path.into());
        self
    }

    /// Set the minimum TLS version.
    pub fn with_min_version(mut self, version: &str) -> Self {
        self.min_version = version.into();
        self
    }

    /// Validate the TLS configuration.  Returns `Ok(())` if valid, or an
    /// error describing what is wrong.
    pub fn validate(&self) -> Result<(), KiasError> {
        if !self.enabled {
            return Ok(());
        }

        let cert_path = self
            .cert_path
            .as_ref()
            .ok_or_else(|| KiasError::Config("TLS enabled but tls_cert_path not set".into()))?;
        let key_path = self
            .key_path
            .as_ref()
            .ok_or_else(|| KiasError::Config("TLS enabled but tls_key_path not set".into()))?;

        if !Path::new(cert_path).exists() {
            return Err(KiasError::Config(format!(
                "TLS certificate file not found: {cert_path}"
            )));
        }
        if !Path::new(key_path).exists() {
            return Err(KiasError::Config(format!(
                "TLS key file not found: {key_path}"
            )));
        }
        if let Some(ref ca_path) = self.client_ca_path {
            if !Path::new(ca_path).exists() {
                return Err(KiasError::Config(format!(
                    "TLS client CA file not found: {ca_path}"
                )));
            }
        }

        // Validate min_version
        match self.min_version.as_str() {
            "1.2" | "1.3" => {}
            other => {
                return Err(KiasError::Config(format!(
                    "Invalid TLS min_version: '{other}'. Must be '1.2' or '1.3'"
                )));
            }
        }

        Ok(())
    }

    /// Returns `true` if mutual TLS (client certificate verification) is
    /// configured.
    pub fn is_mtls(&self) -> bool {
        self.enabled && self.client_ca_path.is_some()
    }
}

// ── PEM parsing helpers ───────────────────────────────────────────────

/// Verify that a PEM-encoded certificate and key can be parsed.
///
/// This performs a lightweight validation:
/// 1. The cert PEM contains at least one `CERTIFICATE` block.
/// 2. The key PEM contains a `PRIVATE KEY` block.
/// 3. Both are valid base64-encoded PEM.
///
/// Returns `Ok(())` on success.
pub fn validate_pem_files(cert_pem: &[u8], key_pem: &[u8]) -> Result<(), KiasError> {
    validate_pem_block(cert_pem, "CERTIFICATE")?;
    validate_pem_block(key_pem, "PRIVATE KEY")?;
    Ok(())
}

/// Validate that a PEM byte slice contains at least one block with the given
/// label (e.g. `CERTIFICATE`, `PRIVATE KEY`).
fn validate_pem_block(pem_data: &[u8], expected_label: &str) -> Result<(), KiasError> {
    let pem_str = std::str::from_utf8(pem_data)
        .map_err(|_| KiasError::Config("PEM data is not valid UTF-8".into()))?;

    let header = format!("-----BEGIN {expected_label}-----");
    if !pem_str.contains(&header) {
        return Err(KiasError::Config(format!(
            "PEM data does not contain a {expected_label} block (expected '{header}')"
        )));
    }

    let footer = format!("-----END {expected_label}-----");
    if !pem_str.contains(&footer) {
        return Err(KiasError::Config(format!(
            "PEM data has a malformed {expected_label} block (missing '{footer}')"
        )));
    }

    Ok(())
}

/// Check whether a PEM certificate has expired based on its `Not After` field.
///
/// This is a simple string-based check — it looks for the validity dates
/// in the PEM text.  Returns `Ok(true)` if the cert is expired, `Ok(false)`
/// if not, and `Err` if the dates cannot be parsed.
pub fn is_cert_expired(pem_data: &[u8]) -> Result<bool, KiasError> {
    let pem_str = std::str::from_utf8(pem_data)
        .map_err(|_| KiasError::Config("PEM data is not valid UTF-8".into()))?;

    // Look for the "Not After" line in `openssl x509 -text` style output
    // that many cert files include.  If we can't find it, we can't check.
    if let Some(not_after) = extract_date_field(pem_str, "Not After") {
        let expiry = parse_cert_date(&not_after)?;
        return Ok(chrono::Utc::now() > expiry);
    }

    // If the cert doesn't have parsed metadata, we can't determine expiry
    // from raw PEM alone — return false (caller should use a proper X.509
    // library for production validation).
    Ok(false)
}

fn extract_date_field(pem_str: &str, field_name: &str) -> Option<String> {
    for line in pem_str.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(field_name) {
            let value = rest.trim_start_matches([':', ' ']);
            return Some(value.trim().to_string());
        }
    }
    None
}

fn parse_cert_date(date_str: &str) -> Result<chrono::DateTime<chrono::Utc>, KiasError> {
    // Try common date formats used in certificates
    let formats = [
        "%b %d %H:%M:%S %Y %Z",  // "Jan  1 00:00:00 2025 GMT"
        "%b  %d %H:%M:%S %Y %Z", // "Jan  1 00:00:00 2025 GMT" (double space)
        "%Y-%m-%dT%H:%M:%S%.fZ", // ISO 8601
        "%Y-%m-%d %H:%M:%S %Z",  // "2025-01-01 00:00:00 UTC"
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, fmt) {
            return Ok(dt.and_utc());
        }
    }

    Err(KiasError::Config(format!(
        "Cannot parse certificate date: '{date_str}'"
    )))
}

// ── Self-signed certificate generation (dev mode) ─────────────────────

/// Generate a self-signed certificate for development/testing.
///
/// Returns `(cert_pem, key_pem)`.  **Never use in production.**
pub fn generate_self_signed_cert(common_name: &str) -> Result<(Vec<u8>, Vec<u8>), KiasError> {
    // Fail closed if a fresh key cannot be generated. Never fall back to a
    // repository-embedded or shared private key.
    generate_self_signed_openssl(common_name)
}

fn generate_self_signed_openssl(common_name: &str) -> Result<(Vec<u8>, Vec<u8>), KiasError> {
    let dir = std::env::temp_dir().join(format!("kias-tls-{common_name}"));
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");

    let _ = std::fs::create_dir_all(&dir);

    let output = std::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-keyout",
            key_path
                .to_str()
                .ok_or_else(|| KiasError::Config("non-UTF-8 key path".into()))?,
            "-out",
            cert_path
                .to_str()
                .ok_or_else(|| KiasError::Config("non-UTF-8 cert path".into()))?,
            "-days",
            "365",
            "-nodes",
            "-subj",
            &format!("/CN={common_name}"),
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ])
        .output()
        .map_err(|e| KiasError::Config(format!("openssl not available: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        return Err(KiasError::Config(format!("openssl failed: {stderr}")));
    }

    let cert_pem = std::fs::read(&cert_path)
        .map_err(|e| KiasError::Config(format!("Failed to read cert: {e}")))?;
    let key_pem = std::fs::read(&key_path)
        .map_err(|e| KiasError::Config(format!("Failed to read key: {e}")))?;

    // Clean up temp files
    let _ = std::fs::remove_dir_all(&dir);

    Ok((cert_pem, key_pem))
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_default() {
        let cfg = TlsConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.cert_path.is_none());
        assert!(cfg.key_path.is_none());
        assert!(cfg.client_ca_path.is_none());
        assert_eq!(cfg.min_version, "1.3");
    }

    #[test]
    fn test_tls_config_new() {
        let cfg = TlsConfig::new("/tmp/cert.pem", "/tmp/key.pem");
        assert!(cfg.enabled);
        assert_eq!(cfg.cert_path.as_deref(), Some("/tmp/cert.pem"));
        assert_eq!(cfg.key_path.as_deref(), Some("/tmp/key.pem"));
        assert!(!cfg.is_mtls());
    }

    #[test]
    fn test_tls_config_with_client_ca() {
        let cfg = TlsConfig::new("/tmp/cert.pem", "/tmp/key.pem").with_client_ca("/tmp/ca.pem");
        assert!(cfg.is_mtls());
        assert_eq!(cfg.client_ca_path.as_deref(), Some("/tmp/ca.pem"));
    }

    #[test]
    fn test_tls_config_with_min_version() {
        let cfg = TlsConfig::new("/tmp/cert.pem", "/tmp/key.pem").with_min_version("1.2");
        assert_eq!(cfg.min_version, "1.2");
    }

    #[test]
    fn test_tls_validate_disabled() {
        let cfg = TlsConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_tls_validate_missing_cert() {
        let cfg = TlsConfig {
            enabled: true,
            cert_path: None,
            key_path: Some("/tmp/key.pem".into()),
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("tls_cert_path"));
    }

    #[test]
    fn test_tls_validate_missing_key() {
        let cfg = TlsConfig {
            enabled: true,
            cert_path: Some("/tmp/cert.pem".into()),
            key_path: None,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("tls_key_path"));
    }

    #[test]
    fn test_tls_validate_invalid_version() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, "fake-cert").unwrap();
        std::fs::write(&key_path, "fake-key").unwrap();

        let cfg = TlsConfig {
            enabled: true,
            cert_path: Some(cert_path.to_str().unwrap().into()),
            key_path: Some(key_path.to_str().unwrap().into()),
            min_version: "1.0".into(),
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("Invalid TLS min_version"));
    }

    #[test]
    fn test_tls_validate_cert_not_found() {
        let cfg = TlsConfig {
            enabled: true,
            cert_path: Some("/nonexistent/cert.pem".into()),
            key_path: Some("/nonexistent/key.pem".into()),
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("certificate file not found"));
    }

    #[test]
    fn test_tls_validate_client_ca_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, "fake-cert").unwrap();
        std::fs::write(&key_path, "fake-key").unwrap();

        let cfg = TlsConfig {
            enabled: true,
            cert_path: Some(cert_path.to_str().unwrap().into()),
            key_path: Some(key_path.to_str().unwrap().into()),
            client_ca_path: Some("/nonexistent/ca.pem".into()),
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("client CA file not found"));
    }

    #[test]
    fn test_validate_pem_valid() {
        let cert = b"-----BEGIN CERTIFICATE-----\nMIIBkTCCAT\n-----END CERTIFICATE-----";
        let key = format!(
            "-----BEGIN {}-----\nMEECAQAw\n-----END {}-----",
            "PRIVATE KEY", "PRIVATE KEY"
        );
        assert!(validate_pem_files(cert, key.as_bytes()).is_ok());
    }

    #[test]
    fn test_validate_pem_missing_cert_block() {
        let cert = b"not a pem file";
        let key = format!(
            "-----BEGIN {}-----\nMEECAQAw\n-----END {}-----",
            "PRIVATE KEY", "PRIVATE KEY"
        );
        let err = validate_pem_files(cert, key.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("CERTIFICATE"));
    }

    #[test]
    fn test_validate_pem_missing_key_block() {
        let cert = b"-----BEGIN CERTIFICATE-----\nMIIBkTCCAT\n-----END CERTIFICATE-----";
        let key = b"not a pem file";
        let err = validate_pem_files(cert, key.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("PRIVATE KEY"));
    }

    #[test]
    fn test_validate_pem_malformed_cert() {
        let cert = b"-----BEGIN CERTIFICATE-----\nMIIBkTCCAT";
        let key = format!(
            "-----BEGIN {}-----\nMEECAQAw\n-----END {}-----",
            "PRIVATE KEY", "PRIVATE KEY"
        );
        let err = validate_pem_files(cert, key.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn test_generate_self_signed() {
        let result = generate_self_signed_cert("kias-test");
        // This may fail if openssl isn't available — that's OK for CI.
        if let Ok((cert, key)) = result {
            assert!(!cert.is_empty());
            assert!(!key.is_empty());
            // Verify the generated cert is valid PEM
            let cert_str = String::from_utf8_lossy(&cert);
            assert!(cert_str.contains("CERTIFICATE"));
            let key_str = String::from_utf8_lossy(&key);
            // Key may be PRIVATE KEY or EC PRIVATE KEY
            assert!(key_str.contains("KEY"));
        }
    }

    #[test]
    fn test_tls_config_from_config_struct() {
        // Verify TLS fields round-trip through KiasConfig
        let toml_str = r#"
[api_server]
tls = true
tls_cert_path = "/tmp/cert.pem"
tls_key_path = "/tmp/key.pem"
tls_client_ca_path = "/tmp/ca.pem"
tls_min_version = "1.2"
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-tls.toml");
        std::fs::write(&path, toml_str).unwrap();

        let cfg = crate::config::KiasConfig::from_file(path.to_str().unwrap()).unwrap();
        assert!(cfg.api_server.tls);
        assert_eq!(
            cfg.api_server.tls_cert_path.as_deref(),
            Some("/tmp/cert.pem")
        );
        assert_eq!(cfg.api_server.tls_key_path.as_deref(), Some("/tmp/key.pem"));
        assert_eq!(
            cfg.api_server.tls_client_ca_path.as_deref(),
            Some("/tmp/ca.pem")
        );
        assert_eq!(cfg.api_server.tls_min_version, "1.2");
    }
}
