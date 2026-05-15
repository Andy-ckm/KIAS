mod services;

use services::KiasServiceManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS System");

    // Load configuration (uses defaults if no config file found).
    let config = kias_common::KiasConfig::load().unwrap_or_default();

    // Initialize all subsystems via the service manager.
    let manager = KiasServiceManager::new(config.clone()).await?;

    // Run initial health check.
    let health = manager.health_check();
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial health check");

    // Get the graceful shutdown coordinator.
    let shutdown = manager.shutdown_handle();

    // Spawn signal handler (SIGTERM/SIGINT).
    let shutdown_for_signal = shutdown.clone();
    let signal_handle = tokio::spawn(async move {
        kias_common::graceful_shutdown::wait_for_signal().await;
        tracing::info!("Signal received, initiating graceful shutdown");
        shutdown_for_signal.shutdown().await;
    });

    // Start the API server.
    let api_config = config.clone();
    let api_handle = tokio::spawn(async move {
        let host = api_config.api_server.host.clone();
        let port = api_config.api_server.port;
        let addr = format!("{}:{}", host, port);

        tracing::info!(addr = %addr, "Starting API server");

        let state = kias_api_server::AppState::new(api_config).await;
        let app = kias_api_server::routes::api::create_router(state);

        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, "Failed to bind API server");
                return;
            }
        };

        tracing::info!(addr = %addr, "API server listening");

        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "API server error");
        }
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

    // Abort API server.
    api_handle.abort();

    // Wait for signal handler to finish.
    let _ = signal_handle.await;

    Ok(())
}
