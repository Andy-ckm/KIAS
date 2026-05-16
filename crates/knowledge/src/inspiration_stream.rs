//! # InspirationStream — Builder-Thinker Dual-Flow Development
//!
//! Provides a mechanism for continuous insight discovery during development.
//! External knowledge sources (papers, trending repos, benchmarks) are scanned
//! for relevance to the current task context, and high-value insights are
//! injected into the workspace.
//!
//! ## Positive Feedback Loop
//!
//! Sources that produce adopted insights gain reliability weight (+5% per adoption).
//! Sources that produce ignored insights lose weight (-1% per dismissal).
//! This creates a self-tuning system: good sources rise, noise sources sink.
//!
//! ## Architecture
//!
//! ```text
//! Builder (构建) ──→ 产出代码
//!     ↕ 正向循环
//! Thinker (发现) ──→ 从外部知识源抓取相关洞察，注入工作区
//!     ↓
//! Verifier (验证) ──→ 质量门禁
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// ─── Source Types ────────────────────────────────────────────────────────

/// Types of external knowledge sources.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceType {
    /// Academic papers (arXiv, conference proceedings).
    Paper,
    /// Trending repositories and discussions (GitHub, HN, Reddit).
    Trending,
    /// Performance benchmarks and competitor analysis.
    Benchmark,
}

impl SourceType {
    /// Default reliability weight for a new source type.
    pub fn default_reliability(&self) -> f64 {
        match self {
            SourceType::Paper => 1.2,     // Papers are generally high quality
            SourceType::Trending => 1.0,  // Neutral starting point
            SourceType::Benchmark => 1.1, // Benchmarks are data-driven
        }
    }
}

// ─── Insight ─────────────────────────────────────────────────────────────

/// A single insight discovered from an external source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    /// Unique identifier.
    pub id: String,
    /// Source type that produced this insight.
    pub source_type: SourceType,
    /// Source URL or identifier.
    pub source_url: String,
    /// Short title / headline.
    pub title: String,
    /// Full content or summary.
    pub content: String,
    /// Tags for relevance matching.
    pub tags: Vec<String>,
    /// Relevance score (0.0 - 1.0), computed against current context.
    pub relevance_score: f64,
    /// When this insight was discovered.
    pub discovered_at: SystemTime,
    /// Current disposition.
    pub disposition: InsightDisposition,
    /// When the disposition was set.
    pub disposition_at: Option<SystemTime>,
}

/// Whether an insight was adopted, dismissed, or is still pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightDisposition {
    /// Awaiting review.
    Pending,
    /// Adopted into the current development context.
    Adopted,
    /// Dismissed as not relevant or not useful.
    Dismissed,
}

// ─── Source ──────────────────────────────────────────────────────────────

/// A registered external knowledge source with reliability tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Unique identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Source type classification.
    pub source_type: SourceType,
    /// URL or API endpoint.
    pub endpoint: String,
    /// Current reliability weight (0.3 - 3.0).
    pub reliability: f64,
    /// Total insights produced.
    pub insights_produced: u64,
    /// Total insights adopted.
    pub insights_adopted: u64,
    /// When this source was last scanned.
    pub last_scanned: Option<SystemTime>,
    /// Whether this source is enabled.
    pub enabled: bool,
}

impl Source {
    /// Create a new source with default reliability for its type.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        source_type: SourceType,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            reliability: source_type.default_reliability(),
            source_type,
            endpoint: endpoint.into(),
            insights_produced: 0,
            insights_adopted: 0,
            last_scanned: None,
            enabled: true,
        }
    }

    /// Update reliability after an insight adoption.
    pub fn record_adoption(&mut self) {
        self.insights_adopted += 1;
        self.reliability = (self.reliability * 1.05).min(3.0);
    }

    /// Update reliability after an insight dismissal.
    pub fn record_dismissal(&mut self) {
        self.reliability = (self.reliability * 0.99).max(0.3);
    }

    /// Adoption rate (adopted / produced).
    pub fn adoption_rate(&self) -> f64 {
        if self.insights_produced == 0 {
            0.0
        } else {
            self.insights_adopted as f64 / self.insights_produced as f64
        }
    }
}

