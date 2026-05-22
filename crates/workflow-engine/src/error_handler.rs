//! Node-level error handler infrastructure.
//!
//! Each workflow node can register a custom [`ErrorHandler`] that decides
//! what to do when the node fails.  This replaces the old engine-level-only
//! error handling with per-node fine-grained control.
//!
//! # Design (inspired by LangGraph v1.2)
//!
//! ```text
//! Node fails → ErrorHandler::on_error(ctx, err) → ErrorAction
//!   Retry { .. }     → retry with backoff
//!   Skip              → mark node as skipped, continue workflow
//!   Fallback(node_id) → jump to a fallback node
//!   Abort             → immediately fail the workflow
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

use crate::node::ExecutionResult;
use crate::state::WorkflowState;

// ─── ErrorAction ─────────────────────────────────────────────────────────

/// The action an [`ErrorHandler`] returns to tell the engine what to do
/// after a node failure.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ErrorAction {
    /// Abort the entire workflow immediately.
    #[default]
    Abort,

    /// Retry the node, optionally overriding the retry policy.
    Retry {
        /// Maximum additional attempts.
        max_attempts: u32,
        /// Base delay between retries (exponential back-off with 2× multiplier).
        backoff: Duration,
    },

    /// Skip this node and continue the workflow as if it succeeded.
    Skip,

    /// Jump to a specific fallback node instead of the normal next node.
    Fallback { node_id: String },
}

impl fmt::Display for ErrorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAction::Retry {
                max_attempts,
                backoff,
            } => write!(
                f,
                "Retry(max_attempts={}, backoff={:?})",
                max_attempts, backoff
            ),
            ErrorAction::Skip => write!(f, "Skip"),
            ErrorAction::Fallback { node_id } => write!(f, "Fallback({})", node_id),
            ErrorAction::Abort => write!(f, "Abort"),
        }
    }
}

// ─── NodeContext ──────────────────────────────────────────────────────────

/// Context passed to an [`ErrorHandler`] so it can make an informed decision.
#[derive(Debug, Clone)]
pub struct NodeErrorContext<'a> {
    /// The ID of the node that failed.
    pub node_id: &'a str,
    /// The current workflow state.
    pub state: &'a WorkflowState,
    /// The number of retry attempts already made (0 = first failure).
    pub attempt: u32,
    /// The execution result from the failed attempt (if available).
    pub failed_result: Option<&'a ExecutionResult>,
}

// ─── ErrorHandler trait ───────────────────────────────────────────────────

/// Trait for node-level error handlers.
///
/// Implement this trait and attach it to a [`Node`] to customize per-node
/// error recovery.  The engine calls [`on_error`] after every failed attempt.
#[async_trait]
pub trait ErrorHandler: Send + Sync + fmt::Debug {
    /// Called when a node execution fails.
    ///
    /// Returns the [`ErrorAction`] the engine should take.
    async fn on_error(&self, ctx: &NodeErrorContext<'_>) -> ErrorAction;
}

// ─── Built-in error handlers ─────────────────────────────────────────────

/// Always retry a fixed number of times, then abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryOnError {
    /// Maximum number of *extra* attempts (beyond the engine's built-in retries).
    pub max_attempts: u32,
    /// Base delay between retries.
    pub backoff: Duration,
}

impl RetryOnError {
    pub fn new(max_attempts: u32, backoff: Duration) -> Self {
        Self {
            max_attempts,
            backoff,
        }
    }
}

#[async_trait]
impl ErrorHandler for RetryOnError {
    async fn on_error(&self, _ctx: &NodeErrorContext<'_>) -> ErrorAction {
        ErrorAction::Retry {
            max_attempts: self.max_attempts,
            backoff: self.backoff,
        }
    }
}

/// Skip the node on failure — the workflow continues as if it succeeded.
#[derive(Debug, Clone, Default)]
pub struct SkipOnError;

#[async_trait]
impl ErrorHandler for SkipOnError {
    async fn on_error(&self, _ctx: &NodeErrorContext<'_>) -> ErrorAction {
        ErrorAction::Skip
    }
}

/// Route to a fallback node on failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackOnError {
    /// The node ID to jump to on failure.
    pub fallback_node_id: String,
}

impl FallbackOnError {
    pub fn new(fallback_node_id: impl Into<String>) -> Self {
        Self {
            fallback_node_id: fallback_node_id.into(),
        }
    }
}

#[async_trait]
impl ErrorHandler for FallbackOnError {
    async fn on_error(&self, _ctx: &NodeErrorContext<'_>) -> ErrorAction {
        ErrorAction::Fallback {
            node_id: self.fallback_node_id.clone(),
        }
    }
}

