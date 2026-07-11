//! # GxP-Compliant Authentication (Phase 2)
//!
//! Authentication and access control module implementing:
//! - **FDA 21 CFR Part 11 §11.200** — Two-factor authentication for electronic signing
//! - **FDA 21 CFR Part 11 §11.300** — Password aging, unique credentials, lockout
//! - **EU Annex 11 Clause 8** — Role-based access control
//!
//! All auth operations produce audit events via [`AuthAuditEvent`].

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, SaltString},
    Argon2, PasswordVerifier,
};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

// ── Errors ──────────────────────────────────────────────────────────────

/// Errors that can occur during GxP authentication operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The account is temporarily locked due to failed login attempts.
    AccountLocked { until: Option<String> },
    /// The provided credentials are incorrect.
    InvalidCredentials,
    /// The user's password has expired and must be changed.
    PasswordExpired,
    /// The new password does not meet the configured password policy.
    PasswordTooWeak { reason: String },
    /// The new password matches one of the recently used passwords.
    PasswordReused,
    /// A secure password hash could not be generated.
    PasswordHashingFailed,
    /// Two-factor authentication is required but not yet completed.
    TwoFactorRequired,
    /// The provided 2FA code is invalid.
    TwoFactorInvalid,
    /// The session has expired.
    SessionExpired,
    /// The requested user was not found.
    UserNotFound,
    /// A user with this username already exists.
    UserAlreadyExists,
    /// The requested role was not found on the user.
    RoleNotFound,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountLocked { until } => match until {
                Some(u) => write!(f, "Account locked until {u}"),
                None => write!(f, "Account locked indefinitely"),
            },
            Self::InvalidCredentials => write!(f, "Invalid credentials"),
            Self::PasswordExpired => write!(f, "Password has expired"),
            Self::PasswordTooWeak { reason } => {
                write!(f, "Password too weak: {reason}")
            }
            Self::PasswordReused => {
                write!(f, "Password matches a recently used password")
            }
            Self::PasswordHashingFailed => write!(f, "Password hashing failed"),
            Self::TwoFactorRequired => {
                write!(f, "Two-factor authentication required")
            }
            Self::TwoFactorInvalid => write!(f, "Invalid two-factor code"),
            Self::SessionExpired => write!(f, "Session expired"),
            Self::UserNotFound => write!(f, "User not found"),
            Self::UserAlreadyExists => write!(f, "User already exists"),
            Self::RoleNotFound => write!(f, "Role not found on user"),
        }
    }
}

impl std::error::Error for AuthError {}

// ── Password Policy (§11.300) ───────────────────────────────────────────

/// Password policy for GxP compliance (§11.300).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordPolicy {
    /// Minimum password length.
    pub min_length: usize,
    /// Require at least one uppercase letter.
    pub require_uppercase: bool,
    /// Require at least one lowercase letter.
    pub require_lowercase: bool,
    /// Require at least one digit.
    pub require_digit: bool,
    /// Require at least one special character.
    pub require_special: bool,
    /// §11.300(b) Maximum password age in days before forced rotation.
    pub max_age_days: u32,
    /// Number of previous passwords that cannot be reused.
    pub history_count: usize,
    /// Number of failed login attempts before account lockout.
    pub lockout_attempts: u32,
    /// Account lockout duration in minutes.
    pub lockout_duration_minutes: u32,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
            max_age_days: 90,
            history_count: 12,
            lockout_attempts: 5,
            lockout_duration_minutes: 30,
        }
    }
}

// ── Two-Factor (§11.200) ────────────────────────────────────────────────

/// Two-factor authentication configuration (§11.200).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorConfig {
    pub enabled: bool,
    pub method: TwoFactorMethod,
    /// Pre-generated backup codes (hashed).
    pub backup_codes: Vec<String>,
}

/// Supported two-factor methods.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TwoFactorMethod {
    /// Time-based OTP (Google Authenticator style).
    Totp,
    /// Email-delivered code.
    Email,
    /// Hardware security key / token.
    Hardware,
}

// ── User ────────────────────────────────────────────────────────────────

/// User credential with GxP tracking fields.
#[derive(Clone, Serialize, Deserialize)]
pub struct GxpUser {
    pub user_id: String,
    /// Unique identification code (§11.100(d)).
    pub username: String,
    /// Printed / display name for §11.50(a)(1).
    pub display_name: String,
    pub email: String,
    /// Hashed password (SHA-256 for demonstration; production should use Argon2/bcrypt).
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// When the password was last changed (for §11.300(b) aging).
    pub password_changed_at: DateTime<Utc>,
    /// Hashes of previous passwords to prevent reuse.
    #[serde(skip_serializing)]
    pub password_history: Vec<String>,
    pub is_active: bool,
    /// Roles for RBAC (EU Annex 11 Clause 8).
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failed_login_attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
    pub two_factor_enabled: bool,
    #[serde(skip_serializing)]
    pub two_factor_secret: Option<String>,
}

