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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    fn build_reply(&self, message: &UnifiedMessage, reply: &ReplyMessage) -> Result<WebhookResponse, String>;
    
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
        Self { token, encoding_aes_key }
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
            sender_id: body["FromUserName"].as_str().unwrap_or("unknown").to_string(),
            sender_name: None,
            receiver_id: body["ToUserName"].as_str().map(|s| s.to_string()),
            content,
            message_type: MessageType::Private,
            timestamp: body["CreateTime"].as_i64().unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(&self, message: &UnifiedMessage, reply: &ReplyMessage) -> Result<WebhookResponse, String> {
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

    fn verify_signature(&self, headers: &HashMap<String, String>, body: &[u8]) -> bool {
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
                url: photo.last().and_then(|p| p["file_id"].as_str()).unwrap_or("").to_string(),
                caption: message["caption"].as_str().map(|s| s.to_string()),
            }
        } else {
            MessageContent::Text("Unsupported message type".to_string())
        };

        let message_type = if chat["type"].as_str() == Some("group") || chat["type"].as_str() == Some("supergroup") {
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

    fn build_reply(&self, message: &UnifiedMessage, reply: &ReplyMessage) -> Result<WebhookResponse, String> {
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

    fn verify_signature(&self, headers: &HashMap<String, String>, body: &[u8]) -> bool {
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
        Self { verification_token, signing_secret }
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
                url: event["file"]["url_private"].as_str().unwrap_or("").to_string(),
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

    fn build_reply(&self, message: &UnifiedMessage, reply: &ReplyMessage) -> Result<WebhookResponse, String> {
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

    fn verify_signature(&self, headers: &HashMap<String, String>, body: &[u8]) -> bool {
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
        Self { verification_token, encrypt_key }
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
                let text_content: serde_json::Value = serde_json::from_str(
                    message["content"].as_str().unwrap_or("{}")
                ).unwrap_or(serde_json::Value::Null);
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
            id: message["message_id"].as_str().unwrap_or("unknown").to_string(),
            platform: ImPlatform::Feishu,
            sender_id: event["sender"]["sender_id"]["open_id"].as_str().unwrap_or("unknown").to_string(),
            sender_name: event["sender"]["sender_id"]["name"].as_str().map(|s| s.to_string()),
            receiver_id: message["chat_id"].as_str().map(|s| s.to_string()),
            content,
            message_type,
            timestamp: message["create_time"].as_str().unwrap_or("0").parse().unwrap_or(0),
            raw_data: Some(body.clone()),
        })
    }

    fn build_reply(&self, message: &UnifiedMessage, reply: &ReplyMessage) -> Result<WebhookResponse, String> {
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

    fn verify_signature(&self, headers: &HashMap<String, String>, body: &[u8]) -> bool {
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
    pub fn create(platform: &ImPlatform, config: &HashMap<String, String>) -> Box<dyn PlatformAdapter> {
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
                let verification_token = config.get("verification_token").cloned().unwrap_or_default();
                let signing_secret = config.get("signing_secret").cloned();
                Box::new(SlackAdapter::new(verification_token, signing_secret))
            }
            ImPlatform::Feishu => {
                let verification_token = config.get("verification_token").cloned().unwrap_or_default();
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
        let adapter = self.adapters.get(&request.platform)
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

    #[test]
    fn test_wechat_adapter() {
        let adapter = WechatAdapter::new("test_token".to_string(), None);
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    #[test]
    fn test_telegram_adapter() {
        let adapter = TelegramAdapter::new("test_token".to_string());
        assert_eq!(adapter.platform_type(), ImPlatform::Telegram);
    }

    #[test]
    fn test_adapter_factory() {
        let config = HashMap::new();
        let adapter = AdapterFactory::create(&ImPlatform::Wechat, &config);
        assert_eq!(adapter.platform_type(), ImPlatform::Wechat);
    }

    #[test]
    fn test_im_integration_manager() {
        let mut manager = ImIntegrationManager::new();
        let adapter = Box::new(WechatAdapter::new("test".to_string(), None));
        manager.register_adapter(ImPlatform::Wechat, adapter);
        
        assert!(manager.adapters.contains_key(&ImPlatform::Wechat));
    }
}
