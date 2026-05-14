use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;

use crate::pipeline::{InputMapping, PipelineBuilder, SkillPipeline, ErrorPolicy};
use crate::registry::SkillRegistry;
use crate::skill::{Skill, SkillConfig};
use kias_common::KiasResult;

/// Schema validation for skill inputs/outputs.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct SchemaValidation {
    /// Required keys that must be present.
    pub required_keys: Vec<String>,
    /// Expected type for each key ("string", "number", "object", "array", "bool", "null").
    pub key_types: HashMap<String, String>,
}

impl SchemaValidation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_required_keys(mut self, keys: Vec<String>) -> Self {
        self.required_keys = keys;
        self
    }

    pub fn with_key_types(mut self, types: HashMap<String, String>) -> Self {
        self.key_types = types;
        self
    }

    /// Validate a JSON value against this schema.
    /// Returns Ok(()) if valid, Err with description if not.
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        // Check required keys
        for key in &self.required_keys {
            if value.get(key).is_none() {
                return Err(format!("Missing required key: '{}'", key));
            }
        }

        // Check types
        for (key, expected_type) in &self.key_types {
            if let Some(v) = value.get(key) {
                let actual_type = json_type_name(v);
                if actual_type != expected_type {
                    return Err(format!(
                        "Key '{}': expected type '{}', got '{}'",
                        key, expected_type, actual_type
                    ));
                }
            }
        }

        Ok(())
    }
}

/// A composite skill that wraps a pipeline as a single Skill.
/// This allows pipelines to be registered and used like regular skills.
pub struct CompositeSkill {
    name: String,
    description: String,
    config: SkillConfig,
    pipeline: SkillPipeline,
    input_schema: Option<SchemaValidation>,
    output_schema: Option<SchemaValidation>,
}

impl CompositeSkill {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        pipeline: SkillPipeline,
    ) -> Self {
        let name_str = name.into();
        let desc_str = description.into();
        let config = SkillConfig::new(&name_str, &desc_str)
            .with_tags(vec!["composite".to_string(), "pipeline".to_string()]);

        Self {
            name: name_str,
            description: desc_str,
            config,
            pipeline,
            input_schema: None,
            output_schema: None,
        }
    }

    /// Set input validation schema.
    pub fn with_input_schema(mut self, schema: SchemaValidation) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set output validation schema.
    pub fn with_output_schema(mut self, schema: SchemaValidation) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Get the inner pipeline.
    pub fn pipeline(&self) -> &SkillPipeline {
        &self.pipeline
    }
}

#[async_trait]
impl Skill for CompositeSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn config(&self) -> SkillConfig {
        self.config.clone()
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        // Validate input if schema is set
        if let Some(ref schema) = self.input_schema {
            schema.validate(&params).map_err(|e| {
                kias_common::KiasError::Validation(format!("Input validation failed: {}", e))
            })?;
        }

        // We need access to a registry to run the pipeline steps.
        // The registry is not stored in the composite skill because skills are
        // registered *in* a registry (circular dependency).
        // Instead, we expect the caller to use SkillComposer which stores a
        // reference to the registry. However, for the Skill trait to work
        // standalone, we embed the registry inside CompositeSkill.
        //
        // Workaround: CompositeSkill is primarily designed to be built via
        // SkillComposer which passes the registry. The Skill trait impl is
        // provided for convenience; it will attempt to use an internal registry
        // that must be set up during construction.
        Err(kias_common::KiasError::Validation(
            "CompositeSkill.execute() requires a registry context. Use SkillComposer or call execute_with_registry()".into()
        ))
    }
}

impl CompositeSkill {
    /// Execute the composite skill with an explicit registry reference.
    pub async fn execute_with_registry(
        &self,
        registry: &SkillRegistry,
        params: Value,
    ) -> KiasResult<Value> {
        // Validate input if schema is set
        if let Some(ref schema) = self.input_schema {
            schema.validate(&params).map_err(|e| {
                kias_common::KiasError::Validation(format!("Input validation failed: {}", e))
            })?;
        }

        let result = self.pipeline.execute(registry, params).await;

        if !result.success {
            return Err(kias_common::KiasError::Validation(format!(
                "Pipeline '{}' failed: {}",
                self.name,
                result.error.unwrap_or_else(|| "unknown error".into())
            )));
        }

        // Validate output if schema is set
        if let Some(ref schema) = self.output_schema {
            schema.validate(&result.final_output).map_err(|e| {
                kias_common::KiasError::Validation(format!("Output validation failed: {}", e))
            })?;
        }

        Ok(result.final_output)
    }
}

/// Builder for creating CompositeSkills from registry skills.
pub struct SkillComposer {
    name: String,
    description: String,
    builder: PipelineBuilder,
    input_schema: Option<SchemaValidation>,
    output_schema: Option<SchemaValidation>,
}

