use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Schema validation engine for Agent operations.
///
/// Validates Agent Cards, tool inputs/outputs, and message payloads
/// against JSON Schema definitions.
///
/// Inspired by EMQX's emqx_schema_validation, extended for Agent governance:
/// - Validate Agent Card on registration
/// - Validate tool call inputs/outputs
/// - Validate inter-Agent message payloads
/// - Reject non-compliant operations at the gate

/// Schema definition (simplified JSON Schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    /// Schema ID
    pub id: String,

    /// Schema name
    pub name: String,

    /// Schema version
    pub version: String,

    /// JSON Schema content
    pub schema: serde_json::Value,

    /// Schema type
    pub schema_type: SchemaType,

    /// Whether this schema is active
    pub active: bool,
}

/// Types of schemas we can validate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchemaType {
    /// Agent Card schema (A2A protocol)
    AgentCard,
    /// Tool input schema
    ToolInput,
    /// Tool output schema
    ToolOutput,
    /// Message payload schema
    MessagePayload,
    /// Custom schema
    Custom(String),
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Whether validation passed
    pub valid: bool,

    /// Schema that was used
    pub schema_id: String,

    /// Validation errors (if any)
    pub errors: Vec<ValidationError>,

    /// Warnings (non-blocking)
    pub warnings: Vec<String>,
}

/// A validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON path to the error
    pub path: String,

    /// Error message
    pub message: String,

    /// Expected type/format
    pub expected: Option<String>,

    /// Actual value
    pub actual: Option<String>,
}

/// Schema registry — stores and manages schemas
pub struct SchemaRegistry {
    schemas: Arc<RwLock<HashMap<String, SchemaDefinition>>>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new schema
    pub async fn register(&self, schema: SchemaDefinition) {
        let mut schemas = self.schemas.write().await;
        schemas.insert(schema.id.clone(), schema);
    }

    /// Get a schema by ID
    pub async fn get(&self, schema_id: &str) -> Option<SchemaDefinition> {
        let schemas = self.schemas.read().await;
        schemas.get(schema_id).cloned()
    }

    /// List all schemas
    pub async fn list(&self) -> Vec<SchemaDefinition> {
        let schemas = self.schemas.read().await;
        schemas.values().cloned().collect()
    }

    /// Delete a schema
    pub async fn delete(&self, schema_id: &str) -> bool {
        let mut schemas = self.schemas.write().await;
        schemas.remove(schema_id).is_some()
    }

    /// Count schemas
    pub async fn count(&self) -> usize {
        let schemas = self.schemas.read().await;
        schemas.len()
    }

    /// Validate a JSON value against a schema
    pub async fn validate(
        &self,
        schema_id: &str,
        data: &serde_json::Value,
    ) -> Result<ValidationReport, SchemaError> {
        let schemas = self.schemas.read().await;
        let schema = schemas
            .get(schema_id)
            .ok_or_else(|| SchemaError::NotFound {
                schema_id: schema_id.to_string(),
            })?;

        if !schema.active {
            return Err(SchemaError::Inactive {
                schema_id: schema_id.to_string(),
            });
        }

        // Perform validation
        let errors = self.validate_value(&schema.schema, data, "$");

        Ok(ValidationReport {
            valid: errors.is_empty(),
            schema_id: schema_id.to_string(),
            errors,
            warnings: vec![],
        })
    }

    /// Validate a value against a JSON Schema (simplified implementation)
    fn validate_value(
        &self,
        schema: &serde_json::Value,
        data: &serde_json::Value,
        path: &str,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for field in required {
                if let Some(field_name) = field.as_str() {
                    if data.get(field_name).is_none() {
                        errors.push(ValidationError {
                            path: format!("{}.{}", path, field_name),
                            message: format!("Required field '{}' is missing", field_name),
                            expected: Some("present".to_string()),
                            actual: Some("missing".to_string()),
                        });
                    }
                }
            }
        }

        // Check type
        if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
            let actual_type = match data {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
            };

