use axum::extract::State;
use axum::Json;

use crate::models::request::{ComponentHealth, HealthResponse};
use crate::AppState;

/// GET /health
/// Liveness probe — always returns 200 if the server is up
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok"
    }))
}

/// GET /readyz
/// Readiness probe — checks that internal state is usable
pub async fn readiness(State(state): State<AppState>) -> Json<HealthResponse> {
    let mut components = vec![];

    // Check agents store
    let agents_healthy = {
        let _lock = state.agents.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "agents_store".to_string(),
        status: if agents_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
    });

    // Check nodes store
    let nodes_healthy = {
        let _lock = state.nodes.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "nodes_store".to_string(),
        status: if nodes_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
    });

    let overall = if agents_healthy && nodes_healthy {
        "healthy"
    } else {
        "degraded"
    };

    Json(HealthResponse {
        status: overall.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        components,
    })
}
