//! InspirationStream — 正向循环灵感注入组件
//!
//! 在开发过程中，Builder 专注写代码，Thinker 专注找更好的做法。
//! 两个注意力流并行，产出汇合到同一个工作区。
//!
//! ## 正向循环
//!
//! ```text
//! Builder 构建 → Thinker 发现相关洞察 → 注入到工作区 → Builder 受启发改进 → ...
//! ```
//!
//! ## 三层架构
//!
//! 1. **灵感源（Source）**: GitHub trending、论文、技术博客、创新 Agent
//! 2. **过滤器（Filter）**: 按相关性、时效性、质量过滤
//! 3. **注入器（Injector）**: 在合适的时机注入到 Builder 的工作流
//!
//! ## 设计原则
//!
//! - Thinker 不打断 Builder，只在检查点注入
//! - 灵感有生命周期，过期自动清理
//! - 每个灵感有「行动建议」，不只是信息
//! - 正向激励：采纳的灵感提升相关源的权重

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

// ============================================================
// 灵感源
// ============================================================

/// 灵感来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InspirationSource {
    /// GitHub trending 项目
    GitHubTrending,
    /// 技术论文 (arXiv 等)
    Paper,
    /// 技术博客 / 文章
    Blog,
    /// 创新 Agent 发现
    InnovationAgent,
    /// 用户提供的参考
    UserReference,
    /// 跨会话记忆（DreamConsolidator）
    Memory,
    /// 对话中的洞察
    ConversationInsight,
}

/// 灵感条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspiration {
    /// 唯一 ID
    pub id: String,
    /// 来源
    pub source: InspirationSource,
    /// 标题（一句话概括）
    pub title: String,
    /// 详细内容
    pub content: String,
    /// 与当前任务的关联描述
    pub relevance: String,
    /// 行动建议（具体可以做什么）
    pub action_suggestion: String,
    /// 相关性分数 0.0 - 1.0
    pub relevance_score: f64,
    /// 质量分数 0.0 - 1.0
    pub quality_score: f64,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 过期时间
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// 状态
    pub status: InspirationStatus,
    /// 被采纳次数
    pub adoption_count: u32,
    /// 来源 URL（如果有）
    pub source_url: Option<String>,
}

/// 灵感状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InspirationStatus {
    /// 新发现，待注入
    Discovered,
    /// 已注入到 Builder 工作流
    Injected,
    /// Builder 已查看
    Viewed,
    /// Builder 采纳了建议
    Adopted,
    /// Builder 忽略了
    Ignored,
    /// 已过期
    Expired,
}

// ============================================================
// 过滤器
// ============================================================

/// 过滤配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// 最低相关性分数
    pub min_relevance: f64,
    /// 最低质量分数
    pub min_quality: f64,
    /// 最大同时持有的灵感数
    pub max_active_inspirations: usize,
    /// 灵感过期时间（秒）
    pub expiration_secs: i64,
    /// 同类源去重窗口（秒）
    pub dedup_window_secs: i64,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            min_relevance: 0.3,
            min_quality: 0.4,
            max_active_inspirations: 20,
            expiration_secs: 3600,  // 1 小时
            dedup_window_secs: 300, // 5 分钟
        }
    }
}

/// 灵感过滤器
pub struct InspirationFilter {
    config: FilterConfig,
    /// 最近注入的灵感标题（用于去重）
    #[allow(clippy::type_complexity)]
    recent_titles: Arc<RwLock<VecDeque<(String, chrono::DateTime<chrono::Utc>)>>>,
}

