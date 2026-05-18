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
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Pattern-based complexity evaluator using keyword matching.
///
/// Recognizes complex task patterns (code generation, multi-step reasoning, research)
/// and simple patterns (greetings, basic queries) to provide more accurate classification
/// than the pure numeric heuristic.
pub struct PatternEvaluator;

/// Keywords that indicate high complexity tasks
const COMPLEX_KEYWORDS: &[&str] = &[
    "analyze",
    "research",
    "debug",
    "refactor",
    "implement",
    "architect",
    "design",
    "optimize",
    "migrate",
    "deploy",
    "multi-step",
    "chain of thought",
    "reasoning",
    "code review",
    "security audit",
    "performance analysis",
    "write a program",
    "generate code",
    "create a system",
    "build a",
];

/// Keywords that indicate low complexity tasks
const SIMPLE_KEYWORDS: &[&str] = &[
    "hello",
    "what is",
    "define",
    "list",
    "show me",
    "tell me",
    "translate",
    "convert",
    "format",
    "summarize briefly",
];

impl PatternEvaluator {
    /// Score complexity based on keyword patterns in the description
    fn pattern_score(description: &str) -> f64 {
        let lower = description.to_lowercase();
        let mut score = 0.0;

        for kw in COMPLEX_KEYWORDS {
            if lower.contains(kw) {
                score += 2.0;
            }
        }
        for kw in SIMPLE_KEYWORDS {
            if lower.contains(kw) {
                score -= 1.0;
            }
        }

        // Question-only tasks are typically simpler
        if lower.chars().filter(|c| *c == '?').count() >= 3 {
            score += 1.5;
        }

        // Code blocks suggest complexity
        if lower.contains("```") || lower.contains("fn ") || lower.contains("def ") {
            score += 2.0;
        }

        score
    }
}

impl ComplexityEvaluator for PatternEvaluator {
    fn evaluate(&self, task: &TaskDescriptor) -> TaskComplexity {
        let pattern = Self::pattern_score(&task.description);
        let base =
            TaskComplexity::estimate(task.input_tokens, task.requires_tools, task.has_context);

        // Combine: pattern score can upgrade or downgrade the base estimate
        let combined = match base {
            TaskComplexity::Simple => 1.0 + pattern,
            TaskComplexity::Medium => 3.0 + pattern,
            TaskComplexity::Complex => 6.0 + pattern,
        };

        if combined >= 5.0 {
            TaskComplexity::Complex
        } else if combined >= 2.0 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Simple
        }
    }
}

/// Composite evaluator: combines multiple evaluators and takes the highest complexity.
///
/// This ensures we never underestimate task complexity — if any evaluator flags
/// a task as complex, we treat it as complex.
pub struct CompositeEvaluator {
    evaluators: Vec<Box<dyn ComplexityEvaluator>>,
}

impl CompositeEvaluator {
    pub fn new() -> Self {
        Self {
            evaluators: vec![Box::new(HeuristicEvaluator), Box::new(PatternEvaluator)],
        }
    }

    pub fn with_evaluators(evaluators: Vec<Box<dyn ComplexityEvaluator>>) -> Self {
        Self { evaluators }
    }
}

