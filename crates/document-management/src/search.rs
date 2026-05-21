//! 文档全文搜索引擎
//! 支持 FTS5 全文搜索 + 元数据过滤

use crate::document::*;
use serde::{Deserialize, Serialize};

/// 搜索查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub doc_type: Option<String>,
    pub status: Option<String>,
    pub created_by: Option<String>,
    pub tags: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            doc_type: None,
            status: None,
            created_by: None,
            tags: Vec::new(),
            date_from: None,
            date_to: None,
            limit: 20,
            offset: 0,
        }
    }
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub documents: Vec<Document>,
    pub total: usize,
    pub query_time_ms: u64,
}

/// 文档搜索引擎
pub struct DocumentSearchEngine {
    index_path: String,
}

impl DocumentSearchEngine {
    pub fn new(index_path: &str) -> Self {
        Self {
            index_path: index_path.to_string(),
        }
    }

    /// 构建 FTS5 查询
    pub fn build_fts_query(query: &SearchQuery) -> Option<String> {
        query.text.as_ref().map(|text| {
            let terms: Vec<String> = text
                .split_whitespace()
                .map(|t| format!("\"{}\"", t.replace('"', "")))
                .collect();
            terms.join(" OR ")
        })
    }

    /// 高亮匹配文本
    pub fn highlight_matches(text: &str, query: &str, context_chars: usize) -> Vec<String> {
        let lower_text = text.to_lowercase();
        let lower_query = query.to_lowercase();
        let mut snippets = Vec::new();

        if let Some(pos) = lower_text.find(&lower_query) {
            let start = pos.saturating_sub(context_chars);
            let end = (pos + query.len() + context_chars).min(text.len());
            let snippet = if start > 0 {
                format!("...{}...", &text[start..end])
            } else {
                format!("{}...", &text[..end])
            };
            snippets.push(snippet);
        }

        snippets
    }
}

/// 标签索引
pub struct TagIndex {
    tags: std::collections::HashMap<String, Vec<String>>, // tag -> doc_ids
}

impl TagIndex {
    pub fn new() -> Self {
        Self {
            tags: std::collections::HashMap::new(),
        }
    }

    pub fn add(&mut self, tag: &str, doc_id: &str) {
        self.tags
            .entry(tag.to_string())
            .or_default()
            .push(doc_id.to_string());
    }

    pub fn find_by_tag(&self, tag: &str) -> Vec<String> {
        self.tags.get(tag).cloned().unwrap_or_default()
    }

    pub fn all_tags(&self) -> Vec<String> {
        self.tags.keys().cloned().collect()
    }
}

impl Default for TagIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_default() {
        let q = SearchQuery::default();
        assert!(q.text.is_none());
        assert!(q.doc_type.is_none());
        assert_eq!(q.limit, 20);
        assert_eq!(q.offset, 0);
        assert!(q.tags.is_empty());
    }

    #[test]
    fn test_build_fts_query_with_text() {
        let q = SearchQuery {
            text: Some("hello world".to_string()),
            ..Default::default()
        };
        let fts = DocumentSearchEngine::build_fts_query(&q);
        assert_eq!(fts, Some("\"hello\" OR \"world\"".to_string()));
    }

    #[test]
    fn test_build_fts_query_no_text() {
        let q = SearchQuery::default();
        assert!(DocumentSearchEngine::build_fts_query(&q).is_none());
    }

    #[test]
    fn test_build_fts_query_single_term() {
        let q = SearchQuery {
            text: Some("rust".to_string()),
            ..Default::default()
        };
        let fts = DocumentSearchEngine::build_fts_query(&q);
        assert_eq!(fts, Some("\"rust\"".to_string()));
    }

    #[test]
    fn test_build_fts_query_strips_quotes() {
        let q = SearchQuery {
            text: Some("test \"quoted\"".to_string()),
            ..Default::default()
        };
        let fts = DocumentSearchEngine::build_fts_query(&q).unwrap();
        assert!(fts.contains("\"quoted\""));
        // embedded quotes should be stripped
        assert!(!fts.contains("\"\"quoted\"\""));
    }

    #[test]
    fn test_highlight_matches_found() {
        let snippets = DocumentSearchEngine::highlight_matches(
            "The quick brown fox jumps over the lazy dog",
            "fox",
            5,
        );
        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].contains("fox"));
    }

    #[test]
    fn test_highlight_matches_not_found() {
        let snippets =
            DocumentSearchEngine::highlight_matches("The quick brown fox", "elephant", 5);
        assert!(snippets.is_empty());
    }

    #[test]
    fn test_highlight_matches_case_insensitive() {
        let snippets = DocumentSearchEngine::highlight_matches("Hello World", "hello", 3);
        assert_eq!(snippets.len(), 1);
    }

    #[test]
    fn test_tag_index_new() {
        let idx = TagIndex::new();
        assert!(idx.all_tags().is_empty());
    }

    #[test]
    fn test_tag_index_default() {
        let idx = TagIndex::default();
        assert!(idx.all_tags().is_empty());
    }

    #[test]
    fn test_tag_index_add_and_find() {
        let mut idx = TagIndex::new();
        idx.add("compliance", "doc-001");
        idx.add("compliance", "doc-002");
        let docs = idx.find_by_tag("compliance");
        assert_eq!(docs.len(), 2);
        assert!(docs.contains(&"doc-001".to_string()));
    }

    #[test]
    fn test_tag_index_find_nonexistent() {
        let idx = TagIndex::new();
        assert!(idx.find_by_tag("missing").is_empty());
    }

    #[test]
    fn test_tag_index_all_tags() {
        let mut idx = TagIndex::new();
        idx.add("tag1", "d1");
        idx.add("tag2", "d2");
        let tags = idx.all_tags();
        assert_eq!(tags.len(), 2);
    }
}
