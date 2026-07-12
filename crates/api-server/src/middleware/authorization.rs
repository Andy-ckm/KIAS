use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::auth::{Claims, Role};

/// Enforce coarse control-plane role boundaries after authentication.
///
/// - Viewer: authenticated read-only access.
/// - Operator: state-changing control-plane actions.
/// - Admin: configuration and configuration-audit access.
///
/// Object- and tenant-level authorization remains a separate pre-1.0 release
/// blocker; this middleware prevents the current role model from collapsing
/// into "every authenticated identity can mutate everything".
pub async fn control_plane_authorization(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let required = required_role(request.method(), request.uri().path());

    if role_rank(claims.role) < role_rank(required) {
        tracing::warn!(
            subject = %kias_common::audit::pseudonymize_identifier(&claims.sub),
            actual_role = %claims.role,
            required_role = %required,
            method = %request.method(),
            path = request.uri().path(),
            "Control-plane authorization denied"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}

fn required_role(method: &Method, path: &str) -> Role {
    if path == "/api/v1/config" || path.starts_with("/api/v1/config/") {
        Role::Admin
    } else if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        Role::Viewer
    } else {
        Role::Operator
    }
}

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
    fn read_requests_require_viewer() {
        assert_eq!(required_role(&Method::GET, "/api/v1/agents"), Role::Viewer);
        assert_eq!(required_role(&Method::HEAD, "/api/v1/agents"), Role::Viewer);
    }

    #[test]
    fn mutations_require_operator() {
        assert_eq!(
            required_role(&Method::POST, "/api/v1/agents"),
            Role::Operator
        );
        assert_eq!(
            required_role(&Method::DELETE, "/api/v1/workflows/example"),
            Role::Operator
        );
    }

    #[test]
    fn configuration_requires_admin_even_for_reads() {
        assert_eq!(required_role(&Method::GET, "/api/v1/config"), Role::Admin);
        assert_eq!(
            required_role(&Method::GET, "/api/v1/config/audit-log"),
            Role::Admin
        );
    }
}
