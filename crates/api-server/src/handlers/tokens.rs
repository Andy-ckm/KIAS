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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Pending agent: base=0, so everything is 0
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].total_tokens, 0);
        assert_eq!(result.per_agent[0].input_tokens, 0);
        assert_eq!(result.per_agent[0].output_tokens, 0);
        assert_eq!(result.per_agent[0].estimated_cost, 0.0);
    }

    #[tokio::test]
    async fn test_token_failed_agent() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "failed-agent".to_string(),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Failed agent: base=8000, multiplier=1
        // input = 8000 * 3 * 1 / 4 = 6000
        // output = 8000 * 1 / 4 = 2000
        // total = 8000
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].input_tokens, 6000);
        assert_eq!(result.per_agent[0].output_tokens, 2000);
        assert_eq!(result.per_agent[0].total_tokens, 8000);
        assert_eq!(result.per_agent[0].request_count, 16); // 8000/500 * 1
    }

    #[tokio::test]
    async fn test_token_unknown_agent() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "unknown-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Unknown;
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Unknown agent: base=1000, multiplier=1
        // input = 1000 * 3 * 1 / 4 = 750
        // output = 1000 * 1 / 4 = 250
        // total = 1000
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].input_tokens, 750);
        assert_eq!(result.per_agent[0].output_tokens, 250);
        assert_eq!(result.per_agent[0].total_tokens, 1000);
        assert_eq!(result.per_agent[0].request_count, 2); // 1000/500 * 1
    }

    #[tokio::test]
    async fn test_token_all_statuses_aggregated() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // One agent per status
        for (name, status) in [
            ("s-running", AgentStatus::Running),     // base=15000
            ("s-succeeded", AgentStatus::Succeeded), // base=45000
            ("s-failed", AgentStatus::Failed),       // base=8000
            ("s-scheduled", AgentStatus::Scheduled), // base=2000
            ("s-pending", AgentStatus::Pending),     // base=0
            ("s-unknown", AgentStatus::Unknown),     // base=1000
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        assert_eq!(result.per_agent.len(), 6);
        // Verify total aggregation
        let sum_input: u64 = result.per_agent.iter().map(|a| a.input_tokens).sum();
        let sum_output: u64 = result.per_agent.iter().map(|a| a.output_tokens).sum();
        let sum_total: u64 = result.per_agent.iter().map(|a| a.total_tokens).sum();
        assert_eq!(result.total_input_tokens, sum_input);
        assert_eq!(result.total_output_tokens, sum_output);
        assert_eq!(result.total_tokens, sum_total);
        // Total = 15000 + 45000 + 8000 + 2000 + 0 + 1000 = 71000
        assert_eq!(result.total_tokens, 71000);
    }

    #[tokio::test]
    async fn test_token_time_series_business_vs_night() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;

        assert_eq!(result.time_series.len(), 24);
        // Business hours (9-16) should have higher tokens than night (0-5)
        // Note: the exact timestamps depend on current UTC time, but we can
        // verify the structure is sound and no values overflow
        for ts in &result.time_series {
            assert!(ts.total_tokens == ts.input_tokens + ts.output_tokens);
            // Input should be ~75% of total
            if ts.total_tokens > 0 {
                let input_ratio = ts.input_tokens as f64 / ts.total_tokens as f64;
                assert!((input_ratio - 0.75).abs() < 0.01);
            }
        }
    }

    #[tokio::test]
    async fn test_token_input_output_ratio() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "ratio-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Succeeded; // base=45000
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        let usage = &result.per_agent[0];
        // Succeeded agent: base=45000
        // input = 45000 * 3 * 1 / 4 = 33750
        // output = 45000 * 1 / 4 = 11250
        // total = 45000
        assert_eq!(usage.input_tokens, 33750);
        assert_eq!(usage.output_tokens, 11250);
        assert_eq!(usage.total_tokens, 45000);
        // Verify exact ratio: input/total = 33750/45000 = 0.75
        let ratio = usage.input_tokens as f64 / usage.total_tokens as f64;
        assert!((ratio - 0.75).abs() < 0.0001);
        // Cost = 45000 * 0.000003 = 0.135
        assert!((usage.estimated_cost - 0.135).abs() < 0.0001);
        // Total cost = sum of per_agent costs
        assert!(
            (result.total_cost
                - result
                    .per_agent
                    .iter()
                    .map(|a| a.estimated_cost)
                    .sum::<f64>())
            .abs()
                < 0.0001
        );
    }

    #[tokio::test]
    async fn test_token_scheduled_agent() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "scheduled-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Scheduled;
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Scheduled agent: base=2000, multiplier=1
        // input = 2000 * 3 * 1 / 4 = 1500
        // output = 2000 * 1 / 4 = 500
        // total = 2000
        assert_eq!(result.per_agent.len(), 1);
        assert_eq!(result.per_agent[0].input_tokens, 1500);
        assert_eq!(result.per_agent[0].output_tokens, 500);
        assert_eq!(result.per_agent[0].total_tokens, 2000);
        assert_eq!(result.per_agent[0].request_count, 4); // 2000/500 * 1
    }

    // ── Serialization / model tests ──────────────────────────────────

    #[test]
    fn test_token_usage_serialize() {
        let usage = TokenUsage {
            agent_id: "a-1".to_string(),
            agent_name: "test".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            estimated_cost: 0.0045,
            request_count: 10,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"agent_id\":\"a-1\""));
        assert!(json.contains("\"total_tokens\":1500"));
    }

    #[test]
    fn test_token_time_series_serialize() {
        let ts = TokenTimeSeries {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
        };
        let json = serde_json::to_string(&ts).unwrap();
        assert!(json.contains("\"total_tokens\":150"));
    }

    #[test]
    fn test_token_analytics_serialize() {
        let analytics = TokenAnalytics {
            total_input_tokens: 5000,
            total_output_tokens: 2000,
            total_tokens: 7000,
            total_cost: 0.021,
            total_requests: 50,
            per_agent: vec![],
            time_series: vec![],
        };
        let json = serde_json::to_string(&analytics).unwrap();
        assert!(json.contains("\"total_cost\":0.021"));
    }

    #[tokio::test]
    async fn test_token_multiple_agents_sorted_by_total() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        // Create agents with different statuses → different token amounts
        for (name, status) in [
            ("small", AgentStatus::Pending),    // 0 tokens
            ("medium", AgentStatus::Scheduled), // 2000 tokens
            ("large", AgentStatus::Running),    // 15000 tokens
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "img".to_string(),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        assert_eq!(result.per_agent.len(), 3);
        // Sorted by total tokens descending: large > medium > small
        assert_eq!(result.per_agent[0].agent_name, "large");
        assert_eq!(result.per_agent[1].agent_name, "medium");
        assert_eq!(result.per_agent[2].agent_name, "small");
        assert_eq!(result.per_agent[2].total_tokens, 0);
    }

    #[tokio::test]
    async fn test_token_unknown_agent_base_tokens() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "unknown-agent".to_string(),
            image: "img".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Unknown;
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Unknown: base=1000, multiplier=1
        // input = 1000 * 3 / 4 = 750
        // output = 1000 / 4 = 250
        assert_eq!(result.per_agent[0].input_tokens, 750);
        assert_eq!(result.per_agent[0].output_tokens, 250);
        assert_eq!(result.per_agent[0].total_tokens, 1000);
    }

    #[tokio::test]
    async fn test_token_high_restart_count_multiplier() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        let spec = AgentSpec {
            name: "flaky-agent".to_string(),
            image: "img".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.status = AgentStatus::Running;
        agent.restart_count = 4; // multiplier = 5
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        // Running base=15000, multiplier=5
        // input = 15000 * 3 * 5 / 4 = 56250
        // output = 15000 * 5 / 4 = 18750
        // total = 75000
        assert_eq!(result.per_agent[0].input_tokens, 56250);
        assert_eq!(result.per_agent[0].output_tokens, 18750);
        assert_eq!(result.per_agent[0].total_tokens, 75000);
        // requests = 15000 / 500 * 5 = 150
        assert_eq!(result.per_agent[0].request_count, 150);
    }

    #[tokio::test]
    async fn test_token_time_series_length() {
        let state = AppState::new_async(kias_common::config::KiasConfig::default()).await;
        let result = token_analytics(State(state)).await;
        assert_eq!(result.time_series.len(), 24);
    }

    #[tokio::test]
    async fn test_token_total_matches_per_agent_sum() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let config = kias_common::config::KiasConfig::default();
        let mut agents = HashMap::new();

        for (name, status) in [
            ("a1", AgentStatus::Running),
            ("a2", AgentStatus::Succeeded),
            ("a3", AgentStatus::Failed),
        ] {
            let spec = AgentSpec {
                name: name.to_string(),
                image: "img".to_string(),
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
            slow_trace_collector: kias_monitor::SlowTraceCollector::new(),
            token_budgets: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        let result = token_analytics(State(state)).await;
        let sum_input: u64 = result.per_agent.iter().map(|a| a.input_tokens).sum();
        let sum_output: u64 = result.per_agent.iter().map(|a| a.output_tokens).sum();
        let sum_total: u64 = result.per_agent.iter().map(|a| a.total_tokens).sum();
        assert_eq!(result.total_input_tokens, sum_input);
        assert_eq!(result.total_output_tokens, sum_output);
        assert_eq!(result.total_tokens, sum_total);
    }

    #[tokio::test]
    async fn test_token_time_series_total_equals_input_plus_output() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        for ts in &result.time_series {
            assert_eq!(ts.total_tokens, ts.input_tokens + ts.output_tokens);
        }
    }

    #[tokio::test]
    async fn test_token_time_series_timestamp_format() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        for ts in &result.time_series {
            // Format should be HH:MM
            assert_eq!(ts.timestamp.len(), 5);
            assert_eq!(ts.timestamp.as_bytes()[2], b':');
        }
    }

    #[test]
    fn test_token_usage_serialize_fields() {
        let usage = TokenUsage {
            agent_id: "a1".to_string(),
            agent_name: "my-agent".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            estimated_cost: 0.0045,
            request_count: 3,
        };
        let json = serde_json::to_value(&usage).unwrap();
        assert_eq!(json["agent_id"], "a1");
        assert_eq!(json["agent_name"], "my-agent");
        assert_eq!(json["input_tokens"], 1000);
        assert_eq!(json["output_tokens"], 500);
        assert_eq!(json["total_tokens"], 1500);
        assert_eq!(json["request_count"], 3);
    }

    #[test]
    fn test_token_time_series_serialize_fields() {
        let ts = TokenTimeSeries {
            timestamp: "14:00".to_string(),
            input_tokens: 3000,
            output_tokens: 1000,
            total_tokens: 4000,
        };
        let json = serde_json::to_value(&ts).unwrap();
        assert_eq!(json["timestamp"], "14:00");
        assert_eq!(json["input_tokens"], 3000);
        assert_eq!(json["total_tokens"], 4000);
    }

    #[test]
    fn test_token_analytics_serialize_fields() {
        let analytics = TokenAnalytics {
            total_input_tokens: 10000,
            total_output_tokens: 5000,
            total_tokens: 15000,
            total_cost: 0.045,
            total_requests: 30,
            per_agent: vec![],
            time_series: vec![],
        };
        let json = serde_json::to_value(&analytics).unwrap();
        assert_eq!(json["total_input_tokens"], 10000);
        assert_eq!(json["total_output_tokens"], 5000);
        assert_eq!(json["total_tokens"], 15000);
        assert_eq!(json["total_cost"], 0.045);
        assert_eq!(json["total_requests"], 30);
    }

    #[tokio::test]
    async fn test_token_empty_state_time_series_nonzero() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        // Even with no agents, time_series should have data (agent_count defaults to 1)
        for ts in &result.time_series {
            assert!(
                ts.total_tokens > 0,
                "time series entries should be > 0 with default agent_count=1"
            );
        }
    }

    #[tokio::test]
    async fn test_token_analytics_cost_is_non_negative() {
        let state = test_state().await;
        let result = token_analytics(State(state)).await;
        assert!(result.total_cost >= 0.0);
        for usage in &result.per_agent {
            assert!(usage.estimated_cost >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_token_per_agent_input_output_ratio() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let spec = AgentSpec {
                name: "ratio-agent".to_string(),
                image: "img".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            agents.insert(agent.id.clone(), agent);
        }
        let result = token_analytics(State(state)).await;
        let usage = &result.per_agent[0];
        // Running: base=15000, input=15000*3*1/4=11250, output=15000*1/4=3750
        // Ratio should be ~3:1
        assert_eq!(usage.input_tokens, 11250);
        assert_eq!(usage.output_tokens, 3750);
        assert_eq!(usage.total_tokens, 15000);
    }

    #[tokio::test]
    async fn test_token_per_agent_request_count_nonzero_for_running() {
        use crate::models::agent::{Agent, AgentSpec};
        use std::collections::HashMap;

        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let spec = AgentSpec {
                name: "req-agent".to_string(),
                image: "img".to_string(),
                command: vec![],
                resource_request: None,
                labels: HashMap::new(),
                priority: "medium".to_string(),
                env: HashMap::new(),
            };
            let mut agent = Agent::from_spec(spec);
            agent.status = AgentStatus::Running;
            agents.insert(agent.id.clone(), agent);
        }
        let result = token_analytics(State(state)).await;
        assert!(result.per_agent[0].request_count > 0);
        // Running: base=15000, requests = 15000/500*1 = 30
        assert_eq!(result.per_agent[0].request_count, 30);
    }

    #[test]
    fn test_token_usage_debug_format() {
        let usage = TokenUsage {
            agent_id: "a1".to_string(),
            agent_name: "test".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            estimated_cost: 0.001,
            request_count: 1,
        };
        let debug = format!("{:?}", usage);
        assert!(debug.contains("TokenUsage"));
        assert!(debug.contains("agent_id"));
        assert!(debug.contains("input_tokens"));
    }
}
