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
}
