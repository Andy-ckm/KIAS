//! # OpenAPI 3.1 Documentation with GxP Compliance Annotations
//!
//! This module provides auto-generated OpenAPI 3.1 documentation for the Kias API Server.
//! All endpoints include GxP compliance annotations for audit trail, electronic signature,
//! and data integrity requirements.
//!
//! ## GxP Compliance Annotations
//!
//! - **§11.100** — Audit Trail: All data changes must be logged with user, timestamp, and action
//! - **§11.200** — Electronic Signature: Critical operations require electronic signature verification
//! - **§11.300** — Access Control: Password policy and user management requirements
//! - **§11.400** — Data Integrity: System documentation and validation requirements

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// OpenAPI 3.1 Document Structure
// ─────────────────────────────────────────────────────────────────────────────

/// OpenAPI 3.1 document root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiDoc {
    pub openapi: String,
    pub info: InfoObject,
    pub servers: Vec<ServerObject>,
    pub paths: std::collections::HashMap<String, PathItem>,
    pub components: Components,
    pub security: Option<Vec<SecurityRequirement>>,
    pub tags: Option<Vec<TagObject>>,
}

/// Info object (metadata about the API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoObject {
    pub title: String,
    pub description: String,
    pub version: String,
    pub contact: Option<ContactObject>,
    pub license: Option<LicenseObject>,
}

/// Server object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerObject {
    pub url: String,
    pub description: Option<String>,
}

/// Path item object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub get: Option<Operation>,
    pub post: Option<Operation>,
    pub put: Option<Operation>,
    pub patch: Option<Operation>,
    pub delete: Option<Operation>,
}

/// Operation object (HTTP method)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub operation_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Option<Vec<Parameter>>,
    pub request_body: Option<RequestBody>,
    pub responses: std::collections::HashMap<String, ResponseObject>,
    pub security: Option<Vec<SecurityRequirement>>,
    pub x_gxp_annotations: Option<GxPAnnotations>,
}

/// Parameter object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    pub required: bool,
    pub schema: SchemaObject,
    pub description: Option<String>,
}

/// Request body object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    pub required: bool,
    pub content: std::collections::HashMap<String, MediaTypeObject>,
    pub description: Option<String>,
}

/// Media type object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTypeObject {
    pub schema: SchemaObject,
}

/// Response object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseObject {
    pub description: String,
    pub content: Option<std::collections::HashMap<String, MediaTypeObject>>,
}

/// Schema object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaObject {
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub format: Option<String>,
    pub properties: Option<std::collections::HashMap<String, SchemaObject>>,
    pub items: Option<Box<SchemaObject>>,
    pub required: Option<Vec<String>>,
    pub description: Option<String>,
    pub example: Option<Value>,
}

/// Components object (reusable schemas, security schemes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    pub schemas: Option<std::collections::HashMap<String, SchemaObject>>,
    pub security_schemes: Option<std::collections::HashMap<String, SecurityScheme>>,
}

/// Security scheme object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScheme {
    #[serde(rename = "type")]
    pub scheme_type: String,
    pub description: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "in")]
    pub location: Option<String>,
    pub scheme: Option<String>,
    pub bearer_format: Option<String>,
}

/// Security requirement object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRequirement {
    #[serde(flatten)]
    pub schemes: std::collections::HashMap<String, Vec<String>>,
}

/// Tag object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagObject {
    pub name: String,
    pub description: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// GxP Compliance Annotations
// ─────────────────────────────────────────────────────────────────────────────

/// GxP compliance annotations for OpenAPI extension field `x-gxp-annotations`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GxPAnnotations {
    /// §11.100 Audit Trail annotation
    pub audit_trail: Option<AuditTrailAnnotation>,
    /// §11.200 Electronic Signature annotation
    pub electronic_signature: Option<ElectronicSignatureAnnotation>,
    /// §11.300 Access Control annotation
    pub access_control: Option<AccessControlAnnotation>,
    /// §11.400 Data Integrity annotation
    pub data_integrity: Option<DataIntegrityAnnotation>,
}

/// Audit Trail annotation (§11.100)
/// Records who did what, when, and why for all data changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrailAnnotation {
    /// Whether this operation generates an audit log entry
    pub logged: bool,
    /// Audit event type
    pub event_type: String,
    /// Whether the old value is captured (for updates/deletes)
    pub captures_old_value: bool,
    /// Whether the new value is captured (for creates/updates)
    pub captures_new_value: bool,
    /// Retention period for audit logs
    pub retention_days: u32,
    /// List of PII fields that should be masked in logs
    pub pii_fields: Option<Vec<String>>,
}

