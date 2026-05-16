//! KIAS API 客户端模块
//!
//! 封装与 KIAS API Server 的 HTTP 通信。

#[allow(unused_imports)]
use futures_util::StreamExt; // TODO: WebSocket streaming 待实现
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// KIAS API 客户端
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

/// API 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub code: Option<String>,
}

/// 通用 API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: String,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// 分页列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
}

/// Agent 信息（来自 API Server）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub spec: Option<AgentSpecInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpecInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

/// 工作流信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub entry: Option<String>,
    pub created_at: Option<String>,
}

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub status: String,
    pub agents: Option<Vec<String>>,
}

/// 集群状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub overall: Option<String>,
    pub nodes: Option<Vec<serde_json::Value>>,
    pub total_agents: Option<u32>,
    pub running_agents: Option<u32>,
    // Legacy fields
    pub status: Option<String>,
    pub version: Option<String>,
}

/// 指标摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
}

/// Token 使用分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAnalytics {
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_cost: f64,
    pub by_model: Option<Vec<ModelUsage>>,
}

/// 模型使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub tokens: u64,
    pub cost: f64,
    pub requests: u64,
}

/// Agent 调用结果（来自 POST /api/v1/agents/{id}/invoke）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub run_id: String,
    pub agent_id: String,
    pub output: String,
    pub tokens_used: Option<u64>,
    pub cost: Option<f64>,
    pub duration_ms: u64,
}

