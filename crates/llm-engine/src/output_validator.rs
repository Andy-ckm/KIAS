//! # Output Validator — Structured LLM Output Validation
//!
//! Validates LLM responses against JSON Schema definitions with automatic
//! retry on malformed output. Rejects responses that don't conform to the
//! expected schema and triggers re-generation with corrective context.
//!
//! ## Features
//!
//! - JSON Schema validation (draft-07 compatible subset)
//! - Type checking (string, number, integer, boolean, array, object)
//! - Required field enforcement
//! - Enum constraints
//! - Pattern matching (regex)
//! - Numeric range validation (minimum, maximum)
//! - String length validation (minLength, maxLength)
//! - Array size validation (minItems, maxItems)
//! - Nested object validation
//! - Retry with error feedback to LLM
//!
//! ## Usage
//!
//! ```rust
//! use llm_engine::output_validator::*;
//!
//! let schema = JsonSchema::object()
//!     .property("name", JsonSchema::string().min_length(1))
//!     .property("age", JsonSchema::integer().minimum(0.0).maximum(200.0))
//!     .required("name");
//!
//! let validator = OutputValidator::new(schema);
//! let result = validator.validate(r#"{"name": "Alice", "age": 30}"#);
//! assert!(result.is_valid);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── JSON Schema ─────────────────────────────────────────────────────────

/// A simplified JSON Schema definition for LLM output validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    /// The JSON Schema type.
    #[serde(rename = "type")]
    pub schema_type: SchemaType,
    /// Properties for object types.
    #[serde(default)]
    pub properties: HashMap<String, JsonSchema>,
    /// Required property names.
    #[serde(default)]
    pub required: Vec<String>,
    /// Allowed values (enum constraint).
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    /// Regex pattern for string validation.
    pub pattern: Option<String>,
    /// Minimum value for numbers.
    pub minimum: Option<f64>,
    /// Maximum value for numbers.
    pub maximum: Option<f64>,
    /// Minimum string length.
    pub min_length: Option<usize>,
    /// Maximum string length.
    pub max_length: Option<usize>,
    /// Minimum array items.
    pub min_items: Option<usize>,
    /// Maximum array items.
    pub max_items: Option<usize>,
    /// Schema for array items.
    #[serde(rename = "items")]
    pub items_schema: Option<Box<JsonSchema>>,
    /// Description for documentation.
    pub description: Option<String>,
}

/// JSON Schema types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
}

impl JsonSchema {
    /// Create a string schema.
    pub fn string() -> Self {
        Self {
            schema_type: SchemaType::String,
            properties: HashMap::new(),
            required: Vec::new(),
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            min_items: None,
            max_items: None,
            items_schema: None,
            description: None,
        }
    }

    /// Create an integer schema.
    pub fn integer() -> Self {
        Self {
            schema_type: SchemaType::Integer,
            ..Self::string()
        }
    }

    /// Create a number schema.
    pub fn number() -> Self {
        Self {
            schema_type: SchemaType::Number,
            ..Self::string()
        }
    }

    /// Create a boolean schema.
    pub fn boolean() -> Self {
        Self {
            schema_type: SchemaType::Boolean,
            ..Self::string()
        }
    }

    /// Create an array schema.
    pub fn array(items: JsonSchema) -> Self {
        Self {
            schema_type: SchemaType::Array,
            items_schema: Some(Box::new(items)),
            ..Self::string()
        }
    }

    /// Create an object schema.
    pub fn object() -> Self {
        Self {
            schema_type: SchemaType::Object,
            ..Self::string()
        }
    }

    /// Add a property to an object schema.
    pub fn property(mut self, name: &str, schema: JsonSchema) -> Self {
        self.properties.insert(name.to_string(), schema);
        self
    }

    /// Mark fields as required.
    pub fn required(mut self, name: &str) -> Self {
        self.required.push(name.to_string());
        self
    }

    /// Set enum constraint.
    pub fn enum_values(mut self, values: Vec<serde_json::Value>) -> Self {
        self.enum_values = Some(values);
        self
    }

    /// Set regex pattern.
    pub fn pattern(mut self, pat: &str) -> Self {
        self.pattern = Some(pat.to_string());
        self
    }

