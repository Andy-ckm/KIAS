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
    LlmResponse {
        content: String,
        tool_calls: Vec<ToolCallRequest>,
    },

    /// 工具调用开始
    ToolCallStart {
        name: String,
        arguments: serde_json::Value,
    },

    /// 工具调用完成
    ToolCallEnd {
        name: String,
        result: String,
        success: bool,
    },

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enabled);
        assert_eq!(config.allowed_paths, vec!["."]);
        assert!(!config.allow_network);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_agent_config_serialization() {
        let config = AgentConfig {
            name: "test-agent".to_string(),
            model: "gpt-4o".to_string(),
            system_prompt: "You are helpful".to_string(),
            max_iterations: 10,
            max_tokens: 4096,
            temperature: 0.7,
            tools: vec!["shell".to_string()],
            sandbox: SandboxConfig::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("gpt-4o"));
    }

    #[test]
    fn test_agent_config_deserialization() {
        let json = r#"{
            "name": "agent1",
            "model": "claude-sonnet-4-20250514",
            "system_prompt": "test",
            "max_iterations": 5,
            "max_tokens": 2048,
            "temperature": 0.5,
            "tools": [],
            "sandbox": {"enabled": false, "allowed_paths": [], "allow_network": true, "timeout_secs": 30}
        }"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "agent1");
        assert!(!config.sandbox.enabled);
        assert!(config.sandbox.allow_network);
    }

    #[test]
    fn test_agent_status_variants() {
        let statuses = vec![
            AgentStatus::Pending,
            AgentStatus::Running,
            AgentStatus::Completed,
            AgentStatus::Failed,
            AgentStatus::Cancelled,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_tool_call_request_serialization() {
        let req = ToolCallRequest {
            id: "call_1".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({"command": "ls"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("shell"));
        assert!(json.contains("ls"));
    }

    #[test]
    fn test_agent_event_tagged() {
        let event = AgentEvent::Started {
            agent_id: "a1".to_string(),
            prompt: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"Started\""));
        assert!(json.contains("a1"));
    }

    #[test]
    fn test_agent_result_serialization() {
        let result = AgentResult {
            success: true,
            output: "done".to_string(),
            iterations: 3,
            tokens_used: 1500,
            cost: 0.05,
            duration_ms: 2000,
            tool_calls: vec![],
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("1500"));
    }
}