/// Electronic Signature annotation (§11.200)
/// Critical operations require electronic signature verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectronicSignatureAnnotation {
    /// Whether this operation requires electronic signature
    pub required: bool,
    /// Meaning of the signature (e.g., "I attest", "I approve")
    pub meaning: String,
    /// Whether 2FA is enforced for this operation
    pub requires_2fa: bool,
    /// Whether this operation creates a non-repudiable record
    pub non_repudiable: bool,
    /// Operations that this signature applies to (e.g., ["create", "update"])
    pub applies_to: Vec<String>,
}

/// Access Control annotation (§11.300)
/// Password policy and user management requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlAnnotation {
    /// Minimum password length
    pub min_password_length: u32,
    /// Whether password rotation is required
    pub password_rotation_days: Option<u32>,
    /// Required roles to access this endpoint
    pub required_roles: Vec<String>,
    /// Whether API key authentication is allowed
    pub allows_api_key: bool,
    /// Session timeout in minutes
    pub session_timeout_minutes: u32,
}

/// Data Integrity annotation (§11.400)
/// System documentation and validation requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataIntegrityAnnotation {
    /// Whether input validation is performed
    pub input_validation: bool,
    /// Whether output sanitization is performed
    pub output_sanitization: bool,
    /// Whether this operation is idempotent
    pub idempotent: bool,
    /// Whether audit checksums are computed
    pub checksum_computed: bool,
    /// Validation rules applied
    pub validation_rules: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAPI Document Generation
// ─────────────────────────────────────────────────────────────────────────────

