
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

    // Get a shutdown handle for background tasks.
    let _shutdown_handle = manager.shutdown_handle();

    tracing::info!("KIAS System started successfully");

    // Wait for Ctrl-C.
    tokio::signal::ctrl_c().await?;

    // Graceful shutdown.
    manager.shutdown().await?;
    tracing::info!("KIAS System shut down gracefully");

    Ok(())
}
