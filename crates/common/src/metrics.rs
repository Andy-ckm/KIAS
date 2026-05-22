//! Prometheus metric definitions for the KIAS system.
//!
//! All metrics are lazily initialised via [`once_cell::sync::Lazy`].

use once_cell::sync::Lazy;
use prometheus::{
    Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};

// ── Global registry ───────────────────────────────────────────────────

/// The global Prometheus registry for KIAS.
pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

// ── Agent metrics ─────────────────────────────────────────────────────

/// Total number of agent creation requests.
pub static AGENT_CREATE_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let counter = IntCounter::with_opts(Opts::new(
        "kias_agent_create_total",
        "Total agent creation requests",
    ))
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(counter.clone()));
    counter
});

/// Current number of running agents, labelled by node.
pub static AGENTS_RUNNING: Lazy<IntGaugeVec> = Lazy::new(|| {
    let gauge = IntGaugeVec::new(
        Opts::new("kias_agents_running", "Running agents by node"),
        &["node_id"],
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(gauge.clone()));
    gauge
});

// ── Scheduler metrics ─────────────────────────────────────────────────

/// Total scheduling decisions, labelled by result (`success`, `no_node`, `error`).
pub static SCHEDULER_DECISIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    let counter = IntCounterVec::new(
        Opts::new(
            "kias_scheduler_decisions_total",
            "Total scheduling decisions",
        ),
        &["result"],
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(counter.clone()));
    counter
});

/// Scheduling latency histogram (seconds).
pub static SCHEDULER_LATENCY: Lazy<Histogram> = Lazy::new(|| {
    let histogram = Histogram::with_opts(
        HistogramOpts::new(
            "kias_scheduler_latency_seconds",
            "Scheduling decision latency",
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(histogram.clone()));
    histogram
});

// ── Cache metrics ─────────────────────────────────────────────────────

/// Cache hit / miss counters, labelled by cache type (`prefix`, `semantic`).
pub static CACHE_OPERATIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    let counter = IntCounterVec::new(
        Opts::new(
            "kias_cache_operations_total",
            "Cache operations by type and result",
        ),
        &["cache_type", "result"],
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(counter.clone()));
    counter
});

// ── Token metrics ─────────────────────────────────────────────────────

/// Total tokens processed, labelled by direction (`input`, `output`).
pub static TOKENS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let counter = IntCounterVec::new(
        Opts::new("kias_tokens_total", "Total tokens processed"),
        &["direction"],
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(counter.clone()));
    counter
});

/// Total estimated cost in USD cents.
pub static COST_TOTAL_CENTS: Lazy<IntCounter> = Lazy::new(|| {
    let counter = IntCounter::with_opts(Opts::new(
        "kias_cost_total_cents",
        "Total estimated cost in USD cents",
    ))
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(counter.clone()));
    counter
});

// ── Node metrics ──────────────────────────────────────────────────────

/// Node resource utilisation gauges, labelled by node and resource type.
pub static NODE_UTILISATION: Lazy<IntGaugeVec> = Lazy::new(|| {
    let gauge = IntGaugeVec::new(
        Opts::new(
            "kias_node_utilisation_percent",
            "Node resource utilisation percentage",
        ),
        &["node_id", "resource"],
    )
    .expect("valid Prometheus metric definition");
    let _ = REGISTRY.register(Box::new(gauge.clone()));
    gauge
});

// ── Encoder helper ────────────────────────────────────────────────────

