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
    /// Issue a short-lived JWT for a local operator or controlled pilot.
    Token {
        /// Token role: viewer, operator, or admin.
        #[arg(long, default_value = "operator")]
        role: String,
        /// Pseudonymous subject recorded in authorization and audit context.
        #[arg(long, default_value = "local-operator")]
        subject: String,
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
        Some(Commands::Token { role, subject }) => issue_local_token(&role, &subject)?,
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

fn issue_local_token(role_name: &str, subject: &str) -> anyhow::Result<()> {
    if subject.trim().is_empty() {
        bail!("token subject must not be empty");
    }

    let config = kias_common::KiasConfig::load()?;
    let secret = config
        .api_server
        .jwt_secret
        .as_deref()
        .context("JWT token issuance requires KIAS_API_SERVER__JWT_SECRET")?;
    let role = match role_name.trim().to_ascii_lowercase().as_str() {
        "viewer" => kias_api_server::auth::Role::Viewer,
        "operator" => kias_api_server::auth::Role::Operator,
        "admin" => kias_api_server::auth::Role::Admin,
        _ => bail!("unsupported role; use viewer, operator, or admin"),
    };
    let issuer = config
        .api_server
        .jwt_issuer
        .clone()
        .unwrap_or_else(|| "kias".to_string());
    let jwt_config = kias_api_server::auth::JwtConfig::new(
        secret,
        issuer,
        config.api_server.jwt_expiration_hours,
    );
    let claims = kias_api_server::auth::create_claims(subject.trim(), role, &jwt_config);
    let token = kias_api_server::auth::generate_token(&claims, &jwt_config.secret)?;

    // Keep stdout machine-readable so operators can pipe the value into a
    // password manager or paste it into the Dashboard connection gate.
    println!("{token}");
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
    if !health.is_healthy() {
        bail!("initial KIAS readiness check failed: {}", health.overall);
    }
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
    let mut api_handle = tokio::spawn(async move {
        tracing::info!(address = %address, "KIAS API server listening");

        let state = kias_api_server::AppState::new(api_config)
            .await
            .with_persistence(sqlite_audit_log, dead_letter_queue);
        let application = kias_api_server::routes::create_router(state);

        axum::serve(listener, application).await
    });

    tracing::info!("KIAS control plane started");
    tracing::info!("Press Ctrl+C to initiate graceful shutdown");

    tokio::select! {
        _ = shutdown.wait_for_phase(
            kias_common::graceful_shutdown::ShutdownPhase::Complete,
            std::time::Duration::from_secs(u64::MAX),
        ) => {}
        server_result = &mut api_handle => {
            match server_result {
                Ok(Ok(())) => bail!("KIAS API server stopped unexpectedly"),
                Ok(Err(error)) => return Err(error).context("KIAS API server failed"),
                Err(error) => return Err(error).context("KIAS API server task failed"),
            }
        }
    }

    manager.shutdown().await?;
    tracing::info!("KIAS control plane shut down gracefully");

    if !api_handle.is_finished() {
        api_handle.abort();
    }
    signal_handle.abort();

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

        let proxy_acknowledged = env_flag("KIAS_TRUSTED_TLS_PROXY");
        let local_container_mode = env_flag("KIAS_LOCAL_CONTAINER_MODE");
        if !proxy_acknowledged && !local_container_mode {
            bail!(
                "refusing plaintext non-loopback listener; use a trusted TLS proxy or set KIAS_LOCAL_CONTAINER_MODE=true only when the container port is published to host loopback"
            );
        }

        if local_container_mode && !proxy_acknowledged {
            tracing::warn!(
                "Local container mode permits a plaintext container listener; publish the host port to 127.0.0.1 only"
            );
        }
    }

    Ok(())
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
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

    #[test]
    fn environment_flags_are_explicit() {
        let key = "KIAS_TEST_BOOLEAN_FLAG";
        std::env::set_var(key, "yes");
        assert!(env_flag(key));
        std::env::set_var(key, "no");
        assert!(!env_flag(key));
        std::env::remove_var(key);
    }
}
