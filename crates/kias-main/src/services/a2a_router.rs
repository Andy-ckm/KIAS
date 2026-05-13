//! A2A (Agent-to-Agent) Task Routing Service
//!
//! Implements intelligent task routing between agents using multiple strategies:
//! - Direct routing: Route to a specific agent by ID
//! - Capability-based: Route based on agent capabilities/skills
//! - Load-balanced: Route to least loaded agent
//! - Broadcast: Send to all agents, aggregate responses
//! - Chain: Sequential agent pipeline

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Task routing strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Route to a specific agent by ID
    Direct,
    /// Route based on agent capabilities
    Capability,
    /// Route to least loaded agent
    LoadBalanced,
    /// Send to all agents, collect responses
    Broadcast,
    /// Sequential agent pipeline
    Chain,
}

/// A task to be routed between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATask {
    pub task_id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub required_capabilities: Vec<String>,
    pub priority: TaskPriority,
    pub strategy: RoutingStrategy,
    pub target_agent: Option<String>,
    pub chain_agents: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl A2ATask {
    pub fn new(task_id: &str, task_type: &str, payload: serde_json::Value) -> Self {
        Self {
            task_id: task_id.to_string(),
            task_type: task_type.to_string(),
            payload,
            required_capabilities: Vec::new(),
            priority: TaskPriority::Normal,
            strategy: RoutingStrategy::Capability,
            target_agent: None,
            chain_agents: Vec::new(),
            created_at: Utc::now(),
            timeout_ms: 30_000,
            max_retries: 3,
        }
    }

    pub fn with_strategy(mut self, strategy: RoutingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_target(mut self, agent_id: &str) -> Self {
        self.target_agent = Some(agent_id.to_string());
        self.strategy = RoutingStrategy::Direct;
        self
    }

    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.required_capabilities = caps;
        self
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_chain(mut self, agents: Vec<String>) -> Self {
        self.chain_agents = agents;
        self.strategy = RoutingStrategy::Chain;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// Task routing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AResponse {
    pub task_id: String,
    pub agent_id: String,
    pub status: ResponseStatus,
    pub result: serde_json::Value,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponseStatus {
    Success,
    Failure,
    Timeout,
    Partial, // For broadcast/chain when some agents fail
}

/// Agent registration in the routing table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub agent_id: String,
    pub capabilities: Vec<String>,
    pub endpoint: String,
    pub max_concurrent_tasks: u32,
    pub current_tasks: u32,
    pub health_score: f64,
    pub avg_latency_ms: f64,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

impl AgentRegistration {
    pub fn new(agent_id: &str, capabilities: Vec<String>, endpoint: &str) -> Self {
        let now = Utc::now();
        Self {
            agent_id: agent_id.to_string(),
            capabilities,
            endpoint: endpoint.to_string(),
            max_concurrent_tasks: 10,
            current_tasks: 0,
            health_score: 1.0,
            avg_latency_ms: 0.0,
            registered_at: now,
            last_heartbeat: now,
        }
    }

    pub fn is_available(&self) -> bool {
        self.current_tasks < self.max_concurrent_tasks && self.health_score > 0.3
    }

    pub fn has_capabilities(&self, required: &[String]) -> bool {
        required.iter().all(|cap| self.capabilities.contains(cap))
    }

    pub fn load_factor(&self) -> f64 {
        if self.max_concurrent_tasks == 0 { 1.0 }
        else { self.current_tasks as f64 / self.max_concurrent_tasks as f64 }
    }
}

/// Routing decision log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub task_id: String,
    pub strategy: RoutingStrategy,
    pub selected_agents: Vec<String>,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

/// A2A Task Router
pub struct A2ARouter {
    agents: Arc<RwLock<HashMap<String, AgentRegistration>>>,
    routing_log: Arc<RwLock<Vec<RoutingDecision>>>,
    max_log_size: usize,
}

impl Default for A2ARouter {
    fn default() -> Self {
        Self::new()
    }
}

impl A2ARouter {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            routing_log: Arc::new(RwLock::new(Vec::new())),
            max_log_size: 10_000,
        }
    }

    /// Register an agent in the routing table
    pub async fn register_agent(&self, registration: AgentRegistration) {
        let mut agents = self.agents.write().await;
        agents.insert(registration.agent_id.clone(), registration);
        tracing::info!("Agent registered in A2A router");
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id);
        tracing::info!(agent_id, "Agent unregistered from A2A router");
    }

    /// Route a task to the appropriate agent(s)
    pub async fn route(&self, task: &A2ATask) -> Vec<String> {
        let agents = self.agents.read().await;
        let selected = match task.strategy {
            RoutingStrategy::Direct => self.route_direct(&agents, task),
            RoutingStrategy::Capability => self.route_by_capability(&agents, task),
            RoutingStrategy::LoadBalanced => self.route_load_balanced(&agents, task),
            RoutingStrategy::Broadcast => self.route_broadcast(&agents, task),
            RoutingStrategy::Chain => self.route_chain(&agents, task),
        };

        // Log routing decision
        let decision = RoutingDecision {
            task_id: task.task_id.clone(),
            strategy: task.strategy.clone(),
            selected_agents: selected.clone(),
            reason: format!("{} agents selected via {:?} strategy", selected.len(), task.strategy),
            timestamp: Utc::now(),
        };

        let mut log = self.routing_log.write().await;
        log.push(decision);
        if log.len() > self.max_log_size {
            let drain = log.len() - self.max_log_size;
            log.drain(0..drain);
        }

        selected
    }

    fn route_direct(&self, agents: &HashMap<String, AgentRegistration>, task: &A2ATask) -> Vec<String> {
        if let Some(ref target) = task.target_agent {
            if let Some(agent) = agents.get(target) {
                if agent.is_available() {
                    return vec![target.clone()];
                }
            }
        }
        Vec::new()
    }

    fn route_by_capability(&self, agents: &HashMap<String, AgentRegistration>, task: &A2ATask) -> Vec<String> {
        let mut candidates: Vec<(&String, &AgentRegistration)> = agents.iter()
            .filter(|(_, a)| a.is_available() && a.has_capabilities(&task.required_capabilities))
            .collect();

        // Sort by health score (best first), then by load factor (lowest first)
        candidates.sort_by(|a, b| {
            b.1.health_score.partial_cmp(&a.1.health_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.load_factor().partial_cmp(&b.1.load_factor())
                    .unwrap_or(std::cmp::Ordering::Equal))
        });

        candidates.into_iter().take(1).map(|(id, _)| id.clone()).collect()
    }

    fn route_load_balanced(&self, agents: &HashMap<String, AgentRegistration>, _task: &A2ATask) -> Vec<String> {
        let mut candidates: Vec<(&String, &AgentRegistration)> = agents.iter()
            .filter(|(_, a)| a.is_available())
            .collect();

        // Sort by load factor (lowest first), then health score (highest first)
        candidates.sort_by(|a, b| {
            a.1.load_factor().partial_cmp(&b.1.load_factor())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.1.health_score.partial_cmp(&a.1.health_score)
                    .unwrap_or(std::cmp::Ordering::Equal))
        });

        candidates.into_iter().take(1).map(|(id, _)| id.clone()).collect()
    }

    fn route_broadcast(&self, agents: &HashMap<String, AgentRegistration>, task: &A2ATask) -> Vec<String> {
        agents.iter()
            .filter(|(_, a)| a.is_available())
            .filter(|(_, a)| {
                task.required_capabilities.is_empty() || a.has_capabilities(&task.required_capabilities)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn route_chain(&self, agents: &HashMap<String, AgentRegistration>, task: &A2ATask) -> Vec<String> {
        // Return chain agents in order, filtering out unavailable ones
        task.chain_agents.iter()
            .filter(|id| agents.get(*id).map_or(false, |a| a.is_available()))
            .cloned()
            .collect()
    }

    /// Update agent heartbeat
    pub async fn heartbeat(&self, agent_id: &str, health_score: f64, current_tasks: u32) {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.last_heartbeat = Utc::now();
            agent.health_score = health_score;
            agent.current_tasks = current_tasks;
        }
    }

    /// Get all registered agents
    pub async fn list_agents(&self) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get available agents (not overloaded, healthy)
    pub async fn available_agents(&self) -> Vec<AgentRegistration> {
        let agents = self.agents.read().await;
        agents.values().filter(|a| a.is_available()).cloned().collect()
    }

    /// Get routing log
    pub async fn routing_log(&self) -> Vec<RoutingDecision> {
        let log = self.routing_log.read().await;
        log.clone()
    }

    /// Get agent count
    pub async fn agent_count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Get available agent count
    pub async fn available_count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.values().filter(|a| a.is_available()).count()
    }

    /// Clear stale agents (no heartbeat for timeout_secs)
    pub async fn evict_stale(&self, timeout_secs: i64) -> Vec<String> {
        let now = Utc::now();
        let mut agents = self.agents.write().await;
        let stale: Vec<String> = agents.iter()
            .filter(|(_, a)| (now - a.last_heartbeat).num_seconds() > timeout_secs)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &stale {
            agents.remove(id);
            tracing::warn!(agent_id = %id, "Evicted stale agent from A2A router");
        }

        stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_agent(id: &str, caps: Vec<String>) -> AgentRegistration {
        AgentRegistration::new(id, caps, &format!("http://localhost:808{}", id.chars().last().unwrap_or('0')))
    }

    fn make_task(id: &str) -> A2ATask {
        A2ATask::new(id, "code", json!({"prompt": "hello"}))
    }

    #[test]
    fn test_task_creation() {
        let task = make_task("t1");
        assert_eq!(task.task_id, "t1");
        assert_eq!(task.strategy, RoutingStrategy::Capability);
        assert_eq!(task.priority, TaskPriority::Normal);
    }

    #[test]
    fn test_task_builder() {
        let task = make_task("t1")
            .with_target("agent-1")
            .with_priority(TaskPriority::High)
            .with_timeout(60_000);
        assert_eq!(task.strategy, RoutingStrategy::Direct);
        assert_eq!(task.target_agent, Some("agent-1".to_string()));
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.timeout_ms, 60_000);
    }

    #[test]
    fn test_task_chain() {
        let task = make_task("t1").with_chain(vec!["a1".into(), "a2".into(), "a3".into()]);
        assert_eq!(task.strategy, RoutingStrategy::Chain);
        assert_eq!(task.chain_agents.len(), 3);
    }

    #[test]
    fn test_agent_registration() {
        let agent = make_agent("a1", vec!["code".into(), "review".into()]);
        assert!(agent.is_available());
        assert!(agent.has_capabilities(&["code".into()]));
        assert!(!agent.has_capabilities(&["deploy".into()]));
    }

    #[test]
    fn test_agent_load_factor() {
        let mut agent = make_agent("a1", vec![]);
        agent.max_concurrent_tasks = 10;
        agent.current_tasks = 5;
        assert!((agent.load_factor() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_agent_unavailable_when_overloaded() {
        let mut agent = make_agent("a1", vec![]);
        agent.max_concurrent_tasks = 5;
        agent.current_tasks = 5;
        assert!(!agent.is_available());
    }

    #[test]
    fn test_agent_unavailable_when_unhealthy() {
        let mut agent = make_agent("a1", vec![]);
        agent.health_score = 0.1;
        assert!(!agent.is_available());
    }

    #[tokio::test]
    async fn test_router_register_unregister() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        assert_eq!(router.agent_count().await, 1);
        router.unregister_agent("a1").await;
        assert_eq!(router.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_route_direct() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        router.register_agent(make_agent("a2", vec!["code".into()])).await;

        let task = make_task("t1").with_target("a1");
        let selected = router.route(&task).await;
        assert_eq!(selected, vec!["a1"]);
    }

    #[tokio::test]
    async fn test_route_direct_unavailable() {
        let router = A2ARouter::new();
        let mut agent = make_agent("a1", vec!["code".into()]);
        agent.current_tasks = 100;
        agent.max_concurrent_tasks = 5;
        router.register_agent(agent).await;

        let task = make_task("t1").with_target("a1");
        let selected = router.route(&task).await;
        assert!(selected.is_empty());
    }

    #[tokio::test]
    async fn test_route_by_capability() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into(), "review".into()])).await;
        router.register_agent(make_agent("a2", vec!["deploy".into()])).await;

        let task = make_task("t1").with_capabilities(vec!["code".into()]);
        let selected = router.route(&task).await;
        assert_eq!(selected, vec!["a1"]);
    }

    #[tokio::test]
    async fn test_route_by_capability_no_match() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;

        let task = make_task("t1").with_capabilities(vec!["deploy".into()]);
        let selected = router.route(&task).await;
        assert!(selected.is_empty());
    }

    #[tokio::test]
    async fn test_route_load_balanced() {
        let router = A2ARouter::new();
        let mut a1 = make_agent("a1", vec!["code".into()]);
        a1.current_tasks = 8;
        a1.max_concurrent_tasks = 10;
        router.register_agent(a1).await;

        let a2 = make_agent("a2", vec!["code".into()]);
        router.register_agent(a2).await;

        let task = make_task("t1").with_strategy(RoutingStrategy::LoadBalanced);
        let selected = router.route(&task).await;
        assert_eq!(selected, vec!["a2"]); // a2 has lower load
    }

    #[tokio::test]
    async fn test_route_broadcast() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        router.register_agent(make_agent("a2", vec!["code".into()])).await;
        router.register_agent(make_agent("a3", vec!["code".into()])).await;

        let task = make_task("t1").with_strategy(RoutingStrategy::Broadcast);
        let selected = router.route(&task).await;
        assert_eq!(selected.len(), 3);
    }

    #[tokio::test]
    async fn test_route_chain() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        router.register_agent(make_agent("a2", vec!["review".into()])).await;
        router.register_agent(make_agent("a3", vec!["deploy".into()])).await;

        let task = make_task("t1").with_chain(vec!["a1".into(), "a2".into(), "a3".into()]);
        let selected = router.route(&task).await;
        assert_eq!(selected, vec!["a1", "a2", "a3"]);
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        router.heartbeat("a1", 0.95, 3).await;
        let agents = router.list_agents().await;
        assert!((agents[0].health_score - 0.95).abs() < f64::EPSILON);
        assert_eq!(agents[0].current_tasks, 3);
    }

    #[tokio::test]
    async fn test_routing_log() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;

        let task = make_task("t1");
        router.route(&task).await;

        let log = router.routing_log().await;
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].task_id, "t1");
    }

    #[tokio::test]
    async fn test_evict_stale() {
        let router = A2ARouter::new();
        let mut agent = make_agent("a1", vec!["code".into()]);
        agent.last_heartbeat = Utc::now() - chrono::Duration::seconds(600);
        router.register_agent(agent).await;

        let evicted = router.evict_stale(300).await;
        assert_eq!(evicted, vec!["a1"]);
        assert_eq!(router.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_available_agents() {
        let router = A2ARouter::new();
        router.register_agent(make_agent("a1", vec!["code".into()])).await;
        let mut a2 = make_agent("a2", vec!["code".into()]);
        a2.current_tasks = 100;
        a2.max_concurrent_tasks = 5;
        router.register_agent(a2).await;

        let available = router.available_agents().await;
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].agent_id, "a1");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }
}
