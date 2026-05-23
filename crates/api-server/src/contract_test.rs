//! API Contract Testing Module
//!
//! Validates API responses against an OpenAPI specification to ensure
//! contract compliance and detect breaking changes between spec versions.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// OpenAPI Spec Types (subset sufficient for contract validation)
// ---------------------------------------------------------------------------

/// Minimal OpenAPI v3 spec representation for contract testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: SpecInfo,
    pub paths: HashMap<String, PathItem>,
    #[serde(default)]
    pub components: Option<Components>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecInfo {
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub responses: HashMap<String, Response>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(default)]
    pub content: Option<HashMap<String, MediaType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Components {
    #[serde(default)]
    pub schemas: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Validation result types
// ---------------------------------------------------------------------------

/// Result of a single validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub checks: Vec<ValidationCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub check_type: CheckType,
    pub path: String,
    pub method: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckType {
    SchemaValidation,
    StatusCode,
    BreakingChange,
}

/// A detected breaking change between two spec versions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreakingChange {
    pub change_type: BreakingChangeType,
    pub path: String,
    pub method: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BreakingChangeType {
    /// An endpoint was removed
    EndpointRemoved,
    /// A required field was added to a response schema
    RequiredFieldAdded,
    /// A response status code was removed
    StatusCodeRemoved,
    /// A field type changed
    FieldTypeChanged,
}

// ---------------------------------------------------------------------------
// ContractValidator
// ---------------------------------------------------------------------------

/// Validates API responses against an OpenAPI specification.
#[derive(Debug)]
pub struct ContractValidator {
    spec: Arc<OpenApiSpec>,
}

impl ContractValidator {
    /// Create a new validator from a parsed OpenAPI spec.
    pub fn new(spec: OpenApiSpec) -> Self {
        Self {
            spec: Arc::new(spec),
        }
    }

    /// Load and parse an OpenAPI spec from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, ContractError> {
        let spec: OpenApiSpec =
            serde_json::from_str(json).map_err(|e| ContractError::SpecParseError(e.to_string()))?;
        Ok(Self::new(spec))
    }

    /// Load and parse an OpenAPI spec from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, ContractError> {
        let spec: OpenApiSpec =
            serde_yaml::from_str(yaml).map_err(|e| ContractError::SpecParseError(e.to_string()))?;
        Ok(Self::new(spec))
    }

    /// Get a reference to the loaded spec.
    pub fn spec(&self) -> &OpenApiSpec {
        &self.spec
    }

    /// Validate a response body against the schema defined for the given
    /// endpoint (path + method + status code).
    ///
    /// Returns `Ok(())` if valid, or a list of validation errors.
    pub fn validate_response_body(
        &self,
        path: &str,
        method: &str,
        status_code: u16,
        body: &serde_json::Value,
    ) -> Result<(), Vec<String>> {
        let path_item = self
            .spec
            .paths
            .get(path)
            .ok_or_else(|| vec![format!("Path '{}' not found in spec", path)])?;

        let operation = Self::get_operation(path_item, method).ok_or_else(|| {
            vec![format!(
                "Method '{}' not defined for path '{}'",
                method, path
            )]
        })?;

        let status_str = status_code.to_string();
        let response = operation.responses.get(&status_str).ok_or_else(|| {
            vec![format!(
                "Status code '{}' not defined for {} {}",
                status_str, method, path
            )]
        })?;

        // Extract the JSON schema from the response content
        let content = response.content.as_ref().ok_or_else(|| {
            vec![format!(
                "No content defined for {} {} status {}",
                method, path, status_str
            )]
        })?;

        let media_type = content.get("application/json").ok_or_else(|| {
            vec![format!(
                "No 'application/json' content type for {} {} status {}",
                method, path, status_str
            )]
        })?;

        let schema = &media_type.schema;

        // Perform schema validation
        let mut errors = Vec::new();
        Self::validate_value_against_schema(body, schema, &self.spec, &mut errors, "$");

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate that a status code is defined in the spec for the given path
    /// and method.
    pub fn validate_status_code(
        &self,
        path: &str,
        method: &str,
        status_code: u16,
    ) -> Result<(), String> {
        let path_item = self
            .spec
            .paths
            .get(path)
            .ok_or_else(|| format!("Path '{}' not found in spec", path))?;

        let operation = Self::get_operation(path_item, method)
            .ok_or_else(|| format!("Method '{}' not defined for path '{}'", method, path))?;

        let status_str = status_code.to_string();
        if operation.responses.contains_key(&status_str) {
            Ok(())
        } else {
            Err(format!(
                "Status code {} not defined for {} {}; defined: {:?}",
                status_code,
                method,
                path,
                operation.responses.keys().collect::<Vec<_>>()
            ))
        }
    }

    /// Detect breaking changes between the current (old) spec and a new spec.
    ///
    /// Breaking changes include:
    /// - Removed endpoints
    /// - Removed response status codes
    /// - Added required fields to response schemas
    /// - Changed field types in response schemas
    pub fn detect_breaking_changes(&self, new_spec: &OpenApiSpec) -> Vec<BreakingChange> {
        let mut changes = Vec::new();

        // Check for removed endpoints
        for (path, old_item) in &self.spec.paths {
            let new_item = match new_spec.paths.get(path) {
                Some(item) => item,
                None => {
                    // All methods on this path are removed
                    Self::collect_endpoint_removed(&mut changes, path, old_item);
                    continue;
                }
            };

            // For each method, check if it was removed or its responses changed
            Self::check_method_changes(
                &mut changes,
                path,
                "get",
                old_item.get.as_ref(),
                new_item.get.as_ref(),
            );
            Self::check_method_changes(
                &mut changes,
                path,
                "post",
                old_item.post.as_ref(),
                new_item.post.as_ref(),
            );
            Self::check_method_changes(
                &mut changes,
                path,
                "put",
                old_item.put.as_ref(),
                new_item.put.as_ref(),
            );
            Self::check_method_changes(
                &mut changes,
                path,
                "patch",
                old_item.patch.as_ref(),
                new_item.patch.as_ref(),
            );
            Self::check_method_changes(
                &mut changes,
                path,
                "delete",
                old_item.delete.as_ref(),
                new_item.delete.as_ref(),
            );
        }

        changes
    }

    /// Validate a full HTTP response (status code + body) in one call.
    pub fn validate_response(
        &self,
        path: &str,
        method: &str,
        status_code: u16,
        body: &serde_json::Value,
    ) -> ValidationReport {
        let mut checks = Vec::new();

        // Check status code
        let status_result = self.validate_status_code(path, method, status_code);
        checks.push(ValidationCheck {
            check_type: CheckType::StatusCode,
            path: path.to_string(),
            method: method.to_string(),
            passed: status_result.is_ok(),
            message: status_result.err().unwrap_or_default(),
        });

        // Check response body schema
        let body_result = self.validate_response_body(path, method, status_code, body);
        checks.push(ValidationCheck {
            check_type: CheckType::SchemaValidation,
            path: path.to_string(),
            method: method.to_string(),
            passed: body_result.is_ok(),
            message: body_result.err().map(|e| e.join("; ")).unwrap_or_default(),
        });

        let passed = checks.iter().all(|c| c.passed);
        ValidationReport { passed, checks }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn get_operation<'a>(item: &'a PathItem, method: &str) -> Option<&'a Operation> {
        match method.to_lowercase().as_str() {
            "get" => item.get.as_ref(),
            "post" => item.post.as_ref(),
            "put" => item.put.as_ref(),
            "patch" => item.patch.as_ref(),
            "delete" => item.delete.as_ref(),
            _ => None,
        }
    }

    fn collect_endpoint_removed(changes: &mut Vec<BreakingChange>, path: &str, item: &PathItem) {
        let methods = [
            ("get", &item.get),
            ("post", &item.post),
            ("put", &item.put),
            ("patch", &item.patch),
            ("delete", &item.delete),
        ];
        for (method, op) in &methods {
            if op.is_some() {
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::EndpointRemoved,
                    path: path.to_string(),
                    method: method.to_string(),
                    detail: format!("Endpoint {} {} was removed", method.to_uppercase(), path),
                });
            }
        }
    }

    fn check_method_changes(
        changes: &mut Vec<BreakingChange>,
        path: &str,
        method: &str,
        old_op: Option<&Operation>,
        new_op: Option<&Operation>,
    ) {
        let old_op = match old_op {
            Some(op) => op,
            None => return, // method didn't exist before, not a breaking change
        };

        let new_op = match new_op {
            Some(op) => op,
            None => {
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::EndpointRemoved,
                    path: path.to_string(),
                    method: method.to_string(),
                    detail: format!("{} {} was removed", method.to_uppercase(), path),
                });
                return;
            }
        };

        // Check for removed status codes
        for status in old_op.responses.keys() {
            if !new_op.responses.contains_key(status) {
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::StatusCodeRemoved,
                    path: path.to_string(),
                    method: method.to_string(),
                    detail: format!(
                        "Response status {} removed from {} {}",
                        status,
                        method.to_uppercase(),
                        path
                    ),
                });
            }
        }

        // Check schema changes for shared status codes
        for (status, old_resp) in &old_op.responses {
            if let Some(new_resp) = new_op.responses.get(status) {
                Self::check_schema_changes(changes, path, method, status, old_resp, new_resp);
            }
        }
    }

    fn check_schema_changes(
        changes: &mut Vec<BreakingChange>,
        path: &str,
        method: &str,
        status: &str,
        old_resp: &Response,
        new_resp: &Response,
    ) {
        let old_schema = match old_resp
            .content
            .as_ref()
            .and_then(|c| c.get("application/json"))
            .map(|m| &m.schema)
        {
            Some(s) => s,
            None => return,
        };

        let new_schema = match new_resp
            .content
            .as_ref()
            .and_then(|c| c.get("application/json"))
            .map(|m| &m.schema)
        {
            Some(s) => s,
            None => return,
        };

        Self::diff_schemas(changes, path, method, status, old_schema, new_schema);
    }

    fn diff_schemas(
        changes: &mut Vec<BreakingChange>,
        path: &str,
        method: &str,
        status: &str,
        old_schema: &serde_json::Value,
        new_schema: &serde_json::Value,
    ) {
        // Resolve $ref if present (simple inline resolution)
        // For this implementation we compare properties directly

        let old_props = old_schema.get("properties");
        let new_props = new_schema.get("properties");

        if let (Some(old_props), Some(new_props)) = (old_props, new_props) {
            let old_obj = old_props.as_object();
            let new_obj = new_props.as_object();

            if let (Some(old_obj), Some(new_obj)) = (old_obj, new_obj) {
                // Check for type changes in existing fields
                for (key, old_val) in old_obj {
                    if let Some(new_val) = new_obj.get(key) {
                        let old_type = Self::extract_type(old_val);
                        let new_type = Self::extract_type(new_val);
                        if old_type != new_type {
                            changes.push(BreakingChange {
                                change_type: BreakingChangeType::FieldTypeChanged,
                                path: path.to_string(),
                                method: method.to_string(),
                                detail: format!(
                                    "Field '{}' type changed from '{}' to '{}' in {} {} status {}",
                                    key,
                                    old_type,
                                    new_type,
                                    method.to_uppercase(),
                                    path,
                                    status
                                ),
                            });
                        }
                    }
                }

                // Check for added required fields
                let new_required: Vec<&str> = new_schema
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();

                let old_required: HashSet<&str> = old_schema
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();

                for req_field in &new_required {
                    if !old_required.contains(req_field) && new_obj.contains_key(*req_field) {
                        changes.push(BreakingChange {
                            change_type: BreakingChangeType::RequiredFieldAdded,
                            path: path.to_string(),
                            method: method.to_string(),
                            detail: format!(
                                "Required field '{}' added to {} {} status {} response schema",
                                req_field,
                                method.to_uppercase(),
                                path,
                                status
                            ),
                        });
                    }
                }
            }
        }

        // Check if type itself changed at the top level
        let old_type = Self::extract_type(old_schema);
        let new_type = Self::extract_type(new_schema);
        if old_type != "any" && new_type != "any" && old_type != new_type {
            changes.push(BreakingChange {
                change_type: BreakingChangeType::FieldTypeChanged,
                path: path.to_string(),
                method: method.to_string(),
                detail: format!(
                    "Response schema type changed from '{}' to '{}' for {} {} status {}",
                    old_type,
                    new_type,
                    method.to_uppercase(),
                    path,
                    status
                ),
            });
        }
    }

    fn extract_type(schema: &serde_json::Value) -> String {
        schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("any")
            .to_string()
    }

    /// Recursively validate a JSON value against a schema object.
    ///
    /// This performs lightweight type-checking and required-field validation
    /// without depending on a full JSON Schema validator crate.
    fn validate_value_against_schema(
        value: &serde_json::Value,
        schema: &serde_json::Value,
        spec: &OpenApiSpec,
        errors: &mut Vec<String>,
        path_prefix: &str,
    ) {
        // Resolve $ref if present
        let resolved = if let Some(reference) = schema.get("$ref").and_then(|r| r.as_str()) {
            Self::resolve_ref(reference, spec).unwrap_or(schema.clone())
        } else {
            schema.clone()
        };

        // Type validation
        if let Some(expected_type) = resolved.get("type").and_then(|t| t.as_str()) {
            let actual_ok = match expected_type {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => true,
            };
            if !actual_ok {
                errors.push(format!(
                    "{}: expected type '{}' but got {}",
                    path_prefix,
                    expected_type,
                    Self::json_type_name(value)
                ));
                return; // No point checking deeper if top-level type is wrong
            }
        }

        // Object: check required fields and recurse into properties
        if let Some(obj) = value.as_object() {
            // Required fields
            if let Some(required) = resolved.get("required").and_then(|r| r.as_array()) {
                for field in required.iter().filter_map(|v| v.as_str()) {
                    if !obj.contains_key(field) {
                        errors.push(format!("{}.{}: required field missing", path_prefix, field));
                    }
                }
            }

            // Validate each present property against its schema
            if let Some(props) = resolved.get("properties").and_then(|p| p.as_object()) {
                for (key, prop_schema) in props {
                    if let Some(val) = obj.get(key) {
                        Self::validate_value_against_schema(
                            val,
                            prop_schema,
                            spec,
                            errors,
                            &format!("{}.{}", path_prefix, key),
                        );
                    }
                }
            }
        }

        // Array: validate items
        if let (Some(arr), Some(item_schema)) = (value.as_array(), resolved.get("items")) {
            for (i, item) in arr.iter().enumerate() {
                Self::validate_value_against_schema(
                    item,
                    item_schema,
                    spec,
                    errors,
                    &format!("{}[{}]", path_prefix, i),
                );
            }
        }
    }

    /// Resolve a JSON Pointer reference (#/components/schemas/Foo).
    fn resolve_ref(reference: &str, spec: &OpenApiSpec) -> Option<serde_json::Value> {
        if !reference.starts_with("#/") {
            return None;
        }
        let parts: Vec<&str> = reference[2..].split('/').collect();
        // We only support /components/schemas/<name>
        if parts.len() == 3 && parts[0] == "components" && parts[1] == "schemas" {
            let name = parts[2];
            return spec
                .components
                .as_ref()
                .and_then(|c| c.schemas.get(name))
                .cloned();
        }
        None
    }

    fn json_type_name(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("Failed to parse OpenAPI spec: {0}")]
    SpecParseError(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal spec JSON for testing.
    fn sample_spec_json() -> String {
        r#"{
            "openapi": "3.0.3",
            "info": { "title": "KIAS API", "version": "1.0.0" },
            "paths": {
                "/health": {
                    "get": {
                        "operationId": "liveness",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "required": ["status"],
                                            "properties": {
                                                "status": { "type": "string" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/api/v1/agents": {
                    "get": {
                        "operationId": "listAgents",
                        "responses": {
                            "200": {
                                "description": "Agent list",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "required": ["id", "name", "status"],
                                                "properties": {
                                                    "id": { "type": "string" },
                                                    "name": { "type": "string" },
                                                    "status": { "type": "string" },
                                                    "node_id": { "type": "string" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "post": {
                        "operationId": "createAgent",
                        "responses": {
                            "201": {
                                "description": "Created",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "required": ["id", "spec", "status"],
                                            "properties": {
                                                "id": { "type": "string" },
                                                "spec": { "type": "object" },
                                                "status": { "type": "string" },
                                                "created_at": { "type": "string" }
                                            }
                                        }
                                    }
                                }
                            },
                            "400": {
                                "description": "Bad request",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "error": {
                                                    "type": "object",
                                                    "properties": {
                                                        "code": { "type": "integer" },
                                                        "message": { "type": "string" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/api/v1/nodes": {
                    "get": {
                        "operationId": "listNodes",
                        "responses": {
                            "200": {
                                "description": "Node list",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "required": ["id", "name", "status"],
                                                "properties": {
                                                    "id": { "type": "string" },
                                                    "name": { "type": "string" },
                                                    "status": { "type": "string" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "Error": {
                        "type": "object",
                        "required": ["code", "message"],
                        "properties": {
                            "code": { "type": "integer" },
                            "message": { "type": "string" }
                        }
                    }
                }
            }
        }"#
        .to_string()
    }

    fn load_validator() -> ContractValidator {
        ContractValidator::from_json(&sample_spec_json()).expect("failed to parse spec")
    }

    // --- Test 1: Successfully load a valid OpenAPI spec ---
    #[test]
    fn test_load_valid_spec() {
        let validator = load_validator();
        assert_eq!(validator.spec().info.title, "KIAS API");
        assert_eq!(validator.spec().info.version, "1.0.0");
        assert_eq!(validator.spec().openapi, "3.0.3");
        assert!(!validator.spec().paths.is_empty());
    }

    // --- Test 2: Fail to load an invalid spec ---
    #[test]
    fn test_load_invalid_spec_returns_error() {
        let result = ContractValidator::from_json("not valid json");
        assert!(result.is_err());
        match result.unwrap_err() {
            ContractError::SpecParseError(msg) => {
                assert!(msg.contains("expected"));
            }
        }
    }

    // --- Test 3: Validate response body against schema — passing case ---
    #[test]
    fn test_validate_response_body_passes() {
        let validator = load_validator();
        let body = serde_json::json!({"status": "ok"});
        let result = validator.validate_response_body("/health", "get", 200, &body);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    }

    // --- Test 4: Validate response body fails for missing required field ---
    #[test]
    fn test_validate_response_body_fails_missing_required_field() {
        let validator = load_validator();
        let body = serde_json::json!({}); // missing "status"
        let result = validator.validate_response_body("/health", "get", 200, &body);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("status") && e.contains("required")));
    }

    // --- Test 5: Validate response body fails for wrong type ---
    #[test]
    fn test_validate_response_body_fails_wrong_type() {
        let validator = load_validator();
        let body = serde_json::json!({"status": 123}); // integer, not string
        let result = validator.validate_response_body("/health", "get", 200, &body);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("expected type 'string'")));
    }

    // --- Test 6: Validate response body for array items ---
    #[test]
    fn test_validate_array_items_schema() {
        let validator = load_validator();
        let body = serde_json::json!([
            {"id": "1", "name": "node-1", "status": "Ready"},
            {"id": "2", "name": "node-2", "status": "Ready"}
        ]);
        let result = validator.validate_response_body("/api/v1/nodes", "get", 200, &body);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    }

    // --- Test 7: Validate array item with missing required field ---
    #[test]
    fn test_validate_array_item_missing_field() {
        let validator = load_validator();
        let body = serde_json::json!([
            {"id": "1", "name": "node-1"}, // missing "status"
            {"id": "2", "name": "node-2", "status": "Ready"}
        ]);
        let result = validator.validate_response_body("/api/v1/nodes", "get", 200, &body);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("[0]") && e.contains("status")));
    }

    // --- Test 8: Validate status code — valid ---
    #[test]
    fn test_validate_status_code_passes() {
        let validator = load_validator();
        assert!(validator
            .validate_status_code("/health", "get", 200)
            .is_ok());
        assert!(validator
            .validate_status_code("/api/v1/agents", "post", 201)
            .is_ok());
        assert!(validator
            .validate_status_code("/api/v1/agents", "post", 400)
            .is_ok());
    }

    // // --- Test 9: Validate status code — invalid ---
    #[test]
    fn test_validate_status_code_fails_for_undeclared_code() {
        let validator = load_validator();
        let result = validator.validate_status_code("/health", "get", 500);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("500"));
        assert!(msg.contains("not defined"));
    }

    // --- Test 10: Validate status code for unknown path ---
    #[test]
    fn test_validate_status_code_unknown_path() {
        let validator = load_validator();
        let result = validator.validate_status_code("/nonexistent", "get", 200);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // --- Test 11: Full response validation report ---
    #[test]
    fn test_validate_response_full_report() {
        let validator = load_validator();
        let body = serde_json::json!({"status": "ok"});
        let report = validator.validate_response("/health", "get", 200, &body);
        assert!(report.passed);
        assert_eq!(report.checks.len(), 2);
        assert!(report.checks.iter().all(|c| c.passed));
    }

    // --- Test 12: Full response validation report with failure ---
    #[test]
    fn test_validate_response_report_with_failure() {
        let validator = load_validator();
        let body = serde_json::json!({});
        let report = validator.validate_response("/health", "get", 200, &body);
        assert!(!report.passed);
        // Status code check passes, schema check fails
        let status_check = report
            .checks
            .iter()
            .find(|c| c.check_type == CheckType::StatusCode)
            .unwrap();
        assert!(status_check.passed);
        let schema_check = report
            .checks
            .iter()
            .find(|c| c.check_type == CheckType::SchemaValidation)
            .unwrap();
        assert!(!schema_check.passed);
    }

    // --- Test 13: Detect breaking changes — no changes ---
    #[test]
    fn test_detect_breaking_changes_none() {
        let validator = load_validator();
        let new_spec: OpenApiSpec = serde_json::from_str(&sample_spec_json()).unwrap();
        let changes = validator.detect_breaking_changes(&new_spec);
        assert!(changes.is_empty(), "Expected no changes, got {:?}", changes);
    }

    // --- Test 14: Detect breaking changes — endpoint removed ---
    #[test]
    fn test_detect_breaking_changes_endpoint_removed() {
        let validator = load_validator();

        let mut new_spec: OpenApiSpec = serde_json::from_str(&sample_spec_json()).unwrap();
        new_spec.paths.remove("/api/v1/nodes");

        let changes = validator.detect_breaking_changes(&new_spec);
        assert!(!changes.is_empty());
        assert!(changes
            .iter()
            .any(|c| c.change_type == BreakingChangeType::EndpointRemoved
                && c.path == "/api/v1/nodes"));
    }

    // --- Test 15: Detect breaking changes — status code removed ---
    #[test]
    fn test_detect_breaking_changes_status_code_removed() {
        let validator = load_validator();

        let mut new_spec: OpenApiSpec = serde_json::from_str(&sample_spec_json()).unwrap();
        if let Some(path_item) = new_spec.paths.get_mut("/api/v1/agents") {
            if let Some(ref mut post_op) = path_item.post {
                post_op.responses.remove("400");
            }
        }

        let changes = validator.detect_breaking_changes(&new_spec);
        assert!(changes
            .iter()
            .any(|c| c.change_type == BreakingChangeType::StatusCodeRemoved
                && c.path == "/api/v1/agents"
                && c.method == "post"
                && c.detail.contains("400")));
    }

    // --- Test 16: Detect breaking changes — required field added ---
    #[test]
    fn test_detect_breaking_changes_required_field_added() {
        let validator = load_validator();

        let mut new_spec: OpenApiSpec = serde_json::from_str(&sample_spec_json()).unwrap();
        if let Some(path_item) = new_spec.paths.get_mut("/health") {
            if let Some(ref mut get_op) = path_item.get {
                if let Some(resp) = get_op.responses.get_mut("200") {
                    if let Some(ref mut content) = resp.content {
                        if let Some(mt) = content.get_mut("application/json") {
                            // Add "uptime" as a new required field
                            mt.schema = serde_json::json!({
                                "type": "object",
                                "required": ["status", "uptime"],
                                "properties": {
                                    "status": { "type": "string" },
                                    "uptime": { "type": "integer" }
                                }
                            });
                        }
                    }
                }
            }
        }

        let changes = validator.detect_breaking_changes(&new_spec);
        assert!(changes
            .iter()
            .any(|c| c.change_type == BreakingChangeType::RequiredFieldAdded
                && c.detail.contains("uptime")));
    }

    // --- Test 17: Detect breaking changes — field type changed ---
    #[test]
    fn test_detect_breaking_changes_field_type_changed() {
        let validator = load_validator();

        let mut new_spec: OpenApiSpec = serde_json::from_str(&sample_spec_json()).unwrap();
        if let Some(path_item) = new_spec.paths.get_mut("/health") {
            if let Some(ref mut get_op) = path_item.get {
                if let Some(resp) = get_op.responses.get_mut("200") {
                    if let Some(ref mut content) = resp.content {
                        if let Some(mt) = content.get_mut("application/json") {
                            // Change "status" from string to integer
                            mt.schema = serde_json::json!({
                                "type": "object",
                                "required": ["status"],
                                "properties": {
                                    "status": { "type": "integer" }
                                }
                            });
                        }
                    }
                }
            }
        }

        let changes = validator.detect_breaking_changes(&new_spec);
        assert!(changes
            .iter()
            .any(|c| c.change_type == BreakingChangeType::FieldTypeChanged
                && c.detail.contains("status")
                && c.detail.contains("string")
                && c.detail.contains("integer")));
    }

    // --- Test 18: $ref resolution ---
    #[test]
    fn test_ref_resolution() {
        let spec_json = r##"{
            "openapi": "3.0.3",
            "info": { "title": "Test", "version": "1.0.0" },
            "paths": {
                "/test": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "$ref": "#/components/schemas/Error"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "Error": {
                        "type": "object",
                        "required": ["code", "message"],
                        "properties": {
                            "code": { "type": "integer" },
                            "message": { "type": "string" }
                        }
                    }
                }
            }
        } "##;

        let validator = ContractValidator::from_json(spec_json).unwrap();

        // Valid ref-resolved body
        let body = serde_json::json!({"code": 404, "message": "not found"});
        assert!(validator
            .validate_response_body("/test", "get", 200, &body)
            .is_ok());

        // Missing required field after ref resolution
        let body = serde_json::json!({"code": 500});
        assert!(validator
            .validate_response_body("/test", "get", 200, &body)
            .is_err());
    }
}
