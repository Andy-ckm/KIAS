//! # Crew Orchestrator
//!
//! CrewAI-inspired multi-agent orchestration with delegation, memory,
//! and intelligent task routing.
//!
//! A `Crew` coordinates multiple agents through structured delegation
//! protocols, shared memory, and skill-based routing.
//!
//! ## Process Modes
//!
//! - **Sequential**: Tasks executed one after another, output feeds next
//! - **Hierarchical**: Manager agent delegates to workers, workers may
//!   sub-delegate to specialists
//!
//! ## Key Innovation: Autonomous Delegation
//!
//! Unlike traditional orchestrators that pre-assign tasks, Crew agents
//! can autonomously decide to delegate work to better-suited peers.
//! The delegation decision is based on:
//! 1. Agent's self-assessment of capability
//! 2. Skill matcher recommendations
//! 3. Historical success rates
//! 4. Current load balancing

use chrono::Utc;
use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::delegation::*;
use super::memory::*;
use super::skill_matcher::*;

/// Execution process mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessMode {
    /// Execute tasks sequentially, output of one feeds next
    Sequential,
    /// Hierarchical delegation with a manager agent
    Hierarchical,
}

/// Crew configuration
#[derive(Debug, Clone)]
pub struct CrewConfig {
    /// Process mode
    pub process_mode: ProcessMode,
    /// Maximum delegation depth (prevents infinite delegation chains)
    pub max_delegation_depth: u32,
    /// Maximum parallel tasks
    pub max_parallel_tasks: usize,
    /// Timeout for a single task (seconds)
    pub task_timeout_secs: u64,
    /// Enable shared memory across agents
    pub enable_shared_memory: bool,
    /// Enable autonomous delegation (agents can delegate to each other)
    pub enable_autonomous_delegation: bool,
}

impl Default for CrewConfig {
    fn default() -> Self {
        Self {
            process_mode: ProcessMode::Sequential,
            max_delegation_depth: 3,
            max_parallel_tasks: 5,
            task_timeout_secs: 300,
            enable_shared_memory: true,
            enable_autonomous_delegation: true,
        }
    }
}

/// A registered agent in the crew
#[derive(Debug, Clone)]
pub struct CrewAgent {
    /// Agent profile for skill matching
    pub profile: AgentProfile,
    /// Agent's delegation capability (can delegate to others)
    pub can_delegate: bool,
    /// Agent's delegation depth (how deep can its delegation chain go)
    pub delegation_depth: u32,
}

/// Crew execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewResult {
    /// Final output
    pub output: String,
    /// Per-task results
    pub task_results: Vec<TaskExecutionResult>,
    /// Delegation records
    pub delegations: Vec<DelegationRecord>,
    /// Total execution time in milliseconds
    pub total_duration_ms: u64,
    /// Whether all tasks succeeded
    pub all_succeeded: bool,
}

/// Result of a single task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    /// Task name/description
    pub task_name: String,
    /// Which agent executed it
    pub agent_id: String,
    /// Whether it was delegated
    pub was_delegated: bool,
    /// Output
    pub output: String,
    /// Whether it succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// A task definition for the crew
#[derive(Debug, Clone)]
pub struct CrewTask {
    /// Task name
    pub name: String,
    /// Task description
    pub description: String,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Expected output format/description
    pub expected_output: Option<String>,
    /// Context from previous tasks
    pub context: serde_json::Value,
    /// Specific agent to assign (None = auto-select)
    pub assigned_agent: Option<String>,
}

/// Trait for task execution - allows pluggable backends
#[async_trait::async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Execute a task with given context and return the result
    async fn execute(&self, agent_id: &str, task: &CrewTask, context: &str) -> KiasResult<String>;
}

/// Simple mock executor for testing
pub struct MockExecutor {
    responses: HashMap<String, String>,
}