impl InspirationFilter {
    pub fn new(config: FilterConfig) -> Self {
        Self {
            config,
            recent_titles: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// 检查灵感是否应该通过过滤
    pub async fn should_accept(&self, inspiration: &Inspiration) -> bool {
        // 分数检查
        if inspiration.relevance_score < self.config.min_relevance {
            debug!(
                "Rejected: relevance {} < {}",
                inspiration.relevance_score, self.config.min_relevance
            );
            return false;
        }
        if inspiration.quality_score < self.config.min_quality {
            debug!(
                "Rejected: quality {} < {}",
                inspiration.quality_score, self.config.min_quality
            );
            return false;
        }

        // 过期检查
        if inspiration.expires_at < chrono::Utc::now() {
            debug!("Rejected: expired");
            return false;
        }

        // 去重检查
        let now = chrono::Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.config.dedup_window_secs);
        let recent = self.recent_titles.read().await;
        let title_lower = inspiration.title.to_lowercase();
        for (recent_title, timestamp) in recent.iter() {
            if *timestamp > cutoff && recent_title.to_lowercase() == title_lower {
                debug!("Rejected: duplicate '{}'", inspiration.title);
                return false;
            }
        }

        true
    }

    /// 记录已注入的灵感标题
    pub async fn record_injection(&self, title: &str) {
        let mut recent = self.recent_titles.write().await;
        recent.push_back((title.to_string(), chrono::Utc::now()));

        // 清理过期记录
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(self.config.dedup_window_secs);
        while let Some((_, ts)) = recent.front() {
            if *ts < cutoff {
                recent.pop_front();
            } else {
                break;
            }
        }
    }
}

// ============================================================
// 正向循环引擎
// ============================================================

/// 正向循环统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PositiveLoopStats {
    /// 总发现数
    pub total_discovered: u64,
    /// 总注入数
    pub total_injected: u64,
    /// 总采纳数
    pub total_adopted: u64,
    /// 总忽略数
    pub total_ignored: u64,
    /// 采纳率
    pub adoption_rate: f64,
    /// 各源的采纳率
    pub source_adoption_rates: HashMap<String, f64>,
    /// 循环次数（发现→注入→采纳→构建→发现）
    pub loop_cycles: u64,
}

/// 正向循环引擎
///
/// 核心机制：
/// 1. Builder 完成一步 → 触发发现
/// 2. Thinker 发现灵感 → 过滤 → 注入
/// 3. Builder 查看 → 采纳/忽略
/// 4. 采纳 → 提升相关源权重 → 更多好灵感
pub struct PositiveLoopEngine {
    /// 源权重（被采纳越多，权重越高）
    source_weights: Arc<RwLock<HashMap<InspirationSource, f64>>>,
    /// 统计
    stats: Arc<RwLock<PositiveLoopStats>>,
}

impl PositiveLoopEngine {
    pub fn new() -> Self {
        let mut weights = HashMap::new();
        weights.insert(InspirationSource::GitHubTrending, 1.0);
        weights.insert(InspirationSource::Paper, 1.2);
        weights.insert(InspirationSource::Blog, 0.8);
        weights.insert(InspirationSource::InnovationAgent, 1.0);
        weights.insert(InspirationSource::UserReference, 1.5);
        weights.insert(InspirationSource::Memory, 1.3);
        weights.insert(InspirationSource::ConversationInsight, 1.4);

        Self {
            source_weights: Arc::new(RwLock::new(weights)),
            stats: Arc::new(RwLock::new(PositiveLoopStats::default())),
        }
    }

    /// 获取源的当前权重
    pub async fn get_weight(&self, source: &InspirationSource) -> f64 {
        let weights = self.source_weights.read().await;
        weights.get(source).copied().unwrap_or(1.0)
    }

    /// 记录采纳 — 提升源权重（正向激励）
    pub async fn record_adoption(&self, source: &InspirationSource) {
        let mut weights = self.source_weights.write().await;
        let weight = weights.entry(source.clone()).or_insert(1.0);
        // 每次采纳提升 5%，上限 3.0
        *weight = (*weight * 1.05).min(3.0);

        let mut stats = self.stats.write().await;
        stats.total_adopted += 1;
        self.update_rates(&mut stats).await;

        info!(
            "Source {:?} weight increased to {:.2} (adoption #{})",
            source, *weight, stats.total_adopted
        );
    }

