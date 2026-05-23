//! Slow Action Tracing for AgentGuard.
//!
//! Inspired by EMQ's slow subscription tracking, but designed for AI Agent
//! operations. Tracks agent actions that exceed configurable latency thresholds
//! and provides diagnostics for performance bottlenecks.
//!
//! # Architecture
//!
//! ```text
//! Agent Action → SlowTraceCollector → threshold check
//!   ├─ fast → discard
//!   └─ slow → store + notify EventBus
//! ```
//!
//! # Surpasses EMQ
//!
//! - EMQ tracks MQTT message delivery latency only
//! - AgentGuard tracks ALL agent actions (LLM calls, tool exec, workflows)
//! - Provides root cause hints (queue depth, token count, model latency)
//! - Integrates with GxP audit trail

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

// ─── Configuration ───────────────────────────────────────────────────

/// Slow trace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowTraceConfig {
    /// Threshold above which an action is considered "slow" (in milliseconds).
    pub threshold_ms: u64,
    /// Maximum number of slow traces to retain in the ring buffer.
    pub max_traces: usize,
    /// Enable/disable slow tracing globally.
    pub enabled: bool,
}

impl Default for SlowTraceConfig {
    fn default() -> Self {
        Self {
            threshold_ms: 1000, // 1 second
            max_traces: 1000,
            enabled: true,
        }
    }
}

// ─── Data Types ──────────────────────────────────────────────────────

/// Category of agent action being traced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    /// LLM inference call.
    LlmInference,
    /// Tool execution.
    ToolExecution,
    /// Workflow step execution.
    WorkflowStep,
    /// Knowledge retrieval (vector/keyword search).
    KnowledgeRetrieval,
    /// Memory read/write operation.
    MemoryOperation,
    /// External API call.
    ExternalApi,
    /// Agent-to-Agent communication.
    A2aCommunication,
    /// Other/custom action.
    Other,
}

impl std::fmt::Display for ActionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LlmInference => write!(f, "llm_inference"),
            Self::ToolExecution => write!(f, "tool_execution"),
            Self::WorkflowStep => write!(f, "workflow_step"),
            Self::KnowledgeRetrieval => write!(f, "knowledge_retrieval"),
            Self::MemoryOperation => write!(f, "memory_operation"),
            Self::ExternalApi => write!(f, "external_api"),
            Self::A2aCommunication => write!(f, "a2a_communication"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Severity classification for slow traces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlowSeverity {
    /// 1x-2x threshold (warning).
    Warning,
    /// 2x-5x threshold (slow).
    Slow,
    /// 5x+ threshold (critical).
    Critical,
}

impl SlowSeverity {
    /// Classify severity based on duration vs threshold.
    pub fn from_duration(duration_ms: u64, threshold_ms: u64) -> Self {
        if duration_ms >= threshold_ms * 5 {
            Self::Critical
        } else if duration_ms >= threshold_ms * 2 {
            Self::Slow
        } else {
            Self::Warning
        }
    }
}

/// Root cause hint for a slow action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootCauseHint {
    /// LLM model was slow to respond.
    ModelLatency,
    /// High queue depth caused scheduling delay.
    QueueBackpressure,
    /// Large token count increased processing time.
    LargeTokenCount,
    /// External dependency was slow.
    ExternalDependency,
    /// Resource contention (CPU/memory/GPU).
    ResourceContention,
    /// Network latency.
    NetworkLatency,
    /// Unknown/unclassified.
    Unknown,
}

/// A single slow action trace record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowTrace {
    /// Unique trace ID.
    pub trace_id: String,
    /// Agent that performed the action.
    pub agent_id: String,
    /// Agent name (for human readability).
    pub agent_name: String,
    /// Category of action.
    pub category: ActionCategory,
    /// Human-readable action description.
    pub action: String,
    /// Duration of the action in milliseconds.
    pub duration_ms: u64,
    /// Severity classification.
    pub severity: SlowSeverity,
    /// Inferred root cause hint.
    pub root_cause: RootCauseHint,
    /// Optional context (model name, tool name, etc.).
    pub context: Option<String>,
    /// ISO-8601 timestamp when the action started.
    pub started_at: String,
    /// ISO-8601 timestamp when the action completed.
    pub completed_at: String,
}

