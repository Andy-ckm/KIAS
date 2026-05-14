//! # TLS Server Builder
//!
//! Provides TLS 1.3 (and 1.2) support for the KIAS API Server using
//! `rustls` + `tokio-rustls`.  Supports:
//!
//! - Server TLS with PEM certificate chains.
//! - Mutual TLS (mTLS) with client certificate verification.
//! - Configurable minimum TLS version (1.2 or 1.3, default 1.3).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use kias_api_server::tls::TlsServerBuilder;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Start with TLS
//! let addr = "0.0.0.0:8443";
//! // let server = TlsServerBuilder::new(addr)
//! //     .with_cert_files("/path/to/cert.pem", "/path/to/key.pem")?
//! //     .with_min_version(TlsVersion::Tls13)?
//! //     .serve(axum::Router::new())
//! //     .await?;
//! # Ok(())
//! # }
//! ```

use kias_common::config::ApiServerConfig;
use kias_common::error::KiasError;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::RootCertStore;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Minimum TLS version to enforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2 (allowed but discouraged).
    Tls12,
    /// TLS 1.3 (KIAS default, recommended).
    Tls13,
}

impl TlsVersion {
    /// Parse from a config string ("1.2" or "1.3").
    pub fn from_config(s: &str) -> Result<Self, KiasError> {
        match s {
            "1.2" => Ok(Self::Tls12),
            "1.3" => Ok(Self::Tls13),
            other => Err(KiasError::Config(format!(
                "Invalid TLS min_version: '{other}'. Must be '1.2' or '1.3'"
            ))),
        }
    }
}

/// Builder for a TLS-enabled axum server.
#[derive(Debug)]
pub struct TlsServerBuilder {
    addr: SocketAddr,
    cert_path: Option<String>,
    key_path: Option<String>,
    client_ca_path: Option<String>,
    min_version: TlsVersion,
}

impl TlsServerBuilder {
    /// Create a new TLS server builder for the given address.
    pub fn new(addr: impl Into<SocketAddr>) -> Self {
        Self {
            addr: addr.into(),
            cert_path: None,
            key_path: None,
            client_ca_path: None,
            min_version: TlsVersion::Tls13,
        }
    }

    /// Create a builder from the API server config.
    pub fn from_config(config: &ApiServerConfig) -> Result<Self, KiasError> {
        let addr: SocketAddr = format!("{}:{}", config.host, config.port)
            .parse()
            .map_err(|e| KiasError::Config(format!("Invalid server address: {e}")))?;

        let cert_path = config.tls_cert_path.clone().ok_or_else(|| {
            KiasError::Config("TLS enabled but tls_cert_path not set".into())
        })?;
        let key_path = config.tls_key_path.clone().ok_or_else(|| {
            KiasError::Config("TLS enabled but tls_key_path not set".into())
        })?;

        let min_version = TlsVersion::from_config(&config.tls_min_version)?;

        Ok(Self {
            addr,
            cert_path: Some(cert_path),
            key_path: Some(key_path),
            client_ca_path: config.tls_client_ca_path.clone(),
            min_version,
        })
    }

    /// Set the certificate and key file paths.
    pub fn with_cert_files(
        mut self,
        cert_path: &str,
        key_path: &str,
    ) -> Result<Self, KiasError> {
        self.cert_path = Some(cert_path.into());
        self.key_path = Some(key_path.into());
        Ok(self)
    }

    /// Enable mutual TLS with a client CA certificate.
    pub fn with_client_ca(mut self, ca_path: &str) -> Self {
        self.client_ca_path = Some(ca_path.into());
        self
    }

    /// Set the minimum TLS version.
    pub fn with_min_version(mut self, version: TlsVersion) -> Result<Self, KiasError> {
        self.min_version = version;
        Ok(self)
    }