/// Abort the workflow on failure (explicit, no retries).
///
/// This is the default behaviour when no error handler is set, but using
/// this handler explicitly lets you add custom logging / metrics before
/// the abort decision.
#[derive(Debug, Clone, Default)]
pub struct AbortOnError;

/// Default error handler: retry up to `max_retries` times with exponential
/// backoff, then abort.
///
/// This is the recommended handler for most production nodes.  It combines
/// a sensible retry policy with an eventual abort so the workflow does not
/// hang forever.
///
/// # Example
///
/// ```ignore
/// use kias_workflow_engine::DefaultErrorHandler;
/// use std::time::Duration;
///
/// let handler = DefaultErrorHandler::new(3, Duration::from_millis(200));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultErrorHandler {
    /// Maximum retry attempts before aborting.
    pub max_retries: u32,
    /// Base backoff duration (exponential with 2× multiplier per attempt).
    pub backoff: Duration,
}

impl DefaultErrorHandler {
    pub fn new(max_retries: u32, backoff: Duration) -> Self {
        Self {
            max_retries,
            backoff,
        }
    }

    /// Convenience constructor with millisecond-based backoff.
    pub fn with_backoff_ms(max_retries: u32, backoff_ms: u64) -> Self {
        Self::new(max_retries, Duration::from_millis(backoff_ms))
    }
}

impl Default for DefaultErrorHandler {
    fn default() -> Self {
        Self::new(3, Duration::from_millis(500))
    }
}

#[async_trait]
impl ErrorHandler for DefaultErrorHandler {
    async fn on_error(&self, ctx: &NodeErrorContext<'_>) -> ErrorAction {
        if ctx.attempt < self.max_retries {
            tracing::warn!(
                node_id = %ctx.node_id,
                attempt = ctx.attempt,
                max_retries = self.max_retries,
                "DefaultErrorHandler: retrying"
            );
            ErrorAction::Retry {
                max_attempts: self.max_retries - ctx.attempt,
                backoff: self.backoff,
            }
        } else {
            tracing::error!(
                node_id = %ctx.node_id,
                attempt = ctx.attempt,
                max_retries = self.max_retries,
                "DefaultErrorHandler: max retries exhausted, aborting"
            );
            ErrorAction::Abort
        }
    }
}

#[async_trait]
impl ErrorHandler for AbortOnError {
    async fn on_error(&self, ctx: &NodeErrorContext<'_>) -> ErrorAction {
        tracing::error!(
            node_id = %ctx.node_id,
            attempt = ctx.attempt,
            "ErrorHandler: aborting after failure"
        );
        ErrorAction::Abort
    }
}

/// Conditional error handler: delegates to different handlers based on
/// the error type or attempt count.
///
/// # Example
///
/// ```ignore
/// ConditionalErrorHandler::new()
///     .with_rule(|ctx| ctx.attempt < 3, RetryOnError::new(2, Duration::from_secs(1)))
///     .with_fallback(SkipOnError)
/// ```
///
/// > **Note**: Because `ConditionalErrorHandler` uses closures, it does not
/// > implement `Serialize`/`Deserialize`.  For persisted error handlers,
/// > use the built-in concrete types or a JSON-configured strategy.
#[derive(Debug)]
pub struct ConditionalErrorHandler {
    rules: Vec<ConditionalRule>,
    fallback: Box<dyn ErrorHandler>,
}

struct ConditionalRule {
    predicate: Box<dyn Fn(&NodeErrorContext<'_>) -> bool + Send + Sync>,
    handler: Box<dyn ErrorHandler>,
}

impl fmt::Debug for ConditionalRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConditionalRule")
            .field("handler", &self.handler)
            .finish()
    }
}

impl ConditionalErrorHandler {
    pub fn new(fallback: impl ErrorHandler + 'static) -> Self {
        Self {
            rules: Vec::new(),
            fallback: Box::new(fallback),
        }
    }

    pub fn with_rule(
        mut self,
        predicate: impl Fn(&NodeErrorContext<'_>) -> bool + Send + Sync + 'static,
        handler: impl ErrorHandler + 'static,
    ) -> Self {
        self.rules.push(ConditionalRule {
            predicate: Box::new(predicate),
            handler: Box::new(handler),
        });
        self
    }
}

#[async_trait]
impl ErrorHandler for ConditionalErrorHandler {
    async fn on_error(&self, ctx: &NodeErrorContext<'_>) -> ErrorAction {
        for rule in &self.rules {
            if (rule.predicate)(ctx) {
                tracing::debug!(
                    node_id = %ctx.node_id,
                    "ConditionalErrorHandler: matched rule, delegating"
                );
                return rule.handler.on_error(ctx).await;
            }
        }
        self.fallback.on_error(ctx).await
    }
}

// ─── Serialisable error handler config ───────────────────────────────────

/// A serialisable configuration for building an [`ErrorHandler`].
///
/// This allows error handler strategies to be defined in JSON/YAML workflow
/// definitions without needing Rust closures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ErrorHandlerConfig {
    /// Retry N times with the given backoff.
    Retry { max_attempts: u32, backoff_ms: u64 },
    /// Skip the node on failure.
    Skip,
    /// Jump to a fallback node.
    Fallback { node_id: String },
    /// Abort the workflow (explicit).
    Abort,
}

