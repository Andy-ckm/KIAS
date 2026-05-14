//! # MCP Protocol
//!
//! A comprehensive MCP (Model Context Protocol) client/server framework.
//!
//! Provides:
//! - JSON-RPC 2.0 message types (Request, Response, Notification)
//! - Tool, Resource, and Prompt definitions
//! - Server capabilities with logging support
//! - Client for connecting to MCP servers and calling tools
//! - Enhanced server with async tool/resource/prompt handlers
//! - Transport layer: stdio, HTTP+SSE, and in-memory

pub mod capabilities;
pub mod client;
pub mod error;
pub mod prompt;
pub mod resource;
pub mod server;
pub mod tool;
pub mod transport;
pub mod types;

pub use capabilities::ServerCapabilities;
pub use client::McpClient;
pub use error::McpError;
pub use prompt::{Prompt, PromptArgument};
pub use resource::Resource;
pub use server::{
    EnhancedServerCapabilities, McpServer, ResourceContent, ResourceHandler, ServerInfo,
    StaticResourceHandler, ToolAnnotations, ToolDefinition, ToolHandler, ToolResult,
    ToolResultContent,
};
pub use tool::Tool;
pub use transport::{
    HttpTransport, InMemoryTransport as ServerInMemoryTransport, McpTransport, StdioTransport,
};
pub use types::{McpNotification, McpRequest, McpResponse, RequestId};

/// JSON-RPC protocol version constant.
pub const JSONRPC_VERSION: &str = "2.0";
