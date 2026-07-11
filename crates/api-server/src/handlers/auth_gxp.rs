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
            let user = auth.get_user(&session.user_id).ok_or_else(|| {
                auth_error_response(
                    AuthError::InvalidCredentials,
                    "User not found after authentication",
                )
            })?;
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
        AuthError::PasswordHashingFailed => {
            (StatusCode::INTERNAL_SERVER_ERROR, "PASSWORD_HASHING_FAILED")
        }
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

    // === Handler-level tests ===

    use axum::extract::State;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// Build AppState with a GxpAuthManager that has test users pre-created.
    /// Users: "admin" (Admin), "operator1" (Operator), "viewer1" (Viewer)
    /// All passwords: "Test1234!@#$"
    async fn auth_test_state() -> AppState {
        let policy = PasswordPolicy::default();
        let mut auth = GxpAuthManager::new(policy);
        auth.create_user(
            "admin",
            "Admin User",
            "admin@test.com",
            "Test1234!@#$",
            vec!["admin".to_string()],
        )
        .unwrap();
        auth.create_user(
            "operator1",
            "Operator User",
            "op@test.com",
            "Test1234!@#$",
            vec!["operator".to_string()],
        )
        .unwrap();
        auth.create_user(
            "viewer1",
            "Viewer User",
            "viewer@test.com",
            "Test1234!@#$",
            vec!["viewer".to_string()],
        )
        .unwrap();

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            idempotency_store: None,
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: crate::handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: Arc::new(Mutex::new(auth)),
            jwt_config: crate::auth::JwtConfig::new("test-jwt-secret-key", "kias", 24),
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    #[tokio::test]
    async fn test_handler_login_success_admin() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "admin".to_string(),
            password: "Test1234!@#$".to_string(),
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert!(!resp.token.is_empty());
        assert_eq!(resp.username, "admin");
        assert_eq!(resp.role, "Admin");
        assert!(!resp.requires_2fa);
    }

    #[tokio::test]
    async fn test_handler_login_success_operator() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "operator1".to_string(),
            password: "Test1234!@#$".to_string(),
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert_eq!(resp.role, "Operator");
        assert!(!resp.requires_2fa);
    }

    #[tokio::test]
    async fn test_handler_login_invalid_password() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "admin".to_string(),
            password: "WrongPassword1!".to_string(),
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "INVALID_CREDENTIALS");
    }

    #[tokio::test]
    async fn test_handler_login_user_not_found() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "nonexistent".to_string(),
            password: "Test1234!@#$".to_string(),
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_handler_login_lockout_after_max_attempts() {
        let state = auth_test_state().await;
        // Fail 5 times (default lockout_attempts)
        for _ in 0..5 {
            let req = LoginRequest {
                username: "viewer1".to_string(),
                password: "WrongPassword1!".to_string(),
                totp_code: None,
            };
            let _ = login(State(state.clone()), Json(req)).await;
        }
        // 6th attempt should be locked out
        let req = LoginRequest {
            username: "viewer1".to_string(),
            password: "Test1234!@#$".to_string(), // even correct password
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::LOCKED);
        assert_eq!(body.code, "ACCOUNT_LOCKED");
    }

    #[tokio::test]
    async fn test_handler_change_password_success() {
        let state = auth_test_state().await;
        let req = ChangePasswordRequest {
            user_id: {
                // Get the actual user_id for "viewer1"
                let auth = state.gxp_auth.lock().await;
                auth.get_user_by_username("viewer1")
                    .unwrap()
                    .user_id
                    .clone()
            },
            old_password: "Test1234!@#$".to_string(),
            new_password: "NewPass5678!@#$".to_string(),
        };
        let result = change_password(State(state.clone()), Json(req)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StatusCode::NO_CONTENT);

        // Verify new password works
        let login_req = LoginRequest {
            username: "viewer1".to_string(),
            password: "NewPass5678!@#$".to_string(),
            totp_code: None,
        };
        let login_result = login(State(state), Json(login_req)).await;
        assert!(login_result.is_ok());
    }

    #[tokio::test]
    async fn test_handler_change_password_wrong_old_password() {
        let state = auth_test_state().await;
        let req = ChangePasswordRequest {
            user_id: {
                let auth = state.gxp_auth.lock().await;
                auth.get_user_by_username("operator1")
                    .unwrap()
                    .user_id
                    .clone()
            },
            old_password: "WrongOldPass1!".to_string(),
            new_password: "NewPass5678!@#$".to_string(),
        };
        let result = change_password(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "INVALID_CREDENTIALS");
    }

    #[tokio::test]
    async fn test_handler_change_password_weak_new_password() {
        let state = auth_test_state().await;
        let req = ChangePasswordRequest {
            user_id: {
                let auth = state.gxp_auth.lock().await;
                auth.get_user_by_username("admin").unwrap().user_id.clone()
            },
            old_password: "Test1234!@#$".to_string(),
            new_password: "weak".to_string(), // too short, no uppercase, etc.
        };
        let result = change_password(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, "PASSWORD_TOO_WEAK");
    }

    #[tokio::test]
    async fn test_handler_change_password_user_not_found() {
        let state = auth_test_state().await;
        let req = ChangePasswordRequest {
            user_id: "nonexistent-user-id".to_string(),
            old_password: "Test1234!@#$".to_string(),
            new_password: "NewPass5678!@#$".to_string(),
        };
        let result = change_password(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        // User not found or invalid credentials
        assert!(status == StatusCode::NOT_FOUND || status == StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_handler_verify_2fa_invalid_code() {
        let state = auth_test_state().await;
        let user_id = {
            let auth = state.gxp_auth.lock().await;
            auth.get_user_by_username("admin").unwrap().user_id.clone()
        };
        let req = Verify2faRequest {
            user_id,
            totp_code: "000000".to_string(), // invalid code
        };
        let result = verify_2fa(State(state), Json(req)).await;
        assert!(result.is_err());
        // Should fail — user doesn't have 2FA enabled, or code is invalid
    }

    #[tokio::test]
    async fn test_handler_verify_2fa_user_not_found() {
        let state = auth_test_state().await;
        let req = Verify2faRequest {
            user_id: "nonexistent-user-id".to_string(),
            totp_code: "123456".to_string(),
        };
        let result = verify_2fa(State(state), Json(req)).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert!(status == StatusCode::NOT_FOUND || status == StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_handler_login_response_has_user_id() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "admin".to_string(),
            password: "Test1234!@#$".to_string(),
            totp_code: None,
        };
        let result = login(State(state.clone()), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        // user_id should match the one in the auth manager
        let auth = state.gxp_auth.lock().await;
        let expected_id = &auth.get_user_by_username("admin").unwrap().user_id;
        assert_eq!(&resp.user_id, expected_id);
    }

    #[tokio::test]
    async fn test_handler_login_viewer_role() {
        let state = auth_test_state().await;
        let req = LoginRequest {
            username: "viewer1".to_string(),
            password: "Test1234!@#$".to_string(),
            totp_code: None,
        };
        let result = login(State(state), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert_eq!(resp.role, "Viewer");
        assert!(!resp.requires_2fa);
        assert!(!resp.token.is_empty());
    }
}
