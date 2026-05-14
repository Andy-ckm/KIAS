use axum::middleware::from_fn_with_state;
use axum::Router;

use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{agents, config, health, knowledge, metrics, nodes, scheduler, tokens, workflows};
use crate::middleware::rate_limit::{RateLimiter, RateLimiterConfig};
use crate::middleware::{auth::auth_middleware, logging::logging_middleware};
use crate::AppState;

/// Build the full application router with all routes and middleware.
///
/// Route layout:
///   /health            GET  — liveness probe (no auth)
///   /readyz            GET  — readiness probe (no auth)
///   /api/v1/agents     GET  — list agents
///   /api/v1/agents     POST — create agent
///   /api/v1/agents/:id GET  — get agent
///   /api/v1/agents/:id DELETE — delete agent
///   /api/v1/agents/:id/status PATCH — update agent status
///   /api/v1/nodes      GET  — list nodes
///   /api/v1/nodes/:id  GET  — get node
///   /api/v1/nodes/:id/agents GET — list agents on node
///   /api/v1/knowledge/search GET — search knowledge
///   /api/v1/metrics/summary  GET — system metrics summary
///   /api/v1/metrics/agents/:id GET — per-agent metrics
///   /api/v1/cluster/status   GET — cluster health overview
///   /api/v1/config     GET  — get sanitized config
///   /api/v1/config     PATCH — update config (Admin only)
///   /api/v1/config/audit-log GET — config audit log
///   /api/v1/tokens     GET  — token usage analytics
///   /api/v1/workflows  GET  — list workflows
///   /api/v1/workflows  POST — create workflow
///   /api/v1/workflows/:id GET — get workflow
///   /api/v1/workflows/:id DELETE — delete workflow
///   /api/v1/scheduler/status GET — scheduler status
pub fn create_router(state: AppState) -> Router {
    // --- Rate limiter ---
    let rate_limiter = RateLimiter::new(RateLimiterConfig {
        requests_per_second: 10.0,
        burst_size: 20.0,
    });

    // --- Public routes (no auth) ---
    let public_routes = Router::new()
        .route("/health", axum::routing::get(health::liveness))
        .route("/readyz", axum::routing::get(health::readiness));

    // --- Agent routes ---
    let agent_routes = Router::new()
        .route(
            "/api/v1/agents",
            axum::routing::get(agents::list_agents).post(agents::create_agent),
        )
        .route(
            "/api/v1/agents/:id",
            axum::routing::get(agents::get_agent).delete(agents::delete_agent),
        )
        .route(
            "/api/v1/agents/:id/status",
            axum::routing::patch(agents::update_agent_status),
        );

    // --- Node routes ---
    let node_routes = Router::new()
        .route("/api/v1/nodes", axum::routing::get(nodes::list_nodes))
        .route("/api/v1/nodes/:id", axum::routing::get(nodes::get_node))
        .route(
            "/api/v1/nodes/:id/agents",
            axum::routing::get(nodes::list_node_agents),
        );

    // --- Knowledge routes ---
    let knowledge_routes = Router::new().route(
        "/api/v1/knowledge/search",
        axum::routing::get(knowledge::search_knowledge),
    );

    // --- Metrics routes ---
    let metrics_routes = Router::new()
        .route(
            "/api/v1/metrics/summary",
            axum::routing::get(metrics::metrics_summary),
        )
        .route(
            "/api/v1/metrics/agents/:id",
            axum::routing::get(metrics::agent_metrics),
        )
        .route(
            "/api/v1/cluster/status",
            axum::routing::get(metrics::cluster_status),
        );

    // --- Config routes ---
    let config_routes = Router::new()
        .route(
            "/api/v1/config",
            axum::routing::get(config::get_config).patch(config::update_config),
        )
        .route(
            "/api/v1/config/audit-log",
            axum::routing::get(config::config_audit_log),
        );

    // --- Token analytics routes ---
    let token_routes = Router::new()
        .route("/api/v1/tokens", axum::routing::get(tokens::token_analytics));

    // --- Workflow routes ---
    let workflow_routes = Router::new()
        .route(
            "/api/v1/workflows",
            axum::routing::get(workflows::list_workflows).post(workflows::create_workflow),
        )
        .route(
            "/api/v1/workflows/:id",
            axum::routing::get(workflows::get_workflow).delete(workflows::delete_workflow),
        );

    // --- Scheduler routes ---
    let scheduler_routes = Router::new().route(
        "/api/v1/scheduler/status",
        axum::routing::get(scheduler::scheduler_status),
    );

    // --- Combine API routes (rate-limit → auth-protected) ---
    let api_routes = agent_routes
        .merge(node_routes)
        .merge(knowledge_routes)
        .merge(metrics_routes)
        .merge(config_routes)
        .merge(token_routes)
        .merge(workflow_routes)
        .merge(scheduler_routes)
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .layer(from_fn_with_state(
            rate_limiter,
            crate::middleware::rate_limit::rate_limit_middleware,
        ));

    // --- CORS layer (allow all origins for dev) ---
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // --- Final assembly ---
    Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .layer(from_fn_with_state(state.clone(), logging_middleware))
        .layer(cors)
        .with_state(state)
}
