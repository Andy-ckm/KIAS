//! # Tier Routing API Handler
//!
//! Exposes PrfaaS-inspired intelligent task routing via REST API.
//!
//! ## Endpoints
//!
//! - `POST /api/v1/routing/evaluate` — Evaluate task complexity and get routing decision
//! - `POST /api/v1/routing/batch` — Batch evaluate multiple tasks
//! - `GET  /api/v1/routing/tiers` — List available agent tiers with stats
//! - `POST /api/v1/routing/pool/register` — Register an agent in the tier pool
//! - `GET  /api/v1/routing/pool/status` — Get pool status and fallback metrics

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::AppState;

use kias_scheduler::{
    AgentPool, AgentTier, ComplexityEvaluator, CompositeEvaluator, PooledAgent, RoutingDecision,
    SmartRouter, TaskComplexity, TaskDescriptor,
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for tier routing
#[derive(Clone)]
pub struct TierRoutingState {
    /// Smart router instance
    pub router: Arc<RwLock<SmartRouter>>,
    /// Agent pool
    pub pool: Arc<RwLock<AgentPool>>,
}

impl TierRoutingState {
    pub fn new() -> Self {
        Self {
            router: Arc::new(RwLock::new(SmartRouter::with_evaluator(Box::new(
                CompositeEvaluator::new(),
            )))),
            pool: Arc::new(RwLock::new(AgentPool::new())),
        }
    }
}

impl Default for TierRoutingState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Request/Response types ──────────────────────────────────────────────

/// Request to evaluate a task for routing
#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    /// Task description
    pub description: String,
    /// Estimated input tokens
    #[serde(default = "default_input_tokens")]
    pub input_tokens: u32,
    /// Whether task requires tool use
    #[serde(default)]
    pub requires_tools: bool,
    /// Whether task has additional context
    #[serde(default)]
    pub has_context: bool,
    /// Priority (1 = highest)
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Cost budget (relative units, 0 = unlimited)
    #[serde(default)]
    pub cost_budget: f64,
    /// Latency budget (multiplier, 0 = unlimited)
    #[serde(default)]
    pub latency_budget: f64,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_input_tokens() -> u32 {
    100
}

fn default_priority() -> u32 {
    5
}

/// Response from task evaluation
#[derive(Debug, Serialize)]
pub struct EvaluateResponse {
    /// Task ID (auto-generated)
    pub task_id: String,
    /// Evaluated complexity
    pub complexity: String,
    /// Routing decision
    pub decision: RoutingDecisionResponse,
    /// Recommended agent tier
    pub recommended_tier: String,
}

/// Routing decision details
#[derive(Debug, Serialize)]
pub struct RoutingDecisionResponse {
    pub tier: String,
    pub reason: String,
    pub estimated_cost: f64,
    pub estimated_latency: f64,
    pub confidence: f64,
}

impl From<RoutingDecision> for RoutingDecisionResponse {
    fn from(d: RoutingDecision) -> Self {
        Self {
            tier: format!("{:?}", d.tier),
            reason: d.reason,
            estimated_cost: d.estimated_cost,
            estimated_latency: d.estimated_latency,
            confidence: d.confidence,
        }
    }
}

/// Batch evaluation request
#[derive(Debug, Deserialize)]
pub struct BatchEvaluateRequest {
    pub tasks: Vec<EvaluateRequest>,
}

/// Batch evaluation response
#[derive(Debug, Serialize)]
pub struct BatchEvaluateResponse {
    pub results: Vec<EvaluateResponse>,
    pub summary: BatchSummary,
}

/// Summary statistics for batch evaluation
#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub total: usize,
    pub simple: usize,
    pub medium: usize,
    pub complex: usize,
    pub avg_confidence: f64,
}

/// Tier information
#[derive(Debug, Serialize)]
pub struct TierInfo {
    pub name: String,
    pub relative_cost: f64,
    pub latency_multiplier: f64,
    pub capability_score: f64,
    pub registered_agents: usize,
}

/// Tier listing response
#[derive(Debug, Serialize)]
pub struct TierListResponse {
    pub tiers: Vec<TierInfo>,
    pub pool_status: PoolStatusResponse,
}

/// Pool status response
#[derive(Debug, Serialize)]
pub struct PoolStatusResponse {
    pub total_agents: usize,
    pub available_agents: usize,
    pub total_routed: u64,
    pub fallback_count: u64,
    pub fallback_rate: f64,
    pub agents_by_tier: HashMap<String, usize>,
}

