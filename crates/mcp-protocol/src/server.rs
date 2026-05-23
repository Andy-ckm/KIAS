//! Enhanced MCP Server implementation with full protocol support.
//!
//! Provides:
//! - `McpServer` with tool, resource, and prompt registries
//! - Async `ToolHandler` trait for tool execution
//! - `ResourceHandler` for dynamic resource reading
//! - `PromptHandler` for dynamic prompt retrieval
//! - Full JSON-RPC 2.0 dispatch: initialize, ping, tools/*, resources/*, prompts/*
//! - `ServerCapabilities` with logging support

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capabilities::{ClientCapabilities, PromptsCapability, ResourcesCapability, ToolsCapability, VersionNegotiation};
use crate::error::McpError;
use crate::prompt::Prompt;
use crate::resource::Resource;
use crate::tool::Tool;
use crate::types::{McpRequest, McpResponse};

// ---------------------------------------------------------------------------
// Tool annotations (per MCP spec)
// ---------------------------------------------------------------------------

/// Annotations that provide hints about tool behavior.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable title for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// If true, the tool does not modify its environment.
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_only_hint: bool,
    /// If true, the tool may perform destructive operations.
    #[serde(default, skip_serializing_if = "is_false")]
    pub destructive_hint: bool,
    /// If true, calling the tool multiple times with the same args has no additional effect.
    #[serde(default, skip_serializing_if = "is_false")]
    pub idempotent_hint: bool,
    /// If true, the tool interacts with the external world.
    #[serde(default, skip_serializing_if = "is_false")]
    pub open_world_hint: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Extended tool definition with annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for input parameters.
    pub input_schema: Value,
    /// Optional behavioral annotations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            annotations: None,
        }
    }

    /// Attach annotations.
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }
}

impl From<ToolDefinition> for Tool {
    fn from(td: ToolDefinition) -> Self {
        Tool::new(td.name, td.description, td.input_schema)
    }
}

// ---------------------------------------------------------------------------
// Tool result types
// ---------------------------------------------------------------------------

/// A single content block in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: ResourceContentBlock },
}

/// Resource content embedded in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentBlock {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Result returned from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content blocks returned by the tool.
    pub content: Vec<ToolResultContent>,
    /// Whether this result represents an error.
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful text result.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    /// Create an error result with a message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text {
                text: message.into(),
            }],
            is_error: true,
        }
    }

    /// Create a result with multiple content blocks.
    pub fn with_content(content: Vec<ToolResultContent>) -> Self {
        Self {
            content,
            is_error: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Async ToolHandler trait
// ---------------------------------------------------------------------------

/// Async trait for tool execution.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with the given parameters and return a result.
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, McpError>;
}

/// Wraps a sync closure as a ToolHandler for convenience.
pub struct ClosureToolHandler<F>(pub F);

#[async_trait]
impl<F> ToolHandler for ClosureToolHandler<F>
where
    F: Fn(Option<Value>) -> Result<ToolResult, String> + Send + Sync,
{
    async fn execute(&self, params: Option<Value>) -> Result<ToolResult, McpError> {
        (self.0)(params).map_err(McpError::InvalidRequest)
    }
}

// ---------------------------------------------------------------------------
// Resource content & handler
// ---------------------------------------------------------------------------

/// Content returned when reading a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The URI of the resource.
    pub uri: String,
    /// Optional MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Text content (if textual).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64-encoded binary content (if binary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

impl ResourceContent {
    /// Create a text resource content.
    pub fn text(uri: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            mime_type: None,
            text: Some(text.into()),
            blob: None,
        }
    }
}

/// Async trait for reading resources.
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    /// Read a resource by URI.
    async fn read(&self, uri: &str) -> Result<ResourceContent, McpError>;
}

/// A static resource handler that returns content from a map.
#[derive(Default)]
pub struct StaticResourceHandler {
    contents: HashMap<String, ResourceContent>,
}

impl StaticResourceHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, uri: impl Into<String>, content: ResourceContent) {
        self.contents.insert(uri.into(), content);
    }
}