impl fmt::Debug for GxpUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GxpUser")
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("display_name", &"[REDACTED]")
            .field("email", &"[REDACTED]")
            .field("password_hash", &"[REDACTED]")
            .field("password_history", &"[REDACTED]")
            .field("is_active", &self.is_active)
            .field("roles", &self.roles)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("failed_login_attempts", &self.failed_login_attempts)
            .field("locked_until", &self.locked_until)
            .field("two_factor_enabled", &self.two_factor_enabled)
            .field("two_factor_secret", &"[REDACTED]")
            .finish()
    }
}

// ── Session (§11.200(a)(1)) ─────────────────────────────────────────────

/// Session with continuous tracking (§11.200(a)(1)).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GxpSession {
    pub session_id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub device_info: Option<String>,
    /// Continuous session indicator for §11.200(a)(1).
    pub is_continuous: bool,
}

// ── Audit Events ────────────────────────────────────────────────────────

/// Audit event for auth operations (linked to GxP audit log).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthAuditEvent {
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub event_type: AuthEventType,
    pub detail: String,
    pub ip_address: Option<String>,
}

/// Categorised auth event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthEventType {
    Login,
    LoginFailed,
    Logout,
    PasswordChanged,
    PasswordExpired,
    AccountLocked,
    AccountUnlocked,
    TwoFactorEnabled,
    TwoFactorVerified,
    RoleAssigned,
    RoleRevoked,
    SessionExpired,
}

// ── Helper functions ────────────────────────────────────────────────────

/// Hash a password with Argon2id and a unique random salt.
fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashingFailed)
}

