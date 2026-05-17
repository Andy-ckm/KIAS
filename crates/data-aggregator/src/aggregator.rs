//! Core aggregation engine.
//!
//! Orchestrates fetching from multiple platform providers in parallel,
//! with deduplication and unified result aggregation.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use tracing::{debug, info, warn};

use crate::error::AggregatorError;
use crate::models::{AggregatedFeed, FetchQuery, Platform};
use crate::traits::PlatformProvider;

/// Configuration for the data aggregator.
#[derive(Debug, Clone)]
pub struct AggregatorConfig {
    /// Default number of posts per platform.
    pub default_limit: u32,
    /// Timeout per platform request in seconds.
    pub request_timeout_secs: u64,
    /// Whether to fetch from all platforms in parallel.
    pub parallel: bool,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            default_limit: 20,
            request_timeout_secs: 30,
            parallel: true,
        }
    }
}

/// The core data aggregator that orchestrates multi-platform fetching.
pub struct DataAggregator {
    providers: HashMap<Platform, Arc<dyn PlatformProvider>>,
    config: AggregatorConfig,
}

impl DataAggregator {
    /// Create a new aggregator with default configuration.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            config: AggregatorConfig::default(),
        }
    }

    /// Create a new aggregator with custom configuration.
    pub fn with_config(config: AggregatorConfig) -> Self {
        Self {
            providers: HashMap::new(),
            config,
        }
    }

    /// Register a platform provider.
    pub fn register(&mut self, provider: Arc<dyn PlatformProvider>) {
        let platform = provider.platform();
        info!(platform = %platform, "Registering platform provider");
        self.providers.insert(platform, provider);
    }

    /// Get the list of registered platforms.
    pub fn platforms(&self) -> Vec<Platform> {
        self.providers.keys().copied().collect()
    }

    /// Check which providers are properly configured.
    pub fn configured_platforms(&self) -> Vec<Platform> {
        self.providers
            .iter()
            .filter(|(_, p)| p.is_configured())
            .map(|(k, _)| *k)
            .collect()
    }

    /// Fetch from a single platform.
    pub async fn fetch_platform(
        &self,
        platform: Platform,
        query: &FetchQuery,
    ) -> Result<AggregatedFeed, AggregatorError> {
        let provider = self
            .providers
            .get(&platform)
            .ok_or_else(|| AggregatorError::UnsupportedPlatform(platform.to_string()))?;

        if !provider.is_configured() {
            return Err(AggregatorError::ConfigError(format!(
                "{platform} provider is not configured"
            )));
        }

        // Apply default limit if not set.
        let mut q = query.clone();
        if q.limit.is_none() {
            q.limit = Some(self.config.default_limit);
        }

        debug!(platform = %platform, query = %q.query, "Fetching from platform");
        provider.fetch(&q).await
    }

    /// Fetch from all registered platforms in parallel.
    ///
    /// Returns a map of platform -> feed. Platforms that fail are logged
    /// but do not cause the entire operation to fail.
    pub async fn fetch_all(
        &self,
        query: &FetchQuery,
    ) -> HashMap<Platform, Result<AggregatedFeed, AggregatorError>> {
        let mut q = query.clone();
        if q.limit.is_none() {
            q.limit = Some(self.config.default_limit);
        }

        if self.config.parallel {
            self.fetch_all_parallel(&q).await
        } else {
            self.fetch_all_sequential(&q).await
        }
    }

    /// Fetch from all platforms in parallel.
    async fn fetch_all_parallel(
        &self,
        query: &FetchQuery,
    ) -> HashMap<Platform, Result<AggregatedFeed, AggregatorError>> {
        let futures: Vec<_> = self
            .providers
            .iter()
            .filter(|(_, p)| p.is_configured())
            .map(|(platform, provider)| {
                let p = provider.clone();
                let q = query.clone();
                async move {
                    let result = p.fetch(&q).await;
                    (*platform, result)
                }
            })
            .collect();

        let results = join_all(futures).await;
        results.into_iter().collect()
    }

    /// Fetch from all platforms sequentially.
    async fn fetch_all_sequential(
        &self,
        query: &FetchQuery,
    ) -> HashMap<Platform, Result<AggregatedFeed, AggregatorError>> {
        let mut results = HashMap::new();

        for (platform, provider) in &self.providers {
            if !provider.is_configured() {
                debug!(platform = %platform, "Skipping unconfigured platform");
                continue;
            }
            let result = provider.fetch(query).await;
            results.insert(*platform, result);
        }

        results
    }

    /// Fetch from all platforms and merge into a single sorted feed.
    pub async fn fetch_merged(
        &self,
        query: &FetchQuery,
    ) -> Result<AggregatedFeed, AggregatorError> {
        let results = self.fetch_all(query).await;

        let mut all_posts = Vec::new();
        let mut errors = Vec::new();
        let mut total = 0u64;

        for (platform, result) in &results {
            match result {
                Ok(feed) => {
                    total += feed.total.unwrap_or(feed.posts.len() as u64);
                    all_posts.extend(feed.posts.clone());
                }
                Err(e) => {
                    warn!(platform = %platform, error = %e, "Failed to fetch from platform");
                    errors.push(format!("{platform}: {e}"));
                }
            }
        }

        // Sort by score descending.
        all_posts.sort_by_key(|b| std::cmp::Reverse(b.score));

        // Deduplicate by URL if present.
        let mut seen_urls = std::collections::HashSet::new();
        all_posts.retain(|post| {
            if let Some(ref url) = post.url {
                seen_urls.insert(url.clone())
            } else {
                true
            }
        });

        if !errors.is_empty() {
            info!(
                errors = %errors.join(", "),
                "Some platforms failed during merged fetch"
            );
        }

        Ok(AggregatedFeed {
            platform: Platform::HackerNews, // Placeholder — merged has no single platform.
            query: query.query.clone(),
            posts: all_posts,
            next_cursor: None,
            fetched_at: Utc::now(),
            total: Some(total),
        })
    }
}

