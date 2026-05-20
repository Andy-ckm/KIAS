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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_deserialize() {
        let json = r#"{"q":"rust programming","limit":10}"#;
        let q: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.q, "rust programming");
        assert_eq!(q.limit, Some(10));
    }

    #[test]
    fn test_search_query_without_limit() {
        let json = r#"{"q":"test"}"#;
        let q: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.q, "test");
        assert!(q.limit.is_none());
    }

    #[test]
    fn test_ingest_request_deserialize() {
        let json =
            r#"{"title":"My Doc","content":"Hello world","tags":["tag1"],"source_type":"paper"}"#;
        let req: IngestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "My Doc");
        assert_eq!(req.content, "Hello world");
        assert_eq!(req.tags, vec!["tag1"]);
        assert_eq!(req.source_type, "paper");
        assert!(req.source_url.is_none());
    }

    #[test]
    fn test_ingest_request_with_source_url() {
        let json = r#"{"title":"Paper","content":"Abstract...","tags":[],"source_type":"paper","source_url":"https://arxiv.org/123"}"#;
        let req: IngestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_url, Some("https://arxiv.org/123".to_string()));
    }

    #[test]
    fn test_ingest_response_serialize() {
        let resp = IngestResponse {
            document_id: "doc-1".to_string(),
            chunks_created: 5,
            status: "ok".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("document_id"));
        assert!(json.contains("doc-1"));
        assert!(json.contains("chunks_created"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_search_result_serialize() {
        let result = SearchResult {
            page_id: "p1".to_string(),
            title: "Test".to_string(),
            snippet: "A snippet".to_string(),
            score: 0.95,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("score"));
        assert!(json.contains("0.95"));
    }

    #[test]
    fn test_search_result_deserialize() {
        let json = r#"{"page_id":"p1","title":"T","snippet":"S","score":0.8}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.page_id, "p1");
        assert_eq!(r.score, 0.8);
    }

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        AppState::new_async(config).await
    }

    #[tokio::test]
    async fn test_ingest_document_success() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "Rust Guide".to_string(),
            content: "Rust is a systems programming language focused on safety and performance."
                .to_string(),
            tags: vec!["rust".to_string(), "programming".to_string()],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let result = ingest_document(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert!(!result.document_id.is_empty());
        assert!(result.chunks_created > 0);
        assert_eq!(result.status, "ingested");
    }

    #[tokio::test]
    async fn test_ingest_document_empty_content_fails() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "Empty".to_string(),
            content: "".to_string(),
            tags: vec![],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let result = ingest_document(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ingest_document_whitespace_content_fails() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "Whitespace".to_string(),
            content: "   \n  ".to_string(),
            tags: vec![],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let result = ingest_document(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ingest_long_document_chunks() {
        let state = test_state().await;
        // Create content with many lines to trigger chunking (>500 chars)
        let long_content = (0..50)
            .map(|i| format!("Line {} about Rust programming and systems.", i))
            .collect::<Vec<_>>()
            .join("\n");
        let req = IngestRequest {
            title: "Long Doc".to_string(),
            content: long_content,
            tags: vec![],
            source_type: "paper".to_string(),
            source_url: None,
        };
        let result = ingest_document(State(state), Json(req)).await.unwrap();
        assert!(
            result.chunks_created >= 2,
            "Long doc should produce multiple chunks, got {}",
            result.chunks_created
        );
    }

    #[tokio::test]
    async fn test_list_documents_empty() {
        let state = test_state().await;
        let result = list_documents(State(state)).await;
        assert_eq!(result["total"], 0);
    }

    #[tokio::test]
    async fn test_list_documents_after_ingest() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "Test Doc".to_string(),
            content: "Some content for testing".to_string(),
            tags: vec!["test".to_string()],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let _ = ingest_document(State(state.clone()), Json(req))
            .await
            .unwrap();

        let result = list_documents(State(state)).await;
        assert_eq!(result["total"], 1);
    }

    #[tokio::test]
    async fn test_search_knowledge_empty() {
        let state = test_state().await;
        let result = search_knowledge(
            State(state),
            Query(SearchQuery {
                q: "nonexistent".to_string(),
                limit: Some(10),
            }),
        )
        .await;
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn test_search_knowledge_after_ingest() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "Rust Ownership".to_string(),
            content: "Rust uses ownership to manage memory without garbage collection.".to_string(),
            tags: vec!["rust".to_string()],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let _ = ingest_document(State(state.clone()), Json(req))
            .await
            .unwrap();

        let result = search_knowledge(
            State(state),
            Query(SearchQuery {
                q: "rust ownership".to_string(),
                limit: Some(10),
            }),
        )
        .await;
        assert!(result.total > 0);
        assert!(result.items[0].score > 0.0);
    }

    #[tokio::test]
    async fn test_search_knowledge_limit() {
        let state = test_state().await;
        // Ingest multiple docs
        for i in 0..5 {
            let req = IngestRequest {
                title: format!("Doc {}", i),
                content: format!("Content about topic {}", i),
                tags: vec![],
                source_type: "doc".to_string(),
                source_url: None,
            };
            let _ = ingest_document(State(state.clone()), Json(req))
                .await
                .unwrap();
        }

        let result = search_knowledge(
            State(state),
            Query(SearchQuery {
                q: "content topic".to_string(),
                limit: Some(2),
            }),
        )
        .await;
        assert!(result.total <= 2);
    }

    #[tokio::test]
    async fn test_search_knowledge_title_match() {
        let state = test_state().await;
        let req = IngestRequest {
            title: "UniqueTitle12345".to_string(),
            content: "generic content".to_string(),
            tags: vec![],
            source_type: "doc".to_string(),
            source_url: None,
        };
        let _ = ingest_document(State(state.clone()), Json(req))
            .await
            .unwrap();

        let result = search_knowledge(
            State(state),
            Query(SearchQuery {
                q: "UniqueTitle12345".to_string(),
                limit: Some(10),
            }),
        )
        .await;
        assert!(result.total > 0);
    }

    #[test]
    fn test_chunk_document_short() {
        let chunks = chunk_document("hello world", 500);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].trim(), "hello world");
    }

    #[test]
    fn test_chunk_document_empty() {
        let chunks = chunk_document("", 500);
        assert_eq!(chunks.len(), 1);
    }
}
