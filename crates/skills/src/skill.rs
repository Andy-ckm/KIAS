use async_trait::async_trait;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};

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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "network".to_string(),
            "api".to_string(),
            "http".to_string(),
        ])
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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "system".to_string(),
            "shell".to_string(),
            "command".to_string(),
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
}
