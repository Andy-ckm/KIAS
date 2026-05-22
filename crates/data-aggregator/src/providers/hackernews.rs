//! Hacker News provider.
//!
//! Uses the official HN Firebase API (https://hacker-news.firebaseio.com/v0/).
//! No authentication required.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use reqwest::Client;
use tracing::{debug, warn};

use crate::error::AggregatorError;
use crate::models::{AggregatedFeed, AggregatedPost, FetchQuery, Platform, PostAuthor};
use crate::traits::PlatformProvider;

const HN_API_BASE: &str = "https://hacker-news.firebaseio.com/v0";
const HN_ALGOLIA_API: &str = "https://hn.algolia.com/api/v1";

/// Hacker News data provider.
///
/// Uses two APIs:
/// - Firebase API for top/new/best stories
/// - Algolia API for search queries
pub struct HackerNewsProvider {
    client: Client,
}

impl HackerNewsProvider {
    /// Create a new Hacker News provider.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("kias-data-aggregator/0.1")
                .build()
                .expect("valid HTTP client config"),
        }
    }

    /// Fetch story IDs from the Firebase API.
    async fn fetch_story_ids(&self, list: &str) -> Result<Vec<u64>, AggregatorError> {
        let url = format!("{HN_API_BASE}/{list}stories.json");
        let resp = self.client.get(&url).send().await?;
        let ids: Vec<u64> = resp.json().await?;
        Ok(ids)
    }

    /// Fetch a single item from the Firebase API.
    async fn fetch_item(&self, id: u64) -> Result<serde_json::Value, AggregatorError> {
        let url = format!("{HN_API_BASE}/item/{id}.json");
        let resp = self.client.get(&url).send().await?;
        let item: serde_json::Value = resp.json().await?;
        Ok(item)
    }

    /// Convert a raw HN JSON item into an AggregatedPost.
    fn item_to_post(&self, item: &serde_json::Value) -> Option<AggregatedPost> {
        let id = item.get("id")?.as_u64()?;
        let item_type = item.get("type")?.as_str()?;

        // Only process stories and comments.
        if item_type != "story" && item_type != "comment" {
            return None;
        }

        let title = item.get("title").and_then(|v| v.as_str()).map(String::from);
        let body = item.get("text").and_then(|v| v.as_str()).map(String::from);
        let url = item.get("url").and_then(|v| v.as_str()).map(String::from);
        let by = item.get("by").and_then(|v| v.as_str()).unwrap_or("unknown");
        let score = item.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
        let descendants = item
            .get("descendants")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let time = item.get("time").and_then(|v| v.as_i64()).unwrap_or(0);

        let created_at: DateTime<Utc> = Utc.timestamp_opt(time, 0).single()?;

        let tags = if let Some(parent_title) = title.as_ref() {
            vec![parent_title.clone()]
        } else {
            vec![]
        };

        Some(AggregatedPost {
            id: format!("hackernews:{id}"),
            native_id: id.to_string(),
            platform: Platform::HackerNews,
            title,
            body,
            url,
            author: PostAuthor {
                id: by.to_string(),
                username: by.to_string(),
                display_name: None,
                avatar_url: None,
                reputation: None,
                platform: Platform::HackerNews,
            },
            score,
            comment_count: descendants,
            created_at,
            tags,
            language: Some("en".to_string()),
            raw: Some(item.clone()),
        })
    }

    /// Fetch using Algolia search API.
    async fn search(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError> {
        let limit = query.limit.unwrap_or(20).min(100);
        let page = query
            .cursor
            .as_ref()
            .and_then(|c| c.parse::<u32>().ok())
            .unwrap_or(0);

        let sort = query.sort.as_deref().unwrap_or("search");
        let endpoint = match sort {
            "search" | "relevant" => "search",
            "date" | "new" => "search_by_date",
            _ => "search",
        };

        let mut params = vec![
            ("query", query.query.clone()),
            ("hitsPerPage", limit.to_string()),
            ("page", page.to_string()),
            ("tags", "story".to_string()),
        ];

        if let Some(ref tw) = query.time_window {
            let ts = match tw.as_str() {
                "hour" => chrono::Utc::now().timestamp() - 3600,
                "day" => chrono::Utc::now().timestamp() - 86400,
                "week" => chrono::Utc::now().timestamp() - 604800,
                "month" => chrono::Utc::now().timestamp() - 2592000,
                _ => chrono::Utc::now().timestamp() - 86400,
            };
            params.push(("numericFilters", format!("created_at_i>{ts}")));
        }

        let url = format!("{HN_ALGOLIA_API}/{endpoint}");
        debug!(url = %url, query = %query.query, "Fetching HN via Algolia");

        let resp = self.client.get(&url).query(&params).send().await?;
        let body: serde_json::Value = resp.json().await?;

        let hits = body
            .get("hits")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let nb_pages = body.get("nbPages").and_then(|v| v.as_u64()).unwrap_or(1);

        let posts: Vec<AggregatedPost> = hits
            .iter()
            .filter_map(|hit| self.algolia_hit_to_post(hit))
            .collect();

        let next_cursor = if page as u64 + 1 < nb_pages {
            Some((page + 1).to_string())
        } else {
            None
        };

        Ok(AggregatedFeed {
            platform: Platform::HackerNews,
            query: query.query.clone(),
            posts,
            next_cursor,
            fetched_at: Utc::now(),
            total: body.get("nbHits").and_then(|v| v.as_u64()),
        })
    }

    /// Convert an Algolia hit to an AggregatedPost.
    fn algolia_hit_to_post(&self, hit: &serde_json::Value) -> Option<AggregatedPost> {
        let object_id = hit.get("objectID")?.as_str()?;
        let title = hit.get("title").and_then(|v| v.as_str()).map(String::from);
        let url = hit.get("url").and_then(|v| v.as_str()).map(String::from);
        let story_text = hit
            .get("story_text")
            .and_then(|v| v.as_str())
            .map(String::from);
        let author = hit
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let points = hit.get("points").and_then(|v| v.as_i64()).unwrap_or(0);
        let num_comments = hit
            .get("num_comments")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let created_at_str = hit.get("created_at").and_then(|v| v.as_str()).unwrap_or("");

        let created_at =
            chrono::NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%dT%H:%M:%S%.fZ")
                .ok()
                .map(|naive| naive.and_utc())
                .unwrap_or_else(Utc::now);

        Some(AggregatedPost {
            id: format!("hackernews:{object_id}"),
            native_id: object_id.to_string(),
            platform: Platform::HackerNews,
            title,
            body: story_text,
            url,
            author: PostAuthor {
                id: author.to_string(),
                username: author.to_string(),
                display_name: None,
                avatar_url: None,
                reputation: None,
                platform: Platform::HackerNews,
            },
            score: points,
            comment_count: num_comments,
            created_at,
            tags: vec![],
            language: Some("en".to_string()),
            raw: Some(hit.clone()),
        })
    }
}

