use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::models::agent::AgentStatus;
use crate::AppState;

/// Current scheduler algorithm
#[derive(Debug, Serialize)]
pub struct SchedulerAlgorithm {
    pub name: String,
    pub description: String,
}

/// Queue depth snapshot
#[derive(Debug, Serialize)]
pub struct QueueDepth {
    pub pending: usize,
    pub scheduled: usize,
    pub running: usize,
}

/// Scheduling throughput metrics
#[derive(Debug, Serialize)]
pub struct SchedulingThroughput {
    pub total_scheduled: usize,
    pub total_completed: usize,
    pub total_failed: usize,
    pub success_rate: f64,
    pub avg_restart_count: f64,
}

/// Recent scheduling decision
#[derive(Debug, Serialize)]
pub struct SchedulingDecision {
    pub agent_id: String,
    pub agent_name: String,
    pub assigned_node: Option<String>,
    pub status: String,
    pub priority: String,
    pub timestamp: String,
}

/// Full scheduler status response
#[derive(Debug, Serialize)]
pub struct SchedulerStatus {
    pub current_algorithm: SchedulerAlgorithm,
    pub queue_depth: QueueDepth,
    pub throughput: SchedulingThroughput,
    pub node_utilization: Vec<NodeUtilization>,
    pub recent_decisions: Vec<SchedulingDecision>,
}

/// Node utilization info
#[derive(Debug, Serialize)]
pub struct NodeUtilization {
    pub node_id: String,
    pub node_name: String,
    pub agent_count: usize,
    pub running_count: usize,
    pub status: String,
}

