//! Reddit provider.
//!
//! Uses Reddit's public JSON API (append `.json` to any Reddit URL).
//! No authentication required for public subreddits.

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use reqwest::Client;
use tracing::debug;

use crate::error::AggregatorError;
use crate::models::{AggregatedFeed, AggregatedPost, FetchQuery, Platform, PostAuthor};
use crate::traits::PlatformProvider;

const REDDIT_BASE: &str = "https://www.reddit.com";

/// Reddit data provider.
///
/// Fetches posts from subreddits or search results using Reddit's public JSON API.
pub struct RedditProvider {
    client: Client,
}

impl RedditProvider {
    /// Create a new Reddit provider.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("kias-data-aggregator/0.1 (by /u/kias_bot)")
                .build()
                .expect("valid HTTP client config"),
        }
    }

    /// Build the subreddit URL path.
    fn build_url(query: &FetchQuery) -> String {
        let sort = query.sort.as_deref().unwrap_or("hot");
        let limit = query.limit.unwrap_or(25).min(100);

        // Detect if it's a search query or a subreddit listing.
        if query.query.contains(' ') || query.query.starts_with("search:") {
            let search_term = query.query.strip_prefix("search:").unwrap_or(&query.query);
            let encoded = urlencoding::encode(search_term);
            return format!(
                "{REDDIT_BASE}/search.json?q={encoded}&sort={sort}&limit={limit}&restrict_sr=1"
            );
        }

        // Treat as subreddit name.
        let subreddit = query.query.trim_start_matches("r/").trim_start_matches('/');
        format!("{REDDIT_BASE}/r/{subreddit}/{sort}.json?limit={limit}&raw_json=1")
    }

    /// Convert a Reddit post listing JSON into an AggregatedPost.
    fn listing_to_post(data: &serde_json::Value) -> Option<AggregatedPost> {
        let name = data.get("name")?.as_str()?;
        let id = data.get("id")?.as_str()?;
        let title = data.get("title").and_then(|v| v.as_str()).map(String::from);
        let selftext = data
            .get("selftext")
            .and_then(|v| v.as_str())
            .map(String::from);
        let url = data.get("url").and_then(|v| v.as_str()).map(String::from);
        let author = data
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("[deleted]");
        let score = data.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
        let num_comments = data
            .get("num_comments")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let created_utc = data
            .get("created_utc")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as i64;
        let subreddit = data
            .get("subreddit")
            .and_then(|v| v.as_str())
            .map(String::from);
        let link_flair_text = data
            .get("link_flair_text")
            .and_then(|v| v.as_str())
            .map(String::from);

        let created_at = Utc.timestamp_opt(created_utc, 0).single()?;

        let mut tags: Vec<String> = Vec::new();
        if let Some(sub) = subreddit {
            tags.push(sub);
        }
        if let Some(flair) = link_flair_text {
            tags.push(flair);
        }

        // Filter out [removed] and [deleted] content.
        if selftext.as_deref() == Some("[removed]") || selftext.as_deref() == Some("[deleted]") {
            return None;
        }

        Some(AggregatedPost {
            id: format!("reddit:{name}"),
            native_id: id.to_string(),
            platform: Platform::Reddit,
            title,
            body: selftext,
            url,
            author: PostAuthor {
                id: author.to_string(),
                username: author.to_string(),
                display_name: None,
                avatar_url: None,
                reputation: None,
                platform: Platform::Reddit,
            },
            score,
            comment_count: num_comments,
            created_at,
            tags,
            language: None,
            raw: Some(data.clone()),
        })
    }
}

impl Default for RedditProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformProvider for RedditProvider {
    fn platform(&self) -> Platform {
        Platform::Reddit
    }

    fn is_configured(&self) -> bool {
        // Reddit public JSON API requires no auth.
        true
    }

    async fn fetch(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError> {
        let mut url = Self::build_url(query);

        // Add pagination cursor (Reddit uses `after` parameter).
        if let Some(ref cursor) = query.cursor {
            url.push_str(&format!("&after={cursor}"));
        }

        debug!(url = %url, "Fetching Reddit data");

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AggregatorError::RateLimited("reddit".into(), 60));
        }

        let body: serde_json::Value = resp.json().await?;

        let data = body.get("data").ok_or_else(|| {
            AggregatorError::ParseError("Missing 'data' field in Reddit response".into())
        })?;

        let children = data
            .get("children")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let after = data.get("after").and_then(|v| v.as_str()).map(String::from);

        let posts: Vec<AggregatedPost> = children
            .iter()
            .filter_map(|child| {
                let child_data = child.get("data")?;
                Self::listing_to_post(child_data)
            })
            .collect();

        Ok(AggregatedFeed {
            platform: Platform::Reddit,
            query: query.query.clone(),
            posts,
            next_cursor: after,
            fetched_at: Utc::now(),
            total: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_metadata() {
        let provider = RedditProvider::new();
        assert_eq!(provider.platform(), Platform::Reddit);
        assert!(provider.is_configured());
    }

    #[test]
    fn test_build_url_subreddit() {
        let query = FetchQuery::new("rust");
        let url = RedditProvider::build_url(&query);
        assert!(url.contains("/r/rust/hot.json"));
        assert!(url.contains("limit=25"));
    }

    #[test]
    fn test_build_url_search() {
        let query = FetchQuery::new("async programming").with_sort("new");
        let url = RedditProvider::build_url(&query);
        assert!(url.contains("/search.json"));
        assert!(url.contains("sort=new"));
    }

    #[test]
    fn test_listing_to_post() {
        let data = serde_json::json!({
            "name": "t3_abc123",
            "id": "abc123",
            "title": "Test Post Title",
            "selftext": "This is the body of the test post.",
            "url": "https://example.com",
            "author": "testuser",
            "score": 150,
            "num_comments": 42,
            "created_utc": 1700000000.0,
            "subreddit": "rust",
            "link_flair_text": "Discussion"
        });
        let post = RedditProvider::listing_to_post(&data).unwrap();
        assert_eq!(post.id, "reddit:t3_abc123");
        assert_eq!(post.platform, Platform::Reddit);
        assert_eq!(post.title.unwrap(), "Test Post Title");
        assert_eq!(post.score, 150);
        assert_eq!(post.comment_count, 42);
        assert!(post.tags.contains(&"rust".to_string()));
        assert!(post.tags.contains(&"Discussion".to_string()));
    }

    #[test]
    fn test_listing_to_post_removed() {
        let data = serde_json::json!({
            "name": "t3_def456",
            "id": "def456",
            "title": "Removed Post",
            "selftext": "[removed]",
            "url": "https://example.com",
            "author": "testuser",
            "score": 0,
            "num_comments": 0,
            "created_utc": 1700000000.0,
            "subreddit": "test"
        });
        assert!(RedditProvider::listing_to_post(&data).is_none());
    }
}
