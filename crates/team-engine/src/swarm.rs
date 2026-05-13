//! # Swarm Orchestrator
//!
//! Lightweight multi-agent parallel orchestration inspired by OpenAI Swarm and golutra.
//! Provides a simple but powerful pattern for distributing work across multiple agents
//! with automatic handoff, parallel execution, and result aggregation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Swarm agent identifier
pub type SwarmAgentId = String;

/// Task identifier
pub type SwarmTaskId = String;

/// Execution strategy for the swarm
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwarmStrategy {
    /// Fan-out: distribute work to all agents, collect all results
    FanOut,
    /// Pipeline: chain agents in sequence, output of one feeds next
    Pipeline,
    /// Race: send to multiple agents, use first result
    Race,
    /// MapReduce: split work, process in parallel, reduce results
    MapReduce,
}

/// Agent capability descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    /// Agent ID
    pub agent_id: SwarmAgentId,
    /// Agent name
    pub name: String,
    /// Capabilities/skills this agent has
    pub capabilities: Vec<String>,
    /// Current load (0.0 = idle, 1.0 = fully loaded)
    pub load: f64,
    /// Whether the agent is available
    pub available: bool,
}

/// A swarm task to be distributed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    /// Task ID
    pub id: SwarmTaskId,
    /// Task description
    pub description: String,
    /// Task payload
    pub payload: serde_json::Value,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Execution strategy
    pub strategy: SwarmStrategy,
    /// Maximum retries
    pub max_retries: u32,
    /// Timeout per agent execution
    pub timeout_ms: u64,
}

/// Result from a single agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Which agent produced this result
    pub agent_id: SwarmAgentId,
    /// The result payload
    pub output: serde_json::Value,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Aggregated swarm result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmResult {
    /// Task ID
    pub task_id: SwarmTaskId,
    /// Results from individual agents
    pub agent_results: Vec<AgentResult>,
    /// Final aggregated output
    pub output: serde_json::Value,
    /// Whether all agents succeeded
    pub all_succeeded: bool,
    /// Total execution time
    pub total_duration_ms: u64,
    /// Strategy used
    pub strategy: SwarmStrategy,
}

/// Swarm orchestrator - coordinates multi-agent work
pub struct SwarmOrchestrator {
    /// Registered agents
    agents: Arc<RwLock<HashMap<SwarmAgentId, AgentCapability>>>,
    /// Task execution history
    history: Arc<RwLock<Vec<SwarmResult>>>,
}

