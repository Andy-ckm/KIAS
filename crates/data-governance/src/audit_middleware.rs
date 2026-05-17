//! # Data Access Audit Middleware
//!
//! Axum middleware that automatically captures data-mutating operations
//! (POST, PUT, PATCH, DELETE) and logs them via the [`AuditLogger`] trait.
//!
//! This middleware sits between the auth layer and the handler layer,
//! intercepting requests to create an audit trail without requiring
//! manual instrumentation in every handler.

use axum::extract::{ConnectInfo, Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::debug;

use kias_common::audit::{AuditAction, AuditEvent, AuditLogger, AuditOutcome};

/// Shared state for the audit middleware.
#[derive(Clone)]
pub struct AuditMiddlewareState {
    /// The audit logger to write events to.
    pub logger: Arc<dyn AuditLogger>,
    /// Whether to audit GET/HEAD/OPTIONS requests (read operations).
    /// Default: false (only audit mutations).
    pub audit_reads: bool,
}

impl AuditMiddlewareState {
    pub fn new(logger: Arc<dyn AuditLogger>) -> Self {
        Self {
            logger,
            audit_reads: false,
        }
    }

    pub fn with_reads(mut self, audit_reads: bool) -> Self {
        self.audit_reads = audit_reads;
        self
    }
}

/// Map an HTTP method to an [`AuditAction`].
fn method_to_action(method: &Method) -> Option<AuditAction> {
    match *method {
        Method::POST => Some(AuditAction::Create),
        Method::PUT | Method::PATCH => Some(AuditAction::Update),
        Method::DELETE => Some(AuditAction::Delete),
        Method::GET | Method::HEAD | Method::OPTIONS => Some(AuditAction::Read),
        _ => None,
    }
}

/// Infer the resource type from the request URI path.
///
/// Extracts the first path segment after `/api/v1/` (e.g. `/api/v1/agents` → "agent").
fn infer_resource_type(uri_path: &str) -> String {
    // Strip /api/v1/ prefix
    let path = uri_path
        .strip_prefix("/api/v1/")
        .unwrap_or(uri_path)
        .trim_start_matches('/')
        .trim_end_matches('/');

    // Take the first segment
    let segment = path.split('/').next().unwrap_or("unknown");
    if segment.is_empty() {
        return "unknown".to_string();
    }

    // Singularize common plurals
    match segment {
        "agents" => "agent".to_string(),
        "nodes" => "node".to_string(),
        "tasks" => "task".to_string(),
        "workflows" => "workflow".to_string(),
        "configs" => "config".to_string(),
        "knowledge" => "knowledge".to_string(),
        "skills" => "skill".to_string(),
        "datasources" => "datasource".to_string(),
        "policies" => "policy".to_string(),
        other => other.to_string(),
    }
}

/// Extract the resource ID from the URI (second segment, if present).
fn infer_resource_id(uri_path: &str) -> String {
    let path = uri_path
        .strip_prefix("/api/v1/")
        .unwrap_or(uri_path)
        .trim_end_matches('/');

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 && !parts[1].is_empty() {
        parts[1].to_string()
    } else {
        "*".to_string()
    }
}

/// Audit middleware that captures data-mutating operations.
///
/// Automatically creates [`AuditEvent`]s for POST/PUT/PATCH/DELETE requests
/// and persists them via the configured [`AuditLogger`].
pub async fn audit_middleware(
    State(state): State<AuditMiddlewareState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().path().to_string();

    // Determine if we should audit this request
    let action = method_to_action(&method);
    let should_audit = match &action {
        Some(AuditAction::Read) => state.audit_reads,
        Some(_) => true,
        None => false,
    };

    if !should_audit {
        return next.run(request).await;
    }

    let action = action.unwrap();
    let resource_type = infer_resource_type(&uri);
    let resource_id = infer_resource_id(&uri);
    let ip = addr.ip().to_string();

    // Extract user agent if present
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Execute the request
    let response = next.run(request).await;
    let status = response.status();

    let outcome = if status.is_success() {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    };

    let event = AuditEvent::new(
        "system", // Will be replaced by auth middleware if Claims are available
        action,
        &resource_type,
        &resource_id,
        outcome,
    )
    .with_details(format!("{method} {uri} → {status}"))
    .with_ip(&ip);

    let event = if let Some(ua) = user_agent {
        event.with_user_agent(&ua)
    } else {
        event
    };

    state.logger.log_event(event).await;
    debug!(
        method = %method,
        uri = %uri,
        status = %status,
        resource = %resource_type,
        "Audit event captured"
    );

    response
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::audit::MemoryAuditLog;

    #[test]
    fn test_method_to_action() {
        assert_eq!(method_to_action(&Method::POST), Some(AuditAction::Create));
        assert_eq!(method_to_action(&Method::PUT), Some(AuditAction::Update));
        assert_eq!(method_to_action(&Method::PATCH), Some(AuditAction::Update));
        assert_eq!(method_to_action(&Method::DELETE), Some(AuditAction::Delete));
        assert_eq!(method_to_action(&Method::GET), Some(AuditAction::Read));
    }

    #[test]
    fn test_infer_resource_type() {
        assert_eq!(infer_resource_type("/api/v1/agents"), "agent");
        assert_eq!(infer_resource_type("/api/v1/agents/123"), "agent");
        assert_eq!(infer_resource_type("/api/v1/nodes"), "node");
        assert_eq!(infer_resource_type("/api/v1/workflows/abc"), "workflow");
        assert_eq!(infer_resource_type("/api/v1/datasources"), "datasource");
        assert_eq!(infer_resource_type("/api/v1/policies"), "policy");
        assert_eq!(infer_resource_type("/other/path"), "other");
    }

    #[test]
    fn test_infer_resource_id() {
        assert_eq!(infer_resource_id("/api/v1/agents"), "*");
        assert_eq!(infer_resource_id("/api/v1/agents/123"), "123");
        assert_eq!(infer_resource_id("/api/v1/workflows/abc/status"), "abc");
    }

    #[tokio::test]
    async fn test_audit_middleware_state_new() {
        let log: Arc<dyn AuditLogger> = Arc::new(MemoryAuditLog::new());
        let state = AuditMiddlewareState::new(log);
        assert!(!state.audit_reads);
    }

    #[tokio::test]
    async fn test_audit_middleware_state_with_reads() {
        let log: Arc<dyn AuditLogger> = Arc::new(MemoryAuditLog::new());
        let state = AuditMiddlewareState::new(log).with_reads(true);
        assert!(state.audit_reads);
    }
}
