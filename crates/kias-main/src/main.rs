mod services;

use std::sync::Arc;

use clap::{CommandFactory, Parser, Subcommand};
use services::KiasServiceManager;

#[derive(Parser)]
#[command(
    name = "kias",
    version,
    about = "Policy-driven control plane for operating AI agents safely"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the KIAS control-plane server.
    Server {
        /// Listen address. The loopback default avoids accidental public exposure.
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Show local build information.
    Status,
    /// Validate that local configuration can be loaded.
    Health,
    /// Show version.
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
            println!("KIAS CLI is available");
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Use the authenticated control-plane health endpoint for runtime status.");
        }
        Some(Commands::Health) => {
            let _ = kias_common::KiasConfig::load()?;
            println!("Configuration: valid");
        }
        Some(Commands::Version) => {
            println!("kias {}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}

async fn start_server(host: String, port: u16) -> anyhow::Result<()> {
    tracing::info!("Starting KIAS control plane");

    // Configuration errors fail startup rather than silently selecting a potentially
    // unsafe fallback. The default configuration remains available through the normal
    // configuration loader.
    let config = kias_common::KiasConfig::load()?;

    // Bind before spawning the serving task so a port or address error is returned to
    // the caller instead of being logged after the process reports successful startup.
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Initialize all required subsystems via the composition root.
    let manager = KiasServiceManager::new(config.clone()).await?;

    let health = manager.health_check();
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial health check");

    let shutdown = manager.shutdown_handle();
    let shutdown_for_signal = shutdown.clone();
    let signal_handle = tokio::spawn(async move {
        kias_common::graceful_shutdown::wait_for_signal().await;
        tracing::info!("Signal received, initiating graceful shutdown");
        shutdown_for_signal.shutdown().await;
    });

    let api_config = config.clone();
    let sqlite_audit_log = Arc::new(manager.audit_log().clone());
    let dead_letter_queue = Arc::new(manager.dlq().clone());
    let api_handle = tokio::spawn(async move {
        tracing::info!(addr = %addr, "KIAS API server listening");

        let state = kias_api_server::AppState::new(api_config)
            .await
            .with_persistence(sqlite_audit_log, dead_letter_queue);
        let app = kias_api_server::routes::api::create_router(state);

        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(%error, "KIAS API server stopped with an error");
        }
    });

    tracing::info!("KIAS control plane started");
    tracing::info!("Press Ctrl+C to initiate graceful shutdown");

    let _ = shutdown
        .wait_for_phase(
            kias_common::graceful_shutdown::ShutdownPhase::Complete,
            std::time::Duration::from_secs(u64::MAX),
        )
        .await;

    manager.shutdown().await?;
    tracing::info!("KIAS control plane shut down gracefully");

    api_handle.abort();
    let _ = signal_handle.await;

    Ok(())
}