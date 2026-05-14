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