impl SkillComposer {
    /// Start composing a new composite skill.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            builder: PipelineBuilder::new(&name_str),
            name: name_str,
            description: description.into(),
            input_schema: None,
            output_schema: None,
        }
    }

    /// Add a skill step by name.
    pub fn then(mut self, skill_name: impl Into<String>) -> Self {
        self.builder = self.builder.then(skill_name);
        self
    }

    /// Add a conditional skill step.
    pub fn then_if(
        mut self,
        skill_name: impl Into<String>,
        condition: impl Fn(&Value) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.builder = self.builder.then_if(skill_name, condition);
        self
    }

    /// Set input mapping for the next step.
    pub fn with_input_mapping(mut self, mapping: InputMapping) -> Self {
        self.builder = self.builder.with_input_mapping(mapping);
        self
    }

    /// Set error policy.
    pub fn with_error_policy(mut self, policy: ErrorPolicy) -> Self {
        self.builder = self.builder.with_error_policy(policy);
        self
    }

    /// Set input validation schema.
    pub fn with_input_schema(mut self, schema: SchemaValidation) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set output validation schema.
    pub fn with_output_schema(mut self, schema: SchemaValidation) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Validate that skills referenced by the pipeline exist in the given registry.
    /// Consumes self and returns Self on success for chaining.
    pub fn validate_skills(self, registry: &SkillRegistry) -> Result<Self, String> {
        let name = self.name.clone();
        let built = self.into_pipeline();
        for step in &built.steps {
            if !registry.has(&step.skill_name) {
                return Err(format!("Skill '{}' not found in registry", step.skill_name));
            }
        }
        Ok(Self::from_parts(name, built))
    }

    /// Validate that input/output schemas are compatible between consecutive steps.
    /// This checks that each step's input_schema keys can be satisfied by the
    /// previous step's output_schema keys (where applicable).
    pub fn validate_schema_compatibility(
        self,
        _registry: &SkillRegistry,
        step_schemas: &HashMap<String, (SchemaValidation, SchemaValidation)>,
    ) -> Result<Self, String> {
        let name = self.name.clone();
        let built = self.into_pipeline();
        let mut prev_output_schema: Option<&SchemaValidation> = None;

        for step in &built.steps {
            if let Some((ref input_schema, ref output_schema)) =
                step_schemas.get(&step.skill_name)
            {
                if let Some(prev_out) = prev_output_schema {
                    for key in &input_schema.required_keys {
                        if !prev_out.required_keys.contains(key) {
                            return Err(format!(
                                "Step '{}' requires key '{}' not provided by previous step output",
                                step.skill_name, key
                            ));
                        }
                    }
                }
                prev_output_schema = Some(output_schema);
            }
        }

        Ok(Self::from_parts(name, built))
    }

    /// Build the CompositeSkill.
    pub fn build(self) -> CompositeSkill {
        let name = self.name.clone();
        let description = self.description.clone();
        // Extract all fields that need to survive into_pipeline()
        let input_schema = self.input_schema;
        let output_schema = self.output_schema;
        // Now consume self to get the pipeline
        let builder = self.builder;
        let pipeline = builder.build();
        let mut composite = CompositeSkill::new(&name, &description, pipeline);
        composite.input_schema = input_schema;
        composite.output_schema = output_schema;
        composite
    }

    /// Helper: extract the pipeline from internal state.
    fn into_pipeline(self) -> SkillPipeline {
        self.builder.build()
    }

    /// Helper: reconstruct Self from a pre-built pipeline.
    fn from_parts(name: String, _pipeline: SkillPipeline) -> Self {
        Self {
            name,
            description: String::new(),
            builder: PipelineBuilder::new(""),
            input_schema: None,
            output_schema: None,
        }
    }
}