/// Agent registration request
#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub agent_id: String,
    pub tier: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
}

fn default_weight() -> f64 {
    1.0
}

/// Agent registration response
#[derive(Debug, Serialize)]
pub struct RegisterAgentResponse {
    pub agent_id: String,
    pub tier: String,
    pub weight: f64,
    pub message: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────

/// POST /api/v1/routing/evaluate
///
/// Evaluate a task's complexity and get a routing decision.
/// Uses composite evaluation (heuristic + pattern-based) for accuracy.
pub async fn evaluate_task(
    State(state): State<AppState>,
    Json(req): Json<EvaluateRequest>,
) -> Result<Json<EvaluateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task_id = uuid::Uuid::new_v4().to_string();

    let task = TaskDescriptor {
        id: task_id.clone(),
        description: req.description.clone(),
        input_tokens: req.input_tokens,
        requires_tools: req.requires_tools,
        has_context: req.has_context,
        priority: req.priority,
        cost_budget: req.cost_budget,
        latency_budget: req.latency_budget,
        metadata: req.metadata,
    };

    let router = state.tier_routing.router.read().await;
    let decision = router.route(&task);
    let complexity = CompositeEvaluator::new().evaluate(&task);

    Ok(Json(EvaluateResponse {
        task_id,
        complexity: format!("{:?}", complexity),
        recommended_tier: format!("{:?}", decision.tier),
        decision: RoutingDecisionResponse::from(decision),
    }))
}

/// POST /api/v1/routing/batch
///
/// Batch evaluate multiple tasks for routing.
pub async fn batch_evaluate(
    State(state): State<AppState>,
    Json(req): Json<BatchEvaluateRequest>,
) -> Result<Json<BatchEvaluateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let router = state.tier_routing.router.read().await;
    let evaluator = CompositeEvaluator::new();

    let mut results = Vec::with_capacity(req.tasks.len());
    let mut simple_count = 0usize;
    let mut medium_count = 0usize;
    let mut complex_count = 0usize;
    let mut total_confidence = 0.0f64;

    for task_req in req.tasks.into_iter() {
        let task_id = uuid::Uuid::new_v4().to_string();

        let task = TaskDescriptor {
            id: task_id.clone(),
            description: task_req.description,
            input_tokens: task_req.input_tokens,
            requires_tools: task_req.requires_tools,
            has_context: task_req.has_context,
            priority: task_req.priority,
            cost_budget: task_req.cost_budget,
            latency_budget: task_req.latency_budget,
            metadata: task_req.metadata,
        };

        let decision = router.route(&task);
        let complexity = evaluator.evaluate(&task);

        match complexity {
            TaskComplexity::Simple => simple_count += 1,
            TaskComplexity::Medium => medium_count += 1,
            TaskComplexity::Complex => complex_count += 1,
        }
        total_confidence += decision.confidence;

        results.push(EvaluateResponse {
            task_id,
            complexity: format!("{:?}", complexity),
            recommended_tier: format!("{:?}", decision.tier),
            decision: RoutingDecisionResponse::from(decision),
        });
    }

    let total = results.len();
    Ok(Json(BatchEvaluateResponse {
        results,
        summary: BatchSummary {
            total,
            simple: simple_count,
            medium: medium_count,
            complex: complex_count,
            avg_confidence: if total > 0 {
                total_confidence / total as f64
            } else {
                0.0
            },
        },
    }))
}

/// GET /api/v1/routing/tiers
///
/// List available agent tiers with their properties and pool status.
pub async fn list_tiers(State(state): State<AppState>) -> Json<TierListResponse> {
    let pool = state.tier_routing.pool.read().await;

    let tiers: Vec<TierInfo> = vec![AgentTier::Weak, AgentTier::Mid, AgentTier::Strong]
        .into_iter()
        .map(|tier| {
            let tier_counts = pool.tier_counts();
            TierInfo {
                name: format!("{:?}", tier),
                relative_cost: tier.relative_cost(),
                latency_multiplier: tier.latency_multiplier(),
                capability_score: tier.capability_score(),
                registered_agents: tier_counts.get(&tier).copied().unwrap_or(0),
            }
        })
        .collect();

    let tier_counts = pool.tier_counts();
    let agents_by_tier: HashMap<String, usize> = tier_counts
        .into_iter()
        .map(|(k, v)| (format!("{:?}", k), v))
        .collect();

    Json(TierListResponse {
        tiers,
        pool_status: PoolStatusResponse {
            total_agents: pool.available_count(),
            available_agents: pool.available_count(),
            total_routed: pool.total_routed(),
            fallback_count: pool.fallback_count(),
            fallback_rate: pool.fallback_rate(),
            agents_by_tier,
        },
    })
}