impl ApiClient {
    /// 创建新的 API 客户端
    pub fn new(base_url: &str, api_key: Option<String>) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        })
    }

    /// 构建带认证头的请求
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, url);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }
        req
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, reqwest::Error> {
        let resp = self.request(reqwest::Method::GET, "/health").send().await?;
        Ok(resp.status().is_success())
    }

    // ─── Agent 操作 ───────────────────────────────────────────────

    /// 列出所有 Agent
    pub async fn list_agents(&self) -> Result<Vec<AgentInfo>, reqwest::Error> {
        let resp: ListResponse<AgentInfo> = self
            .request(reqwest::Method::GET, "/api/v1/agents")
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.items)
    }

    /// 创建 Agent
    pub async fn create_agent(
        &self,
        body: serde_json::Value,
    ) -> Result<AgentInfo, Box<dyn std::error::Error>> {
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/agents")
            .json(&body)
            .send()
            .await?;
        let raw: serde_json::Value = resp.json().await?;
        let data = raw.get("data").cloned().unwrap_or(raw);
        let mut info: AgentInfo = serde_json::from_value(data.clone())?;
        // API returns name inside spec.name
        if info.name.is_empty() {
            if let Some(spec) = &info.spec {
                info.name = spec.name.clone();
            } else if let Some(n) = data
                .get("spec")
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
            {
                info.name = n.to_string();
            }
        }
        Ok(info)
    }

    /// 获取 Agent 详情
    pub async fn get_agent(&self, id: &str) -> Result<AgentInfo, reqwest::Error> {
        let path = format!("/api/v1/agents/{}", id);
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }

    /// 调用 Agent（非交互式执行）
    ///
    /// POST /api/v1/agents/{id}/invoke
    pub async fn invoke_agent(
        &self,
        id: &str,
        prompt: &str,
        timeout_secs: Option<u64>,
    ) -> Result<AgentRunResult, reqwest::Error> {
        let path = format!("/api/v1/agents/{}/invoke", id);
        let body = serde_json::json!({
            "prompt": prompt,
            "timeout_secs": timeout_secs,
        });
        self.request(reqwest::Method::POST, &path)
            .json(&body)
            .send()
            .await?
            .json()
            .await
    }

    /// 删除 Agent
    pub async fn delete_agent(&self, id: &str) -> Result<bool, reqwest::Error> {
        let path = format!("/api/v1/agents/{}", id);
        let resp = self.request(reqwest::Method::DELETE, &path).send().await?;
        Ok(resp.status().is_success())
    }

    /// 更新 Agent 状态
    pub async fn update_agent_status(
        &self,
        id: &str,
        status: &str,
    ) -> Result<bool, reqwest::Error> {
        let path = format!("/api/v1/agents/{}/status", id);
        let resp = self
            .request(reqwest::Method::PATCH, &path)
            .json(&serde_json::json!({"status": status}))
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    // ─── Workflow 操作 ────────────────────────────────────────────

    /// 列出所有工作流
    pub async fn list_workflows(&self) -> Result<Vec<WorkflowInfo>, reqwest::Error> {
        #[derive(serde::Deserialize)]
        struct WorkflowSummary {
            workflows: Vec<WorkflowInfo>,
        }
        let resp: WorkflowSummary = self
            .request(reqwest::Method::GET, "/api/v1/workflows")
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.workflows)
    }

    /// 创建工作流
    pub async fn create_workflow(
        &self,
        body: serde_json::Value,
    ) -> Result<WorkflowInfo, Box<dyn std::error::Error>> {
        let resp = self.request(reqwest::Method::POST, "/api/v1/workflows")
            .json(&body)
            .send()
            .await?;
        let raw: serde_json::Value = resp.json().await?;
        let data = raw.get("data").cloned().unwrap_or(raw);
        Ok(serde_json::from_value(data)?)
    }

    /// 获取工作流详情
    pub async fn get_workflow(&self, id: &str) -> Result<WorkflowInfo, reqwest::Error> {
        let path = format!("/api/v1/workflows/{}", id);
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }

    /// 删除工作流
    pub async fn delete_workflow(&self, id: &str) -> Result<bool, reqwest::Error> {
        let path = format!("/api/v1/workflows/{}", id);
        let resp = self.request(reqwest::Method::DELETE, &path).send().await?;
        Ok(resp.status().is_success())
    }

    // ─── 节点操作 ─────────────────────────────────────────────────

    /// 列出所有节点
    pub async fn list_nodes(&self) -> Result<Vec<NodeInfo>, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/nodes")
            .send()
            .await?
            .json()
            .await
    }

    /// 获取节点详情
    pub async fn get_node(&self, id: &str) -> Result<NodeInfo, reqwest::Error> {
        let path = format!("/api/v1/nodes/{}", id);
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }

    /// 列出节点上的 Agent
    pub async fn list_node_agents(&self, id: &str) -> Result<Vec<AgentInfo>, reqwest::Error> {
        let path = format!("/api/v1/nodes/{}/agents", id);
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }

    // ─── 集群操作 ─────────────────────────────────────────────────

    /// 获取集群状态
    pub async fn cluster_status(&self) -> Result<ClusterStatus, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/cluster/status")
            .send()
            .await?
            .json()
            .await
    }

    // ─── 指标操作 ─────────────────────────────────────────────────

    /// 获取指标摘要
    pub async fn metrics_summary(&self) -> Result<MetricsSummary, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/metrics/summary")
            .send()
            .await?
            .json()
            .await
    }

    /// 获取 Agent 指标
    pub async fn agent_metrics(&self, id: &str) -> Result<serde_json::Value, reqwest::Error> {
        let path = format!("/api/v1/metrics/agents/{}", id);
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }

    // ─── Token 分析 ───────────────────────────────────────────────

    /// 获取 Token 使用分析
    pub async fn token_analytics(&self) -> Result<TokenAnalytics, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/tokens")
            .send()
            .await?
            .json()
            .await
    }

    // ─── 配置操作 ─────────────────────────────────────────────────

    /// 获取配置
    pub async fn get_config(&self) -> Result<serde_json::Value, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/config")
            .send()
            .await?
            .json()
            .await
    }

    /// 更新配置
    pub async fn update_config(&self, body: serde_json::Value) -> Result<bool, reqwest::Error> {
        let resp = self
            .request(reqwest::Method::PATCH, "/api/v1/config")
            .json(&body)
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// 获取配置审计日志
    pub async fn config_audit_log(&self) -> Result<serde_json::Value, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/config/audit-log")
            .send()
            .await?
            .json()
            .await
    }

    // ─── 调度器 ───────────────────────────────────────────────────

    /// 获取调度器状态
    pub async fn scheduler_status(&self) -> Result<serde_json::Value, reqwest::Error> {
        self.request(reqwest::Method::GET, "/api/v1/scheduler/status")
            .send()
            .await?
            .json()
            .await
    }

    // ─── 知识库搜索 ──────────────────────────────────────────────

    /// 搜索知识库
    pub async fn search_knowledge(&self, query: &str) -> Result<serde_json::Value, reqwest::Error> {
        let path = format!("/api/v1/knowledge/search?q={}", urlencoding::encode(query));
        self.request(reqwest::Method::GET, &path)
            .send()
            .await?
            .json()
            .await
    }
}

