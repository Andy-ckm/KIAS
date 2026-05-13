use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::JSONRPC_VERSION;

/// Request identifier – either a string or integer per JSON-RPC 2.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

/// A JSON-RPC 2.0 Request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl McpRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 Response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpResponse {
    pub fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: RequestId, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Returns `true` if this response represents an error.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// A JSON-RPC 2.0 Notification (a Request without an `id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl McpNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_creation() {
        let req = McpRequest::new(RequestId::Number(1), "tools/list", None);
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, RequestId::Number(1));
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_with_params() {
        let params = json!({"name": "test"});
        let req = McpRequest::new(RequestId::String("abc".into()), "tools/call", Some(params.clone()));
        assert_eq!(req.params, Some(params));
        assert_eq!(req.id, RequestId::String("abc".into()));
    }

    #[test]
    fn test_response_success() {
        let resp = McpResponse::success(RequestId::Number(1), json!({"tools": []}));
        assert!(!resp.is_error());
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let resp = McpResponse::error(RequestId::Number(2), -32601, "Method not found");
        assert!(resp.is_error());
        assert!(resp.result.is_none());
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_notification_creation() {
        let notif = McpNotification::new("notifications/progress", Some(json!({"progress": 50})));
        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "notifications/progress");
    }

    #[test]
    fn test_request_serialization_roundtrip() {
        let req = McpRequest::new(
            RequestId::Number(42),
            "initialize",
            Some(json!({"protocolVersion": "2024-11-05"})),
        );
        let json_str = serde_json::to_string(&req).unwrap();
        let deserialized: McpRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.id, RequestId::Number(42));
        assert_eq!(deserialized.method, "initialize");
    }

    #[test]
    fn test_response_serialization_roundtrip() {
        let resp = McpResponse::success(RequestId::String("x".into()), json!(true));
        let json_str = serde_json::to_string(&resp).unwrap();
        let deserialized: McpResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.id, RequestId::String("x".into()));
        assert_eq!(deserialized.result, Some(json!(true)));
    }
}
