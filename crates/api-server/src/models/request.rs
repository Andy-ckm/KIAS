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
