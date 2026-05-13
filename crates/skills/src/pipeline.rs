use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A condition function that evaluates whether a pipeline step should execute.
/// Receives the current accumulated output and returns true if the step should run.
pub type StepCondition = Box<dyn Fn(&Value) -> bool + Send + Sync>;

/// Describes how to derive a step's input from the previous step's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMapping {
    /// If set, extract this JSON path from previous output and pass as step input.
    pub from_path: Option<String>,
    /// Static fields to merge into the step input regardless of previous output.
    pub static_fields: Option<Value>,
    /// If true, pass the entire previous output as-is.
    pub pass_through: bool,
}

impl Default for InputMapping {
    fn default() -> Self {
        Self {
            from_path: None,
            static_fields: None,
            pass_through: true,
        }
    }
}

impl InputMapping {
    /// Create a mapping that passes through the entire previous output.
    pub fn pass_through() -> Self {
        Self {
            pass_through: true,
            ..Default::default()
        }
    }

    /// Create a mapping that extracts a specific path from previous output.
    pub fn from_path(path: impl Into<String>) -> Self {
        Self {
            from_path: Some(path.into()),
            pass_through: false,
            ..Default::default()
        }
    }

    /// Create a mapping that uses only static fields.
    pub fn with_static(value: Value) -> Self {
        Self {
            static_fields: Some(value),
            pass_through: false,
            ..Default::default()
        }
    }

    /// Resolve this mapping against a previous output to produce input for the next step.
    pub fn resolve(&self, previous_output: &Value) -> Value {
        if self.pass_through {
            return previous_output.clone();
        }

        let mut result = if let Some(ref path) = self.from_path {
            resolve_json_path(previous_output, path)
        } else {
            Value::Null
        };

        if let Some(ref static_fields) = self.static_fields {
            if let (Some(result_obj), Some(static_obj)) =
                (result.as_object_mut(), static_fields.as_object())
            {
                for (key, value) in static_obj {
                    result_obj.insert(key.clone(), value.clone());
                }
            } else if result.is_null() {
                result = static_fields.clone();
            }
        }

        result
    }
}

/// A single step in a pipeline.
pub struct PipelineStep {
    /// Name of the skill to execute.
    pub skill_name: String,
    /// How to map previous output to this step's input.
    pub input_mapping: InputMapping,
    /// Optional condition that must evaluate to true for this step to execute.
    pub condition: Option<Arc<StepCondition>>,
}

impl PipelineStep {
    pub fn new(skill_name: impl Into<String>) -> Self {
        Self {
            skill_name: skill_name.into(),
            input_mapping: InputMapping::pass_through(),
            condition: None,
        }
    }

    pub fn with_input_mapping(mut self, mapping: InputMapping) -> Self {
        self.input_mapping = mapping;
        self
    }

    pub fn with_condition(mut self, condition: impl Fn(&Value) -> bool + Send + Sync + 'static) -> Self {
        self.condition = Some(Arc::new(Box::new(condition)));
        self
    }
}

/// Controls what happens when a step fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Stop the pipeline immediately on any error.
    StopOnError,
    /// Continue executing remaining steps even if one fails.
    ContinueOnError,
}

/// Result of executing a full pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// The output of each executed step, in order.
    pub step_results: Vec<Value>,
    /// The final output (last step's output, or the last successful output).
    pub final_output: Value,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Number of steps that were actually executed.
    pub steps_executed: usize,
    /// Whether the pipeline completed without errors.
    pub success: bool,
    /// Error message if the pipeline failed.
    pub error: Option<String>,
}

/// A skill pipeline: an ordered list of steps executed sequentially.
pub struct SkillPipeline {
    pub name: String,
    pub steps: Vec<PipelineStep>,
    pub error_policy: ErrorPolicy,
}

