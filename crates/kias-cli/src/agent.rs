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
