use axum::extract::Query;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::models::request::ListResponse;

/// Knowledge search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

/// A single search result from the knowledge subsystem
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub page_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

/// GET /api/v1/knowledge/search
/// Search knowledge base (stub — wired for future knowledge crate)
pub async fn search_knowledge(
    Query(params): Query<SearchQuery>,
) -> Json<ListResponse<SearchResult>> {
    let limit = params.limit.unwrap_or(10).min(100);

    tracing::info!(query = %params.q, limit = limit, "Knowledge search");

    // Placeholder: return empty results until knowledge crate is integrated
    Json(ListResponse {
        items: vec![],
        total: 0,
    })
}
