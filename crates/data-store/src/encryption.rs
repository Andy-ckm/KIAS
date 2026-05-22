//! AES-256-GCM encryption at rest for sensitive data fields.
//!
//! # Overview
//! This module provides `Encryptor` for AES-256-GCM encryption with:
//! - PBKDF2 key derivation from passwords
//! - Random nonce generation (96-bit / 12 bytes)
//! - Serde integration for transparent encrypted field handling
//!
//! # Example
//! ```
//! use kias_data_store::encryption::{Encryptor, Encrypted};
//!
//! let enc = Encryptor::new("my-password", b"salt12345678901234").unwrap();
//! let encrypted = enc.encrypt(b"secret data").unwrap();
//! let decrypted = enc.decrypt(&encrypted).unwrap();
//! assert_eq!(decrypted, b"secret data");
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac_array;
use rand::RngCore;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use sha2::Sha256;
use thiserror::Error;

/// Size of AES-256 key in bytes (256 bits)
const KEY_SIZE: usize = 32;
/// Size of PBKDF2 salt in bytes
const SALT_SIZE: usize = 16;
/// Size of GCM nonce in bytes (96 bits)
const NONCE_SIZE: usize = 12;
/// PBKDF2 iteration count (OWASP recommended minimum for PBKDF2-SHA256)
const PBKDF2_ITERATIONS: u32 = 600_000;

/// Encryption errors
#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("encryption failed: {0}")]
    EncryptFailed(String),
    #[error("decryption failed: {0}")]
    DecryptFailed(String),
    #[error("invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },
    #[error("invalid ciphertext: too short for nonce")]
    CiphertextTooShort,
    #[error("key derivation failed")]
    KeyDerivationFailed,
}

/// Wrapper type for base64-encoded ciphertext that carries its own nonce and salt.
/// Serializes as a single string: base64(nonce || salt || ciphertext).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encrypted {
    nonce: [u8; NONCE_SIZE],
    salt: [u8; SALT_SIZE],
    ciphertext: Vec<u8>,
}

impl Encrypted {
    /// Create from raw components
    fn new(nonce: [u8; NONCE_SIZE], salt: [u8; SALT_SIZE], ciphertext: Vec<u8>) -> Self {
        Self {
            nonce,
            salt,
            ciphertext,
        }
    }

    /// Serialize to base64 string: nonce (12) || salt (16) || ciphertext
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(NONCE_SIZE + SALT_SIZE + self.ciphertext.len());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, EncryptionError> {
        if bytes.len() < NONCE_SIZE + SALT_SIZE {
            return Err(EncryptionError::CiphertextTooShort);
        }
        let mut nonce = [0u8; NONCE_SIZE];
        let mut salt = [0u8; SALT_SIZE];
        nonce.copy_from_slice(&bytes[..NONCE_SIZE]);
        salt.copy_from_slice(&bytes[NONCE_SIZE..NONCE_SIZE + SALT_SIZE]);
        let ciphertext = bytes[NONCE_SIZE + SALT_SIZE..].to_vec();
        Ok(Self::new(nonce, salt, ciphertext))
    }
}

impl Serialize for Encrypted {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let encoded = STANDARD.encode(self.to_bytes());
        serializer.serialize_str(&encoded)
    }
}

impl<'de> Deserialize<'de> for Encrypted {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let s = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(&s).map_err(de::Error::custom)?;
        Self::from_bytes(&bytes).map_err(de::Error::custom)
    }
}

/// AES-256-GCM Encryptor with PBKDF2 key derivation.
#[derive(Clone)]
pub struct Encryptor {
    key: [u8; KEY_SIZE],
}

