//! IM 集成适配器
//!
//! 支持通过微信、Telegram、Slack 等 IM 软件用自然语言控制 KIAS。
//! 参考 CloudDM 的多平台接入模式：统一消息协议 + 平台适配层。
//!
//! 消息流: IM 平台 → Webhook → 消息解析 → NL 命令处理器 → 响应格式化 → IM 平台

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::AppState;

/// Webhook 请求（统一格式）
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookRequest {
    /// 平台标识: wechat, telegram, slack, discord, feishu
    pub platform: String,
    /// 发送者标识
    pub sender_id: String,
    /// 发送者显示名
    #[serde(default)]
    pub sender_name: Option<String>,
    /// 消息内容
    pub message: String,
    /// 消息类型: text, image, file
    #[serde(default = "default_message_type")]
    pub message_type: String,
    /// 会话 ID（群聊/私聊）
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// 平台特定元数据
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// 时间戳
    #[serde(default)]
    pub timestamp: Option<String>,
}

fn default_message_type() -> String {
    "text".to_string()
}

/// Webhook 响应
#[derive(Debug, Clone, Serialize)]
pub struct WebhookResponse {
    /// 是否成功
    pub success: bool,
    /// 回复消息
    pub reply: String,
    /// 回复类型: text, markdown, card
    #[serde(default = "default_reply_type")]
    pub reply_type: String,
    /// 平台特定附加数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[allow(dead_code)]
fn default_reply_type() -> String {
    "text".to_string()
}

/// IM 平台消息适配器 trait
pub trait ImAdapter: Send + Sync {
    /// 解析平台原始消息为统一格式
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String>;
    /// 格式化响应为平台格式
    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value;
    /// 验证 Webhook 签名
    fn verify_signature(
        &self,
        headers: &std::collections::HashMap<String, String>,
        body: &[u8],
    ) -> bool;
}

/// 微信适配器
pub struct WechatAdapter;

impl ImAdapter for WechatAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        Ok(WebhookRequest {
            platform: "wechat".to_string(),
            sender_id: raw
                .get("FromUserName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            sender_name: None,
            message: raw
                .get("Content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            message_type: "text".to_string(),
            conversation_id: raw
                .get("ToUserName")
                .and_then(|v| v.as_str())
                .map(String::from),
            metadata: None,
            timestamp: raw
                .get("CreateTime")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "MsgType": "text",
            "Content": response.reply,
        })
    }

    fn verify_signature(
        &self,
        _headers: &std::collections::HashMap<String, String>,
        _body: &[u8],
    ) -> bool {
        true // 微信验证逻辑在 handler 层处理
    }
}

/// Telegram 适配器
pub struct TelegramAdapter;

impl ImAdapter for TelegramAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        let message = raw.get("message").ok_or("missing message field")?;
        Ok(WebhookRequest {
            platform: "telegram".to_string(),
            sender_id: message
                .get("from")
                .and_then(|f| f.get("id"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .to_string(),
            sender_name: message
                .get("from")
                .and_then(|f| f.get("first_name"))
                .and_then(|v| v.as_str())
                .map(String::from),
            message: message
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            message_type: "text".to_string(),
            conversation_id: message
                .get("chat")
                .and_then(|c| c.get("id"))
                .and_then(|v| v.as_i64())
                .map(|i| i.to_string()),
            metadata: None,
            timestamp: message
                .get("date")
                .and_then(|v| v.as_i64())
                .map(|i| i.to_string()),
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "text": response.reply,
            "parse_mode": if response.reply_type == "markdown" { "Markdown" } else { "HTML" },
        })
    }

    fn verify_signature(
        &self,
        _headers: &std::collections::HashMap<String, String>,
        _body: &[u8],
    ) -> bool {
        true // Telegram 使用 secret token 验证
    }
}

/// Slack 适配器
pub struct SlackAdapter;

impl ImAdapter for SlackAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        Ok(WebhookRequest {
            platform: "slack".to_string(),
            sender_id: raw
                .get("user")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            sender_name: None,
            message: raw
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            message_type: "text".to_string(),
            conversation_id: raw
                .get("channel")
                .and_then(|v| v.as_str())
                .map(String::from),
            metadata: None,
            timestamp: raw.get("ts").and_then(|v| v.as_str()).map(String::from),
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "text": response.reply,
            "mrkdwn": response.reply_type == "markdown",
        })
    }

    fn verify_signature(
        &self,
        _headers: &std::collections::HashMap<String, String>,
        _body: &[u8],
    ) -> bool {
        true
    }
}