impl SwarmOrchestrator {
    /// Create a new swarm orchestrator
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an agent with the swarm
    pub async fn register_agent(&self, capability: AgentCapability) {
        let mut agents = self.agents.write().await;
        agents.insert(capability.agent_id.clone(), capability);
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) -> bool {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id).is_some()
    }

    /// Get available agents matching required capabilities
    pub async fn find_capable_agents(&self, required: &[String]) -> Vec<AgentCapability> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|a| a.available && required.iter().all(|cap| a.capabilities.contains(cap)))
            .cloned()
            .collect()
    }

    /// Get all registered agents
    pub async fn list_agents(&self) -> Vec<AgentCapability> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Execute a task using the specified strategy
    pub async fn execute(
        &self,
        task: SwarmTask,
        executor: &dyn SwarmExecutor,
    ) -> Result<SwarmResult, String> {
        let start = std::time::Instant::now();

        // Find capable agents
        let capable_agents = self.find_capable_agents(&task.required_capabilities).await;

        if capable_agents.is_empty() {
            return Err("No capable agents available".to_string());
        }

        let agent_results = match task.strategy {
            SwarmStrategy::FanOut => {
                self.execute_fan_out(&task, &capable_agents, executor).await
            }
            SwarmStrategy::Pipeline => {
                self.execute_pipeline(&task, &capable_agents, executor).await
            }
            SwarmStrategy::Race => {
                self.execute_race(&task, &capable_agents, executor).await
            }
            SwarmStrategy::MapReduce => {
                self.execute_map_reduce(&task, &capable_agents, executor).await
            }
        };

        let all_succeeded = agent_results.iter().all(|r| r.success);

        // Aggregate results
        let output = if all_succeeded {
            serde_json::json!({
                "results": agent_results.iter().map(|r| &r.output).collect::<Vec<_>>(),
                "count": agent_results.len(),
            })
        } else {
            let errors: Vec<&str> = agent_results
                .iter()
                .filter_map(|r| r.error.as_deref())
                .collect();
            serde_json::json!({
                "errors": errors,
                "partial_results": agent_results.iter().filter(|r| r.success).map(|r| &r.output).collect::<Vec<_>>(),
            })
        };

        let result = SwarmResult {
            task_id: task.id.clone(),
            agent_results,
            output,
            all_succeeded,
            total_duration_ms: start.elapsed().as_millis() as u64,
            strategy: task.strategy.clone(),
        };

        // Store in history
        {
            let mut history = self.history.write().await;
            history.push(result.clone());
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        Ok(result)
    }

    /// Fan-out: execute on all agents in parallel
    async fn execute_fan_out(
        &self,
        task: &SwarmTask,
        agents: &[AgentCapability],
        executor: &dyn SwarmExecutor,
    ) -> Vec<AgentResult> {
        let mut handles = Vec::new();

        for agent in agents {
            let agent_id = agent.agent_id.clone();
            let payload = task.payload.clone();
            let timeout = task.timeout_ms;

            let handle = executor.execute_on_agent(&agent_id, payload, timeout).await;
            handles.push(handle);
        }

        // Await all results
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(AgentResult {
                    agent_id: "unknown".to_string(),
                    output: serde_json::Value::Null,
                    success: false,
                    error: Some(format!("Join error: {}", e)),
                    duration_ms: 0,
                }),
            }
        }

        results
    }

    /// Pipeline: chain agents sequentially
    async fn execute_pipeline(
        &self,
        task: &SwarmTask,
        agents: &[AgentCapability],
        executor: &dyn SwarmExecutor,
    ) -> Vec<AgentResult> {
        let mut results = Vec::new();
        let mut current_payload = task.payload.clone();

        for agent in agents {
            let handle = executor
                .execute_on_agent(&agent.agent_id, current_payload.clone(), task.timeout_ms)
                .await;

            match handle.await {
                Ok(r) => {
                    if r.success {
                        current_payload = r.output.clone();
                    }
                    results.push(r);
                }
                Err(e) => {
                    results.push(AgentResult {
                        agent_id: agent.agent_id.clone(),
                        output: serde_json::Value::Null,
                        success: false,
                        error: Some(format!("Join error: {}", e)),
                        duration_ms: 0,
                    });
                    break;
                }
            }
        }

        results
    }

    /// Race: execute on all agents, return first success
    async fn execute_race(
        &self,
        task: &SwarmTask,
        agents: &[AgentCapability],
        executor: &dyn SwarmExecutor,
    ) -> Vec<AgentResult> {
        let mut handles = Vec::new();

        for agent in agents {
            let handle = executor.execute_on_agent(&agent.agent_id, task.payload.clone(), task.timeout_ms).await;
            handles.push(handle);
        }

        // Return first successful result
        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                if result.success {
                    results.push(result);
                    return results;
                }
                results.push(result);
            }
        }

        results
    }

    /// MapReduce: split work, process in parallel, aggregate
    async fn execute_map_reduce(
        &self,
        task: &SwarmTask,
        agents: &[AgentCapability],
        executor: &dyn SwarmExecutor,
    ) -> Vec<AgentResult> {
        // For MapReduce, we split the payload array across agents
        let items = task
            .payload
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![task.payload.clone()]);

        let chunk_size = items.len().div_ceil(agents.len());
        let mut handles = Vec::new();

        for (i, agent) in agents.iter().enumerate() {
            let chunk: Vec<serde_json::Value> =
                items[i * chunk_size..std::cmp::min((i + 1) * chunk_size, items.len())].to_vec();

            let payload = serde_json::json!({"items": chunk});
            let handle = executor.execute_on_agent(&agent.agent_id, payload, task.timeout_ms).await;
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// Get execution history
    pub async fn history(&self) -> Vec<SwarmResult> {
        let history = self.history.read().await;
        history.clone()
    }

    /// Get swarm statistics
    pub async fn stats(&self) -> SwarmStats {
        let agents = self.agents.read().await;
        let history = self.history.read().await;

        SwarmStats {
            total_agents: agents.len(),
            available_agents: agents.values().filter(|a| a.available).count(),
            total_tasks: history.len(),
            successful_tasks: history.iter().filter(|r| r.all_succeeded).count(),
            failed_tasks: history.iter().filter(|r| !r.all_succeeded).count(),
        }
    }
}

impl Default for SwarmOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Swarm statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwarmStats {
    pub total_agents: usize,
    pub available_agents: usize,
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub failed_tasks: usize,
}

