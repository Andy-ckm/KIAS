use async_trait::async_trait;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};

/// Permission that a skill may require to execute.
///
/// Inspired by MCP capability declarations and AgentGuard agent sandbox types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillPermission {
    /// Outbound network access (HTTP, TCP, etc.)
    Network,
    /// Read/write access to the filesystem
    Filesystem,
    /// Elevated / root privileges
    Elevated,
    /// GPU device access
    Gpu,
    /// Raw socket access (e.g. nmap, packet capture)
    RawSocket,
    /// Custom permission with an arbitrary name
    Custom(String),
}

impl std::fmt::Display for SkillPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network => write!(f, "network"),
            Self::Filesystem => write!(f, "filesystem"),
            Self::Elevated => write!(f, "elevated"),
            Self::Gpu => write!(f, "gpu"),
            Self::RawSocket => write!(f, "raw_socket"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Risk level for progressive disclosure (SDOF-inspired L0/L1/L2).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum RiskLevel {
    /// L0: Atomic, safe, low-risk operations.
    #[default]
    Low,
    /// L1: Composite, moderate risk, may have side effects.
    Medium,
    /// L2: Strategic, high risk, requires authorization.
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Disclosure level for progressive information revelation.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum DisclosureLevel {
    /// L0: Compact summary only (name + description).
    #[default]
    Summary,
    /// L1: Full metadata (parameters, permissions, dependencies).
    Full,
    /// L2: Complete source including implementation details.
    Complete,
}

impl std::fmt::Display for DisclosureLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Summary => write!(f, "summary"),
            Self::Full => write!(f, "full"),
            Self::Complete => write!(f, "complete"),
        }
    }
}

/// A dependency declared by a skill on another skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDependency {
    /// Name of the required skill
    pub name: String,
    /// SemVer version requirement (e.g. ">=1.0", "^2.0.0").
    /// Empty string means any version.
    pub version_req: String,
    /// If true, the skill can still execute when this dependency is missing
    pub optional: bool,
}

impl SkillDependency {
    /// Create a required dependency (any version).
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version_req: String::new(),
            optional: false,
        }
    }

    /// Create an optional dependency (any version).
    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version_req: String::new(),
            optional: true,
        }
    }

    /// Create a dependency with a version constraint.
    pub fn with_version(mut self, version_req: impl Into<String>) -> Self {
        self.version_req = version_req.into();
        self
    }
}

/// Skill configuration for declarative skill definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: serde_json::Value,
    /// Tags for discovery and categorization
    pub tags: Vec<String>,
    /// Whether this skill requires elevated permissions
    pub requires_elevation: bool,
    /// Fine-grained permissions this skill needs to operate.
    /// When empty the skill requires no special permissions.
    #[serde(default)]
    pub permissions: Vec<SkillPermission>,
    /// Other skills this skill depends on.
    /// Used by the registry for dependency validation and resolution.
    #[serde(default)]
    pub dependencies: Vec<SkillDependency>,
    /// Risk level for progressive disclosure (L0/L1/L2).
    #[serde(default)]
    pub risk_level: RiskLevel,
    /// Current disclosure level for queries.
    #[serde(default)]
    pub disclosure_level: DisclosureLevel,
}

impl SkillConfig {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            version: "1.0.0".to_string(),
            parameters: serde_json::json!({}),
            tags: Vec::new(),
            requires_elevation: false,
            permissions: Vec::new(),
            dependencies: Vec::new(),
            risk_level: RiskLevel::Low,
            disclosure_level: DisclosureLevel::Summary,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set the permissions this skill requires.
    pub fn with_permissions(mut self, permissions: Vec<SkillPermission>) -> Self {
        self.requires_elevation =
            self.requires_elevation || permissions.contains(&SkillPermission::Elevated);
        self.permissions = permissions;
        self
    }

    /// Set the dependencies this skill requires.
    pub fn with_dependencies(mut self, dependencies: Vec<SkillDependency>) -> Self {
        self.dependencies = dependencies;
        self
    }

    /// Returns `true` if this skill requires the given permission.
    pub fn requires_permission(&self, perm: &SkillPermission) -> bool {
        self.permissions.contains(perm)
    }

    /// Returns `true` if all declared dependencies are non-optional.
    pub fn has_required_dependencies(&self) -> bool {
        self.dependencies.iter().any(|d| !d.optional)
    }
}