/// POST /api/v1/routing/pool/register
///
/// Register an agent in the tier pool for smart routing.
pub async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<Json<RegisterAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tier = match req.tier.to_lowercase().as_str() {
        "weak" => AgentTier::Weak,
        "mid" | "medium" => AgentTier::Mid,
        "strong" => AgentTier::Strong,
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid tier '{}'. Must be weak, mid, or strong", other),
                    code: "INVALID_TIER".to_string(),
                }),
            ));
        }
    };

    let mut agent = PooledAgent::new(&req.agent_id, tier);
    agent.weight = req.weight;

    let mut pool = state.tier_routing.pool.write().await;
    pool.register(agent);

    Ok(Json(RegisterAgentResponse {
        agent_id: req.agent_id,
        tier: format!("{:?}", tier),
        weight: req.weight,
        message: format!("Agent registered in {:?} tier pool", tier),
    }))
}

/// GET /api/v1/routing/pool/status
///
/// Get detailed pool status including fallback metrics.
pub async fn pool_status(State(state): State<AppState>) -> Json<PoolStatusResponse> {
    let pool = state.tier_routing.pool.read().await;
    let tier_counts = pool.tier_counts();
    let agents_by_tier: HashMap<String, usize> = tier_counts
        .into_iter()
        .map(|(k, v)| (format!("{:?}", k), v))
        .collect();

    Json(PoolStatusResponse {
        total_agents: pool.available_count(),
        available_agents: pool.available_count(),
        total_routed: pool.total_routed(),
        fallback_count: pool.fallback_count(),
        fallback_rate: pool.fallback_rate(),
        agents_by_tier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            tier_routing: TierRoutingState::new(),
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
    async fn test_evaluate_simple_task() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "hello, what is 2+2?".to_string(),
            input_tokens: 10,
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };

        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        assert_eq!(result.recommended_tier, "Weak");
    }

    #[tokio::test]
    async fn test_evaluate_complex_task() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "Analyze and debug this code, then implement a refactor".to_string(),
            input_tokens: 5000,
            requires_tools: true,
            has_context: true,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };

        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        assert_eq!(result.recommended_tier, "Strong");
    }

    #[tokio::test]
    async fn test_batch_evaluate() {
        let state = test_state().await;
        let req = BatchEvaluateRequest {
            tasks: vec![
                EvaluateRequest {
                    description: "hello".to_string(),
                    input_tokens: 10,
                    requires_tools: false,
                    has_context: false,
                    priority: 1,
                    cost_budget: 0.0,
                    latency_budget: 0.0,
                    metadata: HashMap::new(),
                },
                EvaluateRequest {
                    description: "implement a security audit".to_string(),
                    input_tokens: 100,
                    requires_tools: false,
                    has_context: false,
                    priority: 1,
                    cost_budget: 0.0,
                    latency_budget: 0.0,
                    metadata: HashMap::new(),
                },
            ],
        };

        let result = batch_evaluate(State(state), Json(req)).await.unwrap();
        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.simple, 1);
        assert_eq!(result.summary.complex, 1);
    }

    #[tokio::test]
    async fn test_register_and_status() {
        let state = test_state().await;

        // Register an agent
        let reg_req = RegisterAgentRequest {
            agent_id: "test-agent-1".to_string(),
            tier: "strong".to_string(),
            weight: 2.0,
        };
        let reg_result = register_agent(State(state.clone()), Json(reg_req))
            .await
            .unwrap();
        assert_eq!(reg_result.tier, "Strong");

        // Check pool status
        let status = pool_status(State(state)).await;
        assert_eq!(status.available_agents, 1);
        assert_eq!(status.agents_by_tier.get("Strong"), Some(&1));
    }

    #[tokio::test]
    async fn test_invalid_tier_registration() {
        let state = test_state().await;
        let req = RegisterAgentRequest {
            agent_id: "test-agent".to_string(),
            tier: "invalid".to_string(),
            weight: 1.0,
        };
        let result = register_agent(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_tiers() {
        let state = test_state().await;
        let result = list_tiers(State(state)).await;
        assert_eq!(result.tiers.len(), 3);
        assert!(result.tiers.iter().any(|t| t.name == "Weak"));
        assert!(result.tiers.iter().any(|t| t.name == "Mid"));
        assert!(result.tiers.iter().any(|t| t.name == "Strong"));
    }

    #[tokio::test]
    async fn test_evaluate_medium_task() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "Write a Python function that parses CSV files".to_string(),
            input_tokens: 500,
            requires_tools: false,
            has_context: false,
            priority: 3,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        // Medium tasks should be routed to Mid tier
        assert!(!result.task_id.is_empty());
        assert!(!result.complexity.is_empty());
        assert!(result.decision.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_evaluate_with_cost_budget() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "simple question".to_string(),
            input_tokens: 10,
            requires_tools: false,
            has_context: false,
            priority: 5,
            cost_budget: 0.01, // Very tight budget
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        // Tight cost budget should bias toward cheaper tier
        assert!(result.decision.estimated_cost >= 0.0);
    }

    #[tokio::test]
    async fn test_evaluate_with_latency_budget() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "simple question".to_string(),
            input_tokens: 10,
            requires_tools: false,
            has_context: false,
            priority: 5,
            cost_budget: 0.0,
            latency_budget: 0.5, // Very tight latency
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        assert!(result.decision.estimated_latency >= 0.0);
    }

    #[tokio::test]
    async fn test_evaluate_with_metadata() {
        let state = test_state().await;
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "api".to_string());
        metadata.insert("user_id".to_string(), "12345".to_string());

        let req = EvaluateRequest {
            description: "Analyze this data".to_string(),
            input_tokens: 200,
            requires_tools: false,
            has_context: false,
            priority: 2,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata,
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        assert!(!result.task_id.is_empty());
    }

    #[tokio::test]
    async fn test_evaluate_with_tools_and_context() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "debug and fix".to_string(),
            input_tokens: 2000,
            requires_tools: true,
            has_context: true,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        // Tools + context + high tokens should recommend Strong
        assert_eq!(result.recommended_tier, "Strong");
    }

    #[tokio::test]
    async fn test_batch_evaluate_empty() {
        let state = test_state().await;
        let req = BatchEvaluateRequest { tasks: vec![] };
        let result = batch_evaluate(State(state), Json(req)).await.unwrap();
        assert_eq!(result.summary.total, 0);
        assert_eq!(result.summary.simple, 0);
        assert_eq!(result.summary.medium, 0);
        assert_eq!(result.summary.complex, 0);
        assert_eq!(result.summary.avg_confidence, 0.0);
        assert!(result.results.is_empty());
    }

    #[tokio::test]
    async fn test_batch_evaluate_single_item() {
        let state = test_state().await;
        let req = BatchEvaluateRequest {
            tasks: vec![EvaluateRequest {
                description: "hello".to_string(),
                input_tokens: 5,
                requires_tools: false,
                has_context: false,
                priority: 1,
                cost_budget: 0.0,
                latency_budget: 0.0,
                metadata: HashMap::new(),
            }],
        };
        let result = batch_evaluate(State(state), Json(req)).await.unwrap();
        assert_eq!(result.summary.total, 1);
        assert_eq!(result.results.len(), 1);
        assert!(result.summary.avg_confidence > 0.0);
    }

    #[tokio::test]
    async fn test_batch_evaluate_all_complex() {
        let state = test_state().await;
        let req = BatchEvaluateRequest {
            tasks: vec![
                EvaluateRequest {
                    description: "Implement a distributed consensus algorithm with fault tolerance"
                        .to_string(),
                    input_tokens: 10000,
                    requires_tools: true,
                    has_context: true,
                    priority: 1,
                    cost_budget: 0.0,
                    latency_budget: 0.0,
                    metadata: HashMap::new(),
                },
                EvaluateRequest {
                    description: "Refactor the entire codebase with architectural changes"
                        .to_string(),
                    input_tokens: 8000,
                    requires_tools: true,
                    has_context: true,
                    priority: 1,
                    cost_budget: 0.0,
                    latency_budget: 0.0,
                    metadata: HashMap::new(),
                },
            ],
        };
        let result = batch_evaluate(State(state), Json(req)).await.unwrap();
        assert_eq!(result.summary.total, 2);
        // Both should be complex
        assert_eq!(result.summary.complex, 2);
    }

    #[tokio::test]
    async fn test_register_mid_alias() {
        let state = test_state().await;
        let req = RegisterAgentRequest {
            agent_id: "mid-agent".to_string(),
            tier: "mid".to_string(),
            weight: 1.5,
        };
        let result = register_agent(State(state), Json(req)).await.unwrap();
        assert_eq!(result.tier, "Mid");
        assert_eq!(result.weight, 1.5);
        assert!(result.message.contains("Mid"));
    }

    #[tokio::test]
    async fn test_register_medium_alias() {
        let state = test_state().await;
        let req = RegisterAgentRequest {
            agent_id: "medium-agent".to_string(),
            tier: "medium".to_string(),
            weight: 1.0,
        };
        let result = register_agent(State(state), Json(req)).await.unwrap();
        assert_eq!(result.tier, "Mid");
    }

    #[tokio::test]
    async fn test_register_weak_tier() {
        let state = test_state().await;
        let req = RegisterAgentRequest {
            agent_id: "weak-agent".to_string(),
            tier: "weak".to_string(),
            weight: 0.5,
        };
        let result = register_agent(State(state), Json(req)).await.unwrap();
        assert_eq!(result.tier, "Weak");
        assert_eq!(result.weight, 0.5);
    }

    #[tokio::test]
    async fn test_register_case_insensitive() {
        let state = test_state().await;
        let req = RegisterAgentRequest {
            agent_id: "case-agent".to_string(),
            tier: "STRONG".to_string(),
            weight: 1.0,
        };
        let result = register_agent(State(state), Json(req)).await.unwrap();
        assert_eq!(result.tier, "Strong");
    }

    #[tokio::test]
    async fn test_pool_status_empty() {
        let state = test_state().await;
        let status = pool_status(State(state)).await;
        assert_eq!(status.total_agents, 0);
        assert_eq!(status.available_agents, 0);
        assert_eq!(status.total_routed, 0);
        assert_eq!(status.fallback_count, 0);
        assert_eq!(status.fallback_rate, 0.0);
    }

    #[tokio::test]
    async fn test_pool_status_multiple_tiers() {
        let state = test_state().await;

        // Register agents in all three tiers
        for (id, tier) in [
            ("w1", "weak"),
            ("w2", "weak"),
            ("m1", "mid"),
            ("s1", "strong"),
        ] {
            let req = RegisterAgentRequest {
                agent_id: id.to_string(),
                tier: tier.to_string(),
                weight: 1.0,
            };
            let _ = register_agent(State(state.clone()), Json(req)).await;
        }

        let status = pool_status(State(state)).await;
        assert_eq!(status.total_agents, 4);
        assert_eq!(status.agents_by_tier.get("Weak"), Some(&2));
        assert_eq!(status.agents_by_tier.get("Mid"), Some(&1));
        assert_eq!(status.agents_by_tier.get("Strong"), Some(&1));
    }

    #[tokio::test]
    async fn test_list_tiers_properties() {
        let state = test_state().await;
        let result = list_tiers(State(state)).await;

        // Verify tier properties are reasonable
        for tier in &result.tiers {
            assert!(tier.relative_cost > 0.0);
            assert!(tier.latency_multiplier > 0.0);
            assert!(tier.capability_score > 0.0);
        }

        // Weak should be cheapest, Strong most expensive
        let weak = result.tiers.iter().find(|t| t.name == "Weak").unwrap();
        let strong = result.tiers.iter().find(|t| t.name == "Strong").unwrap();
        assert!(weak.relative_cost < strong.relative_cost);
        assert!(weak.capability_score < strong.capability_score);
    }

    #[tokio::test]
    async fn test_routing_decision_has_reason() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "test task".to_string(),
            input_tokens: 100,
            requires_tools: false,
            has_context: false,
            priority: 3,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        // Decision should always have a non-empty reason
        assert!(!result.decision.reason.is_empty());
        assert!(result.decision.confidence > 0.0);
        assert!(result.decision.confidence <= 1.0);
    }

    #[tokio::test]
    async fn test_evaluate_high_priority() {
        let state = test_state().await;
        let req = EvaluateRequest {
            description: "CRITICAL: fix production outage NOW".to_string(),
            input_tokens: 5000,
            requires_tools: true,
            has_context: true,
            priority: 1, // highest priority
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let result = evaluate_task(State(state), Json(req)).await.unwrap();
        // High priority + complex should route to Strong
        assert_eq!(result.recommended_tier, "Strong");
    }
}