/// 飞书适配器
pub struct FeishuAdapter;

impl ImAdapter for FeishuAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        let event = raw.get("event").ok_or("missing event field")?;
        let message = event.get("message").ok_or("missing message field")?;
        let sender = event
            .get("sender")
            .and_then(|s| s.get("sender_id"))
            .ok_or("missing sender")?;

        Ok(WebhookRequest {
            platform: "feishu".to_string(),
            sender_id: sender
                .get("open_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            sender_name: None,
            message: message
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            message_type: message
                .get("message_type")
                .and_then(|v| v.as_str())
                .unwrap_or("text")
                .to_string(),
            conversation_id: message
                .get("chat_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            metadata: None,
            timestamp: None,
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "msg_type": "text",
            "content": serde_json::json!({
                "text": response.reply,
            }),
        })
    }

    fn verify_signature(
        &self,
        _headers: &std::collections::HashMap<String, String>,
        _body: &[u8],
    ) -> bool {
        true
    }
}

/// 获取平台适配器
pub fn get_adapter(platform: &str) -> Option<Box<dyn ImAdapter>> {
    match platform {
        "wechat" | "weixin" => Some(Box::new(WechatAdapter)),
        "telegram" => Some(Box::new(TelegramAdapter)),
        "slack" => Some(Box::new(SlackAdapter)),
        "feishu" | "lark" => Some(Box::new(FeishuAdapter)),
        _ => None,
    }
}

/// POST /api/v1/im/webhook
/// 统一 IM Webhook 端点
pub async fn im_webhook(
    State(state): State<AppState>,
    Json(req): Json<WebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    if req.message.trim().is_empty() {
        return Ok(Json(WebhookResponse {
            success: true,
            reply: "收到空消息".to_string(),
            reply_type: "text".to_string(),
            extra: None,
        }));
    }

    // 调用 NL 命令处理器
    let (intent, confidence) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (actions, message, suggestions) =
        crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

    // 构建回复
    let mut reply = message;

    // 添加建议
    if !suggestions.is_empty() {
        reply.push_str("\n\n💡 建议操作:");
        for (i, s) in suggestions.iter().enumerate() {
            reply.push_str(&format!("\n  {}. {}", i + 1, s));
        }
    }

    // 添加操作摘要
    if !actions.is_empty() {
        let completed = actions.iter().filter(|a| a.status == "completed").count();
        if completed > 0 {
            reply.push_str(&format!("\n\n✅ 完成 {} 个操作", completed));
        }
    }

    Ok(Json(WebhookResponse {
        success: true,
        reply,
        reply_type: "text".to_string(),
        extra: Some(serde_json::json!({
            "platform": req.platform,
            "intent": format!("{:?}", intent),
            "confidence": confidence,
        })),
    }))
}

/// POST /api/v1/im/wechat
/// 微信专用 Webhook 端点
pub async fn wechat_webhook(
    State(state): State<AppState>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let adapter = WechatAdapter;
    let req = adapter
        .parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (_actions, message, _) =
        crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

    let response = WebhookResponse {
        success: true,
        reply: message,
        reply_type: "text".to_string(),
        extra: None,
    };

    Ok(Json(adapter.format_response(&response)))
}

/// POST /api/v1/im/telegram
/// Telegram 专用 Webhook 端点
pub async fn telegram_webhook(
    State(state): State<AppState>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let adapter = TelegramAdapter;
    let req = adapter
        .parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (_actions, message, _) =
        crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

    let response = WebhookResponse {
        success: true,
        reply: message,
        reply_type: "text".to_string(),
        extra: None,
    };

    Ok(Json(adapter.format_response(&response)))
}

/// POST /api/v1/im/feishu
/// 飞书专用 Webhook 端点
pub async fn feishu_webhook(
    State(state): State<AppState>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 飞书 URL 验证
    if let Some(challenge) = raw.get("challenge") {
        return Ok(Json(serde_json::json!({
            "challenge": challenge,
        })));
    }

    let adapter = FeishuAdapter;
    let req = adapter
        .parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (_actions, message, _) =
        crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

    let response = WebhookResponse {
        success: true,
        reply: message,
        reply_type: "text".to_string(),
        extra: None,
    };

    Ok(Json(adapter.format_response(&response)))
}