impl Default for HackerNewsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformProvider for HackerNewsProvider {
    fn platform(&self) -> Platform {
        Platform::HackerNews
    }

    fn is_configured(&self) -> bool {
        // HN API is public, always available.
        true
    }

    async fn fetch(&self, query: &FetchQuery) -> Result<AggregatedFeed, AggregatorError> {
        // If query is empty or matches a list type, use Firebase API.
        let list = match query.query.to_lowercase().as_str() {
            "top" | "hot" => Some("top"),
            "new" | "latest" => Some("new"),
            "best" => Some("best"),
            _ => None,
        };

        if let Some(list_name) = list {
            return self.fetch_list(query, list_name).await;
        }

        // Otherwise, use Algolia search.
        self.search(query).await
    }
}

impl HackerNewsProvider {
    /// Fetch stories from a named list (top/new/best).
    async fn fetch_list(
        &self,
        query: &FetchQuery,
        list: &str,
    ) -> Result<AggregatedFeed, AggregatorError> {
        let ids = self.fetch_story_ids(list).await?;
        let limit = query.limit.unwrap_or(20).min(100) as usize;

        let mut posts = Vec::with_capacity(limit);
        for &id in ids.iter().take(limit) {
            match self.fetch_item(id).await {
                Ok(item) => {
                    if let Some(post) = self.item_to_post(&item) {
                        posts.push(post);
                    }
                }
                Err(e) => {
                    warn!(item_id = id, error = %e, "Failed to fetch HN item");
                }
            }
        }

        Ok(AggregatedFeed {
            platform: Platform::HackerNews,
            query: query.query.clone(),
            posts,
            next_cursor: None,
            fetched_at: Utc::now(),
            total: Some(ids.len() as u64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_metadata() {
        let provider = HackerNewsProvider::new();
        assert_eq!(provider.platform(), Platform::HackerNews);
        assert!(provider.is_configured());
    }

    #[test]
    fn test_item_to_post_story() {
        let provider = HackerNewsProvider::new();
        let item = serde_json::json!({
            "id": 12345,
            "type": "story",
            "title": "Test Story",
            "url": "https://example.com",
            "by": "testuser",
            "score": 42,
            "descendants": 10,
            "time": 1700000000
        });
        let post = provider.item_to_post(&item).unwrap();
        assert_eq!(post.id, "hackernews:12345");
        assert_eq!(post.platform, Platform::HackerNews);
        assert_eq!(post.title.unwrap(), "Test Story");
        assert_eq!(post.score, 42);
        assert_eq!(post.comment_count, 10);
        assert_eq!(post.author.username, "testuser");
    }

    #[test]
    fn test_item_to_post_comment() {
        let provider = HackerNewsProvider::new();
        let item = serde_json::json!({
            "id": 99999,
            "type": "comment",
            "text": "Great post!",
            "by": "commenter",
            "time": 1700000000
        });
        let post = provider.item_to_post(&item).unwrap();
        assert_eq!(post.body.unwrap(), "Great post!");
        assert!(post.title.is_none());
    }

    #[test]
    fn test_item_to_post_job_skipped() {
        let provider = HackerNewsProvider::new();
        let item = serde_json::json!({
            "id": 11111,
            "type": "job",
            "title": "Hiring",
            "time": 1700000000
        });
        assert!(provider.item_to_post(&item).is_none());
    }

    #[test]
    fn test_algolia_hit_conversion() {
        let provider = HackerNewsProvider::new();
        let hit = serde_json::json!({
            "objectID": "99999",
            "title": "Test Algolia Hit",
            "url": "https://example.com",
            "author": "algolia_user",
            "points": 100,
            "num_comments": 25,
            "created_at": "2024-01-15T10:30:00.000Z"
        });
        let post = provider.algolia_hit_to_post(&hit).unwrap();
        assert_eq!(post.id, "hackernews:99999");
        assert_eq!(post.title.unwrap(), "Test Algolia Hit");
        assert_eq!(post.score, 100);
        assert_eq!(post.comment_count, 25);
    }
}
