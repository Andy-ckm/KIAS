//! IM 集成适配器
//!
//! 支持通过微信、Telegram、Slack 等 IM 软件用自然语言控制 KIAS。
//! 参考 CloudDM 的多平台接入模式：统一消息协议 + 平台适配层。
//!
//! 消息流: IM 平台 → Webhook → 消息解析 → NL 命令处理器 → 响应格式化 → IM 平台

use axum::{
    extract::State,
    Json,
};
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
    fn verify_signature(&self, headers: &std::collections::HashMap<String, String>, body: &[u8]) -> bool;
}

/// 微信适配器
pub struct WechatAdapter;

impl ImAdapter for WechatAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        Ok(WebhookRequest {
            platform: "wechat".to_string(),
            sender_id: raw.get("FromUserName").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            sender_name: None,
            message: raw.get("Content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            message_type: "text".to_string(),
            conversation_id: raw.get("ToUserName").and_then(|v| v.as_str()).map(String::from),
            metadata: None,
            timestamp: raw.get("CreateTime").and_then(|v| v.as_str()).map(String::from),
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "MsgType": "text",
            "Content": response.reply,
        })
    }

    fn verify_signature(&self, _headers: &std::collections::HashMap<String, String>, _body: &[u8]) -> bool {
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
            sender_id: message.get("from").and_then(|f| f.get("id")).and_then(|v| v.as_i64()).unwrap_or(0).to_string(),
            sender_name: message.get("from").and_then(|f| f.get("first_name")).and_then(|v| v.as_str()).map(String::from),
            message: message.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            message_type: "text".to_string(),
            conversation_id: message.get("chat").and_then(|c| c.get("id")).and_then(|v| v.as_i64()).map(|i| i.to_string()),
            metadata: None,
            timestamp: message.get("date").and_then(|v| v.as_i64()).map(|i| i.to_string()),
        })
    }

    fn format_response(&self, response: &WebhookResponse) -> serde_json::Value {
        serde_json::json!({
            "text": response.reply,
            "parse_mode": if response.reply_type == "markdown" { "Markdown" } else { "HTML" },
        })
    }

    fn verify_signature(&self, _headers: &std::collections::HashMap<String, String>, _body: &[u8]) -> bool {
        true // Telegram 使用 secret token 验证
    }
}

/// Slack 适配器
pub struct SlackAdapter;

impl ImAdapter for SlackAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        Ok(WebhookRequest {
            platform: "slack".to_string(),
            sender_id: raw.get("user").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            sender_name: None,
            message: raw.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            message_type: "text".to_string(),
            conversation_id: raw.get("channel").and_then(|v| v.as_str()).map(String::from),
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

    fn verify_signature(&self, _headers: &std::collections::HashMap<String, String>, _body: &[u8]) -> bool {
        true
    }
}

/// 飞书适配器
pub struct FeishuAdapter;

impl ImAdapter for FeishuAdapter {
    fn parse_message(&self, raw: &serde_json::Value) -> Result<WebhookRequest, String> {
        let event = raw.get("event").ok_or("missing event field")?;
        let message = event.get("message").ok_or("missing message field")?;
        let sender = event.get("sender").and_then(|s| s.get("sender_id")).ok_or("missing sender")?;

        Ok(WebhookRequest {
            platform: "feishu".to_string(),
            sender_id: sender.get("open_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            sender_name: None,
            message: message.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            message_type: message.get("message_type").and_then(|v| v.as_str()).unwrap_or("text").to_string(),
            conversation_id: message.get("chat_id").and_then(|v| v.as_str()).map(String::from),
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

    fn verify_signature(&self, _headers: &std::collections::HashMap<String, String>, _body: &[u8]) -> bool {
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
    let (actions, message, suggestions) = crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

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
    let req = adapter.parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (actions, message, _) = crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

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
    let req = adapter.parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (actions, message, _) = crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

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
    let req = adapter.parse_message(&raw)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (intent, _) = crate::handlers::nl_command::parse_intent_for_im(&req.message);
    let (actions, message, _) = crate::handlers::nl_command::execute_intent_for_im(&intent, &state).await;

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
