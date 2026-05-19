//! # GxP Authentication Handlers
//!
//! Endpoints for GxP-compliant authentication:
//! - `POST /auth/login` — Username/password login with optional 2FA
//! - `POST /auth/change-password` — Password rotation (§11.300)
//! - `POST /auth/verify-2fa` — Two-factor verification (§11.200)
//!
//! Reference: axum-login (3937 lines) — https://github.com/maxcountryman/axum-login

use axum::{extract::State, http::StatusCode, Json};
use kias_common::gxp_auth::{AuthError, GxpAuthManager, PasswordPolicy};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::auth::{create_claims, generate_token, Role};
use crate::AppState;

/// Shared GxP auth manager state.
pub type GxpAuthState = Arc<Mutex<GxpAuthManager>>;

/// Create a new GxP auth manager wrapped in shared state.
pub fn create_gxp_auth_state(policy: PasswordPolicy) -> GxpAuthState {
    Arc::new(Mutex::new(GxpAuthManager::new(policy)))
}

// ── Request / Response types ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Optional 2FA code (required if 2FA is enabled for the user).
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub requires_2fa: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub user_id: String,
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct Verify2faRequest {
    pub user_id: String,
    pub totp_code: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

// ── Handlers ──────────────────────────────────────────────────────────

/// POST /auth/login
///
/// Authenticates user with username/password. If 2FA is enabled,
/// returns `requires_2fa: true` and the client must call `/auth/verify-2fa`.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut auth = state.gxp_auth.lock().await;
    let jwt_config = &state.jwt_config;

    match auth.authenticate(&req.username, &req.password) {
        Ok(session) => {
            // Check if 2FA is required
            let user = auth.get_user(&session.user_id).unwrap();
            if user.two_factor_enabled {
                return Ok(Json(LoginResponse {
                    token: String::new(),
                    user_id: session.user_id,
                    username: req.username,
                    role: "pending_2fa".to_string(),
                    requires_2fa: true,
                }));
            }

            // No 2FA — generate JWT
            let role = map_roles(&user.roles);
            let claims = create_claims(&session.user_id, role, jwt_config);
            let token = generate_token(&claims, &jwt_config.secret)
                .map_err(|e| auth_error_response(AuthError::InvalidCredentials, &e.to_string()))?;

            Ok(Json(LoginResponse {
                token,
                user_id: session.user_id,
                username: req.username,
                role: role.to_string(),
                requires_2fa: false,
            }))
        }
        Err(AuthError::TwoFactorRequired) => {
            let user_id = auth
                .get_user_by_username(&req.username)
                .map(|u| u.user_id.clone())
                .unwrap_or_default();
            Ok(Json(LoginResponse {
                token: String::new(),
                user_id,
                username: req.username,
                role: "pending_2fa".to_string(),
                requires_2fa: true,
            }))
        }
        Err(e) => Err(auth_error_response(e, "")),
    }
}

/// POST /auth/verify-2fa
///
/// Verifies a 2FA code and returns a JWT token (§11.200).
pub async fn verify_2fa(
    State(state): State<AppState>,
    Json(req): Json<Verify2faRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut auth = state.gxp_auth.lock().await;
    let jwt_config = &state.jwt_config;

    match auth.verify_two_factor(&req.user_id, &req.totp_code) {
        Ok(_) => {
            let user = auth
                .get_user(&req.user_id)
                .ok_or_else(|| auth_error_response(AuthError::UserNotFound, ""))?
                .clone();

            let role = map_roles(&user.roles);
            let claims = create_claims(&req.user_id, role, jwt_config);
            let token = generate_token(&claims, &jwt_config.secret)
                .map_err(|e| auth_error_response(AuthError::InvalidCredentials, &e.to_string()))?;

            Ok(Json(LoginResponse {
                token,
                user_id: req.user_id,
                username: user.username,
                role: role.to_string(),
                requires_2fa: false,
            }))
        }
        Err(e) => Err(auth_error_response(e, "")),
    }
}