/// Summary statistics for slow traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowTraceSummary {
    /// Total number of slow traces recorded.
    pub total_count: usize,
    /// Number of traces per category.
    pub by_category: std::collections::HashMap<String, usize>,
    /// Number of traces per severity.
    pub by_severity: std::collections::HashMap<String, usize>,
    /// Average duration of slow traces (ms).
    pub avg_duration_ms: f64,
    /// P95 duration of slow traces (ms).
    pub p95_duration_ms: u64,
    /// P99 duration of slow traces (ms).
    pub p99_duration_ms: u64,
    /// Top 5 slowest agents.
    pub top_slow_agents: Vec<SlowAgentSummary>,
    /// Current threshold (ms).
    pub threshold_ms: u64,
    /// Whether tracing is enabled.
    pub enabled: bool,
}

/// Per-agent slow trace summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowAgentSummary {
    pub agent_id: String,
    pub agent_name: String,
    pub slow_count: usize,
    pub avg_duration_ms: f64,
    pub worst_category: String,
}

// ─── Collector ───────────────────────────────────────────────────────

/// Thread-safe slow action trace collector.
///
/// Uses a ring buffer to retain the most recent slow traces.
/// Provides query, summary, and threshold management.
#[derive(Clone)]
pub struct SlowTraceCollector {
    inner: Arc<SlowTraceInner>,
}

struct SlowTraceInner {
    traces: RwLock<VecDeque<SlowTrace>>,
    config: RwLock<SlowTraceConfig>,
}

impl SlowTraceCollector {
    /// Create a new collector with default configuration.
    pub fn new() -> Self {
        Self::with_config(SlowTraceConfig::default())
    }

    /// Create a new collector with custom configuration.
    pub fn with_config(config: SlowTraceConfig) -> Self {
        Self {
            inner: Arc::new(SlowTraceInner {
                traces: RwLock::new(VecDeque::with_capacity(config.max_traces)),
                config: RwLock::new(config),
            }),
        }
    }

    /// Record an agent action. If it exceeds the slow threshold, it's stored.
    ///
    /// Returns `Some(SlowTrace)` if the action was slow, `None` if it was fast
    /// or tracing is disabled.
    pub async fn record(
        &self,
        agent_id: &str,
        agent_name: &str,
        category: ActionCategory,
        action: &str,
        duration: Duration,
        context: Option<String>,
    ) -> Option<SlowTrace> {
        let config = self.inner.config.read().await;
        if !config.enabled {
            return None;
        }

        let duration_ms = duration.as_millis() as u64;
        if duration_ms < config.threshold_ms {
            return None;
        }

        let severity = SlowSeverity::from_duration(duration_ms, config.threshold_ms);
        let root_cause = Self::infer_root_cause(&category, duration_ms, &context);

        let now = Utc::now();
        let started_at = (now - chrono::Duration::milliseconds(duration_ms as i64)).to_rfc3339();

        let trace = SlowTrace {
            trace_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            category,
            action: action.to_string(),
            duration_ms,
            severity,
            root_cause,
            context,
            started_at,
            completed_at: now.to_rfc3339(),
        };

        warn!(
            agent_id = agent_id,
            action = action,
            duration_ms = duration_ms,
            severity = ?trace.severity,
            "Slow action detected"
        );

        let mut traces = self.inner.traces.write().await;
        if traces.len() >= config.max_traces {
            traces.pop_front();
        }
        traces.push_back(trace.clone());

        Some(trace)
    }

    /// Infer root cause hint based on category and context.
    fn infer_root_cause(
        category: &ActionCategory,
        duration_ms: u64,
        context: &Option<String>,
    ) -> RootCauseHint {
        match category {
            ActionCategory::LlmInference => {
                if duration_ms > 30_000 {
                    RootCauseHint::LargeTokenCount
                } else {
                    RootCauseHint::ModelLatency
                }
            }
            ActionCategory::ToolExecution => RootCauseHint::ExternalDependency,
            ActionCategory::KnowledgeRetrieval => RootCauseHint::ResourceContention,
            ActionCategory::ExternalApi => RootCauseHint::NetworkLatency,
            ActionCategory::A2aCommunication => RootCauseHint::NetworkLatency,
            ActionCategory::WorkflowStep => {
                if context.as_deref().is_some_and(|c| c.contains("queue")) {
                    RootCauseHint::QueueBackpressure
                } else {
                    RootCauseHint::Unknown
                }
            }
            _ => RootCauseHint::Unknown,
        }
    }

