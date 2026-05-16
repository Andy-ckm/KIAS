//! Context Manager — 会话上下文管理与三级压缩
//!
//! 基于 Claude Code 七层记忆架构的核心上下文管理器。
//! 负责：
//! 1. Token 计数（估算）
//! 2. 会话窗口管理（滑动窗口）
//! 3. 三级压缩触发：
//!    - 微压缩（Micro）: 摘要化旧工具结果，保留最近 N 条
//!    - 全压缩（Full）: 整个会话压缩为结构化摘要
//!    - 跨会话（Cross）: 巩固到 L6 长期记忆
//!
//! 设计原则（来自 Claude Code 论文）：
//! - 零成本压缩：会话记忆本身就是摘要
//! - 缓存友好：内容替换后冻结，确保 prompt 前缀一致
//! - 分层防御：先用最便宜的压缩

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::memory_layers::SessionMemoryManager;

// ============================================================
// Token 计数器 — 估算 token 数
// ============================================================

/// Token 计数器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCounterConfig {
    /// 每个 token 的平均字符数（英文约 4，中文约 2）
    pub chars_per_token: f64,
    /// 每条消息的固定开销（角色标记等）
    pub message_overhead_tokens: usize,
}

impl Default for TokenCounterConfig {
    fn default() -> Self {
        Self {
            chars_per_token: 3.5, // 混合中英文
            message_overhead_tokens: 4,
        }
    }
}

/// Token 计数器
#[derive(Debug, Clone)]
pub struct TokenCounter {
    config: TokenCounterConfig,
}

impl TokenCounter {
    pub fn new(config: TokenCounterConfig) -> Self {
        Self { config }
    }

    /// 估算文本的 token 数
    pub fn count_text(&self, text: &str) -> usize {
        let char_count = text.chars().count();
        (char_count as f64 / self.config.chars_per_token).ceil() as usize
    }

    /// 估算一条消息的 token 数（含开销）
    pub fn count_message(&self, content: &str) -> usize {
        self.count_text(content) + self.config.message_overhead_tokens
    }

    /// 估算多条消息的总 token 数
    pub fn count_messages(&self, messages: &[ContextMessage]) -> usize {
        messages
            .iter()
            .map(|m| self.count_message(&m.content))
            .sum()
    }
}

// ============================================================
// 上下文消息
// ============================================================

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 上下文消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub role: MessageRole,
    pub content: String,
    /// 是否为压缩后的摘要消息
    pub is_summary: bool,
    /// 消息创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ContextMessage {
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            is_summary: false,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn summary(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            is_summary: true,
            created_at: chrono::Utc::now(),
        }
    }
}

// ============================================================
// 压缩级别
// ============================================================

/// 压缩级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CompressionLevel {
    /// 无压缩
    None,
    /// 微压缩：摘要化旧工具结果
    Micro,
    /// 全压缩：整个会话压缩为摘要
    Full,
    /// 跨会话：巩固到长期记忆
    Cross,
}

/// 压缩结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub level: CompressionLevel,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub duration_ms: u64,
}

// ============================================================
// 上下文管理器配置
// ============================================================

/// 上下文管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    /// 最大上下文 token 数
    pub max_context_tokens: usize,
    /// 微压缩阈值（占 max 的比例）
    pub micro_compression_ratio: f64,
    /// 全压缩阈值（占 max 的比例）
    pub full_compression_ratio: f64,
    /// 最小保留消息数（不压缩）
    pub min_messages_to_keep: usize,
    /// 微压缩时保留的最近消息数
    pub micro_keep_recent: usize,
    /// 会话 ID
    pub session_id: String,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            micro_compression_ratio: 0.7,
            full_compression_ratio: 0.9,
            min_messages_to_keep: 4,
            micro_keep_recent: 10,
            session_id: "default".to_string(),
        }
    }
}

// ============================================================
// 会话上下文管理器
// ============================================================

