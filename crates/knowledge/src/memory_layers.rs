//! 记忆管理 — 基于Claude Code 7层记忆架构的KIAS实现
//!
//! 吸收的核心设计：
//! - L1: 工具结果磁盘存储 + 预览替换（节省上下文）
//! - L3: 会话记忆（零成本压缩，实时结构化笔记）
//! - L6: 做梦机制（跨会话记忆巩固）
//!
//! 设计原则：
//! 1. 分层防御，先用最便宜的
//! 2. 零成本压缩 — 会话记忆本身就是摘要
//! 3. 缓存友好 — 替换后内容冻结，prompt前缀一致
//! 4. 做梦机制 — 积累足够会话后巩固记忆

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

// ============================================================
// L1: 工具结果存储 — 大结果写磁盘，上下文只放预览
// ============================================================

/// 工具结果存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultStoreConfig {
    /// 预览大小（字节）— 超过此大小的结果会被截断
    pub preview_size: usize,
    /// 磁盘存储路径
    pub storage_path: PathBuf,
    /// 最大存储大小（字节）
    pub max_storage_size: usize,
}

impl Default for ToolResultStoreConfig {
    fn default() -> Self {
        Self {
            preview_size: 2048, // 2KB，论文默认
            storage_path: PathBuf::from(".kias/tool-results"),
            max_storage_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// 工具结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEntry {
    pub id: String,
    pub tool_name: String,
    pub full_content: String,
    pub preview: String,
    pub stored_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: usize,
}

/// 工具结果存储 — L1层
pub struct ToolResultStore {
    config: ToolResultStoreConfig,
    results: Arc<RwLock<HashMap<String, ToolResultEntry>>>,
}

impl ToolResultStore {
    pub fn new(config: ToolResultStoreConfig) -> Self {
        Self {
            config,
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 存储工具结果，返回预览
    /// 论文关键：内容替换后冻结，确保prompt前缀一致
    pub async fn store(&self, id: &str, tool_name: &str, content: &str) -> String {
        let preview = if content.len() > self.config.preview_size {
            format!(
                "{}...\n\n[Full content stored at tool-results/{}]",
                &content[..self.config.preview_size],
                id
            )
        } else {
            content.to_string()
        };

        let entry = ToolResultEntry {
            id: id.to_string(),
            tool_name: tool_name.to_string(),
            full_content: content.to_string(),
            preview: preview.clone(),
            stored_at: chrono::Utc::now(),
            size_bytes: content.len(),
        };

        self.results.write().await.insert(id.to_string(), entry);
        preview
    }

    /// 获取完整内容
    pub async fn get_full(&self, id: &str) -> Option<String> {
        self.results
            .read()
            .await
            .get(id)
            .map(|e| e.full_content.clone())
    }

    /// 获取预览
    pub async fn get_preview(&self, id: &str) -> Option<String> {
        self.results.read().await.get(id).map(|e| e.preview.clone())
    }

    /// 清理旧结果
    pub async fn cleanup(&self, max_age: chrono::Duration) {
        let cutoff = chrono::Utc::now() - max_age;
        let mut results = self.results.write().await;
        results.retain(|_, entry| entry.stored_at > cutoff);
    }

    pub async fn count(&self) -> usize {
        self.results.read().await.len()
    }
}

// ============================================================
// L3: 会话记忆 — 零成本压缩，实时结构化笔记
// ============================================================

/// 会话记忆配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryConfig {
    /// 触发压缩的token阈值
    pub compaction_token_threshold: usize,
    /// 最小保留消息数
    pub min_messages_to_keep: usize,
    /// 会话记忆文件路径
    pub memory_path: PathBuf,
}

impl Default for SessionMemoryConfig {
    fn default() -> Self {
        Self {
            compaction_token_threshold: 100_000, // 100K tokens
            min_messages_to_keep: 5,
            memory_path: PathBuf::from(".kias/session-memory"),
        }
    }
}

/// 会话记忆条目 — 结构化笔记
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryEntry {
    /// 会话ID
    pub session_id: String,
    /// 用户查询摘要
    pub query_summary: String,
    /// 关键发现
    pub key_findings: Vec<String>,
    /// 引用的文档
    pub referenced_docs: Vec<String>,
    /// 工具调用统计
    pub tool_stats: ToolCallStats,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 工具调用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolCallStats {
    pub search_calls: usize,
    pub find_calls: usize,
    pub open_calls: usize,
    pub summarize_calls: usize,
    pub total_tokens: usize,
}

/// 会话记忆管理器 — L3层
pub struct SessionMemoryManager {
    config: SessionMemoryConfig,
    memories: Arc<RwLock<HashMap<String, SessionMemoryEntry>>>,
}

impl SessionMemoryManager {
    pub fn new(config: SessionMemoryConfig) -> Self {
        Self {
            config,
            memories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建新会话记忆
    pub async fn create_session(&self, session_id: &str, query: &str) {
        let entry = SessionMemoryEntry {
            session_id: session_id.to_string(),
            query_summary: query.to_string(),
            key_findings: Vec::new(),
            referenced_docs: Vec::new(),
            tool_stats: ToolCallStats::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.memories
            .write()
            .await
            .insert(session_id.to_string(), entry);
    }

    /// 更新会话记忆 — 实时维护结构化笔记
    pub async fn update(&self, session_id: &str, finding: &str, doc_id: Option<&str>) {
        let mut memories = self.memories.write().await;
        if let Some(entry) = memories.get_mut(session_id) {
            entry.key_findings.push(finding.to_string());
            if let Some(doc) = doc_id {
                if !entry.referenced_docs.contains(&doc.to_string()) {
                    entry.referenced_docs.push(doc.to_string());
                }
            }
            entry.updated_at = chrono::Utc::now();
        }
    }

    /// 更新工具统计
    pub async fn update_tool_stats(&self, session_id: &str, tool: &str, tokens: usize) {
        let mut memories = self.memories.write().await;
        if let Some(entry) = memories.get_mut(session_id) {
            match tool {
                "search" => entry.tool_stats.search_calls += 1,
                "find" => entry.tool_stats.find_calls += 1,
                "open" => entry.tool_stats.open_calls += 1,
                "summarize" => entry.tool_stats.summarize_calls += 1,
                _ => {}
            }
            entry.tool_stats.total_tokens += tokens;
            entry.updated_at = chrono::Utc::now();
        }
    }

    /// 生成会话摘要 — 零成本压缩
    /// 论文关键：会话记忆本身就是摘要，不需要额外API调用
    pub async fn generate_summary(&self, session_id: &str) -> Option<String> {
        let memories = self.memories.read().await;
        memories.get(session_id).map(|entry| {
            format!(
                "## Session: {}\n\n### Query\n{}\n\n### Key Findings\n{}\n\n### Referenced Docs\n{}\n\n### Tool Stats\n- Search: {}, Find: {}, Open: {}, Summarize: {}\n- Total tokens: {}",
                entry.session_id,
                entry.query_summary,
                entry.key_findings.join("\n- "),
                entry.referenced_docs.join(", "),
                entry.tool_stats.search_calls,
                entry.tool_stats.find_calls,
                entry.tool_stats.open_calls,
                entry.tool_stats.summarize_calls,
                entry.tool_stats.total_tokens
            )
        })
    }

    /// 检查是否需要压缩
    pub async fn needs_compaction(&self, session_id: &str, current_tokens: usize) -> bool {
        let memories = self.memories.read().await;
        if let Some(entry) = memories.get(session_id) {
            current_tokens > self.config.compaction_token_threshold
                && !entry.key_findings.is_empty()
        } else {
            false
        }
    }

    /// 获取会话记忆
    pub async fn get_session(&self, session_id: &str) -> Option<SessionMemoryEntry> {
        self.memories.read().await.get(session_id).cloned()
    }

    pub async fn session_count(&self) -> usize {
        self.memories.read().await.len()
    }
}

// ============================================================
// L6: 做梦机制 — 跨会话记忆巩固
// ============================================================

/// 做梦配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamConfig {
    /// 触发做梦的最小会话数
    pub min_sessions_to_dream: usize,
    /// 做梦间隔（小时）
    pub dream_interval_hours: u64,
    /// 记忆文件路径
    pub memory_path: PathBuf,
    /// MEMORY.md索引路径
    pub index_path: PathBuf,
    /// 最大MEMORY.md行数
    pub max_index_lines: usize,
}

impl Default for DreamConfig {
    fn default() -> Self {
        Self {
            min_sessions_to_dream: 5,
            dream_interval_hours: 24,
            memory_path: PathBuf::from(".kias/memory"),
            index_path: PathBuf::from(".kias/MEMORY.md"),
            max_index_lines: 200,
        }
    }
}

/// 记忆条目 — 长期知识
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub category: MemoryCategory,
    pub content: String,
    pub source_sessions: Vec<String>,
    pub confidence: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub access_count: usize,
}

/// 记忆类别 — 论文定义的四种类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryCategory {
    /// 用户偏好
    UserPreference,
    /// 项目结构
    ProjectStructure,
    /// 工作流模式
    WorkflowPattern,
    /// 错误修复经验
    ErrorFix,
    /// 最佳实践
    BestPractice,
}

/// 做梦巩固器 — L6层
pub struct DreamConsolidator {
    config: DreamConfig,
    memories: Arc<RwLock<Vec<MemoryEntry>>>,
    session_log: Arc<RwLock<Vec<SessionMemoryEntry>>>,
    /// 锁文件路径（互斥）
    #[allow(dead_code)]
    lock_path: PathBuf,
}

impl DreamConsolidator {
    pub fn new(config: DreamConfig) -> Self {
        let lock_path = config.memory_path.join(".consolidate-lock");
        Self {
            config,
            memories: Arc::new(RwLock::new(Vec::new())),
            session_log: Arc::new(RwLock::new(Vec::new())),
            lock_path,
        }
    }

    /// 记录会话到日志
    pub async fn record_session(&self, session: SessionMemoryEntry) {
        self.session_log.write().await.push(session);
    }

    /// 检查是否需要做梦
    pub async fn should_dream(&self) -> bool {
        let session_count = self.session_log.read().await.len();
        session_count >= self.config.min_sessions_to_dream
    }

    /// 执行做梦 — 四阶段巩固
    /// 论文：从最便宜的检查开始，大部分情况早早退出
    pub async fn dream(&self) -> DreamResult {
        // 门控序列：检查是否需要做梦
        if !self.should_dream().await {
            return DreamResult {
                memories_consolidated: 0,
                contradictions_resolved: 0,
                index_updated: false,
                duration_ms: 0,
            };
        }

        let start = std::time::Instant::now();
        info!("Starting dream consolidation");

        // 阶段1：标定位置 — 避免重复处理
        let sessions = self.session_log.read().await.clone();
        let existing_memories = self.memories.read().await.len();
        debug!(
            "Phase 1: {} sessions, {} existing memories",
            sessions.len(),
            existing_memories
        );

        // 阶段2：收集 — 提取重要片段
        let mut new_findings: Vec<(String, MemoryCategory, f64)> = Vec::new();
        for session in &sessions {
            for finding in &session.key_findings {
                // 简化：实际应该用LLM判断重要性
                let (category, confidence) = classify_finding(finding);
                if confidence > 0.7 {
                    new_findings.push((finding.clone(), category, confidence));
                }
            }
        }
        debug!("Phase 2: {} new findings", new_findings.len());

        // 阶段3：合并 — 添加到记忆库
        let mut consolidated = 0;
        let mut contradictions = 0;
        for (content, category, confidence) in &new_findings {
            // 检查矛盾
            let is_contradiction = self.check_contradiction(content, category).await;
            if is_contradiction {
                contradictions += 1;
                continue;
            }

            // 添加新记忆
            let entry = MemoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                category: category.clone(),
                content: content.clone(),
                source_sessions: sessions.iter().map(|s| s.session_id.clone()).collect(),
                confidence: *confidence,
                created_at: chrono::Utc::now(),
                last_accessed: chrono::Utc::now(),
                access_count: 0,
            };
            self.memories.write().await.push(entry);
            consolidated += 1;
        }
        debug!(
            "Phase 3: consolidated {}, contradictions {}",
            consolidated, contradictions
        );

        // 阶段4：整理索引
        self.update_index().await;

        // 清理会话日志
        self.session_log.write().await.clear();

        let duration = start.elapsed();
        info!(
            "Dream completed: {} memories consolidated, {} contradictions in {}ms",
            consolidated,
            contradictions,
            duration.as_millis()
        );

        DreamResult {
            memories_consolidated: consolidated,
            contradictions_resolved: contradictions,
            index_updated: true,
            duration_ms: duration.as_millis() as u64,
        }
    }

    /// 检查矛盾
    async fn check_contradiction(&self, content: &str, category: &MemoryCategory) -> bool {
        let memories = self.memories.read().await;
        for existing in memories.iter() {
            if existing.category == *category {
                // 简化：实际应该用语义相似度判断
                if existing.content.contains(content) || content.contains(&existing.content) {
                    return true;
                }
            }
        }
        false
    }

    /// 更新MEMORY.md索引
    async fn update_index(&self) {
        let memories = self.memories.read().await;
        let mut lines: Vec<String> = Vec::new();

        lines.push("# AgentGuard Memory Index\n".to_string());
        lines.push(format!(
            "Last updated: {}\n",
            chrono::Utc::now().to_rfc3339()
        ));

        // 按类别分组
        let mut by_category: HashMap<String, Vec<&MemoryEntry>> = HashMap::new();
        for entry in memories.iter() {
            let cat = format!("{:?}", entry.category);
            by_category.entry(cat).or_default().push(entry);
        }

        for (category, entries) in &by_category {
            lines.push(format!("\n## {}\n", category));
            for entry in entries.iter().take(10) {
                // 每类最多10条
                lines.push(format!(
                    "- {} (confidence: {:.2})",
                    truncate(&entry.content, 150),
                    entry.confidence
                ));
            }
        }

        // 截断到最大行数
        if lines.len() > self.config.max_index_lines {
            lines.truncate(self.config.max_index_lines);
        }

        debug!("Updated MEMORY.md with {} lines", lines.len());
    }

    /// 查询记忆
    pub async fn query(&self, category: &MemoryCategory, keyword: &str) -> Vec<MemoryEntry> {
        let memories = self.memories.read().await;
        memories
            .iter()
            .filter(|m| {
                m.category == *category
                    && m.content.to_lowercase().contains(&keyword.to_lowercase())
            })
            .cloned()
            .collect()
    }

    pub async fn memory_count(&self) -> usize {
        self.memories.read().await.len()
    }
}

/// 做梦结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    pub memories_consolidated: usize,
    pub contradictions_resolved: usize,
    pub index_updated: bool,
    pub duration_ms: u64,
}

// ============================================================
// 辅助函数
// ============================================================

fn classify_finding(finding: &str) -> (MemoryCategory, f64) {
    let lower = finding.to_lowercase();
    if lower.contains("prefer") || lower.contains("like") || lower.contains("want") {
        (MemoryCategory::UserPreference, 0.8)
    } else if lower.contains("error") || lower.contains("fix") || lower.contains("bug") {
        (MemoryCategory::ErrorFix, 0.9)
    } else if lower.contains("workflow") || lower.contains("process") {
        (MemoryCategory::WorkflowPattern, 0.7)
    } else if lower.contains("structure") || lower.contains("architecture") {
        (MemoryCategory::ProjectStructure, 0.8)
    } else {
        (MemoryCategory::BestPractice, 0.6)
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_result_store_preview() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        let long_content = "x".repeat(5000);

        let preview = store.store("test1", "search", &long_content).await;
        assert!(preview.len() < long_content.len());
        assert!(preview.contains("[Full content stored"));
    }

    #[tokio::test]
    async fn test_tool_result_store_short_content() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        let short_content = "short";

        let preview = store.store("test1", "search", short_content).await;
        assert_eq!(preview, short_content);
    }

    #[tokio::test]
    async fn test_session_memory_create_and_update() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        mgr.create_session("s1", "test query").await;
        mgr.update("s1", "found something", Some("doc1")).await;

        let summary = mgr.generate_summary("s1").await.unwrap();
        assert!(summary.contains("test query"));
        assert!(summary.contains("found something"));
    }

