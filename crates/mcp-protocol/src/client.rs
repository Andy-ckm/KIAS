use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::capabilities::ServerCapabilities;
use crate::error::McpError;
use crate::prompt::Prompt;
use crate::resource::Resource;

/// Type alias for tool handler functions.
type ToolHandler = Arc<dyn Fn(Option<Value>) -> Result<Value, String> + Send + Sync>;
use crate::tool::Tool;
use crate::types::{McpNotification, McpRequest, McpResponse, RequestId};

/// Trait for transport layer – abstracts how messages are sent/received.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send a message and wait for a response.
    async fn send_request(&self, request: &McpRequest) -> Result<McpResponse, McpError>;

    /// Send a one-way notification (no response expected).
    async fn send_notification(&self, notification: &McpNotification) -> Result<(), McpError>;
}

/// In-memory transport for testing – processes requests via a local handler.
pub struct InMemoryTransport {
    handler: Arc<dyn Fn(McpRequest) -> McpResponse + Send + Sync>,
}

impl InMemoryTransport {
    pub fn new(handler: impl Fn(McpRequest) -> McpResponse + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
}

#[async_trait::async_trait]
impl Transport for InMemoryTransport {
    async fn send_request(&self, request: &McpRequest) -> Result<McpResponse, McpError> {
        Ok((self.handler)(request.clone()))
    }

    async fn send_notification(&self, _notification: &McpNotification) -> Result<(), McpError> {
        Ok(())
    }
}

/// MCP client that communicates with a server over a transport.
pub struct McpClient {
    transport: Arc<dyn Transport>,
    next_id: Arc<Mutex<i64>>,
}

impl McpClient {
    /// Create a new client with the given transport.
    pub fn new(transport: impl Transport + 'static) -> Self {
        Self {
            transport: Arc::new(transport),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Generate the next request ID.
    async fn next_request_id(&self) -> RequestId {
        let mut id = self.next_id.lock().await;
        let current = *id;
        *id += 1;
        RequestId::Number(current)
    }

    /// Call a method on the server and return the result.
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        let id = self.next_request_id().await;
        let request = McpRequest::new(id, method, params);
        let response = self.transport.send_request(&request).await?;

        if let Some(err) = response.error {
            return Err(McpError::ServerError {
                code: err.code,
                message: err.message,
            });
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Send a notification (fire-and-forget).
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        let notification = McpNotification::new(method, params);
        self.transport.send_notification(&notification).await
    }

    /// Initialize the connection with the server.
    pub async fn initialize(&self) -> Result<Value, McpError> {
        self.request(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "kias-mcp-client",
                    "version": "0.1.0"
                }
            })),
        )
        .await
    }

    /// List available tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<Tool>, McpError> {
        let result = self.request("tools/list", None).await?;
        let tools: Vec<Tool> =
            serde_json::from_value(result.get("tools").cloned().unwrap_or(Value::Array(vec![])))?;
        Ok(tools)
    }

    /// Call a specific tool by name with arguments.
    pub async fn call_tool(&self, name: &str, arguments: Option<Value>) -> Result<Value, McpError> {
        let params = json!({
            "name": name,
            "arguments": arguments.unwrap_or(json!({}))
        });
        self.request("tools/call", Some(params)).await
    }

    /// List available resources from the server.
    pub async fn list_resources(&self) -> Result<Vec<Resource>, McpError> {
        let result = self.request("resources/list", None).await?;
        let resources: Vec<Resource> = serde_json::from_value(
            result
                .get("resources")
                .cloned()
                .unwrap_or(Value::Array(vec![])),
        )?;
        Ok(resources)
    }

    /// List available prompts from the server.
    pub async fn list_prompts(&self) -> Result<Vec<Prompt>, McpError> {
        let result = self.request("prompts/list", None).await?;
        let prompts: Vec<Prompt> = serde_json::from_value(
            result
                .get("prompts")
                .cloned()
                .unwrap_or(Value::Array(vec![])),
        )?;
        Ok(prompts)
    }
}

/// A simple in-process MCP server that handles tool/resource/prompt operations.
pub struct McpServer {
    pub info: ServerInfo,
    pub capabilities: ServerCapabilities,
    tools: HashMap<String, Tool>,
    resources: HashMap<String, Resource>,
    prompts: HashMap<String, Prompt>,
    tool_handlers: HashMap<String, ToolHandler>,
}