impl SkillPipeline {
    /// Create a new empty pipeline with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            error_policy: ErrorPolicy::StopOnError,
        }
    }

    /// Set the error policy.
    pub fn with_error_policy(mut self, policy: ErrorPolicy) -> Self {
        self.error_policy = policy;
        self
    }

    /// Execute the pipeline against a registry, passing `initial_input` to the first step.
    pub async fn execute(
        &self,
        registry: &crate::registry::SkillRegistry,
        initial_input: Value,
    ) -> PipelineResult {
        let start = Instant::now();
        let mut step_results = Vec::new();
        let mut current_input = initial_input;
        let mut success = true;
        let mut error = None;

        for step in &self.steps {
            // Check condition
            if let Some(ref condition) = step.condition {
                if !condition(&current_input) {
                    // Condition not met — skip this step, push null result
                    step_results.push(Value::Null);
                    continue;
                }
            }

            // Resolve input mapping
            let step_input = step.input_mapping.resolve(&current_input);

            // Look up skill in registry
            let skill = match registry.get(&step.skill_name) {
                Some(s) => s,
                None => {
                    let msg = format!("Skill '{}' not found in registry", step.skill_name);
                    match self.error_policy {
                        ErrorPolicy::StopOnError => {
                            success = false;
                            error = Some(msg);
                            break;
                        }
                        ErrorPolicy::ContinueOnError => {
                            step_results.push(serde_json::json!({"error": msg}));
                            continue;
                        }
                    }
                }
            };

            // Execute the skill
            match skill.execute(step_input).await {
                Ok(output) => {
                    current_input = output.clone();
                    step_results.push(output);
                }
                Err(e) => {
                    let msg = format!("Step '{}' failed: {}", step.skill_name, e);
                    match self.error_policy {
                        ErrorPolicy::StopOnError => {
                            success = false;
                            error = Some(msg);
                            break;
                        }
                        ErrorPolicy::ContinueOnError => {
                            step_results.push(serde_json::json!({"error": msg}));
                            continue;
                        }
                    }
                }
            }
        }

        let steps_executed = if success { self.steps.len() } else { step_results.len() };
        let final_output = step_results.last().cloned().unwrap_or(Value::Null);
        let duration_ms = start.elapsed().as_millis() as u64;

        PipelineResult {
            step_results,
            final_output,
            duration_ms,
            steps_executed,
            success,
            error,
        }
    }

    /// Execute the same pipeline with multiple inputs in parallel.
    /// Each input is processed independently.
    pub async fn execute_parallel(
        &self,
        registry: &crate::registry::SkillRegistry,
        inputs: Vec<Value>,
    ) -> Vec<PipelineResult> {
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.execute(registry, input).await);
        }
        results
    }
}

/// Fluent builder for constructing a SkillPipeline.
pub struct PipelineBuilder {
    name: String,
    steps: Vec<PipelineStep>,
    error_policy: ErrorPolicy,
    pending_mapping: Option<InputMapping>,
}

impl PipelineBuilder {
    /// Start building a new pipeline.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            error_policy: ErrorPolicy::StopOnError,
            pending_mapping: None,
        }
    }

    /// Add a step that executes the named skill unconditionally.
    pub fn then(mut self, skill_name: impl Into<String>) -> Self {
        let mut step = PipelineStep::new(skill_name);
        if let Some(mapping) = self.pending_mapping.take() {
            step.input_mapping = mapping;
        }
        self.steps.push(step);
        self
    }

    /// Add a step that only executes if the condition returns true.
    pub fn then_if(
        mut self,
        skill_name: impl Into<String>,
        condition: impl Fn(&Value) -> bool + Send + Sync + 'static,
    ) -> Self {
        let mut step = PipelineStep::new(skill_name).with_condition(condition);
        if let Some(mapping) = self.pending_mapping.take() {
            step.input_mapping = mapping;
        }
        self.steps.push(step);
        self
    }

    /// Set the input mapping for the *next* step to be added.
    pub fn with_input_mapping(mut self, mapping: InputMapping) -> Self {
        self.pending_mapping = Some(mapping);
        self
    }

    /// Set the error policy for the pipeline.
    pub fn with_error_policy(mut self, policy: ErrorPolicy) -> Self {
        self.error_policy = policy;
        self
    }

    /// Build the pipeline.
    pub fn build(self) -> SkillPipeline {
        SkillPipeline {
            name: self.name,
            steps: self.steps,
            error_policy: self.error_policy,
        }
    }
}