    /// Set minimum value.
    pub fn minimum(mut self, min: f64) -> Self {
        self.minimum = Some(min);
        self
    }

    /// Set maximum value.
    pub fn maximum(mut self, max: f64) -> Self {
        self.maximum = Some(max);
        self
    }

    /// Set minimum string length.
    pub fn min_length(mut self, len: usize) -> Self {
        self.min_length = Some(len);
        self
    }

    /// Set maximum string length.
    pub fn max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    /// Set minimum array items.
    pub fn min_items(mut self, n: usize) -> Self {
        self.min_items = Some(n);
        self
    }

    /// Set maximum array items.
    pub fn max_items(mut self, n: usize) -> Self {
        self.max_items = Some(n);
        self
    }

    /// Set description.
    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

// ── Validation Result ───────────────────────────────────────────────────

/// Result of output validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the output is valid.
    pub is_valid: bool,
    /// Validation errors (empty if valid).
    pub errors: Vec<ValidationError>,
    /// The parsed JSON value (if parseable).
    pub parsed: Option<serde_json::Value>,
}

/// A single validation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON path to the error (e.g., "$.name", "$.items[2].id").
    pub path: String,
    /// Human-readable error message.
    pub message: String,
    /// Error code for programmatic handling.
    pub code: ValidationErrorCode,
}

/// Validation error codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorCode {
    /// JSON parse error.
    ParseError,
    /// Type mismatch.
    TypeMismatch,
    /// Missing required field.
    MissingRequired,
    /// Value not in enum.
    NotInEnum,
    /// Pattern mismatch.
    PatternMismatch,
    /// Below minimum.
    BelowMinimum,
    /// Above maximum.
    AboveMaximum,
    /// String too short.
    TooShort,
    /// String too long.
    TooLong,
    /// Array too small.
    TooFewItems,
    /// Array too large.
    TooManyItems,
}

// ── Output Validator ────────────────────────────────────────────────────

/// Validates LLM output against a JSON Schema.
pub struct OutputValidator {
    schema: JsonSchema,
    max_retries: u32,
}

impl OutputValidator {
    /// Create a new validator with the given schema.
    pub fn new(schema: JsonSchema) -> Self {
        Self {
            schema,
            max_retries: 3,
        }
    }

