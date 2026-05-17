use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::request::ListResponse;
use crate::AppState;

/// Knowledge search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

/// Document ingest request
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    /// Document title
    pub title: String,
    /// Document content (markdown or plain text)
    pub content: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Source type: paper, code, doc, experience
    pub source_type: String,
    /// Optional source URL
    pub source_url: Option<String>,
}

/// Document ingest response
#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub document_id: String,
    pub chunks_created: usize,
    pub status: String,
}

/// A single search result from the knowledge subsystem
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub page_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

/// POST /api/v1/knowledge/ingest
/// Ingest a document into the knowledge base (stores chunks for future RAG)
pub async fn ingest_document(
    State(app_state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, ApiError> {
    if req.content.trim().is_empty() {
        return Err(ApiError::bad_request("Content cannot be empty"));
    }

    let doc_id = uuid::Uuid::new_v4().to_string();

    // Chunk the document into ~500 char segments
    let chunks = chunk_document(&req.content, 500);
    let chunks_count = chunks.len();

    // Store in memory
    let doc = crate::IngestedDoc {
        id: doc_id.clone(),
        title: req.title.clone(),
        content: req.content.clone(),
        tags: req.tags.clone(),
        source_type: req.source_type.clone(),
        chunks: chunks.clone(),
        ingested_at: chrono::Utc::now().to_rfc3339(),
    };
    app_state.ingested_docs.write().await.push(doc);

    // Log the ingestion
    tracing::info!(
        doc_id = %doc_id,
        title = %req.title,
        chunks = chunks_count,
        tags = ?req.tags,
        source_type = %req.source_type,
        "Document ingested into knowledge base"
    );

    Ok(Json(IngestResponse {
        document_id: doc_id,
        chunks_created: chunks_count,
        status: "ingested".to_string(),
    }))
}

/// POST /api/v1/knowledge/ingest-file
/// Ingest a local file into the knowledge base
pub async fn ingest_file(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<IngestResponse>, ApiError> {
    let path = req
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::bad_request("Missing 'path' field"))?;

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read file: {}", e)))?;

    let title = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();

    let tags = req
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let ingest_req = IngestRequest {
        title,
        content,
        tags,
        source_type: "file".to_string(),
        source_url: Some(path.to_string()),
    };

    ingest_document(State(state), Json(ingest_req)).await
}

/// Chunk a document into segments of roughly `max_chars` characters
fn chunk_document(content: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        if current.len() + line.len() > max_chars && !current.is_empty() {
            chunks.push(current.clone());
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        chunks.push(content.to_string());
    }

    chunks
}

/// GET /api/v1/knowledge/search
/// Search knowledge base using vector similarity search + ingested docs
pub async fn search_knowledge(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Json<ListResponse<SearchResult>> {
    let limit = params.limit.unwrap_or(10).min(100);
    let query = params.q.to_lowercase();

    tracing::info!(query = %params.q, limit = limit, "Knowledge search");

    let mut items: Vec<SearchResult> = Vec::new();

    // 1. Search ingested docs (keyword match)
    let docs = state.ingested_docs.read().await;
    for doc in docs.iter() {
        let mut score = 0.0;
        // Title match
        if doc.title.to_lowercase().contains(&query) {
            score += 0.5;
        }
        // Tag match
        for tag in &doc.tags {
            if query.contains(&tag.to_lowercase()) || tag.to_lowercase().contains(&query) {
                score += 0.3;
            }
        }
        // Content match (chunk level)
        for chunk in &doc.chunks {
            let chunk_lower = chunk.to_lowercase();
            let query_words: Vec<&str> = query.split_whitespace().collect();
            let matched = query_words
                .iter()
                .filter(|w| chunk_lower.contains(**w))
                .count();
            if matched > 0 {
                score += (matched as f64 / query_words.len() as f64) * 0.4;
            }
        }
        if score > 0.1 {
            let snippet = if doc.content.len() > 200 {
                format!("{}...", &doc.content[..200])
            } else {
                doc.content.clone()
            };
            items.push(SearchResult {
                page_id: doc.id.clone(),
                title: doc.title.clone(),
                snippet,
                score,
            });
        }
    }
    drop(docs);

    // 2. Also search via vector retriever
    let scored_nodes = state
        .knowledge_retriever
        .retrieve(&params.q, limit)
        .await
        .unwrap_or_default();

    for scored in scored_nodes {
        let snippet = if scored.node.content.len() > 200 {
            format!("{}...", &scored.node.content[..200])
        } else {
            scored.node.content.clone()
        };
        let title = scored
            .node
            .content
            .lines()
            .next()
            .unwrap_or(&scored.node.content)
            .chars()
            .take(100)
            .collect::<String>();
        items.push(SearchResult {
            page_id: scored.node.id,
            title,
            snippet,
            score: scored.score,
        });
    }

    // Sort by score descending
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items.truncate(limit);
    let total = items.len();

    Json(ListResponse { items, total })
}

/// GET /api/v1/knowledge/documents
/// List all ingested documents
pub async fn list_documents(State(state): State<AppState>) -> Json<serde_json::Value> {
    let docs = state.ingested_docs.read().await;
    let doc_summaries: Vec<serde_json::Value> = docs
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "title": d.title,
                "source_type": d.source_type,
                "tags": d.tags,
                "chunks": d.chunks.len(),
                "ingested_at": d.ingested_at,
            })
        })
        .collect();
    Json(serde_json::json!({
        "total": docs.len(),
        "documents": doc_summaries,
    }))
}