impl Default for CompositeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityEvaluator for CompositeEvaluator {
    fn evaluate(&self, task: &TaskDescriptor) -> TaskComplexity {
        self.evaluators
            .iter()
            .map(|e| e.evaluate(task))
            .max()
            .unwrap_or(TaskComplexity::Simple)
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

/// Agent pool: manages agents of different tiers with fallback support.
///
/// When no agent is available in the requested tier, the pool automatically
/// falls back to adjacent tiers (strong→mid→weak).
pub struct AgentPool {
    /// Agents by tier
    agents: HashMap<AgentTier, Vec<PooledAgent>>,
    /// Total routing decisions (for metrics)
    total_routed: AtomicU64,
    /// Fallback routing decisions
    fallback_count: AtomicU64,
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
    /// Weight for weighted selection (higher = more likely to be selected)
    pub weight: f64,
}

impl PooledAgent {
    /// Create a new pooled agent with default weight
    pub fn new(id: impl Into<String>, tier: AgentTier) -> Self {
        Self {
            id: id.into(),
            tier,
            load: 0.0,
            success_rate: 1.0,
            available: true,
            weight: 1.0,
        }
    }

    /// Effective score for weighted selection: weight × success_rate × (1 - load)
    pub fn effective_score(&self) -> f64 {
        if !self.available || self.load >= 0.95 {
            return 0.0;
        }
        self.weight * self.success_rate * (1.0 - self.load)
    }
}

impl AgentPool {
    /// Create a new empty agent pool
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            total_routed: AtomicU64::new(0),
            fallback_count: AtomicU64::new(0),
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

    /// Get the best agent for a tier using weighted selection.
    ///
    /// Score = weight × success_rate × (1 - load)
    /// Prefers agents with high success rate, low load, and high weight.
    pub fn get_agent_weighted(&self, tier: AgentTier) -> Option<&PooledAgent> {
        self.agents
            .get(&tier)?
            .iter()
            .filter(|a| a.effective_score() > 0.0)
            .max_by(|a, b| {
                a.effective_score()
                    .partial_cmp(&b.effective_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get an agent with tier fallback.
    ///
    /// Tries the requested tier first, then falls back to lower tiers:
    /// Strong → Mid → Weak
    pub fn get_agent_with_fallback(&self, tier: AgentTier) -> Option<(&PooledAgent, AgentTier)> {
        self.total_routed.fetch_add(1, Ordering::Relaxed);

        // Try requested tier first
        if let Some(agent) = self.get_agent_weighted(tier) {
            return Some((agent, tier));
        }

        // Fall back to lower tiers
        let fallback_tiers: Vec<AgentTier> = match tier {
            AgentTier::Strong => vec![AgentTier::Mid, AgentTier::Weak],
            AgentTier::Mid => vec![AgentTier::Weak],
            AgentTier::Weak => vec![],
        };

        for fallback_tier in fallback_tiers {
            if let Some(agent) = self.get_agent_weighted(fallback_tier) {
                self.fallback_count.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(
                    requested = ?tier,
                    fallback = ?fallback_tier,
                    agent_id = %agent.id,
                    "Tier fallback: using lower-tier agent"
                );
                return Some((agent, fallback_tier));
            }
        }

        None
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

    /// Total routing decisions
    pub fn total_routed(&self) -> u64 {
        self.total_routed.load(Ordering::Relaxed)
    }

    /// Fallback routing count
    pub fn fallback_count(&self) -> u64 {
        self.fallback_count.load(Ordering::Relaxed)
    }

    /// Fallback rate (0.0 - 1.0)
    pub fn fallback_rate(&self) -> f64 {
        let total = self.total_routed.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.fallback_count.load(Ordering::Relaxed) as f64 / total as f64
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
        pool.register(PooledAgent::new("weak-1", AgentTier::Weak));
        pool.register(PooledAgent::new("strong-1", AgentTier::Strong));

        assert_eq!(pool.available_count(), 2);
        let agent = pool.get_agent(AgentTier::Weak).unwrap();
        assert_eq!(agent.id, "weak-1");
    }

    #[test]
    fn test_agent_pool_least_loaded() {
        let mut pool = AgentPool::new();
        let mut a1 = PooledAgent::new("a1", AgentTier::Weak);
        a1.load = 0.8;
        pool.register(a1);

        let mut a2 = PooledAgent::new("a2", AgentTier::Weak);
        a2.load = 0.2;
        pool.register(a2);

        let agent = pool.get_agent(AgentTier::Weak).unwrap();
        assert_eq!(agent.id, "a2"); // Less loaded
    }

    // ─── New tests for enhanced features ──────────────────────────────

    #[test]
    fn test_pattern_evaluator_complex() {
        let evaluator = PatternEvaluator;
        let task = TaskDescriptor {
            id: "t1".to_string(),
            description: "Analyze and debug this code, then implement a refactor".to_string(),
            input_tokens: 100,
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        // Pattern keywords should upgrade this to Complex
        assert_eq!(evaluator.evaluate(&task), TaskComplexity::Complex);
    }

    #[test]
    fn test_pattern_evaluator_simple() {
        let evaluator = PatternEvaluator;
        let task = TaskDescriptor {
            id: "t1".to_string(),
            description: "hello, what is the weather?".to_string(),
            input_tokens: 10,
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        // Simple keywords should keep this Simple
        assert_eq!(evaluator.evaluate(&task), TaskComplexity::Simple);
    }

    #[test]
    fn test_pattern_evaluator_code_blocks() {
        let evaluator = PatternEvaluator;
        let task = TaskDescriptor {
            id: "t1".to_string(),
            description: "Fix this code:\n```python\ndef foo():\n  pass\n```".to_string(),
            input_tokens: 50,
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        // Code blocks should upgrade complexity
        let complexity = evaluator.evaluate(&task);
        assert!(complexity >= TaskComplexity::Medium);
    }

    #[test]
    fn test_composite_evaluator() {
        let evaluator = CompositeEvaluator::new();
        // A task that's simple by heuristic but complex by pattern
        let task = TaskDescriptor {
            id: "t1".to_string(),
            description: "Implement a security audit for this system".to_string(),
            input_tokens: 100, // Low token count → Simple by heuristic
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        // Composite should pick the max (Complex from pattern evaluator)
        assert_eq!(evaluator.evaluate(&task), TaskComplexity::Complex);
    }

    #[test]
    fn test_pooled_agent_effective_score() {
        let mut agent = PooledAgent::new("a1", AgentTier::Strong);
        agent.weight = 2.0;
        agent.success_rate = 0.9;
        agent.load = 0.1;
        // score = 2.0 × 0.9 × 0.9 = 1.62
        assert!((agent.effective_score() - 1.62).abs() < 0.01);
    }

    #[test]
    fn test_pooled_agent_unavailable_score() {
        let mut agent = PooledAgent::new("a1", AgentTier::Strong);
        agent.available = false;
        assert_eq!(agent.effective_score(), 0.0);
    }

    #[test]
    fn test_pooled_agent_overloaded_score() {
        let mut agent = PooledAgent::new("a1", AgentTier::Strong);
        agent.load = 0.96;
        assert_eq!(agent.effective_score(), 0.0);
    }

    #[test]
    fn test_weighted_selection() {
        let mut pool = AgentPool::new();

        let mut a1 = PooledAgent::new("low-weight", AgentTier::Weak);
        a1.weight = 1.0;
        a1.load = 0.1;
        pool.register(a1);

        let mut a2 = PooledAgent::new("high-weight", AgentTier::Weak);
        a2.weight = 5.0;
        a2.load = 0.1;
        pool.register(a2);

        let agent = pool.get_agent_weighted(AgentTier::Weak).unwrap();
        assert_eq!(agent.id, "high-weight"); // Higher weight wins
    }

    #[test]
    fn test_tier_fallback_strong_to_mid() {
        let mut pool = AgentPool::new();
        // No strong agents, one mid agent
        pool.register(PooledAgent::new("mid-1", AgentTier::Mid));

        let (agent, tier) = pool.get_agent_with_fallback(AgentTier::Strong).unwrap();
        assert_eq!(agent.id, "mid-1");
        assert_eq!(tier, AgentTier::Mid);
        assert_eq!(pool.fallback_count(), 1);
        assert_eq!(pool.total_routed(), 1);
    }

    #[test]
    fn test_tier_fallback_strong_to_weak() {
        let mut pool = AgentPool::new();
        // No strong or mid agents, one weak agent
        pool.register(PooledAgent::new("weak-1", AgentTier::Weak));

        let (agent, tier) = pool.get_agent_with_fallback(AgentTier::Strong).unwrap();
        assert_eq!(agent.id, "weak-1");
        assert_eq!(tier, AgentTier::Weak);
        assert_eq!(pool.fallback_count(), 1);
    }

    #[test]
    fn test_tier_fallback_none_available() {
        let pool = AgentPool::new();
        assert!(pool.get_agent_with_fallback(AgentTier::Strong).is_none());
        assert_eq!(pool.total_routed(), 1);
    }

    #[test]
    fn test_tier_fallback_preferred_tier_available() {
        let mut pool = AgentPool::new();
        pool.register(PooledAgent::new("strong-1", AgentTier::Strong));
        pool.register(PooledAgent::new("weak-1", AgentTier::Weak));

        let (agent, tier) = pool.get_agent_with_fallback(AgentTier::Strong).unwrap();
        assert_eq!(agent.id, "strong-1");
        assert_eq!(tier, AgentTier::Strong); // No fallback
        assert_eq!(pool.fallback_count(), 0);
    }

    #[test]
    fn test_fallback_rate() {
        let mut pool = AgentPool::new();
        pool.register(PooledAgent::new("weak-1", AgentTier::Weak));

        // First call: fallback from Strong → Weak
        pool.get_agent_with_fallback(AgentTier::Strong);
        // Second call: direct Weak → Weak
        pool.get_agent_with_fallback(AgentTier::Weak);

        assert_eq!(pool.total_routed(), 2);
        assert_eq!(pool.fallback_count(), 1);
        assert!((pool.fallback_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_smart_router_with_composite_evaluator() {
        let router = SmartRouter::with_evaluator(Box::new(CompositeEvaluator::new()));
        let task = TaskDescriptor {
            id: "t1".to_string(),
            description: "Implement a security audit and debug this code".to_string(),
            input_tokens: 100, // Low tokens, but complex patterns
            requires_tools: false,
            has_context: false,
            priority: 1,
            cost_budget: 0.0,
            latency_budget: 0.0,
            metadata: HashMap::new(),
        };
        let decision = router.route(&task);
        // Composite evaluator should detect complexity from patterns
        assert_eq!(decision.tier, AgentTier::Strong);
    }

    #[test]
    fn test_routing_decision_fields() {
        let router = SmartRouter::new();
        let decision = router.route(&simple_task());
        assert_eq!(decision.tier, AgentTier::Weak);
        assert!(decision.estimated_cost > 0.0);
        assert!(decision.estimated_latency > 0.0);
        assert!(decision.confidence > 0.0);
        assert!(!decision.reason.is_empty());
    }
}