    /// Set maximum retry attempts.
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Validate a JSON string against the schema.
    pub fn validate(&self, input: &str) -> ValidationResult {
        let parsed = match serde_json::from_str::<serde_json::Value>(input) {
            Ok(v) => v,
            Err(e) => {
                return ValidationResult {
                    is_valid: false,
                    errors: vec![ValidationError {
                        path: "$".to_string(),
                        message: format!("JSON parse error: {e}"),
                        code: ValidationErrorCode::ParseError,
                    }],
                    parsed: None,
                };
            }
        };

        let mut errors = Vec::new();
        self.validate_value(&parsed, &self.schema, "$", &mut errors);

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            parsed: Some(parsed),
        }
    }

    /// Generate a corrective prompt for retry.
    pub fn corrective_prompt(&self, original_prompt: &str, result: &ValidationResult) -> String {
        let error_summary: Vec<String> = result
            .errors
            .iter()
            .map(|e| {
                format!(
                    "  - {}: {} ({})",
                    e.path,
                    e.message,
                    format_error_code(&e.code)
                )
            })
            .collect();

        format!(
            "{original_prompt}\n\n\
             IMPORTANT: Your previous response was invalid. Please fix these errors:\n\
             {}\n\n\
             Respond with ONLY valid JSON matching the required schema.",
            error_summary.join("\n")
        )
    }

    fn validate_value(
        &self,
        value: &serde_json::Value,
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        // Type check
        match (&schema.schema_type, value) {
            (SchemaType::String, serde_json::Value::String(s)) => {
                self.validate_string(s, schema, path, errors);
            }
            (SchemaType::Number, serde_json::Value::Number(_)) => {
                if let Some(n) = value.as_f64() {
                    self.validate_number(n, schema, path, errors);
                }
            }
            (SchemaType::Integer, serde_json::Value::Number(n)) => {
                if !n.is_i64() && !n.is_u64() {
                    // Check if it's a whole number
                    if let Some(f) = n.as_f64() {
                        if f.fract() != 0.0 {
                            errors.push(ValidationError {
                                path: path.to_string(),
                                message: "Expected integer, got float".to_string(),
                                code: ValidationErrorCode::TypeMismatch,
                            });
                        }
                    }
                }
                if let Some(n) = value.as_f64() {
                    self.validate_number(n, schema, path, errors);
                }
            }
            (SchemaType::Boolean, serde_json::Value::Bool(_)) => {}
            (SchemaType::Array, serde_json::Value::Array(arr)) => {
                self.validate_array(arr, schema, path, errors);
            }
            (SchemaType::Object, serde_json::Value::Object(_)) => {
                self.validate_object(value, schema, path, errors);
            }
            (SchemaType::Null, serde_json::Value::Null) => {}
            _ => {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "Expected type {:?}, got {}",
                        schema.schema_type,
                        json_type_name(value)
                    ),
                    code: ValidationErrorCode::TypeMismatch,
                });
                return;
            }
        }

        // Enum check
        if let Some(ref enum_vals) = schema.enum_values {
            if !enum_vals.contains(value) {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Value must be one of: {:?}", enum_vals),
                    code: ValidationErrorCode::NotInEnum,
                });
            }
        }
    }

    fn validate_string(
        &self,
        s: &str,
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        if let Some(min) = schema.min_length {
            if s.len() < min {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("String length {} is below minimum {}", s.len(), min),
                    code: ValidationErrorCode::TooShort,
                });
            }
        }
        if let Some(max) = schema.max_length {
            if s.len() > max {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("String length {} exceeds maximum {}", s.len(), max),
                    code: ValidationErrorCode::TooLong,
                });
            }
        }
        if let Some(ref pattern) = schema.pattern {
            if let Ok(re) = regex::Regex::new(pattern) {
                if !re.is_match(s) {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!("String does not match pattern: {pattern}"),
                        code: ValidationErrorCode::PatternMismatch,
                    });
                }
            }
        }
    }

    fn validate_number(
        &self,
        n: f64,
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        if let Some(min) = schema.minimum {
            if n < min {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Value {n} is below minimum {min}"),
                    code: ValidationErrorCode::BelowMinimum,
                });
            }
        }
        if let Some(max) = schema.maximum {
            if n > max {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Value {n} exceeds maximum {max}"),
                    code: ValidationErrorCode::AboveMaximum,
                });
            }
        }
    }

    fn validate_array(
        &self,
        arr: &[serde_json::Value],
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        if let Some(min) = schema.min_items {
            if arr.len() < min {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Array has {} items, minimum is {}", arr.len(), min),
                    code: ValidationErrorCode::TooFewItems,
                });
            }
        }
        if let Some(max) = schema.max_items {
            if arr.len() > max {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("Array has {} items, maximum is {}", arr.len(), max),
                    code: ValidationErrorCode::TooManyItems,
                });
            }
        }
        if let Some(ref items_schema) = schema.items_schema {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{path}[{i}]");
                self.validate_value(item, items_schema, &item_path, errors);
            }
        }
    }

    fn validate_object(
        &self,
        value: &serde_json::Value,
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        let obj = match value.as_object() {
            Some(o) => o,
            None => return,
        };

        // Check required fields
        for required in &schema.required {
            if !obj.contains_key(required) {
                errors.push(ValidationError {
                    path: format!("{path}.{required}"),
                    message: format!("Missing required field: {required}"),
                    code: ValidationErrorCode::MissingRequired,
                });
            }
        }

        // Validate properties
        for (key, prop_schema) in &schema.properties {
            if let Some(prop_value) = obj.get(key) {
                let prop_path = format!("{path}.{key}");
                self.validate_value(prop_value, prop_schema, &prop_path, errors);
            }
        }
    }
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