impl MockExecutor {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    pub fn with_response(mut self, task_name: &str, response: &str) -> Self {
        self.responses
            .insert(task_name.to_string(), response.to_string());
        self
    }
}

impl Default for MockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TaskExecutor for MockExecutor {
    async fn execute(
        &self,
        _agent_id: &str,
        task: &CrewTask,
        _context: &str,
    ) -> KiasResult<String> {
        if let Some(response) = self.responses.get(&task.name) {
            Ok(response.clone())
        } else {
            Ok(format!("Executed: {}", task.description))
        }
    }
}

/// The Crew - main orchestrator
pub struct Crew {
    /// Crew name
    name: String,
    /// Configuration
    config: CrewConfig,
    /// Registered agents
    agents: Arc<RwLock<HashMap<String, CrewAgent>>>,
    /// Shared memory
    memory: MemoryManager,
    /// Skill matcher
    matcher: SkillMatcher,
    /// Delegation history
    delegation_history: Arc<RwLock<Vec<DelegationRecord>>>,
}

impl Crew {
    /// Create a new crew
    pub fn new(name: &str, config: CrewConfig) -> Self {
        let matcher_config = MatcherConfig {
            require_all_capabilities: false,
            ..MatcherConfig::default()
        };
        Self {
            name: name.to_string(),
            config,
            agents: Arc::new(RwLock::new(HashMap::new())),
            memory: MemoryManager::default(),
            matcher: SkillMatcher::new(matcher_config),
            delegation_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an agent with the crew
    pub async fn register_agent(&self, agent: CrewAgent) {
        let mut agents = self.agents.write().await;
        agents.insert(agent.profile.agent_id.clone(), agent);
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) -> bool {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id).is_some()
    }

    /// Execute a list of tasks using the configured process mode
    pub async fn execute(
        &self,
        tasks: Vec<CrewTask>,
        executor: &dyn TaskExecutor,
    ) -> KiasResult<CrewResult> {
        let start = std::time::Instant::now();
        tracing::info!(
            crew = %self.name,
            task_count = tasks.len(),
            mode = ?self.config.process_mode,
            "Crew execution started"
        );

        let task_results = match self.config.process_mode {
            ProcessMode::Sequential => self.execute_sequential(tasks, executor).await?,
            ProcessMode::Hierarchical => self.execute_hierarchical(tasks, executor).await?,
        };

        let all_succeeded = task_results.iter().all(|r| r.success);
        let output = task_results
            .iter()
            .map(|r| r.output.clone())
            .collect::<Vec<_>>()
            .join("\n\n");

        let delegations = {
            let history = self.delegation_history.read().await;
            history.clone()
        };

        let result = CrewResult {
            output,
            task_results,
            delegations,
            total_duration_ms: start.elapsed().as_millis() as u64,
            all_succeeded,
        };

        tracing::info!(
            crew = %self.name,
            duration_ms = result.total_duration_ms,
            all_succeeded = result.all_succeeded,
            "Crew execution completed"
        );

        Ok(result)
    }

    /// Sequential execution: tasks run one after another
    async fn execute_sequential(
        &self,
        tasks: Vec<CrewTask>,
        executor: &dyn TaskExecutor,
    ) -> KiasResult<Vec<TaskExecutionResult>> {
        let mut results = Vec::new();
        let mut accumulated_context = String::new();

        for task in tasks {
            let start = std::time::Instant::now();

            // Select agent for this task
            let agent_id = self.select_agent(&task).await?;

            // Build context from previous results
            let task_context = if accumulated_context.is_empty() {
                serde_json::to_string(&task.context).unwrap_or_default()
            } else {
                format!(
                    "{}\n\nPrevious results:\n{}",
                    serde_json::to_string(&task.context).unwrap_or_default(),
                    accumulated_context
                )
            };

            // Execute
            let output = executor.execute(&agent_id, &task, &task_context).await?;
            let duration_ms = start.elapsed().as_millis() as u64;

            // Store in shared memory
            if self.config.enable_shared_memory {
                let mut stm = self.memory.short_term.write().await;
                stm.store(
                    &agent_id,
                    &format!("[{}]: {}", task.name, output),
                    vec![task.name.clone()],
                );
            }

            accumulated_context.push_str(&format!("\n--- {} ---\n{}\n", task.name, output));

            results.push(TaskExecutionResult {
                task_name: task.name,
                agent_id,
                was_delegated: false,
                output,
                success: true,
                duration_ms,
            });
        }

        Ok(results)
    }

    /// Hierarchical execution: manager delegates to workers
    async fn execute_hierarchical(
        &self,
        tasks: Vec<CrewTask>,
        executor: &dyn TaskExecutor,
    ) -> KiasResult<Vec<TaskExecutionResult>> {
        let mut results = Vec::new();

        for task in tasks {
            let start = std::time::Instant::now();

            // Check if a specific agent is assigned
            let selected_agent = if let Some(ref assigned) = task.assigned_agent {
                assigned.clone()
            } else {
                self.select_agent(&task).await?
            };

            // Check if this agent should delegate
            let (final_agent, was_delegated) = if self.config.enable_autonomous_delegation {
                self.maybe_delegate(&task, &selected_agent).await?
            } else {
                (selected_agent, false)
            };

            // Build context from shared memory
            let context = self.build_context_for_agent(&final_agent, &task).await;

            // Execute
            let output = executor.execute(&final_agent, &task, &context).await?;
            let duration_ms = start.elapsed().as_millis() as u64;

            // Store result in memory
            if self.config.enable_shared_memory {
                let mut stm = self.memory.short_term.write().await;
                stm.store(
                    &final_agent,
                    &format!("[{}]: {}", task.name, output),
                    vec![task.name.clone()],
                );
            }

            results.push(TaskExecutionResult {
                task_name: task.name,
                agent_id: final_agent,
                was_delegated,
                output,
                success: true,
                duration_ms,
            });
        }

        Ok(results)
    }

    /// Select the best agent for a task based on capabilities
    async fn select_agent(&self, task: &CrewTask) -> KiasResult<String> {
        let agents = self.agents.read().await;
        let profiles: Vec<AgentProfile> = agents.values().map(|a| a.profile.clone()).collect();

        if profiles.is_empty() {
            return Err(kias_common::KiasError::Scheduler(
                "No agents registered in crew".to_string(),
            ));
        }

        if task.required_capabilities.is_empty() {
            // Round-robin if no specific capabilities needed
            return Ok(profiles[0].agent_id.clone());
        }

        let best = self
            .matcher
            .find_best(&profiles, &task.required_capabilities);

        match best {
            Some(match_result) => Ok(match_result.agent_id),
            None => {
                // Fall back to first available agent
                profiles
                    .iter()
                    .find(|p| p.available)
                    .map(|p| p.agent_id.clone())
                    .ok_or_else(|| {
                        kias_common::KiasError::Scheduler(
                            "No available agents for task".to_string(),
                        )
                    })
            }
        }
    }

    /// Consider delegating a task to a better-suited agent
    async fn maybe_delegate(
        &self,
        task: &CrewTask,
        current_agent_id: &str,
    ) -> KiasResult<(String, bool)> {
        let agents = self.agents.read().await;

        let current_agent = match agents.get(current_agent_id) {
            Some(agent) => agent.clone(),
            None => return Ok((current_agent_id.to_string(), false)),
        };

        // Only delegate if the current agent can delegate
        if !current_agent.can_delegate {
            return Ok((current_agent_id.to_string(), false));
        }

        // Check if current agent has all required capabilities
        let has_all_caps = task
            .required_capabilities
            .iter()
            .all(|cap| current_agent.profile.capabilities.contains_key(cap));

        if has_all_caps {
            // Current agent can handle it, no need to delegate
            return Ok((current_agent_id.to_string(), false));
        }

        // Find a better agent
        let profiles: Vec<AgentProfile> = agents.values().map(|a| a.profile.clone()).collect();
        drop(agents);

        let best = self
            .matcher
            .find_best(&profiles, &task.required_capabilities);

        if let Some(match_result) = best {
            if match_result.agent_id != current_agent_id
                && match_result.all_capabilities_met
                && match_result.score > 0.5
            {
                // Record the delegation
                let delegation = DelegateRequest {
                    delegation_id: uuid::Uuid::new_v4().to_string(),
                    from_agent: current_agent_id.to_string(),
                    to_agent: match_result.agent_id.clone(),
                    task_description: task.description.clone(),
                    required_capabilities: task.required_capabilities.clone(),
                    context: task.context.clone(),
                    priority: DelegationPriority::Normal,
                    timeout_secs: self.config.task_timeout_secs,
                    max_retries: 1,
                    created_at: Utc::now(),
                };

                let mut record = DelegationRecord::from_request(delegation);
                record.state = DelegationState::Accepted;

                let mut history = self.delegation_history.write().await;
                history.push(record);

                tracing::info!(
                    from = %current_agent_id,
                    to = %match_result.agent_id,
                    task = %task.name,
                    score = match_result.score,
                    "Task delegated"
                );

                return Ok((match_result.agent_id, true));
            }
        }

        Ok((current_agent_id.to_string(), false))
    }

    /// Build context from shared memory for an agent
    async fn build_context_for_agent(&self, agent_id: &str, task: &CrewTask) -> String {
        let mut context = String::new();

        // Add task context
        if let Some(ref expected) = task.expected_output {
            context.push_str(&format!("Expected output: {}\n\n", expected));
        }

        // Search short-term memory for relevant context
        if self.config.enable_shared_memory {
            let mut stm = self.memory.short_term.write().await;
            let memories = stm.search(&task.name, 5);
            if !memories.is_empty() {
                context.push_str("Previous results:\n");
                for mem in &memories {
                    if mem.agent_id != agent_id {
                        context.push_str(&format!("{}\n", mem.content));
                    }
                }
            }
        }

        // Add entity memory facts about this agent
        let em = self.memory.entity.read().await;
        let facts = em.get_facts(agent_id);
        if !facts.is_empty() {
            context.push_str("\nAgent knowledge:\n");
            for fact in &facts {
                context.push_str(&format!("[{}] {}\n", fact.fact_type, fact.content));
            }
        }

        context
    }

    /// Get crew statistics
    pub async fn stats(&self) -> CrewStats {
        let agents = self.agents.read().await;
        let delegations = self.delegation_history.read().await;

        CrewStats {
            name: self.name.clone(),
            total_agents: agents.len(),
            available_agents: agents.values().filter(|a| a.profile.available).count(),
            total_delegations: delegations.len(),
            successful_delegations: delegations
                .iter()
                .filter(|d| {
                    d.state == DelegationState::Completed || d.state == DelegationState::Accepted
                })
                .count(),
        }
    }
}

/// Crew statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewStats {
    pub name: String,
    pub total_agents: usize,
    pub available_agents: usize,
    pub total_delegations: usize,
    pub successful_delegations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_crew_agent(id: &str, name: &str, caps: Vec<(&str, f32)>) -> CrewAgent {
        let mut profile = AgentProfile::new(id, name);
        for (cap, prof) in caps {
            profile = profile.with_capability(cap, prof);
        }
        CrewAgent {
            profile,
            can_delegate: true,
            delegation_depth: 3,
        }
    }

