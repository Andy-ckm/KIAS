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
    .expect("metric creation failed");
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

/// Current number of running agents, labelled by node.
pub static AGENTS_RUNNING: Lazy<IntGaugeVec> = Lazy::new(|| {
    let gauge = IntGaugeVec::new(
        Opts::new("kias_agents_running", "Running agents by node"),
        &["node_id"],
    )
    .expect("metric creation failed");
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
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
    .expect("metric creation failed");
    REGISTRY.register(Box::new(counter.clone())).unwrap();
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
    .expect("metric creation failed");
    REGISTRY.register(Box::new(histogram.clone())).unwrap();
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
    .expect("metric creation failed");
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

// ── Token metrics ─────────────────────────────────────────────────────

/// Total tokens processed, labelled by direction (`input`, `output`).
pub static TOKENS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let counter = IntCounterVec::new(
        Opts::new("kias_tokens_total", "Total tokens processed"),
        &["direction"],
    )
    .expect("metric creation failed");
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

/// Total estimated cost in USD cents.
pub static COST_TOTAL_CENTS: Lazy<IntCounter> = Lazy::new(|| {
    let counter = IntCounter::with_opts(Opts::new(
        "kias_cost_total_cents",
        "Total estimated cost in USD cents",
    ))
    .expect("metric creation failed");
    REGISTRY.register(Box::new(counter.clone())).unwrap();
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
    .expect("metric creation failed");
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
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
}
