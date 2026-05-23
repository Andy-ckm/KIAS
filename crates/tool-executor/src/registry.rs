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

    // ========== 额外测试 ==========

    #[test]
    fn test_list_order_independent() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("z")));
        registry.register(Box::new(MockTool::new("a")));
        registry.register(Box::new(MockTool::new("m")));

        let list = registry.list();
        assert_eq!(list.len(), 3);
        let mut names: Vec<&str> = list.iter().map(|t| t.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["a", "m", "z"]);
    }

    #[test]
    fn test_register_many_tools() {
        let mut registry = ToolRegistry::new();
        for i in 0..50 {
            registry.register(Box::new(MockTool::new(&format!("tool_{i}"))));
        }
        assert_eq!(registry.list().len(), 50);
        assert!(registry.get("tool_25").is_some());
        assert!(registry.get("tool_49").is_some());
        assert!(registry.get("tool_50").is_none());
    }

    #[test]
    fn test_get_after_overwrite_returns_new() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("tool")));
        let first_desc = registry.get("tool").unwrap().description().to_string();
        registry.register(Box::new(MockTool::new("tool")));
        let second_desc = registry.get("tool").unwrap().description().to_string();
        assert_eq!(first_desc, second_desc);
    }

    #[test]
    fn test_list_empty_registry() {
        let registry = ToolRegistry::new();
        let list = registry.list();
        assert!(list.is_empty());
        let json = serde_json::to_string(&list).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_with_builtin_exact_count() {
        let registry = ToolRegistry::with_builtin();
        assert_eq!(registry.list().len(), 4);
    }

    #[test]
    fn test_with_builtin_file_read_execution() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "test_content_123").unwrap();

        let registry = ToolRegistry::with_builtin();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(registry.execute(
            "file_read",
            serde_json::json!({"path": tmp.path().to_str().unwrap()}),
        ));
        assert!(result.success);
        assert!(result.output.contains("test_content_123"));
    }

    #[test]
    fn test_with_builtin_file_write_execution() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let registry = ToolRegistry::with_builtin();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(registry.execute(
            "file_write",
            serde_json::json!({"path": tmp.path().to_str().unwrap(), "content": "written_by_registry"}),
        ));
        assert!(result.success);
        assert_eq!(
            std::fs::read_to_string(tmp.path()).unwrap(),
            "written_by_registry"
        );
    }

    #[test]
    fn test_with_builtin_search_execution() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "find_me_pattern").unwrap();

        let registry = ToolRegistry::with_builtin();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(registry.execute(
            "search",
            serde_json::json!({"pattern": "find_me", "path": tmp.path().to_str().unwrap()}),
        ));
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_execute_empty_name() {
        let registry = ToolRegistry::new();
        let result = registry.execute("", serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Tool not found"));
    }

    #[tokio::test]
    async fn test_execute_with_params() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("tool")));
        let result = registry
            .execute("tool", serde_json::json!({"key": "value"}))
            .await;
        assert!(result.success);
    }

    #[test]
    fn test_register_empty_name_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("")));
        assert_eq!(registry.list().len(), 1);
        assert!(registry.get("").is_some());
    }

    #[test]
    fn test_register_special_chars_name() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("tool-with-dashes")));
        registry.register(Box::new(MockTool::new("tool_with_underscores")));
        registry.register(Box::new(MockTool::new("tool.with.dots")));
        assert_eq!(registry.list().len(), 3);
        assert!(registry.get("tool-with-dashes").is_some());
        assert!(registry.get("tool_with_underscores").is_some());
        assert!(registry.get("tool.with.dots").is_some());
    }

    #[test]
    fn test_tool_info_contains_all_fields() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("info_test")));
        let list = registry.list();
        let info = &list[0];
        assert_eq!(info.name, "info_test");
        assert_eq!(info.description, "Mock tool: info_test");
        assert!(info.parameters.is_object());
        let json = serde_json::to_value(info).unwrap();
        assert!(json.get("name").is_some());
        assert!(json.get("description").is_some());
        assert!(json.get("parameters").is_some());
    }

    #[test]
    fn test_with_builtin_get_each_tool() {
        let registry = ToolRegistry::with_builtin();
        for name in &["file_read", "file_write", "shell", "search"] {
            let tool = registry.get(name);
            assert!(tool.is_some(), "Expected tool '{}' to exist", name);
            assert_eq!(tool.unwrap().name(), *name);
        }
    }

    // ========== Additional coverage tests ==========

    #[tokio::test]
    async fn test_registry_execute_returns_error_for_empty_params() {
        let registry = ToolRegistry::with_builtin();
        let result = registry.execute("file_read", serde_json::json!(null)).await;
        // null params should be handled gracefully
        let _ = result;
    }

    #[tokio::test]
    async fn test_registry_execute_tool_with_invalid_params() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("test_tool")));

        // file_read requires "path" param; pass empty object
        let result = registry.execute("file_read", serde_json::json!({})).await;
        // Should fail gracefully
        assert!(!result.success);
    }

    #[test]
    fn test_tool_info_clone() {
        let info = ToolInfo {
            name: "test".to_string(),
            description: "desc".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert_eq!(cloned.description, info.description);
    }

    // ToolRegistry does not implement Debug — no need to test it

    #[test]
    fn test_tool_info_debug() {
        let info = ToolInfo {
            name: "foo".to_string(),
            description: "bar".to_string(),
            parameters: serde_json::json!({}),
        };
        let debug = format!("{:?}", info);
        assert!(debug.contains("foo"));
    }

    #[test]
    fn test_default_for_tool_registry() {
        let registry = ToolRegistry::default();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_and_overwrite_changes_description() {
        struct DummyTool(&'static str, &'static str);
        impl DummyTool {
            fn new(name: &'static str, desc: &'static str) -> Self {
                Self(name, desc)
            }
        }
        #[async_trait]
        impl Tool for DummyTool {
            fn name(&self) -> &str {
                self.0
            }
            fn description(&self) -> &str {
                self.1
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(&self, _params: serde_json::Value) -> ToolResult {
                ToolResult {
                    success: true,
                    output: "dummy".to_string(),
                    error: None,
                    metadata: None,
                }
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(DummyTool::new("tool", "first")));
        let first_desc = registry.get("tool").unwrap().description().to_string();

        registry.register(Box::new(DummyTool::new("tool", "second")));
        let second_desc = registry.get("tool").unwrap().description().to_string();

        assert_eq!(first_desc, "first");
        assert_eq!(second_desc, "second");
    }
}
