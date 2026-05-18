use std::sync::Arc;

use kias_common::audit::MemoryAuditLog;
use kias_common::config::KiasConfig;
use kias_knowledge::graph::KnowledgeGraph;
use kias_knowledge::retriever::Retriever;
use kias_knowledge::vector::{LocalEmbeddingEngine, VectorRetriever};
use tokio::sync::RwLock;

use crate::websocket::{ConnectionRegistry, EventBus, EventReplayBuffer};

/// Shared application state passed to all handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<KiasConfig>,
    /// Agent持久化存储（SQLite）
    pub agent_repository: Option<Arc<kias_data_store::AgentRepository>>,
    /// Agent内存缓存（启动时从SQLite加载）
    pub agents: Arc<RwLock<std::collections::HashMap<String, models::agent::Agent>>>,
    pub nodes: Arc<RwLock<std::collections::HashMap<String, models::node::Node>>>,
    pub workflows: Arc<RwLock<std::collections::HashMap<String, handlers::workflows::Workflow>>>,
    pub audit_log: Arc<MemoryAuditLog>,
    /// SQLite-backed audit log for production persistence (optional)
    pub sqlite_audit_log: Option<Arc<kias_data_store::SqliteAuditLog>>,
    /// Dead letter queue for failed tasks (optional)
    pub dead_letter_queue: Option<Arc<kias_data_store::DeadLetterQueue>>,
    pub event_bus: EventBus,
    pub a2a_tasks: handlers::a2a::A2aTaskStore,
    /// Tracks active WebSocket connections and metrics.
    pub connection_registry: ConnectionRegistry,
    /// Ring buffer for event replay to new WebSocket clients.
    pub event_replay_buffer: EventReplayBuffer,
    /// Knowledge base retriever (vector search + hybrid retrieval)
    pub knowledge_retriever: Arc<dyn Retriever>,
    /// Ingested documents store (for RAG ingest → search pipeline)
    pub ingested_docs: Arc<RwLock<Vec<IngestedDoc>>>,
    /// Session context manager (7-layer memory architecture)
    pub context_manager: Option<Arc<kias_knowledge::context_manager::MultiSessionContextManager>>,
    /// Tier routing state (PrfaaS-inspired intelligent task routing)
    pub tier_routing: handlers::tier_routing::TierRoutingState,
    /// GxP-compliant authentication manager (§11.200, §11.300)
    pub gxp_auth: handlers::auth_gxp::GxpAuthState,
    /// JWT configuration for token generation
    pub jwt_config: auth::JwtConfig,
}

/// An ingested document stored in memory
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngestedDoc {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub source_type: String,
    pub chunks: Vec<String>,
    pub ingested_at: String,
}

impl AppState {
    /// Create AppState (async, for use in tokio runtime)
    pub async fn new(config: KiasConfig) -> Self {
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

        // Build knowledge retriever with an empty graph (populated at runtime)
        let graph = KnowledgeGraph::new();
        let embedding_engine = Arc::new(LocalEmbeddingEngine::default_dim());
        let knowledge_retriever = VectorRetriever::new(graph, embedding_engine)
            .await
            .expect("Failed to initialize knowledge retriever");

        Self {
            config: Arc::new(config),
            agent_repository: None, // Will be set up if SQLite is configured
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(MemoryAuditLog::new()),
            sqlite_audit_log: None,  // Will be set up if SQLite is configured
            dead_letter_queue: None, // Will be set up if SQLite is configured
            event_bus: EventBus::default(),
            a2a_tasks: handlers::a2a::A2aTaskStore::new(),
            connection_registry: ConnectionRegistry::default(),
            event_replay_buffer: EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: handlers::auth_gxp::create_gxp_auth_state(
                kias_common::gxp_auth::PasswordPolicy::default(),
            ),
            jwt_config: auth::JwtConfig::new("kias-default-jwt-secret-change-me", "kias", 24),
        }
    }

    /// Attach persistent audit log and DLQ after construction.
    ///
    /// Called by the service manager once SQLite is initialized.
    pub fn with_persistence(
        mut self,
        audit_log: Arc<kias_data_store::SqliteAuditLog>,
        dlq: Arc<kias_data_store::DeadLetterQueue>,
    ) -> Self {
        self.sqlite_audit_log = Some(audit_log);
        self.dead_letter_queue = Some(dlq);
        self
    }

    /// Create AppState asynchronously (for use in async test contexts)
    pub async fn new_async(config: KiasConfig) -> Self {
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

        // Build knowledge retriever with an empty graph (populated at runtime)
        let graph = KnowledgeGraph::new();
        let embedding_engine = Arc::new(LocalEmbeddingEngine::default_dim());
        let knowledge_retriever = VectorRetriever::new(graph, embedding_engine)
            .await
            .expect("Failed to initialize knowledge retriever");

        Self {
            config: Arc::new(config),
            agent_repository: None, // Will be set up if SQLite is configured
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(MemoryAuditLog::new()),
            sqlite_audit_log: None,  // Will be set up if SQLite is configured
            dead_letter_queue: None, // Will be set up if SQLite is configured
            event_bus: EventBus::default(),
            a2a_tasks: handlers::a2a::A2aTaskStore::new(),
            connection_registry: ConnectionRegistry::default(),
            event_replay_buffer: EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: handlers::auth_gxp::create_gxp_auth_state(
                kias_common::gxp_auth::PasswordPolicy::default(),
            ),
            jwt_config: auth::JwtConfig::new("kias-default-jwt-secret-change-me", "kias", 24),
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
