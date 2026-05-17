//! Browser session abstraction for MCP browser tools.
//!
//! Defines the `BrowserSession` trait that abstracts browser automation backends
//! (Chrome DevTools Protocol, Playwright, Puppeteer, etc.).
//!
//! Browser tools delegate to a `BrowserSession` implementation, making the
//! backend swappable without changing tool logic.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Errors that can occur during browser operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BrowserError {}

impl BrowserError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// Result of a browser page read — structured content extracted from the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    /// Page title.
    pub title: String,
    /// Page URL after navigation.
    pub url: String,
    /// Readable content as markdown text.
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<PageLink>>,
}

/// A hyperlink extracted from the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLink {
    pub text: String,
    pub href: String,
}

/// Result of a screenshot operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResult {
    /// Base64-encoded PNG image data.
    pub data: String,
    /// Image MIME type (always "image/png").
    pub mime_type: String,
    /// Page URL at time of screenshot.
    pub url: String,
}

/// Result of running JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsResult {
    /// The return value of the expression (JSON serialized).
    pub result: Value,
    /// Whether the execution threw an error.
    #[serde(default)]
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Async trait for browser session backends.
///
/// Implementations handle the actual browser connection (CDP, Playwright, etc.).
/// Each session is tied to one agent and manages its own page state.
#[async_trait]
pub trait BrowserSession: Send + Sync {
    /// Navigate to a URL and return the page content.
    async fn navigate(&self, url: &str) -> Result<PageContent, BrowserError>;

    /// Click an element identified by CSS selector or visible text.
    async fn click(&self, selector: &str) -> Result<PageContent, BrowserError>;

    /// Type text into an input field.
    async fn r#type(&self, selector: &str, text: &str) -> Result<PageContent, BrowserError>;

    /// Take a screenshot of the current page.
    async fn screenshot(&self) -> Result<ScreenshotResult, BrowserError>;

    /// Read the current page content as structured markdown.
    async fn read_page(&self) -> Result<PageContent, BrowserError>;

    /// Scroll the page in the given direction.
    async fn scroll(
        &self,
        direction: &str,
        amount: Option<u32>,
    ) -> Result<PageContent, BrowserError>;

    /// Wait for a CSS selector to appear on the page.
    async fn wait_for(&self, selector: &str, timeout_ms: Option<u32>) -> Result<(), BrowserError>;

    /// Execute JavaScript in the page context.
    async fn run_js(&self, expression: &str) -> Result<JsResult, BrowserError>;

    /// Navigate back in browser history.
    async fn go_back(&self) -> Result<PageContent, BrowserError>;

    /// Close the browser session and release resources.
    async fn close(&self) -> Result<(), BrowserError>;
}

/// A no-op browser session for testing.
///
/// Returns canned responses and records calls for assertion.
#[cfg(test)]
pub struct NoopBrowserSession {
    pub navigate_calls: std::sync::Mutex<Vec<String>>,
    pub click_calls: std::sync::Mutex<Vec<String>>,
    pub type_calls: std::sync::Mutex<Vec<(String, String)>>,
    pub closed: std::sync::Mutex<bool>,
}

#[cfg(test)]
impl NoopBrowserSession {
    pub fn new() -> Self {
        Self {
            navigate_calls: std::sync::Mutex::new(Vec::new()),
            click_calls: std::sync::Mutex::new(Vec::new()),
            type_calls: std::sync::Mutex::new(Vec::new()),
            closed: std::sync::Mutex::new(false),
        }
    }

    fn dummy_page(url: &str) -> PageContent {
        PageContent {
            title: format!("Page at {url}"),
            url: url.to_string(),
            content: format!("# Page\n\nContent of {url}"),
            links: Some(vec![PageLink {
                text: "Example".to_string(),
                href: "https://example.com".to_string(),
            }]),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl BrowserSession for NoopBrowserSession {
    async fn navigate(&self, url: &str) -> Result<PageContent, BrowserError> {
        self.navigate_calls.lock().unwrap().push(url.to_string());
        Ok(Self::dummy_page(url))
    }

    async fn click(&self, selector: &str) -> Result<PageContent, BrowserError> {
        self.click_calls.lock().unwrap().push(selector.to_string());
        Ok(Self::dummy_page("https://after-click.example.com"))
    }

    async fn r#type(&self, selector: &str, text: &str) -> Result<PageContent, BrowserError> {
        self.type_calls
            .lock()
            .unwrap()
            .push((selector.to_string(), text.to_string()));
        Ok(Self::dummy_page("https://after-type.example.com"))
    }

    async fn screenshot(&self) -> Result<ScreenshotResult, BrowserError> {
        Ok(ScreenshotResult {
            data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
            mime_type: "image/png".to_string(),
            url: "https://example.com".to_string(),
        })
    }

    async fn read_page(&self) -> Result<PageContent, BrowserError> {
        Ok(Self::dummy_page("https://current.example.com"))
    }

    async fn scroll(
        &self,
        _direction: &str,
        _amount: Option<u32>,
    ) -> Result<PageContent, BrowserError> {
        Ok(Self::dummy_page("https://current.example.com"))
    }

    async fn wait_for(
        &self,
        _selector: &str,
        _timeout_ms: Option<u32>,
    ) -> Result<(), BrowserError> {
        Ok(())
    }

    async fn run_js(&self, expression: &str) -> Result<JsResult, BrowserError> {
        Ok(JsResult {
            result: serde_json::json!({"expression": expression, "value": "noop"}),
            is_error: false,
            error_message: None,
        })
    }

    async fn go_back(&self) -> Result<PageContent, BrowserError> {
        Ok(Self::dummy_page("https://previous.example.com"))
    }

    async fn close(&self) -> Result<(), BrowserError> {
        *self.closed.lock().unwrap() = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_session_navigate() {
        let session = NoopBrowserSession::new();
        let page = session.navigate("https://example.com").await.unwrap();
        assert_eq!(page.url, "https://example.com");
        assert!(page.content.contains("example.com"));
    }

    #[tokio::test]
    async fn test_noop_session_click() {
        let session = NoopBrowserSession::new();
        let page = session.click("#btn").await.unwrap();
        assert_eq!(page.url, "https://after-click.example.com");
        assert_eq!(session.click_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_noop_session_type() {
        let session = NoopBrowserSession::new();
        let page = session.r#type("#input", "hello").await.unwrap();
        assert_eq!(page.url, "https://after-type.example.com");
        let calls = session.type_calls.lock().unwrap();
        assert_eq!(calls[0], ("#input".to_string(), "hello".to_string()));
    }

    #[tokio::test]
    async fn test_noop_session_screenshot() {
        let session = NoopBrowserSession::new();
        let shot = session.screenshot().await.unwrap();
        assert_eq!(shot.mime_type, "image/png");
        assert!(!shot.data.is_empty());
    }

    #[tokio::test]
    async fn test_noop_session_close() {
        let session = NoopBrowserSession::new();
        session.close().await.unwrap();
        assert!(*session.closed.lock().unwrap());
    }

    #[test]
    fn test_browser_error_display() {
        let err = BrowserError::new("not found").with_code("NAV_ERR");
        assert_eq!(err.to_string(), "not found");
        assert_eq!(err.code, Some("NAV_ERR".to_string()));
    }
}