/// Core trait that all skills must implement
#[async_trait]
pub trait Skill: Send + Sync {
    /// Unique skill name
    fn name(&self) -> &str;
    /// Human-readable description
    fn description(&self) -> &str;
    /// Skill configuration/metadata
    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
    }
    /// Execute the skill with given parameters
    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value>;

    /// Health check for the curator system. Returns the current health status.
    /// Default implementation returns `Healthy` — override for real checks.
    async fn health_check(&self) -> KiasResult<crate::curator::SkillHealthStatus> {
        Ok(crate::curator::SkillHealthStatus::Healthy)
    }
}

// ===== Built-in Skill Implementations =====

/// HTTP Call Skill - makes HTTP requests
pub struct HttpCallSkill;

impl HttpCallSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpCallSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for HttpCallSkill {
    fn name(&self) -> &str {
        "http_call"
    }

    fn description(&self) -> &str {
        "Makes HTTP requests to external APIs. Supports GET, POST, PUT, DELETE."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "network".to_string(),
                "api".to_string(),
                "http".to_string(),
            ])
            .with_permissions(vec![crate::skill::SkillPermission::Network])
    }

    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value> {
        let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
            kias_common::KiasError::Validation("Missing 'url' parameter".to_string())
        })?;

        let method = params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let headers: std::collections::HashMap<String, String> = params
            .get("headers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let body = params.get("body").map(|v| v.to_string());

        tracing::info!(url = %url, method = %method, "Executing HTTP call skill");

        let client = reqwest::Client::new();
        let mut req_builder = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unsupported HTTP method: {}",
                    method
                )))
            }
        };

        for (key, value) in &headers {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        if let Some(body) = body {
            req_builder = req_builder
                .header("content-type", "application/json")
                .body(body);
        }

        let response = req_builder
            .send()
            .await
            .map_err(|e| kias_common::KiasError::ExternalService(e.to_string()))?;

        let status = response.status().as_u16();
        let response_body = response
            .text()
            .await
            .map_err(|e| kias_common::KiasError::ExternalService(e.to_string()))?;

        Ok(serde_json::json!({
            "status": status,
            "body": response_body,
            "method": method,
            "url": url,
        }))
    }
}

/// Shell Command Skill - executes shell commands (delegates to sandbox)
pub struct ShellSkill;

impl ShellSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ShellSkill {
    fn name(&self) -> &str {
        "shell_command"
    }

    fn description(&self) -> &str {
        "Executes shell commands with timeout and output capture."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "system".to_string(),
                "shell".to_string(),
                "command".to_string(),
            ])
            .with_permissions(vec![
                crate::skill::SkillPermission::Filesystem,
                crate::skill::SkillPermission::Elevated,
            ])
    }

    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'command' parameter".to_string())
            })?;

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let workdir = params.get("workdir").and_then(|v| v.as_str());

        tracing::info!(command = %command, timeout = timeout_secs, "Executing shell skill");

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        let output =
            tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output()).await;

        match output {
            Ok(Ok(result)) => {
                let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                Ok(serde_json::json!({
                    "exit_code": result.status.code().unwrap_or(-1),
                    "stdout": stdout,
                    "stderr": stderr,
                    "command": command,
                }))
            }
            Ok(Err(e)) => Err(kias_common::KiasError::ExternalService(format!(
                "Command failed: {}",
                e
            ))),
            Err(_) => Ok(serde_json::json!({
                "exit_code": -1,
                "stdout": "",
                "stderr": "Command timed out",
                "command": command,
                "timed_out": true,
            })),
        }
    }
}

/// JSON Transform Skill - transforms and queries JSON data
pub struct JsonTransformSkill;

impl JsonTransformSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonTransformSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for JsonTransformSkill {
    fn name(&self) -> &str {
        "json_transform"
    }

