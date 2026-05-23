use axum::middleware::from_fn_with_state;
use axum::Router;

use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{
    a2a, agents, config, context, dashboard, health, im, knowledge, metrics, nl_command, nodes,
    scheduler, slow_trace, tier_routing, token_budget, tokens, visualization, workflows,
};
use crate::middleware::rate_limit::{RateLimiter, RateLimiterConfig};
use crate::middleware::{auth::auth_middleware, idempotency::idempotency_middleware, logging::logging_middleware};
use crate::AppState;

/// Build the full application router with all routes and middleware.
///
/// Route layout:
///   /health            GET  — liveness probe (no auth)
///   /readyz            GET  — readiness probe (no auth)
///   /.well-known/agent.json GET — A2A agent card discovery (no auth)
///   /api/v1/agents     GET  — list agents
///   /api/v1/agents     POST — create agent
///   /api/v1/agents/:id GET  — get agent
///   /api/v1/agents/:id DELETE — delete agent
///   /api/v1/agents/:id/invoke POST — invoke agent with prompt (CI-friendly)
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
///   /a2a/v1/agents     GET  — list A2A agent cards
///   /a2a/v1/agents/:id GET  — get specific A2A agent card
///   /a2a/v1/tasks      GET  — list A2A tasks
///   /a2a/v1/tasks      POST — send async A2A task
///   /a2a/v1/tasks/:id  GET  — get A2A task status
///   /a2a/v1/tasks/:id  DELETE — delete A2A task
///   /a2a/v1/tasks/:id/cancel POST — cancel A2A task
///   /a2a/v1/tasks/:id/stream GET — SSE stream for task updates
///   /a2a/v1/fire       POST — synchronous fire-and-wait (Sembr-inspired)
pub fn create_router(state: AppState) -> Router {
    // --- Rate limiter ---
    let rate_limiter = RateLimiter::new(RateLimiterConfig {
        requests_per_second: 10.0,
        burst_size: 20.0,
    });

    // --- Public routes (no auth) ---
    let public_routes = Router::new()
        .route("/health", axum::routing::get(health::liveness))
        .route("/readyz", axum::routing::get(health::readiness))
        .route("/healthz/deep", axum::routing::get(health::deep_health))
        .route("/ws", axum::routing::get(crate::websocket::ws_handler))
        .route(
            "/.well-known/agent.json",
            axum::routing::get(a2a::well_known_agent_card),
        )
        .route(
            "/api/v1/ws/stats",
            axum::routing::get(crate::websocket::ws_stats_handler),
        );

    // --- Agent routes ---
    let agent_routes = Router::new()
        .route(
            "/api/v1/agents",
            axum::routing::get(agents::list_agents).post(agents::create_agent),
        )
        .route(
            "/api/v1/agents/:id/invoke",
            axum::routing::post(agents::invoke_agent),
        )
        .route(
            "/api/v1/agents/:id/status",
            axum::routing::patch(agents::update_agent_status),
        )
        .route(
            "/api/v1/agents/:id",
            axum::routing::get(agents::get_agent).delete(agents::delete_agent),
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
    let knowledge_routes = Router::new()
        .route(
            "/api/v1/knowledge/search",
            axum::routing::get(knowledge::search_knowledge),
        )
        .route(
            "/api/v1/knowledge/ingest",
            axum::routing::post(knowledge::ingest_document),
        )
        .route(
            "/api/v1/knowledge/ingest-file",
            axum::routing::post(knowledge::ingest_file),
        )
        .route(
            "/api/v1/knowledge/documents",
            axum::routing::get(knowledge::list_documents),
        );

    // --- Context Manager routes ---
    let context_routes = Router::new()
        .route(
            "/api/v1/context/:session_id/messages",
            axum::routing::post(context::add_message),
        )
        .route(
            "/api/v1/context/:session_id/compress",
            axum::routing::post(context::compress_session),
        )
        .route(
            "/api/v1/context/:session_id/stats",
            axum::routing::get(context::get_context_stats),
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
        .route(
            "/api/v1/tokens",
            axum::routing::get(tokens::token_analytics),
        )
        // --- Real-time Dashboard route ---
        .route(
            "/api/v1/dashboard/realtime",
            axum::routing::get(dashboard::realtime_dashboard),
        );

    // --- Token Budget Management routes ---
    let token_budget_routes = Router::new()
        .route(
            "/api/v1/tokens/budget",
            axum::routing::get(token_budget::budget_overview),
        )
        .route(
            "/api/v1/tokens/budget/:agent_id",
            axum::routing::get(token_budget::agent_budget)
                .put(token_budget::set_budget)
                .delete(token_budget::remove_budget),
        );

    let token_routes = token_routes.merge(token_budget_routes);

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

    // --- Slow Action Tracing routes (surpasses EMQ slow subscription tracking) ---
    let slow_trace_routes = Router::new()
        .route(
            "/api/v1/observability/slow-traces",
            axum::routing::get(slow_trace::list_slow_traces).delete(slow_trace::clear_slow_traces),
        )
        .route(
            "/api/v1/observability/slow-traces/summary",
            axum::routing::get(slow_trace::slow_trace_summary),
        )
        .route(
            "/api/v1/observability/slow-traces/agent/:id",
            axum::routing::get(slow_trace::agent_slow_traces),
        )
        .route(
            "/api/v1/observability/slow-traces/config",
            axum::routing::put(slow_trace::update_slow_trace_config),
        );

    // --- Tier routing routes (PrfaaS-inspired intelligent task routing) ---
    let tier_routing_routes = Router::new()
        .route(
            "/api/v1/routing/evaluate",
            axum::routing::post(tier_routing::evaluate_task),
        )
        .route(
            "/api/v1/routing/batch",
            axum::routing::post(tier_routing::batch_evaluate),
        )
        .route(
            "/api/v1/routing/tiers",
            axum::routing::get(tier_routing::list_tiers),
        )
        .route(
            "/api/v1/routing/pool/register",
            axum::routing::post(tier_routing::register_agent),
        )
        .route(
            "/api/v1/routing/pool/status",
            axum::routing::get(tier_routing::pool_status),
        );

    // --- A2A (Agent-to-Agent) protocol routes ---
    let a2a_routes = Router::new()
        .route("/a2a/v1/agents", axum::routing::get(a2a::list_agent_cards))
        .route(
            "/a2a/v1/agents/:id",
            axum::routing::get(a2a::get_agent_card),
        )
        .route(
            "/a2a/v1/tasks",
            axum::routing::get(a2a::list_tasks).post(a2a::send_task),
        )
        .route(
            "/a2a/v1/tasks/:id",
            axum::routing::get(a2a::get_task).delete(a2a::delete_task),
        )
        .route(
            "/a2a/v1/tasks/:id/cancel",
            axum::routing::post(a2a::cancel_task),
        )
        .route(
            "/a2a/v1/tasks/:id/stream",
            axum::routing::get(a2a::stream_task),
        )
        // Sembr-inspired synchronous fire-and-wait endpoint
        .route("/a2a/v1/fire", axum::routing::post(a2a::fire_agent));

    // --- Natural Language command routes ---
    let nl_routes = Router::new()
        .route(
            "/api/v1/nl/command",
            axum::routing::post(nl_command::nl_command),
        )
        .route(
            "/api/v1/nl/stream",
            axum::routing::post(nl_command::nl_stream),
        );

    // --- Intent recognition routes ---
    let intent_routes = Router::new()
        .route(
            "/api/v1/intent/recognize",
            axum::routing::post(nl_command::recognize_intent),
        )
        .route(
            "/api/v1/intent/decompose",
            axum::routing::post(nl_command::decompose_task),
        );

    // --- IM integration routes ---
    let im_routes = Router::new()
        .route("/api/v1/im/webhook", axum::routing::post(im::im_webhook))
        .route("/api/v1/im/wechat", axum::routing::post(im::wechat_webhook))
        .route(
            "/api/v1/im/telegram",
            axum::routing::post(im::telegram_webhook),
        )
        .route("/api/v1/im/feishu", axum::routing::post(im::feishu_webhook))
        .route(
            "/api/v1/im/platforms",
            axum::routing::get(im::list_platforms),
        );

    // --- Visualization routes (GxP compliance dashboards) ---
    let visualization_routes = Router::new()
        .route(
            "/api/v1/viz/knowledge-graph",
            axum::routing::get(visualization::get_knowledge_graph),
        )
        .route(
            "/api/v1/viz/document-relations",
            axum::routing::get(visualization::get_document_relations),
        )
        .route(
            "/api/v1/viz/audit-timeline",
            axum::routing::get(visualization::get_audit_timeline),
        )
        .route(
            "/api/v1/viz/compliance-status",
            axum::routing::get(visualization::get_compliance_status),
        )
        .route(
            "/viz/knowledge-graph",
            axum::routing::get(visualization::knowledge_graph_html),
        )
        .route(
            "/viz/compliance-dashboard",
            axum::routing::get(visualization::compliance_dashboard_html),
        )
        .route(
            "/viz/audit-timeline",
            axum::routing::get(visualization::audit_timeline_html),
        );

    // --- Combine API routes (rate-limit → auth-protected) ---
    let api_routes = agent_routes
        .merge(node_routes)
        .merge(knowledge_routes)
        .merge(context_routes)
        .merge(metrics_routes)
        .merge(config_routes)
        .merge(token_routes)
        .merge(workflow_routes)
        .merge(scheduler_routes)
        .merge(slow_trace_routes)
        .merge(tier_routing_routes)
        .merge(a2a_routes)
        .merge(nl_routes)
        .merge(intent_routes)
        .merge(im_routes)
        .merge(visualization_routes)
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .layer(from_fn_with_state(
            rate_limiter,
            crate::middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(from_fn_with_state(
            state.clone(),
            idempotency_middleware,
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        AppState::new_async(config).await
    }

    async fn test_state_with_auth() -> AppState {
        let mut config = kias_common::config::KiasConfig::default();
        config.api_server.auth_enabled = true;
        config.api_server.jwt_secret = Some("test-secret".to_string());
        config.api_server.auth_tokens = vec!["test-api-key".to_string()];
        AppState::new_async(config).await
    }

    #[tokio::test]
    async fn test_health_endpoint_public() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readyz_endpoint_public() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_deep_health_endpoint_public() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz/deep")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_wrong_method_returns_405() {
        let app = create_router(test_state().await);
        // /health only accepts GET, POST should return 405
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            response.status() == StatusCode::METHOD_NOT_ALLOWED
                || response.status() == StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn test_list_agents_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_nodes_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/nodes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_scheduler_status_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/scheduler/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_token_analytics_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tokens")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_workflows_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/workflows")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cluster_status_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cluster/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_summary_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/metrics/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_a2a_agent_cards_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/a2a/v1/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_a2a_tasks_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/a2a/v1/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_well_known_agent_card() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/.well-known/agent.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_enabled_rejects_no_token() {
        let app = create_router(test_state_with_auth().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_accepts_api_key() {
        let app = create_router(test_state_with_auth().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents")
                    .header("Authorization", "Bearer test-api-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_enabled_rejects_wrong_key() {
        let app = create_router(test_state_with_auth().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents")
                    .header("Authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_health_still_public() {
        let app = create_router(test_state_with_auth().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Health is in public_routes, should bypass auth
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cors_headers_present() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("Origin", "https://example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // CORS layer should add access-control-allow-origin
        let cors_header = response.headers().get("access-control-allow-origin");
        assert!(cors_header.is_some(), "CORS header should be present");
    }

    #[tokio::test]
    async fn test_im_platforms_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/im/platforms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_routing_tiers_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/routing/tiers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_config_endpoint_auth_disabled() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_context_stats_route_exists() {
        let app = create_router(test_state().await);
        // Non-existent session should return an error but not 404
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/context/test-session/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should not be 404 (route exists) — might be 400/500 depending on handler
        assert_ne!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_knowledge_search_route_exists() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/knowledge/search?q=test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ws_stats_endpoint() {
        let app = create_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/ws/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