    /// 记录忽略 — 轻微降低源权重
    pub async fn record_ignoring(&self, source: &InspirationSource) {
        let mut weights = self.source_weights.write().await;
        let weight = weights.entry(source.clone()).or_insert(1.0);
        // 每次忽略降低 1%，下限 0.3
        *weight = (*weight * 0.99).max(0.3);

        let mut stats = self.stats.write().await;
        stats.total_ignored += 1;
        self.update_rates(&mut stats).await;
    }

    /// 记录发现
    pub async fn record_discovery(&self) {
        let mut stats = self.stats.write().await;
        stats.total_discovered += 1;
    }

    /// 记录注入
    pub async fn record_injection(&self) {
        let mut stats = self.stats.write().await;
        stats.total_injected += 1;
    }

    /// 记录一次完整循环
    pub async fn record_cycle(&self) {
        let mut stats = self.stats.write().await;
        stats.loop_cycles += 1;
        info!("Positive loop cycle #{}", stats.loop_cycles);
    }

    /// 获取统计
    pub async fn stats(&self) -> PositiveLoopStats {
        self.stats.read().await.clone()
    }

    async fn update_rates(&self, stats: &mut PositiveLoopStats) {
        let total = stats.total_adopted + stats.total_ignored;
        if total > 0 {
            stats.adoption_rate = stats.total_adopted as f64 / total as f64;
        }
    }
}

impl Default for PositiveLoopEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// InspirationStream — 主组件
// ============================================================

/// InspirationStream 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspirationStreamConfig {
    /// 过滤配置
    pub filter: FilterConfig,
    /// 是否启用自动注入
    pub auto_inject: bool,
    /// 注入间隔（Builder 完成 N 步后注入一次）
    pub inject_interval_steps: u32,
    /// 最大待处理灵感队列大小
    pub max_queue_size: usize,
}

impl Default for InspirationStreamConfig {
    fn default() -> Self {
        Self {
            filter: FilterConfig::default(),
            auto_inject: true,
            inject_interval_steps: 3,
            max_queue_size: 50,
        }
    }
}

/// InspirationStream — 正向循环灵感注入
///
/// 与 Builder 和 Checker 并行运行的第三个角色。
/// 不打断 Builder，只在检查点注入。
pub struct InspirationStream {
    config: InspirationStreamConfig,
    /// 待注入队列
    queue: Arc<RwLock<VecDeque<Inspiration>>>,
    /// 已注入历史
    history: Arc<RwLock<Vec<Inspiration>>>,
    /// 过滤器
    filter: InspirationFilter,
    /// 正向循环引擎
    loop_engine: PositiveLoopEngine,
    /// Builder 步骤计数
    step_count: Arc<RwLock<u32>>,
    /// 当前任务上下文（用于相关性匹配）
    task_context: Arc<RwLock<String>>,
}

impl InspirationStream {
    pub fn new(config: InspirationStreamConfig) -> Self {
        let filter = InspirationFilter::new(config.filter.clone());
        Self {
            config,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            filter,
            loop_engine: PositiveLoopEngine::new(),
            step_count: Arc::new(RwLock::new(0)),
            task_context: Arc::new(RwLock::new(String::new())),
        }
    }

    /// 更新当前任务上下文
    pub async fn set_task_context(&self, context: String) {
        *self.task_context.write().await = context;
    }

    /// 提交一个灵感（来自任何源）
    pub async fn submit(&self, mut inspiration: Inspiration) -> bool {
        self.loop_engine.record_discovery().await;

        // 应用源权重调整相关性分数
        let weight = self.loop_engine.get_weight(&inspiration.source).await;
        inspiration.relevance_score = (inspiration.relevance_score * weight).min(1.0);

        // 过滤
        if !self.filter.should_accept(&inspiration).await {
            return false;
        }

        // 入队
        let mut queue = self.queue.write().await;
        if queue.len() >= self.config.max_queue_size {
            // 移除最旧的
            queue.pop_front();
        }
        let title = inspiration.title.clone();
        queue.push_back(inspiration);

        self.filter.record_injection(&title).await;
        self.loop_engine.record_injection().await;

        debug!("Inspiration queued: '{}'", title);
        true
    }