#[async_trait]
impl ResourceHandler for StaticResourceHandler {
    async fn read(&self, uri: &str) -> Result<ResourceContent, McpError> {
        self.contents
            .get(uri)
            .cloned()
            .ok_or_else(|| McpError::ResourceNotFound(uri.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Prompt content & handler
// ---------------------------------------------------------------------------

/// A message in a prompt result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role: "user", "assistant", or "system".
    pub role: String,
    /// Message content.
    pub content: PromptMessageContent,
}

/// Content of a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptMessageContent {
    #[serde(rename = "text")]
    Text { text: String },
}

/// Result returned when getting a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResult {
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The messages that make up the prompt.
    pub messages: Vec<PromptMessage>,
}

/// Async trait for getting prompts.
#[async_trait]
pub trait PromptHandler: Send + Sync {
    /// Get a prompt by name with the given arguments.
    async fn get(&self, name: &str, arguments: Option<Value>) -> Result<PromptResult, McpError>;
}

/// A static prompt handler that returns pre-defined prompts.
#[derive(Default)]
pub struct StaticPromptHandler {
    prompts: HashMap<String, PromptResult>,
}

impl StaticPromptHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl Into<String>, result: PromptResult) {
        self.prompts.insert(name.into(), result);
    }
}

#[async_trait]
impl PromptHandler for StaticPromptHandler {
    async fn get(&self, name: &str, _arguments: Option<Value>) -> Result<PromptResult, McpError> {
        self.prompts
            .get(name)
            .cloned()
            .ok_or_else(|| McpError::PromptNotFound(name.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Logging capability
// ---------------------------------------------------------------------------

/// Logging capability marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapability {
    /// Supported log levels.
    #[serde(default)]
    pub supported_levels: Vec<String>,
}

// ---------------------------------------------------------------------------
// Enhanced ServerCapabilities
// ---------------------------------------------------------------------------

/// Extended server capabilities with logging support.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnhancedServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapability>,
}

impl EnhancedServerCapabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tools(mut self) -> Self {
        self.tools = Some(ToolsCapability {
            list_changed: false,
        });
        self
    }

    pub fn with_resources(mut self) -> Self {
        self.resources = Some(ResourcesCapability {
            subscribe: false,
            list_changed: false,
        });
        self
    }

    pub fn with_prompts(mut self) -> Self {
        self.prompts = Some(PromptsCapability {
            list_changed: false,
        });
        self
    }

    pub fn with_logging(mut self, levels: Vec<String>) -> Self {
        self.logging = Some(LoggingCapability {
            supported_levels: levels,
        });
        self
    }
}

// ---------------------------------------------------------------------------
// Tool entry (definition + handler)
// ---------------------------------------------------------------------------

struct ToolEntry {
    definition: ToolDefinition,
    handler: Arc<dyn ToolHandler>,
}

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// Server identification info.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Enhanced MCP Server with full protocol support.
pub struct McpServer {
    pub info: ServerInfo,
    pub capabilities: EnhancedServerCapabilities,
    /// Protocol versions this server supports (newest first).
    supported_versions: Vec<String>,
    tools: HashMap<String, ToolEntry>,
    resources: HashMap<String, Resource>,
    resource_handler: Option<Arc<dyn ResourceHandler>>,
    prompts: HashMap<String, Prompt>,
    prompt_handler: Option<Arc<dyn PromptHandler>>,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: EnhancedServerCapabilities,
    ) -> Self {
        Self {
            info: ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            capabilities,
            supported_versions: vec!["2024-11-05".to_string()],
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_handler: None,
            prompts: HashMap::new(),
            prompt_handler: None,
        }
    }

    /// Create a new MCP server with custom supported protocol versions.
    /// Versions should be ordered newest-first for proper negotiation.
    pub fn new_with_versions(
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: EnhancedServerCapabilities,
        supported_versions: Vec<String>,
    ) -> Self {
        Self {
            info: ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            capabilities,
            supported_versions,
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_handler: None,
            prompts: HashMap::new(),
            prompt_handler: None,
        }
    }

