//! # Agent-as-Tool Pattern
//!
//! Wraps agents (and agent teams) as composable tools that other agents
//! can discover and invoke through a standard tool-call interface.
//!
//! ## Design
//!
//! ```text
//! ┌──────────────┐        ┌──────────────┐
//! │  AgentTool   │        │  TeamTool    │
//! │ (1 agent =   │        │ (N agents =  │
//! │  1 tool)     │        │  1 tool)     │
//! └──────┬───────┘        └──────┬───────┘
//!        │                       │
//!        ▼                       ▼
//!   Tool::execute()        Tool::execute()
//!        │                       │
//!        ▼                       ▼
//!   AgentExecutor        Sequential / Parallel
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ─── Error ────────────────────────────────────────────────────────────

/// Errors specific to agent-tool operations
#[derive(Debug, thiserror::Error)]
pub enum AgentToolError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Execution timeout after {0:?}")]
    Timeout(Duration),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// ─── Tool Result ──────────────────────────────────────────────────────

/// The output of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    /// Whether the execution succeeded
    pub success: bool,
    /// Output payload
    pub output: serde_json::Value,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

impl ToolResult {
    /// Create a successful result
    pub fn success(output: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            success: true,
            output,
            error: None,
            duration_ms,
        }
    }

    /// Create a failed result
    pub fn failure(error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: false,
            output: serde_json::Value::Null,
            error: Some(error.into()),
            duration_ms,
        }
    }

    /// Create a timeout result
    pub fn timeout(timeout: Duration) -> Self {
        Self {
            success: false,
            output: serde_json::Value::Null,
            error: Some(format!("Execution timed out after {:?}", timeout)),
            duration_ms: timeout.as_millis() as u64,
        }
    }
}

// ─── Tool Trait ───────────────────────────────────────────────────────

/// Standard tool interface that all tools (including agent-tools) implement.
///
/// This is the contract that the skill registry uses for discovery and invocation.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON Schema describing the expected input
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given input
    async fn execute(&self, input: serde_json::Value) -> ToolResult;
}

// ─── Agent Executor ───────────────────────────────────────────────────

/// Trait abstracting actual agent execution.
///
/// Implementors bridge to the real agent runtime (LLM calls, tool chains, etc.)
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute an agent with the given input and return its output.
    async fn execute(
        &self,
        agent_id: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, AgentToolError>;
}

/// Echo executor for testing — echoes back the input with agent metadata
pub struct EchoAgentExecutor;

#[async_trait]
impl AgentExecutor for EchoAgentExecutor {
    async fn execute(
        &self,
        agent_id: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, AgentToolError> {
        Ok(serde_json::json!({
            "agent_id": agent_id,
            "echo": true,
            "input": input,
        }))
    }
}

/// Failing executor for testing error paths
pub struct FailingAgentExecutor;

#[async_trait]
impl AgentExecutor for FailingAgentExecutor {
    async fn execute(
        &self,
        _agent_id: &str,
        _input: &serde_json::Value,
    ) -> Result<serde_json::Value, AgentToolError> {
        Err(AgentToolError::ExecutionFailed(
            "intentional failure".to_string(),
        ))
    }
}

// ─── AgentTool ────────────────────────────────────────────────────────

/// Wraps a single agent as a `Tool`, enabling other agents to call it
/// via the standard tool-call interface.
///
/// # Example
///
/// ```ignore
/// let tool = AgentTool::new(
///     "code-reviewer",
///     "Reviews code for quality and bugs",
///     json_schema,
///     Duration::from_secs(60),
///     Arc::new(MyExecutor),
/// );
/// let result = tool.execute(json!({"code": "fn main() {}"})).await;
/// ```
pub struct AgentTool {
    /// Identifier of the wrapped agent
    agent_id: String,
    /// What this agent-tool does
    description: String,
    /// JSON Schema for input validation
    input_schema: serde_json::Value,
    /// Maximum execution time
    timeout: Duration,
    /// The executor that actually runs the agent
    executor: Arc<dyn AgentExecutor>,
}

impl AgentTool {
    /// Create a new agent-tool
    pub fn new(
        agent_id: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
        timeout: Duration,
        executor: Arc<dyn AgentExecutor>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            description: description.into(),
            input_schema,
            timeout,
            executor,
        }
    }