            if expected_type != actual_type {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "Type mismatch: expected '{}', got '{}'",
                        expected_type, actual_type
                    ),
                    expected: Some(expected_type.to_string()),
                    actual: Some(actual_type.to_string()),
                });
            }
        }

        // Check properties for objects
        if let (Some(properties), Some(obj)) = (
            schema.get("properties").and_then(|p| p.as_object()),
            data.as_object(),
        ) {
            for (prop_name, prop_schema) in properties {
                if let Some(prop_value) = obj.get(prop_name) {
                    let prop_path = format!("{}.{}", path, prop_name);
                    errors.extend(self.validate_value(prop_schema, prop_value, &prop_path));
                }
            }
        }

        // Check array items
        if let (Some(items_schema), Some(arr)) = (
            schema.get("items"),
            data.as_array(),
        ) {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                errors.extend(self.validate_value(items_schema, item, &item_path));
            }
        }

        // Check enum values
        if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
            if !enum_values.contains(data) {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "Value not in enum: expected one of {:?}",
                        enum_values
                    ),
                    expected: Some(format!("{:?}", enum_values)),
                    actual: Some(format!("{:?}", data)),
                });
            }
        }

        // Check string length
        if let Some(s) = data.as_str() {
            if let Some(min_len) = schema.get("minLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) < min_len {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "String too short: {} < {}",
                            s.len(),
                            min_len
                        ),
                        expected: Some(format!(">= {} chars", min_len)),
                        actual: Some(format!("{} chars", s.len())),
                    });
                }
            }
            if let Some(max_len) = schema.get("maxLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) > max_len {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "String too long: {} > {}",
                            s.len(),
                            max_len
                        ),
                        expected: Some(format!("<= {} chars", max_len)),
                        actual: Some(format!("{} chars", s.len())),
                    });
                }
            }
        }

        errors
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema errors
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("Schema not found: {schema_id}")]
    NotFound { schema_id: String },

    #[error("Schema is inactive: {schema_id}")]
    Inactive { schema_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = SchemaRegistry::new();

        let schema = SchemaDefinition {
            id: "agent-card-v1".to_string(),
            name: "Agent Card Schema".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["agent_id", "name"],
                "properties": {
                    "agent_id": { "type": "string" },
                    "name": { "type": "string" }
                }
            }),
            schema_type: SchemaType::AgentCard,
            active: true,
        };

        registry.register(schema).await;

        let got = registry.get("agent-card-v1").await.unwrap();
        assert_eq!(got.name, "Agent Card Schema");
    }

    #[tokio::test]
    async fn test_validate_valid_data() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "test-schema".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["name", "version"],
                "properties": {
                    "name": { "type": "string" },
                    "version": { "type": "string" }
                }
            }),
            schema_type: SchemaType::AgentCard,
            active: true,
        }).await;

        let data = serde_json::json!({
            "name": "My Agent",
            "version": "1.0.0"
        });

        let report = registry.validate("test-schema", &data).await.unwrap();
        assert!(report.valid);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_missing_required() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "test-schema".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["name", "version"]
            }),
            schema_type: SchemaType::AgentCard,
            active: true,
        }).await;

        let data = serde_json::json!({
            "name": "My Agent"
            // missing "version"
        });

        let report = registry.validate("test-schema", &data).await.unwrap();
        assert!(!report.valid);
        assert!(!report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_wrong_type() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "test-schema".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": { "type": "number" }
                }
            }),
            schema_type: SchemaType::Custom("test".to_string()),
            active: true,
        }).await;

        let data = serde_json::json!({
            "count": "not a number"
        });

        let report = registry.validate("test-schema", &data).await.unwrap();
        assert!(!report.valid);
    }

    #[tokio::test]
    async fn test_validate_enum() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "status-schema".to_string(),
            name: "Status".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["online", "offline", "lwt"]
                    }
                }
            }),
            schema_type: SchemaType::Custom("test".to_string()),
            active: true,
        }).await;

        let valid = serde_json::json!({ "status": "online" });
        let report = registry.validate("status-schema", &valid).await.unwrap();
        assert!(report.valid);

        let invalid = serde_json::json!({ "status": "unknown" });
        let report = registry.validate("status-schema", &invalid).await.unwrap();
        assert!(!report.valid);
    }

    #[tokio::test]
    async fn test_validate_string_length() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "name-schema".to_string(),
            name: "Name".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "minLength": 3,
                        "maxLength": 50
                    }
                }
            }),
            schema_type: SchemaType::Custom("test".to_string()),
            active: true,
        }).await;

        let valid = serde_json::json!({ "name": "Alice" });
        let report = registry.validate("name-schema", &valid).await.unwrap();
        assert!(report.valid);

        let too_short = serde_json::json!({ "name": "AB" });
        let report = registry.validate("name-schema", &too_short).await.unwrap();
        assert!(!report.valid);
    }

    #[tokio::test]
    async fn test_validate_nested_properties() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "nested".to_string(),
            name: "Nested".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "object",
                        "required": ["id"],
                        "properties": {
                            "id": { "type": "string" }
                        }
                    }
                }
            }),
            schema_type: SchemaType::AgentCard,
            active: true,
        }).await;

        let valid = serde_json::json!({
            "agent": { "id": "a1" }
        });
        let report = registry.validate("nested", &valid).await.unwrap();
        assert!(report.valid);

        let invalid = serde_json::json!({
            "agent": { "name": "test" } // missing "id"
        });
        let report = registry.validate("nested", &invalid).await.unwrap();
        assert!(!report.valid);
    }

    #[tokio::test]
    async fn test_schema_not_found() {
        let registry = SchemaRegistry::new();

        let result = registry.validate("nonexistent", &serde_json::json!({})).await;
        assert!(matches!(result, Err(SchemaError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_inactive_schema() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "inactive".to_string(),
            name: "Inactive".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            schema_type: SchemaType::AgentCard,
            active: false,
        }).await;

        let result = registry.validate("inactive", &serde_json::json!({})).await;
        assert!(matches!(result, Err(SchemaError::Inactive { .. })));
    }

    #[tokio::test]
    async fn test_validate_array_items() {
        let registry = SchemaRegistry::new();

        registry.register(SchemaDefinition {
            id: "array-schema".to_string(),
            name: "Array".to_string(),
            version: "1.0.0".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }),
            schema_type: SchemaType::Custom("test".to_string()),
            active: true,
        }).await;

        let valid = serde_json::json!({ "tags": ["rust", "agent"] });
        let report = registry.validate("array-schema", &valid).await.unwrap();
        assert!(report.valid);

        let invalid = serde_json::json!({ "tags": [1, 2, 3] });
        let report = registry.validate("array-schema", &invalid).await.unwrap();
        assert!(!report.valid);
    }
}