    /// Builder 完成一步后调用
    pub async fn on_builder_step(&self) -> Vec<Inspiration> {
        let mut count = self.step_count.write().await;
        *count += 1;

        if !self.config.auto_inject {
            return vec![];
        }

        // 每 N 步注入一次
        if *count % self.config.inject_interval_steps != 0 {
            return vec![];
        }

        self.drain_inspirations().await
    }

    /// 手动获取待注入的灵感
    pub async fn drain_inspirations(&self) -> Vec<Inspiration> {
        let mut queue = self.queue.write().await;
        let now = chrono::Utc::now();

        // 过滤掉过期的
        queue.retain(|i| i.expires_at > now);

        // 取出最多 3 个最高分的
        let mut items: Vec<Inspiration> = queue.drain(..).collect();
        items.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let selected: Vec<Inspiration> = items.drain(..3.min(items.len())).collect();

        // 剩余放回
        for item in items {
            queue.push_back(item);
        }

        // 记录到历史
        let mut history = self.history.write().await;
        for item in &selected {
            let mut item = item.clone();
            item.status = InspirationStatus::Injected;
            history.push(item);
        }

        if !selected.is_empty() {
            info!("Injected {} inspirations to Builder", selected.len());
        }

        selected
    }

    /// Builder 反馈：采纳了一个灵感
    pub async fn adopt(&self, inspiration_id: &str) {
        let mut history = self.history.write().await;
        if let Some(item) = history.iter_mut().find(|i| i.id == inspiration_id) {
            item.status = InspirationStatus::Adopted;
            item.adoption_count += 1;
            self.loop_engine.record_adoption(&item.source).await;
            info!("Inspiration adopted: '{}'", item.title);
        }
    }

    /// Builder 反馈：忽略了一个灵感
    pub async fn ignore(&self, inspiration_id: &str) {
        let mut history = self.history.write().await;
        if let Some(item) = history.iter_mut().find(|i| i.id == inspiration_id) {
            item.status = InspirationStatus::Ignored;
            self.loop_engine.record_ignoring(&item.source).await;
            debug!("Inspiration ignored: '{}'", item.title);
        }
    }

    /// 获取队列中的灵感数
    pub async fn pending_count(&self) -> usize {
        self.queue.read().await.len()
    }

    /// 获取历史灵感数
    pub async fn history_count(&self) -> usize {
        self.history.read().await.len()
    }

    /// 获取正向循环统计
    pub async fn loop_stats(&self) -> PositiveLoopStats {
        self.loop_engine.stats().await
    }

    /// 获取当前源权重
    pub async fn source_weights(&self) -> HashMap<String, f64> {
        let weights = self.loop_engine.source_weights.read().await;
        weights
            .iter()
            .map(|(k, v)| (format!("{:?}", k), *v))
            .collect()
    }