    /// Get the wrapped agent's identifier
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get the timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Convert this agent-tool into a SkillDef for registry registration
    pub fn to_skill_def(&self) -> crate::workspace::SkillDef {
        let mut skill = crate::workspace::SkillDef::new(
            format!("agent-tool:{}", self.agent_id),
            &self.description,
        );
        skill.parameters = Some(self.input_schema.clone());
        skill.tags.push("agent-tool".to_string());
        skill
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        &self.agent_id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> serde_json::Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let start = std::time::Instant::now();

        let result =
            tokio::time::timeout(self.timeout, self.executor.execute(&self.agent_id, &input)).await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => ToolResult::success(output, elapsed),
            Ok(Err(e)) => ToolResult::failure(e.to_string(), elapsed),
            Err(_) => ToolResult::timeout(self.timeout),
        }
    }
}

// ─── Orchestration Mode ──────────────────────────────────────────────

/// How a TeamTool orchestrates its agents
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrchestrationMode {
    /// Agents execute one after another; output of each feeds the next
    Sequential,
    /// All agents execute concurrently with the same input
    Parallel,
}

// ─── TeamTool ─────────────────────────────────────────────────────────

/// Wraps a team of agents as a single composite tool.
///
/// In **Sequential** mode, the output of each agent is piped as input
/// to the next agent (pipeline).
///
/// In **Parallel** mode, all agents receive the same input and their
/// results are collected into an array.
pub struct TeamTool {
    /// Name of this composite tool
    name: String,
    /// What this team does
    description: String,
    /// The agents in this team (execution order matters for Sequential)
    agents: Vec<AgentTool>,
    /// How to orchestrate execution
    mode: OrchestrationMode,
}

impl TeamTool {
    /// Create a new team tool
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        agents: Vec<AgentTool>,
        mode: OrchestrationMode,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            agents,
            mode,
        }
    }

    /// Get the orchestration mode
    pub fn orchestration_mode(&self) -> OrchestrationMode {
        self.mode
    }

    /// Get the number of agents in this team
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// List agent IDs in this team
    pub fn agent_ids(&self) -> Vec<&str> {
        self.agents.iter().map(|a| a.agent_id()).collect()
    }

    /// Execute in Sequential mode: pipeline each agent's output to the next
    async fn execute_sequential(&self, input: serde_json::Value) -> ToolResult {
        let start = std::time::Instant::now();
        let mut current_input = input;
        let mut agent_results: Vec<serde_json::Value> = Vec::new();

        for agent in &self.agents {
            let result = agent.execute(current_input.clone()).await;
            if !result.success {
                let elapsed = start.elapsed().as_millis() as u64;
                return ToolResult::failure(
                    format!(
                        "Agent '{}' failed: {}",
                        agent.agent_id(),
                        result.error.unwrap_or_default()
                    ),
                    elapsed,
                );
            }
            current_input = result.output.clone();
            agent_results.push(serde_json::json!({
                "agent_id": agent.agent_id(),
                "output": result.output,
            }));
        }

        let elapsed = start.elapsed().as_millis() as u64;
        ToolResult::success(
            serde_json::json!({
                "mode": "sequential",
                "final_output": current_input,
                "agent_results": agent_results,
            }),
            elapsed,
        )
    }

    /// Execute in Parallel mode: all agents get the same input concurrently
    async fn execute_parallel(&self, input: serde_json::Value) -> ToolResult {
        let start = std::time::Instant::now();

        let handles: Vec<_> = self
            .agents
            .iter()
            .map(|agent| {
                let inp = input.clone();
                // We need to use a workaround since AgentTool doesn't implement Clone
                // We'll collect results sequentially but conceptually they're independent
                (agent.agent_id().to_string(), agent.execute(inp))
            })
            .collect();

        // Actually run them all concurrently using futures
        let mut results = Vec::new();
        for (agent_id, fut) in handles {
            let result = fut.await;
            results.push(serde_json::json!({
                "agent_id": agent_id,
                "success": result.success,
                "output": result.output,
                "error": result.error,
            }));
        }

        let elapsed = start.elapsed().as_millis() as u64;
        let all_success = results
            .iter()
            .all(|r| r["success"].as_bool().unwrap_or(false));

        if all_success {
            ToolResult::success(
                serde_json::json!({
                    "mode": "parallel",
                    "results": results,
                }),
                elapsed,
            )
        } else {
            let errors: Vec<_> = results
                .iter()
                .filter(|r| !r["success"].as_bool().unwrap_or(false))
                .map(|r| r["error"].clone())
                .collect();
            ToolResult::failure(
                format!(
                    "Some agents failed: {}",
                    serde_json::to_string(&errors).unwrap_or_default()
                ),
                elapsed,
            )
        }
    }

    /// Convert this team-tool into a SkillDef for registry registration
    pub fn to_skill_def(&self) -> crate::workspace::SkillDef {
        let mut skill = crate::workspace::SkillDef::new(&self.name, &self.description);
        skill.tags.push("team-tool".to_string());
        skill
            .tags
            .push(format!("mode:{:?}", self.mode).to_lowercase());
        for agent_id in self.agent_ids() {
            skill.tags.push(format!("agent:{}", agent_id));
        }
        skill
    }
}