impl Default for DataAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AggregatedFeed, Platform};
    use async_trait::async_trait;

    /// A mock provider for testing.
    struct MockProvider {
        platform: Platform,
        configured: bool,
    }

    #[async_trait]
    impl PlatformProvider for MockProvider {
        fn platform(&self) -> Platform {
            self.platform
        }

        fn is_configured(&self) -> bool {
            self.configured
        }

        async fn fetch(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError> {
            Ok(AggregatedFeed {
                platform: self.platform,
                query: query.query.clone(),
                posts: vec![],
                next_cursor: None,
                fetched_at: Utc::now(),
                total: Some(0),
            })
        }
    }

    #[tokio::test]
    async fn test_aggregator_register_and_fetch() {
        let mut agg = DataAggregator::new();
        agg.register(Arc::new(MockProvider {
            platform: Platform::HackerNews,
            configured: true,
        }));

        assert_eq!(agg.platforms().len(), 1);
        assert_eq!(agg.configured_platforms().len(), 1);

        let result = agg
            .fetch_platform(Platform::HackerNews, &FetchQuery::new("test"))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_aggregator_unsupported_platform() {
        let agg = DataAggregator::new();
        let result = agg
            .fetch_platform(Platform::Reddit, &FetchQuery::new("test"))
            .await;
        assert!(matches!(
            result,
            Err(AggregatorError::UnsupportedPlatform(_))
        ));
    }

    #[tokio::test]
    async fn test_aggregator_unconfigured() {
        let mut agg = DataAggregator::new();
        agg.register(Arc::new(MockProvider {
            platform: Platform::X,
            configured: false,
        }));

        let result = agg
            .fetch_platform(Platform::X, &FetchQuery::new("test"))
            .await;
        assert!(matches!(result, Err(AggregatorError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_fetch_all_parallel() {
        let mut agg = DataAggregator::new();
        agg.register(Arc::new(MockProvider {
            platform: Platform::HackerNews,
            configured: true,
        }));
        agg.register(Arc::new(MockProvider {
            platform: Platform::Reddit,
            configured: true,
        }));
        agg.register(Arc::new(MockProvider {
            platform: Platform::X,
            configured: false, // Should be skipped.
        }));

        let results = agg.fetch_all(&FetchQuery::new("rust")).await;
        assert_eq!(results.len(), 2);
        assert!(results.contains_key(&Platform::HackerNews));
        assert!(results.contains_key(&Platform::Reddit));
    }
}