/// Encode all registered metrics into Prometheus text format.
pub fn encode_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder
        .encode_to_string(&metric_families)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_metrics_not_empty() {
        // Touch some metrics so they exist.
        AGENT_CREATE_TOTAL.inc();
        CACHE_OPERATIONS.with_label_values(&["prefix", "hit"]).inc();

        let output = encode_metrics();
        assert!(output.contains("kias_agent_create_total"));
        assert!(output.contains("kias_cache_operations_total"));
    }

    #[test]
    fn test_agent_create_total_increments() {
        let before = AGENT_CREATE_TOTAL.get();
        AGENT_CREATE_TOTAL.inc();
        let after = AGENT_CREATE_TOTAL.get();
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_agents_running_gauge() {
        AGENTS_RUNNING.with_label_values(&["node-1"]).set(5);
        assert_eq!(AGENTS_RUNNING.with_label_values(&["node-1"]).get(), 5);
        AGENTS_RUNNING.with_label_values(&["node-1"]).inc();
        assert_eq!(AGENTS_RUNNING.with_label_values(&["node-1"]).get(), 6);
    }

    #[test]
    fn test_scheduler_decisions_counter() {
        SCHEDULER_DECISIONS.with_label_values(&["success"]).inc();
        SCHEDULER_DECISIONS.with_label_values(&["success"]).inc();
        SCHEDULER_DECISIONS.with_label_values(&["no_node"]).inc();
        assert_eq!(SCHEDULER_DECISIONS.with_label_values(&["success"]).get(), 2);
        assert_eq!(SCHEDULER_DECISIONS.with_label_values(&["no_node"]).get(), 1);
    }

    #[test]
    fn test_scheduler_latency_histogram() {
        SCHEDULER_LATENCY.observe(0.01);
        SCHEDULER_LATENCY.observe(0.05);
        let metric_families = REGISTRY.gather();
        let histogram = metric_families
            .iter()
            .find(|mf| mf.get_name() == "kias_scheduler_latency_seconds")
            .expect("histogram not found");
        assert_eq!(histogram.get_metric().len(), 1);
    }

    #[test]
    fn test_tokens_total_counter() {
        TOKENS_TOTAL.with_label_values(&["input"]).inc_by(100);
        TOKENS_TOTAL.with_label_values(&["output"]).inc_by(50);
        assert_eq!(TOKENS_TOTAL.with_label_values(&["input"]).get(), 100);
        assert_eq!(TOKENS_TOTAL.with_label_values(&["output"]).get(), 50);
    }

    #[test]
    fn test_cost_total_cents() {
        COST_TOTAL_CENTS.inc_by(25);
        assert!(COST_TOTAL_CENTS.get() >= 25);
    }

    #[test]
    fn test_node_utilisation_gauge() {
        NODE_UTILISATION
            .with_label_values(&["node-1", "cpu"])
            .set(75);
        NODE_UTILISATION
            .with_label_values(&["node-1", "memory"])
            .set(60);
        assert_eq!(
            NODE_UTILISATION.with_label_values(&["node-1", "cpu"]).get(),
            75
        );
        assert_eq!(
            NODE_UTILISATION
                .with_label_values(&["node-1", "memory"])
                .get(),
            60
        );
    }

    #[test]
    fn test_encode_metrics_contains_all_types() {
        // Touch all metric types
        AGENT_CREATE_TOTAL.inc();
        AGENTS_RUNNING.with_label_values(&["test"]).set(1);
        SCHEDULER_DECISIONS.with_label_values(&["test"]).inc();
        SCHEDULER_LATENCY.observe(0.1);
        CACHE_OPERATIONS.with_label_values(&["test", "test"]).inc();
        TOKENS_TOTAL.with_label_values(&["test"]).inc();
        COST_TOTAL_CENTS.inc();
        NODE_UTILISATION.with_label_values(&["test", "test"]).set(1);

        let output = encode_metrics();
        assert!(output.contains("kias_agent_create_total"));
        assert!(output.contains("kias_agents_running"));
        assert!(output.contains("kias_scheduler_decisions_total"));
        assert!(output.contains("kias_scheduler_latency_seconds"));
        assert!(output.contains("kias_cache_operations_total"));
        assert!(output.contains("kias_tokens_total"));
        assert!(output.contains("kias_cost_total_cents"));
        assert!(output.contains("kias_node_utilisation_percent"));
    }
}
