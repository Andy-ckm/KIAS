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
//! - Tool hot-reload from YAML/JSON files (with `hot-reload` feature)
//! - Sandbox execution environments (with `sandbox` feature)

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
#[cfg(feature = "browser")]
pub mod browser;

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "resilience")]
pub mod resilience;

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "credentials")]
pub mod credentials;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

#[cfg(feature = "sandbox")]
pub mod sandbox;

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
    ApiKeyAuthProvider, AuthContext, AuthMethod, AuthProvider, AuthorizationManager,
    JwtAuthProvider, OAuthToken, Permission, Role, TokenClaims, UserInfo,
};

// Re-export resilience types
#[cfg(feature = "resilience")]
pub use resilience::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerMetrics, CircuitState,
    ClientRateLimiter, RateLimiterConfig, RateLimiterStats, SlidingWindowRateLimiter,
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
    AuditAction, AuditEntry, ConsoleRotationNotifier, Credential, CredentialFilter,
    CredentialManager, CredentialManagerConfig, CredentialStatus, CredentialStore, CredentialType,
    InMemoryCredentialStore, InMemoryRotationNotifier, RotationEvent, RotationNotifier,
    RotationPolicy,
};

// Re-export hot-reload types
#[cfg(feature = "hot-reload")]
pub use hot_reload::{
    ToolDefinitionFile, ToolImplementation, ToolRegistry, ToolRegistryEntry, ToolVersion,
};

// Re-export sandbox types
#[cfg(feature = "sandbox")]
pub use sandbox::{
    FilesystemConfig, FirecrackerSandboxBackend, GVisorSandboxBackend, IsolationLevel, MountPoint,
    NetworkPolicy, ProcessSandboxBackend, ResourceLimits, ResourceUsage, SandboxAction,
    SandboxAuditEntry, SandboxBackend, SandboxBackendTrait, SandboxConfig, SandboxInstance,
    SandboxManager, SandboxManagerConfig, SandboxResult, SandboxSnapshot, SandboxState,
    WasmSandboxBackend, WorkspaceProjection,
};

#[cfg(feature = "docker")]
pub use sandbox::DockerSandboxBackend;

// Re-export browser types
#[cfg(feature = "browser")]
pub use browser::{
    BrowserError, BrowserSession, BrowserToolKit, JsResult, PageContent, PageLink, ScreenshotResult,
};

/// JSON-RPC protocol version constant.
pub const JSONRPC_VERSION: &str = "2.0";
