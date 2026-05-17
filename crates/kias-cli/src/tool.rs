//! 工具管理模块

use serde::{Deserialize, Serialize};

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub tool_type: ToolType,
    pub config: ToolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolType {
    Mcp,
    FunctionCall,
    Http,
    Shell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub endpoint: Option<String>,
    pub command: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_parse() {
        let json = r#"{
            "name": "web_search",
            "description": "Search the web",
            "tool_type": "Mcp",
            "config": {"endpoint": "http://localhost:3000", "command": null, "parameters": null}
        }"#;
        let tool: ToolDefinition = serde_json::from_str(json).expect("should parse");
        assert_eq!(tool.name, "web_search");
        assert_eq!(tool.description, "Search the web");
    }

    #[test]
    fn test_tool_config_empty() {
        let json = r#"{"endpoint": null, "command": null, "parameters": null}"#;
        let config: ToolConfig = serde_json::from_str(json).expect("should parse");
        assert!(config.endpoint.is_none());
        assert!(config.command.is_none());
    }

    #[test]
    fn test_tool_type_variants() {
        let json_mcp = r#"{"name":"t","description":"d","tool_type":"Mcp","config":{"endpoint":null,"command":null,"parameters":null}}"#;
        let tool: ToolDefinition = serde_json::from_str(json_mcp).unwrap();
        assert!(matches!(tool.tool_type, ToolType::Mcp));

        let json_fc = json_mcp.replace("Mcp", "FunctionCall");
        let tool: ToolDefinition = serde_json::from_str(&json_fc).unwrap();
        assert!(matches!(tool.tool_type, ToolType::FunctionCall));

        let json_http = json_mcp.replace("Mcp", "Http");
        let tool: ToolDefinition = serde_json::from_str(&json_http).unwrap();
        assert!(matches!(tool.tool_type, ToolType::Http));

        let json_shell = json_mcp.replace("Mcp", "Shell");
        let tool: ToolDefinition = serde_json::from_str(&json_shell).unwrap();
        assert!(matches!(tool.tool_type, ToolType::Shell));
    }

    #[test]
    fn test_tool_config_with_endpoint() {
        let json = r#"{"endpoint": "http://localhost:3000", "command": null, "parameters": {"key": "value"}}"#;
        let config: ToolConfig = serde_json::from_str(json).expect("should parse");
        assert_eq!(config.endpoint.as_deref(), Some("http://localhost:3000"));
        assert!(config.parameters.is_some());
    }

    #[test]
    fn test_tool_config_with_command() {
        let json = r#"{"endpoint": null, "command": "echo hello", "parameters": null}"#;
        let config: ToolConfig = serde_json::from_str(json).expect("should parse");
        assert_eq!(config.command.as_deref(), Some("echo hello"));
    }

    #[test]
    fn test_tool_definition_clone_debug() {
        let json = r#"{"name":"t","description":"d","tool_type":"Shell","config":{"endpoint":null,"command":"ls","parameters":null}}"#;
        let tool: ToolDefinition = serde_json::from_str(json).unwrap();
        let cloned = tool.clone();
        assert_eq!(cloned.name, "t");
        assert!(matches!(cloned.tool_type, ToolType::Shell));
        let _debug = format!("{:?}", cloned);
    }
}