/// Verify a password against a PHC-formatted Argon2id hash.
fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Generate an RFC 6238 compatible six-digit TOTP using HMAC-SHA1.
fn generate_totp(secret: &str, now: DateTime<Utc>) -> String {
    let secret_bytes = hex_decode(secret).unwrap_or_default();
    if secret_bytes.is_empty() {
        return "000000".to_string();
    }
    let counter = (now.timestamp().max(0) as u64) / 30;
    let mut mac =
        Hmac::<Sha1>::new_from_slice(&secret_bytes).expect("HMAC accepts keys of any length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!("{:06}", binary % 1_000_000)
}

fn hash_recovery_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

/// Minimal hex encoding (avoids adding `hex` crate as dependency).
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── GxpAuthManager ──────────────────────────────────────────────────────

/// Central authentication manager implementing GxP-compliant policies.
pub struct GxpAuthManager {
    policy: PasswordPolicy,
    users: HashMap<String, GxpUser>,         // user_id -> user
    username_index: HashMap<String, String>, // username -> user_id
    sessions: HashMap<String, GxpSession>,   // session_id -> session
    audit_log: Vec<AuthAuditEvent>,
    two_factor_configs: HashMap<String, TwoFactorConfig>, // user_id -> 2FA config
    /// The current time, overridable for testing password expiry.
    now_override: Option<DateTime<Utc>>,
}

impl GxpAuthManager {
    /// Create a new auth manager with the given password policy.
    pub fn new(policy: PasswordPolicy) -> Self {
        Self {
            policy,
            users: HashMap::new(),
            username_index: HashMap::new(),
            sessions: HashMap::new(),
            audit_log: Vec::new(),
            two_factor_configs: HashMap::new(),
            now_override: None,
        }
    }

    /// Override the "current time" for testing password expiry.
    #[cfg(test)]
    fn with_now(mut self, now: DateTime<Utc>) -> Self {
        self.now_override = Some(now);
        self
    }

    fn now(&self) -> DateTime<Utc> {
        self.now_override.unwrap_or_else(Utc::now)
    }

    // ── Password Strength Validation ────────────────────────────────

    /// Validate a password against the configured policy.
    pub fn validate_password_strength(&self, password: &str) -> Result<(), AuthError> {
        let p = &self.policy;
        if password.len() < p.min_length {
            return Err(AuthError::PasswordTooWeak {
                reason: format!(
                    "Must be at least {} characters (got {})",
                    p.min_length,
                    password.len()
                ),
            });
        }
        if p.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(AuthError::PasswordTooWeak {
                reason: "Must contain at least one uppercase letter".into(),
            });
        }
        if p.require_lowercase && !password.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(AuthError::PasswordTooWeak {
                reason: "Must contain at least one lowercase letter".into(),
            });
        }
        if p.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(AuthError::PasswordTooWeak {
                reason: "Must contain at least one digit".into(),
            });
        }
        if p.require_special && !password.chars().any(|c| !c.is_ascii_alphanumeric()) {
            return Err(AuthError::PasswordTooWeak {
                reason: "Must contain at least one special character".into(),
            });
        }
        Ok(())
    }

    // ── User Management ─────────────────────────────────────────────

    /// Create a new user with the given credentials and roles.
    pub fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        email: &str,
        password: &str,
        roles: Vec<String>,
    ) -> Result<GxpUser, AuthError> {
        if self.username_index.contains_key(username) {
            return Err(AuthError::UserAlreadyExists);
        }
        self.validate_password_strength(password)?;

        let now = self.now();
        let user_id = Uuid::new_v4().to_string();
        let user = GxpUser {
            user_id: user_id.clone(),
            username: username.to_string(),
            display_name: display_name.to_string(),
            email: email.to_string(),
            password_hash: hash_password(password)?,
            password_changed_at: now,
            password_history: Vec::new(),
            is_active: true,
            roles,
            created_at: now,
            updated_at: now,
            failed_login_attempts: 0,
            locked_until: None,
            two_factor_enabled: false,
            two_factor_secret: None,
        };
        self.username_index
            .insert(username.to_string(), user_id.clone());
        self.users.insert(user_id.clone(), user.clone());
        Ok(user)
    }

    // ── Authentication ──────────────────────────────────────────────

    /// Authenticate a user by username and password. Returns a new session
    /// on success. Checks lockout status and password expiry.
    pub fn authenticate(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<GxpSession, AuthError> {
        let user_id = self
            .username_index
            .get(username)
            .cloned()
            .ok_or(AuthError::InvalidCredentials)?;

        let now = self.now();
        let lockout_attempts = self.policy.lockout_attempts;
        let lockout_duration = self.policy.lockout_duration_minutes;

        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(AuthError::InvalidCredentials)?;

        // Check lockout
        if let Some(locked_until) = user.locked_until {
            if now < locked_until {
                let until_str = locked_until.to_rfc3339();
                return Err(AuthError::AccountLocked {
                    until: Some(until_str),
                });
            }
            // Lock expired, clear it
            user.locked_until = None;
            user.failed_login_attempts = 0;
        }

        // Verify password
        if !verify_password(password, &user.password_hash) {
            user.failed_login_attempts += 1;
            if user.failed_login_attempts >= lockout_attempts {
                let lock_until = now + Duration::minutes(lockout_duration as i64);
                user.locked_until = Some(lock_until);
                self.audit_log.push(AuthAuditEvent {
                    timestamp: now,
                    user_id: user_id.clone(),
                    event_type: AuthEventType::AccountLocked,
                    detail: format!(
                        "Locked after {} failed attempts",
                        user.failed_login_attempts
                    ),
                    ip_address: None,
                });
                return Err(AuthError::AccountLocked {
                    until: Some(lock_until.to_rfc3339()),
                });
            }
            let attempts = user.failed_login_attempts;
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: user_id.clone(),
                event_type: AuthEventType::LoginFailed,
                detail: format!("Failed attempt {attempts}/{lockout_attempts}",),
                ip_address: None,
            });
            return Err(AuthError::InvalidCredentials);
        }

        // Password expired?
        let expiry = user.password_changed_at + Duration::days(self.policy.max_age_days as i64);
        let two_factor_enabled = user.two_factor_enabled;
        if now > expiry {
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: user_id.clone(),
                event_type: AuthEventType::PasswordExpired,
                detail: "Password expired; must change before access".into(),
                ip_address: None,
            });
            return Err(AuthError::PasswordExpired);
        }

        // Check 2FA
        if two_factor_enabled {
            return Err(AuthError::TwoFactorRequired);
        }

        // Success — reset fail count
        user.failed_login_attempts = 0;
        user.locked_until = None;
        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.clone(),
            event_type: AuthEventType::Login,
            detail: "Successful authentication".into(),
            ip_address: None,
        });

        let session = self.create_session_inner(&user_id, None, None, now);
        Ok(session)
    }

    // ── Password Management (§11.300) ───────────────────────────────

    /// Change a user's password. Validates old password, new password
    /// strength, and prevents reuse of recent passwords.
    pub fn change_password(
        &mut self,
        user_id: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        // Validate new password strength first (no borrow needed)
        self.validate_password_strength(new_password)?;

        let new_hash = hash_password(new_password)?;
        let now = self.now();
        let history_limit = self.policy.history_count;

        let user = self.users.get_mut(user_id).ok_or(AuthError::UserNotFound)?;

        // Verify old password
        if !verify_password(old_password, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }

        // Check history (§11.300(c))
        for old_hash in user.password_history.iter().rev().take(history_limit) {
            if verify_password(new_password, old_hash) {
                return Err(AuthError::PasswordReused);
            }
        }
        // Also check current password
        if verify_password(new_password, &user.password_hash) {
            return Err(AuthError::PasswordReused);
        }

        // Rotate password
        let old_hash = user.password_hash.clone();
        user.password_history.push(old_hash);
        // Trim history
        if user.password_history.len() > history_limit * 2 {
            let drain_to = user.password_history.len() - history_limit;
            user.password_history.drain(..drain_to);
        }
        user.password_hash = new_hash;
        user.password_changed_at = now;
        user.updated_at = now;

        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::PasswordChanged,
            detail: "Password changed successfully".into(),
            ip_address: None,
        });
        Ok(())
    }

    /// Check whether the user's password has expired.
    /// Returns `Some(expiry_datetime)` if expired, `None` if still valid.
    pub fn check_password_expiry(&self, user_id: &str) -> Option<DateTime<Utc>> {
        let user = self.users.get(user_id)?;
        let expiry = user.password_changed_at + Duration::days(self.policy.max_age_days as i64);
        if self.now() > expiry {
            Some(expiry)
        } else {
            None
        }
    }

    // ── Two-Factor Authentication (§11.200) ─────────────────────────

    /// Enable two-factor authentication for a user.
    /// Generates a secret and backup codes.
    pub fn enable_two_factor(&mut self, user_id: &str) -> Result<TwoFactorConfig, AuthError> {
        let now = self.now();

        let user = self.users.get_mut(user_id).ok_or(AuthError::UserNotFound)?;

        let mut secret_bytes = [0_u8; 20];
        OsRng.fill_bytes(&mut secret_bytes);
        let secret = hex_encode(&secret_bytes);
        // Generate 8 backup codes
        let backup_codes: Vec<String> = (0..8)
            .map(|_| Uuid::new_v4().to_string()[..8].to_string())
            .collect();

        user.two_factor_enabled = true;
        user.two_factor_secret = Some(secret.clone());
        user.updated_at = now;

        let stored_config = TwoFactorConfig {
            enabled: true,
            method: TwoFactorMethod::Totp,
            backup_codes: backup_codes
                .iter()
                .map(|code| hash_recovery_code(code))
                .collect(),
        };
        self.two_factor_configs
            .insert(user_id.to_string(), stored_config);

        // Recovery codes are returned exactly once; only hashes are retained.
        let enrollment_config = TwoFactorConfig {
            enabled: true,
            method: TwoFactorMethod::Totp,
            backup_codes,
        };

        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::TwoFactorEnabled,
            detail: "Two-factor authentication enabled (TOTP)".into(),
            ip_address: None,
        });

        Ok(enrollment_config)
    }

    /// Verify a two-factor code for a user.
    pub fn verify_two_factor(&mut self, user_id: &str, code: &str) -> Result<bool, AuthError> {
        let user = self.users.get(user_id).ok_or(AuthError::UserNotFound)?;

        if !user.two_factor_enabled {
            return Err(AuthError::TwoFactorRequired);
        }

        let secret = user
            .two_factor_secret
            .as_ref()
            .ok_or(AuthError::TwoFactorRequired)?
            .clone();

        let now = self.now();

        // Check TOTP code
        let expected = generate_totp(&secret, now);
        if code == expected {
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: user_id.to_string(),
                event_type: AuthEventType::TwoFactorVerified,
                detail: "TOTP code verified".into(),
                ip_address: None,
            });
            return Ok(true);
        }

        // Check a recovery code by hash and consume it atomically on success.
        let code_hash = hash_recovery_code(code);
        let used_recovery_code = self
            .two_factor_configs
            .get_mut(user_id)
            .and_then(|config| {
                config
                    .backup_codes
                    .iter()
                    .position(|stored| stored == &code_hash)
                    .map(|index| config.backup_codes.remove(index))
            })
            .is_some();
        if used_recovery_code {
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: user_id.to_string(),
                event_type: AuthEventType::TwoFactorVerified,
                detail: "Recovery code used".into(),
                ip_address: None,
            });
            return Ok(true);
        }

        Err(AuthError::TwoFactorInvalid)
    }

    // ── Session Management ──────────────────────────────────────────

    /// Create a new session for a user.
    pub fn create_session(
        &mut self,
        user_id: &str,
        ip_address: Option<String>,
        device_info: Option<String>,
    ) -> Result<GxpSession, AuthError> {
        if !self.users.contains_key(user_id) {
            return Err(AuthError::UserNotFound);
        }
        let now = self.now();
        Ok(self.create_session_inner(user_id, ip_address, device_info, now))
    }

    fn create_session_inner(
        &mut self,
        user_id: &str,
        ip_address: Option<String>,
        device_info: Option<String>,
        now: DateTime<Utc>,
    ) -> GxpSession {
        let session = GxpSession {
            session_id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            created_at: now,
            last_activity: now,
            expires_at: now + Duration::hours(8), // 8-hour session
            ip_address,
            device_info,
            is_continuous: true, // §11.200(a)(1)
        };
        self.sessions
            .insert(session.session_id.clone(), session.clone());
        session
    }

    /// Validate an existing session. Returns the session if still valid.
    pub fn validate_session(&mut self, session_id: &str) -> Result<GxpSession, AuthError> {
        let now = self.now();
        let session = self
            .sessions
            .get(session_id)
            .ok_or(AuthError::SessionExpired)?
            .clone();

        if now > session.expires_at {
            self.sessions.remove(session_id);
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: session.user_id.clone(),
                event_type: AuthEventType::SessionExpired,
                detail: format!("Session {session_id} expired"),
                ip_address: session.ip_address.clone(),
            });
            return Err(AuthError::SessionExpired);
        }

        Ok(session)
    }

    /// Invalidate (log out) a session.
    pub fn invalidate_session(&mut self, session_id: &str) {
        let now = self.now();
        if let Some(session) = self.sessions.remove(session_id) {
            self.audit_log.push(AuthAuditEvent {
                timestamp: now,
                user_id: session.user_id,
                event_type: AuthEventType::Logout,
                detail: format!("Session {session_id} invalidated"),
                ip_address: session.ip_address,
            });
        }
    }

    // ── Role Management (EU Annex 11 Clause 8) ──────────────────────

    /// Assign a role to a user.
    pub fn assign_role(&mut self, user_id: &str, role: &str) -> Result<(), AuthError> {
        let now = self.now();
        let user = self.users.get_mut(user_id).ok_or(AuthError::UserNotFound)?;

        if !user.roles.iter().any(|r| r == role) {
            user.roles.push(role.to_string());
            user.updated_at = now;
        }

        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::RoleAssigned,
            detail: format!("Role '{role}' assigned"),
            ip_address: None,
        });
        Ok(())
    }

    /// Revoke a role from a user.
    pub fn revoke_role(&mut self, user_id: &str, role: &str) -> Result<(), AuthError> {
        let now = self.now();
        let user = self.users.get_mut(user_id).ok_or(AuthError::UserNotFound)?;

        let idx = user.roles.iter().position(|r| r == role);
        match idx {
            Some(i) => {
                user.roles.remove(i);
                user.updated_at = now;
            }
            None => return Err(AuthError::RoleNotFound),
        }

        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::RoleRevoked,
            detail: format!("Role '{role}' revoked"),
            ip_address: None,
        });
        Ok(())
    }

    // ── Account Lock / Unlock ───────────────────────────────────────

    /// Lock a user account (e.g., by administrator action).
    pub fn lock_account(&mut self, user_id: &str, reason: &str) {
        let now = self.now();
        if let Some(user) = self.users.get_mut(user_id) {
            user.locked_until = Some(now + Duration::days(365 * 10)); // effectively permanent
            user.updated_at = now;
        }
        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::AccountLocked,
            detail: format!("Account locked: {reason}"),
            ip_address: None,
        });
    }

    /// Unlock a user account.
    pub fn unlock_account(&mut self, user_id: &str) {
        let now = self.now();
        if let Some(user) = self.users.get_mut(user_id) {
            user.locked_until = None;
            user.failed_login_attempts = 0;
            user.updated_at = now;
        }
        self.audit_log.push(AuthAuditEvent {
            timestamp: now,
            user_id: user_id.to_string(),
            event_type: AuthEventType::AccountUnlocked,
            detail: "Account unlocked".into(),
            ip_address: None,
        });
    }

    // ── Audit Log ───────────────────────────────────────────────────

    /// Retrieve the full authentication audit log.
    pub fn get_auth_audit_log(&self) -> Vec<AuthAuditEvent> {
        self.audit_log.clone()
    }

    /// Look up a user by user_id.
    pub fn get_user(&self, user_id: &str) -> Option<&GxpUser> {
        self.users.get(user_id)
    }

    /// Look up a user by username.
    pub fn get_user_by_username(&self, username: &str) -> Option<&GxpUser> {
        self.username_index
            .get(username)
            .and_then(|uid| self.users.get(uid))
    }

    /// Check if a user's password has expired (§11.300(b)).
    pub fn is_password_expired(&self, user_id: &str) -> bool {
        if let Some(user) = self.users.get(user_id) {
            let expiry =
                user.password_changed_at + chrono::Duration::days(self.policy.max_age_days as i64);
            self.now() > expiry
        } else {
            false
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_password() -> &'static str {
        "SecureP@ssw0rd!"
    }

    fn weak_password() -> &'static str {
        "weak"
    }

    fn make_manager() -> GxpAuthManager {
        GxpAuthManager::new(PasswordPolicy::default())
    }

    fn make_manager_with_time(now: DateTime<Utc>) -> GxpAuthManager {
        GxpAuthManager::new(PasswordPolicy::default()).with_now(now)
    }

    // ── Test 1: Create user with valid password ─────────────────────

    #[test]
    fn test_create_user_valid_password() {
        let mut mgr = make_manager();
        let result = mgr.create_user(
            "alice",
            "Alice Admin",
            "alice@example.com",
            valid_password(),
            vec!["admin".into()],
        );
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.display_name, "Alice Admin");
        assert!(user.is_active);
        assert_eq!(user.roles, vec!["admin".to_string()]);
    }

    // ── Test 2: Create user with weak password fails ────────────────

    #[test]
    fn test_create_user_weak_password_fails() {
        let mut mgr = make_manager();
        let result = mgr.create_user(
            "bob",
            "Bob Builder",
            "bob@example.com",
            weak_password(),
            vec![],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthError::PasswordTooWeak { .. } => {}
            other => panic!("Expected PasswordTooWeak, got: {other:?}"),
        }
    }

    // ── Test 3: Authenticate successfully ───────────────────────────

    #[test]
    fn test_authenticate_success() {
        let mut mgr = make_manager();
        mgr.create_user(
            "alice",
            "Alice",
            "a@e.com",
            valid_password(),
            vec!["user".into()],
        )
        .unwrap();
        let session = mgr.authenticate("alice", valid_password());
        assert!(session.is_ok());
        let s = session.unwrap();
        assert!(!s.session_id.is_empty());
        assert!(s.is_continuous);
    }

    // ── Test 4: Authenticate with wrong password fails ──────────────

    #[test]
    fn test_authenticate_wrong_password() {
        let mut mgr = make_manager();
        mgr.create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let result = mgr.authenticate("alice", "WrongPass123!");
        assert_eq!(result.unwrap_err(), AuthError::InvalidCredentials);
    }

    // ── Test 5: Account lockout after N failed attempts ─────────────

    #[test]
    fn test_account_lockout_after_failed_attempts() {
        let mut mgr = make_manager();
        mgr.create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        for _ in 0..5 {
            let _ = mgr.authenticate("alice", "WrongPass1!");
        }

        // Next attempt should be AccountLocked
        let result = mgr.authenticate("alice", valid_password());
        match result.unwrap_err() {
            AuthError::AccountLocked { .. } => {}
            other => panic!("Expected AccountLocked, got: {other:?}"),
        }
    }

    // ── Test 6: Password expiry check ───────────────────────────────

    #[test]
    fn test_password_expiry_check() {
        let base = Utc::now();
        let mut mgr = make_manager_with_time(base);
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        // Not expired yet
        assert!(mgr.check_password_expiry(&user.user_id).is_none());

        // Backdate the password to simulate aging
        let uid = user.user_id.clone();
        mgr.users.get_mut(&uid).unwrap().password_changed_at = base - Duration::days(91);
        let expiry = mgr.check_password_expiry(&uid);
        assert!(expiry.is_some());
    }

    // ── Test 7: Password change blocks reuse ────────────────────────

    #[test]
    fn test_password_change_blocks_reuse() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let uid = &user.user_id;

        // Change to a new valid password
        let new_pass = "NewSecureP@ss01!";
        mgr.change_password(uid, valid_password(), new_pass)
            .unwrap();

        // Try to change back to the old password — should be blocked
        let result = mgr.change_password(uid, new_pass, valid_password());
        assert_eq!(result.unwrap_err(), AuthError::PasswordReused);
    }

    // ── Test 8: Password change validates strength ──────────────────

    #[test]
    fn test_password_change_validates_strength() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        let result = mgr.change_password(&user.user_id, valid_password(), "short");
        match result.unwrap_err() {
            AuthError::PasswordTooWeak { .. } => {}
            other => panic!("Expected PasswordTooWeak, got: {other:?}"),
        }
    }

    // ── Test 9: Session creation and validation ─────────────────────

    #[test]
    fn test_session_creation_and_validation() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let session = mgr
            .create_session(
                &user.user_id,
                Some("127.0.0.1".into()),
                Some("test-device".into()),
            )
            .unwrap();

        let validated = mgr.validate_session(&session.session_id);
        assert!(validated.is_ok());
        let v = validated.unwrap();
        assert_eq!(v.user_id, user.user_id);
        assert_eq!(v.ip_address, Some("127.0.0.1".to_string()));
    }

    // ── Test 10: Session expiration ─────────────────────────────────

    #[test]
    fn test_session_expiration() {
        let base = Utc::now();
        let mut mgr = make_manager_with_time(base);
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let session = mgr.create_session(&user.user_id, None, None).unwrap();

        // Session should be valid now
        assert!(mgr.validate_session(&session.session_id).is_ok());

        // Move time forward 9 hours (past 8-hour expiry)
        let future = base + Duration::hours(9);
        let mut mgr = make_manager_with_time(future);
        // Re-register the session in the new manager
        mgr.sessions
            .insert(session.session_id.clone(), session.clone());
        let result = mgr.validate_session(&session.session_id);
        assert_eq!(result.unwrap_err(), AuthError::SessionExpired);
    }

    // ── Test 11: Two-factor enable and verify ───────────────────────

    #[test]
    fn test_two_factor_enable_and_verify() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        let config = mgr.enable_two_factor(&user.user_id).unwrap();
        assert!(config.enabled);
        assert_eq!(config.method, TwoFactorMethod::Totp);
        assert_eq!(config.backup_codes.len(), 8);

        // Get the secret to generate a valid code
        let secret = mgr
            .users
            .get(&user.user_id)
            .unwrap()
            .two_factor_secret
            .clone()
            .unwrap();
        let now = mgr.now();
        let code = generate_totp(&secret, now);
        let verified = mgr.verify_two_factor(&user.user_id, &code).unwrap();
        assert!(verified);
    }

    // ── Test 12: Role assignment ────────────────────────────────────

    #[test]
    fn test_role_assignment() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        mgr.assign_role(&user.user_id, "editor").unwrap();
        let updated = mgr.get_user(&user.user_id).unwrap();
        assert!(updated.roles.contains(&"editor".to_string()));

        // Assigning same role again should succeed (idempotent)
        mgr.assign_role(&user.user_id, "editor").unwrap();
    }

    // ── Test 13: Role revocation ────────────────────────────────────

    #[test]
    fn test_role_revocation() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user(
                "alice",
                "Alice",
                "a@e.com",
                valid_password(),
                vec!["admin".into()],
            )
            .unwrap();

        mgr.revoke_role(&user.user_id, "admin").unwrap();
        let updated = mgr.get_user(&user.user_id).unwrap();
        assert!(!updated.roles.contains(&"admin".to_string()));
    }

    #[test]
    fn test_role_revocation_nonexistent_role() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        let result = mgr.revoke_role(&user.user_id, "nonexistent");
        assert_eq!(result.unwrap_err(), AuthError::RoleNotFound);
    }

    // ── Test 14: Auth audit log records events ──────────────────────

    #[test]
    fn test_auth_audit_log_records_events() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        // Manually insert a stale password_changed_at so authentication succeeds
        // but first verify login produces an audit event
        mgr.authenticate("alice", valid_password()).unwrap();

        let log = mgr.get_auth_audit_log();
        assert!(log.iter().any(|e| e.event_type == AuthEventType::Login));

        // Also test that role changes produce audit events
        mgr.assign_role(&user.user_id, "viewer").unwrap();
        let log = mgr.get_auth_audit_log();
        assert!(log
            .iter()
            .any(|e| e.event_type == AuthEventType::RoleAssigned));
    }

    // ── Test 15: Locked account rejects authentication ──────────────

    #[test]
    fn test_locked_account_rejects_authentication() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        mgr.lock_account(&user.user_id, "admin action");

        // Even with correct password, locked account is rejected
        let result = mgr.authenticate("alice", valid_password());
        match result.unwrap_err() {
            AuthError::AccountLocked { .. } => {}
            other => panic!("Expected AccountLocked, got: {other:?}"),
        }
    }

    // ── Test 16: Unlock account ─────────────────────────────────────

    #[test]
    fn test_unlock_account() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();

        mgr.lock_account(&user.user_id, "test");
        mgr.unlock_account(&user.user_id);

        // Should authenticate successfully after unlock
        let session = mgr.authenticate("alice", valid_password());
        assert!(session.is_ok());
    }

    // ── Test 17: Duplicate username rejected ────────────────────────

    #[test]
    fn test_duplicate_username_rejected() {
        let mut mgr = make_manager();
        mgr.create_user("alice", "Alice A", "a1@e.com", valid_password(), vec![])
            .unwrap();
        let result = mgr.create_user("alice", "Alice B", "a2@e.com", valid_password(), vec![]);
        assert_eq!(result.unwrap_err(), AuthError::UserAlreadyExists);
    }

    // ── Test 18: Password policy defaults ───────────────────────────

    #[test]
    fn test_password_policy_defaults() {
        let policy = PasswordPolicy::default();
        assert_eq!(policy.min_length, 12);
        assert!(policy.require_uppercase);
        assert!(policy.require_lowercase);
        assert!(policy.require_digit);
        assert!(policy.require_special);
        assert_eq!(policy.max_age_days, 90);
        assert_eq!(policy.history_count, 12);
        assert_eq!(policy.lockout_attempts, 5);
        assert_eq!(policy.lockout_duration_minutes, 30);
    }

    // ── Test 19: Authenticate nonexistent user ──────────────────────

    #[test]
    fn test_authenticate_nonexistent_user() {
        let mut mgr = make_manager();
        let result = mgr.authenticate("ghost", valid_password());
        assert_eq!(result.unwrap_err(), AuthError::InvalidCredentials);
    }

    // ── Test 20: Validate password strength edge cases ──────────────

    #[test]
    fn test_validate_password_no_uppercase() {
        let mgr = make_manager();
        assert!(matches!(
            mgr.validate_password_strength("alllowercase1!"),
            Err(AuthError::PasswordTooWeak { .. })
        ));
    }

    #[test]
    fn test_validate_password_no_digit() {
        let mgr = make_manager();
        assert!(matches!(
            mgr.validate_password_strength("NoDigitsHere!"),
            Err(AuthError::PasswordTooWeak { .. })
        ));
    }

    #[test]
    fn test_validate_password_no_special() {
        let mgr = make_manager();
        assert!(matches!(
            mgr.validate_password_strength("NoSpecial1234"),
            Err(AuthError::PasswordTooWeak { .. })
        ));
    }

    #[test]
    fn test_validate_password_valid() {
        let mgr = make_manager();
        assert!(mgr.validate_password_strength(valid_password()).is_ok());
    }

    // ── Test 21: 2FA backup codes work ──────────────────────────────

    #[test]
    fn test_two_factor_backup_code() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let config = mgr.enable_two_factor(&user.user_id).unwrap();
        let backup = config.backup_codes[0].clone();
        let verified = mgr.verify_two_factor(&user.user_id, &backup).unwrap();
        assert!(verified);
    }

    // ── Test 22: 2FA invalid code rejected ──────────────────────────

    #[test]
    fn test_two_factor_invalid_code() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        mgr.enable_two_factor(&user.user_id).unwrap();
        let result = mgr.verify_two_factor(&user.user_id, "000000");
        assert_eq!(result.unwrap_err(), AuthError::TwoFactorInvalid);
    }

    // ── Test 23: Session invalidation ───────────────────────────────

    #[test]
    fn test_session_invalidation() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let session = mgr.create_session(&user.user_id, None, None).unwrap();
        mgr.invalidate_session(&session.session_id);
        let result = mgr.validate_session(&session.session_id);
        assert_eq!(result.unwrap_err(), AuthError::SessionExpired);
    }

    // ── Test 24: Change password wrong old password ─────────────────

    #[test]
    fn test_change_password_wrong_old() {
        let mut mgr = make_manager();
        let user = mgr
            .create_user("alice", "Alice", "a@e.com", valid_password(), vec![])
            .unwrap();
        let result = mgr.change_password(&user.user_id, "WrongOld1!", "NewValid1Pass!");
        assert_eq!(result.unwrap_err(), AuthError::InvalidCredentials);
    }

    // ── Test 25: Display impl for all AuthError variants ────────────

    #[test]
    fn test_auth_error_display() {
        let errors = vec![
            AuthError::AccountLocked { until: None },
            AuthError::InvalidCredentials,
            AuthError::PasswordExpired,
            AuthError::PasswordTooWeak {
                reason: "test".into(),
            },
            AuthError::PasswordReused,
            AuthError::TwoFactorRequired,
            AuthError::TwoFactorInvalid,
            AuthError::SessionExpired,
            AuthError::UserNotFound,
            AuthError::UserAlreadyExists,
            AuthError::RoleNotFound,
        ];
        for err in errors {
            // Just ensure Display doesn't panic
            let _ = format!("{err}");
        }
    }
}