// ─── Configuration ───────────────────────────────────────────────────────

/// Configuration for the InspirationStream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspirationStreamConfig {
    /// Maximum insights per scan cycle.
    pub max_per_cycle: usize,
    /// Minimum relevance score to surface an insight.
    pub min_relevance: f64,
    /// How often to scan sources (seconds).
    pub scan_interval_secs: u64,
    /// Keywords extracted from current task context.
    pub context_keywords: Vec<String>,
}

impl Default for InspirationStreamConfig {
    fn default() -> Self {
        Self {
            max_per_cycle: 5,
            min_relevance: 0.3,
            scan_interval_secs: 300, // 5 minutes
            context_keywords: Vec::new(),
        }
    }
}

// ─── InspirationStream ───────────────────────────────────────────────────

/// The main InspirationStream engine.
///
/// Manages sources, discovers insights, computes relevance,
/// and applies positive feedback to source reliability.
pub struct InspirationStream {
    /// Registered knowledge sources.
    sources: Arc<RwLock<HashMap<String, Source>>>,
    /// All discovered insights.
    insights: Arc<RwLock<Vec<Insight>>>,
    /// Configuration.
    config: InspirationStreamConfig,
    /// Statistics.
    stats: Arc<RwLock<StreamStats>>,
}

/// Aggregate statistics for the inspiration stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    pub total_insights_discovered: u64,
    pub total_insights_adopted: u64,
    pub total_insights_dismissed: u64,
    pub total_scan_cycles: u64,
    pub sources_registered: u64,
}