/// GET /api/v1/scheduler/status
/// Returns comprehensive scheduler status including algorithm, queue, throughput, and node utilization.
pub async fn scheduler_status(State(state): State<AppState>) -> Json<SchedulerStatus> {
    let agents = state.agents.read().await;
    let nodes = state.nodes.read().await;

    // Queue depth
    let pending = agents
        .values()
        .filter(|a| a.status == AgentStatus::Pending)
        .count();
    let scheduled = agents
        .values()
        .filter(|a| a.status == AgentStatus::Scheduled)
        .count();
    let running = agents
        .values()
        .filter(|a| a.status == AgentStatus::Running)
        .count();

    // Throughput
    let total_scheduled = agents.len();
    let total_completed = agents
        .values()
        .filter(|a| a.status == AgentStatus::Succeeded)
        .count();
    let total_failed = agents
        .values()
        .filter(|a| a.status == AgentStatus::Failed)
        .count();
    let success_rate = if total_scheduled > 0 {
        total_completed as f64 / total_scheduled as f64 * 100.0
    } else {
        100.0
    };
    let avg_restart = if !agents.is_empty() {
        agents.values().map(|a| a.restart_count as f64).sum::<f64>() / agents.len() as f64
    } else {
        0.0
    };

    // Node utilization
    let node_utilization: Vec<NodeUtilization> = nodes
        .values()
        .map(|n| {
            let node_agents: Vec<_> = agents
                .values()
                .filter(|a| a.node_id.as_deref() == Some(&n.id))
                .collect();
            let running_count = node_agents
                .iter()
                .filter(|a| a.status == AgentStatus::Running)
                .count();
            NodeUtilization {
                node_id: n.id.clone(),
                node_name: n.name.clone(),
                agent_count: node_agents.len(),
                running_count,
                status: format!("{:?}", n.status),
            }
        })
        .collect();

    // Recent decisions (last 10 agents by updated_at)
    let mut recent: Vec<SchedulingDecision> = agents
        .values()
        .map(|a| SchedulingDecision {
            agent_id: a.id.clone(),
            agent_name: a.spec.name.clone(),
            assigned_node: a.node_id.clone(),
            status: format!("{:?}", a.status),
            priority: a.spec.priority.clone(),
            timestamp: a.updated_at.clone(),
        })
        .collect();
    recent.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    recent.truncate(10);

    Json(SchedulerStatus {
        current_algorithm: SchedulerAlgorithm {
            name: "Weighted Round Robin".to_string(),
            description: "Priority-aware weighted round-robin with cache optimization and affinity constraints"
                .to_string(),
        },
        queue_depth: QueueDepth {
            pending,
            scheduled,
            running,
        },
        throughput: SchedulingThroughput {
            total_scheduled,
            total_completed,
            total_failed,
            success_rate,
            avg_restart_count: avg_restart,
        },
        node_utilization,
        recent_decisions: recent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::AgentStatus;
    use crate::models::node::{Node, NodeStatus, ResourceCapacity};
    use axum::extract::State;
    use std::collections::HashMap;

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        // Seed 2 default nodes matching AppState::new()
        let mut nodes = std::collections::HashMap::new();
        nodes.insert(
            "node-1".to_string(),
            Node {
                id: "node-1".to_string(),
                name: "node-1".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                allocatable: ResourceCapacity {
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
            Node {
                id: "node-2".to_string(),
                name: "node-2".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                allocatable: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
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
    async fn test_scheduler_status_empty() {
        let state = test_state().await;
        let result = scheduler_status(State(state)).await;
        assert_eq!(result.queue_depth.pending, 0);
        assert_eq!(result.queue_depth.running, 0);
        assert_eq!(result.throughput.success_rate, 100.0);
        assert_eq!(result.current_algorithm.name, "Weighted Round Robin");
        assert_eq!(result.node_utilization.len(), 2); // 2 default nodes
    }

    #[tokio::test]
    async fn test_scheduler_status_with_agents() {
        use crate::models::agent::{Agent, AgentSpec};

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // Create agents in different states
        for (name, status) in [
            ("pending-agent", AgentStatus::Pending),
            ("running-agent", AgentStatus::Running),
            ("succeeded-agent", AgentStatus::Succeeded),
            ("failed-agent", AgentStatus::Failed),
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = status;
            agents.insert(agent.id.clone(), agent);
        }

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(kias_knowledge::retriever::KeywordRetriever::new(
                kias_knowledge::graph::KnowledgeGraph::new(),
            )),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.queue_depth.pending, 1);
        assert_eq!(result.queue_depth.running, 1);
        assert_eq!(result.throughput.total_scheduled, 4);
        assert_eq!(result.throughput.total_completed, 1);
        assert_eq!(result.throughput.total_failed, 1);
        assert_eq!(result.recent_decisions.len(), 4);
    }

    #[tokio::test]
    async fn test_node_utilization() {
        let state = test_state().await;
        let result = scheduler_status(State(state)).await;
        // Default state has 2 nodes
        assert_eq!(result.node_utilization.len(), 2);
        for nu in &result.node_utilization {
            assert_eq!(nu.agent_count, 0);
            assert_eq!(nu.running_count, 0);
        }
    }

    #[tokio::test]
    async fn test_agents_assigned_to_nodes() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // Create 3 agents assigned to node-1
        for i in 0..3 {
            let spec = AgentSpec {
                name: format!("agent-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            agent.node_id = Some("node-1".to_string());
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let mut nodes = HashMap::new();
        nodes.insert(
            "node-1".to_string(),
            crate::models::node::Node {
                id: "node-1".to_string(),
                name: "node-1".to_string(),
                status: crate::models::node::NodeStatus::Ready,
                resources: crate::models::node::ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                allocatable: crate::models::node::ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.node_utilization.len(), 1);
        assert_eq!(result.node_utilization[0].agent_count, 3);
        assert_eq!(result.node_utilization[0].running_count, 3);
    }

    #[tokio::test]
    async fn test_recent_decisions_truncated() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // Create 15 agents — recent_decisions should truncate to 10
        for i in 0..15 {
            let spec = AgentSpec {
                name: format!("agent-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            // Set different timestamps for sorting
            agent.updated_at = format!("2026-05-20T{:02}:00:00Z", i);
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.recent_decisions.len(), 10);
        // Should be sorted descending (most recent first)
        assert!(result.recent_decisions[0].timestamp >= result.recent_decisions[1].timestamp);
    }

    #[tokio::test]
    async fn test_success_rate_edge_cases() {
        use crate::models::agent::{Agent, AgentSpec};

        // All failed — success_rate should be 0
        let mut agents = HashMap::new();
        for i in 0..3 {
            let spec = AgentSpec {
                name: format!("agent-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Failed;
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.throughput.success_rate, 0.0);
        assert_eq!(result.throughput.total_failed, 3);
        assert_eq!(result.throughput.total_completed, 0);
    }

    #[tokio::test]
    async fn test_algorithm_name_and_description() {
        let state = test_state().await;
        let result = scheduler_status(State(state)).await;
        assert_eq!(result.current_algorithm.name, "Weighted Round Robin");
        assert!(!result.current_algorithm.description.is_empty());
        assert!(result.current_algorithm.description.contains("weighted"));
    }

    #[tokio::test]
    async fn test_avg_restart_count_nonzero() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // Agent with restart_count = 3
        let spec = AgentSpec {
            name: "restart-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Running;
        agent.restart_count = 3;
        agents.insert(agent.id.clone(), agent);

        // Agent with restart_count = 0
        let spec2 = AgentSpec {
            name: "fresh-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let agent2 = Agent::from_spec(spec2);
        agents.insert(agent2.id.clone(), agent2);

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        // avg_restart = (3 + 0) / 2 = 1.5
        assert_eq!(result.throughput.avg_restart_count, 1.5);
    }

    #[tokio::test]
    async fn test_scheduled_status_agents() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        let spec = AgentSpec {
            name: "scheduled-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "high".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Scheduled;
        agents.insert(agent.id.clone(), agent);

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.queue_depth.scheduled, 1);
        assert_eq!(result.queue_depth.pending, 0);
        assert_eq!(result.queue_depth.running, 0);
        assert_eq!(result.recent_decisions[0].priority, "high");
    }

    #[tokio::test]
    async fn test_mixed_success_rate() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // 2 succeeded, 1 failed, 1 running → success_rate = 2/4 * 100 = 50%
        for (name, status) in [
            ("ok-1", AgentStatus::Succeeded),
            ("ok-2", AgentStatus::Succeeded),
            ("fail-1", AgentStatus::Failed),
            ("run-1", AgentStatus::Running),
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = status;
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.throughput.success_rate, 50.0);
        assert_eq!(result.throughput.total_completed, 2);
        assert_eq!(result.throughput.total_failed, 1);
        assert_eq!(result.throughput.total_scheduled, 4);
    }

    #[tokio::test]
    async fn test_agents_spread_across_nodes() {
        use crate::models::agent::{Agent, AgentSpec};
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let mut agents = HashMap::new();
        // 2 agents on node-1, 1 agent on node-2
        for (i, node_id) in [(0, "node-1"), (1, "node-1"), (2, "node-2")] {
            let spec = AgentSpec {
                name: format!("agent-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            agent.node_id = Some(node_id.to_string());
            agents.insert(agent.id.clone(), agent);
        }

        let mut nodes = HashMap::new();
        nodes.insert(
            "node-1".to_string(),
            Node {
                id: "node-1".to_string(),
                name: "node-1".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                allocatable: ResourceCapacity {
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
            Node {
                id: "node-2".to_string(),
                name: "node-2".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                allocatable: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.node_utilization.len(), 2);
        let n1 = result
            .node_utilization
            .iter()
            .find(|n| n.node_id == "node-1")
            .unwrap();
        let n2 = result
            .node_utilization
            .iter()
            .find(|n| n.node_id == "node-2")
            .unwrap();
        assert_eq!(n1.agent_count, 2);
        assert_eq!(n1.running_count, 2);
        assert_eq!(n2.agent_count, 1);
        assert_eq!(n2.running_count, 1);
    }

    #[tokio::test]
    async fn test_single_agent_boundary() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        let spec = AgentSpec {
            name: "only-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "critical".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Running;
        agent.restart_count = 5;
        agents.insert(agent.id.clone(), agent);

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.throughput.total_scheduled, 1);
        assert_eq!(result.throughput.avg_restart_count, 5.0);
        assert_eq!(result.recent_decisions.len(), 1);
        assert_eq!(result.recent_decisions[0].priority, "critical");
    }

    #[tokio::test]
    async fn test_recent_decisions_sorted_descending() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // Create 3 agents with specific timestamps
        let timestamps = [
            "2026-05-20T10:00:00Z",
            "2026-05-20T14:00:00Z",
            "2026-05-20T12:00:00Z",
        ];
        for (i, ts) in timestamps.iter().enumerate() {
            let spec = AgentSpec {
                name: format!("agent-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            agent.updated_at = ts.to_string();
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.recent_decisions.len(), 3);
        // Should be sorted descending: 14:00 > 12:00 > 10:00
        assert_eq!(result.recent_decisions[0].timestamp, "2026-05-20T14:00:00Z");
        assert_eq!(result.recent_decisions[1].timestamp, "2026-05-20T12:00:00Z");
        assert_eq!(result.recent_decisions[2].timestamp, "2026-05-20T10:00:00Z");
    }

    #[tokio::test]
    async fn test_decision_fields_populated() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        let spec = AgentSpec {
            name: "my-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "high".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Running;
        agent.node_id = Some("node-1".to_string());
        agent.updated_at = "2026-05-20T12:00:00Z".to_string();
        let agent_id = agent.id.clone();
        agents.insert(agent_id.clone(), agent);

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        let decision = &result.recent_decisions[0];
        assert_eq!(decision.agent_id, agent_id);
        assert_eq!(decision.agent_name, "my-agent");
        assert_eq!(decision.assigned_node, Some("node-1".to_string()));
        assert!(decision.status.contains("Running"));
        assert_eq!(decision.priority, "high");
        assert_eq!(decision.timestamp, "2026-05-20T12:00:00Z");
    }

    // === Serialization roundtrip tests ===

    #[test]
    fn test_scheduler_algorithm_serialize() {
        let algo = SchedulerAlgorithm {
            name: "Round Robin".to_string(),
            description: "Simple round-robin".to_string(),
        };
        let json = serde_json::to_string(&algo).unwrap();
        assert!(json.contains("Round Robin"));
        assert!(json.contains("Simple round-robin"));
        // Verify field names are camelCase (default serde)
        assert!(json.contains("name"));
        assert!(json.contains("description"));
    }

    #[test]
    fn test_queue_depth_serialize() {
        let qd = QueueDepth {
            pending: 5,
            scheduled: 3,
            running: 10,
        };
        let json = serde_json::to_string(&qd).unwrap();
        assert!(json.contains("pending"));
        assert!(json.contains("scheduled"));
        assert!(json.contains("running"));
        assert!(json.contains(":5"));
        assert!(json.contains(":3"));
        assert!(json.contains(":10"));
    }

    #[test]
    fn test_scheduling_throughput_serialize() {
        let tp = SchedulingThroughput {
            total_scheduled: 100,
            total_completed: 80,
            total_failed: 10,
            success_rate: 80.0,
            avg_restart_count: 1.5,
        };
        let json = serde_json::to_string(&tp).unwrap();
        assert!(json.contains("total_scheduled"));
        assert!(json.contains("total_completed"));
        assert!(json.contains("total_failed"));
        assert!(json.contains("success_rate"));
        assert!(json.contains("avg_restart_count"));
        assert!(json.contains("80.0"));
        assert!(json.contains("1.5"));
    }

    #[test]
    fn test_scheduling_decision_serialize() {
        let decision = SchedulingDecision {
            agent_id: "agent-123".to_string(),
            agent_name: "my-agent".to_string(),
            assigned_node: Some("node-1".to_string()),
            status: "Running".to_string(),
            priority: "high".to_string(),
            timestamp: "2026-05-20T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("agent_id"));
        assert!(json.contains("agent_name"));
        assert!(json.contains("assigned_node"));
        assert!(json.contains("agent-123"));
        assert!(json.contains("my-agent"));
        assert!(json.contains("node-1"));
    }

    #[test]
    fn test_scheduling_decision_none_node_serialize() {
        let decision = SchedulingDecision {
            agent_id: "agent-456".to_string(),
            agent_name: "unassigned-agent".to_string(),
            assigned_node: None,
            status: "Pending".to_string(),
            priority: "low".to_string(),
            timestamp: "2026-05-20T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        // None should serialize as null
        assert!(json.contains("null"));
        assert!(json.contains("unassigned-agent"));
    }

    #[test]
    fn test_node_utilization_serialize() {
        let nu = NodeUtilization {
            node_id: "node-1".to_string(),
            node_name: "node-1".to_string(),
            agent_count: 5,
            running_count: 3,
            status: "Ready".to_string(),
        };
        let json = serde_json::to_string(&nu).unwrap();
        assert!(json.contains("node_id"));
        assert!(json.contains("node_name"));
        assert!(json.contains("agent_count"));
        assert!(json.contains("running_count"));
        assert!(json.contains("status"));
        assert!(json.contains(":5"));
        assert!(json.contains(":3"));
    }

    #[test]
    fn test_scheduler_status_serialize_full() {
        let status = SchedulerStatus {
            current_algorithm: SchedulerAlgorithm {
                name: "WRR".to_string(),
                description: "desc".to_string(),
            },
            queue_depth: QueueDepth {
                pending: 1,
                scheduled: 2,
                running: 3,
            },
            throughput: SchedulingThroughput {
                total_scheduled: 10,
                total_completed: 5,
                total_failed: 2,
                success_rate: 50.0,
                avg_restart_count: 0.5,
            },
            node_utilization: vec![NodeUtilization {
                node_id: "n1".to_string(),
                node_name: "n1".to_string(),
                agent_count: 3,
                running_count: 2,
                status: "Ready".to_string(),
            }],
            recent_decisions: vec![SchedulingDecision {
                agent_id: "a1".to_string(),
                agent_name: "agent".to_string(),
                assigned_node: Some("n1".to_string()),
                status: "Running".to_string(),
                priority: "medium".to_string(),
                timestamp: "2026-05-20T12:00:00Z".to_string(),
            }],
        };
        let json = serde_json::to_string(&status).unwrap();
        // All top-level fields present
        assert!(json.contains("current_algorithm"));
        assert!(json.contains("queue_depth"));
        assert!(json.contains("throughput"));
        assert!(json.contains("node_utilization"));
        assert!(json.contains("recent_decisions"));
    }

    // === Edge case tests ===

    #[tokio::test]
    async fn test_unassigned_agents_not_in_node_utilization() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        // 2 agents with node_id = None (unassigned)
        for i in 0..2 {
            let spec = AgentSpec {
                name: format!("unassigned-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Pending;
            // node_id is None by default
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        // 2 pending agents counted in queue_depth
        assert_eq!(result.queue_depth.pending, 2);
        // But no nodes → no node_utilization entries
        assert_eq!(result.node_utilization.len(), 0);
        // Decisions still include unassigned agents
        assert_eq!(result.recent_decisions.len(), 2);
        for d in &result.recent_decisions {
            assert_eq!(d.assigned_node, None);
        }
    }

    #[tokio::test]
    async fn test_mixed_statuses_on_same_node() {
        use crate::models::agent::{Agent, AgentSpec};
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let mut agents = HashMap::new();
        // 4 agents on node-1: 2 running, 1 failed, 1 pending
        for (name, status) in [
            ("run-1", AgentStatus::Running),
            ("run-2", AgentStatus::Running),
            ("fail-1", AgentStatus::Failed),
            ("pend-1", AgentStatus::Pending),
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = status;
            agent.node_id = Some("node-1".to_string());
            agents.insert(agent.id.clone(), agent);
        }

        let mut nodes = HashMap::new();
        nodes.insert(
            "node-1".to_string(),
            Node {
                id: "node-1".to_string(),
                name: "node-1".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                allocatable: ResourceCapacity {
                    cpu: "8".to_string(),
                    memory: "16Gi".to_string(),
                    gpu: "1".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.node_utilization.len(), 1);
        let nu = &result.node_utilization[0];
        assert_eq!(nu.agent_count, 4); // all 4 assigned to node-1
        assert_eq!(nu.running_count, 2); // only 2 are Running
        assert_eq!(result.queue_depth.pending, 1);
        assert_eq!(result.queue_depth.running, 2);
        assert_eq!(result.throughput.total_failed, 1);
    }

    #[tokio::test]
    async fn test_all_succeeded_100_percent() {
        use crate::models::agent::{Agent, AgentSpec};

        let mut agents = HashMap::new();
        for i in 0..5 {
            let spec = AgentSpec {
                name: format!("ok-{}", i),
                image: "python:3.11".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Succeeded;
            agents.insert(agent.id.clone(), agent);
        }

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(agents)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.throughput.success_rate, 100.0);
        assert_eq!(result.throughput.total_completed, 5);
        assert_eq!(result.throughput.total_failed, 0);
        assert_eq!(result.queue_depth.pending, 0);
        assert_eq!(result.queue_depth.scheduled, 0);
        assert_eq!(result.queue_depth.running, 0);
    }

    #[tokio::test]
    async fn test_node_status_format_in_utilization() {
        use crate::models::node::{Node, NodeStatus, ResourceCapacity};

        let mut nodes = HashMap::new();
        nodes.insert(
            "node-ready".to_string(),
            Node {
                id: "node-ready".to_string(),
                name: "node-ready".to_string(),
                status: NodeStatus::Ready,
                resources: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                allocatable: ResourceCapacity {
                    cpu: "4".to_string(),
                    memory: "8Gi".to_string(),
                    gpu: "0".to_string(),
                },
                labels: Default::default(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            },
        );

        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        let state = AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(HashMap::new())),
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
        };

        let result = scheduler_status(State(state)).await;
        assert_eq!(result.node_utilization.len(), 1);
        // Status is Debug-formatted: "Ready"
        assert_eq!(result.node_utilization[0].status, "Ready");
    }

    use std::sync::Arc;
    use tokio::sync::RwLock;
}