/// Trait for executing tasks on individual agents
/// This allows pluggable execution backends (local, remote, sandbox, etc.)
#[async_trait::async_trait]
pub trait SwarmExecutor: Send + Sync {
    /// Execute a task on a specific agent, returns a handle to the result
    async fn execute_on_agent(
        &self,
        agent_id: &str,
        payload: serde_json::Value,
        timeout_ms: u64,
    ) -> tokio::task::JoinHandle<AgentResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_swarm_creation() {
        let swarm = SwarmOrchestrator::new();
        let agents = swarm.list_agents().await;
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn test_register_agent() {
        let swarm = SwarmOrchestrator::new();
        swarm
            .register_agent(AgentCapability {
                agent_id: "a1".to_string(),
                name: "Agent 1".to_string(),
                capabilities: vec!["code".to_string()],
                load: 0.0,
                available: true,
            })
            .await;

        let agents = swarm.list_agents().await;
        assert_eq!(agents.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_agent() {
        let swarm = SwarmOrchestrator::new();
        swarm
            .register_agent(AgentCapability {
                agent_id: "a1".to_string(),
                name: "Agent 1".to_string(),
                capabilities: vec![],
                load: 0.0,
                available: true,
            })
            .await;

        assert!(swarm.unregister_agent("a1").await);
        assert!(!swarm.unregister_agent("a1").await);
        assert!(swarm.list_agents().await.is_empty());
    }

    #[tokio::test]
    async fn test_find_capable_agents() {
        let swarm = SwarmOrchestrator::new();
        swarm
            .register_agent(AgentCapability {
                agent_id: "a1".to_string(),
                name: "Coder".to_string(),
                capabilities: vec!["code".to_string(), "test".to_string()],
                load: 0.0,
                available: true,
            })
            .await;
        swarm
            .register_agent(AgentCapability {
                agent_id: "a2".to_string(),
                name: "Writer".to_string(),
                capabilities: vec!["write".to_string()],
                load: 0.0,
                available: true,
            })
            .await;

        let coders = swarm.find_capable_agents(&["code".to_string()]).await;
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].agent_id, "a1");

        let writers = swarm.find_capable_agents(&["write".to_string()]).await;
        assert_eq!(writers.len(), 1);

        let all = swarm.find_capable_agents(&[]).await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_find_unavailable_agents() {
        let swarm = SwarmOrchestrator::new();
        swarm
            .register_agent(AgentCapability {
                agent_id: "a1".to_string(),
                name: "Offline".to_string(),
                capabilities: vec!["code".to_string()],
                load: 0.0,
                available: false,
            })
            .await;

        let found = swarm.find_capable_agents(&["code".to_string()]).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn test_swarm_stats() {
        let swarm = SwarmOrchestrator::new();
        swarm
            .register_agent(AgentCapability {
                agent_id: "a1".to_string(),
                name: "A1".to_string(),
                capabilities: vec![],
                load: 0.0,
                available: true,
            })
            .await;

        let stats = swarm.stats().await;
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.available_agents, 1);
        assert_eq!(stats.total_tasks, 0);
    }

    #[test]
    fn test_swarm_strategies() {
        let strategies = vec![
            SwarmStrategy::FanOut,
            SwarmStrategy::Pipeline,
            SwarmStrategy::Race,
            SwarmStrategy::MapReduce,
        ];
        assert_eq!(strategies.len(), 4);
    }

    #[test]
    fn test_agent_capability() {
        let cap = AgentCapability {
            agent_id: "a1".to_string(),
            name: "Test".to_string(),
            capabilities: vec!["code".to_string()],
            load: 0.5,
            available: true,
        };
        assert!(cap.available);
        assert_eq!(cap.capabilities.len(), 1);
    }

    #[test]
    fn test_swarm_task() {
        let task = SwarmTask {
            id: "t1".to_string(),
            description: "Test task".to_string(),
            payload: json!({"data": "test"}),
            required_capabilities: vec!["code".to_string()],
            strategy: SwarmStrategy::FanOut,
            max_retries: 3,
            timeout_ms: 5000,
        };
        assert_eq!(task.strategy, SwarmStrategy::FanOut);
    }

    #[test]
    fn test_swarm_result() {
        let result = SwarmResult {
            task_id: "t1".to_string(),
            agent_results: vec![],
            output: json!(null),
            all_succeeded: true,
            total_duration_ms: 100,
            strategy: SwarmStrategy::FanOut,
        };
        assert!(result.all_succeeded);
    }

    #[test]
    fn test_swarm_stats_default() {
        let stats = SwarmStats::default();
        assert_eq!(stats.total_agents, 0);
    }
}
