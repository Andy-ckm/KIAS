//! # Agent Tiering & Smart Routing
//!
//! PrfaaS-inspired selective offloading: not all tasks need the strongest agent.
//!
//! Core insight from PrfaaS paper:
//! - Short requests: memory-bound, not compute-bound → waste on strong agents
//! - Long requests: compute-bound → need strong agents
//! - Threshold-based routing: complexity > t → strong agent, else weak agent
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
//! │  Task Input   │───▶│  Complexity  │───▶│   Router     │
//! │               │    │  Evaluator   │    │              │
//! └──────────────┘    └──────────────┘    └──────┬───────┘
//!                                                │
//!                        ┌───────────────────────┼───────────────────────┐
//!                        ▼                       ▼                       ▼
//!                ┌──────────────┐        ┌──────────────┐        ┌──────────────┐
//!                │  Weak Agent  │        │  Mid Agent   │        │ Strong Agent │
//!                │  (fast/cheap)│        │  (balanced)  │        │ (capable)    │
//!                └──────────────┘        └──────────────┘        └──────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent capability tier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentTier {
    /// Weak agent: fast, cheap, limited capability (e.g., small model, local inference)
    Weak,
    /// Mid agent: balanced (e.g., medium model, API call)
    Mid,
    /// Strong agent: slow, expensive, high capability (e.g., large model, complex reasoning)
    Strong,
}

impl AgentTier {
    /// Typical cost per 1M tokens (relative)
    pub fn relative_cost(&self) -> f64 {
        match self {
            AgentTier::Weak => 1.0,
            AgentTier::Mid => 5.0,
            AgentTier::Strong => 20.0,
        }
    }

    /// Typical latency multiplier
    pub fn latency_multiplier(&self) -> f64 {
        match self {
            AgentTier::Weak => 1.0,
            AgentTier::Mid => 2.0,
            AgentTier::Strong => 5.0,
        }
    }

    /// Capability score (0.0 - 1.0)
    pub fn capability_score(&self) -> f64 {
        match self {
            AgentTier::Weak => 0.4,
            AgentTier::Mid => 0.7,
            AgentTier::Strong => 0.95,
        }
    }
}

/// Task complexity classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskComplexity {
    /// Simple task: keyword matching, formatting, basic lookup
    Simple,
    /// Medium task: summarization, translation, basic analysis
    Medium,
    /// Complex task: multi-step reasoning, code generation, research
    Complex,
}

impl TaskComplexity {
    /// Estimate from input characteristics
    pub fn estimate(input_tokens: u32, has_tools: bool, has_context: bool) -> Self {
        let score = (input_tokens as f64 / 1000.0)
            + if has_tools { 2.0 } else { 0.0 }
            + if has_context { 1.5 } else { 0.0 };

        if score > 5.0 {
            TaskComplexity::Complex
        } else if score > 2.0 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Simple
        }
    }

    /// Minimum tier needed for this complexity
    pub fn min_tier(&self) -> AgentTier {
        match self {
            TaskComplexity::Simple => AgentTier::Weak,
            TaskComplexity::Medium => AgentTier::Mid,
            TaskComplexity::Complex => AgentTier::Strong,
        }
    }
}

/// Routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected agent tier
    pub tier: AgentTier,
    /// Reason for selection
    pub reason: String,
    /// Estimated cost (relative)
    pub estimated_cost: f64,
    /// Estimated latency multiplier
    pub estimated_latency: f64,
    /// Confidence in this routing (0.0 - 1.0)
    pub confidence: f64,
}

/// Complexity evaluator trait
pub trait ComplexityEvaluator: Send + Sync {
    /// Evaluate task complexity
    fn evaluate(&self, task: &TaskDescriptor) -> TaskComplexity;
}

/// Task descriptor for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDescriptor {
    /// Task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Estimated input tokens
    pub input_tokens: u32,
    /// Whether task requires tool use
    pub requires_tools: bool,
    /// Whether task has additional context
    pub has_context: bool,
    /// Priority (1 = highest)
    pub priority: u32,
    /// Cost budget (relative units, 0 = unlimited)
    pub cost_budget: f64,
    /// Latency budget (multiplier, 0 = unlimited)
    pub latency_budget: f64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Default complexity evaluator using heuristic scoring
pub struct HeuristicEvaluator;

