use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use tracing::{info, warn};

use crate::agent_span::{AgentSpan, SpanStatus};

/// Configuration for the span collector
#[derive(Debug, Clone)]
pub struct SpanCollectorConfig {
    /// Maximum spans to keep in memory
    pub max_spans: usize,
    /// Whether to collect spans at all
    pub enabled: bool,
    /// OTLP endpoint (e.g., "http://localhost:4317")
    pub otlp_endpoint: Option<String>,
}

impl Default for SpanCollectorConfig {
    fn default() -> Self {
        Self {
            max_spans: 10_000,
            enabled: true,
            otlp_endpoint: None,
        }
    }
}

/// Collects AgentSpans in memory and provides query/export capabilities.
///
/// This is the core of the observability layer — every Agent operation
/// creates a span that flows through this collector.
#[derive(Debug)]
pub struct SpanCollector {
    spans: Arc<RwLock<Vec<AgentSpan>>>,
    config: Arc<RwLock<SpanCollectorConfig>>,
}

impl SpanCollector {
    pub fn new(config: SpanCollectorConfig) -> Self {
        Self {
            spans: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Record a completed span
    pub async fn record(&self, span: AgentSpan) {
        let config = self.config.read().await;
        if !config.enabled {
            return;
        }

        let mut spans = self.spans.write().await;

        // Evict oldest if at capacity
        if spans.len() >= config.max_spans {
            let drain_count = config.max_spans / 10; // evict 10%
            spans.drain(..drain_count);
        }

        spans.push(span);
    }

    /// Record a span and automatically finish it with the given status
    pub async fn record_finished(&self, span: AgentSpan, status: SpanStatus) {
        self.record(span.finish(status)).await;
    }

    /// Get recent spans (newest first)
    pub async fn recent(&self, limit: usize) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans.iter().rev().take(limit).cloned().collect()
    }

    /// Get all spans for a specific agent
    pub async fn by_agent(&self, agent_id: &str) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans
            .iter()
            .filter(|s| s.agent_id == agent_id)
            .cloned()
            .collect()
    }

    /// Get all spans for a specific trace
    pub async fn by_trace(&self, trace_id: &str) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans
            .iter()
            .filter(|s| s.trace_id == trace_id)
            .cloned()
            .collect()
    }

    /// Get spans by operation name
    pub async fn by_name(&self, name: &str) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans
            .iter()
            .filter(|s| s.name == name)
            .cloned()
            .collect()
    }

    /// Get error spans
    pub async fn errors(&self) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans
            .iter()
            .filter(|s| s.status == SpanStatus::Error)
            .cloned()
            .collect()
    }

    /// Get slow spans (duration > threshold_ms)
    pub async fn slow(&self, threshold_ms: i64) -> Vec<AgentSpan> {
        let spans = self.spans.read().await;
        spans
            .iter()
            .filter(|s| s.duration_ms().unwrap_or(0) > threshold_ms)
            .cloned()
            .collect()
    }

    /// Get span count
    pub async fn count(&self) -> usize {
        let spans = self.spans.read().await;
        spans.len()
    }

    /// Get summary statistics
    pub async fn summary(&self) -> SpanSummary {
        let spans = self.spans.read().await;
        let total = spans.len();
        let errors = spans.iter().filter(|s| s.status == SpanStatus::Error).count();
        let in_progress = spans.iter().filter(|s| s.is_in_progress()).count();

        let total_tokens: i64 = spans.iter().filter_map(|s| s.token_count()).sum();
        let total_cost: f64 = spans.iter().filter_map(|s| s.cost_usd()).sum();

        let avg_duration_ms = {
            let finished: Vec<i64> = spans.iter().filter_map(|s| s.duration_ms()).collect();
            if finished.is_empty() {
                0.0
            } else {
                finished.iter().sum::<i64>() as f64 / finished.len() as f64
            }
        };

        SpanSummary {
            total,
            errors,
            in_progress,
            total_tokens,
            total_cost,
            avg_duration_ms,
        }
    }

    /// Clear all spans
    pub async fn clear(&self) {
        let mut spans = self.spans.write().await;
        spans.clear();
    }

    /// Update configuration
    pub async fn update_config(&self, new_config: SpanCollectorConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }

    /// Export spans as JSON (for OTLP HTTP export)
    pub async fn export_json(&self) -> String {
        let spans = self.spans.read().await;
        serde_json::to_string(&*spans).unwrap_or_else(|e| {
            warn!("Failed to serialize spans: {}", e);
            "[]".to_string()
        })
    }
}