    async fn setup_crew() -> Crew {
        let crew = Crew::new("test-crew", CrewConfig::default());
        crew.register_agent(make_crew_agent(
            "coder",
            "Code Expert",
            vec![("code_generation", 0.95), ("testing", 0.7)],
        ))
        .await;
        crew.register_agent(make_crew_agent(
            "researcher",
            "Research Specialist",
            vec![("web_search", 0.9), ("summarization", 0.8)],
        ))
        .await;
        crew
    }

    #[tokio::test]
    async fn test_crew_create() {
        let crew = Crew::new("test", CrewConfig::default());
        let stats = crew.stats().await;
        assert_eq!(stats.total_agents, 0);
    }

    #[tokio::test]
    async fn test_crew_register_agent() {
        let crew = setup_crew().await;
        let stats = crew.stats().await;
        assert_eq!(stats.total_agents, 2);
    }

    #[tokio::test]
    async fn test_crew_unregister_agent() {
        let crew = setup_crew().await;
        assert!(crew.unregister_agent("coder").await);
        let stats = crew.stats().await;
        assert_eq!(stats.total_agents, 1);
    }

    #[tokio::test]
    async fn test_crew_unregister_nonexistent() {
        let crew = setup_crew().await;
        assert!(!crew.unregister_agent("nobody").await);
    }

