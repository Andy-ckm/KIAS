//! Agent 运行时类型定义

use serde::{Deserialize, Serialize};

/// Agent 执行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub model: String,
    pub system_prompt: String,
    pub max_iterations: u32,
    pub max_tokens: u64,
    pub temperature: f64,
    pub tools: Vec<String>,
    pub sandbox: SandboxConfig,
}

/// 沙箱配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub enabled: bool,
    pub allowed_paths: Vec<String>,
    pub allow_network: bool,
    pub timeout_secs: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_paths: vec![".".to_string()],
            allow_network: false,
            timeout_secs: 60,
        }
    }
}

/// Agent 执行结果
#[derive(Debug, Clone, Serialize)]
pub struct AgentResult {
    pub success: bool,
    pub output: String,
    pub iterations: u32,
    pub tokens_used: u64,
    pub cost: f64,
    pub duration_ms: u64,
    pub tool_calls: Vec<ToolCallRecord>,
    pub error: Option<String>,
}

/// 工具调用记录
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// Agent 执行事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// 开始执行
    Started { agent_id: String, prompt: String },

    /// LLM 响应
    LlmResponse { content: String, tool_calls: Vec<ToolCallRequest> },

    /// 工具调用开始
    ToolCallStart { name: String, arguments: serde_json::Value },

    /// 工具调用完成
    ToolCallEnd { name: String, result: String, success: bool },

    /// 迭代完成
    IterationComplete { iteration: u32, tokens_used: u64 },

    /// 执行完成
    Completed { result: AgentResult },

    /// 执行失败
    Failed { error: String },
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Agent 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