    /// Get the server's supported protocol versions.
    pub fn supported_versions(&self) -> &[String] {
        &self.supported_versions
    }

    /// Register a tool with an async handler.
    pub fn register_tool(
        &mut self,
        definition: ToolDefinition,
        handler: impl ToolHandler + 'static,
    ) {
        let name = definition.name.clone();
        self.tools.insert(
            name,
            ToolEntry {
                definition,
                handler: Arc::new(handler),
            },
        );
    }

    /// Register a tool using a simple sync closure.
    pub fn register_tool_fn(
        &mut self,
        definition: ToolDefinition,
        handler: impl Fn(Option<Value>) -> Result<ToolResult, String> + Send + Sync + 'static,
    ) {
        self.register_tool(definition, ClosureToolHandler(handler));
    }

    /// Register a resource definition.
    pub fn register_resource(&mut self, resource: Resource) {
        self.resources.insert(resource.uri.clone(), resource);
    }

    /// Set the resource handler for dynamic resource reading.
    pub fn set_resource_handler(&mut self, handler: impl ResourceHandler + 'static) {
        self.resource_handler = Some(Arc::new(handler));
    }

    /// Register a prompt definition.
    pub fn register_prompt(&mut self, prompt: Prompt) {
        self.prompts.insert(prompt.name.clone(), prompt);
    }

    /// Set the prompt handler for dynamic prompt retrieval.
    pub fn set_prompt_handler(&mut self, handler: impl PromptHandler + 'static) {
        self.prompt_handler = Some(Arc::new(handler));
    }

