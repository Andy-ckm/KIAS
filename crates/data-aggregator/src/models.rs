//! Unified data models for cross-platform content aggregation.
//!
//! These models represent a platform-agnostic view of social content,
//! normalizing posts from X (Twitter), Reddit, and Hacker News into
//! a single schema.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Supported platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// X (formerly Twitter).
    X,
    /// Reddit.
    Reddit,
    /// Hacker News.
    HackerNews,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::X => write!(f, "x"),
            Platform::Reddit => write!(f, "reddit"),
            Platform::HackerNews => write!(f, "hackernews"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "x" | "twitter" => Ok(Platform::X),
            "reddit" => Ok(Platform::Reddit),
            "hackernews" | "hn" | "hacker_news" => Ok(Platform::HackerNews),
            _ => Err(format!("Unknown platform: {s}")),
        }
    }
}

/// Author / user information normalized across platforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostAuthor {
    /// Unique identifier on the platform.
    pub id: String,
    /// Display name or username.
    pub username: String,
    /// Optional display name (real name / screen name).
    pub display_name: Option<String>,
    /// Profile image URL.
    pub avatar_url: Option<String>,
    /// Karma / reputation score if available.
    pub reputation: Option<i64>,
    /// Platform this author belongs to.
    pub platform: Platform,
}

/// A single piece of content (post / tweet / story / comment) in unified format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPost {
    /// Unique identifier (composite: `{platform}:{native_id}`).
    pub id: String,
    /// Platform-specific native identifier.
    pub native_id: String,
    /// Source platform.
    pub platform: Platform,
    /// Post title (HN stories, Reddit posts). None for tweets.
    pub title: Option<String>,
    /// Body / text content.
    pub body: Option<String>,
    /// URL attached to the post (link posts).
    pub url: Option<String>,
    /// Author information.
    pub author: PostAuthor,
    /// Upvote / like count.
    pub score: i64,
    /// Number of comments / replies.
    pub comment_count: u32,
    /// When the post was created (UTC).
    pub created_at: DateTime<Utc>,
    /// Tags / flairs / subreddits.
    pub tags: Vec<String>,
    /// Language hint (ISO 639-1 if known).
    pub language: Option<String>,
    /// Raw platform-specific payload (for advanced consumers).
    pub raw: Option<serde_json::Value>,
}

/// A batch of aggregated posts with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedFeed {
    /// Which platform this feed came from.
    pub platform: Platform,
    /// The query or feed identifier (e.g. "technology", "rust", search query).
    pub query: String,
    /// Aggregated posts.
    pub posts: Vec<AggregatedPost>,
    /// Cursor for pagination (platform-specific opaque string).
    pub next_cursor: Option<String>,
    /// When this feed was fetched.
    pub fetched_at: DateTime<Utc>,
    /// Total number of results available (if the platform reports it).
    pub total: Option<u64>,
}

/// Query parameters for fetching aggregated content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchQuery {
    /// Search terms / subreddit name / etc.
    pub query: String,
    /// Maximum number of posts to return.
    pub limit: Option<u32>,
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Sort order (e.g. "new", "hot", "top").
    pub sort: Option<String>,
    /// Time window filter (e.g. "day", "week", "month").
    pub time_window: Option<String>,
}

