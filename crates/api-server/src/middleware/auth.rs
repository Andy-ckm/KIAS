use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use kias_common::audit::{AuditAction, AuditEvent, AuditOutcome};

use crate::auth::{validate_token, Claims, Role};
use crate::AppState;

/// Authentication middleware.
///
/// Tries **JWT first**, then falls back to static API-key validation:
///
/// 1. If the `Authorization: Bearer <token>` is a valid JWT signed with the
///    configured secret, the decoded [`Claims`] are attached to the request
///    extensions so downstream handlers can access them.
///
/// 2. Otherwise, if the token matches one of the configured API keys, the
///    request is allowed and synthetic Admin [`Claims`] are attached.
///
/// 3. If neither check passes, `401 Unauthorized` is returned.
pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth if not enabled
    if !state.config.api_server.auth_enabled {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            tracing::warn!("Missing or malformed Authorization header");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // ── Try JWT first ────────────────────────────────────────────────
    if let Some(jwt_secret) = state.config.api_server.jwt_secret.as_deref() {
        if let Ok(claims) = validate_token(token, jwt_secret) {
            tracing::debug!(sub = %claims.sub, role = %claims.role, "JWT auth succeeded");
            let audit_event = AuditEvent::new(
                &claims.sub,
                AuditAction::Login,
                "auth",
                "jwt",
                AuditOutcome::Success,
            )
            .with_details("JWT validation succeeded");
            tracing::debug!(audit = ?audit_event, "audit: login success");
            request.extensions_mut().insert(claims);
            return Ok(next.run(request).await);
        }
    }

    // ── Fall back to API key ─────────────────────────────────────────
    if state.config.api_server.auth_tokens.is_empty() {
        tracing::warn!("Auth enabled but no API keys configured — denying all");
        let audit_event = AuditEvent::new(
            "unknown",
            AuditAction::Login,
            "auth",
            "api-key",
            AuditOutcome::Failure,
        )
        .with_details("No API keys configured, denying all");
        tracing::debug!(audit = ?audit_event, "audit: login failure (no keys)");
        return Err(StatusCode::UNAUTHORIZED);
    }

    if state.config.api_server.auth_tokens.iter().any(|k| k == token) {
        tracing::debug!("API-key auth succeeded");
        // Attach a synthetic Admin claim so downstream handlers can use RBAC
        // even when authenticating via static API key.
        let claims = Claims {
            sub: "api-key-user".to_string(),
            role: Role::Admin,
            iat: 0,
            exp: u64::MAX,
            iss: "kias-api-key".to_string(),
        };
        request.extensions_mut().insert(claims);
        return Ok(next.run(request).await);
    }

    tracing::warn!("Invalid credentials");
    let audit_event = AuditEvent::new(
        "unknown",
        AuditAction::Login,
        "auth",
        "jwt-or-apikey",
        AuditOutcome::Failure,
    )
    .with_details("Invalid credentials provided");
    tracing::debug!(audit = ?audit_event, "audit: login failure");
    Err(StatusCode::UNAUTHORIZED)
}

/// Middleware that enforces a minimum role for the request.
///
/// Must be placed **after** `auth_middleware` so that [`Claims`] are in
/// the request extensions.
pub async fn require_role_middleware(
    axum::extract::State(min_role): axum::extract::State<Role>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if role_rank(claims.role) >= role_rank(min_role) {
        Ok(next.run(request).await)
    } else {
        tracing::warn!(
            user_role = %claims.role,
            required = %min_role,
            "Insufficient role"
        );
        Err(StatusCode::FORBIDDEN)
    }
}

