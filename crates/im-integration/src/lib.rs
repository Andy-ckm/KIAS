//! IM集成模块 — 支持微信/Telegram/Slack/飞书
//!
//! 统一Webhook接口，消息解析和回复，支持多平台消息路由。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// IM平台类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ImPlatform {
    /// 微信公众号/企业微信
    Wechat,
    /// Telegram
    Telegram,
    /// Slack
    Slack,
    /// 飞书
    Feishu,
    /// 自定义Webhook
    Custom,
}

/// 统一消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMessage {
    /// 消息ID
    pub id: String,
    /// 平台
    pub platform: ImPlatform,
    /// 发送者ID
    pub sender_id: String,
    /// 发送者名称
    pub sender_name: Option<String>,
    /// 接收者ID（群聊/频道）
    pub receiver_id: Option<String>,
    /// 消息内容
    pub content: MessageContent,
    /// 消息类型
    pub message_type: MessageType,
    /// 时间戳
    pub timestamp: i64,
    /// 原始数据
    pub raw_data: Option<serde_json::Value>,
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    /// 文本消息
    Text(String),
    /// 图片消息
    Image {
        url: String,
        caption: Option<String>,
    },
    /// 文件消息
    File {
        url: String,
        filename: String,
        mime_type: Option<String>,
    },
    /// 位置消息
    Location {
        latitude: f64,
        longitude: f64,
        address: Option<String>,
    },
    /// 事件消息
    Event(EventType),
}

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// 用户关注
    Subscribe,
    /// 用户取消关注
    Unsubscribe,
    /// 加入群组
    JoinGroup,
    /// 离开群组
    LeaveGroup,
    /// 自定义事件
    Custom(String),
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// 私聊消息
    Private,
    /// 群聊消息
    Group,
    /// 频道消息
    Channel,
    /// 系统消息
    System,
}

/// 回复消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyMessage {
    /// 回复内容
    pub content: MessageContent,
    /// 回复目标消息ID（用于回复特定消息）
    pub reply_to: Option<String>,
    /// 是否静默发送
    pub silent: bool,
}

/// Webhook请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRequest {
    /// 平台
    pub platform: ImPlatform,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 请求体
    pub body: serde_json::Value,
    /// 查询参数
    pub query_params: HashMap<String, String>,
}

/// Webhook响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    /// 状态码
    pub status_code: u16,
    /// 响应体
    pub body: serde_json::Value,
    /// 是否需要回复
    pub should_reply: bool,
    /// 回复消息
    pub reply: Option<ReplyMessage>,
}

/// 平台适配器 trait
pub trait PlatformAdapter: Send + Sync {
    /// 解析Webhook请求
    fn parse_webhook(&self, request: &WebhookRequest) -> Result<UnifiedMessage, String>;

    /// 构建回复
    fn build_reply(
        &self,
        message: &UnifiedMessage,
        reply: &ReplyMessage,
    ) -> Result<WebhookResponse, String>;

    /// 验证请求签名
    fn verify_signature(&self, headers: &HashMap<String, String>, body: &[u8]) -> bool;

    /// 获取平台类型
    fn platform_type(&self) -> ImPlatform;
}

/// 微信适配器
pub struct WechatAdapter {
    token: String,
    encoding_aes_key: Option<String>,
}

impl WechatAdapter {
    pub fn new(token: String, encoding_aes_key: Option<String>) -> Self {
        Self {
            token,
            encoding_aes_key,
        }
    }
}

impl PlatformAdapter for WechatAdapter {
    fn parse_webhook(&self, request: &WebhookRequest) -> Result<UnifiedMessage, String> {
        let body = &request.body;

        let msg_type = body["MsgType"].as_str().unwrap_or("text");
        let content = match msg_type {
            "text" => MessageContent::Text(body["Content"].as_str().unwrap_or("").to_string()),
            "image" => MessageContent::Image {
                url: body["PicUrl"].as_str().unwrap_or("").to_string(),
                caption: None,
            },
            _ => MessageContent::Text(body["Content"].as_str().unwrap_or("").to_string()),
        };

        Ok(UnifiedMessage {
            id: body["MsgId"].as_str().unwrap_or("unknown").to_string(),
            platform: ImPlatform::Wechat,
            sender_id: body["FromUserName"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            sender_name: None,
            receiver_id: body["ToUserName"].as_str().map(|s| s.to_string()),
            content,
            message_type: MessageType::Private,
            timestamp: body["CreateTime"].as_i64().unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(
        &self,
        message: &UnifiedMessage,
        reply: &ReplyMessage,
    ) -> Result<WebhookResponse, String> {
        let reply_content = match &reply.content {
            MessageContent::Text(text) => text.clone(),
            _ => "Unsupported message type".to_string(),
        };

        let response_body = serde_json::json!({
            "ToUserName": message.sender_id,
            "FromUserName": message.receiver_id.as_deref().unwrap_or(""),
            "CreateTime": chrono::Utc::now().timestamp(),
            "MsgType": "text",
            "Content": reply_content
        });

        Ok(WebhookResponse {
            status_code: 200,
            body: response_body,
            should_reply: true,
            reply: None,
        })
    }

    fn verify_signature(&self, _headers: &HashMap<String, String>, _body: &[u8]) -> bool {
        // 简化实现，生产环境需要完整的签名验证
        true
    }

    fn platform_type(&self) -> ImPlatform {
        ImPlatform::Wechat
    }
}

/// Telegram适配器
pub struct TelegramAdapter {
    bot_token: String,
}

impl TelegramAdapter {
    pub fn new(bot_token: String) -> Self {
        Self { bot_token }
    }
}

impl PlatformAdapter for TelegramAdapter {
    fn parse_webhook(&self, request: &WebhookRequest) -> Result<UnifiedMessage, String> {
        let body = &request.body;

        let message = &body["message"];
        let chat = &message["chat"];
        let from = &message["from"];

        let content = if let Some(text) = message["text"].as_str() {
            MessageContent::Text(text.to_string())
        } else if let Some(photo) = message["photo"].as_array() {
            MessageContent::Image {
                url: photo
                    .last()
                    .and_then(|p| p["file_id"].as_str())
                    .unwrap_or("")
                    .to_string(),
                caption: message["caption"].as_str().map(|s| s.to_string()),
            }
        } else {
            MessageContent::Text("Unsupported message type".to_string())
        };

        let message_type = if chat["type"].as_str() == Some("group")
            || chat["type"].as_str() == Some("supergroup")
        {
            MessageType::Group
        } else {
            MessageType::Private
        };

        Ok(UnifiedMessage {
            id: message["message_id"].as_i64().unwrap_or(0).to_string(),
            platform: ImPlatform::Telegram,
            sender_id: from["id"].as_i64().unwrap_or(0).to_string(),
            sender_name: from["first_name"].as_str().map(|s| s.to_string()),
            receiver_id: chat["id"].as_i64().map(|i| i.to_string()),
            content,
            message_type,
            timestamp: message["date"].as_i64().unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(
        &self,
        message: &UnifiedMessage,
        reply: &ReplyMessage,
    ) -> Result<WebhookResponse, String> {
        let reply_content = match &reply.content {
            MessageContent::Text(text) => text.clone(),
            _ => "Unsupported message type".to_string(),
        };

        let response_body = serde_json::json!({
            "method": "sendMessage",
            "chat_id": message.receiver_id.as_deref().unwrap_or(&message.sender_id),
            "text": reply_content,
            "reply_to_message_id": reply.reply_to
        });

        Ok(WebhookResponse {
            status_code: 200,
            body: response_body,
            should_reply: true,
            reply: None,
        })
    }

    fn verify_signature(&self, _headers: &HashMap<String, String>, _body: &[u8]) -> bool {
        // Telegram使用secret_path验证，这里简化处理
        true
    }

    fn platform_type(&self) -> ImPlatform {
        ImPlatform::Telegram
    }
}

/// Slack适配器
pub struct SlackAdapter {
    verification_token: String,
    signing_secret: Option<String>,
}

impl SlackAdapter {
    pub fn new(verification_token: String, signing_secret: Option<String>) -> Self {
        Self {
            verification_token,
            signing_secret,
        }
    }
}

impl PlatformAdapter for SlackAdapter {
    fn parse_webhook(&self, request: &WebhookRequest) -> Result<UnifiedMessage, String> {
        let body = &request.body;

        // 处理URL验证挑战
        if body["type"].as_str() == Some("url_verification") {
            return Err("url_verification".to_string());
        }

        let event = &body["event"];
        let event_type = event["type"].as_str().unwrap_or("unknown");

        let content = match event_type {
            "message" => MessageContent::Text(event["text"].as_str().unwrap_or("").to_string()),
            "file_shared" => MessageContent::File {
                url: event["file"]["url_private"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                filename: event["file"]["name"].as_str().unwrap_or("").to_string(),
                mime_type: event["file"]["mimetype"].as_str().map(|s| s.to_string()),
            },
            _ => MessageContent::Text(event["text"].as_str().unwrap_or("").to_string()),
        };

        let message_type = if event["channel_type"].as_str() == Some("group") {
            MessageType::Group
        } else {
            MessageType::Private
        };

        Ok(UnifiedMessage {
            id: event["ts"].as_str().unwrap_or("unknown").to_string(),
            platform: ImPlatform::Slack,
            sender_id: event["user"].as_str().unwrap_or("unknown").to_string(),
            sender_name: None,
            receiver_id: event["channel"].as_str().map(|s| s.to_string()),
            content,
            message_type,
            timestamp: event["ts"].as_str().unwrap_or("0").parse().unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(
        &self,
        message: &UnifiedMessage,
        reply: &ReplyMessage,
    ) -> Result<WebhookResponse, String> {
        let reply_content = match &reply.content {
            MessageContent::Text(text) => text.clone(),
            _ => "Unsupported message type".to_string(),
        };

        let response_body = serde_json::json!({
            "channel": message.receiver_id.as_deref().unwrap_or(&message.sender_id),
            "text": reply_content,
            "thread_ts": reply.reply_to
        });

        Ok(WebhookResponse {
            status_code: 200,
            body: response_body,
            should_reply: true,
            reply: None,
        })
    }

    fn verify_signature(&self, _headers: &HashMap<String, String>, _body: &[u8]) -> bool {
        // 简化实现，生产环境需要完整的签名验证
        true
    }

    fn platform_type(&self) -> ImPlatform {
        ImPlatform::Slack
    }
}

/// 飞书适配器
pub struct FeishuAdapter {
    verification_token: String,
    encrypt_key: Option<String>,
}

impl FeishuAdapter {
    pub fn new(verification_token: String, encrypt_key: Option<String>) -> Self {
        Self {
            verification_token,
            encrypt_key,
        }
    }
}

impl PlatformAdapter for FeishuAdapter {
    fn parse_webhook(&self, request: &WebhookRequest) -> Result<UnifiedMessage, String> {
        let body = &request.body;

        // 处理URL验证挑战
        if body["type"].as_str() == Some("url_verification") {
            return Err("url_verification".to_string());
        }

        let event = &body["event"];
        let message = &event["message"];

        let content = match message["message_type"].as_str() {
            Some("text") => {
                let text_content: serde_json::Value =
                    serde_json::from_str(message["content"].as_str().unwrap_or("{}"))
                        .unwrap_or(serde_json::Value::Null);
                MessageContent::Text(text_content["text"].as_str().unwrap_or("").to_string())
            }
            Some("image") => MessageContent::Image {
                url: message["content"].as_str().unwrap_or("").to_string(),
                caption: None,
            },
            _ => MessageContent::Text(message["content"].as_str().unwrap_or("").to_string()),
        };

        let message_type = if message["chat_type"].as_str() == Some("group") {
            MessageType::Group
        } else {
            MessageType::Private
        };

        Ok(UnifiedMessage {
            id: message["message_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            platform: ImPlatform::Feishu,
            sender_id: event["sender"]["sender_id"]["open_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            sender_name: event["sender"]["sender_id"]["name"]
                .as_str()
                .map(|s| s.to_string()),
            receiver_id: message["chat_id"].as_str().map(|s| s.to_string()),
            content,
            message_type,
            timestamp: message["create_time"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(
        &self,
        _message: &UnifiedMessage,
        reply: &ReplyMessage,
    ) -> Result<WebhookResponse, String> {
        let reply_content = match &reply.content {
            MessageContent::Text(text) => serde_json::json!({"text": text}),
            _ => serde_json::json!({"text": "Unsupported message type"}),
        };

        let response_body = serde_json::json!({
            "msg_type": "text",
            "content": reply_content
        });

        Ok(WebhookResponse {
            status_code: 200,
            body: response_body,
            should_reply: true,
            reply: None,
        })
    }

    fn verify_signature(&self, _headers: &HashMap<String, String>, _body: &[u8]) -> bool {
        // 简化实现，生产环境需要完整的签名验证
        true
    }

    fn platform_type(&self) -> ImPlatform {
        ImPlatform::Feishu
    }
}

/// 适配器工厂
pub struct AdapterFactory;

impl AdapterFactory {
    /// 创建平台适配器
    pub fn create(
        platform: &ImPlatform,
        config: &HashMap<String, String>,
    ) -> Box<dyn PlatformAdapter> {
        match platform {
            ImPlatform::Wechat => {
                let token = config.get("token").cloned().unwrap_or_default();
                let encoding_aes_key = config.get("encoding_aes_key").cloned();
                Box::new(WechatAdapter::new(token, encoding_aes_key))
            }
            ImPlatform::Telegram => {
                let bot_token = config.get("bot_token").cloned().unwrap_or_default();
                Box::new(TelegramAdapter::new(bot_token))
            }
            ImPlatform::Slack => {
                let verification_token = config
                    .get("verification_token")
                    .cloned()
                    .unwrap_or_default();
                let signing_secret = config.get("signing_secret").cloned();
                Box::new(SlackAdapter::new(verification_token, signing_secret))
            }
            ImPlatform::Feishu => {
                let verification_token = config
                    .get("verification_token")
                    .cloned()
                    .unwrap_or_default();
                let encrypt_key = config.get("encrypt_key").cloned();
                Box::new(FeishuAdapter::new(verification_token, encrypt_key))
            }
            _ => Box::new(WechatAdapter::new("default".to_string(), None)),
        }
    }
}

/// IM集成管理器
pub struct ImIntegrationManager {
    adapters: HashMap<ImPlatform, Box<dyn PlatformAdapter>>,
}

impl Default for ImIntegrationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImIntegrationManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// 注册平台适配器
    pub fn register_adapter(&mut self, platform: ImPlatform, adapter: Box<dyn PlatformAdapter>) {
        self.adapters.insert(platform, adapter);
    }

    /// 处理Webhook请求
    pub fn handle_webhook(&self, request: &WebhookRequest) -> Result<WebhookResponse, String> {
        let adapter = self
            .adapters
            .get(&request.platform)
            .ok_or_else(|| format!("No adapter registered for platform: {:?}", request.platform))?;

        let message = adapter.parse_webhook(request)?;

        // 这里可以添加消息处理逻辑
        // 例如：调用KIAS NL接口处理消息

        let reply = ReplyMessage {
            content: MessageContent::Text(format!("Received: {:?}", message.content)),
            reply_to: None,
            silent: false,
        };

        adapter.build_reply(&message, &reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wechat_text_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "12345",
                "MsgType": "text",
                "Content": "Hello AgentGuard",
                "FromUserName": "user_001",
                "ToUserName": "kias_bot",
                "CreateTime": 1700000000
            }),
            query_params: HashMap::new(),
        }
    }

    fn wechat_image_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "12346",
                "MsgType": "image",
                "PicUrl": "https://example.com/img.jpg",
                "FromUserName": "user_001",
                "ToUserName": "kias_bot",
                "CreateTime": 1700000001
            }),
            query_params: HashMap::new(),
        }
    }

    fn telegram_text_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 99,
                    "text": "Hello from TG",
                    "from": {"id": 42, "first_name": "Alice"},
                    "chat": {"id": 42, "type": "private"},
                    "date": 1700000000
                }
            }),
            query_params: HashMap::new(),
        }
    }

    fn telegram_group_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 100,
                    "text": "Group msg",
                    "from": {"id": 43, "first_name": "Bob"},
                    "chat": {"id": -100123, "type": "supergroup"},
                    "date": 1700000002
                }
            }),
            query_params: HashMap::new(),
        }
    }

