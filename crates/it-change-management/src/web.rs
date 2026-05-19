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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    fn create_test_manager() -> Arc<ItChangeManager> {
        let mut manager = ItChangeManager::new();
        let impact = ImpactAssessment {
            affected_systems: vec!["ERP".to_string()],
            affected_users: vec!["all".to_string()],
            downtime_estimate_minutes: 30,
            risk_mitigation: vec!["rollback".to_string()],
            testing_requirements: vec!["smoke test".to_string()],
            gxp_impact: GxpImpact::Direct,
            requires_csv_validation: true,
            affects_data_integrity: false,
        };
        manager.create_change_request(
            "Upgrade ERP".to_string(),
            "Upgrade ERP to v2.0".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::High,
            "alice".to_string(),
            "IT".to_string(),
            "Rollback plan".to_string(),
            "Implementation plan".to_string(),
            impact,
        );
        Arc::new(manager)
    }

    #[tokio::test]
    async fn test_router_creation() {
        let manager = create_test_manager();
        let _router = create_router(manager);
    }

    #[tokio::test]
    async fn test_get_change_valid_id() {
        let manager = create_test_manager();
        let changes = manager.list_changes();
        let id = changes[0].id.clone();
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/changes/{}", id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let change: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(change["title"], "Upgrade ERP");
    }

    #[tokio::test]
    async fn test_get_change_not_found() {
        let manager = create_test_manager();
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/changes/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_changes_empty() {
        let manager = Arc::new(ItChangeManager::new());
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/changes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let changes: Vec<Value> = serde_json::from_slice(&body).unwrap();
        assert!(changes.is_empty());
    }

    #[tokio::test]
    async fn test_list_changes_with_data() {
        let manager = create_test_manager();
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/changes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let changes: Vec<Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["title"], "Upgrade ERP");
    }

    #[tokio::test]
    async fn test_create_change_returns_501() {
        let manager = create_test_manager();
        let app = create_router(manager);

        let request_body = serde_json::json!({
            "title": "New Change",
            "description": "Test",
            "change_type": "Infrastructure",
            "change_category": "Normal",
            "risk_level": "Low",
            "requester": "bob",
            "requester_department": "IT",
            "rollback_plan": "rollback",
            "implementation_plan": "implement",
            "impact_assessment": {
                "affected_systems": [],
                "affected_users": [],
                "downtime_estimate_minutes": 0,
                "risk_mitigation": [],
                "testing_requirements": [],
                "gxp_impact": "None",
                "requires_csv_validation": false,
                "affects_data_integrity": false
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/changes")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn test_get_statistics_empty() {
        let manager = Arc::new(ItChangeManager::new());
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/statistics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(stats["total"], 0);
    }

    #[tokio::test]
    async fn test_get_statistics_with_data() {
        let manager = create_test_manager();
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/statistics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(stats["total"], 1);
        assert_eq!(stats["draft"], 1);
    }

    #[tokio::test]
    async fn test_nonexistent_route() {
        let manager = create_test_manager();
        let app = create_router(manager);

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
    async fn test_get_change_returns_json() {
        let manager = create_test_manager();
        let changes = manager.list_changes();
        let id = changes[0].id.clone();
        let app = create_router(manager);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/changes/{}", id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("application/json"));
    }
}
