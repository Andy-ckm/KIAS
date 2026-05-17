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