impl ComplexityEvaluator for HeuristicEvaluator {
    fn evaluate(&self, task: &TaskDescriptor) -> TaskComplexity {
        TaskComplexity::estimate(task.input_tokens, task.requires_tools, task.has_context)
    }
}

/// Smart router: routes tasks to appropriate agent tiers
pub struct SmartRouter {
    /// Complexity evaluator
    evaluator: Box<dyn ComplexityEvaluator>,
    /// Routing threshold: complexity score above this → upgrade tier
    pub threshold: f64,
    /// Cost optimization: prefer cheaper agents when possible
    pub cost_optimize: bool,
    /// Bandwidth utilization (0.0 - 1.0, for PrfaaS-style bandwidth awareness)
    pub bandwidth_utilization: f64,
    /// Bandwidth threshold: above this, prefer local agents
    pub bandwidth_threshold: f64,
}

impl SmartRouter {
    /// Create a new smart router with default settings
    pub fn new() -> Self {
        Self {
            evaluator: Box::new(HeuristicEvaluator),
            threshold: 3.0,
            cost_optimize: true,
            bandwidth_utilization: 0.0,
            bandwidth_threshold: 0.8,
        }
    }

    /// Create with custom evaluator
    pub fn with_evaluator(evaluator: Box<dyn ComplexityEvaluator>) -> Self {
        Self {
            evaluator,
            threshold: 3.0,
            cost_optimize: true,
            bandwidth_utilization: 0.0,
            bandwidth_threshold: 0.8,
        }
    }

    /// Route a task to the appropriate agent tier
    pub fn route(&self, task: &TaskDescriptor) -> RoutingDecision {
        let complexity = self.evaluator.evaluate(task);
        let mut tier = complexity.min_tier();

        // Bandwidth-aware adjustment (PrfaaS insight):
        // When bandwidth is high, can offload to remote strong agents
        // When bandwidth is constrained, prefer local weak agents
        if self.bandwidth_utilization > self.bandwidth_threshold {
            // Bandwidth constrained — prefer local/weak
            if tier > AgentTier::Weak {
                tier = match tier {
                    AgentTier::Strong => AgentTier::Mid,
                    AgentTier::Mid => AgentTier::Weak,
                    AgentTier::Weak => AgentTier::Weak,
                };
            }
        }

        // Cost optimization: downgrade if budget is tight
        if self.cost_optimize && task.cost_budget > 0.0 {
            while tier.relative_cost() > task.cost_budget && tier > AgentTier::Weak {
                tier = match tier {
                    AgentTier::Strong => AgentTier::Mid,
                    AgentTier::Mid => AgentTier::Weak,
                    AgentTier::Weak => AgentTier::Weak,
                };
            }
        }

        // Latency optimization: downgrade if latency budget is tight
        if task.latency_budget > 0.0 && tier.latency_multiplier() > task.latency_budget {
            while tier.latency_multiplier() > task.latency_budget && tier > AgentTier::Weak {
                tier = match tier {
                    AgentTier::Strong => AgentTier::Mid,
                    AgentTier::Mid => AgentTier::Weak,
                    AgentTier::Weak => AgentTier::Weak,
                };
            }
        }

        RoutingDecision {
            tier,
            reason: format!(
                "complexity={:?}, cost_budget={}, latency_budget={}, bandwidth_util={:.1}",
                complexity, task.cost_budget, task.latency_budget, self.bandwidth_utilization
            ),
            estimated_cost: tier.relative_cost(),
            estimated_latency: tier.latency_multiplier(),
            confidence: 0.8, // Heuristic confidence
        }
    }

    /// Update bandwidth utilization (call periodically)
    pub fn update_bandwidth(&mut self, utilization: f64) {
        self.bandwidth_utilization = utilization.clamp(0.0, 1.0);
    }
}

impl Default for SmartRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent pool: manages agents of different tiers
pub struct AgentPool {
    /// Agents by tier
    agents: HashMap<AgentTier, Vec<PooledAgent>>,
}

/// A pooled agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PooledAgent {
    /// Agent ID
    pub id: String,
    /// Agent tier
    pub tier: AgentTier,
    /// Current load (0.0 - 1.0)
    pub load: f64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Whether agent is available
    pub available: bool,
}

impl AgentPool {
    /// Create a new empty agent pool
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register an agent
    pub fn register(&mut self, agent: PooledAgent) {
        self.agents.entry(agent.tier).or_default().push(agent);
    }

