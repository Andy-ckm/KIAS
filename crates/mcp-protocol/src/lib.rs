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
//! - OAuth 2.0 & API Key authentication (with `auth` feature)
//! - Role-based access control (RBAC) (with `auth` feature)
//! - Circuit breaker & rate limiting (with `resilience` feature)
//! - Metrics collection & Prometheus export (with `metrics` feature)
//! - Credential management with encryption (with `credentials` feature)

// Core modules (always available)
pub mod capabilities;
pub mod client;
pub mod error;
pub mod prompt;
pub mod resource;
pub mod server;
pub mod tool;
pub mod transport;
pub mod types;

// Feature-gated modules
#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "resilience")]
pub mod resilience;

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "credentials")]
pub mod credentials;

// Re-export core types
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

// Re-export auth types
#[cfg(feature = "auth")]
pub use auth::{
    ApiKeyAuthProvider, AuthContext, AuthMethod, AuthProvider, AuthorizationManager, JwtAuthProvider,
    OAuthToken, Permission, Role, TokenClaims, UserInfo,
};

// Re-export resilience types
#[cfg(feature = "resilience")]
pub use resilience::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerMetrics,
    CircuitState, ClientRateLimiter, RateLimiterConfig, RateLimiterStats, SlidingWindowRateLimiter,
    TokenBucketRateLimiter,
};

// Re-export metrics types
#[cfg(feature = "metrics")]
pub use metrics::{
    LatencyMetrics, MetricsCollector, MetricsConfig, MetricsSnapshot, RequestMetrics, RequestTimer,
    ToolMetrics,
};

// Re-export credential types
#[cfg(feature = "credentials")]
pub use credentials::{
    AuditAction, AuditEntry, Credential, CredentialFilter, CredentialManager,
    CredentialManagerConfig, CredentialStatus, CredentialStore, CredentialType,
    InMemoryCredentialStore, RotationPolicy,
};

/// JSON-RPC protocol version constant.
pub const JSONRPC_VERSION: &str = "2.0";
