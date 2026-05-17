//! MCP browser tool definitions and registration.
//!
//! Provides 10 browser automation tools that follow the Kimi WebBridge pattern:
//! - `browser_navigate` — Navigate to URL
//! - `browser_click` — Click element by selector/text
//! - `browser_type` — Type text into input field
//! - `browser_screenshot` — Take page screenshot (base64 PNG)
//! - `browser_read_page` — Read page content as structured markdown
//! - `browser_scroll` — Scroll page in direction
//! - `browser_wait` — Wait for selector to appear
//! - `browser_run_js` — Execute JavaScript expression
//! - `browser_back` — Navigate back in history
//! - `browser_close` — Close browser session
//!
//! `BrowserToolKit` registers all tools on an `McpServer` with handlers
//! that delegate to a `BrowserSession` implementation.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::server::{ToolAnnotations, ToolDefinition, ToolHandler, ToolResult, ToolResultContent};
use crate::McpServer;

use super::session::BrowserSession;

// ---------------------------------------------------------------------------
// Tool definitions (static)
// ---------------------------------------------------------------------------

/// Get all 10 browser tool definitions.
pub fn browser_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "browser_navigate",
            "Navigate a browser to a URL. Returns the page title and readable content as markdown. Opens a persistent browser session.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to navigate to (http/https only)" }
                },
                "required": ["url"]
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Navigate".into()),
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_click",
            "Click an element on the current browser page by CSS selector or visible text. Returns the resulting page state.",
            json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector (e.g., '#submit-btn', '.add-to-cart') or visible text to click" }
                },
                "required": ["selector"]
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Click".into()),
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_type",
            "Type text into an input field on the current browser page.",
            json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector for the input field (e.g., 'input[name=\"email\"]', '#search-box')" },
                    "text": { "type": "string", "description": "The text to type into the field" }
                },
                "required": ["selector", "text"]
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Type".into()),
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_screenshot",
            "Take a screenshot of the current browser page. Returns a base64-encoded PNG image.",
            json!({
                "type": "object",
                "properties": {}
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Screenshot".into()),
            read_only_hint: true,
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_read_page",
            "Read the current browser page content as structured markdown. Use after clicking or navigating to see the updated page.",
            json!({
                "type": "object",
                "properties": {}
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Read Page".into()),
            read_only_hint: true,
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_scroll",
            "Scroll the browser page. Use this to see content below the fold or navigate long pages.",
            json!({
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "description": "Scroll direction: 'up', 'down', 'left', 'right' (default: 'down')" },
                    "amount": { "type": "integer", "description": "Pixels to scroll (default: 600)" }
                }
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Scroll".into()),
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_wait",
            "Wait for a CSS selector to appear on the page. Useful for dynamic content that loads asynchronously.",
            json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector to wait for" },
                    "timeout_ms": { "type": "integer", "description": "Max wait time in milliseconds (default: 5000, max: 30000)" }
                },
                "required": ["selector"]
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Wait".into()),
            read_only_hint: true,
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_run_js",
            "Run JavaScript on the current browser page and return the result. For advanced interactions that other browser tools cannot handle.",
            json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "string", "description": "JavaScript expression to run in the page context" }
                },
                "required": ["expression"]
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Run JS".into()),
            open_world_hint: true,
            destructive_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_back",
            "Go back to the previous page in browser history.",
            json!({
                "type": "object",
                "properties": {}
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Back".into()),
            open_world_hint: true,
            ..Default::default()
        }),
        ToolDefinition::new(
            "browser_close",
            "Close the browser session. The browser will also auto-close when the agent loop ends.",
            json!({
                "type": "object",
                "properties": {}
            }),
        ).with_annotations(ToolAnnotations {
            title: Some("Browser Close".into()),
            destructive_hint: true,
            ..Default::default()
        }),
    ]
}

// ---------------------------------------------------------------------------
// Tool handlers (wrap BrowserSession)
// ---------------------------------------------------------------------------

