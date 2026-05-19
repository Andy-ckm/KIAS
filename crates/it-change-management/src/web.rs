//! # IT变更管理 Web API
//!
//! 使用axum框架提供RESTful API

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::*;

/// Web应用状态
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<ItChangeManager>,
}

/// 创建Web路由器
pub fn create_router(manager: Arc<ItChangeManager>) -> Router {
    let state = AppState { manager };

    Router::new()
        .route("/api/v1/changes/:id", get(get_change))
        .route("/api/v1/changes", get(list_changes))
        .route("/api/v1/statistics", get(get_statistics))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 获取变更请求
async fn get_change(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ItChangeRequest>, StatusCode> {
    let manager = state.manager;
    let change = manager.get_change(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(change.clone()))
}

/// 列出变更请求
async fn list_changes(
    State(state): State<AppState>,
) -> Json<Vec<ItChangeRequest>> {
    let manager = state.manager;
    let changes = manager.list_changes();
    Json(changes.into_iter().cloned().collect())
}

/// 获取统计信息
async fn get_statistics(
    State(state): State<AppState>,
) -> Json<ChangeStatistics> {
    let manager = state.manager;
    let stats = manager.get_statistics();
    Json(stats)
}
