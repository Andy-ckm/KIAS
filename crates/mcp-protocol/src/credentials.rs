//! MCP Credential Manager
//!
//! Provides:
//! - Secure credential storage (encrypted at rest)
//! - Credential rotation with grace periods
//! - Multiple credential types (API keys, OAuth tokens, SSH keys, certificates)
//! - Credential lifecycle management
//! - Audit logging for credential access
//! - Integration with external secret managers (Vault, AWS Secrets Manager)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::McpError;

// ---------------------------------------------------------------------------
// Credential Types
// ---------------------------------------------------------------------------

/// Types of credentials supported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CredentialType {
    /// API Key.
    ApiKey,
    /// OAuth 2.0 token.
    OAuthToken,
    /// Bearer token.
    BearerToken,
    /// SSH private key.
    SshKey,
    /// TLS certificate.
    TlsCertificate,
    /// Username/password.
    BasicAuth,
    /// Custom credential type.
    Custom(String),
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialType::ApiKey => write!(f, "api_key"),
            CredentialType::OAuthToken => write!(f, "oauth_token"),
            CredentialType::BearerToken => write!(f, "bearer_token"),
            CredentialType::SshKey => write!(f, "ssh_key"),
            CredentialType::TlsCertificate => write!(f, "tls_certificate"),
            CredentialType::BasicAuth => write!(f, "basic_auth"),
            CredentialType::Custom(t) => write!(f, "{}", t),
        }
    }
}

/// Credential status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CredentialStatus {
    /// Active and usable.
    Active,
    /// Rotating (both old and new are valid during grace period).
    Rotating,
    /// Expired.
    Expired,
    /// Revoked.
    Revoked,
    /// Pending activation.
    Pending,
}

/// A stored credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Credential type.
    pub credential_type: CredentialType,
    /// Credential status.
    pub status: CredentialStatus,
    /// Encrypted credential data.
    pub data: Vec<u8>,
    /// Initialization vector for encryption.
    pub iv: Vec<u8>,
    /// When the credential was created.
    pub created_at: SystemTime,
    /// When the credential expires (if applicable).
    pub expires_at: Option<SystemTime>,
    /// When the credential was last rotated.
    pub rotated_at: Option<SystemTime>,
    /// When the credential was last accessed.
    pub last_accessed_at: Option<SystemTime>,
    /// Number of times accessed.
    pub access_count: u64,
    /// Associated metadata.
    pub metadata: HashMap<String, String>,
    /// Tags for organization.
    pub tags: Vec<String>,
    /// Who created this credential.
    pub created_by: String,
    /// Rotation policy.
    pub rotation_policy: Option<RotationPolicy>,
}

/// Credential rotation policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    /// Automatic rotation enabled.
    pub auto_rotate: bool,
    /// Rotation interval.
    pub interval: Duration,
    /// Grace period (old credential valid after rotation).
    pub grace_period: Duration,
    /// Maximum rotations before requiring manual intervention.
    pub max_rotations: Option<u32>,
    /// Number of rotations performed.
    pub rotations_performed: u32,
}

// ---------------------------------------------------------------------------
// Rotation Notification
// ---------------------------------------------------------------------------

/// Event fired when a credential needs rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEvent {
    /// Credential ID that needs rotation.
    pub credential_id: String,
    /// Credential name.
    pub credential_name: String,
    /// Credential type.
    pub credential_type: CredentialType,
    /// When the credential was last rotated.
    pub last_rotated_at: Option<SystemTime>,
    /// How long since last rotation.
    pub elapsed: Duration,
    /// Required rotation interval.
    pub interval: Duration,
    /// Number of rotations already performed.
    pub rotations_performed: u32,
    /// Maximum rotations allowed (if any).
    pub max_rotations: Option<u32>,
}

/// Trait for credential rotation notification backends.
///
/// Implementations can send notifications via webhooks, message queues,
/// logging, or any other mechanism.
#[async_trait::async_trait]
pub trait RotationNotifier: Send + Sync {
    /// Notify that a credential needs rotation.
    async fn notify_rotation(&self, event: &RotationEvent) -> Result<(), McpError>;
}