impl FetchQuery {
    /// Create a simple query with just a search term.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: None,
            cursor: None,
            sort: None,
            time_window: None,
        }
    }

    /// Set the maximum number of results.
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set pagination cursor.
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Set sort order.
    pub fn with_sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Set time window.
    pub fn with_time_window(mut self, window: impl Into<String>) -> Self {
        self.time_window = Some(window.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Platform Display ===

    #[test]
    fn platform_display_x() {
        assert_eq!(Platform::X.to_string(), "x");
    }

    #[test]
    fn platform_display_reddit() {
        assert_eq!(Platform::Reddit.to_string(), "reddit");
    }

    #[test]
    fn platform_display_hackernews() {
        assert_eq!(Platform::HackerNews.to_string(), "hackernews");
    }

    // === Platform FromStr ===

    #[test]
    fn platform_from_str_basic() {
        assert_eq!("x".parse::<Platform>().unwrap(), Platform::X);
        assert_eq!("reddit".parse::<Platform>().unwrap(), Platform::Reddit);
        assert_eq!(
            "hackernews".parse::<Platform>().unwrap(),
            Platform::HackerNews
        );
    }

    #[test]
    fn platform_from_str_aliases() {
        assert_eq!("twitter".parse::<Platform>().unwrap(), Platform::X);
        assert_eq!("hn".parse::<Platform>().unwrap(), Platform::HackerNews);
        assert_eq!(
            "hacker_news".parse::<Platform>().unwrap(),
            Platform::HackerNews
        );
    }

    #[test]
    fn platform_from_str_case_insensitive() {
        assert_eq!("Reddit".parse::<Platform>().unwrap(), Platform::Reddit);
        assert_eq!("TWITTER".parse::<Platform>().unwrap(), Platform::X);
    }

    #[test]
    fn platform_from_str_unknown() {
        assert!("unknown".parse::<Platform>().is_err());
        assert!("".parse::<Platform>().is_err());
    }

    #[test]
    fn platform_display_from_str_roundtrip() {
        for p in [Platform::X, Platform::Reddit, Platform::HackerNews] {
            let s = p.to_string();
            let parsed: Platform = s.parse().unwrap();
            assert_eq!(parsed, p);
        }
    }

    // === FetchQuery builder ===

    #[test]
    fn fetch_query_new() {
        let q = FetchQuery::new("rust");
        assert_eq!(q.query, "rust");
        assert!(q.limit.is_none());
        assert!(q.cursor.is_none());
        assert!(q.sort.is_none());
        assert!(q.time_window.is_none());
    }

    #[test]
    fn fetch_query_builder_chain() {
        let q = FetchQuery::new("ai")
            .with_limit(25)
            .with_cursor("abc123")
            .with_sort("top")
            .with_time_window("week");

        assert_eq!(q.query, "ai");
        assert_eq!(q.limit, Some(25));
        assert_eq!(q.cursor.as_deref(), Some("abc123"));
        assert_eq!(q.sort.as_deref(), Some("top"));
        assert_eq!(q.time_window.as_deref(), Some("week"));
    }

    // === Serde round-trip ===

    #[test]
    fn platform_serde_roundtrip() {
        for p in [Platform::X, Platform::Reddit, Platform::HackerNews] {
            let json = serde_json::to_string(&p).unwrap();
            let back: Platform = serde_json::from_str(&json).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn fetch_query_serde_roundtrip() {
        let q = FetchQuery::new("test").with_limit(10).with_sort("hot");
        let json = serde_json::to_string(&q).unwrap();
        let back: FetchQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back.query, "test");
        assert_eq!(back.limit, Some(10));
        assert_eq!(back.sort.as_deref(), Some("hot"));
    }

    #[test]
    fn aggregated_post_serde_roundtrip() {
        let post = AggregatedPost {
            id: "hn:12345".to_string(),
            native_id: "12345".to_string(),
            platform: Platform::HackerNews,
            title: Some("Test Title".to_string()),
            body: Some("Test body".to_string()),
            url: Some("https://example.com".to_string()),
            author: PostAuthor {
                id: "user1".to_string(),
                username: "testuser".to_string(),
                display_name: Some("Test User".to_string()),
                avatar_url: None,
                reputation: Some(1000),
                platform: Platform::HackerNews,
            },
            score: 42,
            comment_count: 7,
            created_at: chrono::Utc::now(),
            tags: vec!["technology".to_string()],
            language: Some("en".to_string()),
            raw: None,
        };
        let json = serde_json::to_string(&post).unwrap();
        let back: AggregatedPost = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "hn:12345");
        assert_eq!(back.score, 42);
        assert_eq!(back.platform, Platform::HackerNews);
        assert_eq!(back.author.username, "testuser");
    }

    // === AggregatorError Display ===

    use crate::error::AggregatorError;

    #[test]
    fn error_display_variants() {
        let err = AggregatorError::UnsupportedPlatform("mastodon".to_string());
        assert!(err.to_string().contains("mastodon"));

        let err = AggregatorError::RateLimited("x".to_string(), 60);
        assert!(err.to_string().contains("60"));

        let err = AggregatorError::AuthError("invalid key".to_string());
        assert!(err.to_string().contains("invalid key"));
    }

    #[test]
    fn error_from_reqwest() {
        // Construct a reqwest error by making a request to an invalid URL
        // We can't easily construct a reqwest::Error, so test the From impl indirectly
        // by checking that AggregatorError implements the right traits
        fn accepts_aggregator_error(e: AggregatorError) -> String {
            e.to_string()
        }
        let err = AggregatorError::HttpRequest("timeout".to_string());
        assert!(accepts_aggregator_error(err).contains("timeout"));
    }

    #[test]
    fn error_to_kias_error() {
        let err = AggregatorError::ParseError("bad json".to_string());
        let kias_err: kias_common::error::KiasError = err.into();
        assert!(kias_err.to_string().contains("bad json"));
    }
}
