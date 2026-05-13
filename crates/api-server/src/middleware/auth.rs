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
    if state.config.api_server.api_keys.is_empty() {
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

    if state.config.api_server.api_keys.iter().any(|k| k == token) {
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
}
