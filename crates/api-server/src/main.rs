use kias_common::config::KiasConfig;
use kias_common::logging::init_logging_with_level;

use kias_api_server::routes::create_router;
use kias_api_server::AppState;

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging
    init_logging_with_level("info", "text");

    tracing::info!("Starting AgentGuard API Server...");

    // Load configuration
    let config = match KiasConfig::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load config: {e}, using defaults");
            KiasConfig::default()
        }
    };

    let port = config.api_server.port;
    let host = config.api_server.host.clone();

    // Build application state
    let state = AppState::new(config).await;

    // Build router with middleware
    let app = create_router(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("AgentGuard API Server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