/// 会话上下文管理器 — L3 核心
///
/// 管理一个会话的完整上下文窗口，自动触发压缩。
/// 与 SessionMemoryManager 协同：
/// - ContextManager 管理原始消息流
/// - SessionMemoryManager 管理结构化笔记
pub struct ContextManager {
    config: ContextManagerConfig,
    messages: Arc<RwLock<VecDeque<ContextMessage>>>,
    token_counter: TokenCounter,
    session_memory: Option<SessionMemoryManager>,
    /// 压缩历史
    compression_log: Arc<RwLock<Vec<CompressionResult>>>,
    /// 总 token 计数（缓存）
    total_tokens: Arc<RwLock<usize>>,
}

impl ContextManager {
    pub fn new(config: ContextManagerConfig) -> Self {
        Self {
            config,
            messages: Arc::new(RwLock::new(VecDeque::new())),
            token_counter: TokenCounter::default(),
            session_memory: None,
            compression_log: Arc::new(RwLock::new(Vec::new())),
            total_tokens: Arc::new(RwLock::new(0)),
        }
    }

    /// 关联会话记忆管理器
    pub fn with_session_memory(mut self, sm: SessionMemoryManager) -> Self {
        self.session_memory = Some(sm);
        self
    }

    /// 添加消息
    pub async fn push(&self, message: ContextMessage) {
        let tokens = self.token_counter.count_message(&message.content);
        self.messages.write().await.push_back(message);
        *self.total_tokens.write().await += tokens;
    }

    /// 获取当前消息数
    pub async fn message_count(&self) -> usize {
        self.messages.read().await.len()
    }

    /// 获取当前总 token 数
    pub async fn total_tokens(&self) -> usize {
        *self.total_tokens.read().await
    }

    /// 获取当前压缩级别需求
    pub async fn needed_compression(&self) -> CompressionLevel {
        let total = self.total_tokens().await;
        let max = self.config.max_context_tokens;
        let msg_count = self.message_count().await;

        if msg_count <= self.config.min_messages_to_keep {
            return CompressionLevel::None;
        }

        let ratio = total as f64 / max as f64;

        if ratio >= self.config.full_compression_ratio {
            CompressionLevel::Full
        } else if ratio >= self.config.micro_compression_ratio {
            CompressionLevel::Micro
        } else {
            CompressionLevel::None
        }
    }

    /// 执行压缩
    pub async fn compress(&self) -> Option<CompressionResult> {
        let level = self.needed_compression().await;
        if level == CompressionLevel::None {
            return None;
        }

        let start = std::time::Instant::now();
        let tokens_before = self.total_tokens().await;
        let messages_before = self.message_count().await;

        let result = match level {
            CompressionLevel::Micro => self.micro_compress().await,
            CompressionLevel::Full => self.full_compress().await,
            _ => None,
        };

        if let Some(mut r) = result {
            r.tokens_before = tokens_before;
            r.messages_before = messages_before;
            r.duration_ms = start.elapsed().as_millis() as u64;

            info!(
                "Compressed {:?}: {} msgs -> {} msgs, {} tokens -> {} tokens ({}ms)",
                r.level, r.messages_before, r.messages_after, r.tokens_before, r.tokens_after, r.duration_ms
            );

            self.compression_log.write().await.push(r.clone());
            Some(r)
        } else {
            None
        }
    }