    /// Get the most recent N slow traces.
    pub async fn recent(&self, limit: usize) -> Vec<SlowTrace> {
        let traces = self.inner.traces.read().await;
        traces.iter().rev().take(limit).cloned().collect()
    }

    /// Get all slow traces for a specific agent.
    pub async fn by_agent(&self, agent_id: &str) -> Vec<SlowTrace> {
        let traces = self.inner.traces.read().await;
        traces
            .iter()
            .filter(|t| t.agent_id == agent_id)
            .cloned()
            .collect()
    }

    /// Get summary statistics.
    pub async fn summary(&self) -> SlowTraceSummary {
        let traces = self.inner.traces.read().await;
        let config = self.inner.config.read().await;

        let mut by_category: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_severity: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut durations: Vec<u64> = Vec::new();
        #[allow(clippy::type_complexity)]
        let mut agent_map: std::collections::HashMap<
            String,
            (String, Vec<u64>, std::collections::HashMap<String, usize>),
        > = std::collections::HashMap::new();

        for trace in traces.iter() {
            *by_category.entry(trace.category.to_string()).or_insert(0) += 1;
            *by_severity
                .entry(format!("{:?}", trace.severity))
                .or_insert(0) += 1;
            durations.push(trace.duration_ms);

            let entry = agent_map.entry(trace.agent_id.clone()).or_insert_with(|| {
                (
                    trace.agent_name.clone(),
                    Vec::new(),
                    std::collections::HashMap::new(),
                )
            });
            entry.1.push(trace.duration_ms);
            *entry.2.entry(trace.category.to_string()).or_insert(0) += 1;
        }

        durations.sort_unstable();
        let avg_duration_ms = if durations.is_empty() {
            0.0
        } else {
            durations.iter().sum::<u64>() as f64 / durations.len() as f64
        };
        let p95_duration_ms = percentile(&durations, 95.0);
        let p99_duration_ms = percentile(&durations, 99.0);

        let mut top_slow_agents: Vec<SlowAgentSummary> = agent_map
            .into_iter()
            .map(|(id, (name, durs, cats))| {
                let worst_category = cats
                    .into_iter()
                    .max_by_key(|(_, c)| *c)
                    .map(|(k, _)| k)
                    .unwrap_or_else(|| "unknown".to_string());
                SlowAgentSummary {
                    agent_id: id,
                    agent_name: name,
                    slow_count: durs.len(),
                    avg_duration_ms: if durs.is_empty() {
                        0.0
                    } else {
                        durs.iter().sum::<u64>() as f64 / durs.len() as f64
                    },
                    worst_category,
                }
            })
            .collect();
        top_slow_agents.sort_by_key(|b| std::cmp::Reverse(b.slow_count));
        top_slow_agents.truncate(5);

        SlowTraceSummary {
            total_count: traces.len(),
            by_category,
            by_severity,
            avg_duration_ms,
            p95_duration_ms,
            p99_duration_ms,
            top_slow_agents,
            threshold_ms: config.threshold_ms,
            enabled: config.enabled,
        }
    }

    /// Update the slow trace configuration.
    pub async fn update_config(&self, config: SlowTraceConfig) {
        let mut current = self.inner.config.write().await;
        info!(
            old_threshold = current.threshold_ms,
            new_threshold = config.threshold_ms,
            enabled = config.enabled,
            "Slow trace config updated"
        );
        *current = config;
    }

    /// Get current configuration.
    pub async fn config(&self) -> SlowTraceConfig {
        self.inner.config.read().await.clone()
    }

    /// Clear all stored traces.
    pub async fn clear(&self) {
        let mut traces = self.inner.traces.write().await;
        let count = traces.len();
        traces.clear();
        info!(cleared = count, "Slow traces cleared");
    }

    /// Total number of stored traces.
    pub async fn count(&self) -> usize {
        self.inner.traces.read().await.len()
    }
}

