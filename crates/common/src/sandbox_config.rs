//! 一键沙盒配置
//!
//! 提供最小依赖快速启动配置，支持开发/测试/生产三种环境。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 沙盒配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub profile: SandboxProfile,
    pub name: String,
    pub description: String,
    pub dependencies: Vec<Dependency>,
    pub environment: HashMap<String, String>,
    pub features: Vec<String>,
    pub resource_limits: ResourceLimits,
}

/// 沙盒配置模板名称
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxProfile {
    Development,
    Testing,
    Production,
}

impl std::fmt::Display for SandboxProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxProfile::Development => write!(f, "Development"),
            SandboxProfile::Testing => write!(f, "Testing"),
            SandboxProfile::Production => write!(f, "Production"),
        }
    }
}

/// 依赖项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub optional: bool,
    pub source: Option<String>,
}

/// 资源限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_cores: u32,
    pub max_disk_mb: u64,
    pub max_network_connections: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,
            max_cpu_cores: 2,
            max_disk_mb: 1024,
            max_network_connections: 10,
        }
    }
}

impl SandboxConfig {
    pub fn new(profile: SandboxProfile, name: &str) -> Self {
        Self {
            profile,
            name: name.to_string(),
            description: String::new(),
            dependencies: Vec::new(),
            environment: HashMap::new(),
            features: Vec::new(),
            resource_limits: ResourceLimits::default(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_dependency(mut self, dep: Dependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.environment.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_feature(mut self, feature: &str) -> Self {
        self.features.push(feature.to_string());
        self
    }

    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }
}

/// 沙盒配置文件生成器
pub struct SandboxConfigBuilder {
    profile: SandboxProfile,
    name: String,
    description: String,
    dependencies: Vec<Dependency>,
    environment: HashMap<String, String>,
    features: Vec<String>,
    resource_limits: ResourceLimits,
}

impl SandboxConfigBuilder {
    pub fn new(profile: SandboxProfile, name: &str) -> Self {
        Self {
            profile,
            name: name.to_string(),
            description: String::new(),
            dependencies: Vec::new(),
            environment: HashMap::new(),
            features: Vec::new(),
            resource_limits: ResourceLimits::default(),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn add_dependency(mut self, name: &str, version: &str, optional: bool) -> Self {
        self.dependencies.push(Dependency {
            name: name.to_string(),
            version: version.to_string(),
            optional,
            source: None,
        });
        self
    }

    pub fn add_env(mut self, key: &str, value: &str) -> Self {
        self.environment.insert(key.to_string(), value.to_string());
        self
    }

    pub fn add_feature(mut self, feature: &str) -> Self {
        self.features.push(feature.to_string());
        self
    }

    pub fn resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    pub fn build(self) -> SandboxConfig {
        SandboxConfig {
            profile: self.profile,
            name: self.name,
            description: self.description,
            dependencies: self.dependencies,
            environment: self.environment,
            features: self.features,
            resource_limits: self.resource_limits,
        }
    }
}

/// 依赖最小化器
pub struct DependencyMinimizer;

impl DependencyMinimizer {
    /// 最小化依赖集合
    pub fn minimize(dependencies: &[Dependency]) -> Vec<Dependency> {
        dependencies
            .iter()
            .filter(|d| !d.optional)
            .cloned()
            .collect()
    }

    /// 获取核心依赖
    pub fn core_dependencies() -> Vec<Dependency> {
        vec![
            Dependency {
                name: "tokio".to_string(),
                version: "1".to_string(),
                optional: false,
                source: Some("crates/common".to_string()),
            },
            Dependency {
                name: "serde".to_string(),
                version: "1".to_string(),
                optional: false,
                source: Some("crates/common".to_string()),
            },
            Dependency {
                name: "tracing".to_string(),
                version: "0.1".to_string(),
                optional: false,
                source: Some("crates/common".to_string()),
            },
        ]
    }

    /// 计算依赖覆盖率
    pub fn coverage(used: &[String], available: &[Dependency]) -> f64 {
        if available.is_empty() {
            return 100.0;
        }
        let used_set: HashMap<&str, ()> = used.iter().map(|s| (s.as_str(), ())).collect();
        let covered = available
            .iter()
            .filter(|d| used_set.contains_key(d.name.as_str()))
            .count();
        (covered as f64 / available.len() as f64) * 100.0
    }
}

/// 沙盒配置注册表
pub struct SandboxRegistry {
    sandboxes: HashMap<String, SandboxConfig>,
}

impl Default for SandboxRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxRegistry {
    pub fn new() -> Self {
        Self {
            sandboxes: HashMap::new(),
        }
    }

    /// 注册沙盒配置
    pub fn register(&mut self, config: SandboxConfig) -> Result<(), SandboxError> {
        let name = config.name.clone();
        if self.sandboxes.contains_key(&name) {
            return Err(SandboxError::AlreadyRegistered(name));
        }
        self.sandboxes.insert(name, config);
        Ok(())
    }

    /// 获取沙盒配置
    pub fn get(&self, name: &str) -> Option<&SandboxConfig> {
        self.sandboxes.get(name)
    }

    /// 按配置文件类型获取
    pub fn by_profile(&self, profile: SandboxProfile) -> Vec<&SandboxConfig> {
        self.sandboxes
            .values()
            .filter(|s| s.profile == profile)
            .collect()
    }

    /// 列出所有
    pub fn list_all(&self) -> Vec<&SandboxConfig> {
        self.sandboxes.values().collect()
    }

    /// 创建开发沙盒
    pub fn create_dev_sandbox(&mut self) -> &SandboxConfig {
        let config = SandboxConfigBuilder::new(SandboxProfile::Development, "dev-sandbox")
            .description("Development sandbox with full debugging")
            .add_dependency("tokio", "1", false)
            .add_dependency("tracing", "0.1", false)
            .add_dependency("debugger", "1.0", true)
            .add_env("RUST_LOG", "debug")
            .add_env("KIAS_ENV", "development")
            .add_feature("debugging")
            .add_feature("hot_reload")
            .resource_limits(ResourceLimits {
                max_memory_mb: 2048,
                max_cpu_cores: 4,
                max_disk_mb: 5120,
                max_network_connections: 50,
            })
            .build();
        let name = config.name.clone();
        self.sandboxes.insert(name, config);
        self.sandboxes.get("dev-sandbox").unwrap()
    }

    /// 创建测试沙盒
    pub fn create_test_sandbox(&mut self) -> &SandboxConfig {
        let config = SandboxConfigBuilder::new(SandboxProfile::Testing, "test-sandbox")
            .description("Testing sandbox with mocks")
            .add_dependency("tokio", "1", false)
            .add_dependency("tracing", "0.1", false)
            .add_dependency("mockall", "0.12", true)
            .add_env("RUST_LOG", "info")
            .add_env("KIAS_ENV", "testing")
            .add_feature("mocking")
            .add_feature("coverage")
            .resource_limits(ResourceLimits {
                max_memory_mb: 1024,
                max_cpu_cores: 2,
                max_disk_mb: 2048,
                max_network_connections: 20,
            })
            .build();
        let name = config.name.clone();
        self.sandboxes.insert(name, config);
        self.sandboxes.get("test-sandbox").unwrap()
    }

    /// 创建生产沙盒
    pub fn create_prod_sandbox(&mut self) -> &SandboxConfig {
        let config = SandboxConfigBuilder::new(SandboxProfile::Production, "prod-sandbox")
            .description("Production sandbox with optimized settings")
            .add_dependency("tokio", "1", false)
            .add_dependency("tracing", "0.1", false)
            .add_env("RUST_LOG", "warn")
            .add_env("KIAS_ENV", "production")
            .add_feature("optimization")
            .resource_limits(ResourceLimits {
                max_memory_mb: 8192,
                max_cpu_cores: 8,
                max_disk_mb: 10240,
                max_network_connections: 100,
            })
            .build();
        let name = config.name.clone();
        self.sandboxes.insert(name, config);
        self.sandboxes.get("prod-sandbox").unwrap()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("Sandbox `{0}` already registered")]
    AlreadyRegistered(String),
    #[error("Sandbox `{0}` not found")]
    NotFound(String),
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_builder() {
        let config = SandboxConfigBuilder::new(SandboxProfile::Development, "test")
            .description("Test sandbox")
            .add_dependency("tokio", "1", false)
            .add_env("KEY", "value")
            .add_feature("test")
            .build();
        assert_eq!(config.name, "test");
        assert_eq!(config.profile, SandboxProfile::Development);
        assert_eq!(config.dependencies.len(), 1);
        assert_eq!(config.environment.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_dependency_minimizer() {
        let deps = vec![
            Dependency {
                name: "tokio".to_string(),
                version: "1".to_string(),
                optional: false,
                source: None,
            },
            Dependency {
                name: "serde".to_string(),
                version: "1".to_string(),
                optional: true,
                source: None,
            },
            Dependency {
                name: "tracing".to_string(),
                version: "0.1".to_string(),
                optional: false,
                source: None,
            },
        ];
        let minimized = DependencyMinimizer::minimize(&deps);
        assert_eq!(minimized.len(), 2);
        assert!(minimized.iter().all(|d| !d.optional));
    }

    #[test]
    fn test_dependency_minimizer_core() {
        let core = DependencyMinimizer::core_dependencies();
        assert!(!core.is_empty());
        assert!(core.iter().all(|d| !d.optional));
    }

    #[test]
    fn test_dependency_minimizer_coverage() {
        let available = vec![
            Dependency {
                name: "tokio".to_string(),
                version: "1".to_string(),
                optional: false,
                source: None,
            },
            Dependency {
                name: "serde".to_string(),
                version: "1".to_string(),
                optional: false,
                source: None,
            },
        ];
        let used = vec!["tokio".to_string()];
        let coverage = DependencyMinimizer::coverage(&used, &available);
        assert_eq!(coverage, 50.0);
    }

    #[test]
    fn test_sandbox_registry_register_and_get() {
        let mut registry = SandboxRegistry::new();
        let config = SandboxConfigBuilder::new(SandboxProfile::Development, "test-sandbox").build();
        registry.register(config).unwrap();
        assert!(registry.get("test-sandbox").is_some());
    }

    #[test]
    fn test_sandbox_registry_duplicate() {
        let mut registry = SandboxRegistry::new();
        let config = SandboxConfigBuilder::new(SandboxProfile::Development, "test-sandbox").build();
        registry.register(config.clone()).unwrap();
        let result = registry.register(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_registry_by_profile() {
        let mut registry = SandboxRegistry::new();
        registry.create_dev_sandbox();
        registry.create_test_sandbox();
        let dev_sandboxes = registry.by_profile(SandboxProfile::Development);
        assert_eq!(dev_sandboxes.len(), 1);
    }

    #[test]
    fn test_sandbox_profile_display() {
        assert_eq!(SandboxProfile::Development.to_string(), "Development");
        assert_eq!(SandboxProfile::Testing.to_string(), "Testing");
        assert_eq!(SandboxProfile::Production.to_string(), "Production");
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_mb, 512);
        assert_eq!(limits.max_cpu_cores, 2);
    }

    #[test]
    fn test_sandbox_config_with_resource_limits() {
        let config = SandboxConfig::new(SandboxProfile::Production, "prod").with_resource_limits(
            ResourceLimits {
                max_memory_mb: 4096,
                max_cpu_cores: 8,
                max_disk_mb: 8192,
                max_network_connections: 50,
            },
        );
        assert_eq!(config.resource_limits.max_memory_mb, 4096);
    }
}