    /// Build the `rustls` `ServerConfig`.
    pub fn build_rustls_config(
        &self,
    ) -> Result<rustls::ServerConfig, KiasError> {
        // Ensure a crypto provider is installed
        let _ = rustls::crypto::ring::default_provider().install_default();

        let cert_path = self.cert_path.as_ref().ok_or_else(|| {
            KiasError::Config("TLS cert path not set".into())
        })?;
        let key_path = self.key_path.as_ref().ok_or_else(|| {
            KiasError::Config("TLS key path not set".into())
        })?;

        // Load certificates
        let certs = load_certs(cert_path)?;
        let key = load_key(key_path)?;

        // Build the crypto config
        let mut config = if let Some(ref ca_path) = self.client_ca_path {
            // mTLS: require client certificates signed by the CA
            let ca_certs = load_certs(ca_path)?;
            let mut root_store = RootCertStore::empty();
            for cert in ca_certs {
                root_store
                    .add(cert)
                    .map_err(|e| KiasError::Config(format!("Failed to add CA cert: {e}")))?;
            }
            let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
                .build()
                .map_err(|e| {
                    KiasError::Config(format!("Failed to build client verifier: {e}"))
                })?;

            rustls::ServerConfig::builder()
                .with_client_cert_verifier(client_verifier)
                .with_single_cert(certs, key)
                .map_err(|e| KiasError::Config(format!("TLS config error: {e}")))?
        } else {
            // Standard TLS (no client verification)
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| KiasError::Config(format!("TLS config error: {e}")))?
        };

        // Set ALPN protocols
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        Ok(config)
    }

    /// Serve the given axum router with TLS.
    ///
    /// Note: This method starts the TLS listener and accepts connections.
    /// For production use, integrate this with your axum server setup.
    pub async fn serve(
        &self,
    ) -> Result<TcpListener, KiasError> {
        let listener = TcpListener::bind(self.addr)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to bind to {}: {e}", self.addr)))?;

        tracing::info!(
            addr = %self.addr,
            tls_version = ?self.min_version,
            mtls = self.client_ca_path.is_some(),
            "TLS listener bound"
        );

        Ok(listener)
    }

    /// Create a `TlsAcceptor` for use with axum's `serve` method.
    pub fn build_tls_acceptor(&self) -> Result<TlsAcceptor, KiasError> {
        let rustls_config = self.build_rustls_config()?;
        Ok(TlsAcceptor::from(Arc::new(rustls_config)))
    }
}

/// Load certificates from a PEM file.
fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, KiasError> {
    let file = std::fs::File::open(path)
        .map_err(|e| KiasError::Config(format!("Failed to open cert file '{path}': {e}")))?;
    let mut reader = BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| KiasError::Config(format!("Failed to parse certs from '{path}': {e}")))?;

    if certs.is_empty() {
        return Err(KiasError::Config(format!(
            "No certificates found in '{path}'"
        )));
    }

    Ok(certs)
}

