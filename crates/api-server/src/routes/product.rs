use axum::middleware::from_fn_with_state;
use axum::Router;

use crate::handlers::{
    a2a, agents, capabilities, config, context, dashboard, health, im, knowledge, metrics,
    nl_command, nodes, scheduler, slow_trace, tier_routing, token_budget, tokens, visualization,
    workflows,
};
use crate::middleware::rate_limit::{RateLimiter, RateLimiterConfig};
use crate::middleware::{
    auth::auth_middleware, idempotency::idempotency_middleware, logging::logging_middleware,
};
use crate::surfaces::SurfaceConfig;
use crate::AppState;

/// Build the effective KIAS product router.
///
/// The default instance mounts only the supported Core control-plane surface.
/// Extensions and Labs are added only through explicit runtime opt-ins. This
/// keeps the running product aligned with the documented support contract.
pub fn create_router(state: AppState) -> Router {
    let surfaces = SurfaceConfig::from_env(&state.config);
    create_router_with_surfaces(state, surfaces)
}

pub fn create_router_with_surfaces(state: AppState, surfaces: SurfaceConfig) -> Router {
    let rate_limiter = RateLimiter::new(RateLimiterConfig {
        requests_per_second: 10.0,
        burst_size: 20.0,
    });

    // Public probes intentionally reveal only basic process availability.
    let mut public_routes = Router::new()
        .route("/health", axum::routing::get(health::liveness))
        .route("/readyz", axum::routing::get(health::readiness));

    // Protocol discovery and the pre-1.0 realtime stream are explicit opt-ins.
    if surfaces.a2a {
        public_routes = public_routes.route(
            "/.well-known/agent.json",
            axum::routing::get(a2a::well_known_agent_card),
        );
    }
    if surfaces.realtime {
        public_routes =
            public_routes.route("/ws", axum::routing::get(crate::websocket::ws_handler));
    }

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

    let node_routes = Router::new()
        .route("/api/v1/nodes", axum::routing::get(nodes::list_nodes))
        .route("/api/v1/nodes/:id", axum::routing::get(nodes::get_node))
        .route(
            "/api/v1/nodes/:id/agents",
            axum::routing::get(nodes::list_node_agents),
        );

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

    // Runtime mutation was removed from the default product surface because the
    // legacy PATCH handler validated values but did not apply them. Configuration
    // remains an explicit deployment input until a transactional config service
    // exists.
    let config_routes = Router::new()
        .route("/api/v1/config", axum::routing::get(config::get_config))
        .route(
            "/api/v1/config/audit-log",
            axum::routing::get(config::config_audit_log),
        );

    let token_routes = Router::new()
        .route(
            "/api/v1/tokens",
            axum::routing::get(tokens::token_analytics),
        )
        .route(
            "/api/v1/dashboard/realtime",
            axum::routing::get(dashboard::realtime_dashboard),
        )
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

    let workflow_routes = Router::new()
        .route(
            "/api/v1/workflows",
            axum::routing::get(workflows::list_workflows).post(workflows::create_workflow),
        )
        .route(
            "/api/v1/workflows/:id",
            axum::routing::get(workflows::get_workflow).delete(workflows::delete_workflow),
        );

    let scheduler_routes = Router::new().route(
        "/api/v1/scheduler/status",
        axum::routing::get(scheduler::scheduler_status),
    );

    let evidence_routes = Router::new()
        .route(
            "/api/v1/system/capabilities",
            axum::routing::get(capabilities::get_capabilities),
        )
        .route("/healthz/deep", axum::routing::get(health::deep_health))
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

    let mut protected_routes = agent_routes
        .merge(node_routes)
        .merge(metrics_routes)
        .merge(config_routes)
        .merge(token_routes)
        .merge(workflow_routes)
        .merge(scheduler_routes)
        .merge(evidence_routes);

    if surfaces.realtime {
        protected_routes = protected_routes.route(
            "/api/v1/ws/stats",
            axum::routing::get(crate::websocket::ws_stats_handler),
        );
    }

    if surfaces.knowledge {
        protected_routes = protected_routes.merge(
            Router::new()
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
                ),
        );
    }

    if surfaces.context {
        protected_routes = protected_routes.merge(
            Router::new()
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
                ),
        );
    }

    if surfaces.tier_routing {
        protected_routes = protected_routes.merge(
            Router::new()
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
                ),
        );
    }

    if surfaces.a2a {
        protected_routes = protected_routes.merge(
            Router::new()
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
                .route("/a2a/v1/fire", axum::routing::post(a2a::fire_agent)),
        );
    }

    if surfaces.nl_commands {
        protected_routes = protected_routes.merge(
            Router::new()
                .route(
                    "/api/v1/nl/command",
                    axum::routing::post(nl_command::nl_command),
                )
                .route(
                    "/api/v1/nl/stream",
                    axum::routing::post(nl_command::nl_stream),
                )
                .route(
                    "/api/v1/intent/recognize",
                    axum::routing::post(nl_command::recognize_intent),
                )
                .route(
                    "/api/v1/intent/decompose",
                    axum::routing::post(nl_command::decompose_task),
                ),
        );
    }

    if surfaces.im {
        protected_routes = protected_routes.merge(
            Router::new()
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
                ),
        );
    }

    if surfaces.visualization {
        protected_routes = protected_routes.merge(
            Router::new()
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
                ),
        );
    }

    let protected_routes = protected_routes
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .layer(from_fn_with_state(
            rate_limiter,
            crate::middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(from_fn_with_state(state.clone(), idempotency_middleware));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(from_fn_with_state(state.clone(), logging_middleware))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        AppState::new(kias_common::config::KiasConfig::default()).await
    }

    async fn authenticated_state() -> AppState {
        let mut config = kias_common::config::KiasConfig::default();
        config.api_server.auth_enabled = true;
        config.api_server.auth_tokens = vec!["synthetic-operator-token".to_string()];
        AppState::new(config).await
    }

    #[tokio::test]
    async fn core_profile_exposes_basic_probe_and_capability_contract() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());

        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let capabilities = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/system/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(capabilities.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn optional_and_labs_routes_are_absent_by_default() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());

        for path in [
            "/api/v1/knowledge/search?q=test",
            "/a2a/v1/agents",
            "/api/v1/im/platforms",
            "/api/v1/nl/command",
            "/api/v1/viz/compliance-status",
            "/api/v1/ws/stats",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn sensitive_diagnostics_require_authentication() {
        let app =
            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz/deep")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn fake_runtime_config_mutation_is_not_advertised() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn cross_origin_access_is_not_wildcard_enabled() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("Origin", "https://example.invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_none());
    }
}
