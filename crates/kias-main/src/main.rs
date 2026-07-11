mod services;

use std::net::IpAddr;
use std::sync::Arc;

use anyhow::{bail, Context};
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
        /// Override the configured listen address.
        #[arg(short, long)]
        host: Option<String>,
        /// Override the configured listen port.
        #[arg(short, long)]
        port: Option<u16>,
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
        Some(Commands::Server { host, port }) => start_server(host, port).await?,
        Some(Commands::Status) => {
            println!("KIAS CLI is available");
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Use the authenticated control-plane health endpoint for runtime status.");
        }
        Some(Commands::Health) => {
            let _ = kias_common::KiasConfig::load()?;
            println!("Configuration file: readable");
            println!("Run `kias server` to perform full startup validation.");
        }
        Some(Commands::Version) => println!("kias {}", env!("CARGO_PKG_VERSION")),
        None => {
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}

async fn start_server(
    host_override: Option<String>,
    port_override: Option<u16>,
) -> anyhow::Result<()> {
    tracing::info!("Starting KIAS control plane");

    let mut config = kias_common::KiasConfig::load()?;
    let host = host_override.unwrap_or_else(|| config.api_server.host.clone());
    let port = port_override.unwrap_or(config.api_server.port);
    config.api_server.host.clone_from(&host);
    config.api_server.port = port;

    validate_listener_security(&host, &config)?;

    let address = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind KIAS listener at {address}"))?;

    let manager = KiasServiceManager::new(config.clone()).await?;
    let health = manager.health_check();
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial readiness check");

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
        tracing::info!(address = %address, "KIAS API server listening");

        let state = kias_api_server::AppState::new(api_config)
            .await
            .with_persistence(sqlite_audit_log, dead_letter_queue);
        let application = kias_api_server::routes::api::create_router(state);

        if let Err(error) = axum::serve(listener, application).await {
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

fn validate_listener_security(host: &str, config: &kias_common::KiasConfig) -> anyhow::Result<()> {
    if config.api_server.tls {
        bail!(
            "native TLS is not wired into the kias server binary; terminate TLS at a trusted proxy and keep api_server.tls=false"
        );
    }

    if !is_loopback_host(host) {
        if !config.api_server.auth_enabled {
            bail!("refusing a non-loopback listener while authentication is disabled");
        }

        let proxy_acknowledged = std::env::var("KIAS_TRUSTED_TLS_PROXY")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !proxy_acknowledged {
            bail!(
                "refusing plaintext public listener; place KIAS behind a trusted TLS proxy and set KIAS_TRUSTED_TLS_PROXY=true"
            );
        }
    }

    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_loopback_hosts() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("::"));
    }

    #[test]
    fn rejects_public_listener_without_authentication() {
        let config = kias_common::KiasConfig::default();
        let error = validate_listener_security("0.0.0.0", &config).unwrap_err();
        assert!(error.to_string().contains("authentication"));
    }

    #[test]
    fn rejects_unimplemented_native_tls_mode() {
        let mut config = kias_common::KiasConfig::default();
        config.api_server.tls = true;
        let error = validate_listener_security("127.0.0.1", &config).unwrap_err();
        assert!(error.to_string().contains("native TLS"));
    }
}