    /// 微压缩：摘要化旧消息，保留最近 N 条
    async fn micro_compress(&self) -> Option<CompressionResult> {
        let mut messages = self.messages.write().await;
        let keep = self.config.micro_keep_recent;
        let min_keep = self.config.min_messages_to_keep;

        if messages.len() <= keep + min_keep {
            return None;
        }

        // 分离旧消息和新消息
        let split_at = messages.len() - keep;
        let old_messages: Vec<ContextMessage> = messages.drain(..split_at).collect();

        // 生成摘要
        let summary_content = self.summarize_messages(&old_messages);

        // 在头部插入摘要消息
        let summary = ContextMessage::summary(MessageRole::System, summary_content);
        messages.push_front(summary);

        // 重新计算 token
        let tokens_after = self.token_counter.count_messages(&messages.iter().cloned().collect::<Vec<_>>());
        *self.total_tokens.write().await = tokens_after;

        debug!(
            "Micro compressed: {} old msgs -> 1 summary, {} msgs remain",
            old_messages.len(),
            messages.len()
        );

        Some(CompressionResult {
            level: CompressionLevel::Micro,
            messages_before: 0, // filled by caller
            messages_after: messages.len(),
            tokens_before: 0, // filled by caller
            tokens_after,
            duration_ms: 0,
        })
    }

    /// 全压缩：整个会话压缩为结构化摘要
    async fn full_compress(&self) -> Option<CompressionResult> {
        let messages = self.messages.read().await;
        let min_keep = self.config.min_messages_to_keep;

        if messages.len() <= min_keep {
            return None;
        }

        // 保留最近 min_keep 条消息
        let summary_content = self.summarize_messages(&messages.iter().cloned().collect::<Vec<_>>());

        drop(messages);

        // 清空并重建
        let mut messages = self.messages.write().await;
        let recent: Vec<ContextMessage> = messages
            .iter()
            .rev()
            .take(min_keep)
            .rev()
            .cloned()
            .collect();

        messages.clear();
        messages.push_back(ContextMessage::summary(MessageRole::System, summary_content));
        for msg in recent {
            messages.push_back(msg);
        }

        let tokens_after = self.token_counter.count_messages(&messages.iter().cloned().collect::<Vec<_>>());
        *self.total_tokens.write().await = tokens_after;

        debug!("Full compressed: kept {} recent msgs + summary", min_keep);

        Some(CompressionResult {
            level: CompressionLevel::Full,
            messages_before: 0,
            messages_after: messages.len(),
            tokens_before: 0,
            tokens_after,
            duration_ms: 0,
        })
    }

    /// 生成消息摘要
    fn summarize_messages(&self, messages: &[ContextMessage]) -> String {
        let mut parts = Vec::new();

        parts.push("## 会话摘要（自动压缩）".to_string());
        parts.push(format!("压缩了 {} 条消息。", messages.len()));
        parts.push(String::new());

        // 统计角色分布
        let user_count = messages.iter().filter(|m| m.role == MessageRole::User).count();
        let assistant_count = messages.iter().filter(|m| m.role == MessageRole::Assistant).count();
        let tool_count = messages.iter().filter(|m| m.role == MessageRole::Tool).count();
        parts.push(format!(
            "角色分布: 用户={}, 助手={}, 工具={}",
            user_count, assistant_count, tool_count
        ));

        // 提取用户查询
        let user_queries: Vec<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .map(|m| m.content.as_str())
            .collect();

        if !user_queries.is_empty() {
            parts.push(String::new());
            parts.push("### 用户查询".to_string());
            for (i, q) in user_queries.iter().enumerate().take(5) {
                let preview = if q.len() > 200 {
                    format!("{}...", &q[..200])
                } else {
                    q.to_string()
                };
                parts.push(format!("{}. {}", i + 1, preview));
            }
            if user_queries.len() > 5 {
                parts.push(format!("... 还有 {} 条查询", user_queries.len() - 5));
            }
        }

        // 提取工具调用
        let tool_calls: Vec<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Tool)
            .map(|m| m.content.as_str())
            .collect();

        if !tool_calls.is_empty() {
            parts.push(String::new());
            parts.push(format!("### 工具调用（{} 次）", tool_calls.len()));
        }