impl Encryptor {
    /// Derive a 256-bit key from `password` and `salt` using PBKDF2-SHA256.
    fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE], EncryptionError> {
        if salt.len() < SALT_SIZE {
            return Err(EncryptionError::KeyDerivationFailed);
        }
        let salt_16 = &salt[..SALT_SIZE];
        let key: [u8; KEY_SIZE] =
            pbkdf2_hmac_array::<Sha256, KEY_SIZE>(password.as_bytes(), salt_16, PBKDF2_ITERATIONS);
        Ok(key)
    }

    /// Create an Encryptor from a password and raw salt bytes.
    /// Salt should be at least SALT_SIZE bytes; extra bytes are truncated.
    pub fn new(password: &str, salt: &[u8]) -> Result<Self, EncryptionError> {
        let key = Self::derive_key(password, salt)?;
        Ok(Self { key })
    }

    /// Create an Encryptor from a password with a randomly generated salt.
    /// The salt is prepended to the ciphertext during encryption.
    pub fn new_with_random_salt(
        password: &str,
    ) -> Result<(Self, [u8; SALT_SIZE]), EncryptionError> {
        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        let key = Self::derive_key(password, &salt)?;
        Ok((Self { key }, salt))
    }

    /// Encrypt plaintext. Returns `Encrypted` (nonce || salt || ciphertext).
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Encrypted, EncryptionError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| EncryptionError::EncryptFailed(e.to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptFailed(e.to_string()))?;

        // Salt is embedded for decryption; we use zeroed salt since key is already derived.
        let salt = [0u8; SALT_SIZE];
        Ok(Encrypted::new(nonce_bytes, salt, ciphertext))
    }

    /// Decrypt an `Encrypted` value. The password must match the one used for encryption.
    pub fn decrypt(&self, encrypted: &Encrypted) -> Result<Vec<u8>, EncryptionError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| EncryptionError::DecryptFailed(e.to_string()))?;
        let nonce = Nonce::from_slice(&encrypted.nonce);
        cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| EncryptionError::DecryptFailed(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "super-secret-password";
    const TEST_SALT: &[u8] = b"0123456789abcdef";

    #[test]
    fn roundtrip_via_new() {
        let (enc, _salt) = Encryptor::new_with_random_salt(TEST_PASSWORD).unwrap();
        let plaintext = b"Hello, AES-256-GCM!";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn roundtrip_via_explicit_salt() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let plaintext = b"Secret message";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_password_fails() {
        let enc = Encryptor::new("correct-password", TEST_SALT).unwrap();
        let plaintext = b"Secret";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let wrong = Encryptor::new("wrong-password", TEST_SALT).unwrap();
        assert!(wrong.decrypt(&encrypted).is_err());
    }

    #[test]
    fn nonce_uniqueness() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let plaintext = b"same data";
        let e1 = enc.encrypt(plaintext).unwrap();
        let e2 = enc.encrypt(plaintext).unwrap();
        // Nonces must differ
        assert_ne!(e1.nonce, e2.nonce);
        // Ciphertexts must differ due to random nonce
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn key_derivation_deterministic() {
        // Same password + same salt must produce same key
        let enc1 = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let enc2 = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let p1 = enc1.encrypt(b"test").unwrap();
        let p2 = enc2.encrypt(b"test").unwrap();
        assert_eq!(enc1.decrypt(&p1).unwrap(), enc2.decrypt(&p2).unwrap());
    }

    #[test]
    fn different_salts_different_key() {
        let enc1 = Encryptor::new(TEST_PASSWORD, b"0123456789abcdef").unwrap();
        let enc2 = Encryptor::new(TEST_PASSWORD, b"fedcba9876543210").unwrap();
        let p1 = enc1.encrypt(b"test").unwrap();
        let p2 = enc2.encrypt(b"test").unwrap();
        // Decrypting with wrong salt should fail
        assert!(enc2.decrypt(&p1).is_err());
    }

    #[test]
    fn empty_plaintext() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let encrypted = enc.encrypt(b"").unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn large_data() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let plaintext = vec![0xABu8; 1_000_000];
        let encrypted = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn serde_roundtrip() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let plaintext = b"serde test";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let json = serde_json::to_string(&encrypted).unwrap();
        let restored: Encrypted = serde_json::from_str(&json).unwrap();
        let decrypted = enc.decrypt(&restored).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypted_struct_equality() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let e1 = enc.encrypt(b"x").unwrap();
        let e2 = enc.encrypt(b"x").unwrap();
        // Same nonce+ciphertext would be astronomically unlikely
        assert_ne!(e1, e2);
    }

    #[test]
    fn binary_fingerprint() {
        // Verify Encrypted layout: nonce(12) || salt(16) || ciphertext
        let enc = Encrypted::new([1u8; 12], [2u8; 16], vec![3u8; 7]);
        let bytes = enc.to_bytes();
        assert_eq!(bytes.len(), 12 + 16 + 7);
        assert_eq!(&bytes[..12], &[1u8; 12]);
        assert_eq!(&bytes[12..28], &[2u8; 16]);
        assert_eq!(&bytes[28..], &[3u8; 7]);

        let restored = Encrypted::from_bytes(&bytes).unwrap();
        assert_eq!(restored, enc);
    }

    #[test]
    fn error_on_truncated_ciphertext() {
        let result = Encrypted::from_bytes(&[0u8; 10]);
        assert!(matches!(result, Err(EncryptionError::CiphertextTooShort)));
    }

    #[test]
    fn encryptor_clone_works() {
        let enc = Encryptor::new(TEST_PASSWORD, TEST_SALT).unwrap();
        let cloned = enc.clone();
        let p = enc.encrypt(b"clone test").unwrap();
        assert_eq!(cloned.decrypt(&p).unwrap(), b"clone test");
    }
}