impl OpenApiDoc {
    /// Generate the complete OpenAPI 3.1 specification
    pub fn generate() -> Self {
        let mut paths = std::collections::HashMap::new();

        // Health endpoints (public, no auth)
        paths.insert(
            "/health".to_string(),
            PathItem {
                summary: Some("Liveness probe".to_string()),
                description: Some("Returns 200 if server is up".to_string()),
                get: Some(Operation {
                    operation_id: "liveness".to_string(),
                    summary: "Liveness check".to_string(),
                    description: Some("Public endpoint returning OK if server is running".to_string()),
                    tags: vec!["Health".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Server is alive"),
                    security: None,
                    x_gxp_annotations: Some(gxp_minimal()),
                }),
                post: None,
                put: None,
                patch: None,
                delete: None,
            },
        );

        paths.insert(
            "/readyz".to_string(),
            PathItem {
                summary: Some("Readiness probe".to_string()),
                description: Some("Checks internal state is usable".to_string()),
                get: Some(Operation {
                    operation_id: "readiness".to_string(),
                    summary: "Readiness check".to_string(),
                    description: Some("Verifies agents store and nodes store are accessible".to_string()),
                    tags: vec!["Health".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Service readiness status"),
                    security: None,
                    x_gxp_annotations: Some(gxp_minimal()),
                }),
                post: None,
                put: None,
                patch: None,
                delete: None,
            },
        );

        // Agent endpoints
        paths.insert(
            "/api/v1/agents".to_string(),
            PathItem {
                summary: Some("Agent management".to_string()),
                description: Some("List and create agents".to_string()),
                get: Some(Operation {
                    operation_id: "list_agents".to_string(),
                    summary: "List all agents".to_string(),
                    description: Some("Returns paginated list of agents".to_string()),
                    tags: vec!["Agents".to_string()],
                    parameters: Some(vec![
                        param_query("offset", "Offset for pagination", false),
                        param_query("limit", "Maximum results per page", false),
                    ]),
                    request_body: None,
                    responses: response_200_json("List of agents"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                post: Some(Operation {
                    operation_id: "create_agent".to_string(),
                    summary: "Create new agent".to_string(),
                    description: Some("Creates a new agent with specified configuration".to_string()),
                    tags: vec!["Agents".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["name", "image"], "Agent specification")),
                    responses: response_201_json("Created agent"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_full()),
                }),
                put: None,
                patch: None,
                delete: None,
            },
        );

        paths.insert(
            "/api/v1/agents/{id}".to_string(),
            PathItem {
                summary: Some("Single agent operations".to_string()),
                description: Some("Get or delete a specific agent".to_string()),
                get: Some(Operation {
                    operation_id: "get_agent".to_string(),
                    summary: "Get agent by ID".to_string(),
                    description: Some("Retrieves agent details".to_string()),
                    tags: vec!["Agents".to_string()],
                    parameters: Some(vec![param_path("id", "Agent ID", true)]),
                    request_body: None,
                    responses: response_200_json("Agent details"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                post: None,
                put: None,
                patch: None,
                delete: Some(Operation {
                    operation_id: "delete_agent".to_string(),
                    summary: "Delete agent".to_string(),
                    description: Some("Permanently removes an agent from the system".to_string()),
                    tags: vec!["Agents".to_string()],
                    parameters: Some(vec![param_path("id", "Agent ID", true)]),
                    request_body: None,
                    responses: response_200_json("Deletion confirmation"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_delete()),
                }),
            },
        );

        paths.insert(
            "/api/v1/agents/{id}/invoke".to_string(),
            PathItem {
                summary: Some("Agent invocation".to_string()),
                description: Some("Invoke an agent with a prompt".to_string()),
                post: Some(Operation {
                    operation_id: "invoke_agent".to_string(),
                    summary: "Invoke agent".to_string(),
                    description: Some("Sends a prompt to the agent and returns the response".to_string()),
                    tags: vec!["Agents".to_string()],
                    parameters: Some(vec![param_path("id", "Agent ID", true)]),
                    request_body: Some(request_body_json(vec!["prompt"], "Invocation request")),
                    responses: response_200_json("Agent response"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_execution()),
                }),
                ..Default::default()
            },
        );

        // Node endpoints
        paths.insert(
            "/api/v1/nodes".to_string(),
            PathItem {
                summary: Some("Node listing".to_string()),
                get: Some(Operation {
                    operation_id: "list_nodes".to_string(),
                    summary: "List all nodes".to_string(),
                    description: Some("Returns all registered nodes in the cluster".to_string()),
                    tags: vec!["Nodes".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("List of nodes"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                ..Default::default()
            },
        );

        paths.insert(
            "/api/v1/nodes/{id}".to_string(),
            PathItem {
                summary: Some("Node details".to_string()),
                get: Some(Operation {
                    operation_id: "get_node".to_string(),
                    summary: "Get node by ID".to_string(),
                    description: Some("Retrieves details of a specific node".to_string()),
                    tags: vec!["Nodes".to_string()],
                    parameters: Some(vec![param_path("id", "Node ID", true)]),
                    request_body: None,
                    responses: response_200_json("Node details"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                ..Default::default()
            },
        );

        // Workflow endpoints
        paths.insert(
            "/api/v1/workflows".to_string(),
            PathItem {
                summary: Some("Workflow management".to_string()),
                get: Some(Operation {
                    operation_id: "list_workflows".to_string(),
                    summary: "List workflows".to_string(),
                    description: Some("Returns all defined workflows".to_string()),
                    tags: vec!["Workflows".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("List of workflows"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                post: Some(Operation {
                    operation_id: "create_workflow".to_string(),
                    summary: "Create workflow".to_string(),
                    description: Some("Creates a new workflow definition".to_string()),
                    tags: vec!["Workflows".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["name", "steps"], "Workflow definition")),
                    responses: response_201_json("Created workflow"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_full()),
                }),
                ..Default::default()
            },
        );

        // Config endpoints
        paths.insert(
            "/api/v1/config".to_string(),
            PathItem {
                summary: Some("Configuration management".to_string()),
                get: Some(Operation {
                    operation_id: "get_config".to_string(),
                    summary: "Get configuration".to_string(),
                    description: Some("Returns sanitized configuration (secrets redacted)".to_string()),
                    tags: vec!["Config".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Configuration"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                patch: Some(Operation {
                    operation_id: "update_config".to_string(),
                    summary: "Update configuration".to_string(),
                    description: Some("Updates configuration (Admin only)".to_string()),
                    tags: vec!["Config".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["value"], "Configuration patch")),
                    responses: response_200_json("Updated configuration"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_admin()),
                }),
                ..Default::default()
            },
        );

        // Metrics endpoints
        paths.insert(
            "/api/v1/metrics/summary".to_string(),
            PathItem {
                summary: Some("System metrics".to_string()),
                get: Some(Operation {
                    operation_id: "metrics_summary".to_string(),
                    summary: "Get metrics summary".to_string(),
                    description: Some("Returns aggregated system metrics".to_string()),
                    tags: vec!["Metrics".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Metrics summary"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                ..Default::default()
            },
        );

        // Token analytics endpoints
        paths.insert(
            "/api/v1/tokens".to_string(),
            PathItem {
                summary: Some("Token usage analytics".to_string()),
                get: Some(Operation {
                    operation_id: "token_analytics".to_string(),
                    summary: "Get token usage".to_string(),
                    description: Some("Returns token consumption analytics".to_string()),
                    tags: vec!["Tokens".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Token analytics"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                ..Default::default()
            },
        );

        // Knowledge endpoints
        paths.insert(
            "/api/v1/knowledge/search".to_string(),
            PathItem {
                summary: Some("Knowledge base search".to_string()),
                get: Some(Operation {
                    operation_id: "search_knowledge".to_string(),
                    summary: "Search knowledge base".to_string(),
                    description: Some("Performs vector similarity search on the knowledge base".to_string()),
                    tags: vec!["Knowledge".to_string()],
                    parameters: Some(vec![param_query("q", "Search query", true)]),
                    request_body: None,
                    responses: response_200_json("Search results"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                ..Default::default()
            },
        );

        // A2A endpoints
        paths.insert(
            "/a2a/v1/agents".to_string(),
            PathItem {
                summary: Some("A2A agent discovery".to_string()),
                get: Some(Operation {
                    operation_id: "list_agent_cards".to_string(),
                    summary: "List A2A agent cards".to_string(),
                    description: Some("Returns all agent cards for A2A protocol discovery".to_string()),
                    tags: vec!["A2A".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Agent cards"),
                    security: None, // A2A well-known endpoint is public
                    x_gxp_annotations: Some(gxp_minimal()),
                }),
                ..Default::default()
            },
        );

        paths.insert(
            "/a2a/v1/tasks".to_string(),
            PathItem {
                summary: Some("A2A task management".to_string()),
                get: Some(Operation {
                    operation_id: "list_tasks".to_string(),
                    summary: "List A2A tasks".to_string(),
                    description: Some("Returns all A2A tasks".to_string()),
                    tags: vec!["A2A".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("Task list"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_audit_read()),
                }),
                post: Some(Operation {
                    operation_id: "send_task".to_string(),
                    summary: "Send A2A task".to_string(),
                    description: Some("Submits an async task to another agent".to_string()),
                    tags: vec!["A2A".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["agent_id", "prompt"], "Task payload")),
                    responses: response_202_json("Task submitted"),
                    security: Some(vec![bearer_auth()]),
                    x_gxp_annotations: Some(gxp_full()),
                }),
                ..Default::default()
            },
        );

        // WebSocket stats endpoint
        paths.insert(
            "/api/v1/ws/stats".to_string(),
            PathItem {
                summary: Some("WebSocket statistics".to_string()),
                get: Some(Operation {
                    operation_id: "ws_stats".to_string(),
                    summary: "Get WebSocket stats".to_string(),
                    description: Some("Returns WebSocket connection statistics".to_string()),
                    tags: vec!["WebSocket".to_string()],
                    parameters: None,
                    request_body: None,
                    responses: response_200_json("WebSocket statistics"),
                    security: None,
                    x_gxp_annotations: Some(gxp_minimal()),
                }),
                ..Default::default()
            },
        );

        // Auth endpoints (GxP)
        paths.insert(
            "/auth/login".to_string(),
            PathItem {
                summary: Some("GxP Authentication".to_string()),
                description: Some("§11.200 Electronic Signature & §11.300 Access Control".to_string()),
                post: Some(Operation {
                    operation_id: "login".to_string(),
                    summary: "User login".to_string(),
                    description: Some("Authenticates user with username/password. If 2FA is enabled, returns requires_2fa=true".to_string()),
                    tags: vec!["Auth".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["username", "password"], "Login credentials")),
                    responses: response_200_json("Login response with JWT"),
                    security: None,
                    x_gxp_annotations: Some(gxp_auth_login()),
                }),
                ..Default::default()
            },
        );

        paths.insert(
            "/auth/verify-2fa".to_string(),
            PathItem {
                summary: Some("Two-factor verification".to_string()),
                post: Some(Operation {
                    operation_id: "verify_2fa".to_string(),
                    summary: "Verify 2FA code".to_string(),
                    description: Some("Verifies TOTP code and returns final JWT".to_string()),
                    tags: vec!["Auth".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["user_id", "totp_code"], "2FA verification")),
                    responses: response_200_json("2FA verification result"),
                    security: None,
                    x_gxp_annotations: Some(gxp_2fa()),
                }),
                ..Default::default()
            },
        );

        paths.insert(
            "/auth/change-password".to_string(),
            PathItem {
                summary: Some("Password management".to_string()),
                description: Some("§11.300 Password rotation requirements".to_string()),
                post: Some(Operation {
                    operation_id: "change_password".to_string(),
                    summary: "Change password".to_string(),
                    description: Some("Changes user password with old password verification".to_string()),
                    tags: vec!["Auth".to_string()],
                    parameters: None,
                    request_body: Some(request_body_json(vec!["user_id", "old_password", "new_password"], "Password change")),
                    responses: response_200_json("Password changed successfully"),
                    security: None,
                    x_gxp_annotations: Some(gxp_password_change()),
                }),
                ..Default::default()
            },
        );

        // Build the document
        let mut doc = OpenApiDoc {
            openapi: "3.1.0".to_string(),
            info: InfoObject {
                title: "Kias API Server".to_string(),
                description: "AgentGuard AI Agent Compliance & Governance System\n\n## GxP Compliance\n\nThis API implements GxP (Good Practice) compliance requirements:\n\n- **§11.100** Audit Trail: All data changes are logged with user, timestamp, and action\n- **§11.200** Electronic Signature: Critical operations require electronic signature verification\n- **§11.300** Access Control: Password policy and user management requirements\n- **§11.400** Data Integrity: System documentation and validation requirements\n\nAll endpoints that modify data include `x-gxp-annotations` extension fields with compliance details."
                    .to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                contact: Some(ContactObject {
                    name: Some("KIAS Support".to_string()),
                    email: Some("support@kias.example.com".to_string()),
                    url: Some("https://kias.example.com".to_string()),
                }),
                license: Some(LicenseObject {
                    name: "MIT".to_string(),
                    url: Some("https://opensource.org/licenses/MIT".to_string()),
                }),
            },
            servers: vec![ServerObject {
                url: "http://localhost:8080".to_string(),
                description: Some("Local development server".to_string()),
            }],
            paths,
            components: Components {
                schemas: Some(serde_json::json!({
                    "Agent": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "format": "uuid"},
                            "name": {"type": "string"},
                            "status": {"type": "string", "enum": ["ready", "running", "stopped", "error"]},
                            "created_at": {"type": "string", "format": "date-time"}
                        },
                        "required": ["id", "name", "status"]
                    },
                    "Workflow": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "name": {"type": "string"},
                            "status": {"type": "string"}
                        },
                        "required": ["id", "name"]
                    },
                    "Error": {
                        "type": "object",
                        "properties": {
                            "error": {"type": "string"},
                            "code": {"type": "string"}
                        }
                    }
                }).as_object().unwrap().clone().into_iter().collect()),
                security_schemes: Some(serde_json::json!({
                    "BearerAuth": {
                        "type": "http",
                        "scheme": "bearer",
                        "bearerFormat": "JWT",
                        "description": "JWT token obtained from /auth/login"
                    },
                    "ApiKeyAuth": {
                        "type": "apiKey",
                        "in": "header",
                        "name": "X-API-Key",
                        "description": "API key for service-to-service communication"
                    }
                }).as_object().unwrap().clone().into_iter().collect()),
            },
            security: Some(vec![bearer_auth()]),
            tags: Some(vec![
                TagObject { name: "Health".to_string(), description: Some("Health check endpoints".to_string()) },
                TagObject { name: "Agents".to_string(), description: Some("Agent lifecycle management".to_string()) },
                TagObject { name: "Nodes".to_string(), description: Some("Cluster node management".to_string()) },
                TagObject { name: "Workflows".to_string(), description: Some("Workflow definitions and execution".to_string()) },
                TagObject { name: "Config".to_string(), description: Some("System configuration".to_string()) },
                TagObject { name: "Metrics".to_string(), description: Some("System metrics and monitoring".to_string()) },
                TagObject { name: "Tokens".to_string(), description: Some("Token usage analytics".to_string()) },
                TagObject { name: "Knowledge".to_string(), description: Some("Knowledge base operations".to_string()) },
                TagObject { name: "A2A".to_string(), description: Some("Agent-to-Agent protocol".to_string()) },
                TagObject { name: "WebSocket".to_string(), description: Some("WebSocket management".to_string()) },
                TagObject { name: "Auth".to_string(), description: Some("GxP Authentication & Access Control".to_string()) },
            ]),
        };

        // Ensure all paths have x_gxp_annotations
        for path_item in doc.paths.values_mut() {
            if let Some(ref mut op) = path_item.get {
                if op.x_gxp_annotations.is_none() {
                    op.x_gxp_annotations = Some(gxp_audit_read());
                }
            }
            if let Some(ref mut op) = path_item.post {
                if op.x_gxp_annotations.is_none() {
                    op.x_gxp_annotations = Some(gxp_full());
                }
            }
        }

        doc
    }

    /// Get the JSON representation of this OpenAPI document
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("OpenAPI doc should be serializable")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

impl Default for PathItem {
    fn default() -> Self {
        PathItem {
            summary: None,
            description: None,
            get: None,
            post: None,
            put: None,
            patch: None,
            delete: None,
        }
    }
}

fn response_200_json(description: &str) -> Option<Vec<ResponseObject>> {
    Some(vec![ResponseObject {
        description: description.to_string(),
        content: Some(serde_json::json!({
            "application/json": {
                "schema": {"type": "object"}
            }
        }).as_object().unwrap().clone().into_iter().collect()),
    }])
}

fn response_201_json(description: &str) -> Option<Vec<ResponseObject>> {
    Some(vec![ResponseObject {
        description: description.to_string(),
        content: Some(serde_json::json!({
            "application/json": {
                "schema": {"type": "object"}
            }
        }).as_object().unwrap().clone().into_iter().collect()),
    }])
}

fn response_202_json(description: &str) -> Option<Vec<ResponseObject>> {
    Some(vec![ResponseObject {
        description: description.to_string(),
        content: Some(serde_json::json!({
            "application/json": {
                "schema": {"type": "object"}
            }
        }).as_object().unwrap().clone().into_iter().collect()),
    }])
}

fn param_query(name: &str, description: &str, required: bool) -> Parameter {
    Parameter {
        name: name.to_string(),
        location: "query".to_string(),
        required,
        schema: SchemaObject {
            type_name: Some("string".to_string()),
            format: None,
            properties: None,
            items: None,
            required: None,
            description: Some(description.to_string()),
            example: None,
        },
        description: Some(description.to_string()),
    }
}

fn param_path(name: &str, description: &str, required: bool) -> Parameter {
    Parameter {
        name: name.to_string(),
        location: "path".to_string(),
        required,
        schema: SchemaObject {
            type_name: Some("string".to_string()),
            format: None,
            properties: None,
            items: None,
            required: None,
            description: Some(description.to_string()),
            example: None,
        },
        description: Some(description.to_string()),
    }
}

/// Helper to create a SecurityRequirement for BearerAuth
fn bearer_auth() -> SecurityRequirement {
    let mut schemes = std::collections::HashMap::new();
    schemes.insert("BearerAuth".to_string(), vec![]);
    SecurityRequirement { schemes }
}

fn request_body_json(required_fields: Vec<&str>, description: &str) -> Option<RequestBody> {
    let mut props = std::collections::HashMap::new();
    for field in &required_fields {
        props.insert(
            field.to_string(),
            SchemaObject {
                type_name: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                required: None,
                description: None,
                example: None,
            },
        );
    }

    Some(RequestBody {
        required: true,
        content: serde_json::json!({
            "application/json": {
                "schema": {
                    "type": "object",
                    "properties": props,
                    "required": required_fields
                }
            }
        }).as_object().unwrap().clone().into_iter().collect(),
        description: Some(description.to_string()),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// GxP Annotation Builders
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal GxP annotation for public/read-only endpoints
fn gxp_minimal() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "read".to_string(),
            captures_old_value: false,
            captures_new_value: false,
            retention_days: 90,
            pii_fields: None,
        }),
        electronic_signature: None,
        access_control: Some(AccessControlAnnotation {
            min_password_length: 0,
            password_rotation_days: None,
            required_roles: vec![],
            allows_api_key: true,
            session_timeout_minutes: 60,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: false,
            output_sanitization: true,
            idempotent: true,
            checksum_computed: false,
            validation_rules: vec![],
        }),
    }
}

/// GxP annotation for read operations (list, get)
fn gxp_audit_read() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "read".to_string(),
            captures_old_value: false,
            captures_new_value: false,
            retention_days: 2555, // ~7 years for GxP
            pii_fields: Some(vec!["password".to_string(), "token".to_string()]),
        }),
        electronic_signature: None,
        access_control: Some(AccessControlAnnotation {
            min_password_length: 8,
            password_rotation_days: None,
            required_roles: vec!["viewer".to_string(), "operator".to_string(), "admin".to_string()],
            allows_api_key: true,
            session_timeout_minutes: 30,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: false,
            output_sanitization: true,
            idempotent: true,
            checksum_computed: false,
            validation_rules: vec![],
        }),
    }
}

/// Full GxP annotation for data modification operations (create, update)
fn gxp_full() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "create".to_string(),
            captures_old_value: false,
            captures_new_value: true,
            retention_days: 2555,
            pii_fields: Some(vec!["password".to_string(), "token".to_string(), "secret".to_string()]),
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: false,
            meaning: "I attest that this data is accurate".to_string(),
            requires_2fa: false,
            non_repudiable: true,
            applies_to: vec!["create".to_string(), "update".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 12,
            password_rotation_days: Some(90),
            required_roles: vec!["operator".to_string(), "admin".to_string()],
            allows_api_key: false,
            session_timeout_minutes: 15,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["required_fields".to_string(), "type_check".to_string()],
        }),
    }
}

/// GxP annotation for delete operations
fn gxp_delete() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "delete".to_string(),
            captures_old_value: true,
            captures_new_value: false,
            retention_days: 2555,
            pii_fields: Some(vec!["password".to_string(), "token".to_string()]),
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: true,
            meaning: "I confirm deletion of this record".to_string(),
            requires_2fa: true,
            non_repudiable: true,
            applies_to: vec!["delete".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 12,
            password_rotation_days: Some(90),
            required_roles: vec!["admin".to_string()],
            allows_api_key: false,
            session_timeout_minutes: 15,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: false,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["exists_check".to_string()],
        }),
    }
}

/// GxP annotation for execution operations (agent invoke)
fn gxp_execution() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "execute".to_string(),
            captures_old_value: false,
            captures_new_value: true,
            retention_days: 2555,
            pii_fields: Some(vec!["prompt".to_string(), "response".to_string()]),
        }),
        electronic_signature: None,
        access_control: Some(AccessControlAnnotation {
            min_password_length: 8,
            password_rotation_days: None,
            required_roles: vec!["operator".to_string(), "admin".to_string()],
            allows_api_key: true,
            session_timeout_minutes: 30,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["prompt_not_empty".to_string()],
        }),
    }
}

/// GxP annotation for admin-only operations
fn gxp_admin() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "admin".to_string(),
            captures_old_value: true,
            captures_new_value: true,
            retention_days: 2555,
            pii_fields: Some(vec!["password".to_string(), "secret".to_string(), "key".to_string()]),
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: true,
            meaning: "I approve this administrative action".to_string(),
            requires_2fa: true,
            non_repudiable: true,
            applies_to: vec!["update".to_string(), "delete".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 16,
            password_rotation_days: Some(30),
            required_roles: vec!["admin".to_string()],
            allows_api_key: false,
            session_timeout_minutes: 10,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["admin_role_check".to_string(), "config_validation".to_string()],
        }),
    }
}

/// GxP annotation for login
fn gxp_auth_login() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "auth_login".to_string(),
            captures_old_value: false,
            captures_new_value: false,
            retention_days: 2555,
            pii_fields: Some(vec!["password".to_string()]),
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: true,
            meaning: "I attest to my identity".to_string(),
            requires_2fa: false,
            non_repudiable: true,
            applies_to: vec!["login".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 8,
            password_rotation_days: Some(90),
            required_roles: vec![],
            allows_api_key: false,
            session_timeout_minutes: 480, // 8 hours
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: false,
            validation_rules: vec!["credentials_format".to_string()],
        }),
    }
}

