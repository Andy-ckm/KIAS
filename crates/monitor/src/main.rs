use kias_monitor::telemetry::{EventType, Severity, TelemetryEvent};
use kias_monitor::{MetricsCollector, TelemetryCollector};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS Monitor Service");

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

    println!("Collected {} telemetry events", telemetry.count());
    println!("Tasks started: {}", metrics.get_counter("tasks_started"));
    println!(
        "Latency P50: {:.2}ms",
        metrics.histogram_percentile("task_latency", 50.0)
    );
    println!("\nPrometheus export:\n{}", metrics.export_prometheus());

    let stats = telemetry.stats();
    println!(
        "\nEvent stats: {} events, error rate: {:.2}%",
        stats.total_events,
        stats.error_rate * 100.0
    );

    tracing::info!("KIAS Monitor Service finished");
    Ok(())
}
