//! # Data Governance API Handlers
//!
//! REST endpoints for managing data sources, access policies, and querying
//! the audit trail.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use kias_common::audit::AuditAction;
use kias_data_store::SqliteAuditLog;

use crate::datasource::DataSourceRegistry;
use crate::policy::{PolicyEngine, PolicyEffect, ResourcePolicy};

/// Shared state for data governance handlers.
#[derive(Clone)]
pub struct GovernanceState {
    pub registry: Arc<DataSourceRegistry>,
    pub policy_engine: Arc<PolicyEngine>,
    pub audit_log: Option<Arc<SqliteAuditLog>>,
}

// ── Data Source Handlers ─────────────────────────────────────────────

/// Response for a data source listing.
#[derive(Debug, Serialize)]
pub struct DataSourceInfo {
    pub name: String,
    pub status: String,
}

/// GET /api/v1/datasources
pub async fn list_datasources(
    State(state): State<GovernanceState>,
) -> impl IntoResponse {
    let names = state.registry.list_names().await;
    let mut sources = Vec::new();
    for name in names {
        if let Some(ds) = state.registry.get(&name).await {
            sources.push(DataSourceInfo {
                name: ds.name().to_string(),
                status: ds.status().to_string(),
            });
        }
    }
    Json(sources)
}

/// GET /api/v1/datasources/health
pub async fn datasources_health(
    State(state): State<GovernanceState>,
) -> impl IntoResponse {
    let results = state.registry.health_check_all().await;
    Json(results)
}

// ── Policy Handlers ──────────────────────────────────────────────────

/// Request body for creating/updating a policy.
#[derive(Debug, Deserialize)]
pub struct PolicyRequest {
    pub id: Option<String>,
    pub name: String,
    pub role: String,
    pub resource_type: String,
    pub action: String,
    pub effect: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// POST /api/v1/policies
pub async fn create_policy(
    State(state): State<GovernanceState>,
    Json(req): Json<PolicyRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate the action string
    if parse_audit_action(&req.action).is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let effect = parse_policy_effect(&req.effect).ok_or(StatusCode::BAD_REQUEST)?;

    let id = req
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let policy = ResourcePolicy {
        id: id.clone(),
        name: req.name,
        role: req.role,
        resource_type: req.resource_type,
        action: req.action,
        effect,
        description: req.description,
        enabled: req.enabled,
    };

    state.policy_engine.add_policy(policy).await;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// GET /api/v1/policies
pub async fn list_policies(
    State(state): State<GovernanceState>,
) -> impl IntoResponse {
    let policies = state.policy_engine.list_policies().await;
    Json(policies)
}

/// DELETE /api/v1/policies/:id
pub async fn delete_policy(
    State(state): State<GovernanceState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    if state.policy_engine.remove_policy(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// ── Audit Handlers ───────────────────────────────────────────────────

/// Query parameters for audit log queries.
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

/// GET /api/v1/audit
pub async fn query_audit(
    State(state): State<GovernanceState>,
    Query(params): Query<AuditQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let audit_log = state.audit_log.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let events = audit_log
        .query(
            params.actor.as_deref(),
            params.action.as_deref(),
            params.resource_type.as_deref(),
            params.limit,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert to serializable response
    let response: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "timestamp": e.timestamp.to_rfc3339(),
                "actor": e.actor,
                "action": e.action.to_string(),
                "resource_type": e.resource_type,
                "resource_id": e.resource_id,
                "details": e.details,
                "ip_address": e.ip_address,
                "user_agent": e.user_agent,
                "outcome": e.outcome.to_string(),
            })
        })
        .collect();

    Ok(Json(response))
}

/// GET /api/v1/audit/count
pub async fn audit_count(
    State(state): State<GovernanceState>,
) -> Result<impl IntoResponse, StatusCode> {
    let audit_log = state.audit_log.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let count = audit_log
        .count()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "count": count })))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn parse_audit_action(s: &str) -> Option<AuditAction> {
    match s.to_lowercase().as_str() {
        "create" => Some(AuditAction::Create),
        "read" => Some(AuditAction::Read),
        "update" => Some(AuditAction::Update),
        "delete" => Some(AuditAction::Delete),
        "login" => Some(AuditAction::Login),
        "logout" => Some(AuditAction::Logout),
        "schedule" => Some(AuditAction::Schedule),
        "execute" => Some(AuditAction::Execute),
        "configchange" | "config_change" => Some(AuditAction::ConfigChange),
        _ => None,
    }
}

fn parse_policy_effect(s: &str) -> Option<PolicyEffect> {
    match s.to_lowercase().as_str() {
        "allow" => Some(PolicyEffect::Allow),
        "deny" => Some(PolicyEffect::Deny),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_audit_action() {
        assert_eq!(parse_audit_action("create"), Some(AuditAction::Create));
        assert_eq!(parse_audit_action("READ"), Some(AuditAction::Read));
        assert_eq!(parse_audit_action("delete"), Some(AuditAction::Delete));
        assert_eq!(
            parse_audit_action("config_change"),
            Some(AuditAction::ConfigChange)
        );
        assert_eq!(parse_audit_action("invalid"), None);
    }

    #[test]
    fn test_parse_policy_effect() {
        assert_eq!(parse_policy_effect("allow"), Some(PolicyEffect::Allow));
        assert_eq!(parse_policy_effect("DENY"), Some(PolicyEffect::Deny));
        assert_eq!(parse_policy_effect("unknown"), None);
    }
}
