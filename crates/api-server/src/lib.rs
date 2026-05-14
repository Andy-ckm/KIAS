use std::sync::Arc;

use kias_common::audit::MemoryAuditLog;
use kias_common::config::KiasConfig;
use tokio::sync::RwLock;

use crate::websocket::EventBus;

/// Shared application state passed to all handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<KiasConfig>,
    pub agents: Arc<RwLock<std::collections::HashMap<String, models::agent::Agent>>>,
    pub nodes: Arc<RwLock<std::collections::HashMap<String, models::node::Node>>>,
    pub workflows: Arc<RwLock<std::collections::HashMap<String, handlers::workflows::Workflow>>>,
    pub audit_log: Arc<MemoryAuditLog>,
    pub event_bus: EventBus,
    pub a2a_tasks: handlers::a2a::A2aTaskStore,
}

impl AppState {
    pub fn new(config: KiasConfig) -> Self {
        let mut nodes = std::collections::HashMap::new();

        // Seed default demo nodes
        nodes.insert(
            "node-1".to_string(),
            models::node::Node {
                id: "node-1".to_string(),
                name: "node-1".to_string(),
                status: models::node::NodeStatus::Ready,
                resources: models::node::ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                allocatable: models::node::ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );
        nodes.insert(
            "node-2".to_string(),
            models::node::Node {
                id: "node-2".to_string(),
                name: "node-2".to_string(),
                status: models::node::NodeStatus::Ready,
                resources: models::node::ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                allocatable: models::node::ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        Self {
            config: Arc::new(config),
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(MemoryAuditLog::new()),
            event_bus: EventBus::default(),
            a2a_tasks: handlers::a2a::A2aTaskStore::new(),
        }
    }
}

pub mod auth;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod tls;
pub mod websocket;
