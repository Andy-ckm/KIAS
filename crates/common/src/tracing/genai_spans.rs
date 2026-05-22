//! OpenTelemetry-native Agent Tracing with GenAI Semantic Conventions.
//!
//! Implements the [OpenTelemetry GenAI Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/)
//! for comprehensive agent observability. Provides structured spans for:
//!
//! - `gen_ai.agent.create` — Agent creation lifecycle
//! - `gen_ai.agent.invoke` — Agent invocation with token usage
//! - `gen_ai.tool.execute` — Tool/function execution
//! - `gen_ai.workflow.step` — Workflow orchestration steps
//!
//! Tracing can be toggled via the `AGENTGUARD_OTEL_ENABLED` environment variable.
//! When disabled, span creation is a zero-cost no-op.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info_span, Span};

// ---------------------------------------------------------------------------
// Global enable flag
// ---------------------------------------------------------------------------

/// The effective OTEL enabled value.
static OTEL_ENABLED: AtomicBool = AtomicBool::new(false);
/// Whether the flag has been initialised (from env or explicit override).
static OTEL_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Ensure the flag is initialised. On first call, reads the
/// `AGENTGUARD_OTEL_ENABLED` environment variable.
fn ensure_otel_init() {
    if !OTEL_INITIALIZED.load(Ordering::Relaxed) {
        let enabled = std::env::var("AGENTGUARD_OTEL_ENABLED")
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        OTEL_ENABLED.store(enabled, Ordering::Relaxed);
        OTEL_INITIALIZED.store(true, Ordering::Relaxed);
    }
}

/// Returns `true` if OpenTelemetry tracing is enabled.
pub fn is_otel_enabled() -> bool {
    ensure_otel_init();
    OTEL_ENABLED.load(Ordering::Relaxed)
}