/// Summary statistics for spans
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpanSummary {
    pub total: usize,
    pub errors: usize,
    pub in_progress: usize,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_duration_ms: f64,
}

impl Default for SpanCollector {
    fn default() -> Self {
        Self::new(SpanCollectorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_span::{SpanKind, SpanStatus, AgentSpan, AttributeValue};

    #[tokio::test]
    async fn test_record_and_recent() {
        let collector = SpanCollector::default();

        let span = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
            .finish(SpanStatus::Ok);
        collector.record(span).await;

        let recent = collector.recent(10).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "llm.chat");
    }

    #[tokio::test]
    async fn test_by_agent() {
        let collector = SpanCollector::default();

        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal).finish(SpanStatus::Ok)
        ).await;
        collector.record(
            AgentSpan::new("tool.exec", "agent-2", SpanKind::Client).finish(SpanStatus::Ok)
        ).await;
        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal).finish(SpanStatus::Ok)
        ).await;

        let agent1_spans = collector.by_agent("agent-1").await;
        assert_eq!(agent1_spans.len(), 2);

        let agent2_spans = collector.by_agent("agent-2").await;
        assert_eq!(agent2_spans.len(), 1);
    }

    #[tokio::test]
    async fn test_by_trace() {
        let collector = SpanCollector::default();

        let span1 = AgentSpan::new("agent.decide", "agent-1", SpanKind::Internal)
            .finish(SpanStatus::Ok);
        let trace_id = span1.trace_id.clone();

        let span2 = AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
            .with_trace_id(&trace_id)
            .finish(SpanStatus::Ok);

        collector.record(span1).await;
        collector.record(span2).await;

        let trace_spans = collector.by_trace(&trace_id).await;
        assert_eq!(trace_spans.len(), 2);
    }

    #[tokio::test]
    async fn test_errors() {
        let collector = SpanCollector::default();

        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal).finish(SpanStatus::Ok)
        ).await;
        collector.record(
            AgentSpan::new("tool.exec", "agent-1", SpanKind::Client).finish(SpanStatus::Error)
        ).await;

        let errors = collector.errors().await;
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "tool.exec");
    }

    #[tokio::test]
    async fn test_summary() {
        let collector = SpanCollector::default();

        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal)
                .with_attribute("llm.token_count", AttributeValue::Int(1000))
                .with_attribute("agent.cost_usd", AttributeValue::Double(0.03))
                .finish(SpanStatus::Ok)
        ).await;
        collector.record(
            AgentSpan::new("tool.exec", "agent-1", SpanKind::Client)
                .finish(SpanStatus::Error)
        ).await;

        let summary = collector.summary().await;
        assert_eq!(summary.total, 2);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.total_tokens, 1000);
        assert_eq!(summary.total_cost, 0.03);
    }

    #[tokio::test]
    async fn test_eviction() {
        let config = SpanCollectorConfig {
            max_spans: 10,
            enabled: true,
            otlp_endpoint: None,
        };
        let collector = SpanCollector::new(config);

        for i in 0..15 {
            collector.record(
                AgentSpan::new(&format!("op-{}", i), "agent-1", SpanKind::Internal)
                    .finish(SpanStatus::Ok)
            ).await;
        }

        // Should have evicted some
        let count = collector.count().await;
        assert!(count <= 10, "Expected <= 10, got {}", count);
    }

    #[tokio::test]
    async fn test_disabled() {
        let config = SpanCollectorConfig {
            enabled: false,
            ..Default::default()
        };
        let collector = SpanCollector::new(config);

        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal).finish(SpanStatus::Ok)
        ).await;

        assert_eq!(collector.count().await, 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let collector = SpanCollector::default();

        collector.record(
            AgentSpan::new("llm.chat", "agent-1", SpanKind::Internal).finish(SpanStatus::Ok)
        ).await;

        assert_eq!(collector.count().await, 1);
        collector.clear().await;
        assert_eq!(collector.count().await, 0);
    }
}
