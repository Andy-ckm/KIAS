mod services;

use services::KiasServiceManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS System");

    // Load configuration (uses defaults if no config file found).
    let config = kias_common::KiasConfig::load().unwrap_or_default();

    // Initialize all subsystems via the service manager.
    let manager = KiasServiceManager::new(config).await?;

    // Run initial health check.
    let health = manager.health_check();
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial health check");

    // Get the graceful shutdown coordinator.
    let shutdown = manager.graceful_shutdown();

    // Spawn signal handler (SIGTERM/SIGINT).
    let shutdown_for_signal = shutdown.clone();
    let signal_handle = tokio::spawn(async move {
        kias_common::graceful_shutdown::wait_for_signal().await;
        tracing::info!("Signal received, initiating graceful shutdown");
        shutdown_for_signal.shutdown().await;
    });

    tracing::info!("KIAS System started successfully");
    tracing::info!("Press Ctrl+C to initiate graceful shutdown");

    // Wait for shutdown signal.
    let _ = shutdown
        .wait_for_phase(
            kias_common::graceful_shutdown::ShutdownPhase::Complete,
            std::time::Duration::from_secs(u64::MAX), // Wait forever
        )
        .await;

    // Graceful shutdown.
    manager.shutdown().await?;
    tracing::info!("KIAS System shut down gracefully");

    // Wait for signal handler to finish.
    let _ = signal_handle.await;

    Ok(())
}