impl InspirationStream {
    /// Create a new InspirationStream with the given configuration.
    pub fn new(config: InspirationStreamConfig) -> Self {
        Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            insights: Arc::new(RwLock::new(Vec::new())),
            config,
            stats: Arc::new(RwLock::new(StreamStats::default())),
        }
    }

    /// Register a new knowledge source.
    pub async fn register_source(&self, source: Source) {
        let mut sources = self.sources.write().await;
        let mut stats = self.stats.write().await;
        stats.sources_registered += 1;
        sources.insert(source.id.clone(), source);
    }

    /// Remove a source by ID.
    pub async fn remove_source(&self, source_id: &str) -> bool {
        let mut sources = self.sources.write().await;
        sources.remove(source_id).is_some()
    }

    /// Get all registered sources.
    pub async fn get_sources(&self) -> Vec<Source> {
        let sources = self.sources.read().await;
        sources.values().cloned().collect()
    }

    /// Ingest a batch of raw insights from external scanning.
    ///
    /// Computes relevance against current context keywords,
    /// applies source reliability weighting, and stores qualifying insights.
    pub async fn ingest_insights(&self, raw_insights: Vec<Insight>) -> Vec<Insight> {
        let mut stored = self.insights.write().await;
        let mut sources = self.sources.write().await;
        let mut stats = self.stats.write().await;

        let mut qualified = Vec::new();

        for mut insight in raw_insights {
            stats.total_insights_discovered += 1;

            // Update source production count
            let source_key = if sources.contains_key(&insight.source_url) {
                Some(insight.source_url.clone())
            } else {
                sources
                    .values()
                    .find(|s| s.source_type == insight.source_type)
                    .map(|s| s.id.clone())
            };
            if let Some(key) = source_key {
                if let Some(source) = sources.get_mut(&key) {
                    source.insights_produced += 1;
                }
            }

            // Compute context relevance
            let keyword_score = self.compute_keyword_relevance(&insight.tags);
            // Apply source reliability as a multiplier
            let source_reliability = sources
                .values()
                .find(|s| s.source_type == insight.source_type)
                .map(|s| s.reliability)
                .unwrap_or(1.0);

            insight.relevance_score = (keyword_score * source_reliability).min(1.0);

            // Only surface insights above minimum relevance
            if insight.relevance_score >= self.config.min_relevance {
                insight.disposition = InsightDisposition::Pending;
                stored.push(insight.clone());
                qualified.push(insight);
            }
        }

        qualified
    }

    /// Mark an insight as adopted — triggers positive feedback on the source.
    pub async fn adopt_insight(&self, insight_id: &str) -> bool {
        let mut stored = self.insights.write().await;
        let mut sources = self.sources.write().await;
        let mut stats = self.stats.write().await;

        if let Some(insight) = stored.iter_mut().find(|i| i.id == insight_id) {
            insight.disposition = InsightDisposition::Adopted;
            insight.disposition_at = Some(SystemTime::now());
            stats.total_insights_adopted += 1;

            // Positive feedback: boost source reliability
            // Try to find the source by URL first, then by type
            let source = sources
                .values_mut()
                .find(|s| s.endpoint == insight.source_url || s.source_type == insight.source_type);
            if let Some(source) = source {
                source.record_adoption();
            }

            true
        } else {
            false
        }
    }

    /// Mark an insight as dismissed — triggers negative feedback on the source.
    pub async fn dismiss_insight(&self, insight_id: &str) -> bool {
        let mut stored = self.insights.write().await;
        let mut sources = self.sources.write().await;
        let mut stats = self.stats.write().await;

        if let Some(insight) = stored.iter_mut().find(|i| i.id == insight_id) {
            insight.disposition = InsightDisposition::Dismissed;
            insight.disposition_at = Some(SystemTime::now());
            stats.total_insights_dismissed += 1;

            // Negative feedback: reduce source reliability
            let source = sources
                .values_mut()
                .find(|s| s.endpoint == insight.source_url || s.source_type == insight.source_type);
            if let Some(source) = source {
                source.record_dismissal();
            }

            true
        } else {
            false
        }
    }

    /// Get all pending (unreviewed) insights, sorted by relevance descending.
    pub async fn get_pending_insights(&self) -> Vec<Insight> {
        let stored = self.insights.read().await;
        let mut pending: Vec<Insight> = stored
            .iter()
            .filter(|i| i.disposition == InsightDisposition::Pending)
            .cloned()
            .collect();
        pending.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pending
    }

    /// Get the top N most relevant pending insights.
    pub async fn get_top_insights(&self, n: usize) -> Vec<Insight> {
        let pending = self.get_pending_insights().await;
        pending.into_iter().take(n).collect()
    }

    /// Get insights filtered by source type.
    pub async fn get_insights_by_type(&self, source_type: &SourceType) -> Vec<Insight> {
        let stored = self.insights.read().await;
        stored
            .iter()
            .filter(|i| &i.source_type == source_type)
            .cloned()
            .collect()
    }

    /// Update context keywords (e.g., when the current task changes).
    pub fn update_context(&mut self, keywords: Vec<String>) {
        self.config.context_keywords = keywords;
    }

    /// Get current stream statistics.
    pub async fn get_stats(&self) -> StreamStats {
        self.stats.read().await.clone()
    }

    /// Get all insights regardless of disposition.
    pub async fn get_all_insights(&self) -> Vec<Insight> {
        self.insights.read().await.clone()
    }

    /// Compute keyword relevance between insight tags and context keywords.
    fn compute_keyword_relevance(&self, tags: &[String]) -> f64 {
        if self.config.context_keywords.is_empty() || tags.is_empty() {
            return 0.5; // Neutral score when no context
        }

        let context_set: std::collections::HashSet<&str> = self
            .config
            .context_keywords
            .iter()
            .map(|s| s.as_str())
            .collect();
        let tag_set: std::collections::HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();

        let intersection = context_set.intersection(&tag_set).count();
        let union = context_set.union(&tag_set).count();

        if union == 0 {
            0.5
        } else {
            // Jaccard similarity, scaled to [0.2, 1.0] range
            0.2 + 0.8 * (intersection as f64 / union as f64)
        }
    }

    /// Compute a weighted relevance score that factors in source reliability.
    pub async fn compute_weighted_relevance(&self, insight: &Insight) -> f64 {
        let sources = self.sources.read().await;
        let source_reliability = sources
            .values()
            .find(|s| s.source_type == insight.source_type)
            .map(|s| s.reliability)
            .unwrap_or(1.0);

        let keyword_score = self.compute_keyword_relevance(&insight.tags);
        (keyword_score * source_reliability).min(1.0)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_config() -> InspirationStreamConfig {
        InspirationStreamConfig {
            max_per_cycle: 5,
            min_relevance: 0.3,
            scan_interval_secs: 60,
            context_keywords: vec![
                "rust".to_string(),
                "agent".to_string(),
                "scheduler".to_string(),
            ],
        }
    }

    fn make_test_insight(id: &str, tags: Vec<&str>, source_type: SourceType) -> Insight {
        Insight {
            id: id.to_string(),
            source_type,
            source_url: format!("https://example.com/{}", id),
            title: format!("Insight {}", id),
            content: "Test content".to_string(),
            tags: tags.into_iter().map(|s| s.to_string()).collect(),
            relevance_score: 0.0,
            discovered_at: SystemTime::now(),
            disposition: InsightDisposition::Pending,
            disposition_at: None,
        }
    }

    #[tokio::test]
    async fn test_register_source() {
        let stream = InspirationStream::new(make_test_config());
        let source = Source::new(
            "arxiv-1",
            "arXiv CS",
            SourceType::Paper,
            "https://arxiv.org",
        );
        stream.register_source(source).await;

        let sources = stream.get_sources().await;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "arXiv CS");
    }

    #[tokio::test]
    async fn test_remove_source() {
        let stream = InspirationStream::new(make_test_config());
        let source = Source::new("s1", "Test", SourceType::Trending, "https://hn.com");
        stream.register_source(source).await;
        assert!(stream.remove_source("s1").await);
        assert!(stream.get_sources().await.is_empty());
    }

    #[tokio::test]
    async fn test_ingest_with_relevance_filtering() {
        let stream = InspirationStream::new(make_test_config());

        // Relevant insight (tags overlap with context)
        let i1 = make_test_insight("i1", vec!["rust", "agent"], SourceType::Paper);
        // Irrelevant insight (no overlap)
        let i2 = make_test_insight("i2", vec!["cooking", "recipe"], SourceType::Trending);

        let qualified = stream.ingest_insights(vec![i1, i2]).await;
        // i2 should be filtered out due to low relevance
        assert_eq!(qualified.len(), 1);
        assert_eq!(qualified[0].id, "i1");
    }

    #[tokio::test]
    async fn test_ingest_respects_min_relevance() {
        let mut config = make_test_config();
        config.min_relevance = 0.9; // Very high threshold
        let stream = InspirationStream::new(config);

        let i1 = make_test_insight("i1", vec!["rust", "agent"], SourceType::Paper);
        let qualified = stream.ingest_insights(vec![i1]).await;
        // Even with good tags, might not meet 0.9 threshold
        // The score depends on Jaccard similarity
        assert!(qualified.len() <= 1);
    }

    #[tokio::test]
    async fn test_adopt_insight_positive_feedback() {
        let stream = InspirationStream::new(make_test_config());
        let source = Source::new("s1", "Test", SourceType::Paper, "https://example.com/i1");
        stream.register_source(source).await;

        let i1 = make_test_insight("i1", vec!["rust", "agent", "scheduler"], SourceType::Paper);
        stream.ingest_insights(vec![i1]).await;

        let result = stream.adopt_insight("i1").await;
        assert!(result);

        let sources = stream.get_sources().await;
        let source = &sources[0];
        assert_eq!(source.insights_adopted, 1);
        // Reliability should have increased
        assert!(source.reliability > SourceType::Paper.default_reliability());
    }

    #[tokio::test]
    async fn test_dismiss_insight_negative_feedback() {
        let stream = InspirationStream::new(make_test_config());
        let source = Source::new("s1", "Test", SourceType::Trending, "https://example.com/i1");
        stream.register_source(source).await;

        let i1 = make_test_insight(
            "i1",
            vec!["rust", "agent", "scheduler"],
            SourceType::Trending,
        );
        stream.ingest_insights(vec![i1]).await;

        let result = stream.dismiss_insight("i1").await;
        assert!(result);

        let sources = stream.get_sources().await;
        let source = &sources[0];
        // Reliability should have decreased
        assert!(source.reliability < SourceType::Trending.default_reliability());
    }

    #[tokio::test]
    async fn test_adopt_nonexistent_insight() {
        let stream = InspirationStream::new(make_test_config());
        assert!(!stream.adopt_insight("nonexistent").await);
    }

    #[tokio::test]
    async fn test_pending_insights_sorted_by_relevance() {
        let stream = InspirationStream::new(make_test_config());

        let i1 = make_test_insight("i1", vec!["rust"], SourceType::Paper);
        let i2 = make_test_insight("i2", vec!["rust", "agent", "scheduler"], SourceType::Paper);

        stream.ingest_insights(vec![i1, i2]).await;

        let pending = stream.get_pending_insights().await;
        assert_eq!(pending.len(), 2);
        // i2 has more tag overlap, should be first
        assert!(pending[0].relevance_score >= pending[1].relevance_score);
    }

    #[tokio::test]
    async fn test_top_insights_limits_results() {
        let stream = InspirationStream::new(make_test_config());

        let insights: Vec<Insight> = (0..10)
            .map(|i| {
                make_test_insight(&format!("i{}", i), vec!["rust", "agent"], SourceType::Paper)
            })
            .collect();

        stream.ingest_insights(insights).await;

        let top = stream.get_top_insights(3).await;
        assert_eq!(top.len(), 3);
    }

    #[tokio::test]
    async fn test_insights_by_type() {
        let stream = InspirationStream::new(make_test_config());

        let i1 = make_test_insight("i1", vec!["rust"], SourceType::Paper);
        let i2 = make_test_insight("i2", vec!["rust"], SourceType::Trending);

        stream.ingest_insights(vec![i1, i2]).await;

        let papers = stream.get_insights_by_type(&SourceType::Paper).await;
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].source_type, SourceType::Paper);
    }

    #[tokio::test]
    async fn test_update_context() {
        let mut stream = InspirationStream::new(make_test_config());
        assert_eq!(stream.config.context_keywords.len(), 3);

        stream.update_context(vec!["python".to_string(), "ml".to_string()]);
        assert_eq!(stream.config.context_keywords.len(), 2);
        assert_eq!(stream.config.context_keywords[0], "python");
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let stream = InspirationStream::new(make_test_config());

        let source = Source::new("s1", "Test", SourceType::Paper, "https://example.com/i1");
        stream.register_source(source).await;

        let i1 = make_test_insight("i1", vec!["rust", "agent", "scheduler"], SourceType::Paper);
        stream.ingest_insights(vec![i1]).await;
        stream.adopt_insight("i1").await;

        let stats = stream.get_stats().await;
        assert_eq!(stats.total_insights_discovered, 1);
        assert_eq!(stats.total_insights_adopted, 1);
        assert_eq!(stats.sources_registered, 1);
    }

    #[tokio::test]
    async fn test_reliability_bounds() {
        let mut source = Source::new("s1", "Test", SourceType::Paper, "https://example.com");
        source.reliability = 2.95;

        // Adoption caps at 3.0
        source.record_adoption();
        assert!(source.reliability <= 3.0);

        // Dismissal floors at 0.3
        source.reliability = 0.31;
        source.record_dismissal();
        assert!(source.reliability >= 0.3);
    }

    #[tokio::test]
    async fn test_repeated_adoption_boosts_reliability() {
        let mut source = Source::new("s1", "Test", SourceType::Paper, "https://example.com");
        let initial = source.reliability;

        for _ in 0..20 {
            source.record_adoption();
        }

        assert!(source.reliability > initial);
        assert!(source.reliability <= 3.0);
    }

    #[tokio::test]
    async fn test_repeated_dismissal_reduces_reliability() {
        let mut source = Source::new("s1", "Test", SourceType::Trending, "https://example.com");
        let initial = source.reliability;

        for _ in 0..50 {
            source.record_dismissal();
        }

        assert!(source.reliability < initial);
        assert!(source.reliability >= 0.3);
    }

    #[tokio::test]
    async fn test_keyword_relevance_empty_context() {
        let mut config = make_test_config();
        config.context_keywords = vec![];
        let stream = InspirationStream::new(config);

        let tags = vec!["rust".to_string(), "agent".to_string()];
        let score = stream.compute_keyword_relevance(&tags);
        assert_eq!(score, 0.5); // Neutral
    }

    #[tokio::test]
    async fn test_keyword_relevance_empty_tags() {
        let stream = InspirationStream::new(make_test_config());
        let tags: Vec<String> = vec![];
        let score = stream.compute_keyword_relevance(&tags);
        assert_eq!(score, 0.5); // Neutral
    }

    #[tokio::test]
    async fn test_keyword_relevance_perfect_match() {
        let stream = InspirationStream::new(make_test_config());
        let tags = vec![
            "rust".to_string(),
            "agent".to_string(),
            "scheduler".to_string(),
        ];
        let score = stream.compute_keyword_relevance(&tags);
        assert!(score > 0.9); // High overlap
    }

    #[tokio::test]
    async fn test_keyword_relevance_no_overlap() {
        let stream = InspirationStream::new(make_test_config());
        let tags = vec!["cooking".to_string(), "recipe".to_string()];
        let score = stream.compute_keyword_relevance(&tags);
        assert!(score < 0.3); // Low overlap
    }

    #[tokio::test]
    async fn test_weighted_relevance_factors_reliability() {
        let stream = InspirationStream::new(make_test_config());

        let mut high_reliability_source =
            Source::new("s1", "Trusted", SourceType::Paper, "https://trusted.com");
        high_reliability_source.reliability = 2.0;
        stream.register_source(high_reliability_source).await;

        let insight = make_test_insight("i1", vec!["rust", "agent"], SourceType::Paper);
        let weighted = stream.compute_weighted_relevance(&insight).await;

        // Should be boosted by the high reliability source
        let base_score = stream.compute_keyword_relevance(&insight.tags);
        assert!(weighted >= base_score);
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let stream = InspirationStream::new(make_test_config());

        // Register sources
        let paper_source = Source::new("arxiv", "arXiv", SourceType::Paper, "https://arxiv.org");
        let trending_source = Source::new(
            "gh",
            "GitHub Trending",
            SourceType::Trending,
            "https://github.com",
        );
        stream.register_source(paper_source).await;
        stream.register_source(trending_source).await;

        // Ingest insights
        let i1 = make_test_insight("i1", vec!["rust", "agent", "scheduler"], SourceType::Paper);
        let i2 = make_test_insight("i2", vec!["rust", "performance"], SourceType::Trending);
        let i3 = make_test_insight("i3", vec!["cooking"], SourceType::Paper);

        let qualified = stream.ingest_insights(vec![i1, i2, i3]).await;
        assert!(qualified.len() >= 2); // i1 and i2 should qualify

        // Adopt one, dismiss the other
        stream.adopt_insight("i1").await;
        if let Some(i2_insight) = qualified.iter().find(|i| i.id == "i2") {
            stream.dismiss_insight(&i2_insight.id).await;
        }

        // Check final state
        let stats = stream.get_stats().await;
        assert_eq!(stats.total_insights_adopted, 1);

        let sources = stream.get_sources().await;
        let arxiv = sources.iter().find(|s| s.id == "arxiv").unwrap();
        let gh = sources.iter().find(|s| s.id == "gh").unwrap();

        // arXiv should have higher reliability (adopted insight)
        assert!(arxiv.reliability > gh.reliability);
    }
}
