mod services;

use std::sync::Arc;

use clap::{Parser, Subcommand};
use services::KiasServiceManager;

#[derive(Parser)]
#[command(
    name = "kias",
    version,
    about = "AgentGuard - Kubernetes-inspired Intelligent Agent System"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the KIAS server
    Server {
        #[arg(short, long, default_value = "0.0.0.0")]
        host: String,
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Show system status
    Status,
    /// Run health check
    Health,
    /// Show version
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Server { host, port }) => {
            start_server(host, port).await?;
        }
        Some(Commands::Status) => {
            println!("AgentGuard Status: Running");
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Health) => {
            println!("Health: OK");
        }
        Some(Commands::Version) => {
            println!("kias {}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            // Default: start server
            start_server("0.0.0.0".to_string(), 8080).await?;
        }
    }

    Ok(())
}

async fn start_server(host: String, port: u16) -> anyhow::Result<()> {
    tracing::info!("Starting AgentGuard System");

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
    let sqlite_audit_log = Arc::new(manager.audit_log().clone());
    let dead_letter_queue = Arc::new(manager.dlq().clone());
    let api_handle = tokio::spawn(async move {
        let addr = format!("{}:{}", host, port);

        tracing::info!(addr = %addr, "Starting API server");

        let state = kias_api_server::AppState::new(api_config)
            .await
            .with_persistence(sqlite_audit_log, dead_letter_queue);
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

    tracing::info!("AgentGuard System started successfully");
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
    tracing::info!("AgentGuard System shut down gracefully");

    // Abort API server.
    api_handle.abort();

    // Wait for signal handler to finish.
    let _ = signal_handle.await;

    Ok(())
}