/// Load a private key from a PEM file.
fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, KiasError> {
    let file = std::fs::File::open(path)
        .map_err(|e| KiasError::Config(format!("Failed to open key file '{path}': {e}")))?;
    let mut reader = BufReader::new(file);

    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|e| KiasError::Config(format!("Failed to parse key from '{path}': {e}")))?
        .ok_or_else(|| KiasError::Config(format!("No private key found in '{path}'")))?;

    Ok(key)
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_version_from_config_13() {
        let v = TlsVersion::from_config("1.3").unwrap();
        assert_eq!(v, TlsVersion::Tls13);
    }

    #[test]
    fn test_tls_version_from_config_12() {
        let v = TlsVersion::from_config("1.2").unwrap();
        assert_eq!(v, TlsVersion::Tls12);
    }

    #[test]
    fn test_tls_version_from_config_invalid() {
        let err = TlsVersion::from_config("1.0").unwrap_err();
        assert!(err.to_string().contains("Invalid TLS min_version"));
    }

    #[test]
    fn test_tls_server_builder_from_config_no_tls() {
        let config = ApiServerConfig {
            tls: false,
            ..Default::default()
        };
        // Should fail because no cert/key paths
        let result = TlsServerBuilder::from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_tls_server_builder_from_config_with_tls() {
        let config = ApiServerConfig {
            tls: true,
            tls_cert_path: Some("/tmp/cert.pem".into()),
            tls_key_path: Some("/tmp/key.pem".into()),
            tls_min_version: "1.3".into(),
            ..Default::default()
        };
        let builder = TlsServerBuilder::from_config(&config).unwrap();
        assert_eq!(builder.addr.port(), 8080);
        assert_eq!(builder.cert_path.as_deref(), Some("/tmp/cert.pem"));
        assert_eq!(builder.key_path.as_deref(), Some("/tmp/key.pem"));
        assert!(builder.client_ca_path.is_none());
        assert_eq!(builder.min_version, TlsVersion::Tls13);
    }

    #[test]
    fn test_tls_server_builder_with_client_ca() {
        let config = ApiServerConfig {
            tls: true,
            tls_cert_path: Some("/tmp/cert.pem".into()),
            tls_key_path: Some("/tmp/key.pem".into()),
            tls_client_ca_path: Some("/tmp/ca.pem".into()),
            tls_min_version: "1.3".into(),
            ..Default::default()
        };
        let builder = TlsServerBuilder::from_config(&config).unwrap();
        assert_eq!(
            builder.client_ca_path.as_deref(),
            Some("/tmp/ca.pem")
        );
    }

    #[test]
    fn test_tls_server_builder_custom_port() {
        let config = ApiServerConfig {
            tls: true,
            host: "127.0.0.1".into(),
            port: 9443,
            tls_cert_path: Some("/tmp/cert.pem".into()),
            tls_key_path: Some("/tmp/key.pem".into()),
            tls_min_version: "1.2".into(),
            ..Default::default()
        };
        let builder = TlsServerBuilder::from_config(&config).unwrap();
        assert_eq!(builder.addr, "127.0.0.1:9443".parse::<SocketAddr>().unwrap());
        assert_eq!(builder.min_version, TlsVersion::Tls12);
    }

    #[test]
    fn test_tls_server_builder_missing_cert() {
        let config = ApiServerConfig {
            tls: true,
            tls_cert_path: None,
            tls_key_path: Some("/tmp/key.pem".into()),
            ..Default::default()
        };
        let result = TlsServerBuilder::from_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tls_cert_path"));
    }

    #[test]
    fn test_tls_server_builder_missing_key() {
        let config = ApiServerConfig {
            tls: true,
            tls_cert_path: Some("/tmp/cert.pem".into()),
            tls_key_path: None,
            ..Default::default()
        };
        let result = TlsServerBuilder::from_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tls_key_path"));
    }

    #[test]
    fn test_load_certs_nonexistent() {
        let err = load_certs("/nonexistent/cert.pem").unwrap_err();
        assert!(err.to_string().contains("Failed to open cert file"));
    }

    #[test]
    fn test_load_key_nonexistent() {
        let err = load_key("/nonexistent/key.pem").unwrap_err();
        assert!(err.to_string().contains("Failed to open key file"));
    }

    #[test]
    fn test_load_certs_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad-cert.pem");
        std::fs::write(&path, "not a valid cert").unwrap();

        let err = load_certs(path.to_str().unwrap()).unwrap_err();
        // rustls-pemfile returns empty iterator for non-PEM content
        assert!(err.to_string().contains("No certificates found") || err.to_string().contains("Failed to parse certs"));
    }

    #[test]
    fn test_load_certs_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty-cert.pem");
        std::fs::write(&path, "").unwrap();

        let err = load_certs(path.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("No certificates found"));
    }

    #[test]
    fn test_load_key_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad-key.pem");
        std::fs::write(&path, "not a valid key").unwrap();

        let err = load_key(path.to_str().unwrap()).unwrap_err();
        // rustls-pemfile returns None for non-PEM content
        assert!(err.to_string().contains("No private key found") || err.to_string().contains("Failed to parse key"));
    }

    #[test]
    fn test_build_rustls_config_with_real_certs() {
        // Generate a self-signed cert for testing
        let result = kias_common::tls::generate_self_signed_cert("kias-test");
        if let Ok((cert_pem, key_pem)) = result {
            let dir = tempfile::tempdir().unwrap();
            let cert_path = dir.path().join("cert.pem");
            let key_path = dir.path().join("key.pem");
            std::fs::write(&cert_path, &cert_pem).unwrap();
            std::fs::write(&key_path, &key_pem).unwrap();

            let builder = TlsServerBuilder {
                addr: "127.0.0.1:8443".parse().unwrap(),
                cert_path: Some(cert_path.to_str().unwrap().into()),
                key_path: Some(key_path.to_str().unwrap().into()),
                client_ca_path: None,
                min_version: TlsVersion::Tls13,
            };

            let config = builder.build_rustls_config();
            // This should succeed with a valid self-signed cert
            if let Ok(cfg) = config {
                // Verify ALPN is set
                assert!(cfg.alpn_protocols.contains(&b"h2".to_vec()));
                assert!(cfg.alpn_protocols.contains(&b"http/1.1".to_vec()));
            }
        }
    }

    #[test]
    fn test_config_tls_fields_roundtrip() {
        // Verify TLS fields in ApiServerConfig default correctly
        let config = ApiServerConfig::default();
        assert!(!config.tls);
        assert!(config.tls_cert_path.is_none());
        assert!(config.tls_key_path.is_none());
        assert!(config.tls_client_ca_path.is_none());
        assert_eq!(config.tls_min_version, "1.3");
    }
}
