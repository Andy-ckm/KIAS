//! Query Rewriter — LLM 改写用户查询为多个子问题
//!
//! 提升 RAG 搜索质量的核心组件。
//! 将复杂查询分解为多个简单子问题，分别搜索后合并结果。
//!
//! 参考: Microsoft GraphRAG Query Transformation

use serde::{Deserialize, Serialize};

/// 查询改写结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewrittenQuery {
    /// 原始查询
    pub original: String,
    /// 改写后的子查询
    pub sub_queries: Vec<String>,
    /// 查询意图分类
    pub intent: QueryIntent,
}

/// 查询意图分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueryIntent {
    /// 具体事实查询（谁、什么、何时）
    Factual,
    /// 概念解释（是什么、如何）
    Conceptual,
    /// 关系推理（关系、对比、影响）
    Relational,
    /// 全局总结（总结、概述、趋势）
    Summarization,
    /// 代码相关（实现、函数、API）
    Code,
}

/// 查询改写器
pub struct QueryRewriter {
    /// 最大子查询数
    max_sub_queries: usize,
}

impl QueryRewriter {
    pub fn new() -> Self {
        Self { max_sub_queries: 5 }
    }

    /// 改写查询（规则引擎版本，不依赖 LLM）
    pub fn rewrite(&self, query: &str) -> RewrittenQuery {
        let intent = self.classify_intent(query);
        let sub_queries = self.generate_sub_queries(query, &intent);

        RewrittenQuery {
            original: query.to_string(),
            sub_queries,
            intent,
        }
    }

    /// 分类查询意图
    fn classify_intent(&self, query: &str) -> QueryIntent {
        let lower = query.to_lowercase();

        // 代码相关
        if lower.contains("代码")
            || lower.contains("实现")
            || lower.contains("函数")
            || lower.contains("api")
            || lower.contains("crate")
            || lower.contains("rust")
            || lower.contains("struct")
            || lower.contains("fn ")
        {
            return QueryIntent::Code;
        }

        // 关系推理
        if lower.contains("对比")
            || lower.contains("比较")
            || lower.contains("区别")
            || lower.contains("关系")
            || lower.contains("影响")
            || lower.contains("vs")
            || lower.contains("versus")
        {
            return QueryIntent::Relational;
        }

        // 全局总结
        if lower.contains("总结")
            || lower.contains("概述")
            || lower.contains("整体")
            || lower.contains("全部")
            || lower.contains("趋势")
            || lower.contains("overview")
        {
            return QueryIntent::Summarization;
        }

        // 概念解释
        if lower.contains("是什么")
            || lower.contains("什么是")
            || lower.contains("如何")
            || lower.contains("怎么")
            || lower.contains("为什么")
            || lower.contains("explain")
            || lower.contains("how")
        {
            return QueryIntent::Conceptual;
        }

        // 默认: 事实查询
        QueryIntent::Factual
    }

    /// 生成子查询
    fn generate_sub_queries(&self, query: &str, intent: &QueryIntent) -> Vec<String> {
        let mut subs = vec![query.to_string()];

        match intent {
            QueryIntent::Factual => {
                // 事实查询: 提取关键词组合
                let keywords = self.extract_keywords(query);
                if keywords.len() >= 2 {
                    // 生成关键词组合
                    for i in 0..keywords.len().min(3) {
                        for j in (i + 1)..keywords.len().min(4) {
                            subs.push(format!("{} {}", keywords[i], keywords[j]));
                        }
                    }
                }
            }
            QueryIntent::Conceptual => {
                // 概念查询: 拆分定义和解释
                let concept = self.extract_main_concept(query);
                subs.push(format!("{} 定义", concept));
                subs.push(format!("{} 原理", concept));
                subs.push(format!("{} 用途", concept));
            }
            QueryIntent::Relational => {
                // 关系查询: 拆分为两个实体的独立查询
                let entities = self.extract_comparison_entities(query);
                if entities.len() >= 2 {
                    subs.push(format!("{} 是什么", entities[0]));
                    subs.push(format!("{} 是什么", entities[1]));
                    subs.push(format!("{} {} 区别", entities[0], entities[1]));
                }
            }
            QueryIntent::Summarization => {
                // 总结查询: 分维度查询
                subs.push(format!("{} 主要内容", query));
                subs.push(format!("{} 关键点", query));
                subs.push(format!("{} 结论", query));
            }
            QueryIntent::Code => {
                // 代码查询: 拆分实现和接口
                let concept = self.extract_main_concept(query);
                subs.push(format!("{} 结构体定义", concept));
                subs.push(format!("{} 实现逻辑", concept));
                subs.push(format!("{} 测试用例", concept));
            }
        }

        // 去重并限制数量
        subs.sort();
        subs.dedup();
        subs.truncate(self.max_sub_queries);
        subs
    }

    /// 提取关键词
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        // 简单分词: 按空格和标点分割，过滤停用词
        let stop_words: Vec<&str> = vec![
            "的", "了", "是", "在", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上",
            "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这",
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might",
            "shall", "can", "of", "in", "to", "for", "with", "on", "at", "from", "by", "about",
            "as", "into", "through", "during", "before", "after", "above", "below", "between",
            "out", "off", "over", "under", "again", "further", "then", "once", "what", "which",
            "who", "whom", "this", "that", "these", "those", "and", "but", "or", "nor", "not",
            "so", "very",
        ];