    /// Handle an incoming JSON-RPC request and produce a response.
    pub async fn handle_request(&self, request: &McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "ping" => self.handle_ping(request),
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request),
            "resources/read" => self.handle_resources_read(request).await,
            "prompts/list" => self.handle_prompts_list(request),
            "prompts/get" => self.handle_prompts_get(request).await,
            _ => McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    /// Synchronous request handling for non-async dispatch (convenience).
    pub fn handle_request_sync(&self, request: &McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "ping" => self.handle_ping(request),
            "tools/list" => self.handle_tools_list(request),
            "resources/list" => self.handle_resources_list(request),
            "prompts/list" => self.handle_prompts_list(request),
            _ => McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found (sync): {}", request.method),
            ),
        }
    }

    // -- Internal handlers ---------------------------------------------------

    fn handle_initialize(&self, request: &McpRequest) -> McpResponse {
        let result = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": serde_json::to_value(&self.capabilities).unwrap_or_default(),
            "serverInfo": {
                "name": self.info.name,
                "version": self.info.version
            }
        });
        McpResponse::success(request.id.clone(), result)
    }

    fn handle_ping(&self, request: &McpRequest) -> McpResponse {
        McpResponse::success(request.id.clone(), json!({}))
    }

    fn handle_tools_list(&self, request: &McpRequest) -> McpResponse {
        let tools: Vec<Value> = self
            .tools
            .values()
            .map(|entry| {
                let mut val = json!({
                    "name": entry.definition.name,
                    "description": entry.definition.description,
                    "inputSchema": entry.definition.input_schema,
                });
                if let Some(ref annotations) = entry.definition.annotations {
                    if let Ok(ann_val) = serde_json::to_value(annotations) {
                        val["annotations"] = ann_val;
                    }
                }
                val
            })
            .collect();
        McpResponse::success(request.id.clone(), json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, request: &McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params for tools/call",
                );
            }
        };
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return McpResponse::error(request.id.clone(), -32602, "Missing tool name");
            }
        };
        match self.tools.get(name) {
            Some(entry) => {
                let args = params.get("arguments").cloned();
                match entry.handler.execute(args).await {
                    Ok(result) => {
                        let val = serde_json::to_value(result).unwrap_or_default();
                        McpResponse::success(request.id.clone(), val)
                    }
                    Err(e) => McpResponse::error(
                        request.id.clone(),
                        -32000,
                        format!("Tool execution error: {}", e),
                    ),
                }
            }
            None => McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Tool not found: {}", name),
            ),
        }
    }

    fn handle_resources_list(&self, request: &McpRequest) -> McpResponse {
        let resources: Vec<&Resource> = self.resources.values().collect();
        McpResponse::success(request.id.clone(), json!({ "resources": resources }))
    }

    async fn handle_resources_read(&self, request: &McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params for resources/read",
                );
            }
        };
        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return McpResponse::error(request.id.clone(), -32602, "Missing resource URI");
            }
        };
        // Check the resource is registered
        if !self.resources.contains_key(uri) {
            return McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Resource not found: {}", uri),
            );
        }
        // Try the handler
        match &self.resource_handler {
            Some(handler) => match handler.read(uri).await {
                Ok(content) => McpResponse::success(
                    request.id.clone(),
                    json!({
                        "contents": [content]
                    }),
                ),
                Err(e) => McpResponse::error(
                    request.id.clone(),
                    -32000,
                    format!("Resource read error: {}", e),
                ),
            },
            None => {
                McpResponse::error(request.id.clone(), -32601, "No resource handler configured")
            }
        }
    }

    fn handle_prompts_list(&self, request: &McpRequest) -> McpResponse {
        let prompts: Vec<&Prompt> = self.prompts.values().collect();
        McpResponse::success(request.id.clone(), json!({ "prompts": prompts }))
    }

    async fn handle_prompts_get(&self, request: &McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params for prompts/get",
                );
            }
        };
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return McpResponse::error(request.id.clone(), -32602, "Missing prompt name");
            }
        };
        // Check the prompt is registered
        if !self.prompts.contains_key(name) {
            return McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Prompt not found: {}", name),
            );
        }
        // Try the handler
        match &self.prompt_handler {
            Some(handler) => {
                let args = params.get("arguments").cloned();
                match handler.get(name, args).await {
                    Ok(result) => McpResponse::success(
                        request.id.clone(),
                        serde_json::to_value(result).unwrap_or_default(),
                    ),
                    Err(e) => McpResponse::error(
                        request.id.clone(),
                        -32000,
                        format!("Prompt error: {}", e),
                    ),
                }
            }
            None => McpResponse::error(request.id.clone(), -32601, "No prompt handler configured"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RequestId;
    use serde_json::json;

    /// Helper: create a test server with sample tools, resources, prompts.
    fn make_test_server() -> McpServer {
        let caps = EnhancedServerCapabilities::new()
            .with_tools()
            .with_resources()
            .with_prompts()
            .with_logging(vec![
                "debug".into(),
                "info".into(),
                "warn".into(),
                "error".into(),
            ]);

        let mut server = McpServer::new("test-server", "1.0.0", caps);

        // Register echo tool
        server.register_tool_fn(
            ToolDefinition::new(
                "echo",
                "Echo back the input",
                json!({"type": "object", "properties": {"text": {"type": "string"}}}),
            ),
            |args| {
                let text = args
                    .as_ref()
                    .and_then(|a| a.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(ToolResult::text(format!("echoed: {}", text)))
            },
        );

        // Register error tool
        server.register_tool_fn(
            ToolDefinition::new("fail", "Always fails", json!({"type": "object"})),
            |_| Err("intentional failure".to_string()),
        );

        // Register resource
        server.register_resource(
            Resource::new(
                "file:///tmp/test.txt",
                "test.txt",
                Some("A test file".to_string()),
            )
            .with_mime_type("text/plain"),
        );

        // Set resource handler
        let mut rh = StaticResourceHandler::new();
        rh.add(
            "file:///tmp/test.txt",
            ResourceContent::text("file:///tmp/test.txt", "Hello, world!"),
        );
        server.set_resource_handler(rh);

        // Register prompt
        server.register_prompt(
            Prompt::new("summarize", Some("Summarize text".to_string())).with_argument(
                crate::prompt::PromptArgument::new(
                    "text",
                    Some("Text to summarize".to_string()),
                    true,
                ),
            ),
        );

        // Set prompt handler
        let mut ph = StaticPromptHandler::new();
        ph.add(
            "summarize",
            PromptResult {
                description: Some("Summarize text".to_string()),
                messages: vec![PromptMessage {
                    role: "user".to_string(),
                    content: PromptMessageContent::Text {
                        text: "Please summarize the following text.".to_string(),
                    },
                }],
            },
        );
        server.set_prompt_handler(ph);

        server
    }

    // 1. Tool registration and listing
    #[tokio::test]
    async fn test_tool_registration_and_listing() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(1), "tools/list", None);
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 2); // echo + fail
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"fail"));
    }

    // 2. Tool execution - success
    #[tokio::test]
    async fn test_tool_execution_success() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(2),
            "tools/call",
            Some(json!({"name": "echo", "arguments": {"text": "hello"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result: ToolResult = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!result.is_error);
        match &result.content[0] {
            ToolResultContent::Text { text } => assert_eq!(text, "echoed: hello"),
            _ => panic!("Expected text content"),
        }
    }

    // 3. Tool execution - error from handler
    #[tokio::test]
    async fn test_tool_execution_handler_error() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(3),
            "tools/call",
            Some(json!({"name": "fail"})),
        );
        let resp = server.handle_request(&req).await;
        // The tool returns Err which maps to a JSON-RPC error
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32000);
    }

    // 4. Tool execution - unknown tool
    #[tokio::test]
    async fn test_tool_execution_unknown() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(4),
            "tools/call",
            Some(json!({"name": "nonexistent"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 5. Tool execution - missing params
    #[tokio::test]
    async fn test_tool_call_missing_params() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(5), "tools/call", None);
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    // 6. Tool execution - missing tool name
    #[tokio::test]
    async fn test_tool_call_missing_name() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(6), "tools/call", Some(json!({})));
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    // 7. Resource listing
    #[tokio::test]
    async fn test_resource_listing() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(7), "resources/list", None);
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let resources = resp.result.unwrap()["resources"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0]["name"], "test.txt");
    }

    // 8. Resource reading
    #[tokio::test]
    async fn test_resource_reading() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(8),
            "resources/read",
            Some(json!({"uri": "file:///tmp/test.txt"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["text"], "Hello, world!");
    }

    // 9. Resource reading - not found
    #[tokio::test]
    async fn test_resource_reading_not_found() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(9),
            "resources/read",
            Some(json!({"uri": "file:///nonexistent"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 10. Resource reading - missing URI
    #[tokio::test]
    async fn test_resource_reading_missing_uri() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(10), "resources/read", Some(json!({})));
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    // 11. Prompt listing
    #[tokio::test]
    async fn test_prompt_listing() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(11), "prompts/list", None);
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let prompts = resp.result.unwrap()["prompts"].as_array().unwrap().clone();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0]["name"], "summarize");
    }

    // 12. Prompt getting
    #[tokio::test]
    async fn test_prompt_getting() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(12),
            "prompts/get",
            Some(json!({"name": "summarize", "arguments": {"text": "hello world"}})),
        );
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    // 13. Prompt getting - not found
    #[tokio::test]
    async fn test_prompt_getting_not_found() {
        let server = make_test_server();
        let req = McpRequest::new(
            RequestId::Number(13),
            "prompts/get",
            Some(json!({"name": "nonexistent"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 14. Initialize handshake
    #[tokio::test]
    async fn test_initialize_handshake() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(14), "initialize", None);
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "test-server");
        assert_eq!(result["serverInfo"]["version"], "1.0.0");
        // Check capabilities include tools, resources, prompts
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["prompts"].is_object());
        assert!(result["capabilities"]["logging"].is_object());
    }

    // 15. Ping
    #[tokio::test]
    async fn test_ping() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(15), "ping", None);
        let resp = server.handle_request(&req).await;
        assert!(!resp.is_error());
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    // 16. Unknown method
    #[tokio::test]
    async fn test_unknown_method() {
        let server = make_test_server();
        let req = McpRequest::new(RequestId::Number(16), "unknown/method", None);
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 17. Server capabilities serialization
    #[tokio::test]
    async fn test_server_capabilities() {
        let caps = EnhancedServerCapabilities::new()
            .with_tools()
            .with_resources()
            .with_prompts()
            .with_logging(vec!["info".into()]);
        let val = serde_json::to_value(&caps).unwrap();
        assert!(val["tools"].is_object());
        assert!(val["resources"].is_object());
        assert!(val["prompts"].is_object());
        assert!(val["logging"].is_object());
        assert_eq!(val["logging"]["supported_levels"][0], "info");
    }

    // 18. Tool annotations
    #[tokio::test]
    async fn test_tool_annotations() {
        let mut server = McpServer::new(
            "annotated-server",
            "1.0.0",
            EnhancedServerCapabilities::new().with_tools(),
        );
        let ann = ToolAnnotations {
            title: Some("Annotated Tool".to_string()),
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        };
        server.register_tool_fn(
            ToolDefinition::new(
                "annotated",
                "A tool with annotations",
                json!({"type": "object"}),
            )
            .with_annotations(ann),
            |_| Ok(ToolResult::text("ok")),
        );
        let req = McpRequest::new(RequestId::Number(1), "tools/list", None);
        let resp = server.handle_request(&req).await;
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["annotations"]["title"], "Annotated Tool");
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
    }

    // 19. ToolResult construction
    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("hello");
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("oops");
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    // 20. ResourceContent construction
    #[test]
    fn test_resource_content_text() {
        let rc = ResourceContent::text("file:///a", "content");
        assert_eq!(rc.uri, "file:///a");
        assert_eq!(rc.text.as_deref(), Some("content"));
        assert!(rc.blob.is_none());
    }

    // 21. PromptMessage serialization
    #[test]
    fn test_prompt_message_serialization() {
        let msg = PromptMessage {
            role: "user".to_string(),
            content: PromptMessageContent::Text {
                text: "hello".to_string(),
            },
        };
        let val = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["role"], "user");
        assert_eq!(val["content"]["type"], "text");
        assert_eq!(val["content"]["text"], "hello");
    }

    // 22. JSON-RPC request/response format
    #[test]
    fn test_jsonrpc_request_format() {
        let req = McpRequest::new(
            RequestId::Number(42),
            "initialize",
            Some(json!({"key": "val"})),
        );
        let serialized = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["method"], "initialize");
        assert_eq!(parsed["params"]["key"], "val");
    }

    // 23. No resource handler configured
    #[tokio::test]
    async fn test_no_resource_handler() {
        let mut server = McpServer::new(
            "no-handler",
            "1.0.0",
            EnhancedServerCapabilities::new().with_resources(),
        );
        server.register_resource(Resource::new("file:///a", "a", None));
        let req = McpRequest::new(
            RequestId::Number(1),
            "resources/read",
            Some(json!({"uri": "file:///a"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 24. No prompt handler configured
    #[tokio::test]
    async fn test_no_prompt_handler() {
        let mut server = McpServer::new(
            "no-handler",
            "1.0.0",
            EnhancedServerCapabilities::new().with_prompts(),
        );
        server.register_prompt(Prompt::new("test", None));
        let req = McpRequest::new(
            RequestId::Number(1),
            "prompts/get",
            Some(json!({"name": "test"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    // 25. ToolDefinition construction
    #[test]
    fn test_tool_definition() {
        let td = ToolDefinition::new("test", "desc", json!({})).with_annotations(ToolAnnotations {
            title: Some("Test".to_string()),
            ..Default::default()
        });
        assert_eq!(td.name, "test");
        assert_eq!(td.description, "desc");
        assert!(td.annotations.is_some());
        assert_eq!(td.annotations.unwrap().title.unwrap(), "Test");
    }
}