/// Handler for `browser_navigate`.
pub struct BrowserNavigateHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserNavigateHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserNavigateHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let url = extract_required_string(&params, "url")?;
        match self.session.navigate(&url).await {
            Ok(page) => Ok(ToolResult::text(format!(
                "# {}\n\nURL: {}\n\n{}",
                page.title, page.url, page.content
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_navigate error: {e}"))),
        }
    }
}

/// Handler for `browser_click`.
pub struct BrowserClickHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserClickHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserClickHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let selector = extract_required_string(&params, "selector")?;
        match self.session.click(&selector).await {
            Ok(page) => Ok(ToolResult::text(format!(
                "# {}\n\nURL: {}\n\n{}",
                page.title, page.url, page.content
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_click error: {e}"))),
        }
    }
}

/// Handler for `browser_type`.
pub struct BrowserTypeHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserTypeHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserTypeHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let selector = extract_required_string(&params, "selector")?;
        let text = extract_required_string(&params, "text")?;
        match self.session.r#type(&selector, &text).await {
            Ok(page) => Ok(ToolResult::text(format!(
                "# {}\n\nURL: {}\n\n{}",
                page.title, page.url, page.content
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_type error: {e}"))),
        }
    }
}

/// Handler for `browser_screenshot`.
pub struct BrowserScreenshotHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserScreenshotHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserScreenshotHandler {
    async fn execute(&self, _params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        match self.session.screenshot().await {
            Ok(shot) => Ok(ToolResult::with_content(vec![
                ToolResultContent::Text {
                    text: format!("Screenshot of {}", shot.url),
                },
                ToolResultContent::Image {
                    data: shot.data,
                    mime_type: shot.mime_type,
                },
            ])),
            Err(e) => Ok(ToolResult::error(format!("browser_screenshot error: {e}"))),
        }
    }
}

/// Handler for `browser_read_page`.
pub struct BrowserReadPageHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserReadPageHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserReadPageHandler {
    async fn execute(&self, _params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        match self.session.read_page().await {
            Ok(page) => {
                let mut text = format!("# {}\n\nURL: {}\n\n{}", page.title, page.url, page.content);
                if let Some(links) = &page.links {
                    if !links.is_empty() {
                        text.push_str("\n\n## Links\n");
                        for link in links {
                            text.push_str(&format!("- [{}]({})\n", link.text, link.href));
                        }
                    }
                }
                Ok(ToolResult::text(text))
            }
            Err(e) => Ok(ToolResult::error(format!("browser_read_page error: {e}"))),
        }
    }
}

/// Handler for `browser_scroll`.
pub struct BrowserScrollHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserScrollHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserScrollHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let direction =
            extract_optional_string(&params, "direction").unwrap_or_else(|| "down".to_string());
        let amount = params
            .as_ref()
            .and_then(|p| p.get("amount"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        match self.session.scroll(&direction, amount).await {
            Ok(page) => Ok(ToolResult::text(format!(
                "# {}\n\nURL: {}\n\n{}",
                page.title, page.url, page.content
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_scroll error: {e}"))),
        }
    }
}

/// Handler for `browser_wait`.
pub struct BrowserWaitHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserWaitHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserWaitHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let selector = extract_required_string(&params, "selector")?;
        let timeout_ms = params
            .as_ref()
            .and_then(|p| p.get("timeout_ms"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        match self.session.wait_for(&selector, timeout_ms).await {
            Ok(()) => Ok(ToolResult::text(format!(
                "Element '{}' appeared on page",
                selector
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_wait error: {e}"))),
        }
    }
}

/// Handler for `browser_run_js`.
pub struct BrowserRunJsHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserRunJsHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserRunJsHandler {
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        let expression = extract_required_string(&params, "expression")?;
        match self.session.run_js(&expression).await {
            Ok(result) => {
                if result.is_error {
                    Ok(ToolResult::error(format!(
                        "JavaScript error: {}",
                        result
                            .error_message
                            .unwrap_or_else(|| "unknown".to_string())
                    )))
                } else {
                    Ok(ToolResult::text(format!("JS result: {}", result.result)))
                }
            }
            Err(e) => Ok(ToolResult::error(format!("browser_run_js error: {e}"))),
        }
    }
}

/// Handler for `browser_back`.
pub struct BrowserBackHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserBackHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserBackHandler {
    async fn execute(&self, _params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        match self.session.go_back().await {
            Ok(page) => Ok(ToolResult::text(format!(
                "# {}\n\nURL: {}\n\n{}",
                page.title, page.url, page.content
            ))),
            Err(e) => Ok(ToolResult::error(format!("browser_back error: {e}"))),
        }
    }
}

/// Handler for `browser_close`.
pub struct BrowserCloseHandler {
    session: Arc<dyn BrowserSession>,
}

impl BrowserCloseHandler {
    pub fn new(session: Arc<dyn BrowserSession>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ToolHandler for BrowserCloseHandler {
    async fn execute(&self, _params: Option<Value>) -> Result<ToolResult, crate::McpError> {
        match self.session.close().await {
            Ok(()) => Ok(ToolResult::text("Browser session closed".to_string())),
            Err(e) => Ok(ToolResult::error(format!("browser_close error: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// BrowserToolKit — registers all browser tools on an McpServer
// ---------------------------------------------------------------------------

/// Convenience struct that registers all 10 browser tools on an McpServer.
///
/// # Example
/// ```ignore
/// use std::sync::Arc;
/// use kias_mcp_protocol::browser::{BrowserToolKit, BrowserSession};
///
/// let session: Arc<dyn BrowserSession> = /* ... */;
/// let mut server = McpServer::new("kias", "0.1.0", caps);
/// BrowserToolKit::register(&mut server, session);
/// ```
pub struct BrowserToolKit;

impl BrowserToolKit {
    /// Register all 10 browser tools on the given server.
    pub fn register(server: &mut McpServer, session: Arc<dyn BrowserSession>) {
        let defs = browser_tool_definitions();

        let s = session.clone();
        server.register_tool(defs[0].clone(), BrowserNavigateHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[1].clone(), BrowserClickHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[2].clone(), BrowserTypeHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[3].clone(), BrowserScreenshotHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[4].clone(), BrowserReadPageHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[5].clone(), BrowserScrollHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[6].clone(), BrowserWaitHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[7].clone(), BrowserRunJsHandler::new(s));
        let s = session.clone();
        server.register_tool(defs[8].clone(), BrowserBackHandler::new(s));
        server.register_tool(defs[9].clone(), BrowserCloseHandler::new(session));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_required_string(params: &Option<Value>, key: &str) -> Result<String, crate::McpError> {
    params
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            crate::McpError::InvalidRequest(format!("Missing required parameter: {key}"))
        })
}

fn extract_optional_string(params: &Option<Value>, key: &str) -> Option<String> {
    params
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::session::NoopBrowserSession;
    use crate::server::McpServer as EnhancedMcpServer;
    use crate::types::{McpRequest, RequestId};

    fn make_server_with_browser() -> EnhancedMcpServer {
        let caps = crate::server::EnhancedServerCapabilities::new().with_tools();
        let mut server = EnhancedMcpServer::new("test-browser", "1.0.0", caps);
        let session: Arc<dyn BrowserSession> = Arc::new(NoopBrowserSession::new());
        BrowserToolKit::register(&mut server, session);
        server
    }

    #[test]
    fn test_browser_tool_definitions_count() {
        let defs = browser_tool_definitions();
        assert_eq!(defs.len(), 10);
    }

    #[test]
    fn test_browser_tool_names() {
        let defs = browser_tool_definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"browser_navigate"));
        assert!(names.contains(&"browser_click"));
        assert!(names.contains(&"browser_type"));
        assert!(names.contains(&"browser_screenshot"));
        assert!(names.contains(&"browser_read_page"));
        assert!(names.contains(&"browser_scroll"));
        assert!(names.contains(&"browser_wait"));
        assert!(names.contains(&"browser_run_js"));
        assert!(names.contains(&"browser_back"));
        assert!(names.contains(&"browser_close"));
    }

    #[test]
    fn test_browser_tool_definitions_have_schemas() {
        for def in browser_tool_definitions() {
            assert!(def.input_schema.is_object(), "{} missing schema", def.name);
            assert!(
                !def.description.is_empty(),
                "{} missing description",
                def.name
            );
        }
    }

    #[test]
    fn test_browser_tool_annotations() {
        let defs = browser_tool_definitions();
        let navigate = defs.iter().find(|d| d.name == "browser_navigate").unwrap();
        assert!(navigate.annotations.is_some());
        let ann = navigate.annotations.as_ref().unwrap();
        assert!(ann.open_world_hint);

        let screenshot = defs
            .iter()
            .find(|d| d.name == "browser_screenshot")
            .unwrap();
        let ann = screenshot.annotations.as_ref().unwrap();
        assert!(ann.read_only_hint);

        let close = defs.iter().find(|d| d.name == "browser_close").unwrap();
        let ann = close.annotations.as_ref().unwrap();
        assert!(ann.destructive_hint);
    }

    #[test]
    fn test_tools_list_includes_browser() {
        let server = make_server_with_browser();
        let req = McpRequest::new(RequestId::Number(1), "tools/list", None);
        let resp = server.handle_request_sync(&req);
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 10);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"browser_navigate"));
        assert!(names.contains(&"browser_close"));
    }

    #[tokio::test]
    async fn test_browser_navigate_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(10),
            "tools/call",
            Some(json!({"name": "browser_navigate", "arguments": {"url": "https://example.com"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("example.com"));
    }

    #[tokio::test]
    async fn test_browser_click_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(11),
            "tools/call",
            Some(json!({"name": "browser_click", "arguments": {"selector": "#btn"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert!(!result["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_browser_type_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(12),
            "tools/call",
            Some(
                json!({"name": "browser_type", "arguments": {"selector": "#input", "text": "hello"}}),
            ),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert!(!result["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_browser_screenshot_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(13),
            "tools/call",
            Some(json!({"name": "browser_screenshot", "arguments": {}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.len(), 2); // text + image
        assert_eq!(content[1]["type"], "image");
    }

    #[tokio::test]
    async fn test_browser_close_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(14),
            "tools/call",
            Some(json!({"name": "browser_close", "arguments": {}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("closed"));
    }

    #[tokio::test]
    async fn test_browser_missing_required_param() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(15),
            "tools/call",
            Some(json!({"name": "browser_navigate", "arguments": {}})),
        );
        let resp = server.handle_request(&req).await;
        // Missing required param causes McpError::InvalidRequest → JSON-RPC error
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32000); // Tool error
    }

    #[tokio::test]
    async fn test_browser_scroll_with_defaults() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(16),
            "tools/call",
            Some(json!({"name": "browser_scroll", "arguments": {}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert!(!result["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_browser_wait_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(17),
            "tools/call",
            Some(json!({"name": "browser_wait", "arguments": {"selector": ".loaded"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(".loaded"));
    }

    #[tokio::test]
    async fn test_browser_run_js_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(18),
            "tools/call",
            Some(json!({"name": "browser_run_js", "arguments": {"expression": "document.title"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("document.title"));
    }

    #[tokio::test]
    async fn test_browser_back_handler() {
        let server = make_server_with_browser();
        let req = McpRequest::new(
            RequestId::Number(19),
            "tools/call",
            Some(json!({"name": "browser_back", "arguments": {}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("previous"));
    }

    #[test]
    fn test_helper_extract_required_string() {
        let params = Some(json!({"url": "https://test.com"}));
        assert_eq!(
            extract_required_string(&params, "url").unwrap(),
            "https://test.com"
        );
        assert!(extract_required_string(&params, "missing").is_err());
    }

    #[test]
    fn test_helper_extract_optional_string() {
        let params = Some(json!({"direction": "up"}));
        assert_eq!(
            extract_optional_string(&params, "direction"),
            Some("up".to_string())
        );
        assert_eq!(extract_optional_string(&params, "missing"), None);
    }
}
