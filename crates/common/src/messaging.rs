//! # Agent Message Bus
//!
//! Inter-agent communication system inspired by hcom and OpenAI Swarm patterns.
//! Provides pub/sub topic-based messaging and direct agent-to-agent communication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Unique message identifier
pub type MessageId = String;

/// Topic name for pub/sub
pub type Topic = String;

/// Agent message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message ID
    pub id: MessageId,
    /// Sender agent ID
    pub from: String,
    /// Target agent ID (None for broadcast)
    pub to: Option<String>,
    /// Topic/channel
    pub topic: Topic,
    /// Message payload
    pub payload: serde_json::Value,
    /// Message type for routing
    pub msg_type: MessageType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Correlation ID for request/reply patterns
    pub correlation_id: Option<String>,
}

/// Message types for routing and filtering
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    /// Regular data message
    Data,
    /// Task delegation request
    TaskRequest,
    /// Task result response
    TaskResponse,
    /// Health check ping
    Ping,
    /// Health check pong
    Pong,
    /// Agent spawning request (inspired by hcom)
    Spawn,
    /// Agent status update
    StatusUpdate,
    /// Error notification
    Error,
    /// Custom type
    Custom(String),
}

/// Message bus statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BusStats {
    pub messages_published: u64,
    pub messages_delivered: u64,
    pub active_subscriptions: usize,
    pub active_topics: usize,
}

/// Agent Message Bus - pub/sub + direct messaging
pub struct MessageBus {
    /// Topic-based broadcast channels
    topics: Arc<RwLock<HashMap<Topic, broadcast::Sender<AgentMessage>>>>,
    /// Message history for replay (ring buffer per topic)
    history: Arc<RwLock<HashMap<Topic, Vec<AgentMessage>>>>,
    /// History size limit per topic
    history_limit: usize,
    /// Statistics
    stats: Arc<RwLock<BusStats>>,
}

impl MessageBus {
    /// Create a new message bus with default history limit
    pub fn new() -> Self {
        Self::with_history_limit(1000)
    }

