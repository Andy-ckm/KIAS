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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::ToolResult;
    use async_trait::async_trait;

    /// Mock tool for testing the registry
    struct MockTool {
        tool_name: String,
        tool_description: String,
    }

    impl MockTool {
        fn new(name: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                tool_description: format!("Mock tool: {name}"),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            &self.tool_description
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }

        async fn execute(&self, _params: serde_json::Value) -> ToolResult {
            ToolResult {
                success: true,
                output: format!("executed:{}", self.tool_name),
                error: None,
                metadata: None,
            }
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_default_trait() {
        let registry = ToolRegistry::default();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("alpha")));

        let tool = registry.get("alpha");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "alpha");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_multiple_and_list() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("a")));
        registry.register(Box::new(MockTool::new("b")));
        registry.register(Box::new(MockTool::new("c")));

        let list = registry.list();
        assert_eq!(list.len(), 3);

        let names: Vec<&str> = list.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }

    #[test]
    fn test_list_contains_description_and_parameters() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("my_tool")));

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "my_tool");
        assert_eq!(list[0].description, "Mock tool: my_tool");
        assert!(list[0].parameters.is_object());
    }

    #[test]
    fn test_register_overwrites_same_name() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("dup")));
        registry.register(Box::new(MockTool::new("dup"))); // overwrite

        // Should still have exactly one tool
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.get("dup").unwrap().name(), "dup");
    }

    #[tokio::test]
    async fn test_execute_registered_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("echo")));

        let result = registry.execute("echo", serde_json::json!({})).await;
        assert!(result.success);
        assert_eq!(result.output, "executed:echo");
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let registry = ToolRegistry::new();

        let result = registry.execute("missing", serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Tool not found: missing"));
    }

    #[tokio::test]
    async fn test_execute_uses_correct_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("first")));
        registry.register(Box::new(MockTool::new("second")));

        let r1 = registry.execute("first", serde_json::json!({})).await;
        let r2 = registry.execute("second", serde_json::json!({})).await;

        assert_eq!(r1.output, "executed:first");
        assert_eq!(r2.output, "executed:second");
    }

    #[test]
    fn test_with_builtin_creates_populated_registry() {
        let registry = ToolRegistry::with_builtin();
        let list = registry.list();
        // Should have at least the 4 builtin tools
        assert!(list.len() >= 4);

        let names: Vec<&str> = list.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"file_write"));
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"search"));
    }

    #[tokio::test]
    async fn test_with_builtin_shell_execution() {
        let registry = ToolRegistry::with_builtin();

        let result = registry
            .execute("shell", serde_json::json!({"command": "echo builtin_test"}))
            .await;
        assert!(result.success);
        assert!(result.output.contains("builtin_test"));
    }

    #[tokio::test]
    async fn test_with_builtin_not_found() {
        let registry = ToolRegistry::with_builtin();

        let result = registry
            .execute("nonexistent_tool", serde_json::json!({}))
            .await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Tool not found"));
    }

    #[test]
    fn test_tool_info_serialization() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("serializable")));

        let list = registry.list();
        let json = serde_json::to_string(&list[0]).unwrap();
        assert!(json.contains("serializable"));
        assert!(json.contains("Mock tool: serializable"));
    }
}