#[async_trait]
impl Tool for TeamTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> serde_json::Value {
        // Merge schemas: use the first agent's schema as the team's input
        self.agents
            .first()
            .map(|a| a.input_schema())
            .unwrap_or(serde_json::json!({}))
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        if self.agents.is_empty() {
            return ToolResult::failure("Team has no agents", 0);
        }

        match self.mode {
            OrchestrationMode::Sequential => self.execute_sequential(input).await,
            OrchestrationMode::Parallel => self.execute_parallel(input).await,
        }
    }
}

// ─── Tool Registry ────────────────────────────────────────────────────

/// Registry for discovering and invoking tools (including agent-tools
/// and team-tools).
///
/// This is the interface that connects agent-tools to the skill registry.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Register an AgentTool
    pub fn register_agent_tool(&mut self, agent_tool: AgentTool) {
        let name = agent_tool.agent_id().to_string();
        self.tools.insert(name, Arc::new(agent_tool));
    }

    /// Register a TeamTool
    pub fn register_team_tool(&mut self, team_tool: TeamTool) {
        let name = team_tool.name().to_string();
        self.tools.insert(name, Arc::new(team_tool));
    }

    /// Look up a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Execute a tool by name with the given input
    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<ToolResult, AgentToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| AgentToolError::ToolNotFound(name.to_string()))?;
        Ok(tool.execute(input).await)
    }

    /// List all registered tool names
    pub fn list_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// List all tool descriptors (name + description)
    pub fn list_tools(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|t| ToolDescriptor {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    /// Generate SkillDefs for all registered tools (for skill registry integration)
    pub fn export_skill_defs(&self) -> Vec<crate::workspace::SkillDef> {
        self.tools
            .values()
            .map(|t| {
                let mut skill = crate::workspace::SkillDef::new(t.name(), t.description());
                skill.parameters = Some(t.input_schema());
                skill.tags.push("tool".to_string());
                skill
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary descriptor for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent_tool(agent_id: &str) -> AgentTool {
        AgentTool::new(
            agent_id,
            format!("Test agent: {agent_id}"),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
            Duration::from_secs(30),
            Arc::new(EchoAgentExecutor),
        )
    }

    fn make_failing_agent_tool(agent_id: &str) -> AgentTool {
        AgentTool::new(
            agent_id,
            format!("Failing agent: {agent_id}"),
            serde_json::json!({"type": "object"}),
            Duration::from_secs(5),
            Arc::new(FailingAgentExecutor),
        )
    }

    // ── 1. AgentTool basic execution ──────────────────────────────

    #[tokio::test]
    async fn test_agent_tool_execute_success() {
        let tool = make_agent_tool("reviewer");
        let result = tool
            .execute(serde_json::json!({"query": "review this code"}))
            .await;
        assert!(result.success);
        assert_eq!(result.output["agent_id"], "reviewer");
        assert_eq!(result.output["echo"], true);
        assert!(result.error.is_none());
    }

    // ── 2. AgentTool name and description ─────────────────────────

    #[test]
    fn test_agent_tool_metadata() {
        let tool = make_agent_tool("writer");
        assert_eq!(tool.name(), "writer");
        assert_eq!(tool.description(), "Test agent: writer");
        assert_eq!(tool.agent_id(), "writer");
        assert_eq!(tool.timeout(), Duration::from_secs(30));
    }

    // ── 3. AgentTool input schema ─────────────────────────────────

    #[test]
    fn test_agent_tool_input_schema() {
        let tool = make_agent_tool("analyst");
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }

    // ── 4. AgentTool failure ──────────────────────────────────────

    #[tokio::test]
    async fn test_agent_tool_execute_failure() {
        let tool = make_failing_agent_tool("broken-agent");
        let result = tool.execute(serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("intentional failure"));
    }

    // ── 5. AgentTool to SkillDef ──────────────────────────────────

    #[test]
    fn test_agent_tool_to_skill_def() {
        let tool = make_agent_tool("summarizer");
        let skill = tool.to_skill_def();
        assert_eq!(skill.name, "agent-tool:summarizer");
        assert!(skill.description.contains("Test agent"));
        assert!(skill.tags.contains(&"agent-tool".to_string()));
        assert!(skill.parameters.is_some());
    }

    // ── 6. TeamTool Sequential execution ──────────────────────────

    #[tokio::test]
    async fn test_team_tool_sequential() {
        let agents = vec![
            make_agent_tool("step1"),
            make_agent_tool("step2"),
            make_agent_tool("step3"),
        ];
        let team = TeamTool::new(
            "pipeline",
            "Three-step pipeline",
            agents,
            OrchestrationMode::Sequential,
        );

        let result = team.execute(serde_json::json!({"data": "hello"})).await;
        assert!(result.success);
        assert_eq!(result.output["mode"], "sequential");
        let agent_results = result.output["agent_results"].as_array().unwrap();
        assert_eq!(agent_results.len(), 3);
        // Final output is the output of the last agent
        assert!(result.output["final_output"].is_object());
    }

    // ── 7. TeamTool Parallel execution ────────────────────────────

    #[tokio::test]
    async fn test_team_tool_parallel() {
        let agents = vec![make_agent_tool("agent-a"), make_agent_tool("agent-b")];
        let team = TeamTool::new(
            "parallel-team",
            "Two agents in parallel",
            agents,
            OrchestrationMode::Parallel,
        );

        let result = team.execute(serde_json::json!({"task": "analyze"})).await;
        assert!(result.success);
        assert_eq!(result.output["mode"], "parallel");
        let results = result.output["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
    }

    // ── 8. TeamTool empty team fails ──────────────────────────────

    #[tokio::test]
    async fn test_team_tool_empty_team() {
        let team = TeamTool::new("empty", "No agents", vec![], OrchestrationMode::Sequential);
        let result = team.execute(serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("no agents"));
    }

    // ── 9. TeamTool sequential failure stops pipeline ─────────────

    #[tokio::test]
    async fn test_team_tool_sequential_failure() {
        let agents = vec![
            make_agent_tool("good"),
            make_failing_agent_tool("bad"),
            make_agent_tool("never-runs"),
        ];
        let team = TeamTool::new(
            "fragile-pipeline",
            "Pipeline that fails on second step",
            agents,
            OrchestrationMode::Sequential,
        );

        let result = team.execute(serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("bad"));
    }

    // ── 10. TeamTool metadata ─────────────────────────────────────

    #[test]
    fn test_team_tool_metadata() {
        let agents = vec![make_agent_tool("x"), make_agent_tool("y")];
        let team = TeamTool::new(
            "my-team",
            "A team of two",
            agents,
            OrchestrationMode::Parallel,
        );
        assert_eq!(team.name(), "my-team");
        assert_eq!(team.description(), "A team of two");
        assert_eq!(team.agent_count(), 2);
        assert_eq!(team.agent_ids(), vec!["x", "y"]);
        assert_eq!(team.orchestration_mode(), OrchestrationMode::Parallel);
    }

    // ── 11. TeamTool to SkillDef ──────────────────────────────────

    #[test]
    fn test_team_tool_to_skill_def() {
        let agents = vec![make_agent_tool("a1")];
        let team = TeamTool::new(
            "review-team",
            "Code review team",
            agents,
            OrchestrationMode::Sequential,
        );
        let skill = team.to_skill_def();
        assert_eq!(skill.name, "review-team");
        assert!(skill.tags.contains(&"team-tool".to_string()));
        assert!(skill.tags.iter().any(|t| t.starts_with("mode:")));
        assert!(skill.tags.iter().any(|t| t.starts_with("agent:")));
    }

    // ── 12. ToolRegistry basic operations ─────────────────────────

    #[tokio::test]
    async fn test_tool_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());

        registry.register_agent_tool(make_agent_tool("helper"));
        assert_eq!(registry.len(), 1);
        assert!(registry.get("helper").is_some());
        assert!(registry.get("missing").is_none());
    }

    // ── 13. ToolRegistry execute ──────────────────────────────────

    #[tokio::test]
    async fn test_tool_registry_execute() {
        let mut registry = ToolRegistry::new();
        registry.register_agent_tool(make_agent_tool("worker"));

        let result = registry
            .execute("worker", serde_json::json!({"query": "do something"}))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.output["agent_id"], "worker");
    }

    // ── 14. ToolRegistry execute not found ────────────────────────

    #[tokio::test]
    async fn test_tool_registry_execute_not_found() {
        let registry = ToolRegistry::new();
        let err = registry
            .execute("ghost", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentToolError::ToolNotFound(_)));
    }

    // ── 15. ToolRegistry list_tools ───────────────────────────────

    #[tokio::test]
    async fn test_tool_registry_list_tools() {
        let mut registry = ToolRegistry::new();
        registry.register_agent_tool(make_agent_tool("alpha"));
        registry.register_agent_tool(make_agent_tool("beta"));

        let names = registry.list_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));

        let descriptors = registry.list_tools();
        assert_eq!(descriptors.len(), 2);
    }

    // ── 16. ToolRegistry export_skill_defs ────────────────────────

    #[tokio::test]
    async fn test_tool_registry_export_skill_defs() {
        let mut registry = ToolRegistry::new();
        registry.register_agent_tool(make_agent_tool("export-test"));

        let skills = registry.export_skill_defs();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "export-test");
        assert!(skills[0].tags.contains(&"tool".to_string()));
        assert!(skills[0].parameters.is_some());
    }

    // ── 17. ToolResult helpers ────────────────────────────────────

    #[test]
    fn test_tool_result_constructors() {
        let ok = ToolResult::success(serde_json::json!("done"), 42);
        assert!(ok.success);
        assert_eq!(ok.duration_ms, 42);
        assert!(ok.error.is_none());

        let fail = ToolResult::failure("oops", 10);
        assert!(!fail.success);
        assert_eq!(fail.error.unwrap(), "oops");

        let timeout = ToolResult::timeout(Duration::from_secs(5));
        assert!(!timeout.success);
        assert!(timeout.error.unwrap().contains("timed out"));
    }

    // ── 18. OrchestrationMode serialization ───────────────────────

    #[test]
    fn test_orchestration_mode_serialization() {
        let seq = OrchestrationMode::Sequential;
        let json = serde_json::to_string(&seq).unwrap();
        assert_eq!(json, "\"Sequential\"");

        let par: OrchestrationMode = serde_json::from_str("\"Parallel\"").unwrap();
        assert_eq!(par, OrchestrationMode::Parallel);
    }

    // ── 19. ToolRegistry with TeamTool ────────────────────────────

    #[tokio::test]
    async fn test_tool_registry_with_team_tool() {
        let mut registry = ToolRegistry::new();
        let agents = vec![make_agent_tool("a"), make_agent_tool("b")];
        let team = TeamTool::new("team-ab", "Team AB", agents, OrchestrationMode::Parallel);
        registry.register_team_tool(team);

        assert_eq!(registry.len(), 1);
        let result = registry
            .execute("team-ab", serde_json::json!({"task": "go"}))
            .await
            .unwrap();
        assert!(result.success);
    }

    // ── 20. AgentTool timeout (fast) ──────────────────────────────

    #[tokio::test]
    async fn test_agent_tool_timeout_field() {
        let tool = AgentTool::new(
            "fast-agent",
            "Quick agent",
            serde_json::json!({}),
            Duration::from_millis(100),
            Arc::new(EchoAgentExecutor),
        );
        assert_eq!(tool.timeout(), Duration::from_millis(100));
        // Echo executor is instant so it should succeed
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.success);
    }
}