/// URL 编码辅助
mod urlencoding {
    /// 简单的 URL 编码
    pub fn encode(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len() * 3);
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char)
                }
                b' ' => encoded.push('+'),
                _ => {
                    encoded.push('%');
                    encoded.push_str(&format!("{:02X}", byte));
                }
            }
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encoding() {
        assert_eq!(urlencoding::encode("hello world"), "hello+world");
        assert_eq!(urlencoding::encode("test&foo"), "test%26foo");
        assert_eq!(urlencoding::encode("abc123"), "abc123");
    }

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new("http://localhost:8080", None);
        assert!(client.is_ok());
        let client = client.expect("client should be created");
        assert_eq!(client.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_api_client_new_with_trailing_slash() {
        let client = ApiClient::new("http://localhost:8080/", Some("key".to_string()));
        assert!(client.is_ok());
        let client = client.expect("client should be created");
        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.api_key, Some("key".to_string()));
    }

    #[test]
    fn test_agent_info_deserialize() {
        let json = r#"{
            "id": "a-001",
            "name": "test-agent",
            "status": "running",
            "model": "gpt-4",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": null
        }"#;
        let info: Result<AgentInfo, _> = serde_json::from_str(json);
        assert!(info.is_ok());
        let info = info.expect("should deserialize");
        assert_eq!(info.name, "test-agent");
        assert_eq!(info.status, "running");
    }

    #[test]
    fn test_cluster_status_deserialize() {
        let json = r#"{
            "overall": "healthy",
            "nodes": [{"id":"n1"},{"id":"n2"},{"id":"n3"}],
            "total_agents": 5,
            "running_agents": 3
        }"#;
        let status: Result<ClusterStatus, _> = serde_json::from_str(json);
        assert!(status.is_ok());
        let status = status.expect("should deserialize");
        assert_eq!(status.overall.as_deref(), Some("healthy"));
        assert_eq!(status.total_agents, Some(5));
    }

    #[test]
    fn test_token_analytics_deserialize() {
        let json = r#"{
            "total_tokens": 100000,
            "prompt_tokens": 60000,
            "completion_tokens": 40000,
            "total_cost": 1.5,
            "by_model": null
        }"#;
        let analytics: Result<TokenAnalytics, _> = serde_json::from_str(json);
        assert!(analytics.is_ok());
        let analytics = analytics.expect("should deserialize");
        assert_eq!(analytics.total_tokens, 100000);
    }

    #[test]
    fn test_agent_run_result_deserialize() {
        // InvokeResponse from POST /api/v1/agents/{id}/invoke
        let json = r#"{
            "run_id": "run-001",
            "agent_id": "agent-abc",
            "output": "Hello world",
            "tokens_used": 150,
            "cost": 0.003,
            "duration_ms": 1200
        }"#;
        let result: Result<AgentRunResult, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let result = result.expect("should deserialize");
        assert_eq!(result.run_id, "run-001");
        assert_eq!(result.agent_id, "agent-abc");
        assert_eq!(result.output, "Hello world");
        assert_eq!(result.tokens_used, Some(150));
        assert_eq!(result.cost, Some(0.003));
        assert_eq!(result.duration_ms, 1200);
    }
}
