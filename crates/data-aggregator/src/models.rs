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