    /// Create a new message bus with specified history limit per topic
    pub fn with_history_limit(limit: usize) -> Self {
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            history_limit: limit,
            stats: Arc::new(RwLock::new(BusStats::default())),
        }
    }

    /// Publish a message to a topic
    pub async fn publish(&self, message: AgentMessage) -> Result<(), String> {
        let topic = message.topic.clone();

        // Ensure topic exists
        let sender = {
            let mut topics = self.topics.write().await;
            topics
                .entry(topic.clone())
                .or_insert_with(|| {
                    let (tx, _) = broadcast::channel(256);
                    tx
                })
                .clone()
        };

        // Store in history
        {
            let mut history = self.history.write().await;
            let entry = history.entry(topic.clone()).or_default();
            entry.push(message.clone());
            if entry.len() > self.history_limit {
                entry.remove(0);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.messages_published += 1;
        }

        // Broadcast (ignore if no receivers)
        let _ = sender.send(message);
        Ok(())
    }

    /// Subscribe to a topic, returns a receiver
    pub async fn subscribe(&self, topic: &str) -> broadcast::Receiver<AgentMessage> {
        let mut topics = self.topics.write().await;
        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(256);
            tx
        });

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_subscriptions += 1;
        }

        sender.subscribe()
    }

    /// Get message history for a topic
    pub async fn history(&self, topic: &str, limit: Option<usize>) -> Vec<AgentMessage> {
        let history = self.history.read().await;
        match history.get(topic) {
            Some(msgs) => {
                let n = limit.unwrap_or(msgs.len());
                msgs.iter().rev().take(n).cloned().collect()
            }
            None => Vec::new(),
        }
    }

    /// Get all active topics
    pub async fn topics(&self) -> Vec<Topic> {
        let topics = self.topics.read().await;
        topics.keys().cloned().collect()
    }

    /// Get bus statistics
    pub async fn stats(&self) -> BusStats {
        let stats = self.stats.read().await;
        let topics = self.topics.read().await;
        BusStats {
            active_topics: topics.len(),
            ..stats.clone()
        }
    }

    /// Direct message to a specific agent (via their inbox topic)
    pub async fn send_direct(
        &self,
        from: &str,
        to: &str,
        payload: serde_json::Value,
        msg_type: MessageType,
    ) -> Result<MessageId, String> {
        let msg = AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.to_string(),
            to: Some(to.to_string()),
            topic: format!("agent:{}:inbox", to),
            payload,
            msg_type,
            timestamp: Utc::now(),
            correlation_id: None,
        };
        let id = msg.id.clone();
        self.publish(msg).await?;
        Ok(id)
    }

    /// Send a request and wait for a correlated response
    pub async fn request_reply(
        &self,
        from: &str,
        to: &str,
        payload: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<AgentMessage, String> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let reply_topic = format!("agent:{}:reply:{}", from, correlation_id);

        let msg = AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.to_string(),
            to: Some(to.to_string()),
            topic: format!("agent:{}:inbox", to),
            payload,
            msg_type: MessageType::TaskRequest,
            timestamp: Utc::now(),
            correlation_id: Some(correlation_id.clone()),
        };

        // Subscribe to reply topic before sending
        let mut rx = self.subscribe(&reply_topic).await;

        // Send the request
        self.publish(msg).await?;

        // Wait for reply with timeout
        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(e)) => Err(format!("Channel error: {}", e)),
            Err(_) => Err("Request timed out".to_string()),
        }
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_message_bus_creation() {
        let bus = MessageBus::new();
        let topics = bus.topics().await;
        assert!(topics.is_empty());
    }

    #[tokio::test]
    async fn test_publish_and_subscribe() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe("test-topic").await;

        let msg = AgentMessage {
            id: "msg-1".to_string(),
            from: "agent-1".to_string(),
            to: None,
            topic: "test-topic".to_string(),
            payload: json!({"hello": "world"}),
            msg_type: MessageType::Data,
            timestamp: Utc::now(),
            correlation_id: None,
        };

        bus.publish(msg.clone()).await.unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.id, "msg-1");
        assert_eq!(received.from, "agent-1");
        assert_eq!(received.payload, json!({"hello": "world"}));
    }

    #[tokio::test]
    async fn test_topic_history() {
        let bus = MessageBus::new();

        for i in 0..5 {
            let msg = AgentMessage {
                id: format!("msg-{}", i),
                from: "agent-1".to_string(),
                to: None,
                topic: "history-topic".to_string(),
                payload: json!({"index": i}),
                msg_type: MessageType::Data,
                timestamp: Utc::now(),
                correlation_id: None,
            };
            bus.publish(msg).await.unwrap();
        }

        let history = bus.history("history-topic", None).await;
        assert_eq!(history.len(), 5);

        let limited = bus.history("history-topic", Some(3)).await;
        assert_eq!(limited.len(), 3);
    }

    #[tokio::test]
    async fn test_direct_messaging() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe("agent:bob:inbox").await;

        bus.send_direct(
            "alice",
            "bob",
            json!({"task": "hello"}),
            MessageType::TaskRequest,
        )
        .await
        .unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.from, "alice");
        assert_eq!(received.to, Some("bob".to_string()));
        assert_eq!(received.topic, "agent:bob:inbox");
    }

    #[tokio::test]
    async fn test_bus_stats() {
        let bus = MessageBus::new();

        bus.publish(AgentMessage {
            id: "msg-1".to_string(),
            from: "a".to_string(),
            to: None,
            topic: "t1".to_string(),
            payload: json!(null),
            msg_type: MessageType::Data,
            timestamp: Utc::now(),
            correlation_id: None,
        })
        .await
        .unwrap();

        let stats = bus.stats().await;
        assert_eq!(stats.messages_published, 1);
        assert_eq!(stats.active_topics, 1);
    }

    #[tokio::test]
    async fn test_message_types() {
        let types = vec![
            MessageType::Data,
            MessageType::TaskRequest,
            MessageType::TaskResponse,
            MessageType::Ping,
            MessageType::Pong,
            MessageType::Spawn,
            MessageType::StatusUpdate,
            MessageType::Error,
            MessageType::Custom("custom".to_string()),
        ];
        assert_eq!(types.len(), 9);
    }

    #[tokio::test]
    async fn test_history_limit() {
        let bus = MessageBus::with_history_limit(3);

        for i in 0..5 {
            bus.publish(AgentMessage {
                id: format!("msg-{}", i),
                from: "a".to_string(),
                to: None,
                topic: "t".to_string(),
                payload: json!(null),
                msg_type: MessageType::Data,
                timestamp: Utc::now(),
                correlation_id: None,
            })
            .await
            .unwrap();
        }

        let history = bus.history("t", None).await;
        assert_eq!(history.len(), 3);
    }
}