    #[tokio::test]
    async fn test_crew_sequential_execution() {
        let crew = setup_crew().await;
        let executor = MockExecutor::new()
            .with_response("analyze", "Analysis complete")
            .with_response("summarize", "Summary done");

        let tasks = vec![
            CrewTask {
                name: "analyze".to_string(),
                description: "Analyze the codebase".to_string(),
                required_capabilities: vec!["code_generation".to_string()],
                expected_output: None,
                context: serde_json::json!({}),
                assigned_agent: None,
            },
            CrewTask {
                name: "summarize".to_string(),
                description: "Summarize findings".to_string(),
                required_capabilities: vec!["summarization".to_string()],
                expected_output: None,
                context: serde_json::json!({}),
                assigned_agent: None,
            },
        ];

        let result = crew.execute(tasks, &executor).await.unwrap();
        assert!(result.all_succeeded);
        assert_eq!(result.task_results.len(), 2);
    }

    #[tokio::test]
    async fn test_crew_hierarchical_execution() {
        let config = CrewConfig {
            process_mode: ProcessMode::Hierarchical,
            ..CrewConfig::default()
        };
        let crew = Crew::new("hierarchical-crew", config);
        crew.register_agent(make_crew_agent(
            "coder",
            "Coder",
            vec![("code_generation", 0.9)],
        ))
        .await;

        let executor = MockExecutor::new();
        let tasks = vec![CrewTask {
            name: "code".to_string(),
            description: "Write code".to_string(),
            required_capabilities: vec!["code_generation".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        }];

        let result = crew.execute(tasks, &executor).await.unwrap();
        assert!(result.all_succeeded);
    }

    #[tokio::test]
    async fn test_crew_specific_agent_assignment() {
        let crew = setup_crew().await;
        let executor = MockExecutor::new();

        let tasks = vec![CrewTask {
            name: "research".to_string(),
            description: "Research topic".to_string(),
            required_capabilities: vec!["web_search".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: Some("researcher".to_string()),
        }];

        let result = crew.execute(tasks, &executor).await.unwrap();
        assert!(result.all_succeeded);
        assert_eq!(result.task_results[0].agent_id, "researcher");
    }

    #[tokio::test]
    async fn test_crew_empty_tasks() {
        let crew = setup_crew().await;
        let executor = MockExecutor::new();
        let result = crew.execute(vec![], &executor).await.unwrap();
        assert!(result.all_succeeded);
        assert_eq!(result.task_results.len(), 0);
    }

    #[tokio::test]
    async fn test_crew_no_agents_fails() {
        let crew = Crew::new("empty", CrewConfig::default());
        let executor = MockExecutor::new();
        let tasks = vec![CrewTask {
            name: "t".to_string(),
            description: "d".to_string(),
            required_capabilities: vec!["cap".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        }];

        let result = crew.execute(tasks, &executor).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_crew_stats() {
        let crew = setup_crew().await;
        let stats = crew.stats().await;
        assert_eq!(stats.name, "test-crew");
        assert_eq!(stats.total_agents, 2);
        assert_eq!(stats.available_agents, 2);
    }

    #[tokio::test]
    async fn test_crew_shared_memory_stores_results() {
        let crew = setup_crew().await;
        let executor = MockExecutor::new().with_response("task1", "result1");

        let tasks = vec![CrewTask {
            name: "task1".to_string(),
            description: "do something".to_string(),
            required_capabilities: vec![],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        }];

        crew.execute(tasks, &executor).await.unwrap();

        // Check memory was stored
        let mut stm = crew.memory.short_term.write().await;
        let memories = stm.search("task1", 10);
        assert_eq!(memories.len(), 1);
        assert!(memories[0].content.contains("result1"));
    }

    #[tokio::test]
    async fn test_crew_delegation_to_better_agent() {
        let crew = setup_crew().await;

        // Coder doesn't have web_search, should delegate to researcher
        let task = CrewTask {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            required_capabilities: vec!["web_search".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        };

        let (agent, was_delegated) = crew.maybe_delegate(&task, "coder").await.unwrap();
        assert_eq!(agent, "researcher");
        assert!(was_delegated);
    }

    #[tokio::test]
    async fn test_crew_no_delegation_when_capable() {
        let crew = setup_crew().await;

        // Coder has code_generation, no need to delegate
        let task = CrewTask {
            name: "code".to_string(),
            description: "Write code".to_string(),
            required_capabilities: vec!["code_generation".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        };

        let (agent, was_delegated) = crew.maybe_delegate(&task, "coder").await.unwrap();
        assert_eq!(agent, "coder");
        assert!(!was_delegated);
    }

    #[tokio::test]
    async fn test_crew_delegation_disabled() {
        let config = CrewConfig {
            enable_autonomous_delegation: false,
            ..CrewConfig::default()
        };
        let crew = Crew::new("no-delegation", config);
        crew.register_agent(make_crew_agent(
            "coder",
            "Coder",
            vec![("code_generation", 0.9)],
        ))
        .await;
        crew.register_agent(make_crew_agent(
            "researcher",
            "Researcher",
            vec![("web_search", 0.9)],
        ))
        .await;

        let executor = MockExecutor::new();
        let tasks = vec![CrewTask {
            name: "search".to_string(),
            description: "Search".to_string(),
            required_capabilities: vec!["web_search".to_string()],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        }];

        let result = crew.execute(tasks, &executor).await.unwrap();
        // With delegation disabled, coder gets assigned (first agent) and just runs it
        assert!(result.all_succeeded);
    }

    #[tokio::test]
    async fn test_mock_executor_custom_response() {
        let executor = MockExecutor::new().with_response("test", "custom result");
        let task = CrewTask {
            name: "test".to_string(),
            description: "test".to_string(),
            required_capabilities: vec![],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        };
        let result = executor.execute("a1", &task, "").await.unwrap();
        assert_eq!(result, "custom result");
    }

    #[tokio::test]
    async fn test_mock_executor_default_response() {
        let executor = MockExecutor::new();
        let task = CrewTask {
            name: "unknown".to_string(),
            description: "some task".to_string(),
            required_capabilities: vec![],
            expected_output: None,
            context: serde_json::json!({}),
            assigned_agent: None,
        };
        let result = executor.execute("a1", &task, "").await.unwrap();
        assert!(result.contains("some task"));
    }

    #[test]
    fn test_crew_config_default() {
        let config = CrewConfig::default();
        assert_eq!(config.process_mode, ProcessMode::Sequential);
        assert_eq!(config.max_delegation_depth, 3);
        assert!(config.enable_shared_memory);
        assert!(config.enable_autonomous_delegation);
    }

    #[test]
    fn test_process_mode_variants() {
        assert_ne!(ProcessMode::Sequential, ProcessMode::Hierarchical);
    }
}