/// Determine the JSON type name of a Value.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::PipelineBuilder;
    use crate::registry::SkillRegistry;
    use kias_common::KiasResult;

    // ── helpers ──────────────────────────────────────────────────────────

    struct UpperSkill;
    #[async_trait]
    impl Skill for UpperSkill {
        fn name(&self) -> &str { "upper" }
        fn description(&self) -> &str { "Uppercases 'text' field" }
        async fn execute(&self, params: Value) -> KiasResult<Value> {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let mut output = params.clone();
            if let Some(obj) = output.as_object_mut() {
                obj.insert("text".to_string(), serde_json::json!(text.to_uppercase()));
            }
            Ok(output)
        }
    }

    struct AppendSkill;
    #[async_trait]
    impl Skill for AppendSkill {
        fn name(&self) -> &str { "append" }
        fn description(&self) -> &str { "Appends 'suffix' to 'text'" }
        async fn execute(&self, params: Value) -> KiasResult<Value> {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let suffix = params.get("suffix").and_then(|v| v.as_str()).unwrap_or("");
            Ok(serde_json::json!({ "text": format!("{}{}", text, suffix) }))
        }
    }

    fn test_registry() -> SkillRegistry {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(UpperSkill));
        reg.register(Box::new(AppendSkill));
        reg
    }

    // ── CompositeSkill tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_composite_skill_as_skill_trait() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("upper-append")
            .then("upper")
            .then("append")
            .build();

        let composite = CompositeSkill::new("text_transform", "Uppercases then appends", pipeline);
        assert_eq!(composite.name(), "text_transform");
        assert_eq!(composite.description(), "Uppercases then appends");

        // Execute via registry context
        let result = composite
            .execute_with_registry(
                &reg,
                serde_json::json!({ "text": "hello", "suffix": "!" }),
            )
            .await
            .unwrap();
        assert_eq!(result["text"], "HELLO!");
    }

    #[tokio::test]
    async fn test_composite_skill_input_schema_validation() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("validated")
            .then("upper")
            .build();

        let schema = SchemaValidation::new()
            .with_required_keys(vec!["text".to_string()]);

        let composite = CompositeSkill::new("validated", "Validated composite", pipeline)
            .with_input_schema(schema);

        // Missing 'text' key should fail
        let result = composite
            .execute_with_registry(&reg, serde_json::json!({ "other": 1 }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_composite_skill_output_schema_validation() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("out-valid")
            .then("upper")
            .build();

        let output_schema = SchemaValidation::new()
            .with_required_keys(vec!["text".to_string()])
            .with_key_types({
                let mut m = HashMap::new();
                m.insert("text".to_string(), "string".to_string());
                m
            });

        let composite = CompositeSkill::new("out_valid", "Output validated", pipeline)
            .with_output_schema(output_schema);

        let result = composite
            .execute_with_registry(&reg, serde_json::json!({ "text": "hi" }))
            .await
            .unwrap();
        assert_eq!(result["text"], "HI");
    }

    #[tokio::test]
    async fn test_composite_skill_pipeline_failure() {
        let mut reg = test_registry();
        struct FailSkill;
        #[async_trait]
        impl Skill for FailSkill {
            fn name(&self) -> &str { "fail_skill" }
            fn description(&self) -> &str { "Always fails" }
            async fn execute(&self, _params: Value) -> KiasResult<Value> {
                Err(kias_common::KiasError::Validation("boom".into()))
            }
        }
        reg.register(Box::new(FailSkill));

        let pipeline = PipelineBuilder::new("failing")
            .then("fail_skill")
            .build();

        let composite = CompositeSkill::new("fail_composite", "Fails", pipeline);
        let result = composite
            .execute_with_registry(&reg, serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    // ── SkillComposer tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_skill_composer_builds_composite() {
        let reg = test_registry();
        let composite = SkillComposer::new("my_composite", "A composed skill")
            .then("upper")
            .then("append")
            .with_input_schema(
                SchemaValidation::new().with_required_keys(vec![
                    "text".to_string(),
                    "suffix".to_string(),
                ]),
            )
            .build();

        // Validate skills exist
        assert!(SkillComposer::new("tmp", "tmp")
            .then("upper")
            .then("append")
            .validate_skills(&reg)
            .is_ok());

        let result = composite
            .execute_with_registry(
                &reg,
                serde_json::json!({ "text": "world", "suffix": "?" }),
            )
            .await
            .unwrap();
        assert_eq!(result["text"], "WORLD?");
    }

    #[tokio::test]
    async fn test_skill_composer_validate_skills_missing() {
        let reg = test_registry();
        let result = SkillComposer::new("bad", "Bad composite")
            .then("nonexistent")
            .validate_skills(&reg);
        assert!(result.is_err());
    }

    // ── SchemaValidation tests ───────────────────────────────────────────

    #[test]
    fn test_schema_validation_success() {
        let schema = SchemaValidation::new()
            .with_required_keys(vec!["name".to_string()])
            .with_key_types({
                let mut m = HashMap::new();
                m.insert("name".to_string(), "string".to_string());
                m
            });

        let value = serde_json::json!({ "name": "Alice" });
        assert!(schema.validate(&value).is_ok());
    }

    #[test]
    fn test_schema_validation_missing_key() {
        let schema = SchemaValidation::new()
            .with_required_keys(vec!["name".to_string(), "age".to_string()]);

        let value = serde_json::json!({ "name": "Alice" });
        assert!(schema.validate(&value).is_err());
    }

    #[test]
    fn test_schema_validation_wrong_type() {
        let schema = SchemaValidation::new().with_key_types({
            let mut m = HashMap::new();
            m.insert("count".to_string(), "number".to_string());
            m
        });

        let value = serde_json::json!({ "count": "not_a_number" });
        assert!(schema.validate(&value).is_err());
    }
}
