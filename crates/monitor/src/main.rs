use kias_monitor::telemetry::{EventType, Severity, TelemetryEvent};
use kias_monitor::{MetricsCollector, TelemetryCollector};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting AgentGuard Monitor Service");

    let mut telemetry = TelemetryCollector::new();
    let mut metrics = MetricsCollector::new();

    // 测试遥测收集
    let event = TelemetryEvent::new(
        EventType::TaskStarted,
        "agent-1",
        serde_json::json!({"task": "test-task"}),
    )
    .with_severity(Severity::Info);

    telemetry.collect(event);
    metrics.increment_counter("tasks_started", 1);
    metrics.register_histogram("task_latency");
    metrics.observe_histogram("task_latency", 42.0);

    tracing::info!(count = telemetry.count(), "Collected telemetry events");
    tracing::info!(
        count = metrics.get_counter("tasks_started"),
        "Tasks started"
    );
    tracing::info!(
        latency_p50 = %format!("{:.2}ms", metrics.histogram_percentile("task_latency", 50.0)),
        "Latency P50"
    );
    tracing::info!(prometheus = %metrics.export_prometheus(), "Prometheus export");

    let stats = telemetry.stats();
    tracing::info!(
        total_events = stats.total_events,
        error_rate = %format!("{:.2}%", stats.error_rate * 100.0),
        "Event stats"
    );

    tracing::info!("AgentGuard Monitor Service finished");
    Ok(())
}
