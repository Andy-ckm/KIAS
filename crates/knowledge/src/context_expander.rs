//! Context Expander — 扩展匹配 chunk 的上下文窗口
//!
//! 向量搜索返回单个 chunk，但上下文往往分布在相邻 chunks 中。
//! ContextExpander 将匹配 chunk 的前后 N 个 chunks 也纳入结果，
//! 提供更完整的上下文。
//!
//! 参考: GraphRAG Context Window Expansion

use serde::{Deserialize, Serialize};

/// 扩展后的上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedContext {
    /// 匹配的核心 chunk
    pub core_chunk: Chunk,
    /// 扩展的上下文 chunks（按位置排序）
    pub context_chunks: Vec<Chunk>,
    /// 完整文本（合并后）
    pub full_text: String,
}

/// Chunk 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub text: String,
    pub index: usize, // 在文档中的位置
    pub score: Option<f64>,
}

/// 上下文扩展器
pub struct ContextExpander {
    /// 默认扩展窗口大小（前后各 N 个 chunks）
    default_window: usize,
}

impl ContextExpander {
    pub fn new() -> Self {
        Self { default_window: 2 }
    }

    pub fn with_window(window: usize) -> Self {
        Self {
            default_window: window,
        }
    }

    /// 扩展单个 chunk 的上下文
    pub fn expand(
        &self,
        core_chunk: &Chunk,
        all_chunks: &[Chunk],
        window: Option<usize>,
    ) -> ExpandedContext {
        let w = window.unwrap_or(self.default_window);

        // 找到同文档的所有 chunks，按 index 排序
        let mut doc_chunks: Vec<&Chunk> = all_chunks
            .iter()
            .filter(|c| c.doc_id == core_chunk.doc_id)
            .collect();
        doc_chunks.sort_by_key(|c| c.index);

        // 找到 core_chunk 在排序后列表中的位置
        let core_pos = doc_chunks
            .iter()
            .position(|c| c.id == core_chunk.id)
            .unwrap_or(0);

        // 扩展窗口
        let start = core_pos.saturating_sub(w);
        let end = (core_pos + w + 1).min(doc_chunks.len());

        let context_chunks: Vec<Chunk> = doc_chunks[start..end]
            .iter()
            .filter(|c| c.id != core_chunk.id)
            .map(|&c| c.clone())
            .collect();

        // 合并文本
        let mut full_parts: Vec<String> = Vec::new();
        for chunk in &doc_chunks[start..end] {
            if chunk.id == core_chunk.id {
                full_parts.push(format!("[MATCH] {} [/MATCH]", chunk.text));
            } else {
                full_parts.push(chunk.text.clone());
            }
        }

        ExpandedContext {
            core_chunk: core_chunk.clone(),
            context_chunks,
            full_text: full_parts.join("\n"),
        }
    }

    /// 批量扩展多个 chunks
    pub fn expand_batch(
        &self,
        core_chunks: &[Chunk],
        all_chunks: &[Chunk],
        window: Option<usize>,
    ) -> Vec<ExpandedContext> {
        core_chunks
            .iter()
            .map(|c| self.expand(c, all_chunks, window))
            .collect()
    }
}

impl Default for ContextExpander {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunks(doc_id: &str, count: usize) -> Vec<Chunk> {
        (0..count)
            .map(|i| Chunk {
                id: format!("{}_{}", doc_id, i),
                doc_id: doc_id.to_string(),
                text: format!("chunk {} of {}", i, doc_id),
                index: i,
                score: None,
            })
            .collect()
    }

    #[test]
    fn test_expand_middle_chunk() {
        let expander = ContextExpander::new();
        let chunks = make_chunks("doc1", 10);
        let core = &chunks[5];

        let result = expander.expand(core, &chunks, Some(2));

        // 应包含 chunks 3,4,6,7 (前后各2，排除 core)
        assert_eq!(result.context_chunks.len(), 4);
        assert!(result.full_text.contains("[MATCH]"));
        assert!(result.full_text.contains("[/MATCH]"));
    }

    #[test]
    fn test_expand_first_chunk() {
        let expander = ContextExpander::new();
        let chunks = make_chunks("doc1", 10);
        let core = &chunks[0];

        let result = expander.expand(core, &chunks, Some(2));

        // 前面没有 chunk，只有后面的
        assert_eq!(result.context_chunks.len(), 2);
    }

    #[test]
    fn test_expand_last_chunk() {
        let expander = ContextExpander::new();
        let chunks = make_chunks("doc1", 10);
        let core = &chunks[9];

        let result = expander.expand(core, &chunks, Some(2));

        // 后面没有 chunk，只有前面的
        assert_eq!(result.context_chunks.len(), 2);
    }

    #[test]
    fn test_expand_different_doc() {
        let expander = ContextExpander::new();
        let mut chunks = make_chunks("doc1", 5);
        chunks.extend(make_chunks("doc2", 5));

        let core = &chunks[7]; // doc2 的 chunk 2
        let result = expander.expand(core, &chunks, Some(2));

        // 只应包含 doc2 的 chunks
        for c in &result.context_chunks {
            assert_eq!(c.doc_id, "doc2");
        }
    }

    #[test]
    fn test_expand_default_window() {
        let expander = ContextExpander::new();
        assert_eq!(expander.default_window, 2);

        let expander = ContextExpander::with_window(5);
        assert_eq!(expander.default_window, 5);
    }

    #[test]
    fn test_expand_batch() {
        let expander = ContextExpander::new();
        let chunks = make_chunks("doc1", 10);
        let cores = vec![chunks[2].clone(), chunks[7].clone()];

        let results = expander.expand_batch(&cores, &chunks, Some(1));
        assert_eq!(results.len(), 2);
    }
}
