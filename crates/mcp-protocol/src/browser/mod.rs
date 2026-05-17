//! # Browser Automation Tools for MCP
//!
//! Provides 10 browser automation tools as MCP tool handlers, following the
//! Kimi WebBridge pattern for agent-driven web interaction.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │  MCP Server (tools/list, tools/call)        │
//! │  ┌─────────────────────────────────────┐    │
//! │  │  BrowserToolKit (10 tools)          │    │
//! │  │  ├─ browser_navigate                │    │
//! │  │  ├─ browser_click                   │    │
//! │  │  ├─ browser_type                    │    │
//! │  │  ├─ browser_screenshot              │    │
//! │  │  ├─ browser_read_page               │    │
//! │  │  ├─ browser_scroll                  │    │
//! │  │  ├─ browser_wait                    │    │
//! │  │  ├─ browser_run_js                  │    │
//! │  │  ├─ browser_back                    │    │
//! │  │  └─ browser_close                   │    │
//! │  └──────────┬──────────────────────────┘    │
//! │             │ delegates to                   │
//! │  ┌──────────▼──────────────────────────┐    │
//! │  │  BrowserSession trait               │    │
//! │  │  (CDP / Playwright / Noop impl)     │    │
//! │  └─────────────────────────────────────┘    │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use std::sync::Arc;
//! use kias_mcp_protocol::browser::{BrowserToolKit, BrowserSession};
//! use kias_mcp_protocol::McpServer;
//!
//! // 1. Create a browser session (CDP, Playwright, etc.)
//! let session: Arc<dyn BrowserSession> = Arc::new(MyCdpSession::new().await?);
//!
//! // 2. Register all browser tools on the server
//! let mut server = McpServer::new("kias", "0.1.0", caps);
//! BrowserToolKit::register(&mut server, session);
//! ```
//!
//! ## Tool Summary
//!
//! | Tool | Description | Read-only | Destructive |
//! |------|-------------|-----------|-------------|
//! | `browser_navigate` | Navigate to URL | ✗ | ✗ |
//! | `browser_click` | Click element | ✗ | ✗ |
//! | `browser_type` | Type into field | ✗ | ✗ |
//! | `browser_screenshot` | Take screenshot (base64 PNG) | ✓ | ✗ |
//! | `browser_read_page` | Read page as markdown | ✓ | ✗ |
//! | `browser_scroll` | Scroll page | ✗ | ✗ |
//! | `browser_wait` | Wait for selector | ✓ | ✗ |
//! | `browser_run_js` | Run JavaScript | ✗ | ✓ |
//! | `browser_back` | Go back | ✗ | ✗ |
//! | `browser_close` | Close session | ✗ | ✓ |

pub mod session;
pub mod tools;

pub use session::{
    BrowserError, BrowserSession, JsResult, PageContent, PageLink, ScreenshotResult,
};
pub use tools::{browser_tool_definitions, BrowserToolKit};