impl ErrorHandlerConfig {
    /// Build a concrete [`ErrorHandler`] from this config.
    pub fn build(&self) -> Box<dyn ErrorHandler> {
        match self {
            ErrorHandlerConfig::Retry {
                max_attempts,
                backoff_ms,
            } => Box::new(RetryOnError::new(
                *max_attempts,
                Duration::from_millis(*backoff_ms),
            )),
            ErrorHandlerConfig::Skip => Box::new(SkipOnError),
            ErrorHandlerConfig::Fallback { node_id } => {
                Box::new(FallbackOnError::new(node_id.clone()))
            }
            ErrorHandlerConfig::Abort => Box::new(AbortOnError),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WorkflowState;

    fn make_ctx<'a>(
        node_id: &'a str,
        state: &'a WorkflowState,
        attempt: u32,
    ) -> NodeErrorContext<'a> {
        NodeErrorContext {
            node_id,
            state,
            attempt,
            failed_result: None,
        }
    }

    #[tokio::test]
    async fn test_retry_on_error() {
        let handler = RetryOnError::new(3, Duration::from_millis(500));
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(
            action,
            ErrorAction::Retry {
                max_attempts: 3,
                backoff: Duration::from_millis(500),
            }
        );
    }

    #[tokio::test]
    async fn test_skip_on_error() {
        let handler = SkipOnError;
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Skip);
    }

    #[tokio::test]
    async fn test_fallback_on_error() {
        let handler = FallbackOnError::new("fallback-node");
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(
            action,
            ErrorAction::Fallback {
                node_id: "fallback-node".into()
            }
        );
    }

    #[tokio::test]
    async fn test_abort_on_error() {
        let handler = AbortOnError;
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Abort);
    }

    #[tokio::test]
    async fn test_conditional_handler_first_rule() {
        let handler = ConditionalErrorHandler::new(AbortOnError)
            .with_rule(|ctx| ctx.attempt < 3, SkipOnError);
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Skip);
    }

    #[tokio::test]
    async fn test_conditional_handler_fallback() {
        let handler = ConditionalErrorHandler::new(AbortOnError)
            .with_rule(|ctx| ctx.attempt >= 5, SkipOnError);
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 1);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Abort);
    }

    #[test]
    fn test_error_action_display() {
        assert_eq!(
            ErrorAction::Retry {
                max_attempts: 3,
                backoff: Duration::from_millis(500),
            }
            .to_string(),
            "Retry(max_attempts=3, backoff=500ms)"
        );
        assert_eq!(ErrorAction::Skip.to_string(), "Skip");
        assert_eq!(
            ErrorAction::Fallback {
                node_id: "fb".into()
            }
            .to_string(),
            "Fallback(fb)"
        );
        assert_eq!(ErrorAction::Abort.to_string(), "Abort");
    }

    #[test]
    fn test_error_action_default_is_abort() {
        assert_eq!(ErrorAction::default(), ErrorAction::Abort);
    }

    #[test]
    fn test_handler_config_build() {
        let cfg = ErrorHandlerConfig::Retry {
            max_attempts: 5,
            backoff_ms: 1000,
        };
        let _handler = cfg.build();

        let cfg = ErrorHandlerConfig::Skip;
        let _handler = cfg.build();

        let cfg = ErrorHandlerConfig::Fallback {
            node_id: "fb".into(),
        };
        let _handler = cfg.build();

        let cfg = ErrorHandlerConfig::Abort;
        let _handler = cfg.build();
    }

    // ── New tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_conditional_handler_second_rule() {
        // First rule doesn't match (attempt < 1), second does (attempt >= 1)
        let handler = ConditionalErrorHandler::new(AbortOnError)
            .with_rule(
                |ctx| ctx.attempt < 1,
                RetryOnError::new(2, Duration::from_millis(100)),
            )
            .with_rule(|ctx| ctx.attempt >= 1, SkipOnError);
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 5);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Skip);
    }

    #[tokio::test]
    async fn test_conditional_handler_no_rules_uses_fallback() {
        let handler = ConditionalErrorHandler::new(SkipOnError);
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 0);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Skip);
    }

    #[test]
    fn test_error_action_serialization_roundtrip() {
        let actions = vec![
            ErrorAction::Abort,
            ErrorAction::Skip,
            ErrorAction::Retry {
                max_attempts: 5,
                backoff: Duration::from_secs(2),
            },
            ErrorAction::Fallback {
                node_id: "fb".into(),
            },
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let restored: ErrorAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, restored);
        }
    }

    #[test]
    fn test_handler_config_serialization() {
        let configs = vec![
            ErrorHandlerConfig::Retry {
                max_attempts: 3,
                backoff_ms: 500,
            },
            ErrorHandlerConfig::Skip,
            ErrorHandlerConfig::Fallback {
                node_id: "fb-node".into(),
            },
            ErrorHandlerConfig::Abort,
        ];
        for cfg in configs {
            let json = serde_json::to_string(&cfg).unwrap();
            let restored: ErrorHandlerConfig = serde_json::from_str(&json).unwrap();
            // Re-serialize and compare
            let json2 = serde_json::to_string(&restored).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[tokio::test]
    async fn test_retry_on_error_with_zero_attempts() {
        let handler = RetryOnError::new(0, Duration::from_millis(100));
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 0);
        let action = handler.on_error(&ctx).await;
        assert_eq!(
            action,
            ErrorAction::Retry {
                max_attempts: 0,
                backoff: Duration::from_millis(100),
            }
        );
    }

    // ── DefaultErrorHandler tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_default_handler_retries_before_max() {
        let handler = DefaultErrorHandler::new(3, Duration::from_millis(100));
        let state = WorkflowState::new("wf-1", "n1");
        // Attempt 0 (first failure) → should retry
        let ctx = make_ctx("n1", &state, 0);
        let action = handler.on_error(&ctx).await;
        assert_eq!(
            action,
            ErrorAction::Retry {
                max_attempts: 3,
                backoff: Duration::from_millis(100),
            }
        );
    }

    #[tokio::test]
    async fn test_default_handler_aborts_after_max() {
        let handler = DefaultErrorHandler::new(3, Duration::from_millis(100));
        let state = WorkflowState::new("wf-1", "n1");
        // Attempt 3 → exhausted → abort
        let ctx = make_ctx("n1", &state, 3);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Abort);
    }

    #[tokio::test]
    async fn test_default_handler_decreasing_retry_count() {
        let handler = DefaultErrorHandler::new(5, Duration::from_millis(200));
        let state = WorkflowState::new("wf-1", "n1");

        // attempt 2 → 3 remaining
        let ctx = make_ctx("n1", &state, 2);
        let action = handler.on_error(&ctx).await;
        assert_eq!(
            action,
            ErrorAction::Retry {
                max_attempts: 3,
                backoff: Duration::from_millis(200),
            }
        );
    }

    #[test]
    fn test_default_handler_with_backoff_ms_constructor() {
        let handler = DefaultErrorHandler::with_backoff_ms(2, 1000);
        assert_eq!(handler.max_retries, 2);
        assert_eq!(handler.backoff, Duration::from_millis(1000));
    }

    #[test]
    fn test_default_handler_default_values() {
        let handler = DefaultErrorHandler::default();
        assert_eq!(handler.max_retries, 3);
        assert_eq!(handler.backoff, Duration::from_millis(500));
    }

    #[test]
    fn test_default_handler_serialization_roundtrip() {
        let handler = DefaultErrorHandler::new(4, Duration::from_millis(750));
        let json = serde_json::to_string(&handler).unwrap();
        let restored: DefaultErrorHandler = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.max_retries, 4);
        assert_eq!(restored.backoff, Duration::from_millis(750));
    }

    #[tokio::test]
    async fn test_default_handler_with_zero_max_retries_aborts_immediately() {
        let handler = DefaultErrorHandler::new(0, Duration::from_millis(100));
        let state = WorkflowState::new("wf-1", "n1");
        let ctx = make_ctx("n1", &state, 0);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Abort);
    }

    #[test]
    fn test_error_action_retry_backoff_ms_display() {
        let action = ErrorAction::Retry {
            max_attempts: 2,
            backoff: Duration::from_millis(250),
        };
        assert_eq!(action.to_string(), "Retry(max_attempts=2, backoff=250ms)");
    }

    #[tokio::test]
    async fn test_conditional_handler_with_default_handler_fallback() {
        let handler =
            ConditionalErrorHandler::new(DefaultErrorHandler::new(2, Duration::from_millis(100)))
                .with_rule(|ctx| ctx.attempt == 0, SkipOnError);
        let state = WorkflowState::new("wf-1", "n1");

        // attempt 0 → rule matches → skip
        let ctx = make_ctx("n1", &state, 0);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Skip);

        // attempt 2 → no rule matches → falls through to DefaultErrorHandler
        let ctx = make_ctx("n1", &state, 2);
        let action = handler.on_error(&ctx).await;
        assert_eq!(action, ErrorAction::Abort);
    }
}