    fn slack_text_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "Hello Slack",
                    "user": "U123",
                    "channel": "C456",
                    "ts": "1700000000.000001",
                    "channel_type": "im"
                }
            }),
            query_params: HashMap::new(),
        }
    }

    // ===== WechatAdapter tests =====

    #[test]
    fn test_wechat_adapter_platform_type() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    #[test]
    fn test_wechat_parse_text_message() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        assert_eq!(msg.id, "12345");
        assert_eq!(msg.platform, ImPlatform::Wechat);
        assert_eq!(msg.sender_id, "user_001");
        assert_eq!(msg.receiver_id, Some("kias_bot".to_string()));
        assert_eq!(msg.timestamp, 1700000000);
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Hello AgentGuard"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_wechat_parse_image_message() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_image_request()).unwrap();
        match &msg.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "https://example.com/img.jpg");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_wechat_build_reply() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("Reply text".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
        assert_eq!(resp.body["Content"], "Reply text");
        assert_eq!(resp.body["ToUserName"], "user_001");
    }

    #[test]
    fn test_wechat_verify_signature_always_true() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        assert!(adapter.verify_signature(&HashMap::new(), b"body"));
    }

    // ===== TelegramAdapter tests =====

    #[test]
    fn test_telegram_platform_type() {
        let adapter = TelegramAdapter::new("bot_token".to_string());
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    #[test]
    fn test_telegram_parse_private_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_text_request()).unwrap();
        assert_eq!(msg.id, "99");
        assert_eq!(msg.platform, ImPlatform::Telegram);
        assert_eq!(msg.sender_id, "42");
        assert_eq!(msg.sender_name, Some("Alice".to_string()));
        assert_eq!(msg.message_type, MessageType::Private);
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Hello from TG"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_telegram_parse_group_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_group_request()).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
        assert_eq!(msg.receiver_id, Some("-100123".to_string()));
    }

    #[test]
    fn test_telegram_build_reply() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("TG reply".to_string()),
            reply_to: Some("99".to_string()),
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["text"], "TG reply");
        assert_eq!(resp.body["reply_to_message_id"], "99");
    }

    // ===== SlackAdapter tests =====

    #[test]
    fn test_slack_platform_type() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        assert_eq!(adapter.platform_type(), ImPlatform::Slack);
    }

    #[test]
    fn test_slack_parse_text_message() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        assert_eq!(msg.id, "1700000000.000001");
        assert_eq!(msg.platform, ImPlatform::Slack);
        assert_eq!(msg.sender_id, "U123");
        assert_eq!(msg.receiver_id, Some("C456".to_string()));
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Hello Slack"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_slack_url_verification_challenge() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({"type": "url_verification", "challenge": "abc"}),
            query_params: HashMap::new(),
        };
        let result = adapter.parse_webhook(&request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "url_verification");
    }

    #[test]
    fn test_slack_build_reply() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("Slack reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    // ===== FeishuAdapter tests =====

    #[test]
    fn test_feishu_platform_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        assert_eq!(adapter.platform_type(), ImPlatform::Feishu);
    }

    // ===== AdapterFactory tests =====

    #[test]
    fn test_factory_creates_all_platforms() {
        let config = HashMap::new();
        for platform in &[
            ImPlatform::Wechat,
            ImPlatform::Telegram,
            ImPlatform::Slack,
            ImPlatform::Feishu,
        ] {
            let adapter = AdapterFactory::create(platform, &config);
            assert_eq!(adapter.platform_type(), *platform);
        }
    }

    #[test]
    fn test_factory_custom_falls_back_to_wechat() {
        let config = HashMap::new();
        let adapter = AdapterFactory::create(&ImPlatform::Custom, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    #[test]
    fn test_factory_passes_config_to_wechat() {
        let mut config = HashMap::new();
        config.insert("token".to_string(), "my_token".to_string());
        config.insert("encoding_aes_key".to_string(), "my_key".to_string());
        let adapter = AdapterFactory::create(&ImPlatform::Wechat, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    // ===== ImIntegrationManager tests =====

    #[test]
    fn test_manager_register_adapter() {
        let mut manager = ImIntegrationManager::new();
        let adapter = Box::new(WechatAdapter::new("t".to_string(), None));
        manager.register_adapter(ImPlatform::Wechat, adapter);
        assert!(manager.adapters.contains_key(&ImPlatform::Wechat));
    }

    #[test]
    fn test_manager_default_trait() {
        let manager = ImIntegrationManager::default();
        assert!(manager.adapters.is_empty());
    }

    #[test]
    fn test_manager_handle_webhook_no_adapter() {
        let manager = ImIntegrationManager::new();
        let request = wechat_text_request();
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No adapter registered"));
    }

    #[test]
    fn test_manager_handle_webhook_success() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        let request = wechat_text_request();
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
    }

    #[test]
    fn test_manager_multiple_platforms() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        manager.register_adapter(
            ImPlatform::Telegram,
            Box::new(TelegramAdapter::new("t".to_string())),
        );
        manager.register_adapter(
            ImPlatform::Slack,
            Box::new(SlackAdapter::new("t".to_string(), None)),
        );
        assert_eq!(manager.adapters.len(), 3);

        // Handle Wechat
        let resp = manager.handle_webhook(&wechat_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);

        // Handle Telegram
        let resp = manager.handle_webhook(&telegram_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);

        // Handle Slack
        let resp = manager.handle_webhook(&slack_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    // ===== Serialization tests =====

    #[test]
    fn test_unified_message_serialization() {
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "u1".to_string(),
            sender_name: Some("Alice".to_string()),
            receiver_id: None,
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: UnifiedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "1");
        assert_eq!(deserialized.platform, ImPlatform::Wechat);
    }

    #[test]
    fn test_message_content_variants_serialization() {
        let variants = vec![
            MessageContent::Text("hello".to_string()),
            MessageContent::Image {
                url: "http://img.jpg".to_string(),
                caption: Some("cap".to_string()),
            },
            MessageContent::File {
                url: "http://f.txt".to_string(),
                filename: "f.txt".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
            MessageContent::Location {
                latitude: 39.9,
                longitude: 116.4,
                address: Some("Beijing".to_string()),
            },
            MessageContent::Event(EventType::Subscribe),
            MessageContent::Event(EventType::Custom("test".to_string())),
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: MessageContent = serde_json::from_str(&json).unwrap();
            // Just verify round-trip works
            let json2 = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_im_platform_hash() {
        let mut map = HashMap::new();
        map.insert(ImPlatform::Wechat, 1);
        map.insert(ImPlatform::Telegram, 2);
        map.insert(ImPlatform::Slack, 3);
        map.insert(ImPlatform::Feishu, 4);
        map.insert(ImPlatform::Custom, 5);
        assert_eq!(map.len(), 5);
    }

    // ===== Edge cases =====

    #[test]
    fn test_wechat_parse_missing_fields() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "unknown");
        assert_eq!(msg.sender_id, "unknown");
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, ""),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_telegram_parse_photo_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 200,
                    "photo": [
                        {"file_id": "small_001"},
                        {"file_id": "large_002"}
                    ],
                    "caption": "A photo",
                    "from": {"id": 50, "first_name": "Charlie"},
                    "chat": {"id": 50, "type": "private"},
                    "date": 1700000100
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "large_002"); // last photo
                assert_eq!(caption.as_deref(), Some("A photo"));
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_slack_parse_file_shared() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "file_shared",
                    "file": {
                        "url_private": "https://slack.com/file/123",
                        "name": "doc.pdf",
                        "mimetype": "application/pdf"
                    },
                    "user": "U789",
                    "channel": "C012",
                    "ts": "1700000200.000001",
                    "channel_type": "group"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
        match &msg.content {
            MessageContent::File {
                url,
                filename,
                mime_type,
            } => {
                assert_eq!(url, "https://slack.com/file/123");
                assert_eq!(filename, "doc.pdf");
                assert_eq!(mime_type.as_deref(), Some("application/pdf"));
            }
            _ => panic!("Expected File content"),
        }
    }

    // ===== FeishuAdapter tests (expanded) =====

    fn feishu_text_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_001",
                        "message_type": "text",
                        "content": "{\"text\":\"Hello Feishu\"}",
                        "chat_id": "oc_abc",
                        "chat_type": "p2p",
                        "create_time": "1700000000"
                    },
                    "sender": {
                        "sender_id": {
                            "open_id": "ou_xyz",
                            "name": "TestUser"
                        }
                    }
                }
            }),
            query_params: HashMap::new(),
        }
    }

    fn feishu_group_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_002",
                        "message_type": "text",
                        "content": "{\"text\":\"Group hello\"}",
                        "chat_id": "oc_group",
                        "chat_type": "group",
                        "create_time": "1700000001"
                    },
                    "sender": {
                        "sender_id": {
                            "open_id": "ou_abc"
                        }
                    }
                }
            }),
            query_params: HashMap::new(),
        }
    }

    fn feishu_image_request() -> WebhookRequest {
        WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_003",
                        "message_type": "image",
                        "content": "img_v2_key_123",
                        "chat_id": "oc_img",
                        "chat_type": "p2p",
                        "create_time": "1700000002"
                    },
                    "sender": {
                        "sender_id": {
                            "open_id": "ou_img"
                        }
                    }
                }
            }),
            query_params: HashMap::new(),
        }
    }

    #[test]
    fn test_feishu_parse_text_message() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_text_request()).unwrap();
        assert_eq!(msg.id, "msg_001");
        assert_eq!(msg.platform, ImPlatform::Feishu);
        assert_eq!(msg.sender_id, "ou_xyz");
        assert_eq!(msg.sender_name, Some("TestUser".to_string()));
        assert_eq!(msg.receiver_id, Some("oc_abc".to_string()));
        assert_eq!(msg.message_type, MessageType::Private);
        assert_eq!(msg.timestamp, 1700000000);
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Hello Feishu"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_feishu_parse_group_message() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_group_request()).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
        assert_eq!(msg.receiver_id, Some("oc_group".to_string()));
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Group hello"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_feishu_parse_image_message() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_image_request()).unwrap();
        match &msg.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "img_v2_key_123");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_feishu_url_verification_challenge() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({"type": "url_verification", "challenge": "xyz"}),
            query_params: HashMap::new(),
        };
        let result = adapter.parse_webhook(&request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "url_verification");
    }

    #[test]
    fn test_feishu_build_reply_text() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("Feishu reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
        assert_eq!(resp.body["msg_type"], "text");
        assert_eq!(resp.body["content"]["text"], "Feishu reply");
    }

    #[test]
    fn test_feishu_build_reply_non_text() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Image {
                url: "http://img.jpg".to_string(),
                caption: None,
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["content"]["text"], "Unsupported message type");
    }

    #[test]
    fn test_feishu_verify_signature_always_true() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        assert!(adapter.verify_signature(&HashMap::new(), b"body"));
    }

    #[test]
    fn test_feishu_parse_missing_fields() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({"event": {}}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "unknown");
        assert_eq!(msg.sender_id, "unknown");
        assert_eq!(msg.timestamp, 0);
        assert_eq!(msg.message_type, MessageType::Private);
    }

    #[test]
    fn test_feishu_parse_unsupported_message_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_x",
                        "message_type": "video",
                        "content": "some_video_data",
                        "chat_id": "oc_x",
                        "chat_type": "p2p",
                        "create_time": "0"
                    },
                    "sender": {"sender_id": {"open_id": "ou_x"}}
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "some_video_data"),
            _ => panic!("Expected fallback Text content"),
        }
    }

    // ===== Non-text reply content tests =====

    #[test]
    fn test_wechat_build_reply_non_text() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::File {
                url: "http://file.txt".to_string(),
                filename: "f.txt".to_string(),
                mime_type: None,
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["Content"], "Unsupported message type");
    }

    #[test]
    fn test_telegram_build_reply_non_text() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Image {
                url: "http://img.jpg".to_string(),
                caption: None,
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["text"], "Unsupported message type");
    }

    #[test]
    fn test_slack_build_reply_non_text() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Location {
                latitude: 39.9,
                longitude: 116.4,
                address: None,
            },
            reply_to: Some("thread_123".to_string()),
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["text"], "Unsupported message type");
        assert_eq!(resp.body["thread_ts"], "thread_123");
    }

    // ===== Verify signature tests =====

    #[test]
    fn test_telegram_verify_signature() {
        let adapter = TelegramAdapter::new("token".to_string());
        assert!(adapter.verify_signature(&HashMap::new(), b"body"));
    }

    #[test]
    fn test_slack_verify_signature() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        assert!(adapter.verify_signature(&HashMap::new(), b"body"));
    }

    // ===== Slack edge cases =====

    #[test]
    fn test_slack_parse_missing_fields() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({"event": {}}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "unknown");
        assert_eq!(msg.sender_id, "unknown");
        assert_eq!(msg.timestamp, 0);
    }

    #[test]
    fn test_slack_parse_unknown_event_type() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "reaction_added",
                    "user": "U999",
                    "channel": "C999",
                    "ts": "1700000999",
                    "channel_type": "im"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.sender_id, "U999");
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, ""),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_slack_group_channel_type() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "group msg",
                    "user": "U001",
                    "channel": "C_grp",
                    "ts": "1700000300",
                    "channel_type": "group"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
    }

    // ===== Telegram edge cases =====

    #[test]
    fn test_telegram_parse_empty_photo_array() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 300,
                    "photo": [],
                    "from": {"id": 60, "first_name": "Dave"},
                    "chat": {"id": 60, "type": "private"},
                    "date": 1700000200
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Image { url, .. } => assert_eq!(url, ""),
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_telegram_parse_missing_message_id() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "text": "no id",
                    "from": {"id": 70},
                    "chat": {"id": 70, "type": "private"},
                    "date": 1700000300
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "0");
        assert_eq!(msg.sender_name, None);
    }

    // ===== ImIntegrationManager edge cases =====

    #[test]
    fn test_manager_handle_feishu_no_adapter() {
        let manager = ImIntegrationManager::new();
        let request = feishu_text_request();
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Feishu"));
    }

    #[test]
    fn test_manager_handle_slack_no_adapter() {
        let manager = ImIntegrationManager::new();
        let request = slack_text_request();
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Slack"));
    }

    #[test]
    fn test_manager_handle_telegram_success() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Telegram,
            Box::new(TelegramAdapter::new("t".to_string())),
        );
        let request = telegram_text_request();
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[test]
    fn test_manager_handle_feishu_success() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Feishu,
            Box::new(FeishuAdapter::new("t".to_string(), None)),
        );
        let request = feishu_text_request();
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    #[test]
    fn test_manager_register_overwrites() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("first".to_string(), None)),
        );
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("second".to_string(), None)),
        );
        assert_eq!(manager.adapters.len(), 1);
    }

    // ===== AdapterFactory with config =====

    #[test]
    fn test_factory_telegram_with_config() {
        let mut config = HashMap::new();
        config.insert("bot_token".to_string(), "bot123:abc".to_string());
        let adapter = AdapterFactory::create(&ImPlatform::Telegram, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    #[test]
    fn test_factory_slack_with_config() {
        let mut config = HashMap::new();
        config.insert("verification_token".to_string(), "xoxb".to_string());
        config.insert("signing_secret".to_string(), "secret".to_string());
        let adapter = AdapterFactory::create(&ImPlatform::Slack, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Slack);
    }

    #[test]
    fn test_factory_feishu_with_config() {
        let mut config = HashMap::new();
        config.insert("verification_token".to_string(), "vt".to_string());
        config.insert("encrypt_key".to_string(), "ek".to_string());
        let adapter = AdapterFactory::create(&ImPlatform::Feishu, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Feishu);
    }

    // ===== ImPlatform serialization =====

    #[test]
    fn test_im_platform_serde_roundtrip() {
        let platforms = vec![
            ImPlatform::Wechat,
            ImPlatform::Telegram,
            ImPlatform::Slack,
            ImPlatform::Feishu,
            ImPlatform::Custom,
        ];
        for p in platforms {
            let json = serde_json::to_string(&p).unwrap();
            let back: ImPlatform = serde_json::from_str(&json).unwrap();
            assert_eq!(p, back);
        }
    }

    // ===== MessageContent / EventType serialization =====

    #[test]
    fn test_event_type_serde_roundtrip() {
        let events = vec![
            EventType::Subscribe,
            EventType::Unsubscribe,
            EventType::JoinGroup,
            EventType::LeaveGroup,
            EventType::Custom("custom".to_string()),
        ];
        for e in events {
            let json = serde_json::to_string(&e).unwrap();
            let back: EventType = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2);
        }
    }

    // ===== ReplyMessage / WebhookRequest / WebhookResponse serialization =====

    #[test]
    fn test_reply_message_serde() {
        let reply = ReplyMessage {
            content: MessageContent::Text("test".to_string()),
            reply_to: Some("msg_1".to_string()),
            silent: true,
        };
        let json = serde_json::to_string(&reply).unwrap();
        let back: ReplyMessage = serde_json::from_str(&json).unwrap();
        assert!(back.silent);
        assert_eq!(back.reply_to, Some("msg_1".to_string()));
    }

    #[test]
    fn test_webhook_request_serde() {
        let req = wechat_text_request();
        let json = serde_json::to_string(&req).unwrap();
        let back: WebhookRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.platform, ImPlatform::Wechat);
    }

    #[test]
    fn test_webhook_response_serde() {
        let resp = WebhookResponse {
            status_code: 200,
            body: serde_json::json!({"ok": true}),
            should_reply: true,
            reply: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WebhookResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status_code, 200);
        assert!(back.should_reply);
        assert!(back.reply.is_none());
    }

    // ===== NEW TESTS: Edge cases, error paths, boundary conditions =====

    #[test]
    fn test_wechat_parse_unknown_msg_type_fallback() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "99999",
                "MsgType": "video",
                "Content": "video_content",
                "FromUserName": "user_002",
                "CreateTime": 1700000050
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // Unknown MsgType falls through to the _ => Text(Content) branch
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "video_content"),
            _ => panic!("Expected Text content for unknown MsgType fallback"),
        }
    }

    #[test]
    fn test_wechat_parse_missing_to_user_name() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "55555",
                "MsgType": "text",
                "Content": "hi",
                "FromUserName": "user_003",
                "CreateTime": 1700000060
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // Missing ToUserName means receiver_id should be None
        assert!(msg.receiver_id.is_none());
    }

    #[test]
    fn test_wechat_build_reply_with_no_receiver() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "u1".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        // With receiver_id None, FromUserName should be empty string
        assert_eq!(resp.body["FromUserName"], "");
    }

    #[test]
    fn test_telegram_parse_group_type_not_supergroup() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 501,
                    "text": "plain group",
                    "from": {"id": 80, "first_name": "Eve"},
                    "chat": {"id": -200, "type": "group"},
                    "date": 1700000400
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // "group" type (not just "supergroup") should also yield Group
        assert_eq!(msg.message_type, MessageType::Group);
    }

    #[test]
    fn test_telegram_parse_unsupported_content_fallback() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 502,
                    "from": {"id": 81},
                    "chat": {"id": 81, "type": "private"},
                    "date": 1700000401
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected fallback Text content"),
        }
    }

    #[test]
    fn test_telegram_parse_photo_without_caption() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 503,
                    "photo": [{"file_id": "small"}],
                    "from": {"id": 82, "first_name": "Frank"},
                    "chat": {"id": 82, "type": "private"},
                    "date": 1700000402
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "small");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_telegram_build_reply_with_no_receiver() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Telegram,
            sender_id: "42".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        // With receiver_id None, chat_id should fall back to sender_id
        assert_eq!(resp.body["chat_id"], "42");
        assert!(resp.body["reply_to_message_id"].is_null());
    }

    #[test]
    fn test_slack_build_reply_with_none_reply_to() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("Slack reply no thread".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.body["thread_ts"].is_null());
    }

    #[test]
    fn test_slack_parse_file_shared_missing_file_fields() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "file_shared",
                    "file": {},
                    "user": "U_file",
                    "channel": "C_file",
                    "ts": "1700000500",
                    "channel_type": "im"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::File {
                url,
                filename,
                mime_type,
            } => {
                assert_eq!(url, "");
                assert_eq!(filename, "");
                assert!(mime_type.is_none());
            }
            _ => panic!("Expected File content"),
        }
    }

    #[test]
    fn test_feishu_parse_text_with_invalid_json_content() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_invalid",
                        "message_type": "text",
                        "content": "not valid json",
                        "chat_id": "oc_inv",
                        "chat_type": "p2p",
                        "create_time": "1700000600"
                    },
                    "sender": {"sender_id": {"open_id": "ou_inv"}}
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // Invalid JSON for content should fall back to empty text
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, ""),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_feishu_parse_empty_event_body() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // Missing event entirely — all fields should default
        assert_eq!(msg.id, "unknown");
        assert_eq!(msg.sender_id, "unknown");
        assert!(msg.sender_name.is_none());
        assert!(msg.receiver_id.is_none());
    }

    #[test]
    fn test_manager_handle_slack_url_verification_propagates() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Slack,
            Box::new(SlackAdapter::new("t".to_string(), None)),
        );
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({"type": "url_verification", "challenge": "abc"}),
            query_params: HashMap::new(),
        };
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "url_verification");
    }

    #[test]
    fn test_manager_handle_feishu_url_verification_propagates() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Feishu,
            Box::new(FeishuAdapter::new("t".to_string(), None)),
        );
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({"type": "url_verification", "challenge": "xyz"}),
            query_params: HashMap::new(),
        };
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "url_verification");
    }

    #[test]
    fn test_manager_handle_custom_platform_no_adapter() {
        let manager = ImIntegrationManager::new();
        let request = WebhookRequest {
            platform: ImPlatform::Custom,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Custom"));
    }

    #[test]
    fn test_webhook_response_with_reply_some_serde() {
        let resp = WebhookResponse {
            status_code: 201,
            body: serde_json::json!({"msg": "created"}),
            should_reply: false,
            reply: Some(ReplyMessage {
                content: MessageContent::Text("follow-up".to_string()),
                reply_to: Some("orig_123".to_string()),
                silent: true,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WebhookResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status_code, 201);
        assert!(!back.should_reply);
        assert!(back.reply.is_some());
        let reply = back.reply.unwrap();
        assert!(reply.silent);
        assert_eq!(reply.reply_to, Some("orig_123".to_string()));
    }

    #[test]
    fn test_reply_message_with_none_reply_to_serde() {
        let reply = ReplyMessage {
            content: MessageContent::Image {
                url: "http://img.jpg".to_string(),
                caption: Some("cap".to_string()),
            },
            reply_to: None,
            silent: false,
        };
        let json = serde_json::to_string(&reply).unwrap();
        let back: ReplyMessage = serde_json::from_str(&json).unwrap();
        assert!(back.reply_to.is_none());
        assert!(!back.silent);
        match back.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "http://img.jpg");
                assert_eq!(caption, Some("cap".to_string()));
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_factory_with_empty_config_telegram() {
        let config = HashMap::new();
        let adapter = AdapterFactory::create(&ImPlatform::Telegram, &config);
        // Should use empty default for bot_token
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    #[test]
    fn test_factory_with_empty_config_slack() {
        let config = HashMap::new();
        let adapter = AdapterFactory::create(&ImPlatform::Slack, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Slack);
    }

    #[test]
    fn test_factory_with_empty_config_feishu() {
        let config = HashMap::new();
        let adapter = AdapterFactory::create(&ImPlatform::Feishu, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Feishu);
    }

    // ===== Adapter constructors with all config =====

    #[test]
    fn test_wechat_adapter_with_encoding_aes_key() {
        let adapter = WechatAdapter::new("token".to_string(), Some("aes_key_123".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    #[test]
    fn test_slack_adapter_with_signing_secret() {
        let adapter =
            SlackAdapter::new("token".to_string(), Some("signing_secret_abc".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Slack);
    }

    #[test]
    fn test_feishu_adapter_with_encrypt_key() {
        let adapter = FeishuAdapter::new("token".to_string(), Some("encrypt_key_xyz".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Feishu);
    }

    // ===== MessageType Display =====

    #[test]
    fn test_message_type_display() {
        assert_eq!(format!("{:?}", MessageType::Private), "Private");
        assert_eq!(format!("{:?}", MessageType::Group), "Group");
        assert_eq!(format!("{:?}", MessageType::Channel), "Channel");
        assert_eq!(format!("{:?}", MessageType::System), "System");
    }

    // ===== MessageContent Location serialization =====

    #[test]
    fn test_location_content_serde() {
        let loc = MessageContent::Location {
            latitude: 31.2304,
            longitude: 121.4737,
            address: Some("Shanghai".to_string()),
        };
        let json = serde_json::to_string(&loc).unwrap();
        let back: MessageContent = serde_json::from_str(&json).unwrap();
        match back {
            MessageContent::Location {
                latitude,
                longitude,
                address,
            } => {
                assert!((latitude - 31.2304).abs() < 0.001);
                assert!((longitude - 121.4737).abs() < 0.001);
                assert_eq!(address.as_deref(), Some("Shanghai"));
            }
            _ => panic!("Expected Location"),
        }
    }

    // ===== EventType variants =====

    #[test]
    fn test_event_type_display() {
        assert_eq!(format!("{:?}", EventType::Subscribe), "Subscribe");
        assert_eq!(format!("{:?}", EventType::Unsubscribe), "Unsubscribe");
        assert_eq!(format!("{:?}", EventType::JoinGroup), "JoinGroup");
        assert_eq!(format!("{:?}", EventType::LeaveGroup), "LeaveGroup");
        assert_eq!(
            format!("{:?}", EventType::Custom("test".to_string())),
            "Custom(\"test\")"
        );
    }

    // ===== ImPlatform Display =====

    #[test]
    fn test_im_platform_display() {
        assert_eq!(format!("{:?}", ImPlatform::Wechat), "Wechat");
        assert_eq!(format!("{:?}", ImPlatform::Telegram), "Telegram");
        assert_eq!(format!("{:?}", ImPlatform::Slack), "Slack");
        assert_eq!(format!("{:?}", ImPlatform::Feishu), "Feishu");
        assert_eq!(format!("{:?}", ImPlatform::Custom), "Custom");
    }

    // ===== UnifiedMessage with all content types =====

    #[test]
    fn test_unified_message_with_event_content() {
        let msg = UnifiedMessage {
            id: "evt1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "user1".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Event(EventType::Subscribe),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: UnifiedMessage = serde_json::from_str(&json).unwrap();
        match &back.content {
            MessageContent::Event(EventType::Subscribe) => {}
            _ => panic!("Expected Event(Subscribe)"),
        }
    }

    // ===== WebhookResponse with reply Some =====

    #[test]
    fn test_webhook_response_with_reply() {
        let resp = WebhookResponse {
            status_code: 200,
            body: serde_json::json!({"ok": true}),
            should_reply: true,
            reply: Some(ReplyMessage {
                content: MessageContent::Text("hello".to_string()),
                reply_to: Some("msg_1".to_string()),
                silent: false,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WebhookResponse = serde_json::from_str(&json).unwrap();
        assert!(back.reply.is_some());
        assert!(!back.reply.as_ref().unwrap().silent);
    }

    // ===== Manager with all 4 platforms =====

    #[test]
    fn test_manager_all_four_platforms() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        manager.register_adapter(
            ImPlatform::Telegram,
            Box::new(TelegramAdapter::new("t".to_string())),
        );
        manager.register_adapter(
            ImPlatform::Slack,
            Box::new(SlackAdapter::new("t".to_string(), None)),
        );
        manager.register_adapter(
            ImPlatform::Feishu,
            Box::new(FeishuAdapter::new("t".to_string(), None)),
        );
        assert_eq!(manager.adapters.len(), 4);
    }

    // ===== Empty string edge cases =====

    #[test]
    fn test_wechat_parse_empty_body() {
        let adapter = WechatAdapter::new("t".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "unknown");
        assert_eq!(msg.sender_id, "unknown");
    }

    #[test]
    fn test_telegram_parse_empty_body() {
        let adapter = TelegramAdapter::new("t".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        // Should not panic
        let _ = adapter.parse_webhook(&request);
    }

    #[test]
    fn test_feishu_parse_empty_body() {
        let adapter = FeishuAdapter::new("t".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({}),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.id, "unknown");
    }

    // ===== Additional edge case tests =====

    #[test]
    fn test_telegram_photo_without_caption() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 300,
                    "photo": [{"file_id": "small"}, {"file_id": "big"}],
                    "from": {"id": 10, "first_name": "NoCaption"},
                    "chat": {"id": 10, "type": "private"},
                    "date": 1700000300
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "big");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_telegram_photo_empty_array() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 301,
                    "photo": [],
                    "from": {"id": 11},
                    "chat": {"id": 11, "type": "private"},
                    "date": 1700000301
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Image { url, .. } => assert_eq!(url, ""),
            _ => panic!("Expected Image with empty url"),
        }
    }

    #[test]
    fn test_slack_file_shared_without_mimetype() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "file_shared",
                    "file": {
                        "url_private": "https://slack.com/file/456",
                        "name": "no-mime.txt"
                    },
                    "user": "U111",
                    "channel": "C222",
                    "ts": "1700000300.000001",
                    "channel_type": "im"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Private);
        match &msg.content {
            MessageContent::File {
                url,
                filename,
                mime_type,
            } => {
                assert_eq!(url, "https://slack.com/file/456");
                assert_eq!(filename, "no-mime.txt");
                assert!(mime_type.is_none());
            }
            _ => panic!("Expected File"),
        }
    }

    #[test]
    fn test_feishu_file_message_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_file",
                        "message_type": "file",
                        "content": "file_key_abc",
                        "chat_id": "oc_file",
                        "chat_type": "p2p",
                        "create_time": "1700000400"
                    },
                    "sender": {
                        "sender_id": {"open_id": "ou_file"}
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // "file" is not "text" or "image", falls through to default
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "file_key_abc"),
            _ => panic!("Expected Text fallback for file type"),
        }
    }

    #[test]
    fn test_wechat_build_reply_with_reply_to() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "user1".to_string(),
            sender_name: None,
            receiver_id: Some("bot1".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("hello back".to_string()),
            reply_to: Some("msg_1".to_string()),
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["FromUserName"], "bot1");
        assert_eq!(resp.body["ToUserName"], "user1");
    }

    #[test]
    fn test_slack_build_reply_with_receiver_none() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Slack,
            sender_id: "U999".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        // When receiver_id is None, falls back to sender_id
        assert_eq!(resp.body["channel"], "U999");
    }

    #[test]
    fn test_manager_handle_wrong_platform_request() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        // Send a Telegram request but only Wechat adapter registered
        let request = telegram_text_request();
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No adapter registered"));
    }

    #[test]
    fn test_webhook_request_with_query_params() {
        let mut query = HashMap::new();
        query.insert("timestamp".to_string(), "12345".to_string());
        query.insert("nonce".to_string(), "abc".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({"MsgType": "text", "Content": "hello"}),
            query_params: query,
        };
        assert_eq!(request.query_params.get("timestamp").unwrap(), "12345");
    }

    #[test]
    fn test_unified_message_with_raw_data() {
        let raw = serde_json::json!({"extra": "data"});
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Telegram,
            sender_id: "u1".to_string(),
            sender_name: Some("User".to_string()),
            receiver_id: Some("c1".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Group,
            timestamp: 999,
            raw_data: Some(raw.clone()),
        };
        assert_eq!(msg.raw_data.unwrap()["extra"], "data");
        assert_eq!(msg.sender_name.unwrap(), "User");
    }

    #[test]
    fn test_message_content_image_without_caption() {
        let content = MessageContent::Image {
            url: "http://img.jpg".to_string(),
            caption: None,
        };
        let json = serde_json::to_string(&content).unwrap();
        let back: MessageContent = serde_json::from_str(&json).unwrap();
        match back {
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "http://img.jpg");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_message_content_file_without_mime() {
        let content = MessageContent::File {
            url: "http://f.txt".to_string(),
            filename: "f.txt".to_string(),
            mime_type: None,
        };
        let json = serde_json::to_string(&content).unwrap();
        let back: MessageContent = serde_json::from_str(&json).unwrap();
        match back {
            MessageContent::File { mime_type, .. } => assert!(mime_type.is_none()),
            _ => panic!("Expected File"),
        }
    }

    #[test]
    fn test_event_type_subscribe_serde() {
        let et = EventType::Subscribe;
        let json = serde_json::to_string(&et).unwrap();
        let back: EventType = serde_json::from_str(&json).unwrap();
        match back {
            EventType::Subscribe => {}
            _ => panic!("Expected Subscribe"),
        }
    }

    #[test]
    fn test_event_type_custom_serde() {
        let et = EventType::Custom("deploy_complete".to_string());
        let json = serde_json::to_string(&et).unwrap();
        let back: EventType = serde_json::from_str(&json).unwrap();
        match back {
            EventType::Custom(s) => assert_eq!(s, "deploy_complete"),
            _ => panic!("Expected Custom"),
        }
    }

    // ===== More edge case tests =====

    #[test]
    fn test_wechat_parse_video_message() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "vid_001",
                "MsgType": "video",
                "Content": "video_url_here",
                "FromUserName": "user1",
                "ToUserName": "bot1",
                "CreateTime": 1700000500
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // "video" is not "text" or "image", falls to default
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "video_url_here"),
            _ => panic!("Expected Text fallback for video"),
        }
    }

    #[test]
    fn test_wechat_parse_voice_message() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "voice_001",
                "MsgType": "voice",
                "Content": "voice_recognition_text",
                "FromUserName": "user2",
                "ToUserName": "bot2",
                "CreateTime": 1700000501
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "voice_recognition_text"),
            _ => panic!("Expected Text fallback for voice"),
        }
    }

    #[test]
    fn test_telegram_parse_document_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 400,
                    "document": {"file_id": "doc_123", "file_name": "report.pdf"},
                    "from": {"id": 20, "first_name": "DocUser"},
                    "chat": {"id": 20, "type": "private"},
                    "date": 1700000400
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // "document" has no "text" or "photo", falls to else branch
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected Text fallback for document"),
        }
    }

    #[test]
    fn test_telegram_parse_sticker_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 401,
                    "sticker": {"file_id": "sticker_123"},
                    "from": {"id": 21},
                    "chat": {"id": 21, "type": "private"},
                    "date": 1700000401
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected Text fallback for sticker"),
        }
    }

    #[test]
    fn test_slack_parse_message_with_mention() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "<@U123> please help",
                    "user": "U456",
                    "channel": "C789",
                    "ts": "1700000500.000001",
                    "channel_type": "mpim"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "<@U123> please help"),
            _ => panic!("Expected Text"),
        }
        assert_eq!(msg.message_type, MessageType::Private);
    }

    #[test]
    fn test_feishu_parse_unknown_message_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_unknown",
                        "message_type": "audio",
                        "content": "audio_key_123",
                        "chat_id": "oc_audio",
                        "chat_type": "p2p",
                        "create_time": "1700000600"
                    },
                    "sender": {
                        "sender_id": {"open_id": "ou_audio"}
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // "audio" is not "text" or "image", falls to default
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "audio_key_123"),
            _ => panic!("Expected Text fallback for audio"),
        }
    }

    #[test]
    fn test_feishu_parse_video_message_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_video",
                        "message_type": "video",
                        "content": "video_key_456",
                        "chat_id": "oc_video",
                        "chat_type": "group",
                        "create_time": "1700000601"
                    },
                    "sender": {
                        "sender_id": {"open_id": "ou_video"}
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "video_key_456"),
            _ => panic!("Expected Text fallback for video"),
        }
    }

    #[test]
    fn test_factory_create_all_with_config() {
        let mut config = HashMap::new();
        config.insert("token".to_string(), "test_token".to_string());
        config.insert("encoding_aes_key".to_string(), "test_key".to_string());
        config.insert("bot_token".to_string(), "bot_123".to_string());
        config.insert("verification_token".to_string(), "verify_123".to_string());
        config.insert("signing_secret".to_string(), "sign_123".to_string());
        config.insert("encrypt_key".to_string(), "enc_123".to_string());

        for platform in &[
            ImPlatform::Wechat,
            ImPlatform::Telegram,
            ImPlatform::Slack,
            ImPlatform::Feishu,
        ] {
            let adapter = AdapterFactory::create(platform, &config);
            assert_eq!(adapter.platform_type(), *platform);
        }
    }

    #[test]
    fn test_manager_register_overwrites_existing() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("first".to_string(), None)),
        );
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("second".to_string(), None)),
        );
        assert_eq!(manager.adapters.len(), 1);
    }

    #[test]
    fn test_manager_unregister_adapter() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        assert_eq!(manager.adapters.len(), 1);
        manager.adapters.remove(&ImPlatform::Wechat);
        assert_eq!(manager.adapters.len(), 0);
    }

    // ===== Channel type tests =====

    #[test]
    fn test_telegram_parse_channel_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 600,
                    "text": "Broadcast message",
                    "from": {"id": 90, "first_name": "Admin"},
                    "chat": {"id": -100200, "type": "channel"},
                    "date": 1700000700
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // Telegram "channel" type falls to else => Private (not Channel)
        assert_eq!(msg.message_type, MessageType::Private);
        assert_eq!(msg.receiver_id, Some("-100200".to_string()));
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Broadcast message"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_slack_parse_channel_type() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "Channel broadcast",
                    "user": "U001",
                    "channel": "C_broadcast",
                    "ts": "1700000800.000001",
                    "channel_type": "channel"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // channel_type="channel" falls to else => Private (not ideal, but matches impl)
        assert_eq!(msg.message_type, MessageType::Private);
    }

    // ===== Silent reply tests =====

    #[test]
    fn test_telegram_build_reply_silent() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("silent reply".to_string()),
            reply_to: None,
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        // silent flag has no effect in current impl
        assert_eq!(resp.body["text"], "silent reply");
    }

    #[test]
    fn test_feishu_build_reply_silent() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("silent feishu".to_string()),
            reply_to: None,
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
    }

    #[test]
    fn test_wechat_build_reply_silent() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("silent wechat".to_string()),
            reply_to: None,
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
    }

    #[test]
    fn test_slack_build_reply_silent() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("silent slack".to_string()),
            reply_to: None,
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
    }

    // ===== Telegram venue / location edge cases =====

    #[test]
    fn test_telegram_parse_venue_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 700,
                    "venue": {
                        "title": "Office",
                        "address": "123 Main St"
                    },
                    "from": {"id": 100, "first_name": "Bob"},
                    "chat": {"id": 100, "type": "private"},
                    "date": 1700000900
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // venue has no "text" or "photo" => falls to "Unsupported"
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected Text fallback for venue"),
        }
    }

    #[test]
    fn test_telegram_parse_contact_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 701,
                    "contact": {
                        "first_name": "Charlie",
                        "phone_number": "+1234567890"
                    },
                    "from": {"id": 101, "first_name": "Dave"},
                    "chat": {"id": 101, "type": "private"},
                    "date": 1700000901
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // contact has no "text" or "photo" => falls to "Unsupported"
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected Text fallback for contact"),
        }
    }

    // ===== Wechat image with MediaId (no PicUrl) =====

    #[test]
    fn test_wechat_parse_image_with_media_id_only() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "img_002",
                "MsgType": "image",
                "MediaId": "media123",
                "FromUserName": "user_005",
                "ToUserName": "kias_bot",
                "CreateTime": 1700000100
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            // No PicUrl => Image { url: "", caption: None }
            MessageContent::Image { url, caption } => {
                assert_eq!(url, "");
                assert!(caption.is_none());
            }
            _ => panic!("Expected Image content"),
        }
    }

    // ===== Wechat text with FromUserName missing =====

    #[test]
    fn test_wechat_parse_missing_from_user_name() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "msg_miss",
                "MsgType": "text",
                "Content": "test",
                "ToUserName": "bot",
                "CreateTime": 1700000200
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.sender_id, "unknown");
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "test"),
            _ => panic!("Expected Text"),
        }
    }

    // ===== Manager with Custom platform =====

    #[test]
    fn test_manager_register_custom_platform() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Custom,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        let request = WebhookRequest {
            platform: ImPlatform::Custom,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "12345",
                "MsgType": "text",
                "Content": "Hello AgentGuard",
                "FromUserName": "user_001",
                "ToUserName": "kias_bot",
                "CreateTime": 1700000000
            }),
            query_params: HashMap::new(),
        };
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    // ===== Manager parse_webhook error propagation =====

    #[test]
    fn test_manager_url_verification_error_propagates() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Slack,
            Box::new(SlackAdapter::new("token".to_string(), None)),
        );
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({"type": "url_verification", "challenge": "test"}),
            query_params: HashMap::new(),
        };
        let result = manager.handle_webhook(&request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "url_verification");
    }

    // ===== Feishu sender name missing =====

    #[test]
    fn test_feishu_parse_sender_name_missing() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_nm",
                        "message_type": "text",
                        "content": "{\"text\":\"hello\"}",
                        "chat_id": "oc_no_name",
                        "chat_type": "p2p",
                        "create_time": "1700000300"
                    },
                    "sender": {
                        "sender_id": {
                            "open_id": "ou_no_name"
                        }
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.sender_id, "ou_no_name");
        assert!(msg.sender_name.is_none());
    }

    // ===== Feishu chat_type missing =====

    #[test]
    fn test_feishu_parse_chat_type_missing() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_no_ct",
                        "message_type": "text",
                        "content": "{\"text\":\"hello\"}",
                        "chat_id": "oc_no_ct",
                        "create_time": "1700000400"
                    },
                    "sender": {
                        "sender_id": {
                            "open_id": "ou_no_ct"
                        }
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // chat_type missing => Private
        assert_eq!(msg.message_type, MessageType::Private);
    }

    // ===== Slack channel_type missing =====

    #[test]
    fn test_slack_parse_channel_type_missing() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "no channel_type",
                    "user": "U_no_ct",
                    "ts": "1700000500.000001"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Private);
        assert!(msg.receiver_id.is_none());
    }

    // ===== MessageType Channel serde =====

    #[test]
    fn test_message_type_channel_serde() {
        let mt = MessageType::Channel;
        let json = serde_json::to_string(&mt).unwrap();
        let back: MessageType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, MessageType::Channel);
    }

    #[test]
    fn test_message_type_system_serde() {
        let mt = MessageType::System;
        let json = serde_json::to_string(&mt).unwrap();
        let back: MessageType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, MessageType::System);
    }

    // ===== EventType JoinGroup / LeaveGroup =====

    #[test]
    fn test_event_type_join_group_serde() {
        let et = EventType::JoinGroup;
        let json = serde_json::to_string(&et).unwrap();
        let back: EventType = serde_json::from_str(&json).unwrap();
        match back {
            EventType::JoinGroup => {}
            _ => panic!("Expected JoinGroup"),
        }
    }

    #[test]
    fn test_event_type_leave_group_serde() {
        let et = EventType::LeaveGroup;
        let json = serde_json::to_string(&et).unwrap();
        let back: EventType = serde_json::from_str(&json).unwrap();
        match back {
            EventType::LeaveGroup => {}
            _ => panic!("Expected LeaveGroup"),
        }
    }

    // ===== UnifiedMessage with Location content =====

    #[test]
    fn test_unified_message_location_content() {
        let msg = UnifiedMessage {
            id: "loc1".to_string(),
            platform: ImPlatform::Telegram,
            sender_id: "u_loc".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Location {
                latitude: 40.7128,
                longitude: -74.0060,
                address: Some("New York, NY".to_string()),
            },
            message_type: MessageType::Private,
            timestamp: 1234567890,
            raw_data: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: UnifiedMessage = serde_json::from_str(&json).unwrap();
        match &back.content {
            MessageContent::Location {
                latitude,
                longitude,
                address,
            } => {
                assert!((latitude - 40.7128).abs() < 0.001);
                assert!((longitude - (-74.0060)).abs() < 0.001);
                assert_eq!(address.as_deref(), Some("New York, NY"));
            }
            _ => panic!("Expected Location"),
        }
    }

    // ===== ImPlatform Custom serde =====

    #[test]
    fn test_im_platform_custom_serde() {
        let p = ImPlatform::Custom;
        let json = serde_json::to_string(&p).unwrap();
        let back: ImPlatform = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ImPlatform::Custom);
    }

    // ===== Slack url verification - wrong platform =====

    #[test]
    fn test_wechat_url_verification_returns_err() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "123",
                "MsgType": "text",
                "Content": "hello"
            }),
            query_params: HashMap::new(),
        };
        let result = adapter.parse_webhook(&request);
        assert!(result.is_ok());
        // WechatAdapter doesn't do url_verification check
    }

    // ===== Telegram parse with null from =====

    #[test]
    fn test_telegram_parse_from_null() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 800,
                    "text": "hello",
                    "chat": {"id": 200, "type": "private"},
                    "date": 1700001000
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.sender_id, "0"); // null from => id 0
    }

    // ===== Feishu build_reply with Location =====

    #[test]
    fn test_feishu_build_reply_location() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&feishu_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Location {
                latitude: 30.0,
                longitude: 120.0,
                address: None,
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["content"]["text"], "Unsupported message type");
    }

    // ===== Wechat build_reply with Location =====

    #[test]
    fn test_wechat_build_reply_location() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Location {
                latitude: 31.2,
                longitude: 121.4,
                address: Some("Shanghai".to_string()),
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["Content"], "Unsupported message type");
    }

    // ===== Slack build_reply with File =====

    #[test]
    fn test_slack_build_reply_file() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = adapter.parse_webhook(&slack_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::File {
                url: "http://file.pdf".to_string(),
                filename: "report.pdf".to_string(),
                mime_type: Some("application/pdf".to_string()),
            },
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["text"], "Unsupported message type");
    }

    // ===== Telegram parse voice message =====

    #[test]
    fn test_telegram_parse_voice_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 900,
                    "voice": {"file_id": "voice_abc", "duration": 5},
                    "from": {"id": 300},
                    "chat": {"id": 300, "type": "private"},
                    "date": 1700001100
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // voice has no "text" or "photo" => falls to "Unsupported"
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected Text fallback"),
        }
    }

    // ===== Manager all adapters then remove one =====

    #[test]
    fn test_manager_remove_then_add_different_adapter() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("first".to_string(), None)),
        );
        manager.adapters.remove(&ImPlatform::Wechat);
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(TelegramAdapter::new("token".to_string())),
        );
        assert_eq!(manager.adapters.len(), 1);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 1,
                    "text": "hi",
                    "from": {"id": 1},
                    "chat": {"id": 1, "type": "private"},
                    "date": 0
                }
            }),
            query_params: HashMap::new(),
        };
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    // ===== Verify all adapter verify_signature returns true =====

    #[test]
    fn test_all_adapters_verify_signature_always_true() {
        let wechat = WechatAdapter::new("token".to_string(), None);
        let telegram = TelegramAdapter::new("token".to_string());
        let slack = SlackAdapter::new("token".to_string(), None);
        let feishu = FeishuAdapter::new("token".to_string(), None);

        assert!(wechat.verify_signature(&HashMap::new(), b"body"));
        assert!(telegram.verify_signature(&HashMap::new(), b"body"));
        assert!(slack.verify_signature(&HashMap::new(), b"body"));
        assert!(feishu.verify_signature(&HashMap::new(), b"body"));
    }

    // ===== UnifiedMessage with System message type =====

    #[test]
    fn test_unified_message_system_message_type() {
        let msg = UnifiedMessage {
            id: "sys1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "system".to_string(),
            sender_name: None,
            receiver_id: None,
            content: MessageContent::Event(EventType::Custom("system_start".to_string())),
            message_type: MessageType::System,
            timestamp: 999,
            raw_data: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: UnifiedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.message_type, MessageType::System);
    }

    // ===== Wechat build_reply edge: receiver_id present but empty string =====

    #[test]
    fn test_wechat_build_reply_empty_receiver_id() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "user1".to_string(),
            sender_name: None,
            receiver_id: Some("".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        // empty string is still Some(""), not unwrapped as default
        assert_eq!(resp.body["FromUserName"], "");
    }

    // ===== Telegram reply with reply_to set =====

    #[test]
    fn test_telegram_build_reply_with_reply_to() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = adapter.parse_webhook(&telegram_text_request()).unwrap();
        let reply = ReplyMessage {
            content: MessageContent::Text("replying to 99".to_string()),
            reply_to: Some("99".to_string()),
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["reply_to_message_id"], "99");
        assert_eq!(resp.body["chat_id"], "42");
    }

    // ===== Manager handle_webhook with no should_reply from adapter =====

    #[test]
    fn test_manager_response_should_reply_true() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(
            ImPlatform::Wechat,
            Box::new(WechatAdapter::new("t".to_string(), None)),
        );
        let resp = manager.handle_webhook(&wechat_text_request()).unwrap();
        // build_reply always sets should_reply = true
        assert!(resp.should_reply);
    }

    // ===== NEW GAP TESTS =====

    // --- Slack app_mention event type (falls through to Text with "") ---
    #[test]
    fn test_slack_parse_app_mention() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "app_mention",
                    "text": "<@U123> hello",
                    "user": "U456",
                    "channel": "C789",
                    "ts": "1700000700.000001",
                    "channel_type": "channel"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // app_mention is not "message" or "file_shared", falls through to Text(event["text"])
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "<@U123> hello"),
            _ => panic!("Expected Text fallback for app_mention"),
        }
        assert_eq!(msg.sender_id, "U456");
        assert_eq!(msg.receiver_id, Some("C789".to_string()));
        assert_eq!(msg.message_type, MessageType::Private); // channel_type is not "group"
    }

    // --- Slack channel_type = "channel" yields Private (not Group) ---
    #[test]
    fn test_slack_parse_channel_type_channel() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "message",
                    "text": "broadcast",
                    "user": "U111",
                    "channel": "C222",
                    "ts": "1700000800.000001",
                    "channel_type": "channel"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // channel_type "channel" is not "group", so yields Private
        assert_eq!(msg.message_type, MessageType::Private);
    }

    // --- Wechat location message type falls through to Text(Content) ---
    #[test]
    fn test_wechat_parse_location_message() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "loc001",
                "MsgType": "location",
                "Content": "Beijing location description",
                "LocationX": "39.9042",
                "LocationY": "116.4074",
                "Scale": "15",
                "Label": "should be ignored",
                "FromUserName": "user_loc",
                "ToUserName": "kias_bot",
                "CreateTime": 1700000900
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // location is unknown MsgType, falls through to _ => Text(Content)
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Beijing location description"),
            _ => panic!("Expected Text fallback for location"),
        }
    }

    // --- Feishu with missing chat_type falls through to Private ---
    #[test]
    fn test_feishu_parse_missing_chat_type() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_no_type",
                        "message_type": "text",
                        "content": "{\"text\":\"no chat_type\"}",
                        "create_time": "1700001000"
                    },
                    "sender": {
                        "sender_id": {"open_id": "ou_no_type"}
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        // chat_type missing -> else branch -> Private
        assert_eq!(msg.message_type, MessageType::Private);
    }

    // --- Telegram location message falls through to "Unsupported message type" ---
    #[test]
    fn test_telegram_parse_location_message() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 888,
                    "location": {"latitude": 39.9, "longitude": 116.4},
                    "from": {"id": 99, "first_name": "LocUser"},
                    "chat": {"id": 99, "type": "private"},
                    "date": 1700001100
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected unsupported message type fallback"),
        }
    }

    // --- Telegram with sticker message falls through to "Unsupported message type" ---
    #[test]
    fn test_telegram_parse_sticker_message_2() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 999,
                    "sticker": {"file_id": "sticker123"},
                    "from": {"id": 100, "first_name": "StickerUser"},
                    "chat": {"id": 100, "type": "private"},
                    "date": 1700001200
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "Unsupported message type"),
            _ => panic!("Expected unsupported message type fallback for sticker"),
        }
    }

    // --- Wechat build_reply with receiver_id Some (normal case) ---
    #[test]
    fn test_wechat_build_reply_normal() {
        let adapter = WechatAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "r1".to_string(),
            platform: ImPlatform::Wechat,
            sender_id: "sender".to_string(),
            sender_name: None,
            receiver_id: Some("receiver".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("reply".to_string()),
            reply_to: None,
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["ToUserName"], "sender");
        assert_eq!(resp.body["FromUserName"], "receiver");
    }

    // --- Slack build_reply with both receiver and sender ---
    #[test]
    fn test_slack_build_reply_normal() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "r2".to_string(),
            platform: ImPlatform::Slack,
            sender_id: "U_abc".to_string(),
            sender_name: None,
            receiver_id: Some("C_def".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Group,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("slack reply".to_string()),
            reply_to: Some("thread_xyz".to_string()),
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["channel"], "C_def");
        assert_eq!(resp.body["thread_ts"], "thread_xyz");
    }

    // --- Telegram build_reply normal ---
    #[test]
    fn test_telegram_build_reply_normal() {
        let adapter = TelegramAdapter::new("token".to_string());
        let msg = UnifiedMessage {
            id: "r3".to_string(),
            platform: ImPlatform::Telegram,
            sender_id: "555".to_string(),
            sender_name: None,
            receiver_id: Some("555".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Private,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("tg reply".to_string()),
            reply_to: Some("msg_555".to_string()),
            silent: false,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.body["chat_id"], "555");
        assert_eq!(resp.body["reply_to_message_id"], "msg_555");
    }

    // --- Feishu build_reply normal ---
    #[test]
    fn test_feishu_build_reply_normal() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let msg = UnifiedMessage {
            id: "r4".to_string(),
            platform: ImPlatform::Feishu,
            sender_id: "ou_f".to_string(),
            sender_name: Some("FeishuUser".to_string()),
            receiver_id: Some("oc_f".to_string()),
            content: MessageContent::Text("hi".to_string()),
            message_type: MessageType::Group,
            timestamp: 100,
            raw_data: None,
        };
        let reply = ReplyMessage {
            content: MessageContent::Text("feishu reply".to_string()),
            reply_to: Some("orig_f".to_string()),
            silent: true,
        };
        let resp = adapter.build_reply(&msg, &reply).unwrap();
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body["msg_type"], "text");
        assert_eq!(resp.body["content"]["text"], "feishu reply");
    }

    // --- Manager: all 4 platforms handle_webhook simultaneously ---
    #[test]
    fn test_manager_four_platforms_simultaneous() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(ImPlatform::Wechat, Box::new(WechatAdapter::new("t".to_string(), None)));
        manager.register_adapter(ImPlatform::Telegram, Box::new(TelegramAdapter::new("t".to_string())));
        manager.register_adapter(ImPlatform::Slack, Box::new(SlackAdapter::new("t".to_string(), None)));
        manager.register_adapter(ImPlatform::Feishu, Box::new(FeishuAdapter::new("t".to_string(), None)));

        // Wechat
        let r1 = manager.handle_webhook(&wechat_text_request()).unwrap();
        assert_eq!(r1.status_code, 200);
        // Telegram
        let r2 = manager.handle_webhook(&telegram_text_request()).unwrap();
        assert_eq!(r2.status_code, 200);
        // Slack
        let r3 = manager.handle_webhook(&slack_text_request()).unwrap();
        assert_eq!(r3.status_code, 200);
        // Feishu
        let r4 = manager.handle_webhook(&feishu_text_request()).unwrap();
        assert_eq!(r4.status_code, 200);
    }

    // --- UnifiedMessage with Location content through manager ---
    #[test]
    fn test_manager_wechat_location_message() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(ImPlatform::Wechat, Box::new(WechatAdapter::new("t".to_string(), None)));
        let request = WebhookRequest {
            platform: ImPlatform::Wechat,
            headers: HashMap::new(),
            body: serde_json::json!({
                "MsgId": "loc002",
                "MsgType": "location",
                "Content": " Somewhere",
                "LocationX": "40.0",
                "LocationY": "116.5",
                "Scale": "10",
                "Label": "Beijing",
                "FromUserName": "user_loc2",
                "ToUserName": "kias_bot",
                "CreateTime": 1700001300
            }),
            query_params: HashMap::new(),
        };
        let resp = manager.handle_webhook(&request).unwrap();
        assert_eq!(resp.status_code, 200);
        // Falls through to _ => Text(Content), Display = " Somewhere"
        assert!(resp.body["Content"].as_str().unwrap().contains("Somewhere"));
    }

    // --- AdapterFactory: each platform produces the right adapter type ---
    #[test]
    fn test_factory_exact_platform_types() {
        let cfg = HashMap::new();
        // Wechat
        let a = AdapterFactory::create(&ImPlatform::Wechat, &cfg);
        assert_eq!(a.platform_type(), ImPlatform::Wechat);
        // Telegram
        let a = AdapterFactory::create(&ImPlatform::Telegram, &cfg);
        assert_eq!(a.platform_type(), ImPlatform::Telegram);
        // Slack
        let a = AdapterFactory::create(&ImPlatform::Slack, &cfg);
        assert_eq!(a.platform_type(), ImPlatform::Slack);
        // Feishu
        let a = AdapterFactory::create(&ImPlatform::Feishu, &cfg);
        assert_eq!(a.platform_type(), ImPlatform::Feishu);
        // Custom falls back to Wechat
        let a = AdapterFactory::create(&ImPlatform::Custom, &cfg);
        assert_eq!(a.platform_type(), ImPlatform::Wechat);
    }

    // --- UnifiedMessage serde: roundtrip preserves all fields including raw_data ---
    #[test]
    fn test_unified_message_full_roundtrip() {
        let raw = serde_json::json!({"platform_data": "extra", "nested": {"key": 42}});
        let msg = UnifiedMessage {
            id: "full_round".to_string(),
            platform: ImPlatform::Feishu,
            sender_id: "ou_full".to_string(),
            sender_name: Some("FullUser".to_string()),
            receiver_id: Some("oc_full".to_string()),
            content: MessageContent::Location {
                latitude: 31.2,
                longitude: 121.4,
                address: Some("Shanghai".to_string()),
            },
            message_type: MessageType::Group,
            timestamp: 99999,
            raw_data: Some(raw.clone()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: UnifiedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "full_round");
        assert_eq!(back.platform, ImPlatform::Feishu);
        assert_eq!(back.sender_id, "ou_full");
        assert_eq!(back.sender_name, Some("FullUser".to_string()));
        assert_eq!(back.receiver_id, Some("oc_full".to_string()));
        assert_eq!(back.timestamp, 99999);
        let back_raw = back.raw_data.unwrap();
        assert_eq!(back_raw["nested"]["key"], 42);
    }

    // --- WechatAdapter token and encoding_aes_key access through behavior ---
    #[test]
    fn test_wechat_adapter_with_both_keys() {
        let adapter = WechatAdapter::new("my_token".to_string(), Some("my_aes_key_43chars_base64_encoded".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
        let msg = adapter.parse_webhook(&wechat_text_request()).unwrap();
        assert_eq!(msg.platform, ImPlatform::Wechat);
    }

    // --- TelegramAdapter bot_token behavior ---
    #[test]
    fn test_telegram_adapter_token_behavior() {
        let adapter = TelegramAdapter::new("123456:ABC-DEF".to_string());
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    // --- SlackAdapter with both verification and signing secret ---
    #[test]
    fn test_slack_adapter_both_secrets() {
        let adapter = SlackAdapter::new("vtoken".to_string(), Some("signing_secret_xyz".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Slack);
    }

    // --- FeishuAdapter with both verification and encrypt key ---
    #[test]
    fn test_feishu_adapter_both_keys() {
        let adapter = FeishuAdapter::new("vtoken".to_string(), Some("encrypt_abc".to_string()));
        assert_eq!(adapter.platform_type(), ImPlatform::Feishu);
    }

    // --- WebhookResponse: should_reply=false serde ---
    #[test]
    fn test_webhook_response_should_reply_false() {
        let resp = WebhookResponse {
            status_code: 204,
            body: serde_json::json!({"ok": true}),
            should_reply: false,
            reply: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WebhookResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status_code, 204);
        assert!(!back.should_reply);
        assert!(back.reply.is_none());
    }

    // --- Manager register_adapter returns () not self ---
    #[test]
    fn test_manager_register_returns_unit() {
        let mut manager = ImIntegrationManager::new();
        let result = manager.register_adapter(ImPlatform::Wechat, Box::new(WechatAdapter::new("t".to_string(), None)));
        assert_eq!(result, ());
        assert!(manager.adapters.contains_key(&ImPlatform::Wechat));
    }

    // --- ImIntegrationManager new is empty ---
    #[test]
    fn test_manager_new_is_empty() {
        let manager = ImIntegrationManager::new();
        assert!(manager.adapters.is_empty());
    }

    // --- AdapterFactory::create with extra unused config keys ---
    #[test]
    fn test_factory_ignores_extra_config_keys() {
        let mut config = HashMap::new();
        config.insert("unused_key".to_string(), "unused_value".to_string());
        config.insert("bot_token".to_string(), "real_token".to_string());
        let adapter = AdapterFactory::create(&ImPlatform::Telegram, &config);
        // Should not panic, just ignore the extra key
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    // --- Telegram: chat id as negative number (supergroup) ---
    #[test]
    fn test_telegram_chat_id_negative() {
        let adapter = TelegramAdapter::new("token".to_string());
        let request = WebhookRequest {
            platform: ImPlatform::Telegram,
            headers: HashMap::new(),
            body: serde_json::json!({
                "message": {
                    "message_id": 777,
                    "text": "supergroup msg",
                    "from": {"id": 11, "first_name": "NegId"},
                    "chat": {"id": -2147483648, "type": "supergroup"},
                    "date": 1700002000
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.message_type, MessageType::Group);
        assert_eq!(msg.receiver_id, Some("-2147483648".to_string()));
    }

    // --- Feishu: sender without name field ---
    #[test]
    fn test_feishu_parse_sender_without_name() {
        let adapter = FeishuAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Feishu,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "message": {
                        "message_id": "msg_noname",
                        "message_type": "text",
                        "content": "{\"text\":\"no name\"}",
                        "chat_id": "oc_noname",
                        "chat_type": "p2p",
                        "create_time": "1700002100"
                    },
                    "sender": {
                        "sender_id": {"open_id": "ou_noname"}
                    }
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        assert_eq!(msg.sender_name, None);
    }

    // --- Slack: file_shared with mimetype but no name ---
    #[test]
    fn test_slack_file_shared_no_name() {
        let adapter = SlackAdapter::new("token".to_string(), None);
        let request = WebhookRequest {
            platform: ImPlatform::Slack,
            headers: HashMap::new(),
            body: serde_json::json!({
                "event": {
                    "type": "file_shared",
                    "file": {
                        "url_private": "https://slack.com/f/no-name",
                        "mimetype": "image/png"
                    },
                    "user": "U_no_name",
                    "channel": "C_no_name",
                    "ts": "1700002200.000001",
                    "channel_type": "im"
                }
            }),
            query_params: HashMap::new(),
        };
        let msg = adapter.parse_webhook(&request).unwrap();
        match &msg.content {
            MessageContent::File { url, filename, mime_type } => {
                assert_eq!(url, "https://slack.com/f/no-name");
                assert_eq!(filename, "");
                assert_eq!(mime_type.as_deref(), Some("image/png"));
            }
            _ => panic!("Expected File content"),
        }
    }

    // --- UnifiedMessage with Channel message type ---
    #[test]
    fn test_unified_message_channel_type() {
        let msg = UnifiedMessage {
            id: "ch1".to_string(),
            platform: ImPlatform::Slack,
            sender_id: "U_ch".to_string(),
            sender_name: Some("ChannelUser".to_string()),
            receiver_id: Some("C_announce".to_string()),
            content: MessageContent::Text("announcement".to_string()),
            message_type: MessageType::Channel,
            timestamp: 1000,
            raw_data: None,
        };
        assert_eq!(msg.message_type, MessageType::Channel);
        let json = serde_json::to_string(&msg).unwrap();
        let back: UnifiedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.message_type, MessageType::Channel);
    }

    // --- Manager handle_webhook with Feishu adapter that has encrypt key ---
    #[test]
    fn test_manager_feishu_with_encrypt_key() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(ImPlatform::Feishu, Box::new(FeishuAdapter::new("vt".to_string(), Some("ek_123".to_string()))));
        let resp = manager.handle_webhook(&feishu_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.should_reply);
    }

    // --- Manager handle_webhook with Slack adapter that has signing secret ---
    #[test]
    fn test_manager_slack_with_signing_secret() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(ImPlatform::Slack, Box::new(SlackAdapter::new("vtt".to_string(), Some("ss_456".to_string()))));
        let resp = manager.handle_webhook(&slack_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);
    }

    // --- Manager handle_webhook with Wechat adapter that has encoding_aes_key ---
    #[test]
    fn test_manager_wechat_with_encoding_aes_key() {
        let mut manager = ImIntegrationManager::new();
        manager.register_adapter(ImPlatform::Wechat, Box::new(WechatAdapter::new("wt".to_string(), Some("aes_789".to_string()))));
        let resp = manager.handle_webhook(&wechat_text_request()).unwrap();
        assert_eq!(resp.status_code, 200);
    }
}
