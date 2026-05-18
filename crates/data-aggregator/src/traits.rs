//! Trait definitions for platform providers.

use async_trait::async_trait;

use crate::error::AggregatorError;
use crate::models::{AggregatedFeed, FetchQuery, Platform};

/// Trait for platform-specific data providers.
///
/// Each provider handles authentication, HTTP requests, response parsing,
/// and normalization into the unified [`AggregatedPost`] format.
///
/// # Implementation guidelines
///
/// - Providers should be stateless or hold only configuration.
/// - All I/O must be async.
/// - Rate limiting should be handled gracefully (return [`AggregatorError::RateLimited`]).
/// - Use `tracing` for logging, not `println!`.
#[async_trait]
pub trait PlatformProvider: Send + Sync {
    /// Which platform this provider handles.
    fn platform(&self) -> Platform;

    /// Whether this provider is properly configured (e.g. has API key).
    fn is_configured(&self) -> bool;

    /// Fetch content matching the given query.
    async fn fetch(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError>;

    /// Fetch the next page using a cursor from a previous response.
    async fn fetch_next(&self, feed: &AggregatedFeed) -> Result<AggregatedFeed, AggregatorError> {
        match &feed.next_cursor {
            Some(cursor) => {
                let q = FetchQuery {
                    query: feed.query.clone(),
                    limit: None,
                    cursor: Some(cursor.clone()),
                    sort: None,
                    time_window: None,
                };
                self.fetch(&q).await
            }
            None => Ok(AggregatedFeed {
                platform: feed.platform,
                query: feed.query.clone(),
                posts: vec![],
                next_cursor: None,
                fetched_at: chrono::Utc::now(),
                total: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AggregatedFeed, AggregatedPost, Platform, PostAuthor};
    use async_trait::async_trait;
    use chrono::Utc;

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
                posts: vec![AggregatedPost {
                    id: "hackernews:test-1".to_string(),
                    native_id: "test-1".to_string(),
                    platform: self.platform,
                    title: Some("Test Post".to_string()),
                    body: Some("Test content".to_string()),
                    url: Some("https://example.com/1".to_string()),
                    author: PostAuthor {
                        id: "user-1".to_string(),
                        username: "testuser".to_string(),
                        display_name: Some("Test User".to_string()),
                        avatar_url: None,
                        reputation: Some(1000),
                        platform: self.platform,
                    },
                    score: 100,
                    comment_count: 10,
                    created_at: Utc::now(),
                    tags: vec!["rust".to_string()],
                    language: Some("en".to_string()),
                    raw: None,
                }],
                next_cursor: Some("cursor-123".to_string()),
                fetched_at: Utc::now(),
                total: Some(1),
            })
        }
    }

    #[tokio::test]
    async fn test_fetch_next_with_cursor() {
        let provider = MockProvider {
            platform: Platform::HackerNews,
            configured: true,
        };

        let feed = AggregatedFeed {
            platform: Platform::HackerNews,
            query: "rust".to_string(),
            posts: vec![],
            next_cursor: Some("cursor-456".to_string()),
            fetched_at: Utc::now(),
            total: Some(0),
        };

        let result = provider.fetch_next(&feed).await.unwrap();
        assert_eq!(result.posts.len(), 1);
        assert_eq!(result.posts[0].id, "hackernews:test-1");
    }

    #[tokio::test]
    async fn test_fetch_next_without_cursor() {
        let provider = MockProvider {
            platform: Platform::HackerNews,
            configured: true,
        };

        let feed = AggregatedFeed {
            platform: Platform::HackerNews,
            query: "rust".to_string(),
            posts: vec![],
            next_cursor: None,
            fetched_at: Utc::now(),
            total: Some(0),
        };

        let result = provider.fetch_next(&feed).await.unwrap();
        assert!(result.posts.is_empty());
        assert!(result.next_cursor.is_none());
    }

    #[test]
    fn test_platform_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockProvider>();
    }
}
