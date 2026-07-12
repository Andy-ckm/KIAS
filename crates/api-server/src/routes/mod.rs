pub mod product;

use axum::middleware::{from_fn, from_fn_with_state};
use axum::Router;

use crate::handlers::runs;
use crate::middleware::rate_limit::{RateLimiter, RateLimiterConfig};
use crate::middleware::{
    auth::auth_middleware, authorization::control_plane_authorization,
    idempotency::idempotency_middleware, logging::logging_middleware,
};
use crate::surfaces::SurfaceConfig;
use crate::AppState;

fn run_routes(state: AppState) -> Router {
    let rate_limiter = RateLimiter::new(RateLimiterConfig {
        requests_per_second: 10.0,
        burst_size: 20.0,
    });

    Router::new()
        .route(
            "/api/v1/agents/:id/runs",
            axum::routing::post(runs::create_run),
        )
        .route("/api/v1/runs", axum::routing::get(runs::list_runs))
        .route("/api/v1/runs/:id", axum::routing::get(runs::get_run))
        .route(
            "/api/v1/runs/:id/logs",
            axum::routing::get(runs::get_run_logs),
        )
        .route(
            "/api/v1/runs/:id/evidence",
            axum::routing::get(runs::get_run_evidence),
        )
        .route(
            "/api/v1/runs/:id/checkpoint",
            axum::routing::get(runs::get_run_checkpoint),
        )
        .route(
            "/api/v1/runs/:id/cancel",
            axum::routing::post(runs::cancel_run),
        )
        .route(
            "/api/v1/runs/:id/retry",
            axum::routing::post(runs::retry_run),
        )
        .route(
            "/api/v1/runs/:id/recover",
            axum::routing::post(runs::recover_run),
        )
        .layer(from_fn_with_state(state.clone(), idempotency_middleware))
        .layer(from_fn_with_state(
            rate_limiter,
            crate::middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(from_fn(control_plane_authorization))
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .layer(from_fn_with_state(state.clone(), logging_middleware))
        .with_state(state)
}

pub fn create_router(state: AppState) -> Router {
    let core = product::create_router(state.clone());
    core.merge(run_routes(state))
}

pub fn create_router_with_surfaces(state: AppState, surfaces: SurfaceConfig) -> Router {
    let core = product::create_router_with_surfaces(state.clone(), surfaces);
    core.merge(run_routes(state))
}

/// Pre-1.0 source compatibility for callers using `routes::api::create_router`.
pub mod api {
    pub use super::{create_router, create_router_with_surfaces};
}