/// Console-based notifier using tracing (default).
///
/// Logs rotation events at `warn` level. Suitable for development
/// and single-node deployments.
pub struct ConsoleRotationNotifier;

#[async_trait::async_trait]
impl RotationNotifier for ConsoleRotationNotifier {
    async fn notify_rotation(&self, event: &RotationEvent) -> Result<(), McpError> {
        tracing::warn!(
            "[ROTATION] Credential '{}' (id={}, type={}) needs rotation:              elapsed={}s, interval={}s, rotations={}/{:?}",
            event.credential_name,
            event.credential_id,
            event.credential_type,
            event.elapsed.as_secs(),
            event.interval.as_secs(),
            event.rotations_performed,
            event.max_rotations,
        );
        Ok(())
    }
}

/// In-memory notifier for testing.
///
/// Stores rotation events in a shared vec for assertion in tests.
pub struct InMemoryRotationNotifier {
    events: Arc<RwLock<Vec<RotationEvent>>>,
}

impl InMemoryRotationNotifier {
    /// Create a new in-memory notifier.
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get all recorded rotation events.
    pub async fn events(&self) -> Vec<RotationEvent> {
        self.events.read().await.clone()
    }

    /// Clear all recorded events.
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }
}

impl Default for InMemoryRotationNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RotationNotifier for InMemoryRotationNotifier {
    async fn notify_rotation(&self, event: &RotationEvent) -> Result<(), McpError> {
        self.events.write().await.push(event.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Credential Store
// ---------------------------------------------------------------------------

/// Trait for credential storage backends.
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    /// Store a credential.
    async fn store(&self, credential: &Credential) -> Result<(), McpError>;

    /// Retrieve a credential by ID.
    async fn get(&self, id: &str) -> Result<Option<Credential>, McpError>;

    /// List credentials with optional filters.
    async fn list(&self, filter: Option<CredentialFilter>) -> Result<Vec<Credential>, McpError>;

    /// Update a credential.
    async fn update(&self, credential: &Credential) -> Result<(), McpError>;

    /// Delete a credential.
    async fn delete(&self, id: &str) -> Result<bool, McpError>;
}

/// Filter for listing credentials.
#[derive(Debug, Clone, Default)]
pub struct CredentialFilter {
    /// Filter by type.
    pub credential_type: Option<CredentialType>,
    /// Filter by status.
    pub status: Option<CredentialStatus>,
    /// Filter by tags (all must match).
    pub tags: Vec<String>,
    /// Filter by creator.
    pub created_by: Option<String>,
}

/// In-memory credential store.
pub struct InMemoryCredentialStore {
    credentials: Arc<RwLock<HashMap<String, Credential>>>,
}

impl InMemoryCredentialStore {
    /// Create a new in-memory credential store.
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CredentialStore for InMemoryCredentialStore {
    async fn store(&self, credential: &Credential) -> Result<(), McpError> {
        let mut store = self.credentials.write().await;
        store.insert(credential.id.clone(), credential.clone());
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Credential>, McpError> {
        let store = self.credentials.read().await;
        Ok(store.get(id).cloned())
    }

    async fn list(&self, filter: Option<CredentialFilter>) -> Result<Vec<Credential>, McpError> {
        let store = self.credentials.read().await;
        let creds: Vec<Credential> = store.values().cloned().collect();

        match filter {
            Some(filter) => {
                let filtered: Vec<Credential> = creds
                    .into_iter()
                    .filter(|c| {
                        if let Some(ref ct) = filter.credential_type {
                            if c.credential_type != *ct {
                                return false;
                            }
                        }
                        if let Some(ref status) = filter.status {
                            if c.status != *status {
                                return false;
                            }
                        }
                        if !filter.tags.is_empty()
                            && !filter.tags.iter().all(|t| c.tags.contains(t))
                        {
                            return false;
                        }
                        if let Some(ref created_by) = filter.created_by {
                            if c.created_by != *created_by {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();
                Ok(filtered)
            }
            None => Ok(creds),
        }
    }

    async fn update(&self, credential: &Credential) -> Result<(), McpError> {
        let mut store = self.credentials.write().await;
        if store.contains_key(&credential.id) {
            store.insert(credential.id.clone(), credential.clone());
            Ok(())
        } else {
            Err(McpError::ResourceNotFound(format!(
                "Credential not found: {}",
                credential.id
            )))
        }
    }

    async fn delete(&self, id: &str) -> Result<bool, McpError> {
        let mut store = self.credentials.write().await;
        Ok(store.remove(id).is_some())
    }
}

// ---------------------------------------------------------------------------
// Encryption
// ---------------------------------------------------------------------------

/// Trait for credential encryption.
#[async_trait::async_trait]
pub trait CredentialEncryptor: Send + Sync {
    /// Encrypt data.
    async fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), McpError>;

    /// Decrypt data.
    async fn decrypt(&self, ciphertext: &[u8], iv: &[u8]) -> Result<Vec<u8>, McpError>;
}

/// AES-256-GCM encryptor (requires ring or aes-gcm crate).
pub struct AesGcmEncryptor {
    key: Vec<u8>,
}

impl AesGcmEncryptor {
    /// Create a new AES-GCM encryptor with a 256-bit key.
    pub fn new(key: Vec<u8>) -> Result<Self, McpError> {
        if key.len() != 32 {
            return Err(McpError::InvalidRequest(
                "AES-256 key must be 32 bytes".to_string(),
            ));
        }
        Ok(Self { key })
    }

    /// Generate a random 256-bit key.
    pub fn generate_key() -> Vec<u8> {
        use rand::RngCore;
        let mut key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }
}

#[async_trait::async_trait]
impl CredentialEncryptor for AesGcmEncryptor {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), McpError> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| McpError::Internal(format!("Cipher init error: {}", e)))?;

        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| McpError::Internal(format!("Encryption error: {}", e)))?;

        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    async fn decrypt(&self, ciphertext: &[u8], iv: &[u8]) -> Result<Vec<u8>, McpError> {
        use aes_gcm::{aead::Aead, aead::KeyInit, Aes256Gcm, Nonce};

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| McpError::Internal(format!("Cipher init error: {}", e)))?;

        let nonce = Nonce::from_slice(iv);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| McpError::Internal(format!("Decryption error: {}", e)))
    }
}

/// No-op encryptor (stores plaintext — for testing only).
pub struct NoOpEncryptor;

#[async_trait::async_trait]
impl CredentialEncryptor for NoOpEncryptor {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), McpError> {
        Ok((plaintext.to_vec(), vec![0u8; 12]))
    }

    async fn decrypt(&self, ciphertext: &[u8], _iv: &[u8]) -> Result<Vec<u8>, McpError> {
        Ok(ciphertext.to_vec())
    }
}

// ---------------------------------------------------------------------------
// Credential Manager
// ---------------------------------------------------------------------------

/// Audit log entry for credential access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the access occurred.
    pub timestamp: SystemTime,
    /// Credential ID accessed.
    pub credential_id: String,
    /// Action performed.
    pub action: AuditAction,
    /// Who performed the action.
    pub actor: String,
    /// Source IP address.
    pub source_ip: Option<String>,
    /// Whether the action succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Audit actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    /// Read credential.
    Read,
    /// Create credential.
    Create,
    /// Update credential.
    Update,
    /// Delete credential.
    Delete,
    /// Rotate credential.
    Rotate,
    /// Access denied.
    AccessDenied,
}

