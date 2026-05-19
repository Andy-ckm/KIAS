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
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::*;

/// 创建变更请求
#[derive(Debug, Deserialize)]
pub struct CreateChangeApiRequest {
    pub title: String,
    pub description: String,
    pub change_type: ChangeType,
    pub change_category: ChangeCategory,
    pub risk_level: RiskLevel,
    pub requester: String,
    pub requester_department: String,
    pub rollback_plan: String,
    pub implementation_plan: String,
    pub impact_assessment: ImpactAssessment,
}

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
        .route("/api/v1/changes", get(list_changes).post(create_change))
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
async fn list_changes(State(state): State<AppState>) -> Json<Vec<ItChangeRequest>> {
    let manager = state.manager;
    let changes = manager.list_changes();
    Json(changes.into_iter().cloned().collect())
}

/// 创建变更请求
async fn create_change(
    State(_state): State<AppState>,
    Json(_request): Json<CreateChangeApiRequest>,
) -> Result<Json<ItChangeRequest>, StatusCode> {
    // 注意：ItChangeManager需要&mut self，但AppState是共享的
    // 这里需要使用内部可变性模式（Mutex或RwLock）
    // 暂时返回501 Not Implemented
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// 获取统计信息
async fn get_statistics(State(state): State<AppState>) -> Json<ChangeStatistics> {
    let manager = state.manager;
    let stats = manager.get_statistics();
    Json(stats)
}
