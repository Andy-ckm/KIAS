//! 可组合插件框架
//!
//! 提供插件化架构，支持 Model/Tool/Strategy/Storage/Observability 五种插件类型。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 插件类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    Model,
    Tool,
    Strategy,
    Storage,
    Observability,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Model => write!(f, "Model"),
            PluginType::Tool => write!(f, "Tool"),
            PluginType::Strategy => write!(f, "Strategy"),
            PluginType::Storage => write!(f, "Storage"),
            PluginType::Observability => write!(f, "Observability"),
        }
    }
}

/// 插件元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
}

/// 插件初始化上下文
#[derive(Debug, Clone, Default)]
pub struct PluginContext {
    pub config: HashMap<String, String>,
}

/// 插件 trait - 所有插件必须实现
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 插件名称
    fn name(&self) -> &str;
    /// 插件版本
    fn version(&self) -> &str;
    /// 插件描述
    fn description(&self) -> &str;
    /// 插件类型
    fn plugin_type(&self) -> PluginType;
    /// 初始化插件
    async fn init(&self, ctx: PluginContext) -> Result<(), PluginError>;
    /// 关闭插件
    async fn shutdown(&self) -> Result<(), PluginError>;
}

/// 插件错误
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin `{0}` not found")]
    NotFound(String),
    #[error("Plugin `{0}` already registered")]
    AlreadyRegistered(String),
    #[error("Plugin `{0}` initialization failed: {1}")]
    InitFailed(String, String),
    #[error("Plugin `{0}` shutdown failed: {1}")]
    ShutdownFailed(String, String),
    #[error("Invalid plugin type: {0}")]
    InvalidType(String),
}

/// 插件注册表
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn Plugin>>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    /// 创建新的插件注册表
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// 注册插件
    pub async fn register<P: Plugin + 'static>(&self, plugin: P) -> Result<(), PluginError> {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write().await;
        if plugins.contains_key(&name) {
            return Err(PluginError::AlreadyRegistered(name));
        }
        plugins.insert(name, Arc::new(plugin));
        Ok(())
    }

    /// 发现插件
    pub async fn discover(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).cloned()
    }

    /// 获取所有已注册插件
    pub async fn list(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .map(|p| PluginMetadata {
                name: p.name().to_string(),
                version: p.version().to_string(),
                description: p.description().to_string(),
                plugin_type: p.plugin_type(),
            })
            .collect()
    }

    /// 按类型查找插件
    pub async fn find_by_type(&self, plugin_type: PluginType) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|p| p.plugin_type() == plugin_type)
            .map(|p| PluginMetadata {
                name: p.name().to_string(),
                version: p.version().to_string(),
                description: p.description().to_string(),
                plugin_type: p.plugin_type(),
            })
            .collect()
    }

    /// 初始化插件
    pub async fn init_plugin(&self, name: &str, ctx: PluginContext) -> Result<(), PluginError> {
        let plugin = self
            .discover(name)
            .await
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;
        plugin
            .init(ctx)
            .await
            .map_err(|e| PluginError::InitFailed(name.to_string(), e.to_string()))
    }

    /// 关闭插件
    pub async fn shutdown_plugin(&self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .discover(name)
            .await
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;
        plugin
            .shutdown()
            .await
            .map_err(|e| PluginError::ShutdownFailed(name.to_string(), e.to_string()))
    }

    /// 注销插件
    pub async fn unregister(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        if plugins.remove(name).is_none() {
            return Err(PluginError::NotFound(name.to_string()));
        }
        Ok(())
    }

    /// 获取插件数量
    pub async fn count(&self) -> usize {
        let plugins = self.plugins.read().await;
        plugins.len()
    }
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModelPlugin;
    struct TestToolPlugin;

    #[async_trait]
    impl Plugin for TestModelPlugin {
        fn name(&self) -> &str {
            "test-model"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn description(&self) -> &str {
            "A test model plugin"
        }
        fn plugin_type(&self) -> PluginType {
            PluginType::Model
        }
        async fn init(&self, _ctx: PluginContext) -> Result<(), PluginError> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<(), PluginError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Plugin for TestToolPlugin {
        fn name(&self) -> &str {
            "test-tool"
        }
        fn version(&self) -> &str {
            "2.0.0"
        }
        fn description(&self) -> &str {
            "A test tool plugin"
        }
        fn plugin_type(&self) -> PluginType {
            PluginType::Tool
        }
        async fn init(&self, _ctx: PluginContext) -> Result<(), PluginError> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<(), PluginError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_plugin_registry_register_and_discover() {
        let registry = PluginRegistry::new();
        registry.register(TestModelPlugin).await.unwrap();
        let plugin = registry.discover("test-model").await;
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().name(), "test-model");
    }

    #[tokio::test]
    async fn test_plugin_registry_duplicate_registration() {
        let registry = PluginRegistry::new();
        registry.register(TestModelPlugin).await.unwrap();
        let result = registry.register(TestModelPlugin).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_plugin_registry_list_all() {
        let registry = PluginRegistry::new();
        registry.register(TestModelPlugin).await.unwrap();
        registry.register(TestToolPlugin).await.unwrap();
        let list = registry.list().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_plugin_registry_find_by_type() {
        let registry = PluginRegistry::new();
        registry.register(TestModelPlugin).await.unwrap();
        registry.register(TestToolPlugin).await.unwrap();
        let models = registry.find_by_type(PluginType::Model).await;
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "test-model");
    }

    #[tokio::test]
    async fn test_plugin_registry_not_found() {
        let registry = PluginRegistry::new();
        let result = registry.discover("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_plugin_registry_unregister() {
        let registry = PluginRegistry::new();
        registry.register(TestModelPlugin).await.unwrap();
        registry.unregister("test-model").await.unwrap();
        assert!(registry.discover("test-model").await.is_none());
    }

    #[tokio::test]
    async fn test_plugin_registry_count() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.count().await, 0);
        registry.register(TestModelPlugin).await.unwrap();
        registry.register(TestToolPlugin).await.unwrap();
        assert_eq!(registry.count().await, 2);
    }

    #[tokio::test]
    async fn test_plugin_type_display() {
        assert_eq!(PluginType::Model.to_string(), "Model");
        assert_eq!(PluginType::Tool.to_string(), "Tool");
        assert_eq!(PluginType::Strategy.to_string(), "Strategy");
        assert_eq!(PluginType::Storage.to_string(), "Storage");
        assert_eq!(PluginType::Observability.to_string(), "Observability");
    }
}
