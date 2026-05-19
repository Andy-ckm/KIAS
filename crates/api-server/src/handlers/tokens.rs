use axum::extract::State;
use axum::Json;
use chrono::Timelike;
use serde::Serialize;

use crate::models::agent::AgentStatus;
use crate::AppState;

/// Token usage record for a single agent
#[derive(Debug, Serialize)]
pub struct TokenUsage {
    pub agent_id: String,
    pub agent_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost: f64,
    pub request_count: u64,
}

/// Time-series data point for token usage over time
#[derive(Debug, Serialize)]
pub struct TokenTimeSeries {
    pub timestamp: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Aggregate token analytics response
#[derive(Debug, Serialize)]
pub struct TokenAnalytics {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub total_requests: u64,
    pub per_agent: Vec<TokenUsage>,
    pub time_series: Vec<TokenTimeSeries>,
}

/// GET /api/v1/tokens
/// Returns token usage analytics aggregated across all agents.
/// Generates simulated time-series data based on current agent state.
pub async fn token_analytics(State(state): State<AppState>) -> Json<TokenAnalytics> {
    let agents = state.agents.read().await;

    // Generate per-agent token usage from agent state
    let mut per_agent = Vec::new();
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_requests: u64 = 0;

    for agent in agents.values() {
        // Simulate token usage based on agent status and restart count
        let base_tokens: u64 = match agent.status {
            AgentStatus::Running => 15_000,
            AgentStatus::Succeeded => 45_000,
            AgentStatus::Failed => 8_000,
            AgentStatus::Scheduled => 2_000,
            AgentStatus::Pending => 0,
            AgentStatus::Unknown => 1_000,
        };
        let restart_multiplier = 1 + agent.restart_count as u64;
        let input = base_tokens * 3 * restart_multiplier / 4; // ~75% input
        let output = base_tokens * restart_multiplier / 4; // ~25% output
        let total = input + output;
        let requests = base_tokens / 500 * restart_multiplier; // ~500 tokens per request
        let cost = total as f64 * 0.000003; // $3 per 1M tokens

        total_input += input;
        total_output += output;
        total_requests += requests;

        per_agent.push(TokenUsage {
            agent_id: agent.id.clone(),
            agent_name: agent.spec.name.clone(),
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
            estimated_cost: cost,
            request_count: requests,
        });
    }

    // Sort by total tokens descending
    per_agent.sort_by_key(|b| std::cmp::Reverse(b.total_tokens));

    // Generate time-series data (last 24 hours, hourly buckets)
    let now = chrono::Utc::now();
    let agent_count = agents.len().max(1) as u64;
    let time_series: Vec<TokenTimeSeries> = (0..24)
        .map(|h| {
            let ts = now - chrono::Duration::hours(23 - h);
            // Simulate varying token usage with peak hours
            let hour = ts.hour();
            let multiplier = if (9..17).contains(&hour) {
                3.0 // Business hours peak
            } else if (0..6).contains(&hour) {
                0.3 // Night low
            } else {
                1.5 // Moderate
            };
            let base = (agent_count * 800) as f64 * multiplier;
            let input = base as u64 * 3 / 4;
            let output = base as u64 / 4;
            TokenTimeSeries {
                timestamp: ts.format("%H:%M").to_string(),
                input_tokens: input,
                output_tokens: output,
                total_tokens: input + output,
            }
        })
        .collect();

    let total_tokens = total_input + total_output;
    let total_cost = total_tokens as f64 * 0.000003;

    Json(TokenAnalytics {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_tokens,
        total_cost,
        total_requests,
        per_agent,
        time_series,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn test_state() -> AppState {
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
    async fn test_token_analytics_empty() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.total_requests, 0);
        assert!(result.per_agent.is_empty());
        assert_eq!(result.time_series.len(), 24);
    }

    #[tokio::test]
    async fn test_token_analytics_with_agents() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // Add a running agent
        let spec = AgentSpec {
            name: "test-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string()],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = crate::models::agent::AgentStatus::Running;
        agents.insert(agent.id.clone(), agent);

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

        let result = token_analytics(State(state)).await;
        assert!(result.total_tokens > 0);
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].agent_name, "test-agent");
        assert!(result.per_agent[0].input_tokens > 0);
        assert!(result.per_agent[0].output_tokens > 0);
    }

    #[tokio::test]
    async fn test_token_time_series_has_24_hours() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        assert_eq!(result.time_series.len(), 24);
        // All entries should have non-negative values
        for ts in &result.time_series {
            // total_tokens is u64, always >= 0; verify it's reasonable
            let _ = ts.total_tokens;
        }
    }

    #[tokio::test]
    async fn test_token_per_agent_sorted_by_total() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // Create agents with different statuses (different base tokens)
        for (name, status) in [
            ("low-agent", AgentStatus::Pending),    // 0 tokens
            ("mid-agent", AgentStatus::Scheduled),  // 2000 base
            ("high-agent", AgentStatus::Succeeded), // 45000 base
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

        let result = token_analytics(State(state)).await;
        assert_eq!(result.per_agent.len(), 3);
        // Should be sorted descending by total_tokens
        assert!(result.per_agent[0].total_tokens >= result.per_agent[1].total_tokens);
        assert!(result.per_agent[1].total_tokens >= result.per_agent[2].total_tokens);
        // Highest should be "high-agent" (Succeeded status)
        assert_eq!(result.per_agent[0].agent_name, "high-agent");
    }

    #[tokio::test]
    async fn test_token_cost_calculation() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "cost-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Running;
        agents.insert(agent.id.clone(), agent);

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

        let result = token_analytics(State(state)).await;
        // Running agent: base=15000, restart_multiplier=1
        // input = 15000 * 3 * 1 / 4 = 11250
        // output = 15000 * 1 / 4 = 3750
        // total = 15000
        // cost = 15000 * 0.000003 = 0.045
        assert_eq!(result.per_agent.len(), 1);
        let usage = &result.per_agent[0];
        assert_eq!(usage.input_tokens, 11250);
        assert_eq!(usage.output_tokens, 3750);
        assert_eq!(usage.total_tokens, 15000);
        assert!((usage.estimated_cost - 0.045).abs() < 0.0001);
        assert_eq!(result.total_cost, usage.estimated_cost);
    }

    #[tokio::test]
    async fn test_token_restart_multiplier() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

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
        agent.restart_count = 3; // 3 restarts → multiplier = 4
        agents.insert(agent.id.clone(), agent);

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

        let result = token_analytics(State(state)).await;
        // Running agent with 3 restarts: base=15000, multiplier=4
        // input = 15000 * 3 * 4 / 4 = 45000
        // output = 15000 * 4 / 4 = 15000
        // total = 60000
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].total_tokens, 60000);
        // With restarts, tokens should be 4x base
        assert_eq!(result.per_agent[0].input_tokens, 45000);
        assert_eq!(result.per_agent[0].output_tokens, 15000);
    }

    #[tokio::test]
    async fn test_token_pending_agent_zero_tokens() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "pending-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Pending;
        agents.insert(agent.id.clone(), agent);

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

        let result = token_analytics(State(state)).await;
        // Pending agent: base=0, so everything is 0
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].total_tokens, 0);
        assert_eq!(result.per_agent[0].input_tokens, 0);
        assert_eq!(result.per_agent[0].output_tokens, 0);
        assert_eq!(result.per_agent[0].estimated_cost, 0.0);
    }
}