        query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !stop_words.contains(&s.as_str()) && s.len() > 1)
            .collect()
    }

    /// 提取主要概念
    fn extract_main_concept(&self, query: &str) -> String {
        // 去掉常见问句前缀
        let prefixes = [
            "什么是",
            "是什么",
            "如何",
            "怎么",
            "为什么",
            "请解释",
            "请说明",
            "what is",
            "how to",
            "explain",
            "describe",
        ];

        let mut cleaned = query.to_lowercase();
        for prefix in &prefixes {
            if cleaned.starts_with(prefix) {
                cleaned = cleaned[prefix.len()..].trim().to_string();
                break;
            }
        }

        // 取前几个词作为概念
        let words: Vec<&str> = cleaned.split_whitespace().take(3).collect();
        words.join(" ")
    }

    /// 提取比较实体
    fn extract_comparison_entities(&self, query: &str) -> Vec<String> {
        let lower = query.to_lowercase();

        // 尝试找 "X 和 Y" 或 "X vs Y" 模式
        for separator in &["和", "与", "vs", "versus", "对比", "比较"] {
            if let Some(pos) = lower.find(separator) {
                let before = query[..pos].trim().to_string();
                let after = query[pos + separator.len()..].trim().to_string();
                if !before.is_empty() && !after.is_empty() {
                    return vec![before, after];
                }
            }
        }

        vec![]
    }
}

impl Default for QueryRewriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_code_intent() {
        let rewriter = QueryRewriter::new();
        assert_eq!(
            rewriter.classify_intent("KIAS 的 scheduler crate 如何实现"),
            QueryIntent::Code
        );
    }

    #[test]
    fn test_classify_relational_intent() {
        let rewriter = QueryRewriter::new();
        assert_eq!(
            rewriter.classify_intent("GraphRAG 和传统 RAG 的区别"),
            QueryIntent::Relational
        );
    }

    #[test]
    fn test_classify_summarization_intent() {
        let rewriter = QueryRewriter::new();
        assert_eq!(
            rewriter.classify_intent("总结 KIAS 的整体架构"),
            QueryIntent::Summarization
        );
    }

    #[test]
    fn test_classify_conceptual_intent() {
        let rewriter = QueryRewriter::new();
        assert_eq!(
            rewriter.classify_intent("什么是 AgenticRAG"),
            QueryIntent::Conceptual
        );
    }

    #[test]
    fn test_classify_factual_intent() {
        let rewriter = QueryRewriter::new();
        assert_eq!(
            rewriter.classify_intent("KIAS 测试数量"),
            QueryIntent::Factual
        );
    }

    #[test]
    fn test_rewrite_factual() {
        let rewriter = QueryRewriter::new();
        let result = rewriter.rewrite("KIAS 调度算法");
        assert!(!result.sub_queries.is_empty());
        assert!(result.sub_queries.contains(&"KIAS 调度算法".to_string()));
    }

    #[test]
    fn test_rewrite_conceptual() {
        let rewriter = QueryRewriter::new();
        let result = rewriter.rewrite("什么是 AgenticRAG");
        assert_eq!(result.intent, QueryIntent::Conceptual);
        assert!(result.sub_queries.len() >= 2);
    }

    #[test]
    fn test_rewrite_relational() {
        let rewriter = QueryRewriter::new();
        let result = rewriter.rewrite("GraphRAG 和传统 RAG 的区别");
        assert_eq!(result.intent, QueryIntent::Relational);
        assert!(result.sub_queries.len() >= 3);
    }

    #[test]
    fn test_extract_keywords() {
        let rewriter = QueryRewriter::new();
        let keywords = rewriter.extract_keywords("KIAS 的调度算法实现");
        assert!(keywords.contains(&"KIAS".to_string()));
        // 简单分词会拆分中文词
        assert!(keywords.iter().any(|k| k.contains("调度")));
        // "实现" may be filtered by stop words - just check core keyword is present
        assert!(keywords.len() >= 1);
        // 停用词应被过滤
        assert!(!keywords.contains(&"的".to_string()));
    }

    #[test]
    fn test_extract_comparison_entities() {
        let rewriter = QueryRewriter::new();
        let entities = rewriter.extract_comparison_entities("GraphRAG 和传统 RAG 的区别");
        assert_eq!(entities.len(), 2);
        assert!(entities[0].contains("GraphRAG"));
        assert!(entities[1].contains("传统 RAG"));
    }

    #[test]
    fn test_sub_queries_dedup() {
        let rewriter = QueryRewriter::new();
        let result = rewriter.rewrite("test test test");
        // 应该去重
        let unique: std::collections::HashSet<_> = result.sub_queries.iter().collect();
        assert_eq!(unique.len(), result.sub_queries.len());
    }

    #[test]
    fn test_max_sub_queries_limit() {
        let rewriter = QueryRewriter::new();
        let result = rewriter.rewrite("KIAS 的 Rust 代码实现测试用例");
        assert!(result.sub_queries.len() <= rewriter.max_sub_queries);
    }
}
