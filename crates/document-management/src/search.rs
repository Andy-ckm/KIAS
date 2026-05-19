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
#[allow(dead_code)]
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
