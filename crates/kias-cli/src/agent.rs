//! Agent YAML 定义 - 超越 AgentRun

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// KIAS Agent 定义（K8S 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: AgentMetadata,
    pub spec: AgentSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub name: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub prompt: String,
    pub model: ModelConfig,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub sandboxes: Vec<String>,
    #[serde(default)]
    pub resources: Option<ResourceConfig>,
    #[serde(default)]
    pub permissions: Option<PermissionConfig>,
    #[serde(default)]
    pub cost: Option<CostConfig>,
    #[serde(default)]
    pub audit: Option<AuditConfig>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub cpu: Option<f64>,
    #[serde(default)]
    pub gpu: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    #[serde(default)]
    pub max_tokens_per_run: Option<u64>,
    #[serde(default)]
    pub max_cost_per_day: Option<f64>,
    #[serde(default)]
    pub max_cost_per_run: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_retention")]
    pub retention: String,
}

fn default_log_level() -> String {
    "detailed".to_string()
}

fn default_retention() -> String {
    "90d".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_backoff")]
    pub backoff_ms: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_backoff() -> u64 {
    1000
}

impl AgentDefinition {
    /// 从 YAML 文件加载
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// 验证定义
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.api_version != "kias/v1" {
            errors.push(format!("不支持的 api_version: {}", self.api_version));
        }

        if self.kind != "Agent" {
            errors.push(format!("不支持的 kind: {}", self.kind));
        }

        if self.metadata.name.is_empty() {
            errors.push("name 不能为空".to_string());
        }

        if self.spec.prompt.is_empty() {
            errors.push("prompt 不能为空".to_string());
        }

        if self.spec.model.name.is_empty() {
            errors.push("model.name 不能为空".to_string());
        }

        // 验证 temperature 范围
        if let Some(temp) = self.spec.model.temperature {
            if !(0.0..=2.0).contains(&temp) {
                errors.push(format!("temperature 必须在 0.0-2.0 之间，当前值: {}", temp));
            }
        }

        // 验证 max_tokens
        if let Some(tokens) = self.spec.model.max_tokens {
            if tokens == 0 {
                errors.push("max_tokens 不能为 0".to_string());
            }
        }

        // 验证 timeout
        if let Some(timeout) = self.spec.timeout {
            if timeout == 0 {
                errors.push("timeout 不能为 0".to_string());
            }
        }

        // 验证 retry
        if let Some(ref retry) = self.spec.retry {
            if retry.backoff_ms == 0 {
                errors.push("retry.backoff_ms 不能为 0".to_string());
            }
        }

        // 验证 name 格式（只允许小写字母、数字、连字符）
        if !self.metadata.name.is_empty()
            && !self
                .metadata
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errors.push("name 只能包含小写字母、数字和连字符".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 转换为运行时配置
    pub fn to_runtime_config(&self) -> RuntimeAgentConfig {
        RuntimeAgentConfig {
            name: self.metadata.name.clone(),
            prompt: self.spec.prompt.clone(),
            model: self.spec.model.name.clone(),
            tools: self.spec.tools.clone(),
            skills: self.spec.skills.clone(),
            max_tokens: self.spec.model.max_tokens.unwrap_or(4096),
            temperature: self.spec.model.temperature.unwrap_or(0.7),
        }
    }
}

/// 运行时 Agent 配置
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeAgentConfig {
    pub name: String,
    pub prompt: String,
    pub model: String,
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub max_tokens: u64,
    pub temperature: f64,
}

/// Workflow 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: AgentMetadata,
    pub spec: WorkflowSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub entry: String,
    pub nodes: Vec<WorkflowNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub name: String,
    pub agent: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub condition: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_YAML: &str = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
  namespace: default
  labels:
    env: test
spec:
  prompt: "You are a helpful assistant"
  model:
    name: gpt-4
    temperature: 0.7
    max_tokens: 4096
  tools:
    - web_search
    - code_exec
  skills:
    - summarization
  timeout: 300
"#;

    #[test]
    fn test_from_yaml_valid() {
        let def = AgentDefinition::from_yaml(VALID_YAML);
        assert!(def.is_ok());
        let def = def.expect("should parse");
        assert_eq!(def.api_version, "kias/v1");
        assert_eq!(def.kind, "Agent");
        assert_eq!(def.metadata.name, "test-agent");
        assert_eq!(def.spec.model.name, "gpt-4");
        assert_eq!(def.spec.tools.len(), 2);
    }

    #[test]
    fn test_from_yaml_invalid_syntax() {
        let result = AgentDefinition::from_yaml("not: [valid: yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid() {
        let def = AgentDefinition::from_yaml(VALID_YAML).expect("should parse");
        assert!(def.validate().is_ok());
    }

    #[test]
    fn test_validate_wrong_api_version() {
        let yaml = r#"
apiVersion: v2
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("api_version")));
    }

    #[test]
    fn test_validate_wrong_kind() {
        let yaml = r#"
apiVersion: kias/v1
kind: Workflow
metadata:
  name: test
spec:
  prompt: "hello"
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("kind")));
    }

    #[test]
    fn test_validate_empty_name() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: ""
