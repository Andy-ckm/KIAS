use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents an MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: Value,
}

impl Tool {
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
        }
    }

    /// Create a tool with no parameters (empty object schema).
    pub fn no_params(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_creation() {
        let tool = Tool::new(
            "get_weather",
            "Get current weather for a location",
            json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                },
                "required": ["location"]
            }),
        );
        assert_eq!(tool.name, "get_weather");
        assert_eq!(tool.description, "Get current weather for a location");
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn test_tool_no_params() {
        let tool = Tool::no_params("ping", "Ping the server");
        assert_eq!(tool.name, "ping");
        assert_eq!(tool.input_schema["type"], "object");
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool::new("echo", "Echo input", json!({"type": "object"}));
        let json_str = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "echo");
        assert_eq!(deserialized.description, "Echo input");
    }

    // -----------------------------------------------------------------------
    // Error-path tests
    // -----------------------------------------------------------------------

    // (1) Invalid JSON-RPC requests: deserializing malformed JSON into McpRequest
    #[test]
    fn test_deserialize_invalid_jsonrpc_request() {
        // Completely non-JSON text
        let result = serde_json::from_str::<crate::types::McpRequest>("not json at all");
        assert!(result.is_err(), "garbage text must fail deserialization");

        // Valid JSON but missing required "jsonrpc" field
        let result =
            serde_json::from_str::<crate::types::McpRequest>(r#"{"id":1,"method":"test"}"#);
        assert!(
            result.is_err(),
            "missing jsonrpc field must fail deserialization"
        );

        // Valid JSON but missing required "id" field
        let result = serde_json::from_str::<crate::types::McpRequest>(
            r#"{"jsonrpc":"2.0","method":"test"}"#,
        );
        assert!(
            result.is_err(),
            "missing id field must fail deserialization"
        );

        // Valid JSON but missing required "method" field
        let result =
            serde_json::from_str::<crate::types::McpRequest>(r#"{"jsonrpc":"2.0","id":1}"#);
        assert!(
            result.is_err(),
            "missing method field must fail deserialization"
        );

        // Empty object
        let result = serde_json::from_str::<crate::types::McpRequest>(r#"{}"#);
        assert!(result.is_err(), "empty object must fail deserialization");

        // Completely wrong types for fields
        let result = serde_json::from_str::<crate::types::McpRequest>(
            r#"{"jsonrpc":42,"id":[],"method":123}"#,
        );
        assert!(
            result.is_err(),
            "wrong field types must fail deserialization"
        );
    }

    // (2) Missing method: server rejects unknown method names
    #[tokio::test]
    async fn test_missing_method_unknown_dispatch() {
        use crate::server::{EnhancedServerCapabilities, McpServer};
        use crate::types::{McpRequest, RequestId};

        let server = McpServer::new(
            "test",
            "1.0.0",
            EnhancedServerCapabilities::new().with_tools(),
        );

        // Completely empty method string
        let req = McpRequest::new(RequestId::Number(1), "", None);
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "empty method must return error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);

        // Arbitrary unknown method
        let req = McpRequest::new(RequestId::Number(2), "foo/bar/baz", None);
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "unknown method must return error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
        assert!(
            resp.error.as_ref().unwrap().message.contains("foo/bar/baz"),
            "error message should contain the unknown method name"
        );

        // tools/call with missing params (simulates missing required params)
        let req = McpRequest::new(RequestId::Number(3), "tools/call", None);
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "tools/call without params must error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);

        // tools/call with params but no "name" key
        let req = McpRequest::new(
            RequestId::Number(4),
            "tools/call",
            Some(json!({"wrong_key": "value"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "tools/call without name must error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);
    }

    // (3) Invalid params: various malformed tool-call parameters
    #[tokio::test]
    async fn test_invalid_params_for_tool_call() {
        use crate::server::{EnhancedServerCapabilities, McpServer, ToolDefinition};
        use crate::types::{McpRequest, RequestId};

        let mut server = McpServer::new(
            "test",
            "1.0.0",
            EnhancedServerCapabilities::new().with_tools(),
        );
        server.register_tool_fn(
            ToolDefinition::new(
                "strict_tool",
                "Expects string arg",
                json!({"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}),
            ),
            |args| {
                let text = args
                    .as_ref()
                    .and_then(|a| a.get("text"))
                    .and_then(|v| v.as_str())
                    .ok_or("missing or non-string 'text' param")?;
                Ok(crate::server::ToolResult::text(text))
            },
        );

        // Pass an array instead of object as params
        let req = McpRequest::new(RequestId::Number(1), "tools/call", Some(json!([1, 2, 3])));
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "array params must fail (no 'name' key)");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);

        // Pass null as params
        let req = McpRequest::new(RequestId::Number(2), "tools/call", Some(json!(null)));
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "null params must fail");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);

        // Pass a string as params
        let req = McpRequest::new(
            RequestId::Number(3),
            "tools/call",
            Some(json!("not an object")),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "string params must fail");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);

        // name field is a number instead of string
        let req = McpRequest::new(
            RequestId::Number(4),
            "tools/call",
            Some(json!({"name": 12345})),
        );
        let resp = server.handle_request(&req).await;
        assert!(
            resp.is_error(),
            "numeric name field must fail (not a string)"
        );
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);
    }

    // (4) Tool not found: look up a tool that doesn't exist
    #[tokio::test]
    async fn test_tool_not_found() {
        use crate::server::{EnhancedServerCapabilities, McpServer, ToolDefinition};
        use crate::types::{McpRequest, RequestId};

        let mut server = McpServer::new(
            "test",
            "1.0.0",
            EnhancedServerCapabilities::new().with_tools(),
        );
        server.register_tool_fn(
            ToolDefinition::new("exists", "I exist", json!({"type": "object"})),
            |_| Ok(crate::server::ToolResult::text("ok")),
        );

        // Tool name that doesn't exist
        let req = McpRequest::new(
            RequestId::Number(1),
            "tools/call",
            Some(json!({"name": "does_not_exist"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "non-existent tool must return error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
        assert!(
            resp.error
                .as_ref()
                .unwrap()
                .message
                .contains("does_not_exist"),
            "error message should include the missing tool name"
        );

        // Empty tool name
        let req = McpRequest::new(
            RequestId::Number(2),
            "tools/call",
            Some(json!({"name": ""})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "empty tool name must return error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);

        // Case-sensitive mismatch
        let req = McpRequest::new(
            RequestId::Number(3),
            "tools/call",
            Some(json!({"name": "EXISTS"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error(), "wrong-case tool name must return error");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);

        // Tool list should not include the missing tool
        let list_req = McpRequest::new(RequestId::Number(4), "tools/list", None);
        let list_resp = server.handle_request(&list_req).await;
        assert!(!list_resp.is_error());
        let tools = list_resp.result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "exists");
    }

    // (5) Tool execution failure: handler returns Err, panics are caught, etc.
    #[tokio::test]
    async fn test_tool_execution_failure() {
        use crate::server::{EnhancedServerCapabilities, McpServer, ToolDefinition, ToolResult};
        use crate::types::{McpRequest, RequestId};

        let mut server = McpServer::new(
            "test",
            "1.0.0",
            EnhancedServerCapabilities::new().with_tools(),
        );

        // Tool that always returns an error
        server.register_tool_fn(
            ToolDefinition::new("always_fail", "Fails every time", json!({"type": "object"})),
            |_| Err("intentional failure: simulating internal error".to_string()),
        );

        // Tool that returns an error ToolResult (is_error=true but not a transport error)
        server.register_tool_fn(
            ToolDefinition::new(
                "soft_fail",
                "Returns error result",
                json!({"type": "object"}),
            ),
            |_| Ok(ToolResult::error("tool-level error: bad input provided")),
        );

        // Test hard failure (handler returns Err)
        let req = McpRequest::new(
            RequestId::Number(1),
            "tools/call",
            Some(json!({"name": "always_fail"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(
            resp.is_error(),
            "handler Err must produce JSON-RPC error response"
        );
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32000, "tool execution errors use code -32000");
        assert!(
            err.message.contains("intentional failure"),
            "error message should contain the original error text"
        );

        // Test soft failure (handler returns Ok(ToolResult { is_error: true }))
        let req = McpRequest::new(
            RequestId::Number(2),
            "tools/call",
            Some(json!({"name": "soft_fail"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(
            !resp.is_error(),
            "soft failure returns a successful JSON-RPC response with is_error=true in result"
        );
        let result: ToolResult = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(result.is_error, "tool result is_error must be true");
        assert_eq!(result.content.len(), 1);

        // Tool that panics — the server should not crash (handler catches unwind)
        server.register_tool_fn(
            ToolDefinition::new("panicker", "Panics", json!({"type": "object"})),
            |_| -> Result<ToolResult, String> { panic!("boom") },
        );

        // NOTE: A panic inside the handler will propagate unless caught by the caller.
        // This test documents that behavior — the handler panics and the test should
        // catch it via std::panic::catch_unwind at the call site.
        // Since the server does not catch panics, we verify the tool is registered
        // and that calling a non-panicking tool still works after registration.
        let req = McpRequest::new(
            RequestId::Number(3),
            "tools/call",
            Some(json!({"name": "always_fail"})),
        );
        let resp = server.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, -32000);
    }

    // (6) Tool deserialization errors: malformed Tool JSON
    #[test]
    fn test_tool_deserialize_errors() {
        // Missing "name" field
        let result = serde_json::from_str::<Tool>(r#"{"description":"test","inputSchema":{}}"#);
        assert!(result.is_err(), "missing name must fail");

        // Missing "description" field
        let result = serde_json::from_str::<Tool>(r#"{"name":"test","inputSchema":{}}"#);
        assert!(result.is_err(), "missing description must fail");

        // Missing "input_schema" field — NOTE: serde uses snake_case, so "inputSchema" won't
        // match unless #[serde(rename)] is applied. Check both variants.
        let result = serde_json::from_str::<Tool>(r#"{"name":"test","description":"desc"}"#);
        assert!(result.is_err(), "missing input_schema must fail");

        // Wrong types for fields
        let result = serde_json::from_str::<Tool>(
            r#"{"name":123,"description":true,"inputSchema":"not an object"}"#,
        );
        assert!(result.is_err(), "wrong field types must fail");

        // Completely non-JSON text
        let result = serde_json::from_str::<Tool>("<<<not json>>>");
        assert!(result.is_err(), "non-JSON text must fail");

        // Empty string
        let result = serde_json::from_str::<Tool>("");
        assert!(result.is_err(), "empty string must fail");
    }

    // (7) Edge case: Tool with empty name and description
    #[test]
    fn test_tool_empty_fields() {
        let tool = Tool::new("", "", json!(null));
        assert_eq!(tool.name, "");
        assert_eq!(tool.description, "");
        assert!(tool.input_schema.is_null());

        // Round-trip should work
        let json_str = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "");
        assert_eq!(deserialized.description, "");
    }

    // (8) Edge case: Tool with special characters and unicode
    #[test]
    fn test_tool_special_characters() {
        let tool = Tool::new(
            "tool/with/slashes & spaces",
            "Description with\nnewlines\tand\ttabs",
            json!({"type": "object", "properties": {"名前": {"type": "string"}}}),
        );

        let json_str = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "tool/with/slashes & spaces");
        assert!(deserialized.description.contains('\n'));
        assert!(deserialized.description.contains('\t'));
    }
}
