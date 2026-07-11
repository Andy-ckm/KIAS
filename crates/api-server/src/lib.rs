use std::sync::Arc;

use kias_common::audit::MemoryAuditLog;
use kias_common::config::KiasConfig;
use kias_knowledge::graph::KnowledgeGraph;
use kias_knowledge::retriever::Retriever;
use kias_knowledge::vector::{LocalEmbeddingEngine, VectorRetriever};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::websocket::{ConnectionRegistry, EventBus, EventReplayBuffer};

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<KiasConfig>,
    /// Agent persistence repository, when configured.
    pub agent_repository: Option<Arc<kias_data_store::AgentRepository>>,
    /// In-memory agent working set.
    pub agents: Arc<RwLock<std::collections::HashMap<String, models::agent::Agent>>>,
    pub nodes: Arc<RwLock<std::collections::HashMap<String, models::node::Node>>>,
    pub workflows: Arc<RwLock<std::collections::HashMap<String, handlers::workflows::Workflow>>>,
    pub audit_log: Arc<MemoryAuditLog>,
    /// SQLite-backed audit log for durable evidence, when configured.
    pub sqlite_audit_log: Option<Arc<kias_data_store::SqliteAuditLog>>,
    /// Dead-letter queue for failed tasks, when configured.
    pub dead_letter_queue: Option<Arc<kias_data_store::DeadLetterQueue>>,
    /// Idempotency store for API duplicate-request detection, when configured.
    pub idempotency_store: Option<Arc<kias_data_store::IdempotencyRepository>>,
    pub event_bus: EventBus,
    pub a2a_tasks: handlers::a2a::A2aTaskStore,
    pub connection_registry: ConnectionRegistry,
    pub event_replay_buffer: EventReplayBuffer,
    /// Optional knowledge retriever used by knowledge endpoints.
    pub knowledge_retriever: Arc<dyn Retriever>,
    pub ingested_docs: Arc<RwLock<Vec<IngestedDoc>>>,
    pub context_manager: Option<Arc<kias_knowledge::context_manager::MultiSessionContextManager>>,
    pub tier_routing: handlers::tier_routing::TierRoutingState,
    pub gxp_auth: handlers::auth_gxp::GxpAuthState,
    /// JWT signing configuration. A configured runtime secret is always preferred.
    pub jwt_config: auth::JwtConfig,
    pub slow_trace_collector: kias_monitor::SlowTraceCollector,
    pub token_budgets:
        Arc<RwLock<std::collections::HashMap<String, handlers::token_budget::TokenBudget>>>,
}

/// An ingested document stored in memory.
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
    /// Create application state for a Tokio runtime.
    pub async fn new(config: KiasConfig) -> Self {
        let mut nodes = std::collections::HashMap::new();

        // Synthetic nodes remain available for the pre-1.0 dashboard and handler
        // contract tests. Production discovery must replace this bootstrap state.
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

        let graph = KnowledgeGraph::new();
        let embedding_engine = Arc::new(LocalEmbeddingEngine::default_dim());
        let knowledge_retriever = VectorRetriever::new(graph, embedding_engine)
            .await
            .expect("local knowledge retriever initialization must succeed");

        let jwt_secret = config.api_server.jwt_secret.clone().unwrap_or_else(|| {
            // No shared fallback is embedded in the binary. This process-local key
            // only supports configurations that do not use JWT authentication; the
            // production composition root rejects auth-enabled configurations without
            // an explicit verifier before the listener starts.
            format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
        });
        let jwt_issuer = config
            .api_server
            .jwt_issuer
            .clone()
            .unwrap_or_else(|| "kias".to_string());
        let jwt_expiration_hours = config.api_server.jwt_expiration_hours;

        Self {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            idempotency_store: None,
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
            jwt_config: auth::JwtConfig::new(
                jwt_secret,
                jwt_issuer,
                jwt_expiration_hours,
            ),
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn with_persistence(
        mut self,
        audit_log: Arc<kias_data_store::SqliteAuditLog>,
        dead_letter_queue: Arc<kias_data_store::DeadLetterQueue>,
    ) -> Self {
        self.sqlite_audit_log = Some(audit_log);
        self.dead_letter_queue = Some(dead_letter_queue);
        self
    }

    pub fn with_idempotency_store(
        mut self,
        repository: Arc<kias_data_store::IdempotencyRepository>,
    ) -> Self {
        self.idempotency_store = Some(repository);
        self
    }

    /// Compatibility wrapper for existing async test code.
    pub async fn new_async(config: KiasConfig) -> Self {
        Self::new(config).await
    }
}

pub mod auth;
pub mod contract_test;
pub mod error;
pub mod gateway;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod tls;
pub mod websocket;