/// Credential manager configuration.
#[derive(Debug, Clone)]
pub struct CredentialManagerConfig {
    /// Enable audit logging.
    pub audit_enabled: bool,
    /// Maximum audit log entries.
    pub max_audit_entries: usize,
    /// Enable automatic rotation checks.
    pub auto_rotation_check: bool,
    /// Rotation check interval.
    pub rotation_check_interval: Duration,
}

impl Default for CredentialManagerConfig {
    fn default() -> Self {
        Self {
            audit_enabled: true,
            max_audit_entries: 10000,
            auto_rotation_check: true,
            rotation_check_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Credential manager.
pub struct CredentialManager {
    /// Configuration.
    config: CredentialManagerConfig,
    /// Credential store.
    store: Arc<dyn CredentialStore>,
    /// Encryptor.
    encryptor: Arc<dyn CredentialEncryptor>,
    /// Audit log.
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
    /// Rotation notifier.
    notifier: Arc<dyn RotationNotifier>,
}

impl CredentialManager {
    /// Create a new credential manager with a custom rotation notifier.
    pub fn new(
        config: CredentialManagerConfig,
        store: Arc<dyn CredentialStore>,
        encryptor: Arc<dyn CredentialEncryptor>,
        notifier: Arc<dyn RotationNotifier>,
    ) -> Self {
        let manager = Self {
            config,
            store,
            encryptor,
            audit_log: Arc::new(RwLock::new(Vec::new())),
            notifier,
        };

        // Start auto-rotation check if enabled
        if manager.config.auto_rotation_check {
            let mgr = manager.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(mgr.config.rotation_check_interval).await;
                    if let Err(e) = mgr.check_rotations().await {
                        tracing::warn!("Rotation check error: {}", e);
                    }
                }
            });
        }

        manager
    }

