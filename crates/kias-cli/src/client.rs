//! KIAS API 客户端模块
//!
//! 封装与 KIAS API Server 的 HTTP 通信。

use futures_util::StreamExt;
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

/// WebSocket event type (mirrors API server EventType)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WsEventType {
    AgentStatusChanged,
    AgentCreated,
    AgentDeleted,
    NodeHealthChanged,
    TaskCompleted,
    TaskFailed,
    WorkflowUpdate,
    SchedulerDecision,
    SystemAlert,
    A2aTaskSubmitted,
    A2aTaskWorking,
    A2aTaskCompleted,
    A2aTaskCancelled,
    A2aTaskDeleted,
}

/// WebSocket event received from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    #[serde(rename = "type")]
    pub event_type: WsEventType,
    pub data: serde_json::Value,
    pub timestamp: String,
}

/// Subscription filter sent to the WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsSubscription {
    pub subscribe: Vec<WsEventType>,
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
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/workflows")
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

    // ─── WebSocket 实时事件流 ─────────────────────────────────────

    /// 连接 WebSocket 事件流
    ///
    /// 建立到 `/ws` 的 WebSocket 连接，可选发送事件类型订阅过滤。
    /// 返回一个 `SplitStream`，调用方通过 `.next().await` 逐条接收事件。
    pub async fn stream_events(
        &self,
        event_types: Vec<WsEventType>,
    ) -> Result<
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        Box<dyn std::error::Error>,
    > {
        // 将 http://host:port 转换为 ws://host:port
        let ws_url = self
            .base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let url = format!("{}/ws", ws_url);

        let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await?;

        let (mut write, read) = ws_stream.split();

        // 发送订阅过滤
        if !event_types.is_empty() {
            let sub = WsSubscription {
                subscribe: event_types,
            };
            let msg = serde_json::to_string(&sub)?;
            use futures_util::SinkExt;
            write
                .send(tokio_tungstenite::tungstenite::Message::Text(msg))
                .await?;
        }

        Ok(read)
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

    #[test]
    fn test_ws_event_deserialize() {
        let json = r#"{"type":"agent_status_changed","data":{"agent_id":"a-001","old_status":"pending","new_status":"running"},"timestamp":"2025-01-01T00:00:00Z"}"#;
        let event: WsEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, WsEventType::AgentStatusChanged);
        assert_eq!(event.data["agent_id"], "a-001");
        assert_eq!(event.timestamp, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_ws_subscription_serialize() {
        let sub = WsSubscription {
            subscribe: vec![WsEventType::AgentCreated, WsEventType::TaskCompleted],
        };
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("agent_created"));
        assert!(json.contains("task_completed"));
    }

    #[test]
    fn test_ws_event_type_roundtrip() {
        let types = vec![
            WsEventType::AgentStatusChanged,
            WsEventType::AgentCreated,
            WsEventType::AgentDeleted,
            WsEventType::TaskCompleted,
            WsEventType::TaskFailed,
            WsEventType::SystemAlert,
        ];
        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let back: WsEventType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_workflow_info_deserialize() {
        let json = r#"{"id":"wf-001","name":"pipeline","status":"running","entry":"start","created_at":"2024-01-01T00:00:00Z"}"#;
        let info: WorkflowInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "wf-001");
        assert_eq!(info.name, "pipeline");
        assert_eq!(info.status, "running");
        assert_eq!(info.entry.as_deref(), Some("start"));
    }

    #[test]
    fn test_node_info_deserialize() {
        let json = r#"{"id":"n-001","name":"node-1","address":"10.0.0.1:8080","status":"ready","agents":["a1","a2"]}"#;
        let info: NodeInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "n-001");
        assert_eq!(info.address, "10.0.0.1:8080");
        assert_eq!(info.agents.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_metrics_summary_deserialize() {
        let json = r#"{"total_requests":1000,"total_tokens":500000,"total_cost":12.5,"avg_latency_ms":234.5}"#;
        let metrics: MetricsSummary = serde_json::from_str(json).unwrap();
        assert_eq!(metrics.total_requests, 1000);
        assert_eq!(metrics.total_tokens, 500000);
        assert!((metrics.total_cost - 12.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_agent_spec_info_deserialize() {
        let json = r#"{"name":"my-agent","image":"python:3.11","priority":"high"}"#;
        let spec: AgentSpecInfo = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name, "my-agent");
        assert_eq!(spec.image.as_deref(), Some("python:3.11"));
        assert_eq!(spec.priority.as_deref(), Some("high"));
    }

    #[test]
    fn test_agent_spec_info_defaults() {
        let json = r#"{}"#;
        let spec: AgentSpecInfo = serde_json::from_str(json).unwrap();
        assert!(spec.name.is_empty());
        assert!(spec.image.is_none());
        assert!(spec.priority.is_none());
    }

    #[test]
    fn test_url_encoding_empty_string() {
        assert_eq!(urlencoding::encode(""), "");
    }

    #[test]
    fn test_url_encoding_special_chars() {
        assert_eq!(urlencoding::encode("a=b&c=d"), "a%3Db%26c%3Dd");
        assert_eq!(urlencoding::encode("hello@world"), "hello%40world");
        assert_eq!(urlencoding::encode("100%"), "100%25");
    }

    #[test]
    fn test_url_encoding_preserves_safe_chars() {
        assert_eq!(urlencoding::encode("test-path_file.txt"), "test-path_file.txt");
        assert_eq!(urlencoding::encode("~home"), "~home");
    }

    #[test]
    fn test_agent_info_defaults() {
        let json = r#"{"id":"a-002"}"#;
        let info: AgentInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "a-002");
        assert!(info.name.is_empty());
        assert!(info.status.is_empty());
        assert!(info.model.is_none());
        assert!(info.created_at.is_none());
        assert!(info.updated_at.is_none());
        assert!(info.spec.is_none());
    }

    #[test]
    fn test_agent_info_with_spec() {
        let json = r#"{"id":"a-003","spec":{"name":"my-agent","image":"rust:1.77"}}"#;
        let info: AgentInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "a-003");
        assert_eq!(info.spec.as_ref().unwrap().name, "my-agent");
        assert_eq!(info.spec.as_ref().unwrap().image.as_deref(), Some("rust:1.77"));
    }

    #[test]
    fn test_cluster_status_legacy_fields() {
        let json = r#"{"status":"degraded","version":"1.0.0"}"#;
        let status: ClusterStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status.as_deref(), Some("degraded"));
        assert_eq!(status.version.as_deref(), Some("1.0.0"));
        assert!(status.overall.is_none());
        assert!(status.nodes.is_none());
    }

    #[test]
    fn test_workflow_info_minimal() {
        let json = r#"{"id":"wf-002","name":"simple","status":"pending"}"#;
        let info: WorkflowInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "wf-002");
        assert!(info.entry.is_none());
        assert!(info.created_at.is_none());
    }

    #[test]
    fn test_node_info_minimal() {
        let json = r#"{"id":"n-002","name":"node-2","address":"10.0.0.2:9090","status":"offline"}"#;
        let info: NodeInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "n-002");
        assert!(info.agents.is_none());
    }

    #[test]
    fn test_model_usage_deserialize() {
        let json = r#"{"model":"gpt-4","tokens":50000,"cost":1.5,"requests":100}"#;
        let usage: ModelUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.model, "gpt-4");
        assert_eq!(usage.tokens, 50000);
        assert!((usage.cost - 1.5).abs() < f64::EPSILON);
        assert_eq!(usage.requests, 100);
    }

    #[test]
    fn test_token_analytics_with_models() {
        let json = r#"{
            "total_tokens": 200000,
            "prompt_tokens": 120000,
            "completion_tokens": 80000,
            "total_cost": 3.0,
            "by_model": [
                {"model":"gpt-4","tokens":150000,"cost":2.5,"requests":80},
                {"model":"gpt-3.5","tokens":50000,"cost":0.5,"requests":20}
            ]
        }"#;
        let analytics: TokenAnalytics = serde_json::from_str(json).unwrap();
        assert_eq!(analytics.total_tokens, 200000);
        let models = analytics.by_model.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model, "gpt-4");
        assert_eq!(models[1].model, "gpt-3.5");
    }

}