    /// Get the least loaded agent of a given tier
    pub fn get_agent(&self, tier: AgentTier) -> Option<&PooledAgent> {
        self.agents
            .get(&tier)?
            .iter()
            .filter(|a| a.available && a.load < 0.9)
            .min_by(|a, b| {
                a.load
                    .partial_cmp(&b.load)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get agent counts by tier
    pub fn tier_counts(&self) -> HashMap<AgentTier, usize> {
        self.agents
            .iter()
            .map(|(tier, agents)| (*tier, agents.len()))
            .collect()
    }

    /// Get total available agents
    pub fn available_count(&self) -> usize {
        self.agents
            .values()
            .flat_map(|agents| agents.iter())
            .filter(|a| a.available)
            .count()
    }
}

impl Default for AgentPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_task() -> TaskDescriptor {
        TaskDescriptor {
            id: "t1".to_string(),
            description: "What is 2+2?".to_string(),
            input_tokens: 10,
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        }
    }

    fn complex_task() -> TaskDescriptor {
        TaskDescriptor {
            id: "t2".to_string(),
            description: "Analyze this codebase and suggest improvements".to_string(),
            input_tokens: 5000,
            requires_tools: true,
            has_context: true,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_tier_ordering() {
        assert!(AgentTier::Weak < AgentTier::Mid);
        assert!(AgentTier::Mid < AgentTier::Strong);
    }

    #[test]
    fn test_tier_costs() {
        assert!(AgentTier::Weak.relative_cost() < AgentTier::Strong.relative_cost());
    }

    #[test]
    fn test_complexity_estimation() {
        assert_eq!(
            TaskComplexity::estimate(10, false, false),
            TaskComplexity::Simple
        );
        assert_eq!(
            TaskComplexity::estimate(5000, true, true),
            TaskComplexity::Complex
        );
    }

    #[test]
    fn test_router_simple_task() {
        let router = SmartRouter::new();
        let decision = router.route(&simple_task());
        assert_eq!(decision.tier, AgentTier::Weak);
    }

    #[test]
    fn test_router_complex_task() {
        let router = SmartRouter::new();
        let decision = router.route(&complex_task());
        assert_eq!(decision.tier, AgentTier::Strong);
    }

    #[test]
    fn test_router_cost_constraint() {
        let router = SmartRouter::new();
        let mut task = complex_task();
        task.cost_budget = 3.0; // Only afford weak
        let decision = router.route(&task);
        assert_eq!(decision.tier, AgentTier::Weak);
    }

    #[test]
    fn test_router_latency_constraint() {
        let router = SmartRouter::new();
        let mut task = complex_task();
        task.latency_budget = 1.5; // Only afford weak (1.0x)
        let decision = router.route(&task);
        assert!(decision.estimated_latency <= task.latency_budget);
    }

    #[test]
    fn test_router_bandwidth_aware() {
        let mut router = SmartRouter::new();
        router.update_bandwidth(0.9); // High bandwidth utilization
        let decision = router.route(&complex_task());
        // Should downgrade due to bandwidth constraint
        assert!(decision.tier < AgentTier::Strong);
    }

    #[test]
    fn test_agent_pool() {
        let mut pool = AgentPool::new();
        pool.register(PooledAgent {
            id: "weak-1".to_string(),
            tier: AgentTier::Weak,
            load: 0.1,
            success_rate: 0.95,
            available: true,
        });
        pool.register(PooledAgent {
            id: "strong-1".to_string(),
            tier: AgentTier::Strong,
            load: 0.5,
            success_rate: 0.99,
            available: true,
        });

        assert_eq!(pool.available_count(), 2);
        let agent = pool.get_agent(AgentTier::Weak).unwrap();
        assert_eq!(agent.id, "weak-1");
    }

    #[test]
    fn test_agent_pool_least_loaded() {
        let mut pool = AgentPool::new();
        pool.register(PooledAgent {
            id: "a1".to_string(),
            tier: AgentTier::Weak,
            load: 0.8,
            success_rate: 0.95,
            available: true,
        });
        pool.register(PooledAgent {
            id: "a2".to_string(),
            tier: AgentTier::Weak,
            load: 0.2,
            success_rate: 0.95,
            available: true,
        });

        let agent = pool.get_agent(AgentTier::Weak).unwrap();
        assert_eq!(agent.id, "a2"); // Less loaded
    }
}