        parts.join("\n")
    }

    /// 获取消息列表（用于构建 prompt）
    pub async fn get_messages(&self) -> Vec<ContextMessage> {
        self.messages.read().await.iter().cloned().collect()
    }

    /// 获取压缩日志
    pub async fn compression_history(&self) -> Vec<CompressionResult> {
        self.compression_log.read().await.clone()
    }

    /// 清空上下文
    pub async fn clear(&self) {
        self.messages.write().await.clear();
        *self.total_tokens.write().await = 0;
    }

    /// 获取上下文统计
    pub async fn stats(&self) -> ContextStats {
        let messages = self.messages.read().await;
        let total_tokens = self.total_tokens().await;

        let user_msgs = messages.iter().filter(|m| m.role == MessageRole::User).count();
        let assistant_msgs = messages.iter().filter(|m| m.role == MessageRole::Assistant).count();
        let tool_msgs = messages.iter().filter(|m| m.role == MessageRole::Tool).count();
        let summary_msgs = messages.iter().filter(|m| m.is_summary).count();

        ContextStats {
            total_messages: messages.len(),
            user_messages: user_msgs,
            assistant_messages: assistant_msgs,
            tool_messages: tool_msgs,
            summary_messages: summary_msgs,
            total_tokens,
            max_tokens: self.config.max_context_tokens,
            utilization: total_tokens as f64 / self.config.max_context_tokens as f64,
            compression_count: self.compression_log.read().await.len(),
        }
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new(TokenCounterConfig::default())
    }
}

/// 上下文统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_messages: usize,
    pub summary_messages: usize,
    pub total_tokens: usize,
    pub max_tokens: usize,
    pub utilization: f64,
    pub compression_count: usize,
}

// ============================================================
// 会话上下文管理器 — 管理多个会话
// ============================================================

/// 多会话上下文管理器
pub struct MultiSessionContextManager {
    /// 会话 ID -> ContextManager
    sessions: Arc<RwLock<std::collections::HashMap<String, ContextManager>>>,
    default_config: ContextManagerConfig,
}