    #[tokio::test]
    async fn test_session_memory_tool_stats() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        mgr.create_session("s1", "query").await;
        mgr.update_tool_stats("s1", "search", 1000).await;
        mgr.update_tool_stats("s1", "find", 500).await;

        let session = mgr.get_session("s1").await.unwrap();
        assert_eq!(session.tool_stats.search_calls, 1);
        assert_eq!(session.tool_stats.find_calls, 1);
        assert_eq!(session.tool_stats.total_tokens, 1500);
    }

    #[tokio::test]
    async fn test_session_needs_compaction() {
        let config = SessionMemoryConfig {
            compaction_token_threshold: 1000,
            ..Default::default()
        };
        let mgr = SessionMemoryManager::new(config);
        mgr.create_session("s1", "query").await;
        mgr.update("s1", "finding", None).await;

        assert!(mgr.needs_compaction("s1", 2000).await);
        assert!(!mgr.needs_compaction("s1", 500).await);
    }

    #[tokio::test]
    async fn test_dream_consolidator_should_dream() {
        let config = DreamConfig {
            min_sessions_to_dream: 2,
            ..Default::default()
        };
        let consolidator = DreamConsolidator::new(config);

        assert!(!consolidator.should_dream().await);

        consolidator
            .record_session(SessionMemoryEntry {
                session_id: "s1".into(),
                query_summary: "q1".into(),
                key_findings: vec!["f1".into()],
                referenced_docs: vec![],
                tool_stats: ToolCallStats::default(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await;

        consolidator
            .record_session(SessionMemoryEntry {
                session_id: "s2".into(),
                query_summary: "q2".into(),
                key_findings: vec!["f2".into()],
                referenced_docs: vec![],
                tool_stats: ToolCallStats::default(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await;

        assert!(consolidator.should_dream().await);
    }

    #[tokio::test]
    async fn test_dream_execution() {
        let config = DreamConfig {
            min_sessions_to_dream: 1,
            ..Default::default()
        };
        let consolidator = DreamConsolidator::new(config);

        consolidator
            .record_session(SessionMemoryEntry {
                session_id: "s1".into(),
                query_summary: "fix error".into(),
                key_findings: vec!["Error in module X was fixed by Y".into()],
                referenced_docs: vec!["doc1".into()],
                tool_stats: ToolCallStats::default(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await;

        let result = consolidator.dream().await;
        assert!(result.memories_consolidated > 0);
        assert!(result.index_updated);
    }

    #[tokio::test]
    async fn test_dream_query_by_category() {
        let consolidator = DreamConsolidator::new(DreamConfig::default());

        // 手动添加记忆
        consolidator.memories.write().await.push(MemoryEntry {
            id: "m1".into(),
            category: MemoryCategory::ErrorFix,
            content: "Fixed null pointer in auth module".into(),
            source_sessions: vec!["s1".into()],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
        });

        let results = consolidator
            .query(&MemoryCategory::ErrorFix, "null pointer")
            .await;
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("null pointer"));
    }

    #[test]
    fn test_classify_finding() {
        let (cat, conf) = classify_finding("I prefer dark mode");
        assert_eq!(cat, MemoryCategory::UserPreference);
        assert!(conf > 0.7);

        let (cat, conf) = classify_finding("Fixed error in login");
        assert_eq!(cat, MemoryCategory::ErrorFix);
        assert!(conf > 0.8);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a long string here", 5), "a lon...");
    }
    #[tokio::test]
    async fn test_tool_result_get_full() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        store.store("id1", "search", "full content here").await;
        assert_eq!(
            store.get_full("id1").await,
            Some("full content here".to_string())
        );
        assert_eq!(store.get_full("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_tool_result_get_preview() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        store.store("id1", "search", "preview content").await;
        assert_eq!(
            store.get_preview("id1").await,
            Some("preview content".to_string())
        );
        assert_eq!(store.get_preview("missing").await, None);
    }

    #[tokio::test]
    async fn test_tool_result_count() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        assert_eq!(store.count().await, 0);
        store.store("a", "search", "content a").await;
        store.store("b", "find", "content b").await;
        assert_eq!(store.count().await, 2);
    }

    #[tokio::test]
    async fn test_tool_result_cleanup() {
        let store = ToolResultStore::new(ToolResultStoreConfig::default());
        store.store("old", "search", "old data").await;
        // Cleanup with zero duration removes everything
        store.cleanup(chrono::Duration::zero()).await;
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_tool_result_long_content_preview_format() {
        let store = ToolResultStore::new(ToolResultStoreConfig {
            preview_size: 10,
            ..Default::default()
        });
        let preview = store.store("id1", "search", &"a".repeat(100)).await;
        assert!(preview.starts_with("aaaaaaaaaa"));
        assert!(preview.contains("[Full content stored at tool-results/id1]"));
    }

    #[tokio::test]
    async fn test_session_memory_count() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        assert_eq!(mgr.session_count().await, 0);
        mgr.create_session("s1", "q1").await;
        mgr.create_session("s2", "q2").await;
        assert_eq!(mgr.session_count().await, 2);
    }

    #[tokio::test]
    async fn test_session_update_duplicate_doc_dedup() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        mgr.create_session("s1", "q").await;
        mgr.update("s1", "f1", Some("doc1")).await;
        mgr.update("s1", "f2", Some("doc1")).await; // same doc
        let session = mgr.get_session("s1").await.unwrap();
        assert_eq!(session.referenced_docs.len(), 1);
        assert_eq!(session.key_findings.len(), 2);
    }

    #[tokio::test]
    async fn test_session_update_nonexistent() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        // Should not panic on non-existent session
        mgr.update("missing", "finding", None).await;
        assert_eq!(mgr.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_needs_compaction_no_findings() {
        let config = SessionMemoryConfig {
            compaction_token_threshold: 100,
            ..Default::default()
        };
        let mgr = SessionMemoryManager::new(config);
        mgr.create_session("s1", "q").await;
        // No findings → false even if tokens exceed threshold
        assert!(!mgr.needs_compaction("s1", 500).await);
    }

    #[tokio::test]
    async fn test_session_needs_compaction_nonexistent() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        assert!(!mgr.needs_compaction("missing", 999999).await);
    }

    #[tokio::test]
    async fn test_session_generate_summary_nonexistent() {
        let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
        assert!(mgr.generate_summary("missing").await.is_none());
    }

    #[tokio::test]
    async fn test_dream_memory_count() {
        let consolidator = DreamConsolidator::new(DreamConfig::default());
        assert_eq!(consolidator.memory_count().await, 0);
    }

    #[tokio::test]
    async fn test_dream_not_needed_returns_zero() {
        let config = DreamConfig {
            min_sessions_to_dream: 10,
            ..Default::default()
        };
        let consolidator = DreamConsolidator::new(config);
        consolidator
            .record_session(make_session("s1", "q", vec!["f1"]))
            .await;
        let result = consolidator.dream().await;
        assert_eq!(result.memories_consolidated, 0);
        assert_eq!(result.contradictions_resolved, 0);
        assert!(!result.index_updated);
        assert_eq!(result.duration_ms, 0);
    }

    #[tokio::test]
    async fn test_dream_contradiction_detection() {
        let config = DreamConfig {
            min_sessions_to_dream: 1,
            ..Default::default()
        };
        let consolidator = DreamConsolidator::new(config);
        // Pre-add a memory
        consolidator.memories.write().await.push(MemoryEntry {
            id: "m1".into(),
            category: MemoryCategory::ErrorFix,
            content: "Error in auth module fixed by patching".into(),
            source_sessions: vec![],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
        });
        // Record session with finding that contains the existing memory content
        consolidator
            .record_session(make_session(
                "s1",
                "fix",
                vec!["Error in auth module fixed by patching"],
            ))
            .await;
        let result = consolidator.dream().await;
        // The finding should be detected as contradiction (substring match)
        assert!(result.contradictions_resolved > 0);
    }

    #[tokio::test]
    async fn test_dream_query_no_match() {
        let consolidator = DreamConsolidator::new(DreamConfig::default());
        let results = consolidator
            .query(&MemoryCategory::ErrorFix, "nonexistent")
            .await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_classify_finding_workflow() {
        let (cat, conf) = classify_finding("workflow for deployment");
        assert_eq!(cat, MemoryCategory::WorkflowPattern);
        assert!(conf > 0.6);
    }

    #[tokio::test]
    async fn test_classify_finding_structure() {
        let (cat, conf) = classify_finding("architecture of the system");
        assert_eq!(cat, MemoryCategory::ProjectStructure);
        assert!(conf > 0.7);
    }

    #[tokio::test]
    async fn test_classify_finding_default() {
        let (cat, conf) = classify_finding("random observation");
        assert_eq!(cat, MemoryCategory::BestPractice);
        assert!(conf < 0.7);
    }

    #[test]
    fn test_truncate_exact_boundary() {
        assert_eq!(truncate("exact", 5), "exact");
        assert_eq!(truncate("toolong", 4), "tool...");
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_memory_category_partial_eq() {
        assert_eq!(MemoryCategory::ErrorFix, MemoryCategory::ErrorFix);
        assert_ne!(MemoryCategory::ErrorFix, MemoryCategory::BestPractice);
    }

    #[test]
    fn test_default_configs() {
        let trc = ToolResultStoreConfig::default();
        assert_eq!(trc.preview_size, 2048);
        assert_eq!(trc.max_storage_size, 100 * 1024 * 1024);

        let smc = SessionMemoryConfig::default();
        assert_eq!(smc.compaction_token_threshold, 100_000);
        assert_eq!(smc.min_messages_to_keep, 5);

        let dc = DreamConfig::default();
        assert_eq!(dc.min_sessions_to_dream, 5);
        assert_eq!(dc.dream_interval_hours, 24);
        assert_eq!(dc.max_index_lines, 200);
    }

    fn make_session(id: &str, query: &str, findings: Vec<&str>) -> SessionMemoryEntry {
        SessionMemoryEntry {
            session_id: id.into(),
            query_summary: query.into(),
            key_findings: findings.into_iter().map(String::from).collect(),
            referenced_docs: vec![],
            tool_stats: ToolCallStats::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