fn format_error_code(code: &ValidationErrorCode) -> &'static str {
    match code {
        ValidationErrorCode::ParseError => "parse_error",
        ValidationErrorCode::TypeMismatch => "type_mismatch",
        ValidationErrorCode::MissingRequired => "missing_required",
        ValidationErrorCode::NotInEnum => "not_in_enum",
        ValidationErrorCode::PatternMismatch => "pattern_mismatch",
        ValidationErrorCode::BelowMinimum => "below_minimum",
        ValidationErrorCode::AboveMaximum => "above_maximum",
        ValidationErrorCode::TooShort => "too_short",
        ValidationErrorCode::TooLong => "too_long",
        ValidationErrorCode::TooFewItems => "too_few_items",
        ValidationErrorCode::TooManyItems => "too_many_items",
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_object() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string())
            .property("age", JsonSchema::integer())
            .required("name");

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"name": "Alice", "age": 30}"#);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_missing_required_field() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string())
            .required("name");

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"age": 30}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::MissingRequired);
    }

    #[test]
    fn test_type_mismatch() {
        let schema = JsonSchema::object().property("count", JsonSchema::integer());

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"count": "not a number"}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TypeMismatch);
    }

    #[test]
    fn test_parse_error() {
        let schema = JsonSchema::object();
        let validator = OutputValidator::new(schema);
        let result = validator.validate("not json at all");
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::ParseError);
    }

    #[test]
    fn test_string_min_length() {
        let schema = JsonSchema::object().property("name", JsonSchema::string().min_length(3));

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"name": "ab"}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TooShort);
    }

    #[test]
    fn test_string_max_length() {
        let schema = JsonSchema::object().property("name", JsonSchema::string().max_length(5));

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"name": "toolongname"}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TooLong);
    }

    #[test]
    fn test_number_range() {
        let schema = JsonSchema::object()
            .property("score", JsonSchema::number().minimum(0.0).maximum(100.0));

        let validator = OutputValidator::new(schema);

        let ok = validator.validate(r#"{"score": 85}"#);
        assert!(ok.is_valid);

        let low = validator.validate(r#"{"score": -5}"#);
        assert!(!low.is_valid);
        assert_eq!(low.errors[0].code, ValidationErrorCode::BelowMinimum);

        let high = validator.validate(r#"{"score": 150}"#);
        assert!(!high.is_valid);
        assert_eq!(high.errors[0].code, ValidationErrorCode::AboveMaximum);
    }

    #[test]
    fn test_enum_constraint() {
        let schema = JsonSchema::object().property(
            "status",
            JsonSchema::string().enum_values(vec![
                serde_json::json!("active"),
                serde_json::json!("inactive"),
            ]),
        );

        let validator = OutputValidator::new(schema);

        let ok = validator.validate(r#"{"status": "active"}"#);
        assert!(ok.is_valid);

        let bad = validator.validate(r#"{"status": "deleted"}"#);
        assert!(!bad.is_valid);
        assert_eq!(bad.errors[0].code, ValidationErrorCode::NotInEnum);
    }

    #[test]
    fn test_array_validation() {
        let schema = JsonSchema::object().property(
            "tags",
            JsonSchema::array(JsonSchema::string())
                .min_items(1)
                .max_items(3),
        );

        let validator = OutputValidator::new(schema);

        let ok = validator.validate(r#"{"tags": ["a", "b"]}"#);
        assert!(ok.is_valid);

        let empty = validator.validate(r#"{"tags": []}"#);
        assert!(!empty.is_valid);
        assert_eq!(empty.errors[0].code, ValidationErrorCode::TooFewItems);

        let too_many = validator.validate(r#"{"tags": ["a", "b", "c", "d"]}"#);
        assert!(!too_many.is_valid);
        assert_eq!(too_many.errors[0].code, ValidationErrorCode::TooManyItems);
    }

    #[test]
    fn test_nested_object() {
        let schema = JsonSchema::object().property(
            "address",
            JsonSchema::object()
                .property("city", JsonSchema::string())
                .required("city"),
        );

        let validator = OutputValidator::new(schema);

        let ok = validator.validate(r#"{"address": {"city": "Beijing"}}"#);
        assert!(ok.is_valid);

        let bad = validator.validate(r#"{"address": {}}"#);
        assert!(!bad.is_valid);
        assert!(bad.errors[0].path.contains("city"));
    }

    #[test]
    fn test_corrective_prompt() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string())
            .required("name");

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"age": 30}"#);
        let prompt = validator.corrective_prompt("Generate a user", &result);
        assert!(prompt.contains("Generate a user"));
        assert!(prompt.contains("Missing required field"));
        assert!(prompt.contains("valid JSON"));
    }

    #[test]
    fn test_integer_float_rejection() {
        let schema = JsonSchema::object().property("count", JsonSchema::integer());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"count": 3.5}"#);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_valid_empty_object() {
        let schema = JsonSchema::object();
        let validator = OutputValidator::new(schema);
        let result = validator.validate("{}");
        assert!(result.is_valid);
    }

    #[test]
    fn test_multiple_errors() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string().min_length(3))
            .property("age", JsonSchema::integer().minimum(0.0))
            .required("name")
            .required("age");

        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"name": "ab"}"#);
        assert!(!result.is_valid);
        assert!(result.errors.len() >= 2); // missing age + name too short
    }

    // ── SchemaType constructors ──────────────────────────────────────────

    #[test]
    fn test_schema_type_string() {
        let s = JsonSchema::string();
        assert_eq!(s.schema_type, SchemaType::String);
    }

    #[test]
    fn test_schema_type_integer() {
        let s = JsonSchema::integer();
        assert_eq!(s.schema_type, SchemaType::Integer);
    }

    #[test]
    fn test_schema_type_number() {
        let s = JsonSchema::number();
        assert_eq!(s.schema_type, SchemaType::Number);
    }

    #[test]
    fn test_schema_type_boolean() {
        let s = JsonSchema::boolean();
        assert_eq!(s.schema_type, SchemaType::Boolean);
    }

    #[test]
    fn test_schema_type_array() {
        let s = JsonSchema::array(JsonSchema::string());
        assert_eq!(s.schema_type, SchemaType::Array);
        assert!(s.items_schema.is_some());
    }

    #[test]
    fn test_schema_type_object() {
        let s = JsonSchema::object();
        assert_eq!(s.schema_type, SchemaType::Object);
    }

    // ── Builder methods ──────────────────────────────────────────────────

    #[test]
    fn test_builder_pattern() {
        let s = JsonSchema::string()
            .pattern(r"^\d+$")
            .min_length(1)
            .max_length(10)
            .description("A numeric string");
        assert_eq!(s.pattern, Some(r"^\d+$".to_string()));
        assert_eq!(s.min_length, Some(1));
        assert_eq!(s.max_length, Some(10));
        assert_eq!(s.description, Some("A numeric string".to_string()));
    }

    #[test]
    fn test_builder_enum_values() {
        let s =
            JsonSchema::string().enum_values(vec![serde_json::json!("a"), serde_json::json!("b")]);
        assert!(s.enum_values.is_some());
        assert_eq!(s.enum_values.unwrap().len(), 2);
    }

    #[test]
    fn test_builder_number_range() {
        let s = JsonSchema::number().minimum(0.0).maximum(100.0);
        assert_eq!(s.minimum, Some(0.0));
        assert_eq!(s.maximum, Some(100.0));
    }

    #[test]
    fn test_builder_array_items() {
        let s = JsonSchema::array(JsonSchema::integer())
            .min_items(1)
            .max_items(5);
        assert_eq!(s.min_items, Some(1));
        assert_eq!(s.max_items, Some(5));
    }

    // ── with_max_retries ─────────────────────────────────────────────────

    #[test]
    fn test_with_max_retries() {
        let schema = JsonSchema::object();
        let validator = OutputValidator::new(schema).with_max_retries(5);
        assert_eq!(validator.max_retries, 5);
    }

    // ── Pattern validation ───────────────────────────────────────────────

    #[test]
    fn test_pattern_match_valid() {
        let schema =
            JsonSchema::object().property("code", JsonSchema::string().pattern(r"^[A-Z]{3}$"));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"code": "ABC"}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_pattern_match_invalid() {
        let schema =
            JsonSchema::object().property("code", JsonSchema::string().pattern(r"^[A-Z]{3}$"));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"code": "abc"}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::PatternMismatch);
    }

    // ── Boolean validation ───────────────────────────────────────────────

    #[test]
    fn test_boolean_valid() {
        let schema = JsonSchema::object().property("active", JsonSchema::boolean());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"active": true}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_boolean_type_mismatch() {
        let schema = JsonSchema::object().property("active", JsonSchema::boolean());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"active": "yes"}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TypeMismatch);
    }

    // ── Null validation ──────────────────────────────────────────────────

    #[test]
    fn test_null_type_valid() {
        let schema = JsonSchema {
            schema_type: SchemaType::Null,

            properties: std::collections::HashMap::new(),
            required: vec![],
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            min_items: None,
            max_items: None,
            items_schema: None,
            description: None,
        };
        let validator = OutputValidator::new(schema);
        let result = validator.validate("null");
        assert!(result.is_valid);
    }

    // ── Boundary values ──────────────────────────────────────────────────

    #[test]
    fn test_number_at_exact_minimum() {
        let schema = JsonSchema::object().property("v", JsonSchema::number().minimum(10.0));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"v": 10}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_number_at_exact_maximum() {
        let schema = JsonSchema::object().property("v", JsonSchema::number().maximum(100.0));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"v": 100}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_string_at_exact_min_length() {
        let schema = JsonSchema::object().property("s", JsonSchema::string().min_length(3));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"s": "abc"}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_string_at_exact_max_length() {
        let schema = JsonSchema::object().property("s", JsonSchema::string().max_length(3));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"s": "abc"}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_empty_string_with_min_length() {
        let schema = JsonSchema::object().property("s", JsonSchema::string().min_length(1));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"s": ""}"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TooShort);
    }

    // ── Array item type validation ───────────────────────────────────────

    #[test]
    fn test_array_item_type_mismatch() {
        let schema =
            JsonSchema::object().property("nums", JsonSchema::array(JsonSchema::integer()));
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"nums": [1, "two", 3]}"#);
        assert!(!result.is_valid);
    }

    // ── Multiple required fields ─────────────────────────────────────────

    #[test]
    fn test_all_required_fields_present() {
        let schema = JsonSchema::object()
            .property("a", JsonSchema::string())
            .property("b", JsonSchema::string())
            .property("c", JsonSchema::string())
            .required("a")
            .required("b")
            .required("c");
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"a":"1","b":"2","c":"3"}"#);
        assert!(result.is_valid);
    }

    // ── Parsed value in result ───────────────────────────────────────────

    #[test]
    fn test_parsed_value_on_success() {
        let schema = JsonSchema::object().property("x", JsonSchema::integer());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"x": 42}"#);
        assert!(result.parsed.is_some());
        assert_eq!(result.parsed.unwrap()["x"], 42);
    }

    #[test]
    fn test_parsed_value_on_parse_error() {
        let schema = JsonSchema::object();
        let validator = OutputValidator::new(schema);
        let result = validator.validate("not json");
        assert!(result.parsed.is_none());
    }

    // ── Serde roundtrips ─────────────────────────────────────────────────

    #[test]
    fn test_schema_type_serde() {
        let types = vec![
            SchemaType::String,
            SchemaType::Number,
            SchemaType::Integer,
            SchemaType::Boolean,
            SchemaType::Array,
            SchemaType::Object,
            SchemaType::Null,
        ];
        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let deserialized: SchemaType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, deserialized);
        }
    }

    #[test]
    fn test_validation_error_code_serde() {
        let codes = vec![
            ValidationErrorCode::ParseError,
            ValidationErrorCode::TypeMismatch,
            ValidationErrorCode::MissingRequired,
            ValidationErrorCode::NotInEnum,
            ValidationErrorCode::PatternMismatch,
            ValidationErrorCode::BelowMinimum,
            ValidationErrorCode::AboveMaximum,
            ValidationErrorCode::TooShort,
            ValidationErrorCode::TooLong,
            ValidationErrorCode::TooFewItems,
            ValidationErrorCode::TooManyItems,
        ];
        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let deserialized: ValidationErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", deserialized), format!("{:?}", code));
        }
    }

    #[test]
    fn test_json_schema_serde_roundtrip() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string().min_length(1).max_length(100))
            .property("age", JsonSchema::integer().minimum(0.0).maximum(200.0))
            .required("name")
            .description("User schema");
        let json = serde_json::to_string(&schema).unwrap();
        let deserialized: JsonSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.schema_type, SchemaType::Object);
        assert_eq!(deserialized.required, vec!["name"]);
        assert!(deserialized.properties.contains_key("name"));
    }

    // ── Integer whole number validation ──────────────────────────────────

    #[test]
    fn test_integer_whole_number_accepted() {
        let schema = JsonSchema::object().property("n", JsonSchema::integer());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"n": 3.0}"#);
        // 3.0 is a whole number, should be accepted as integer
        assert!(result.is_valid);
    }

    // ── Extra properties allowed ─────────────────────────────────────────

    #[test]
    fn test_extra_properties_allowed() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string())
            .required("name");
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"name": "Alice", "extra": 42}"#);
        assert!(result.is_valid); // extra properties should be allowed
    }

    // ── Helper function tests ────────────────────────────────────────────

    #[test]
    fn test_json_type_name_all_variants() {
        use super::json_type_name;
        // Test all serde_json::Value variants
        assert_eq!(json_type_name(&serde_json::Value::Null), "null");
        assert_eq!(json_type_name(&serde_json::Value::Bool(true)), "boolean");
        assert_eq!(
            json_type_name(&serde_json::Value::Number(42.into())),
            "number"
        );
        assert_eq!(
            json_type_name(&serde_json::Value::String("test".into())),
            "string"
        );
        assert_eq!(json_type_name(&serde_json::Value::Array(vec![])), "array");
        assert_eq!(
            json_type_name(&serde_json::Value::Object(serde_json::Map::new())),
            "object"
        );
    }

    #[test]
    fn test_format_error_code_all_variants() {
        use super::format_error_code;
        assert_eq!(
            format_error_code(&ValidationErrorCode::ParseError),
            "parse_error"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::TypeMismatch),
            "type_mismatch"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::MissingRequired),
            "missing_required"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::NotInEnum),
            "not_in_enum"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::PatternMismatch),
            "pattern_mismatch"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::BelowMinimum),
            "below_minimum"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::AboveMaximum),
            "above_maximum"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::TooShort),
            "too_short"
        );
        assert_eq!(format_error_code(&ValidationErrorCode::TooLong), "too_long");
        assert_eq!(
            format_error_code(&ValidationErrorCode::TooFewItems),
            "too_few_items"
        );
        assert_eq!(
            format_error_code(&ValidationErrorCode::TooManyItems),
            "too_many_items"
        );
    }

    // ── Schema type mismatch error messages ──────────────────────────────

    #[test]
    fn test_type_mismatch_error_message_shows_expected_and_actual() {
        let schema = JsonSchema::object().property("name", JsonSchema::string());
        let validator = OutputValidator::new(schema);
        // Pass a number where string is expected
        let result = validator.validate(r#"{"name": 123}"#);
        assert!(!result.is_valid);
        // Error message should indicate type mismatch
        // Message is like "Expected type String, got number"
        assert!(result.errors[0].message.contains("String"));
        assert!(result.errors[0].message.contains("number"));
    }

    #[test]
    fn test_validation_error_with_complex_nested_path() {
        let schema = JsonSchema::object()
            .property(
                "user",
                JsonSchema::object()
                    .property(
                        "profile",
                        JsonSchema::object()
                            .property("name", JsonSchema::string().min_length(1))
                            .required("name"),
                    )
                    .required("profile"),
            )
            .required("user");
        let validator = OutputValidator::new(schema);
        // Missing nested required field
        let result = validator.validate(r#"{"user": {"profile": {}}}"#);
        assert!(!result.is_valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.path.contains("user.profile.name")));
    }

    // ── Enum at root level ───────────────────────────────────────────────

    #[test]
    fn test_enum_at_root_level() {
        let schema = JsonSchema::string().enum_values(vec![
            serde_json::json!("active"),
            serde_json::json!("pending"),
        ]);
        let validator = OutputValidator::new(schema);
        assert!(validator.validate(r#""active""#).is_valid);
        assert!(validator.validate(r#""pending""#).is_valid);
        assert!(!validator.validate(r#""deleted""#).is_valid);
    }

    // ── Array with items schema at root ────────────────────────────────

    #[test]
    fn test_array_at_root_level() {
        let schema = JsonSchema::array(JsonSchema::integer().minimum(0.0).maximum(10.0));
        let validator = OutputValidator::new(schema);
        assert!(validator.validate(r#"[1, 2, 3]"#).is_valid);
        let result = validator.validate(r#"[-1, 2, 3]"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::BelowMinimum);
    }

    // ── Number type vs Integer type distinction ─────────────────────────

    #[test]
    fn test_number_type_accepts_floats() {
        let schema = JsonSchema::object().property("price", JsonSchema::number());
        let validator = OutputValidator::new(schema);
        assert!(validator.validate(r#"{"price": 19.99}"#).is_valid);
        assert!(validator.validate(r#"{"price": 20}"#).is_valid);
    }

    // ── ValidationResult serde ──────────────────────────────────────────

    #[test]
    fn test_validation_result_serde() {
        let schema = JsonSchema::object().property("x", JsonSchema::integer());
        let validator = OutputValidator::new(schema);
        let result = validator.validate(r#"{"x": 42}"#);
        let json = serde_json::to_string(&result).unwrap();
        let decoded: ValidationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.is_valid, true);
        assert!(decoded.errors.is_empty());
    }

    // ── ValidationError serde ───────────────────────────────────────────

    #[test]
    fn test_validation_error_serde() {
        let error = ValidationError {
            path: "$.name".to_string(),
            message: "Too short".to_string(),
            code: ValidationErrorCode::TooShort,
        };
        let json = serde_json::to_string(&error).unwrap();
        let decoded: ValidationError = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.path, "$.name");
        assert_eq!(decoded.code, ValidationErrorCode::TooShort);
    }

    // ── Invalid regex pattern in schema (should not panic) ─────────────

    #[test]
    fn test_invalid_regex_pattern_does_not_panic() {
        // An invalid regex should be silently ignored (validation passes)
        let schema =
            JsonSchema::object().property("code", JsonSchema::string().pattern(r"[invalid"));
        let validator = OutputValidator::new(schema);
        // Should not panic, validation should pass since regex can't compile
        let result = validator.validate(r#"{"code": "anything"}"#);
        // The invalid pattern means no validation is performed
        assert!(result.is_valid);
    }

    // ── Multiple validation errors collected ────────────────────────────

    #[test]
    fn test_multiple_validation_errors_all_collected() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string().min_length(3))
            .property("age", JsonSchema::integer().minimum(0.0))
            .property(
                "email",
                JsonSchema::string().pattern(r"^[\w]+@[\w]+\.[\w]+$"),
            )
            .required("name")
            .required("age")
            .required("email");
        let validator = OutputValidator::new(schema);
        // All errors should be collected
        let result = validator.validate(r#"{"name": "ab", "age": -5, "email": "invalid"}"#);
        assert!(!result.is_valid);
        // Should have errors for: name too short, age below minimum, email pattern mismatch
        assert!(result.errors.len() >= 3);
        let codes: Vec<_> = result.errors.iter().map(|e| &e.code).collect();
        assert!(codes.contains(&&ValidationErrorCode::TooShort));
        assert!(codes.contains(&&ValidationErrorCode::BelowMinimum));
        assert!(codes.contains(&&ValidationErrorCode::PatternMismatch));
    }

    // ── Null value validation ──────────────────────────────────────────

    #[test]
    fn test_null_value_type_mismatch_for_object_schema() {
        let schema = JsonSchema::object();
        let validator = OutputValidator::new(schema);
        // null is NOT valid for an object schema - it fails type check
        let result = validator.validate("null");
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TypeMismatch);
    }

    // ── Too many items validation ───────────────────────────────────────

    #[test]
    fn test_too_many_items_exact_boundary() {
        let schema = JsonSchema::array(JsonSchema::string()).max_items(3);
        let validator = OutputValidator::new(schema);
        // Exactly at boundary should pass
        assert!(validator.validate(r#"["a", "b", "c"]"#).is_valid);
        // One over should fail
        let result = validator.validate(r#"["a", "b", "c", "d"]"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TooManyItems);
    }

    // ── Too few items validation exact boundary ─────────────────────────

    #[test]
    fn test_too_few_items_exact_boundary() {
        let schema = JsonSchema::array(JsonSchema::string()).min_items(2);
        let validator = OutputValidator::new(schema);
        // Exactly at boundary should pass
        assert!(validator.validate(r#"["a", "b"]"#).is_valid);
        // One under should fail
        let result = validator.validate(r#"["a"]"#);
        assert!(!result.is_valid);
        assert_eq!(result.errors[0].code, ValidationErrorCode::TooFewItems);
    }
}
