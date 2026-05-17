//! Platform provider implementations.
//!
//! Each provider handles fetching and normalizing data from a specific platform.

pub mod hackernews;
pub mod reddit;
pub mod x_platform;

pub use hackernews::HackerNewsProvider;
pub use reddit::RedditProvider;
pub use x_platform::XProvider;
