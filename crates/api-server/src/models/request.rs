use serde::{Deserialize, Serialize};

/// Standard envelope for list responses
#[derive(Debug, Serialize)]
pub struct ListResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: usize,
}

/// Standard envelope for detail responses
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

/// Standard envelope for action responses (create, delete, etc.)
#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub message: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub components: Vec<ComponentHealth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: String,
}

/// Pagination query parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PaginationParams {
    pub fn offset(&self) -> usize {
        let page = self.page.unwrap_or(1).max(1);
        let per_page = self.per_page.unwrap_or(20);
        ((page - 1) * per_page) as usize
    }

    pub fn limit(&self) -> usize {
        self.per_page.unwrap_or(20).min(100) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let resp = HealthResponse {
            status: "ok".to_string(),
            version: "0.1.0".to_string(),
            components: vec![
                ComponentHealth { name: "api".to_string(), status: "ok".to_string() },
                ComponentHealth { name: "db".to_string(), status: "ok".to_string() },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(""status":"ok""));
        assert!(json.contains(""version":"0.1.0""));
        assert!(json.contains(""components":["));
    }

    #[test]
    fn test_health_response_deserialization() {
        let json = r#"{"status":"ok","version":"1.0","components":[{"name":"api","status":"ok"}]}"#;
        let resp: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.version, "1.0");
        assert_eq!(resp.components.len(), 1);
        assert_eq!(resp.components[0].name, "api");
    }

    #[test]
    fn test_pagination_defaults() {
        let p = PaginationParams { page: None, per_page: None };
        assert_eq!(p.offset(), 0);
        assert_eq!(p.limit(), 20);
    }

    #[test]
    fn test_pagination_page_1() {
        let p = PaginationParams { page: Some(1), per_page: Some(10) };
        assert_eq!(p.offset(), 0);
        assert_eq!(p.limit(), 10);
    }

    #[test]
    fn test_pagination_page_3() {
        let p = PaginationParams { page: Some(3), per_page: Some(15) };
        assert_eq!(p.offset(), 30);
        assert_eq!(p.limit(), 15);
    }

    #[test]
    fn test_pagination_page_0_clamps_to_1() {
        let p = PaginationParams { page: Some(0), per_page: Some(10) };
        assert_eq!(p.offset(), 0);
    }

    #[test]
    fn test_pagination_limit_capped_at_100() {
        let p = PaginationParams { page: Some(1), per_page: Some(500) };
        assert_eq!(p.limit(), 100);
    }

    #[test]
    fn test_list_response_serialization() {
        let resp = ListResponse { items: vec!["a", "b"], total: 2 };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(""items":["a","b"]"));
        assert!(json.contains(""total":2"));
    }

    #[test]
    fn test_api_response_serialization() {
        let resp = ApiResponse { data: "hello" };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(""data":"hello""));
    }

    #[test]
    fn test_action_response_serialization() {
        let resp = ActionResponse { message: "created".to_string() };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(""message":"created""));
    }
}