/// GxP annotation for 2FA verification
fn gxp_2fa() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "auth_2fa".to_string(),
            captures_old_value: false,
            captures_new_value: false,
            retention_days: 2555,
            pii_fields: None,
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: true,
            meaning: "I confirm two-factor authentication".to_string(),
            requires_2fa: false, // 2FA verification doesn't require 2FA itself
            non_repudiable: true,
            applies_to: vec!["verify".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 6, // TOTP code length
            password_rotation_days: None,
            required_roles: vec![],
            allows_api_key: false,
            session_timeout_minutes: 5,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["totp_format".to_string(), "totp_not_expired".to_string()],
        }),
    }
}

/// GxP annotation for password change
fn gxp_password_change() -> GxPAnnotations {
    GxPAnnotations {
        audit_trail: Some(AuditTrailAnnotation {
            logged: true,
            event_type: "auth_password_change".to_string(),
            captures_old_value: false,
            captures_new_value: false,
            retention_days: 2555,
            pii_fields: Some(vec!["old_password".to_string(), "new_password".to_string()]),
        }),
        electronic_signature: Some(ElectronicSignatureAnnotation {
            required: true,
            meaning: "I confirm password change".to_string(),
            requires_2fa: true,
            non_repudiable: true,
            applies_to: vec!["change".to_string()],
        }),
        access_control: Some(AccessControlAnnotation {
            min_password_length: 12,
            password_rotation_days: Some(90),
            required_roles: vec![],
            allows_api_key: false,
            session_timeout_minutes: 15,
        }),
        data_integrity: Some(DataIntegrityAnnotation {
            input_validation: true,
            output_sanitization: true,
            idempotent: false,
            checksum_computed: true,
            validation_rules: vec!["password_strength".to_string(), "password_history".to_string()],
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP Handler for OpenAPI Docs
// ─────────────────────────────────────────────────────────────────────────────

/// GET /api-docs
///
/// Returns the OpenAPI 3.1 specification as JSON
pub async fn get_openapi_docs(State(_state): State<AppState>) -> Json<Value> {
    let doc = OpenApiDoc::generate();
    Json(doc.to_json())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_generation_succeeds() {
        let doc = OpenApiDoc::generate();
        assert_eq!(doc.openapi, "3.1.0");
        assert!(!doc.info.title.is_empty());
        assert!(!doc.paths.is_empty());
    }

    #[test]
    fn test_openapi_json_serialization() {
        let doc = OpenApiDoc::generate();
        let json = doc.to_json();
        assert!(json.is_object());
        assert!(json.get("openapi").is_some());
        assert!(json.get("paths").is_some());
    }

    #[test]
    fn test_endpoint_count() {
        let doc = OpenApiDoc::generate();
        // Count paths (endpoints)
        let path_count = doc.paths.len();
        assert!(
            path_count >= 15,
            "Expected at least 15 endpoints, got {}",
            path_count
        );
    }

    #[test]
    fn test_gxp_annotations_present() {
        let doc = OpenApiDoc::generate();
        let mut annotated_count = 0;
        let mut total_operations = 0;

        for path_item in doc.paths.values() {
            for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.patch, &path_item.delete] {
                if let Some(operation) = op {
                    total_operations += 1;
                    if operation.x_gxp_annotations.is_some() {
                        annotated_count += 1;
                    }
                }
            }
        }

        assert_eq!(
            annotated_count, total_operations,
            "All {} operations should have GxP annotations, but only {} do",
            total_operations, annotated_count
        );
    }

    #[test]
    fn test_audit_trail_annotations() {
        let doc = OpenApiDoc::generate();
        let mut has_audit_trail = false;

        for path_item in doc.paths.values() {
            for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.patch, &path_item.delete] {
                if let Some(operation) = op {
                    if let Some(ref gxp) = operation.x_gxp_annotations {
                        if let Some(ref audit) = gxp.audit_trail {
                            has_audit_trail = true;
                            assert!(
                                audit.retention_days >= 90,
                                "Audit trail retention should be at least 90 days"
                            );
                        }
                    }
                }
            }
        }

        assert!(has_audit_trail, "Should have audit trail annotations");
    }

    #[test]
    fn test_electronic_signature_annotations() {
        let doc = OpenApiDoc::generate();
        let mut has_esig = false;

        for path_item in doc.paths.values() {
            for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.patch, &path_item.delete] {
                if let Some(operation) = op {
                    if let Some(ref gxp) = operation.x_gxp_annotations {
                        if gxp.electronic_signature.is_some() {
                            has_esig = true;
                        }
                    }
                }
            }
        }

        assert!(has_esig, "Should have electronic signature annotations on some endpoints");
    }

    #[test]
    fn test_access_control_annotations() {
        let doc = OpenApiDoc::generate();
        let mut has_access_control = false;

        for path_item in doc.paths.values() {
            for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.patch, &path_item.delete] {
                if let Some(operation) = op {
                    if let Some(ref gxp) = operation.x_gxp_annotations {
                        if let Some(ref ac) = gxp.access_control {
                            has_access_control = true;
                            assert!(
                                ac.min_password_length >= 8 || ac.min_password_length == 0,
                                "Password min length should be >= 8 or 0 for public endpoints"
                            );
                        }
                    }
                }
            }
        }

        assert!(has_access_control, "Should have access control annotations");
    }

    #[test]
    fn test_data_integrity_annotations() {
        let doc = OpenApiDoc::generate();
        let mut has_data_integrity = false;

        for path_item in doc.paths.values() {
            for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.patch, &path_item.delete] {
                if let Some(operation) = op {
                    if let Some(ref gxp) = operation.x_gxp_annotations {
                        if let Some(ref di) = gxp.data_integrity {
                            has_data_integrity = true;
                            // Sanitization should be true for outputs
                            assert!(di.output_sanitization, "Output sanitization should be true");
                        }
                    }
                }
            }
        }

        assert!(has_data_integrity, "Should have data integrity annotations");
    }

    #[test]
    fn test_json_output_validity() {
        let doc = OpenApiDoc::generate();
        let json_str = serde_json::to_string_pretty(&doc);
        assert!(json_str.is_ok(), "Should serialize to valid JSON");

        let json_bytes = json_str.unwrap().into_bytes();
        let parsed: Result<Value, _> = serde_json::from_slice(&json_bytes);
        assert!(parsed.is_ok(), "Should deserialize from JSON");
    }

    #[test]
    fn test_components_have_schemas() {
        let doc = OpenApiDoc::generate();
        assert!(doc.components.schemas.is_some());
        let schemas = doc.components.schemas.unwrap();
        assert!(!schemas.is_empty(), "Should have at least one schema");
    }

    #[test]
    fn test_components_have_security_schemes() {
        let doc = OpenApiDoc::generate();
        assert!(doc.components.security_schemes.is_some());
        let schemes = doc.components.security_schemes.unwrap();
        assert!(schemes.contains_key("BearerAuth"), "Should have BearerAuth security scheme");
    }

    #[test]
    fn test_tags_are_defined() {
        let doc = OpenApiDoc::generate();
        assert!(doc.tags.is_some());
        let tags = doc.tags.unwrap();
        assert!(tags.len() >= 5, "Should have at least 5 tags");
    }

    #[test]
    fn test_servers_are_defined() {
        let doc = OpenApiDoc::generate();
        assert!(!doc.servers.is_empty(), "Should have at least one server");
    }

    #[test]
    fn test_info_version() {
        let doc = OpenApiDoc::generate();
        assert!(!doc.info.version.is_empty(), "Version should not be empty");
    }

    #[test]
    fn test_description_contains_gxp() {
        let doc = OpenApiDoc::generate();
        let desc = doc.info.description.to_lowercase();
        assert!(
            desc.contains("gxp") || desc.contains("§11"),
            "Description should mention GxP compliance"
        );
    }
}
