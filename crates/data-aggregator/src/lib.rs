//! # KIAS Data Aggregator
//!
//! Cross-platform data aggregation framework inspired by Kimi WebBridge.
//! Provides unified fetching and structuring from X (Twitter), Reddit,
//! and Hacker News.
//!
//! ## Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────┐
//! │              DataAggregator                    │
//! │  (orchestrates parallel multi-platform fetch)  │
//! └────────┬──────────┬──────────┬────────────────┘
//!          │          │          │
//!    ┌─────▼───┐ ┌────▼────┐ ┌──▼──────────┐
//!    │  X API  │ │ Reddit  │ │ Hacker News │
//!    │ Provider│ │ Provider│ │  Provider   │
//!    └─────────┘ └─────────┘ └─────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use kias_data_aggregator::{
//!     DataAggregator, FetchQuery,
//!     HackerNewsProvider, RedditProvider,
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut agg = DataAggregator::new();
//! agg.register(Arc::new(HackerNewsProvider::new()));
//! agg.register(Arc::new(RedditProvider::new()));
//!
//! let query = FetchQuery::new("rust programming").with_limit(10);
//! let results = agg.fetch_all(&query).await;
//!
//! for (platform, feed) in &results {
//!     match feed {
//!         Ok(f) => println!("{}: {} posts", platform, f.posts.len()),
//!         Err(e) => eprintln!("{}: error: {}", platform, e),
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod aggregator;
pub mod error;
pub mod models;
pub mod providers;
pub mod traits;

pub use aggregator::{AggregatorConfig, DataAggregator};
pub use error::AggregatorError;
pub use models::{AggregatedFeed, AggregatedPost, FetchQuery, Platform, PostAuthor};
pub use providers::{HackerNewsProvider, RedditProvider, XProvider};
pub use traits::PlatformProvider;

// pub // mod cost_panel; // TODO: fix compilation // TODO: fix compilation