impl Default for SlowTraceCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate percentile from a sorted slice.
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (p / 100.0 * sorted.len() as f64) as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collector_default_config() {
        let collector = SlowTraceCollector::new();
        let cfg = collector.config().await;
        assert_eq!(cfg.threshold_ms, 1000);
        assert_eq!(cfg.max_traces, 1000);
        assert!(cfg.enabled);
    }

    #[tokio::test]
    async fn test_fast_action_not_recorded() {
        let collector = SlowTraceCollector::new();
        let result = collector
            .record(
                "a1",
                "test-agent",
                ActionCategory::LlmInference,
                "quick call",
                Duration::from_millis(100),
                None,
            )
            .await;
        assert!(result.is_none());
        assert_eq!(collector.count().await, 0);
    }

    #[tokio::test]
    async fn test_slow_action_recorded() {
        let collector = SlowTraceCollector::new();
        let result = collector
            .record(
                "a1",
                "test-agent",
                ActionCategory::LlmInference,
                "slow LLM call",
                Duration::from_millis(2000),
                Some("gpt-4".to_string()),
            )
            .await;
        assert!(result.is_some());
        let trace = result.unwrap();
        assert_eq!(trace.agent_id, "a1");
        assert_eq!(trace.duration_ms, 2000);
        assert_eq!(trace.severity, SlowSeverity::Slow); // 2x threshold
        assert_eq!(trace.root_cause, RootCauseHint::ModelLatency);
        assert_eq!(trace.context, Some("gpt-4".to_string()));
        assert_eq!(collector.count().await, 1);
    }

    #[tokio::test]
    async fn test_critical_severity() {
        let collector = SlowTraceCollector::new();
        let result = collector
            .record(
                "a1",
                "test-agent",
                ActionCategory::ToolExecution,
                "very slow tool",
                Duration::from_millis(6000),
                None,
            )
            .await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, SlowSeverity::Critical); // 6x threshold
    }

    #[tokio::test]
    async fn test_warning_severity() {
        let collector = SlowTraceCollector::new();
        let result = collector
            .record(
                "a1",
                "test-agent",
                ActionCategory::KnowledgeRetrieval,
                "slow search",
                Duration::from_millis(1500),
                None,
            )
            .await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, SlowSeverity::Warning); // 1.5x threshold
    }

    #[tokio::test]
    async fn test_disabled_tracing() {
        let collector = SlowTraceCollector::new();
        collector
            .update_config(SlowTraceConfig {
                enabled: false,
                ..Default::default()
            })
            .await;

        let result = collector
            .record(
                "a1",
                "test-agent",
                ActionCategory::LlmInference,
                "should be ignored",
                Duration::from_millis(5000),
                None,
            )
            .await;
        assert!(result.is_none());
        assert_eq!(collector.count().await, 0);
    }

    #[tokio::test]
    async fn test_ring_buffer_eviction() {
        let collector = SlowTraceCollector::with_config(SlowTraceConfig {
            max_traces: 3,
            ..Default::default()
        });

        for i in 0..5 {
            collector
                .record(
                    &format!("a{}", i),
                    &format!("agent-{}", i),
                    ActionCategory::Other,
                    &format!("action-{}", i),
                    Duration::from_millis(1500),
                    None,
                )
                .await;
        }

        assert_eq!(collector.count().await, 3);
        let recent = collector.recent(10).await;
        // Should have actions 2, 3, 4 (evicted 0 and 1)
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].agent_id, "a4"); // most recent first
    }

    #[tokio::test]
    async fn test_filter_by_agent() {
        let collector = SlowTraceCollector::new();

        collector
            .record(
                "a1",
                "agent-1",
                ActionCategory::LlmInference,
                "action-1",
                Duration::from_millis(2000),
                None,
            )
            .await;
        collector
            .record(
                "a2",
                "agent-2",
                ActionCategory::ToolExecution,
                "action-2",
                Duration::from_millis(3000),
                None,
            )
            .await;
        collector
            .record(
                "a1",
                "agent-1",
                ActionCategory::LlmInference,
                "action-3",
                Duration::from_millis(1500),
                None,
            )
            .await;

        let a1_traces = collector.by_agent("a1").await;
        assert_eq!(a1_traces.len(), 2);
        let a2_traces = collector.by_agent("a2").await;
        assert_eq!(a2_traces.len(), 1);
    }

    #[tokio::test]
    async fn test_summary_statistics() {
        let collector = SlowTraceCollector::new();

        collector
            .record(
                "a1",
                "agent-1",
                ActionCategory::LlmInference,
                "slow-1",
                Duration::from_millis(2000),
                None,
            )
            .await;
        collector
            .record(
                "a2",
                "agent-2",
                ActionCategory::ToolExecution,
                "slow-2",
                Duration::from_millis(3000),
                None,
            )
            .await;

        let summary = collector.summary().await;
        assert_eq!(summary.total_count, 2);
        assert!(summary.by_category.contains_key("llm_inference"));
        assert!(summary.by_category.contains_key("tool_execution"));
        assert!(summary.avg_duration_ms > 0.0);
        assert_eq!(summary.threshold_ms, 1000);
        assert!(summary.enabled);
    }

    #[tokio::test]
    async fn test_root_cause_inference() {
        let collector = SlowTraceCollector::new();

        // LLM with very long duration → LargeTokenCount
        let result = collector
            .record(
                "a1",
                "agent",
                ActionCategory::LlmInference,
                "huge prompt",
                Duration::from_millis(35000),
                None,
            )
            .await;
        assert_eq!(result.unwrap().root_cause, RootCauseHint::LargeTokenCount);

        // Tool execution → ExternalDependency
        let result = collector
            .record(
                "a1",
                "agent",
                ActionCategory::ToolExecution,
                "slow tool",
                Duration::from_millis(2000),
                None,
            )
            .await;
        assert_eq!(
            result.unwrap().root_cause,
            RootCauseHint::ExternalDependency
        );

        // External API → NetworkLatency
        let result = collector
            .record(
                "a1",
                "agent",
                ActionCategory::ExternalApi,
                "api call",
                Duration::from_millis(2000),
                None,
            )
            .await;
        assert_eq!(result.unwrap().root_cause, RootCauseHint::NetworkLatency);
    }

    #[tokio::test]
    async fn test_custom_threshold() {
        let collector = SlowTraceCollector::with_config(SlowTraceConfig {
            threshold_ms: 500,
            ..Default::default()
        });

        // 600ms is slow with 500ms threshold
        let result = collector
            .record(
                "a1",
                "agent",
                ActionCategory::Other,
                "medium action",
                Duration::from_millis(600),
                None,
            )
            .await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_clear_traces() {
        let collector = SlowTraceCollector::new();
        collector
            .record(
                "a1",
                "agent",
                ActionCategory::Other,
                "action",
                Duration::from_millis(2000),
                None,
            )
            .await;
        assert_eq!(collector.count().await, 1);

        collector.clear().await;
        assert_eq!(collector.count().await, 0);
    }

    #[test]
    fn test_severity_classification() {
        assert_eq!(
            SlowSeverity::from_duration(1500, 1000),
            SlowSeverity::Warning
        );
        assert_eq!(SlowSeverity::from_duration(2500, 1000), SlowSeverity::Slow);
        assert_eq!(
            SlowSeverity::from_duration(6000, 1000),
            SlowSeverity::Critical
        );
    }

    #[test]
    fn test_percentile() {
        let data = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        // p50: idx = 50/100 * 10 = 5 -> sorted[5] = 600
        assert_eq!(percentile(&data, 50.0), 600);
        assert_eq!(percentile(&data, 95.0), 1000);
        assert_eq!(percentile(&data, 99.0), 1000);
        assert_eq!(percentile(&[], 50.0), 0);
    }

    #[test]
    fn test_action_category_display() {
        assert_eq!(ActionCategory::LlmInference.to_string(), "llm_inference");
        assert_eq!(ActionCategory::ToolExecution.to_string(), "tool_execution");
        assert_eq!(
            ActionCategory::KnowledgeRetrieval.to_string(),
            "knowledge_retrieval"
        );
    }

    #[tokio::test]
    async fn test_trace_serialization() {
        let collector = SlowTraceCollector::new();
        let trace = collector
            .record(
                "a1",
                "agent",
                ActionCategory::LlmInference,
                "test",
                Duration::from_millis(2000),
                Some("model-x".to_string()),
            )
            .await
            .unwrap();

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("a1"));
        assert!(json.contains("llm_inference"));
        assert!(json.contains("model-x"));
    }
}