    fn description(&self) -> &str {
        "Transforms and queries JSON data using JSONPath-like expressions."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "data".to_string(),
            "json".to_string(),
            "transform".to_string(),
        ])
    }

    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value> {
        let operation = params
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("query");

        let data = params.get("data").cloned().unwrap_or(serde_json::json!({}));

        match operation {
            "query" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("$");
                // Simple path query: split by '.' and traverse
                let result = query_json_path(&data, path);
                Ok(serde_json::json!({"result": result}))
            }
            "merge" => {
                let patch = params
                    .get("patch")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let merged = merge_json(&data, &patch);
                Ok(serde_json::json!({"result": merged}))
            }
            "keys" => {
                let keys: Vec<String> = if let Some(obj) = data.as_object() {
                    obj.keys().cloned().collect()
                } else {
                    Vec::new()
                };
                Ok(serde_json::json!({"keys": keys}))
            }
            _ => Err(kias_common::KiasError::Validation(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }
}

/// Simple JSON path query (supports dotted paths like "a.b.c")
fn query_json_path(data: &serde_json::Value, path: &str) -> serde_json::Value {
    if path == "$" {
        return data.clone();
    }
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;
    for part in parts {
        match current.get(part) {
            Some(v) => current = v,
            None => return serde_json::Value::Null,
        }
    }
    current.clone()
}

/// Merge two JSON values (shallow merge of objects)
fn merge_json(base: &serde_json::Value, patch: &serde_json::Value) -> serde_json::Value {
    if let (Some(base_obj), Some(patch_obj)) = (base.as_object(), patch.as_object()) {
        let mut merged = base_obj.clone();
        for (key, value) in patch_obj {
            merged.insert(key.clone(), value.clone());
        }
        serde_json::Value::Object(merged)
    } else {
        patch.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_config_creation() {
        let config = SkillConfig::new("test", "A test skill");
        assert_eq!(config.name, "test");
        assert_eq!(config.description, "A test skill");
        assert!(config.tags.is_empty());
        assert!(!config.requires_elevation);
    }

    #[test]
    fn test_skill_config_with_tags() {
        let config = SkillConfig::new("test", "desc")
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);
        assert_eq!(config.tags.len(), 2);
    }

    #[test]
    fn test_http_skill_metadata() {
        let skill = HttpCallSkill::new();
        assert_eq!(skill.name(), "http_call");
        let config = skill.config();
        assert!(config.tags.contains(&"http".to_string()));
    }

    #[test]
    fn test_shell_skill_metadata() {
        let skill = ShellSkill::new();
        assert_eq!(skill.name(), "shell_command");
    }

    #[test]
    fn test_json_transform_skill_metadata() {
        let skill = JsonTransformSkill::new();
        assert_eq!(skill.name(), "json_transform");
    }

    #[tokio::test]
    async fn test_json_transform_query() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "query",
                "data": {"user": {"name": "Alice", "age": 30}},
                "path": "user.name"
            }))
            .await
            .unwrap();
        assert_eq!(result["result"], "Alice");
    }

    #[tokio::test]
    async fn test_json_transform_query_root() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "query",
                "data": {"key": "value"},
                "path": "$"
            }))
            .await
            .unwrap();
        assert_eq!(result["result"]["key"], "value");
    }

    #[tokio::test]
    async fn test_json_transform_query_missing() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "query",
                "data": {"key": "value"},
                "path": "nonexistent.path"
            }))
            .await
            .unwrap();
        assert_eq!(result["result"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_json_transform_merge() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "merge",
                "data": {"a": 1, "b": 2},
                "patch": {"b": 3, "c": 4}
            }))
            .await
            .unwrap();
        assert_eq!(result["result"]["a"], 1);
        assert_eq!(result["result"]["b"], 3);
        assert_eq!(result["result"]["c"], 4);
    }

    #[tokio::test]
    async fn test_json_transform_keys() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "keys",
                "data": {"name": "Alice", "age": 30}
            }))
            .await
            .unwrap();
        let keys = result["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_json_transform_unknown_operation() {
        let skill = JsonTransformSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "operation": "invalid"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_skill_echo() {
        let skill = ShellSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "command": "echo hello"
            }))
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_shell_skill_exit_code() {
        let skill = ShellSkill::new();
        let result = skill
            .execute(serde_json::json!({
                "command": "exit 42"
            }))
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 42);
    }

    #[tokio::test]
    async fn test_shell_skill_missing_command() {
        let skill = ShellSkill::new();
        let result = skill.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_query_json_path() {
        let data = serde_json::json!({"a": {"b": {"c": 42}}});
        assert_eq!(query_json_path(&data, "a.b.c"), serde_json::json!(42));
    }

    #[test]
    fn test_merge_json() {
        let base = serde_json::json!({"a": 1, "b": 2});
        let patch = serde_json::json!({"b": 3, "c": 4});
        let merged = merge_json(&base, &patch);
        assert_eq!(merged["a"], 1);
        assert_eq!(merged["b"], 3);
        assert_eq!(merged["c"], 4);
    }

    // ===== SkillPermission tests =====

    #[test]
    fn test_skill_permission_display() {
        assert_eq!(SkillPermission::Network.to_string(), "network");
        assert_eq!(SkillPermission::Filesystem.to_string(), "filesystem");
        assert_eq!(SkillPermission::Elevated.to_string(), "elevated");
        assert_eq!(SkillPermission::Gpu.to_string(), "gpu");
        assert_eq!(SkillPermission::RawSocket.to_string(), "raw_socket");
        assert_eq!(SkillPermission::Custom("db".into()).to_string(), "db");
    }

    #[test]
    fn test_skill_permission_equality() {
        assert_eq!(SkillPermission::Network, SkillPermission::Network);
        assert_ne!(SkillPermission::Network, SkillPermission::Filesystem);
        assert_eq!(
            SkillPermission::Custom("x".into()),
            SkillPermission::Custom("x".into())
        );
        assert_ne!(
            SkillPermission::Custom("x".into()),
            SkillPermission::Custom("y".into())
        );
    }

    #[test]
    fn test_skill_permission_serialization_roundtrip() {
        let perm = SkillPermission::Network;
        let json = serde_json::to_string(&perm).unwrap();
        let deserialized: SkillPermission = serde_json::from_str(&json).unwrap();
        assert_eq!(perm, deserialized);

        let custom = SkillPermission::Custom("my_perm".into());
        let json = serde_json::to_string(&custom).unwrap();
        let deserialized: SkillPermission = serde_json::from_str(&json).unwrap();
        assert_eq!(custom, deserialized);
    }

    // ===== SkillDependency tests =====

    #[test]
    fn test_skill_dependency_required() {
        let dep = SkillDependency::required("http_call");
        assert_eq!(dep.name, "http_call");
        assert!(dep.version_req.is_empty());
        assert!(!dep.optional);
    }

    #[test]
    fn test_skill_dependency_optional() {
        let dep = SkillDependency::optional("shell_command");
        assert_eq!(dep.name, "shell_command");
        assert!(dep.optional);
    }

    #[test]
    fn test_skill_dependency_with_version() {
        let dep = SkillDependency::required("http_call").with_version("^1.0.0");
        assert_eq!(dep.version_req, "^1.0.0");
        assert!(!dep.optional);
    }

    #[test]
    fn test_skill_dependency_serialization() {
        let dep = SkillDependency::required("test").with_version(">=2.0");
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: SkillDependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.version_req, ">=2.0");
        assert!(!deserialized.optional);
    }

    // ===== SkillConfig permissions/dependencies tests =====

    #[test]
    fn test_skill_config_with_permissions() {
        let config = SkillConfig::new("test", "desc")
            .with_permissions(vec![SkillPermission::Network, SkillPermission::Elevated]);
        assert_eq!(config.permissions.len(), 2);
        assert!(config.requires_permission(&SkillPermission::Network));
        assert!(config.requires_permission(&SkillPermission::Elevated));
        assert!(!config.requires_permission(&SkillPermission::Gpu));
        // Elevated permission should set requires_elevation
        assert!(config.requires_elevation);
    }

    #[test]
    fn test_skill_config_with_dependencies() {
        let config = SkillConfig::new("composite", "desc").with_dependencies(vec![
            SkillDependency::required("http_call"),
            SkillDependency::optional("shell_command"),
        ]);
        assert_eq!(config.dependencies.len(), 2);
        assert!(config.has_required_dependencies());
    }

    #[test]
    fn test_skill_config_no_dependencies() {
        let config = SkillConfig::new("simple", "desc");
        assert!(!config.has_required_dependencies());
        assert!(config.permissions.is_empty());
        assert!(config.dependencies.is_empty());
    }

    #[test]
    fn test_skill_config_serde_default_fields() {
        // Verify that old JSON without permissions/dependencies still deserializes
        let json = r#"{"name":"old_skill","description":"legacy","version":"1.0.0","parameters":{},"tags":[],"requires_elevation":false}"#;
        let config: SkillConfig = serde_json::from_str(json).unwrap();
        assert!(config.permissions.is_empty());
        assert!(config.dependencies.is_empty());
    }

    #[test]
    fn test_skill_config_serialization_roundtrip() {
        let config = SkillConfig::new("rt_test", "desc")
            .with_tags(vec!["tag1".to_string()])
            .with_permissions(vec![SkillPermission::Network])
            .with_dependencies(vec![SkillDependency::required("dep1")]);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SkillConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "rt_test");
        assert_eq!(deserialized.permissions.len(), 1);
        assert_eq!(deserialized.dependencies.len(), 1);
        assert_eq!(deserialized.dependencies[0].name, "dep1");
    }

    // ===== Built-in skill permission tests =====

    #[test]
    fn test_http_call_skill_has_network_permission() {
        let skill = HttpCallSkill::new();
        let config = skill.config();
        assert!(config.requires_permission(&SkillPermission::Network));
        assert!(!config.requires_permission(&SkillPermission::Filesystem));
    }

    #[test]
    fn test_shell_skill_has_elevated_permission() {
        let skill = ShellSkill::new();
        let config = skill.config();
        assert!(config.requires_permission(&SkillPermission::Filesystem));
        assert!(config.requires_permission(&SkillPermission::Elevated));
        assert!(config.requires_elevation);
    }
}
