use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::models::request::ListResponse;
use crate::AppState;

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
/// Search knowledge base using vector similarity search
pub async fn search_knowledge(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Json<ListResponse<SearchResult>> {
    let limit = params.limit.unwrap_or(10).min(100);

    tracing::info!(query = %params.q, limit = limit, "Knowledge search");

    // Retrieve knowledge entries using the vector retriever
    let scored_nodes = state
        .knowledge_retriever
        .retrieve(&params.q, limit)
        .await
        .unwrap_or_default();

    let items: Vec<SearchResult> = scored_nodes
        .into_iter()
        .map(|scored| {
            // Extract a snippet from the content (first 200 chars)
            let snippet = if scored.node.content.len() > 200 {
                format!("{}...", &scored.node.content[..200])
            } else {
                scored.node.content.clone()
            };

            // Use node ID as page_id, content preview as title
            let title = scored
                .node
                .content
                .lines()
                .next()
                .unwrap_or(&scored.node.content)
                .chars()
                .take(100)
                .collect::<String>();

            SearchResult {
                page_id: scored.node.id,
                title,
                snippet,
                score: scored.score,
            }
        })
        .collect();

    let total = items.len();

    Json(ListResponse { items, total })
}