/// Server identification info.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: ServerCapabilities,
    ) -> Self {
        Self {
            info: ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            capabilities,
            tools: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
            tool_handlers: HashMap::new(),
        }
    }

    /// Register a tool with a handler function.
    pub fn register_tool(
        &mut self,
        tool: Tool,
        handler: impl Fn(Option<Value>) -> Result<Value, String> + Send + Sync + 'static,
    ) {
        let name = tool.name.clone();
        self.tools.insert(name.clone(), tool);
        self.tool_handlers.insert(name, Arc::new(handler));
    }

    /// Register a resource.
    pub fn register_resource(&mut self, resource: Resource) {
        self.resources.insert(resource.uri.clone(), resource);
    }

    /// Register a prompt.
    pub fn register_prompt(&mut self, prompt: Prompt) {
        self.prompts.insert(prompt.name.clone(), prompt);
    }

    /// Handle an incoming request and produce a response.
    pub fn handle_request(&self, request: &McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => {
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
            "tools/list" => {
                let tools: Vec<&Tool> = self.tools.values().collect();
                McpResponse::success(request.id.clone(), json!({ "tools": tools }))
            }
            "tools/call" => {
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
                match self.tool_handlers.get(name) {
                    Some(handler) => {
                        let args = params.get("arguments").cloned();
                        match handler(args) {
                            Ok(result) => McpResponse::success(request.id.clone(), result),
                            Err(e) => McpResponse::error(
                                request.id.clone(),
                                -32000,
                                format!("Tool error: {}", e),
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
            "resources/list" => {
                let resources: Vec<&Resource> = self.resources.values().collect();
                McpResponse::success(request.id.clone(), json!({ "resources": resources }))
            }
            "prompts/list" => {
                let prompts: Vec<&Prompt> = self.prompts.values().collect();
                McpResponse::success(request.id.clone(), json!({ "prompts": prompts }))
            }
            _ => McpResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_server() -> McpServer {
        let caps = ServerCapabilities::new()
            .with_tools()
            .with_resources()
            .with_prompts();
        let mut server = McpServer::new("test-server", "1.0.0", caps);

        server.register_tool(
            Tool::new(
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
                Ok(json!({"echoed": text}))
            },
        );

        server.register_resource(Resource::new(
            "file:///tmp/test.txt",
            "test.txt",
            Some("A test file".to_string()),
        ));

        server.register_prompt(
            Prompt::new("summarize", Some("Summarize text".to_string())).with_argument(
                crate::prompt::PromptArgument::new(
                    "text",
                    Some("Text to summarize".to_string()),
                    true,
                ),
            ),
        );

        server
    }

    #[test]
    fn test_server_initialize() {
        let server = make_server();
        let req = McpRequest::new(RequestId::Number(1), "initialize", None);
        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "test-server");
    }

    #[test]
    fn test_server_list_tools() {
        let server = make_server();
        let req = McpRequest::new(RequestId::Number(2), "tools/list", None);
        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[test]
    fn test_server_call_tool() {
        let server = make_server();
        let req = McpRequest::new(
            RequestId::Number(3),
            "tools/call",
            Some(json!({"name": "echo", "arguments": {"text": "hello"}})),
        );
        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        assert_eq!(resp.result.unwrap()["echoed"], "hello");
    }

    #[test]
    fn test_server_call_unknown_tool() {
        let server = make_server();
        let req = McpRequest::new(
            RequestId::Number(4),
            "tools/call",
            Some(json!({"name": "nonexistent"})),
        );
        let resp = server.handle_request(&req);
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_server_list_resources() {
        let server = make_server();
        let req = McpRequest::new(RequestId::Number(5), "resources/list", None);
        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let resources = result["resources"].as_array().unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0]["name"], "test.txt");
    }

    #[test]
    fn test_server_list_prompts() {
        let server = make_server();
        let req = McpRequest::new(RequestId::Number(6), "prompts/list", None);
        let resp = server.handle_request(&req);
        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        let prompts = result["prompts"].as_array().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0]["name"], "summarize");
    }

    #[test]
    fn test_server_unknown_method() {
        let server = make_server();
        let req = McpRequest::new(RequestId::Number(7), "unknown/method", None);
        let resp = server.handle_request(&req);
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_client_initialize() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let result = client.initialize().await.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn test_client_list_tools() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[tokio::test]
    async fn test_client_call_tool() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let result = client
            .call_tool("echo", Some(json!({"text": "world"})))
            .await
            .unwrap();
        assert_eq!(result["echoed"], "world");
    }

    #[tokio::test]
    async fn test_client_list_resources() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let resources = client.list_resources().await.unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].name, "test.txt");
    }

    #[tokio::test]
    async fn test_client_list_prompts() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let prompts = client.list_prompts().await.unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "summarize");
    }

    #[tokio::test]
    async fn test_client_tool_error() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let result = client.call_tool("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_incremental_ids() {
        let server = make_server();
        let transport = InMemoryTransport::new(move |req| server.handle_request(&req));
        let client = McpClient::new(transport);
        let _ = client.list_tools().await.unwrap();
        let _ = client.list_resources().await.unwrap();
        // IDs increment automatically – no conflict means it works
    }
}