spec:
  prompt: "hello"
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_validate_empty_prompt() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: ""
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("prompt")));
    }

    #[test]
    fn test_validate_empty_model_name() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: ""
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("model.name")));
    }

    #[test]
    fn test_validate_temperature_out_of_range() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
    temperature: 3.0
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("temperature")));
    }

    #[test]
    fn test_validate_zero_max_tokens() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
    max_tokens: 0
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("max_tokens")));
    }

    #[test]
    fn test_validate_zero_timeout() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
  timeout: 0
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("timeout")));
    }

    #[test]
    fn test_validate_invalid_name_chars() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: "Test_Agent!"
spec:
  prompt: "hello"
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(errors.iter().any(|e| e.contains("小写字母")));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let yaml = r#"
apiVersion: v2
kind: Workflow
metadata:
  name: ""
spec:
  prompt: ""
  model:
    name: ""
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let errors = def.validate().expect_err("should fail");
        assert!(
            errors.len() >= 4,
            "Expected at least 4 errors, got {}",
            errors.len()
        );
    }

    #[test]
    fn test_to_runtime_config() {
        let def = AgentDefinition::from_yaml(VALID_YAML).expect("should parse");
        let runtime = def.to_runtime_config();
        assert_eq!(runtime.name, "test-agent");
        assert_eq!(runtime.model, "gpt-4");
        assert_eq!(runtime.max_tokens, 4096);
        assert!((runtime.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(runtime.tools, vec!["web_search", "code_exec"]);
        assert_eq!(runtime.skills, vec!["summarization"]);
    }

    #[test]
    fn test_to_runtime_config_defaults() {
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: minimal-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        let runtime = def.to_runtime_config();
        assert_eq!(runtime.max_tokens, 4096);
        assert!((runtime.temperature - 0.7).abs() < f64::EPSILON);
        assert!(runtime.tools.is_empty());
        assert!(runtime.skills.is_empty());
    }

    #[test]
    fn test_workflow_definition_parse() {
        let yaml = r#"
apiVersion: kias/v1
kind: Workflow
metadata:
  name: my-workflow
spec:
  entry: step1
  nodes:
    - name: step1
      agent: agent-a
      prompt: "do something"
    - name: step2
      agent: agent-b
  edges:
    - from: step1
      to: step2
"#;
        let wf: WorkflowDefinition = serde_yaml::from_str(yaml).expect("should parse");
        assert_eq!(wf.api_version, "kias/v1");
        assert_eq!(wf.metadata.name, "my-workflow");
        assert_eq!(wf.spec.entry, "step1");
        assert_eq!(wf.spec.nodes.len(), 2);
        assert_eq!(wf.spec.edges.len(), 1);
    }

    #[test]
    fn test_agent_metadata_labels() {
        let def = AgentDefinition::from_yaml(VALID_YAML).expect("should parse");
        assert_eq!(def.metadata.labels.get("env"), Some(&"test".to_string()));
    }

    #[test]
    fn test_validate_boundary_temperature() {
        // temperature = 0.0 is valid
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
    temperature: 0.0
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        assert!(def.validate().is_ok());

        // temperature = 2.0 is valid
        let yaml = r#"
apiVersion: kias/v1
kind: Agent
metadata:
  name: test-agent
spec:
  prompt: "hello"
  model:
    name: gpt-4
    temperature: 2.0
"#;
        let def = AgentDefinition::from_yaml(yaml).expect("should parse");
        assert!(def.validate().is_ok());
    }
}