    /// 获取 Builder 步骤计数
    pub async fn step_count(&self) -> u32 {
        *self.step_count.read().await
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 创建灵感的便捷函数
pub fn create_inspiration(
    source: InspirationSource,
    title: &str,
    content: &str,
    relevance: &str,
    action_suggestion: &str,
    relevance_score: f64,
) -> Inspiration {
    let now = chrono::Utc::now();
    Inspiration {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        title: title.to_string(),
        content: content.to_string(),
        relevance: relevance.to_string(),
        action_suggestion: action_suggestion.to_string(),
        relevance_score,
        quality_score: 0.5,
        created_at: now,
        expires_at: now + chrono::Duration::hours(1),
        status: InspirationStatus::Discovered,
        adoption_count: 0,
        source_url: None,
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspiration_creation() {
        let insp = create_inspiration(
            InspirationSource::GitHubTrending,
            "New Rust Agent Framework",
            "A new framework with interesting design...",
            "Related to AgentGuard architecture",
            "Consider adopting their plugin system",
            0.8,
        );
        assert_eq!(insp.title, "New Rust Agent Framework");
        assert_eq!(insp.status, InspirationStatus::Discovered);
        assert!(insp.relevance_score > 0.0);
    }

    #[test]
    fn test_filter_config_default() {
        let config = FilterConfig::default();
        assert_eq!(config.min_relevance, 0.3);
        assert_eq!(config.max_active_inspirations, 20);
    }

    #[tokio::test]
    async fn test_filter_accepts_good_inspiration() {
        let filter = InspirationFilter::new(FilterConfig::default());
        let insp = create_inspiration(
            InspirationSource::Blog,
            "Good Article",
            "Content",
            "Relevant",
            "Do this",
            0.8,
        );
        assert!(filter.should_accept(&insp).await);
    }

    #[tokio::test]
    async fn test_filter_rejects_low_relevance() {
        let filter = InspirationFilter::new(FilterConfig::default());
        let insp = create_inspiration(
            InspirationSource::Blog,
            "Low Relevance",
            "Content",
            "Not relevant",
            "Skip",
            0.1,
        );
        assert!(!filter.should_accept(&insp).await);
    }

    #[tokio::test]
    async fn test_filter_rejects_expired() {
        let filter = InspirationFilter::new(FilterConfig::default());
        let mut insp = create_inspiration(
            InspirationSource::Blog,
            "Expired",
            "Content",
            "Relevant",
            "Do this",
            0.8,
        );
        insp.expires_at = chrono::Utc::now() - chrono::Duration::hours(1);
        assert!(!filter.should_accept(&insp).await);
    }

    #[tokio::test]
    async fn test_filter_dedup() {
        let filter = InspirationFilter::new(FilterConfig {
            dedup_window_secs: 300,
            ..Default::default()
        });
        let insp1 = create_inspiration(
            InspirationSource::Blog,
            "Same Title",
            "Content 1",
            "Relevant",
            "Do this",
            0.8,
        );
        let insp2 = create_inspiration(
            InspirationSource::Blog,
            "Same Title",
            "Content 2",
            "Relevant",
            "Do this",
            0.8,
        );

        assert!(filter.should_accept(&insp1).await);
        filter.record_injection("Same Title").await;
        assert!(!filter.should_accept(&insp2).await);
    }

    #[tokio::test]
    async fn test_positive_loop_engine_adoption() {
        let engine = PositiveLoopEngine::new();
        let initial_weight = engine.get_weight(&InspirationSource::Blog).await;

        engine.record_adoption(&InspirationSource::Blog).await;
        let new_weight = engine.get_weight(&InspirationSource::Blog).await;

        assert!(new_weight > initial_weight);
    }

    #[tokio::test]
    async fn test_positive_loop_engine_ignoring() {
        let engine = PositiveLoopEngine::new();
        let initial_weight = engine.get_weight(&InspirationSource::Blog).await;

        engine.record_ignoring(&InspirationSource::Blog).await;
        let new_weight = engine.get_weight(&InspirationSource::Blog).await;

        assert!(new_weight < initial_weight);
    }

    #[tokio::test]
    async fn test_positive_loop_weight_bounds() {
        let engine = PositiveLoopEngine::new();

        // 多次采纳，权重应该有上限
        for _ in 0..100 {
            engine.record_adoption(&InspirationSource::Blog).await;
        }
        let weight = engine.get_weight(&InspirationSource::Blog).await;
        assert!(weight <= 3.0);

        // 多次忽略，权重应该有下限
        for _ in 0..100 {
            engine
                .record_ignoring(&InspirationSource::GitHubTrending)
                .await;
        }
        let weight = engine.get_weight(&InspirationSource::GitHubTrending).await;
        assert!(weight >= 0.3);
    }

    #[tokio::test]
    async fn test_inspiration_stream_submit() {
        let stream = InspirationStream::new(InspirationStreamConfig::default());
        let insp = create_inspiration(
            InspirationSource::InnovationAgent,
            "Interesting Pattern",
            "Content about a new pattern",
            "Related to current task",
            "Try implementing this",
            0.7,
        );

        let accepted = stream.submit(insp).await;
        assert!(accepted);
        assert_eq!(stream.pending_count().await, 1);
    }

    #[tokio::test]
    async fn test_inspiration_stream_auto_inject() {
        let config = InspirationStreamConfig {
            auto_inject: true,
            inject_interval_steps: 2,
            ..Default::default()
        };
        let stream = InspirationStream::new(config);

        // 提交一个灵感
        let insp = create_inspiration(
            InspirationSource::Blog,
            "Test",
            "Content",
            "Relevant",
            "Do this",
            0.8,
        );
        stream.submit(insp).await;

        // 第 1 步不应该注入
        let result = stream.on_builder_step().await;
        assert!(result.is_empty());

        // 第 2 步应该注入
        let result = stream.on_builder_step().await;
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_inspiration_stream_adoption_feedback() {
        let stream = InspirationStream::new(InspirationStreamConfig::default());
        let insp = create_inspiration(
            InspirationSource::InnovationAgent,
            "Great Idea",
            "Content",
            "Very relevant",
            "Implement this",
            0.9,
        );
        let id = insp.id.clone();
        stream.submit(insp).await;

        // 手动取出
        let inspirations = stream.drain_inspirations().await;
        assert_eq!(inspirations.len(), 1);

        // 采纳
        stream.adopt(&id).await;
        let stats = stream.loop_stats().await;
        assert_eq!(stats.total_adopted, 1);
    }

    #[tokio::test]
    async fn test_inspiration_stream_ignore_feedback() {
        let stream = InspirationStream::new(InspirationStreamConfig::default());
        let insp = create_inspiration(
            InspirationSource::Blog,
            "Not Useful",
            "Content",
            "Somewhat relevant",
            "Skip this",
            0.5,
        );
        let id = insp.id.clone();
        stream.submit(insp).await;

        let _inspirations = stream.drain_inspirations().await;
        stream.ignore(&id).await;

        let stats = stream.loop_stats().await;
        assert_eq!(stats.total_ignored, 1);
    }

    #[tokio::test]
    async fn test_inspiration_stream_context_update() {
        let stream = InspirationStream::new(InspirationStreamConfig::default());
        stream
            .set_task_context("Building a new scheduler for AgentGuard".to_string())
            .await;
        assert_eq!(
            *stream.task_context.read().await,
            "Building a new scheduler for AgentGuard"
        );
    }

    #[test]
    fn test_inspiration_status_variants() {
        let statuses = [
            InspirationStatus::Discovered,
            InspirationStatus::Injected,
            InspirationStatus::Viewed,
            InspirationStatus::Adopted,
            InspirationStatus::Ignored,
            InspirationStatus::Expired,
        ];
        assert_eq!(statuses.len(), 6);
    }

    #[test]
    fn test_inspiration_source_variants() {
        let sources = [
            InspirationSource::GitHubTrending,
            InspirationSource::Paper,
            InspirationSource::Blog,
            InspirationSource::InnovationAgent,
            InspirationSource::UserReference,
            InspirationSource::Memory,
            InspirationSource::ConversationInsight,
        ];
        assert_eq!(sources.len(), 7);
    }

    #[tokio::test]
    async fn test_positive_loop_stats_default() {
        let stats = PositiveLoopStats::default();
        assert_eq!(stats.total_discovered, 0);
        assert_eq!(stats.adoption_rate, 0.0);
    }

    #[tokio::test]
    async fn test_inspiration_stream_queue_overflow() {
        let config = InspirationStreamConfig {
            max_queue_size: 3,
            auto_inject: false,
            ..Default::default()
        };
        let stream = InspirationStream::new(config);

        for i in 0..5 {
            let insp = create_inspiration(
                InspirationSource::Blog,
                &format!("Inspiration {}", i),
                "Content",
                "Relevant",
                "Do this",
                0.8,
            );
            stream.submit(insp).await;
        }

        // 队列应该只有 3 个（max_queue_size）
        assert_eq!(stream.pending_count().await, 3);
    }
}
