//! X (Twitter) provider.
//!
//! Uses the X API v2 (https://api.twitter.com/2/).
//! Requires a Bearer Token set via `KIAS_X_BEARER_TOKEN` env var.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use tracing::debug;

use crate::error::AggregatorError;
use crate::models::{AggregatedFeed, AggregatedPost, FetchQuery, Platform, PostAuthor};
use crate::traits::PlatformProvider;

const X_API_BASE: &str = "https://api.twitter.com/2";

/// X (Twitter) data provider.
///
/// Requires a Bearer Token for authentication. Set `KIAS_X_BEARER_TOKEN`
/// in the environment or pass it to [`XProvider::new`].
pub struct XProvider {
    client: Client,
    bearer_token: Option<String>,
}

impl XProvider {
    /// Create a new X provider, reading the bearer token from the environment.
    pub fn new() -> Self {
        let bearer_token = std::env::var("KIAS_X_BEARER_TOKEN")
            .or_else(|_| std::env::var("X_BEARER_TOKEN"))
            .ok();
        Self {
            client: Client::builder()
                .user_agent("kias-data-aggregator/0.1")
                .build()
                .expect("Failed to build HTTP client"),
            bearer_token,
        }
    }

    /// Create a new X provider with an explicit bearer token.
    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("kias-data-aggregator/0.1")
                .build()
                .expect("Failed to build HTTP client"),
            bearer_token: Some(token.into()),
        }
    }

    /// Build the search URL.
    fn build_search_url(query: &FetchQuery) -> String {
        let encoded = urlencoding::encode(&query.query);
        let max_results = query.limit.unwrap_or(10).min(100);
        let mut url = format!(
            "{X_API_BASE}/tweets/search/recent?query={encoded}&max_results={max_results}&tweet.fields=created_at,public_metrics,lang,author_id&expansions=author_id&user.fields=username,name,profile_image_url,public_metrics"
        );

        if let Some(ref cursor) = query.cursor {
            url.push_str(&format!("&next_token={cursor}"));
        }

        url
    }

    /// Convert a raw X API tweet into an AggregatedPost.
    fn tweet_to_post(
        tweet: &serde_json::Value,
        users: &[serde_json::Value],
    ) -> Option<AggregatedPost> {
        let id = tweet.get("id")?.as_str()?;
        let text = tweet.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let author_id = tweet
            .get("author_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let created_at_str = tweet
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(created_at_str)
            .ok()?
            .with_timezone(&Utc);

        let metrics = tweet.get("public_metrics");
        let retweet_count = metrics
            .and_then(|m| m.get("retweet_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let like_count = metrics
            .and_then(|m| m.get("like_count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let reply_count = metrics
            .and_then(|m| m.get("reply_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let lang = tweet.get("lang").and_then(|v| v.as_str()).map(String::from);

        // Find the matching user.
        let user = users.iter().find(|u| {
            u.get("id")
                .and_then(|v| v.as_str())
                .map(|uid| uid == author_id)
                .unwrap_or(false)
        });

        let username = user
            .and_then(|u| u.get("username"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let display_name = user
            .and_then(|u| u.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let avatar_url = user
            .and_then(|u| u.get("profile_image_url"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let followers = user
            .and_then(|u| u.get("public_metrics"))
            .and_then(|m| m.get("followers_count"))
            .and_then(|v| v.as_i64());

        Some(AggregatedPost {
            id: format!("x:{id}"),
            native_id: id.to_string(),
            platform: Platform::X,
            title: None,
            body: Some(text.to_string()),
            url: Some(format!("https://x.com/{username}/status/{id}")),
            author: PostAuthor {
                id: author_id.to_string(),
                username: username.to_string(),
                display_name,
                avatar_url,
                reputation: followers,
                platform: Platform::X,
            },
            score: like_count + (retweet_count as i64),
            comment_count: reply_count,
            created_at,
            tags: vec![],
            language: lang,
            raw: Some(tweet.clone()),
        })
    }
}

impl Default for XProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformProvider for XProvider {
    fn platform(&self) -> Platform {
        Platform::X
    }

    fn is_configured(&self) -> bool {
        self.bearer_token.is_some()
    }

    async fn fetch(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError> {
        let token = self
            .bearer_token
            .as_ref()
            .ok_or_else(|| AggregatorError::AuthError("X bearer token not configured".into()))?;

        let url = Self::build_search_url(query);
        debug!(url = %url, "Fetching X data");

        let resp = self.client.get(&url).bearer_auth(token).send().await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AggregatorError::RateLimited("x".into(), 900));
        }

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AggregatorError::AuthError(
                "X API authentication failed".into(),
            ));
        }

        let body: serde_json::Value = resp.json().await?;

        let tweets = body
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let users = body
            .get("includes")
            .and_then(|inc| inc.get("users"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let next_token = body
            .get("meta")
            .and_then(|m| m.get("next_token"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let posts: Vec<AggregatedPost> = tweets
            .iter()
            .filter_map(|tweet| Self::tweet_to_post(tweet, &users))
            .collect();

        let result_count = body
            .get("meta")
            .and_then(|m| m.get("result_count"))
            .and_then(|v| v.as_u64());

        Ok(AggregatedFeed {
            platform: Platform::X,
            query: query.query.clone(),
            posts,
            next_cursor: next_token,
            fetched_at: Utc::now(),
            total: result_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_metadata() {
        let provider = XProvider::with_token("test_token");
        assert_eq!(provider.platform(), Platform::X);
        assert!(provider.is_configured());
    }

    #[test]
    fn test_provider_no_token() {
        let provider = XProvider::new();
        // Without env var set, should not be configured.
        // (This test only works when the env var is not set.)
        if std::env::var("KIAS_X_BEARER_TOKEN").is_err() && std::env::var("X_BEARER_TOKEN").is_err()
        {
            assert!(!provider.is_configured());
        }
    }

    #[test]
    fn test_build_search_url() {
        let query = FetchQuery::new("rust lang").with_limit(50);
        let url = XProvider::build_search_url(&query);
        assert!(url.contains("query=rust%20lang"));
        assert!(url.contains("max_results=50"));
        assert!(url.contains("tweet.fields="));
    }

    #[test]
    fn test_tweet_to_post() {
        let tweet = serde_json::json!({
            "id": "1234567890",
            "text": "Hello from the test!",
            "author_id": "user1",
            "created_at": "2024-01-15T10:30:00.000Z",
            "lang": "en",
            "public_metrics": {
                "retweet_count": 10,
                "like_count": 50,
                "reply_count": 5,
                "quote_count": 2
            }
        });
        let users = vec![serde_json::json!({
            "id": "user1",
            "username": "testuser",
            "name": "Test User",
            "profile_image_url": "https://example.com/pic.jpg",
            "public_metrics": {
                "followers_count": 1000
            }
        })];

        let post = XProvider::tweet_to_post(&tweet, &users).unwrap();
        assert_eq!(post.id, "x:1234567890");
        assert_eq!(post.platform, Platform::X);
        assert_eq!(post.body.unwrap(), "Hello from the test!");
        assert_eq!(post.author.username, "testuser");
        assert_eq!(post.author.display_name.unwrap(), "Test User");
        assert_eq!(post.author.reputation.unwrap(), 1000);
        assert_eq!(post.score, 60); // 50 likes + 10 retweets
        assert_eq!(post.comment_count, 5);
        assert_eq!(post.language.unwrap(), "en");
        assert!(post.url.unwrap().contains("testuser"));
    }

    #[test]
    fn test_tweet_to_post_no_user() {
        let tweet = serde_json::json!({
            "id": "999",
            "text": "No user info",
            "author_id": "missing_user",
            "created_at": "2024-01-15T10:30:00.000Z",
            "public_metrics": {
                "retweet_count": 0,
                "like_count": 0,
                "reply_count": 0
            }
        });

        let post = XProvider::tweet_to_post(&tweet, &[]).unwrap();
        assert_eq!(post.author.username, "unknown");
    }
}