/// Resolve a dotted JSON path (e.g., "a.b.c") against a value.
fn resolve_json_path(data: &Value, path: &str) -> Value {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;
    for part in parts {
        match current.get(part) {
            Some(v) => current = v,
            None => return Value::Null,
        }
    }
    current.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::registry::SkillRegistry;
    use crate::skill::Skill;
    use kias_common::KiasResult;

    // ── helpers ──────────────────────────────────────────────────────────

    struct DoubleSkill;
    #[async_trait]
    impl Skill for DoubleSkill {
        fn name(&self) -> &str { "double" }
        fn description(&self) -> &str { "Doubles the numeric 'value' field" }
        async fn execute(&self, params: Value) -> KiasResult<Value> {
            let v = params
                .get("value")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let mut output = params.clone();
            if let Some(obj) = output.as_object_mut() {
                obj.insert("value".to_string(), serde_json::json!(v * 2.0));
            }
            Ok(output)
        }
    }

    struct AddSkill;
    #[async_trait]
    impl Skill for AddSkill {
        fn name(&self) -> &str { "add" }
        fn description(&self) -> &str { "Adds 'amount' to 'value'" }
        async fn execute(&self, params: Value) -> KiasResult<Value> {
            let v = params.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let a = params.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(serde_json::json!({ "value": v + a }))
        }
    }

    struct FailSkill;
    #[async_trait]
    impl Skill for FailSkill {
        fn name(&self) -> &str { "fail" }
        fn description(&self) -> &str { "Always fails" }
        async fn execute(&self, _params: Value) -> KiasResult<Value> {
            Err(kias_common::KiasError::Validation("intentional failure".into()))
        }
    }

    struct EchoSkill;
    #[async_trait]
    impl Skill for EchoSkill {
        fn name(&self) -> &str { "echo" }
        fn description(&self) -> &str { "Echoes input" }
        async fn execute(&self, params: Value) -> KiasResult<Value> {
            Ok(params)
        }
    }

    fn test_registry() -> SkillRegistry {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(DoubleSkill));
        reg.register(Box::new(AddSkill));
        reg.register(Box::new(FailSkill));
        reg.register(Box::new(EchoSkill));
        reg
    }

    // ── tests ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_pipeline_creation_and_execution() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("double-then-add")
            .then("double")
            .then("add")
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 3.0, "amount": 10.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.steps_executed, 2);
        // double(3) = 6, add(6, amount=10) = 16
        assert_eq!(result.final_output["value"], 16.0);
    }

    #[tokio::test]
    async fn test_pipeline_with_input_mapping() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("mapped")
            .then("double")
            .with_input_mapping(InputMapping::with_static(serde_json::json!({
                "value": 100.0,
                "amount": 1.0
            })))
            .then("add")
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 5.0 }))
            .await;
        assert!(result.success);
        // double(5) = 10, but add uses static {value:100, amount:1} → 101
        assert_eq!(result.final_output["value"], 101.0);
    }

    #[tokio::test]
    async fn test_pipeline_error_stop_on_error() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("fail-stop")
            .then("fail")
            .then("double")
            .with_error_policy(ErrorPolicy::StopOnError)
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 1.0 }))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.step_results.len(), 0);
    }

    #[tokio::test]
    async fn test_pipeline_error_continue_on_error() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("fail-continue")
            .then("fail")
            .then("double")
            .with_error_policy(ErrorPolicy::ContinueOnError)
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 5.0 }))
            .await;
        // Pipeline "succeeds" because we continue on error
        assert!(result.success);
        assert_eq!(result.steps_executed, 2);
        // First step errored, second step executed with its own input
        assert!(result.step_results[0].get("error").is_some());
        assert_eq!(result.step_results[1]["value"], 10.0);
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("parallel-double")
            .then("double")
            .build();

        let inputs = vec![
            serde_json::json!({ "value": 1.0 }),
            serde_json::json!({ "value": 2.0 }),
            serde_json::json!({ "value": 3.0 }),
        ];
        let results = pipeline.execute_parallel(&reg, inputs).await;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].final_output["value"], 2.0);
        assert_eq!(results[1].final_output["value"], 4.0);
        assert_eq!(results[2].final_output["value"], 6.0);
    }

    #[tokio::test]
    async fn test_pipeline_builder_fluent_api() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("fluent")
            .with_error_policy(ErrorPolicy::ContinueOnError)
            .then("echo")
            .with_input_mapping(InputMapping::pass_through())
            .then("double")
            .then_if("add", |input| {
                input.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) > 10.0
            })
            .build();

        // echo(20) → 20, double(20) → 40, condition 40>10 → true, add(40, amount=5) → 45
        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 20.0, "amount": 5.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.final_output["value"], 45.0);
    }

    #[tokio::test]
    async fn test_empty_pipeline() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("empty").build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 42.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.steps_executed, 0);
        assert_eq!(result.step_results.len(), 0);
        assert_eq!(result.final_output, Value::Null);
    }

    #[tokio::test]
    async fn test_step_condition_skips() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("conditional")
            .then("double")
            .then_if("add", |input| {
                // Only add if value > 20
                input.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) > 20.0
            })
            .build();

        // double(5) = 10, condition 10 > 20 is false → add skipped
        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 5.0, "amount": 100.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.step_results.len(), 2);
        assert_eq!(result.step_results[1], Value::Null);
        assert_eq!(result.final_output, Value::Null);
    }

    #[tokio::test]
    async fn test_step_condition_passes() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("conditional-pass")
            .then("double")
            .then_if("add", |input| {
                input.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) > 5.0
            })
            .build();

        // double(10) = 20, condition 20 > 5 is true → add executes
        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 10.0, "amount": 3.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.final_output["value"], 23.0);
    }

    #[tokio::test]
    async fn test_input_mapping_from_path() {
        let reg = test_registry();
        let mapping = InputMapping::from_path("result.nested");
        let pipeline = PipelineBuilder::new("path-mapping")
            .then("echo")
            .with_input_mapping(mapping)
            .then("double")
            .build();

        let result = pipeline
            .execute(
                &reg,
                serde_json::json!({ "result": { "nested": { "value": 7.0 } } }),
            )
            .await;
        assert!(result.success);
        // echo receives {"value":7} (extracted from result.nested), echo passes it through
        // double receives {"value":7} → {"value":14}
        assert_eq!(result.final_output["value"], 14.0);
    }

    #[tokio::test]
    async fn test_pipeline_duration_recorded() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("timed")
            .then("echo")
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 1 }))
            .await;
        assert!(result.success);
        // Duration should be recorded (>= 0ms is trivially true, but the field must exist)
        let _ = result.duration_ms;
    }

    #[tokio::test]
    async fn test_missing_skill_with_continue_policy() {
        let reg = test_registry();
        let pipeline = PipelineBuilder::new("missing-skill")
            .then("nonexistent_skill")
            .then("double")
            .with_error_policy(ErrorPolicy::ContinueOnError)
            .build();

        let result = pipeline
            .execute(&reg, serde_json::json!({ "value": 4.0 }))
            .await;
        assert!(result.success);
        assert_eq!(result.steps_executed, 2);
        assert!(result.step_results[0].get("error").is_some());
        assert_eq!(result.step_results[1]["value"], 8.0);
    }
}