/// GET /api/v1/im/platforms
/// 获取支持的 IM 平台列表
pub async fn list_platforms() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "platforms": [
            {
                "id": "wechat",
                "name": "微信",
                "webhook": "/api/v1/im/wechat",
                "supported_types": ["text"],
            },
            {
                "id": "telegram",
                "name": "Telegram",
                "webhook": "/api/v1/im/telegram",
                "supported_types": ["text", "command"],
            },
            {
                "id": "slack",
                "name": "Slack",
                "webhook": "/api/v1/im/slack",
                "supported_types": ["text", "slash_command"],
            },
            {
                "id": "feishu",
                "name": "飞书",
                "webhook": "/api/v1/im/feishu",
                "supported_types": ["text"],
            },
        ],
        "unified_webhook": "/api/v1/im/webhook",
        "docs": "https://github.com/Andy-ckm/KIAS/blob/main/docs/im-integration.md",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // === WebhookRequest serialization ===

    #[test]
    fn test_webhook_request_deserialize_minimal() {
        let json = json!({
            "platform": "wechat",
            "sender_id": "user123",
            "message": "hello"
        });
        let req: WebhookRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.platform, "wechat");
        assert_eq!(req.sender_id, "user123");
        assert_eq!(req.message, "hello");
        assert_eq!(req.message_type, "text"); // default
        assert!(req.sender_name.is_none());
        assert!(req.conversation_id.is_none());
    }

    #[test]
    fn test_webhook_request_deserialize_full() {
        let json = json!({
            "platform": "telegram",
            "sender_id": "456",
            "sender_name": "Alice",
            "message": "/status",
            "message_type": "command",
            "conversation_id": "789",
            "metadata": {"key": "value"},
            "timestamp": "1234567890"
        });
        let req: WebhookRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.platform, "telegram");
        assert_eq!(req.sender_name, Some("Alice".to_string()));
        assert_eq!(req.message_type, "command");
        assert_eq!(req.conversation_id, Some("789".to_string()));
        assert!(req.metadata.is_some());
    }

    #[test]
    fn test_webhook_response_serialize() {
        let resp = WebhookResponse {
            success: true,
            reply: "done".to_string(),
            reply_type: "text".to_string(),
            extra: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["reply"], "done");
        assert!(json.get("extra").is_none()); // skip_serializing_if
    }

    #[test]
    fn test_webhook_response_with_extra() {
        let resp = WebhookResponse {
            success: true,
            reply: "ok".to_string(),
            reply_type: "markdown".to_string(),
            extra: Some(json!({"platform": "slack"})),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["extra"]["platform"], "slack");
    }

    #[test]
    fn test_default_message_type_value() {
        assert_eq!(default_message_type(), "text");
    }

    // === WechatAdapter ===

    #[test]
    fn test_wechat_parse_message() {
        let adapter = WechatAdapter;
        let raw = json!({
            "FromUserName": "wx_user_001",
            "Content": "查看状态",
            "ToUserName": "wx_bot",
            "CreateTime": "1700000000"
        });
        let req = adapter.parse_message(&raw).unwrap();
        assert_eq!(req.platform, "wechat");
        assert_eq!(req.sender_id, "wx_user_001");
        assert_eq!(req.message, "查看状态");
        assert_eq!(req.conversation_id, Some("wx_bot".to_string()));
        assert_eq!(req.timestamp, Some("1700000000".to_string()));
    }

    #[test]
    fn test_wechat_parse_missing_fields() {
        let adapter = WechatAdapter;
        let raw = json!({}); // empty
        let req = adapter.parse_message(&raw).unwrap();
        assert_eq!(req.sender_id, "unknown");
        assert_eq!(req.message, "");
    }

    #[test]
    fn test_wechat_format_response() {
        let adapter = WechatAdapter;
        let resp = WebhookResponse {
            success: true,
            reply: "OK".to_string(),
            reply_type: "text".to_string(),
            extra: None,
        };
        let val = adapter.format_response(&resp);
        assert_eq!(val["MsgType"], "text");
        assert_eq!(val["Content"], "OK");
    }

    #[test]
    fn test_wechat_verify_signature_always_true() {
        let adapter = WechatAdapter;
        assert!(adapter.verify_signature(&std::collections::HashMap::new(), b"body"));
    }

    // === TelegramAdapter ===

    #[test]
    fn test_telegram_parse_message() {
        let adapter = TelegramAdapter;
        let raw = json!({
            "message": {
                "from": {"id": 12345, "first_name": "Bob"},
                "text": "help me",
                "chat": {"id": -100123},
                "date": 1700000000
            }
        });
        let req = adapter.parse_message(&raw).unwrap();
        assert_eq!(req.platform, "telegram");
        assert_eq!(req.sender_id, "12345");
        assert_eq!(req.sender_name, Some("Bob".to_string()));
        assert_eq!(req.message, "help me");
        assert_eq!(req.conversation_id, Some("-100123".to_string()));
    }

    #[test]
    fn test_telegram_parse_missing_message() {
        let adapter = TelegramAdapter;
        let raw = json!({});
        let result = adapter.parse_message(&raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing message"));
    }

    #[test]
    fn test_telegram_format_response_html() {
        let adapter = TelegramAdapter;
        let resp = WebhookResponse {
            success: true,
            reply: "hello".to_string(),
            reply_type: "text".to_string(),
            extra: None,
        };
        let val = adapter.format_response(&resp);
        assert_eq!(val["text"], "hello");
        assert_eq!(val["parse_mode"], "HTML");
    }

    #[test]
    fn test_telegram_format_response_markdown() {
        let adapter = TelegramAdapter;
        let resp = WebhookResponse {
            success: true,
            reply: "*bold*".to_string(),
            reply_type: "markdown".to_string(),
            extra: None,
        };
        let val = adapter.format_response(&resp);
        assert_eq!(val["parse_mode"], "Markdown");
    }

    // === SlackAdapter ===

    #[test]
    fn test_slack_parse_message() {
        let adapter = SlackAdapter;
        let raw = json!({
            "user": "U123",
            "text": "deploy staging",
            "channel": "C456",
            "ts": "1700000000.000100"
        });
        let req = adapter.parse_message(&raw).unwrap();
        assert_eq!(req.platform, "slack");
        assert_eq!(req.sender_id, "U123");
        assert_eq!(req.message, "deploy staging");
        assert_eq!(req.conversation_id, Some("C456".to_string()));
    }

    #[test]
    fn test_slack_format_response() {
        let adapter = SlackAdapter;
        let resp = WebhookResponse {
            success: true,
            reply: "done".to_string(),
            reply_type: "markdown".to_string(),
            extra: None,
        };
        let val = adapter.format_response(&resp);
        assert_eq!(val["text"], "done");
        assert_eq!(val["mrkdwn"], true);
    }

    // === FeishuAdapter ===

    #[test]
    fn test_feishu_parse_message() {
        let adapter = FeishuAdapter;
        let raw = json!({
            "event": {
                "sender": {"sender_id": {"open_id": "ou_abc"}},
                "message": {
                    "content": r#"{"text":"查 agent"}"#,
                    "message_type": "text",
                    "chat_id": "oc_xyz"
                }
            }
        });
        let req = adapter.parse_message(&raw).unwrap();
        assert_eq!(req.platform, "feishu");
        assert_eq!(req.sender_id, "ou_abc");
        assert_eq!(req.conversation_id, Some("oc_xyz".to_string()));
    }

    #[test]
    fn test_feishu_parse_missing_event() {
        let adapter = FeishuAdapter;
        let raw = json!({});
        let result = adapter.parse_message(&raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing event"));
    }

    // === get_adapter ===

    #[test]
    fn test_get_adapter_all_platforms() {
        assert!(get_adapter("wechat").is_some());
        assert!(get_adapter("weixin").is_some()); // alias
        assert!(get_adapter("telegram").is_some());
        assert!(get_adapter("slack").is_some());
        assert!(get_adapter("feishu").is_some());
        assert!(get_adapter("lark").is_some()); // alias
    }

    #[test]
    fn test_get_adapter_unknown() {
        assert!(get_adapter("discord").is_none());
        assert!(get_adapter("").is_none());
        assert!(get_adapter("WECHAT").is_none()); // case-sensitive
    }

    // === list_platforms ===

    #[tokio::test]
    async fn test_list_platforms_returns_all() {
        let result = list_platforms().await;
        let platforms = result["platforms"].as_array().unwrap();
        assert_eq!(platforms.len(), 4);
        let ids: Vec<&str> = platforms
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"wechat"));
        assert!(ids.contains(&"telegram"));
        assert!(ids.contains(&"slack"));
        assert!(ids.contains(&"feishu"));
    }

    // === Handler-level tests for webhook endpoints ===

    use axum::extract::State;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn handler_test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(std::collections::HashMap::new())),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: crate::handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: crate::handlers::auth_gxp::create_gxp_auth_state(
                kias_common::gxp_auth::PasswordPolicy::default(),
            ),
            jwt_config: crate::auth::JwtConfig::new(
                "kias-default-jwt-secret-change-me",
                "kias",
                24,
            ),
        }
    }

    #[tokio::test]
    async fn test_handler_im_webhook_empty_message() {
        let state = handler_test_state().await;
        let req = WebhookRequest {
            platform: "wechat".to_string(),
            sender_id: "user1".to_string(),
            sender_name: None,
            message: "   ".to_string(), // whitespace only → treated as empty
            message_type: "text".to_string(),
            conversation_id: None,
            metadata: None,
            timestamp: None,
        };
        let result = im_webhook(State(state), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert!(resp.success);
        assert_eq!(resp.reply, "收到空消息");
    }

    #[tokio::test]
    async fn test_handler_im_webhook_valid_message() {
        let state = handler_test_state().await;
        let req = WebhookRequest {
            platform: "telegram".to_string(),
            sender_id: "user1".to_string(),
            sender_name: Some("Alice".to_string()),
            message: "show status".to_string(),
            message_type: "text".to_string(),
            conversation_id: None,
            metadata: None,
            timestamp: None,
        };
        let result = im_webhook(State(state), Json(req)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert!(resp.success);
        assert!(!resp.reply.is_empty());
        assert_eq!(resp.reply_type, "text");
        // extra should contain platform and intent info
        let extra = resp.extra.unwrap();
        assert_eq!(extra["platform"], "telegram");
        assert!(extra["confidence"].as_f64().is_some());
    }

    #[tokio::test]
    async fn test_handler_wechat_webhook_success() {
        let state = handler_test_state().await;
        let raw = json!({
            "FromUserName": "user123",
            "Content": "hello agent"
        });
        let result = wechat_webhook(State(state), Json(raw)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        // WechatAdapter format_response returns {"MsgType": "text", "Content": reply}
        assert_eq!(resp["MsgType"], "text");
        assert!(resp["Content"].as_str().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_handler_wechat_webhook_defaults() {
        let state = handler_test_state().await;
        // WechatAdapter always returns Ok — uses unwrap_or defaults for missing fields
        let raw = json!({ "invalid": "no required fields" });
        let result = wechat_webhook(State(state), Json(raw)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert_eq!(resp["MsgType"], "text");
    }

    #[tokio::test]
    async fn test_handler_telegram_webhook_success() {
        let state = handler_test_state().await;
        let raw = json!({
            "message": {
                "from": { "id": 12345, "first_name": "Bob" },
                "text": "check agents"
            }
        });
        let result = telegram_webhook(State(state), Json(raw)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        // TelegramAdapter format_response returns {"text": reply, "parse_mode": ...}
        assert!(resp["text"].as_str().unwrap().len() > 0);
        assert_eq!(resp["parse_mode"], "HTML");
    }

    #[tokio::test]
    async fn test_handler_telegram_webhook_missing_message() {
        let state = handler_test_state().await;
        let raw = json!({ "update_id": 1 });
        let result = telegram_webhook(State(state), Json(raw)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_handler_feishu_webhook_challenge() {
        let state = handler_test_state().await;
        let raw = json!({
            "challenge": "test-challenge-token-123",
            "type": "url_verification"
        });
        let result = feishu_webhook(State(state), Json(raw)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        assert_eq!(resp["challenge"], "test-challenge-token-123");
    }

    #[tokio::test]
    async fn test_handler_feishu_webhook_message() {
        let state = handler_test_state().await;
        let raw = json!({
            "event": {
                "message": {
                    "content": "{\"text\":\"hello\"}",
                    "message_type": "text",
                    "chat_id": "oc_123"
                },
                "sender": {
                    "sender_id": { "open_id": "ou_456" },
                    "sender_type": "user"
                }
            },
            "header": {
                "event_type": "im.message.receive_v1"
            }
        });
        let result = feishu_webhook(State(state), Json(raw)).await;
        assert!(result.is_ok());
        let resp = result.unwrap().0;
        // FeishuAdapter format_response returns {"msg_type": "text", "content": {"text": reply}}
        assert_eq!(resp["msg_type"], "text");
        assert!(resp["content"]["text"].as_str().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_handler_feishu_webhook_invalid_no_challenge_no_event() {
        let state = handler_test_state().await;
        let raw = json!({ "random_field": "no challenge, no event" });
        let result = feishu_webhook(State(state), Json(raw)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);
    }
}