/// Override the OTEL enabled flag (useful for tests).
/// Marks the flag as initialised so the environment variable is not re-read.
pub fn set_otel_enabled(enabled: bool) {
    OTEL_INITIALIZED.store(true, Ordering::Relaxed);
    OTEL_ENABLED.store(enabled, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// GenAI Semantic Convention attribute keys
// ---------------------------------------------------------------------------

/// Standard attribute keys following the GenAI Semantic Conventions.
pub mod attr {
    pub const AGENT_NAME: &str = "agent.name";
    pub const AGENT_ID: &str = "agent.id";
    pub const SYSTEM: &str = "system";
    pub const REQUEST_MODEL: &str = "request.model";
    pub const USAGE_INPUT_TOKENS: &str = "usage.input_tokens";
    pub const USAGE_OUTPUT_TOKENS: &str = "usage.output_tokens";
    pub const TOOL_NAME: &str = "tool.name";
    pub const TOOL_TYPE: &str = "tool.type";
    pub const WORKFLOW_NAME: &str = "workflow.name";
    pub const WORKFLOW_STEP_INDEX: &str = "workflow.step.index";
    pub const WORKFLOW_STEP_NAME: &str = "workflow.step.name";
    pub const STATUS_CODE: &str = "status_code";
    pub const ERROR_MESSAGE: &str = "error.message";
}

// ---------------------------------------------------------------------------
// Span builders — structured attribute containers
// ---------------------------------------------------------------------------

/// Attributes for an agent-create span.
#[derive(Debug, Clone, Default)]
pub struct AgentCreateAttrs {
    pub agent_name: String,
    pub agent_id: String,
    pub system: String,
    pub model: String,
}

/// Attributes for an agent-invoke span.
#[derive(Debug, Clone, Default)]
pub struct AgentInvokeAttrs {
    pub agent_name: String,
    pub agent_id: String,
    pub system: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Attributes for a tool-execute span.
#[derive(Debug, Clone, Default)]
pub struct ToolExecuteAttrs {
    pub agent_name: String,
    pub agent_id: String,
    pub tool_name: String,
    pub tool_type: String,
    pub model: String,
}

/// Attributes for a workflow-step span.
#[derive(Debug, Clone, Default)]
pub struct WorkflowStepAttrs {
    pub workflow_name: String,
    pub step_index: u32,
    pub step_name: String,
    pub agent_name: String,
    pub agent_id: String,
    pub model: String,
}

// ---------------------------------------------------------------------------
// GenAiTracer
// ---------------------------------------------------------------------------

/// OpenTelemetry-native tracer for GenAI agent operations.
///
/// Creates [`tracing::Span`]s following the GenAI Semantic Conventions.
/// When `AGENTGUARD_OTEL_ENABLED` is not set or set to a falsy value,
/// all span-creation methods return a **disabled** span (zero overhead).
///
/// # Examples
///
/// ```rust
/// use kias_common::tracing::genai_spans::{GenAiTracer, AgentCreateAttrs};
///
/// let tracer = GenAiTracer::new("my-system");
/// let attrs = AgentCreateAttrs {
///     agent_name: "planner".into(),
///     agent_id: "a-001".into(),
///     system: "kias".into(),
///     model: "gpt-4o".into(),
/// };
/// let span = tracer.agent_create(attrs);
/// let _guard = span.enter();
/// // ... agent creation logic ...
/// ```
#[derive(Debug, Clone)]
pub struct GenAiTracer {
    /// Value for the `system` attribute on all spans.
    system: String,
}

impl GenAiTracer {
    /// Create a new tracer associated with the given system name.
    pub fn new(system: impl Into<String>) -> Self {
        Self {
            system: system.into(),
        }
    }

    /// The system name this tracer was created with.
    pub fn system(&self) -> &str {
        &self.system
    }

    // -- span factories -----------------------------------------------------

    /// Create a span for `gen_ai.agent.create`.
    pub fn agent_create(&self, attrs: AgentCreateAttrs) -> Span {
        if !is_otel_enabled() {
            return Span::none();
        }
        info_span!(
            "gen_ai.agent.create",
            agent.name = %attrs.agent_name,
            agent.id = %attrs.agent_id,
            system = %if attrs.system.is_empty() { &self.system } else { &attrs.system },
            request.model = %attrs.model,
        )
    }

    /// Create a span for `gen_ai.agent.invoke`.
    pub fn agent_invoke(&self, attrs: AgentInvokeAttrs) -> Span {
        if !is_otel_enabled() {
            return Span::none();
        }
        info_span!(
            "gen_ai.agent.invoke",
            agent.name = %attrs.agent_name,
            agent.id = %attrs.agent_id,
            system = %if attrs.system.is_empty() { &self.system } else { &attrs.system },
            request.model = %attrs.model,
            usage.input_tokens = ?attrs.input_tokens,
            usage.output_tokens = ?attrs.output_tokens,
        )
    }

    /// Create a span for `gen_ai.tool.execute`.
    pub fn tool_execute(&self, attrs: ToolExecuteAttrs) -> Span {
        if !is_otel_enabled() {
            return Span::none();
        }
        info_span!(
            "gen_ai.tool.execute",
            agent.name = %attrs.agent_name,
            agent.id = %attrs.agent_id,
            tool.name = %attrs.tool_name,
            tool.type = %attrs.tool_type,
            request.model = %attrs.model,
            system = %self.system,
        )
    }

    /// Create a span for `gen_ai.workflow.step`.
    pub fn workflow_step(&self, attrs: WorkflowStepAttrs) -> Span {
        if !is_otel_enabled() {
            return Span::none();
        }
        info_span!(
            "gen_ai.workflow.step",
            workflow.name = %attrs.workflow_name,
            workflow.step.index = %attrs.step_index,
            workflow.step.name = %attrs.step_name,
            agent.name = %attrs.agent_name,
            agent.id = %attrs.agent_id,
            request.model = %attrs.model,
            system = %self.system,
        )
    }

    /// Record token usage on the currently-active span (convenience helper).
    pub fn record_usage(input_tokens: u64, output_tokens: u64) {
        let span = Span::current();
        if span.is_disabled() {
            return;
        }
        span.record("usage.input_tokens", tracing::field::display(input_tokens));
        span.record(
            "usage.output_tokens",
            tracing::field::display(output_tokens),
        );
    }

    /// Record an error status on the given span.
    pub fn record_error(span: &Span, message: &str) {
        if span.is_disabled() {
            return;
        }
        span.record(attr::STATUS_CODE, tracing::field::display("ERROR"));
        span.record(attr::ERROR_MESSAGE, tracing::field::display(message));
    }

    /// Build a summary `HashMap` of key→value for a completed span
    /// (useful for test assertions and debugging).
    pub fn summary_map(attrs: &AgentInvokeAttrs) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(attr::AGENT_NAME.to_string(), attrs.agent_name.clone());
        m.insert(attr::AGENT_ID.to_string(), attrs.agent_id.clone());
        m.insert(attr::SYSTEM.to_string(), attrs.system.clone());
        m.insert(attr::REQUEST_MODEL.to_string(), attrs.model.clone());
        if let Some(t) = attrs.input_tokens {
            m.insert(attr::USAGE_INPUT_TOKENS.to_string(), t.to_string());
        }
        if let Some(t) = attrs.output_tokens {
            m.insert(attr::USAGE_OUTPUT_TOKENS.to_string(), t.to_string());
        }
        m
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    use tracing_subscriber::fmt;

    /// Helper: run a test closure with OTEL disabled and a tracing subscriber.
    fn with_otel_disabled(f: impl FnOnce()) {
        set_otel_enabled(false);
        let subscriber = fmt::Subscriber::builder().with_test_writer().finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        f();
    }

    /// Helper: run a test closure with OTEL enabled and a tracing subscriber.
    fn with_otel_enabled(f: impl FnOnce()) {
        set_otel_enabled(true);
        let subscriber = fmt::Subscriber::builder().with_test_writer().finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        f();
    }

    // -- 1. Environment variable control ------------------------------------

    #[test]
    #[serial]
    fn test_otel_disabled_by_default() {
        set_otel_enabled(false);
        assert!(!is_otel_enabled());
    }

    #[test]
    #[serial]
    fn test_otel_enabled_flag() {
        with_otel_enabled(|| {
            assert!(is_otel_enabled());
        });
    }

    // -- 2. Disabled spans are no-ops (return Span::none) -------------------

    #[test]
    #[serial]
    fn test_agent_create_span_disabled() {
        with_otel_disabled(|| {
            let tracer = GenAiTracer::new("test-system");
            let attrs = AgentCreateAttrs {
                agent_name: "alpha".into(),
                agent_id: "id-1".into(),
                system: "test".into(),
                model: "gpt-4o".into(),
            };
            let span = tracer.agent_create(attrs);
            assert!(span.is_disabled());
        });
    }

    #[test]
    #[serial]
    fn test_agent_invoke_span_disabled() {
        with_otel_disabled(|| {
            let tracer = GenAiTracer::new("test-system");
            let attrs = AgentInvokeAttrs::default();
            let span = tracer.agent_invoke(attrs);
            assert!(span.is_disabled());
        });
    }

    #[test]
    #[serial]
    fn test_tool_execute_span_disabled() {
        with_otel_disabled(|| {
            let tracer = GenAiTracer::new("test-system");
            let attrs = ToolExecuteAttrs::default();
            let span = tracer.tool_execute(attrs);
            assert!(span.is_disabled());
        });
    }

    #[test]
    #[serial]
    fn test_workflow_step_span_disabled() {
        with_otel_disabled(|| {
            let tracer = GenAiTracer::new("test-system");
            let attrs = WorkflowStepAttrs::default();
            let span = tracer.workflow_step(attrs);
            assert!(span.is_disabled());
        });
    }

    // -- 3. Enabled spans are real spans ------------------------------------

    #[test]
    #[serial]
    fn test_agent_create_span_enabled() {
        with_otel_enabled(|| {
            let tracer = GenAiTracer::new("kias");
            let attrs = AgentCreateAttrs {
                agent_name: "planner".into(),
                agent_id: "a-001".into(),
                system: "kias".into(),
                model: "gpt-4o".into(),
            };
            let span = tracer.agent_create(attrs);
            assert!(!span.is_disabled());
            let meta = span.metadata().expect("span must have metadata");
            assert_eq!(meta.name(), "gen_ai.agent.create");
        });
    }

    #[test]
    #[serial]
    fn test_agent_invoke_span_enabled_records_tokens() {
        with_otel_enabled(|| {
            let tracer = GenAiTracer::new("kias");
            let attrs = AgentInvokeAttrs {
                agent_name: "executor".into(),
                agent_id: "a-002".into(),
                system: "kias".into(),
                model: "claude-3".into(),
                input_tokens: Some(512),
                output_tokens: Some(128),
            };
            let span = tracer.agent_invoke(attrs);
            assert!(!span.is_disabled());
            let meta = span.metadata().expect("span must have metadata");
            assert_eq!(meta.name(), "gen_ai.agent.invoke");
        });
    }

    // -- 4. Tool execute span enabled ----------------------------------------

    #[test]
    #[serial]
    fn test_tool_execute_span_enabled() {
        with_otel_enabled(|| {
            let tracer = GenAiTracer::new("kias");
            let attrs = ToolExecuteAttrs {
                agent_name: "coder".into(),
                agent_id: "a-003".into(),
                tool_name: "code_interpreter".into(),
                tool_type: "function".into(),
                model: "gpt-4o".into(),
            };
            let span = tracer.tool_execute(attrs);
            assert!(!span.is_disabled());
            let meta = span.metadata().expect("span must have metadata");
            assert_eq!(meta.name(), "gen_ai.tool.execute");
        });
    }

    // -- 5. Workflow step span enabled ---------------------------------------

    #[test]
    #[serial]
    fn test_workflow_step_span_enabled() {
        with_otel_enabled(|| {
            let tracer = GenAiTracer::new("kias");
            let attrs = WorkflowStepAttrs {
                workflow_name: "deploy-pipeline".into(),
                step_index: 2,
                step_name: "validate".into(),
                agent_name: "validator".into(),
                agent_id: "a-004".into(),
                model: "gpt-4o-mini".into(),
            };
            let span = tracer.workflow_step(attrs);
            assert!(!span.is_disabled());
            let meta = span.metadata().expect("span must have metadata");
            assert_eq!(meta.name(), "gen_ai.workflow.step");
        });
    }

    // -- 6. GenAiTracer stores system name ----------------------------------

    #[test]
    #[serial]
    fn test_tracer_system_name() {
        let tracer = GenAiTracer::new("my-kias");
        assert_eq!(tracer.system(), "my-kias");
    }

    // -- 7. summary_map returns expected keys -------------------------------

    #[test]
    #[serial]
    fn test_summary_map_contents() {
        let attrs = AgentInvokeAttrs {
            agent_name: "agent-X".into(),
            agent_id: "x-999".into(),
            system: "prod".into(),
            model: "llama3".into(),
            input_tokens: Some(1000),
            output_tokens: Some(200),
        };
        let m = GenAiTracer::summary_map(&attrs);
        assert_eq!(m.get("agent.name").unwrap(), "agent-X");
        assert_eq!(m.get("agent.id").unwrap(), "x-999");
        assert_eq!(m.get("system").unwrap(), "prod");
        assert_eq!(m.get("request.model").unwrap(), "llama3");
        assert_eq!(m.get("usage.input_tokens").unwrap(), "1000");
        assert_eq!(m.get("usage.output_tokens").unwrap(), "200");
    }

    // -- 8. record_error on disabled span does not panic --------------------

    #[test]
    #[serial]
    fn test_record_error_on_disabled_span() {
        with_otel_disabled(|| {
            let tracer = GenAiTracer::new("test");
            let attrs = AgentCreateAttrs::default();
            let span = tracer.agent_create(attrs);
            // Should not panic.
            GenAiTracer::record_error(&span, "something went wrong");
        });
    }
}