/// Numeric rank for role comparison. Higher = more privileges.
fn role_rank(role: Role) -> u8 {
    match role {
        Role::Admin => 3,
        Role::Operator => 2,
        Role::Viewer => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    use crate::auth::{create_claims, generate_token, JwtConfig, Role};
    use crate::AppState;

    async fn test_state_auth_disabled() -> AppState {
        AppState::new_async(kias_common::config::KiasConfig::default()).await
    }

    async fn test_state_auth_enabled() -> AppState {
        let mut config = kias_common::config::KiasConfig::default();
        config.api_server.auth_enabled = true;
        config.api_server.jwt_secret = Some("test-jwt-secret".to_string());
        config.api_server.auth_tokens = vec!["test-api-key-123".to_string()];
        AppState::new_async(config).await
    }

    async fn test_state_auth_no_keys() -> AppState {
        let mut config = kias_common::config::KiasConfig::default();
        config.api_server.auth_enabled = true;
        config.api_server.jwt_secret = Some("test-jwt-secret".to_string());
        config.api_server.auth_tokens = vec![];
        AppState::new_async(config).await
    }

    fn create_auth_router(state: AppState) -> Router {
        Router::new()
            .route("/protected", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    fn create_role_router(state: AppState, min_role: Role) -> Router {
        Router::new()
            .route("/role-protected", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                min_role,
                require_role_middleware,
            ))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    fn make_jwt(secret: &str, role: Role) -> String {
        let config = JwtConfig::new(secret, "test", 24);
        let claims = create_claims("test-user", role, &config);
        generate_token(&claims, secret).unwrap()
    }

    // ── role_rank tests ──────────────────────────────────────────────

    #[test]
    fn test_role_rank_ordering() {
        assert!(role_rank(Role::Admin) > role_rank(Role::Operator));
        assert!(role_rank(Role::Operator) > role_rank(Role::Viewer));
    }

    #[test]
    fn test_role_rank_values() {
        assert_eq!(role_rank(Role::Admin), 3);
        assert_eq!(role_rank(Role::Operator), 2);
        assert_eq!(role_rank(Role::Viewer), 1);
    }

    // ── auth_middleware tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_auth_disabled_passes_through() {
        let app = create_auth_router(test_state_auth_disabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_enabled_no_header_returns_401() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_malformed_header_returns_401() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, "Basic abc123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_empty_bearer_returns_401() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, "Bearer ")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_valid_jwt_passes() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let token = make_jwt("test-jwt-secret", Role::Admin);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_invalid_jwt_valid_api_key_passes() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, "Bearer test-api-key-123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_invalid_jwt_no_keys_returns_401() {
        let app = create_auth_router(test_state_auth_no_keys().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, "Bearer some-invalid-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_invalid_jwt_wrong_api_key_returns_401() {
        let app = create_auth_router(test_state_auth_enabled().await);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, "Bearer wrong-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_jwt_wrong_secret_falls_to_api_key() {
        // JWT signed with wrong secret should fail, then API key should also fail
        let app = create_auth_router(test_state_auth_enabled().await);
        let token = make_jwt("wrong-secret", Role::Admin);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // JWT fails, token is not a valid API key either → 401
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── require_role_middleware tests ─────────────────────────────────

    #[tokio::test]
    async fn test_require_role_admin_passes_for_admin() {
        let state = test_state_auth_enabled().await;
        let app = create_role_router(state, Role::Admin);
        let token = make_jwt("test-jwt-secret", Role::Admin);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/role-protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_role_admin_rejects_viewer() {
        let state = test_state_auth_enabled().await;
        let app = create_role_router(state, Role::Admin);
        let token = make_jwt("test-jwt-secret", Role::Viewer);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/role-protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_require_role_viewer_passes_for_admin() {
        let state = test_state_auth_enabled().await;
        let app = create_role_router(state, Role::Viewer);
        let token = make_jwt("test-jwt-secret", Role::Admin);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/role-protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_role_operator_passes_for_operator() {
        let state = test_state_auth_enabled().await;
        let app = create_role_router(state, Role::Operator);
        let token = make_jwt("test-jwt-secret", Role::Operator);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/role-protected")
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_role_no_claims_returns_401() {
        let state = test_state_auth_disabled().await;
        // Role middleware without auth middleware → no Claims in extensions
        let app = Router::new()
            .route("/role-protected", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                Role::Viewer,
                require_role_middleware,
            ))
            .with_state(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/role-protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