/// POST /auth/change-password
///
/// Changes password with old-password verification (§11.300).
pub async fn change_password(
    State(state): State<AppState>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mut auth = state.gxp_auth.lock().await;

    match auth.change_password(&req.user_id, &req.old_password, &req.new_password) {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(auth_error_response(e, "")),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Map GxP roles to API server roles.
fn map_roles(gxp_roles: &[String]) -> Role {
    if gxp_roles.iter().any(|r| r == "admin" || r == "Admin") {
        Role::Admin
    } else if gxp_roles.iter().any(|r| r == "operator" || r == "Operator") {
        Role::Operator
    } else {
        Role::Viewer
    }
}

fn auth_error_response(err: AuthError, detail: &str) -> (StatusCode, Json<ErrorResponse>) {
    let (status, code) = match &err {
        AuthError::AccountLocked { .. } => (StatusCode::LOCKED, "ACCOUNT_LOCKED"),
        AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS"),
        AuthError::PasswordExpired => (StatusCode::FORBIDDEN, "PASSWORD_EXPIRED"),
        AuthError::PasswordTooWeak { .. } => (StatusCode::BAD_REQUEST, "PASSWORD_TOO_WEAK"),
        AuthError::PasswordReused => (StatusCode::BAD_REQUEST, "PASSWORD_REUSED"),
        AuthError::TwoFactorRequired => (StatusCode::UNAUTHORIZED, "2FA_REQUIRED"),
        AuthError::TwoFactorInvalid => (StatusCode::UNAUTHORIZED, "2FA_INVALID"),
        AuthError::SessionExpired => (StatusCode::UNAUTHORIZED, "SESSION_EXPIRED"),
        AuthError::UserNotFound => (StatusCode::NOT_FOUND, "USER_NOT_FOUND"),
        AuthError::UserAlreadyExists => (StatusCode::CONFLICT, "USER_ALREADY_EXISTS"),
        AuthError::RoleNotFound => (StatusCode::NOT_FOUND, "ROLE_NOT_FOUND"),
    };

    let msg = if detail.is_empty() {
        err.to_string()
    } else {
        format!("{err}: {detail}")
    };

    (
        status,
        Json(ErrorResponse {
            error: msg,
            code: code.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_roles_admin() {
        assert_eq!(map_roles(&["admin".to_string()]), Role::Admin);
    }

    #[test]
    fn test_map_roles_operator() {
        assert_eq!(map_roles(&["operator".to_string()]), Role::Operator);
    }

    #[test]
    fn test_map_roles_viewer() {
        assert_eq!(map_roles(&["viewer".to_string()]), Role::Viewer);
    }

    #[test]
    fn test_map_roles_admin_priority() {
        assert_eq!(
            map_roles(&["operator".to_string(), "admin".to_string()]),
            Role::Admin
        );
    }

    #[test]
    fn test_map_roles_empty_defaults_to_viewer() {
        assert_eq!(map_roles(&[]), Role::Viewer);
    }

    #[test]
    fn test_map_roles_unknown_role_defaults_to_viewer() {
        assert_eq!(map_roles(&["unknown".to_string()]), Role::Viewer);
    }

    #[test]
    fn test_map_roles_case_sensitive_admin() {
        // "Admin" matches because of the || check for "Admin"
        assert_eq!(map_roles(&["Admin".to_string()]), Role::Admin);
    }

    #[test]
    fn test_map_roles_case_sensitive_operator() {
        assert_eq!(map_roles(&["Operator".to_string()]), Role::Operator);
    }

    #[test]
    fn test_auth_error_invalid_credentials() {
        let (status, body) = auth_error_response(AuthError::InvalidCredentials, "");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "INVALID_CREDENTIALS");
    }

    #[test]
    fn test_auth_error_user_not_found() {
        let (status, body) = auth_error_response(AuthError::UserNotFound, "");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.code, "USER_NOT_FOUND");
    }

    #[test]
    fn test_auth_error_two_factor_invalid() {
        let (status, body) = auth_error_response(AuthError::TwoFactorInvalid, "");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "2FA_INVALID");
    }

    #[test]
    fn test_auth_error_session_expired() {
        let (status, body) = auth_error_response(AuthError::SessionExpired, "");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "SESSION_EXPIRED");
    }

    #[test]
    fn test_auth_error_password_reused() {
        let (status, body) = auth_error_response(AuthError::PasswordReused, "");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, "PASSWORD_REUSED");
    }

    #[test]
    fn test_auth_error_with_detail() {
        let (status, body) = auth_error_response(AuthError::InvalidCredentials, "extra info");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.error.contains("extra info"));
    }

    #[test]
    fn test_create_gxp_auth_state() {
        let state = create_gxp_auth_state(PasswordPolicy::default());
        // Should be a valid Arc<Mutex<GxpAuthManager>>
        assert!(Arc::strong_count(&state) >= 1);
    }
}
