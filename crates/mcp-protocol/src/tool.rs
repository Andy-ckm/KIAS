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
}