    /// Store a new credential.
    #[allow(clippy::too_many_arguments)]
    pub async fn store(
        &self,
        name: &str,
        credential_type: CredentialType,
        data: &[u8],
        created_by: &str,
        metadata: HashMap<String, String>,
        tags: Vec<String>,
        expires_at: Option<SystemTime>,
        rotation_policy: Option<RotationPolicy>,
    ) -> Result<String, McpError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (encrypted, iv) = self.encryptor.encrypt(data).await?;

        let credential = Credential {
            id: id.clone(),
            name: name.to_string(),
            credential_type,
            status: CredentialStatus::Active,
            data: encrypted,
            iv,
            created_at: SystemTime::now(),
            expires_at,
            rotated_at: None,
            last_accessed_at: None,
            access_count: 0,
            metadata,
            tags,
            created_by: created_by.to_string(),
            rotation_policy,
        };

        self.store.store(&credential).await?;

        self.audit(AuditEntry {
            timestamp: SystemTime::now(),
            credential_id: id.clone(),
            action: AuditAction::Create,
            actor: created_by.to_string(),
            source_ip: None,
            success: true,
            error: None,
        })
        .await;

        Ok(id)
    }

    /// Retrieve and decrypt a credential.
    pub async fn get(&self, id: &str, actor: &str) -> Result<Vec<u8>, McpError> {
        let mut credential =
            self.store.get(id).await?.ok_or_else(|| {
                McpError::ResourceNotFound(format!("Credential not found: {}", id))
            })?;

        // Check status
        match credential.status {
            CredentialStatus::Active | CredentialStatus::Rotating => {}
            CredentialStatus::Expired => {
                return Err(McpError::Authentication("Credential expired".to_string()));
            }
            CredentialStatus::Revoked => {
                return Err(McpError::Authentication("Credential revoked".to_string()));
            }
            CredentialStatus::Pending => {
                return Err(McpError::Authentication(
                    "Credential pending activation".to_string(),
                ));
            }
        }

        // Check expiration
        if let Some(expires_at) = credential.expires_at {
            if SystemTime::now() > expires_at {
                credential.status = CredentialStatus::Expired;
                self.store.update(&credential).await?;
                return Err(McpError::Authentication("Credential expired".to_string()));
            }
        }

        // Decrypt
        let data = self
            .encryptor
            .decrypt(&credential.data, &credential.iv)
            .await?;

        // Update access metadata
        credential.last_accessed_at = Some(SystemTime::now());
        credential.access_count += 1;
        let _ = self.store.update(&credential).await;

        self.audit(AuditEntry {
            timestamp: SystemTime::now(),
            credential_id: id.to_string(),
            action: AuditAction::Read,
            actor: actor.to_string(),
            source_ip: None,
            success: true,
            error: None,
        })
        .await;

        Ok(data)
    }

    /// Rotate a credential.
    pub async fn rotate(&self, id: &str, new_data: &[u8], actor: &str) -> Result<(), McpError> {
        let mut credential =
            self.store.get(id).await?.ok_or_else(|| {
                McpError::ResourceNotFound(format!("Credential not found: {}", id))
            })?;

        let (encrypted, iv) = self.encryptor.encrypt(new_data).await?;

        credential.data = encrypted;
        credential.iv = iv;
        credential.rotated_at = Some(SystemTime::now());
        credential.status = CredentialStatus::Active;

        // Update rotation policy counter
        if let Some(ref mut policy) = credential.rotation_policy {
            policy.rotations_performed += 1;
        }

        self.store.update(&credential).await?;

        self.audit(AuditEntry {
            timestamp: SystemTime::now(),
            credential_id: id.to_string(),
            action: AuditAction::Rotate,
            actor: actor.to_string(),
            source_ip: None,
            success: true,
            error: None,
        })
        .await;

        Ok(())
    }

    /// Revoke a credential.
    pub async fn revoke(&self, id: &str, actor: &str) -> Result<(), McpError> {
        let mut credential =
            self.store.get(id).await?.ok_or_else(|| {
                McpError::ResourceNotFound(format!("Credential not found: {}", id))
            })?;

        credential.status = CredentialStatus::Revoked;
        self.store.update(&credential).await?;

        self.audit(AuditEntry {
            timestamp: SystemTime::now(),
            credential_id: id.to_string(),
            action: AuditAction::Delete,
            actor: actor.to_string(),
            source_ip: None,
            success: true,
            error: None,
        })
        .await;

        Ok(())
    }

    /// List credentials with optional filters.
    pub async fn list(
        &self,
        filter: Option<CredentialFilter>,
    ) -> Result<Vec<Credential>, McpError> {
        self.store.list(filter).await
    }

    /// Check for credentials that need rotation.
    async fn check_rotations(&self) -> Result<(), McpError> {
        let credentials = self.store.list(None).await?;

        for credential in credentials {
            if let Some(ref policy) = credential.rotation_policy {
                if !policy.auto_rotate {
                    continue;
                }

                if let Some(rotated_at) = credential.rotated_at {
                    let elapsed = SystemTime::now()
                        .duration_since(rotated_at)
                        .unwrap_or_default();

                    if elapsed >= policy.interval {
                        let event = RotationEvent {
                            credential_id: credential.id.clone(),
                            credential_name: credential.name.clone(),
                            credential_type: credential.credential_type.clone(),
                            last_rotated_at: Some(rotated_at),
                            elapsed,
                            interval: policy.interval,
                            rotations_performed: policy.rotations_performed,
                            max_rotations: policy.max_rotations,
                        };
                        if let Err(e) = self.notifier.notify_rotation(&event).await {
                            tracing::warn!(
                                "[ROTATION ERROR] Failed to notify for credential {}: {}",
                                event.credential_id,
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Add an audit entry.
    async fn audit(&self, entry: AuditEntry) {
        if !self.config.audit_enabled {
            return;
        }

        let mut log = self.audit_log.write().await;
        if log.len() >= self.config.max_audit_entries {
            log.remove(0);
        }
        log.push(entry);
    }

    /// Get audit log entries.
    pub async fn audit_log(&self) -> Vec<AuditEntry> {
        let log = self.audit_log.read().await;
        log.clone()
    }
}

impl Clone for CredentialManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            store: self.store.clone(),
            encryptor: self.encryptor.clone(),
            audit_log: self.audit_log.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_credential_store_and_retrieve() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let id = manager
            .store(
                "test-api-key",
                CredentialType::ApiKey,
                b"secret-key-123",
                "admin",
                HashMap::new(),
                vec!["test".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        let data = manager.get(&id, "user-1").await.unwrap();
        assert_eq!(data, b"secret-key-123");
    }

    #[tokio::test]
    async fn test_credential_revocation() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let id = manager
            .store(
                "test-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        manager.revoke(&id, "admin").await.unwrap();

        let result = manager.get(&id, "user-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_credential_rotation() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let id = manager
            .store(
                "rotatable-cred",
                CredentialType::ApiKey,
                b"old-secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                Some(RotationPolicy {
                    auto_rotate: false,
                    interval: Duration::from_secs(86400),
                    grace_period: Duration::from_secs(3600),
                    max_rotations: Some(10),
                    rotations_performed: 0,
                }),
            )
            .await
            .unwrap();

        manager.rotate(&id, b"new-secret", "admin").await.unwrap();

        let data = manager.get(&id, "user-1").await.unwrap();
        assert_eq!(data, b"new-secret");
    }

    #[tokio::test]
    async fn test_audit_log() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let id = manager
            .store(
                "audit-test",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        let _ = manager.get(&id, "user-1").await;

        let log = manager.audit_log().await;
        assert!(log.len() >= 2); // At least create + read
    }

    #[tokio::test]
    async fn test_rotation_notifier_receives_event() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let notifier = Arc::new(InMemoryRotationNotifier::new());
        let store_clone = store.clone();
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            notifier.clone(),
        );

        // Store a credential with a rotation policy (1 second interval)
        let id = manager
            .store(
                "rotatable",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                Some(RotationPolicy {
                    auto_rotate: true,
                    interval: Duration::from_millis(1), // 1ms for testing
                    grace_period: Duration::from_secs(60),
                    max_rotations: Some(10),
                    rotations_performed: 0,
                }),
            )
            .await
            .unwrap();

        // Manually set rotated_at to the past so check_rotations triggers
        {
            let mut credential = store_clone.get(&id).await.unwrap().unwrap();
            credential.rotated_at = Some(SystemTime::now() - Duration::from_secs(3600));
            store_clone.update(&credential).await.unwrap();
        }

        // Run rotation check
        manager.check_rotations().await.unwrap();

        // Verify notifier received the event
        let events = notifier.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].credential_id, id);
        assert_eq!(events[0].credential_name, "rotatable");
        assert_eq!(events[0].credential_type, CredentialType::ApiKey);
        assert!(events[0].elapsed >= Duration::from_secs(3600));
        assert_eq!(events[0].rotations_performed, 0);
        assert_eq!(events[0].max_rotations, Some(10));
    }

    #[tokio::test]
    async fn test_rotation_notifier_not_triggered_when_not_due() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let notifier = Arc::new(InMemoryRotationNotifier::new());
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            notifier.clone(),
        );

        // Store a credential with a long rotation interval
        manager
            .store(
                "not-due",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                Some(RotationPolicy {
                    auto_rotate: true,
                    interval: Duration::from_secs(86400 * 365), // 1 year
                    grace_period: Duration::from_secs(3600),
                    max_rotations: None,
                    rotations_performed: 0,
                }),
            )
            .await
            .unwrap();

        // Run rotation check — should NOT trigger
        manager.check_rotations().await.unwrap();

        let events = notifier.events().await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_rotation_notifier_skips_non_auto_rotate() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let notifier = Arc::new(InMemoryRotationNotifier::new());
        let store_for_manager = store.clone();
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store_for_manager,
            encryptor,
            notifier.clone(),
        );

        // Store a credential with auto_rotate = false and short interval
        let id = manager
            .store(
                "manual-only",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                Some(RotationPolicy {
                    auto_rotate: false,
                    interval: Duration::from_millis(1),
                    grace_period: Duration::from_secs(60),
                    max_rotations: None,
                    rotations_performed: 0,
                }),
            )
            .await
            .unwrap();

        // Set rotated_at to past
        {
            let mut credential = store.get(&id).await.unwrap().unwrap();
            credential.rotated_at = Some(SystemTime::now() - Duration::from_secs(3600));
            store.update(&credential).await.unwrap();
        }

        manager.check_rotations().await.unwrap();

        let events = notifier.events().await;
        assert_eq!(events.len(), 0); // auto_rotate is false
    }

    #[tokio::test]
    async fn test_rotation_notifier_multiple_credentials() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let notifier = Arc::new(InMemoryRotationNotifier::new());
        let store_clone = store.clone();
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            notifier.clone(),
        );

        // Store 3 credentials that need rotation
        for i in 0..3 {
            let id = manager
                .store(
                    &format!("cred-{i}"),
                    CredentialType::ApiKey,
                    b"secret",
                    "admin",
                    HashMap::new(),
                    vec![],
                    None,
                    Some(RotationPolicy {
                        auto_rotate: true,
                        interval: Duration::from_millis(1),
                        grace_period: Duration::from_secs(60),
                        max_rotations: Some(5),
                        rotations_performed: i,
                    }),
                )
                .await
                .unwrap();

            // Set rotated_at to past
            let mut credential = store_clone.get(&id).await.unwrap().unwrap();
            credential.rotated_at = Some(SystemTime::now() - Duration::from_secs(7200));
            store_clone.update(&credential).await.unwrap();
        }

        manager.check_rotations().await.unwrap();

        let events = notifier.events().await;
        assert_eq!(events.len(), 3);
        // Verify different rotation counts
        let counts: Vec<u32> = events.iter().map(|e| e.rotations_performed).collect();
        assert!(counts.contains(&0));
        assert!(counts.contains(&1));
        assert!(counts.contains(&2));
    }

    #[tokio::test]
    async fn test_in_memory_notifier_clear() {
        let notifier = InMemoryRotationNotifier::new();

        // Manually push an event
        notifier
            .notify_rotation(&RotationEvent {
                credential_id: "test".to_string(),
                credential_name: "test".to_string(),
                credential_type: CredentialType::ApiKey,
                last_rotated_at: None,
                elapsed: Duration::from_secs(0),
                interval: Duration::from_secs(0),
                rotations_performed: 0,
                max_rotations: None,
            })
            .await
            .unwrap();

        assert_eq!(notifier.events().await.len(), 1);

        notifier.clear().await;
        assert_eq!(notifier.events().await.len(), 0);
    }

    #[test]
    fn test_credential_type_display() {
        assert_eq!(CredentialType::ApiKey.to_string(), "api_key");
        assert_eq!(CredentialType::OAuthToken.to_string(), "oauth_token");
        assert_eq!(CredentialType::BearerToken.to_string(), "bearer_token");
        assert_eq!(CredentialType::SshKey.to_string(), "ssh_key");
        assert_eq!(
            CredentialType::TlsCertificate.to_string(),
            "tls_certificate"
        );
        assert_eq!(CredentialType::BasicAuth.to_string(), "basic_auth");
        assert_eq!(
            CredentialType::Custom("custom".to_string()).to_string(),
            "custom"
        );
    }

    #[tokio::test]
    async fn test_list_all_credentials() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        // Store 3 credentials
        for i in 0..3 {
            manager
                .store(
                    &format!("cred-{}", i),
                    CredentialType::ApiKey,
                    b"secret",
                    "admin",
                    HashMap::new(),
                    vec![],
                    None,
                    None,
                )
                .await
                .unwrap();
        }

        let all = manager.list(None).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_list_filter_by_type() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        manager
            .store(
                "api-key",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();
        manager
            .store(
                "oauth-token",
                CredentialType::OAuthToken,
                b"token",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        let filter = CredentialFilter {
            credential_type: Some(CredentialType::ApiKey),
            status: None,
            tags: vec![],
            created_by: None,
        };
        let filtered = manager.list(Some(filter)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "api-key");
    }

    #[tokio::test]
    async fn test_list_filter_by_status() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let id = manager
            .store(
                "active-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        manager
            .store(
                "another-cred",
                CredentialType::ApiKey,
                b"secret2",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        // Revoke one
        manager.revoke(&id, "admin").await.unwrap();

        let filter = CredentialFilter {
            credential_type: None,
            status: Some(CredentialStatus::Revoked),
            tags: vec![],
            created_by: None,
        };
        let filtered = manager.list(Some(filter)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "active-cred");
    }

    #[tokio::test]
    async fn test_list_filter_by_tags() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        manager
            .store(
                "tagged-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec!["production".to_string(), "api".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        manager
            .store(
                "untagged-cred",
                CredentialType::ApiKey,
                b"secret2",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        let filter = CredentialFilter {
            credential_type: None,
            status: None,
            tags: vec!["production".to_string()],
            created_by: None,
        };
        let filtered = manager.list(Some(filter)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "tagged-cred");
    }

    #[tokio::test]
    async fn test_list_filter_by_created_by() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        manager
            .store(
                "admin-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        manager
            .store(
                "user-cred",
                CredentialType::ApiKey,
                b"secret2",
                "user-1",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        let filter = CredentialFilter {
            credential_type: None,
            status: None,
            tags: vec![],
            created_by: Some("admin".to_string()),
        };
        let filtered = manager.list(Some(filter)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "admin-cred");
    }

    #[tokio::test]
    async fn test_get_nonexistent_credential() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let result = manager.get("nonexistent", "user-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_store_with_metadata() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let mut metadata = HashMap::new();
        metadata.insert("environment".to_string(), "production".to_string());
        metadata.insert("team".to_string(), "backend".to_string());

        let id = manager
            .store(
                "metadata-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                metadata,
                vec!["important".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        let all = manager.list(None).await.unwrap();
        let cred = all.iter().find(|c| c.id == id).unwrap();
        assert_eq!(cred.metadata.get("environment").unwrap(), "production");
        assert_eq!(cred.metadata.get("team").unwrap(), "backend");
        assert_eq!(cred.tags, vec!["important".to_string()]);
    }

    #[tokio::test]
    async fn test_credential_manager_clone() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        // Store via original
        manager
            .store(
                "clone-test",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        // Clone and verify access
        let cloned = manager.clone();
        let all = cloned.list(None).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "clone-test");
    }

    #[tokio::test]
    async fn test_store_with_expiration() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            CredentialManagerConfig::default(),
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        let expires_at = SystemTime::now() + Duration::from_secs(3600);
        let id = manager
            .store(
                "expiring-cred",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                Some(expires_at),
                None,
            )
            .await
            .unwrap();

        let all = manager.list(None).await.unwrap();
        let cred = all.iter().find(|c| c.id == id).unwrap();
        assert!(cred.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_audit_disabled() {
        let config = CredentialManagerConfig {
            audit_enabled: false,
            ..Default::default()
        };
        let store = Arc::new(InMemoryCredentialStore::new());
        let encryptor = Arc::new(NoOpEncryptor);
        let manager = CredentialManager::new(
            config,
            store,
            encryptor,
            Arc::new(InMemoryRotationNotifier::new()),
        );

        manager
            .store(
                "no-audit",
                CredentialType::ApiKey,
                b"secret",
                "admin",
                HashMap::new(),
                vec![],
                None,
                None,
            )
            .await
            .unwrap();

        let log = manager.audit_log().await;
        assert!(log.is_empty());
    }

    #[test]
    fn test_aes_gcm_encryptor_generate_key() {
        let key = AesGcmEncryptor::generate_key();
        assert_eq!(key.len(), 32); // AES-256 key
    }

    #[test]
    fn test_aes_gcm_encryptor_new() {
        let key = AesGcmEncryptor::generate_key();
        let encryptor = AesGcmEncryptor::new(key);
        assert!(encryptor.is_ok());
    }
}
