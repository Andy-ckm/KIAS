//! 工具注册表

use crate::builtin::{Tool, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;

/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Arc<Box<dyn Tool>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册工具
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }

    /// 获取工具
    pub fn get(&self, name: &str) -> Option<Arc<Box<dyn Tool>>> {
        self.tools.get(name).cloned()
    }

    /// 列出所有工具
    pub fn list(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect()
    }

    /// 执行工具
    pub async fn execute(&self, name: &str, params: serde_json::Value) -> ToolResult {
        if let Some(tool) = self.get(name) {
            tool.execute(params).await
        } else {
            ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Tool not found: {}", name)),
                metadata: None,
            }
        }
    }

    /// 创建包含所有内置工具的注册表
    pub fn with_builtin() -> Self {
        let mut registry = Self::new();
        for tool in crate::builtin::get_builtin_tools() {
            registry.register(tool);
        }
        registry
    }
}

/// 工具信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