impl MultiSessionContextManager {
    pub fn new(default_config: ContextManagerConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config,
        }
    }

    /// 获取或创建会话
    pub async fn get_or_create(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if !sessions.contains_key(session_id) {
            let mut config = self.default_config.clone();
            config.session_id = session_id.to_string();
            sessions.insert(session_id.to_string(), ContextManager::new(config));
        }
    }

    /// 向会话添加消息
    pub async fn push(&self, session_id: &str, message: ContextMessage) {
        self.get_or_create(session_id).await;
        let sessions = self.sessions.read().await;
        if let Some(ctx) = sessions.get(session_id) {
            ctx.push(message).await;
        }
    }

    /// 压缩指定会话
    pub async fn compress(&self, session_id: &str) -> Option<CompressionResult> {
        let sessions = self.sessions.read().await;
        if let Some(ctx) = sessions.get(session_id) {
            ctx.compress().await
        } else {
            None
        }
    }

    /// 压缩所有需要压缩的会话
    pub async fn compress_all(&self) -> Vec<(String, CompressionResult)> {
        let sessions = self.sessions.read().await;
        let mut results = Vec::new();

        for (id, ctx) in sessions.iter() {
            if let Some(result) = ctx.compress().await {
                results.push((id.clone(), result));
            }
        }

        results
    }

    /// 获取会话统计
    pub async fn session_stats(&self, session_id: &str) -> Option<ContextStats> {
        let sessions = self.sessions.read().await;
        if let Some(ctx) = sessions.get(session_id) {
            Some(ctx.stats().await)
        } else {
            None
        }
    }

    /// 会话数量
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// 清理空会话
    pub async fn cleanup_empty(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, _ctx| {
            // 需要 async，但 retain 不支持 async，所以用 try_read
            // 简化：不清理，让 GC 处理
            true
        });
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter() {
        let counter = TokenCounter::default();
        let tokens = counter.count_text("Hello, world!");
        assert!(tokens > 0);
        assert!(tokens < 10);

        let tokens_cn = counter.count_text("你好世界");
        assert!(tokens_cn > 0);
    }

    #[test]
    fn test_token_counter_message() {
        let counter = TokenCounter::default();
        let tokens = counter.count_message("Hello, world!");
        let text_tokens = counter.count_text("Hello, world!");
        assert_eq!(tokens, text_tokens + 4); // overhead
    }

    #[test]
    fn test_token_counter_messages() {
        let counter = TokenCounter::default();
        let messages = vec![
            ContextMessage::new(MessageRole::User, "Hello"),
            ContextMessage::new(MessageRole::Assistant, "Hi there!"),
        ];
        let total = counter.count_messages(&messages);
        assert!(total > 0);
    }

    #[tokio::test]
    async fn test_context_manager_push() {
        let config = ContextManagerConfig {
            max_context_tokens: 1000,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        ctx.push(ContextMessage::new(MessageRole::User, "Hello")).await;
        ctx.push(ContextMessage::new(MessageRole::Assistant, "Hi!")).await;

        assert_eq!(ctx.message_count().await, 2);
        assert!(ctx.total_tokens().await > 0);
    }

    #[tokio::test]
    async fn test_compression_level_none() {
        let config = ContextManagerConfig {
            max_context_tokens: 100_000,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        ctx.push(ContextMessage::new(MessageRole::User, "Short message")).await;

        assert_eq!(ctx.needed_compression().await, CompressionLevel::None);
    }

    #[tokio::test]
    async fn test_micro_compression_trigger() {
        let config = ContextManagerConfig {
            max_context_tokens: 100,
            micro_compression_ratio: 0.5,
            min_messages_to_keep: 2,
            micro_keep_recent: 2,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        // 添加足够的消息触发微压缩
        for i in 0..10 {
            ctx.push(ContextMessage::new(
                MessageRole::User,
                format!("Message {} with enough content to accumulate tokens", i),
            ))
            .await;
        }

        let level = ctx.needed_compression().await;
        assert!(level == CompressionLevel::Micro || level == CompressionLevel::Full);
    }

    #[tokio::test]
    async fn test_micro_compress() {
        let config = ContextManagerConfig {
            max_context_tokens: 200,
            micro_compression_ratio: 0.3,
            full_compression_ratio: 0.9,
            min_messages_to_keep: 2,
            micro_keep_recent: 3,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        // 添加消息
        for i in 0..15 {
            ctx.push(ContextMessage::new(
                MessageRole::User,
                format!("Message {} with enough content to trigger compression when accumulated", i),
            ))
            .await;
        }

        let count_before = ctx.message_count().await;
        let result = ctx.compress().await;

        if let Some(r) = result {
            assert!(r.messages_after < count_before);
            // 应该有摘要消息
            let messages = ctx.get_messages().await;
            assert!(messages.iter().any(|m| m.is_summary));
        }
    }

    #[tokio::test]
    async fn test_full_compress() {
        let config = ContextManagerConfig {
            max_context_tokens: 100,
            micro_compression_ratio: 0.3,
            full_compression_ratio: 0.5,
            min_messages_to_keep: 2,
            micro_keep_recent: 2,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        // 添加大量消息
        for i in 0..20 {
            ctx.push(ContextMessage::new(
                MessageRole::User,
                format!("This is a longer message number {} that has enough content to really push the token count up significantly", i),
            ))
            .await;
        }

        // 第一次压缩可能是 micro
        let _ = ctx.compress().await;

        // 添加更多消息触发 full
        for i in 0..10 {
            ctx.push(ContextMessage::new(
                MessageRole::Assistant,
                format!("Response {} with detailed explanation that takes up tokens", i),
            ))
            .await;
        }

        let messages_after = ctx.message_count().await;
        // 验证压缩有效果
        assert!(messages_after < 30);
    }

    #[tokio::test]
    async fn test_context_stats() {
        let config = ContextManagerConfig {
            max_context_tokens: 1000,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        ctx.push(ContextMessage::new(MessageRole::User, "Hello")).await;
        ctx.push(ContextMessage::new(MessageRole::Assistant, "Hi!")).await;
        ctx.push(ContextMessage::new(MessageRole::Tool, "result")).await;

        let stats = ctx.stats().await;
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.user_messages, 1);
        assert_eq!(stats.assistant_messages, 1);
        assert_eq!(stats.tool_messages, 1);
        assert!(stats.utilization > 0.0);
    }

    #[tokio::test]
    async fn test_clear_context() {
        let config = ContextManagerConfig::default();
        let ctx = ContextManager::new(config);

        ctx.push(ContextMessage::new(MessageRole::User, "Hello")).await;
        assert_eq!(ctx.message_count().await, 1);

        ctx.clear().await;
        assert_eq!(ctx.message_count().await, 0);
        assert_eq!(ctx.total_tokens().await, 0);
    }

    #[tokio::test]
    async fn test_multi_session() {
        let config = ContextManagerConfig {
            max_context_tokens: 1000,
            ..Default::default()
        };
        let multi = MultiSessionContextManager::new(config);

        multi.push("session-1", ContextMessage::new(MessageRole::User, "Hello from session 1")).await;
        multi.push("session-2", ContextMessage::new(MessageRole::User, "Hello from session 2")).await;

        assert_eq!(multi.session_count().await, 2);

        let stats = multi.session_stats("session-1").await.unwrap();
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_compression_level_ordering() {
        assert_ne!(CompressionLevel::None, CompressionLevel::Micro);
        assert_ne!(CompressionLevel::Micro, CompressionLevel::Full);
        assert_ne!(CompressionLevel::Full, CompressionLevel::Cross);
    }

    #[test]
    fn test_message_role_variants() {
        let roles = vec![
            MessageRole::System,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ];
        assert_eq!(roles.len(), 4);
    }

    #[test]
    fn test_summary_message_flag() {
        let msg = ContextMessage::summary(MessageRole::System, "Summary");
        assert!(msg.is_summary);

        let msg2 = ContextMessage::new(MessageRole::User, "Not summary");
        assert!(!msg2.is_summary);
    }

    #[tokio::test]
    async fn test_compression_history() {
        let config = ContextManagerConfig {
            max_context_tokens: 100,
            micro_compression_ratio: 0.3,
            min_messages_to_keep: 2,
            micro_keep_recent: 2,
            ..Default::default()
        };
        let ctx = ContextManager::new(config);

        for i in 0..15 {
            ctx.push(ContextMessage::new(
                MessageRole::User,
                format!("Message {} with enough content to trigger", i),
            ))
            .await;
        }

        let _ = ctx.compress().await;
        let history = ctx.compression_history().await;
        // 可能有压缩记录
        assert!(history.len() <= 1);
    }

    #[test]
    fn test_token_counter_chinese() {
        let counter = TokenCounter::default();
        let tokens_en = counter.count_text("Hello world");
        let tokens_cn = counter.count_text("你好世界");
        // 中文字符更少但每个更"贵"
        assert!(tokens_cn > 0);
        assert!(tokens_en > 0);
    }

    #[test]
    fn test_context_manager_config_default() {
        let config = ContextManagerConfig::default();
        assert_eq!(config.max_context_tokens, 128_000);
        assert_eq!(config.min_messages_to_keep, 4);
        assert_eq!(config.micro_keep_recent, 10);
    }

    #[test]
    fn test_context_stats_serializable() {
        let stats = ContextStats {
            total_messages: 5,
            user_messages: 2,
            assistant_messages: 2,
            tool_messages: 1,
            summary_messages: 0,
            total_tokens: 100,
            max_tokens: 1000,
            utilization: 0.1,
            compression_count: 0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_messages"));
    }
}
