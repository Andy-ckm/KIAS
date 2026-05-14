//! Agent lifecycle state machine and orchestration.
//!
//! The [`AgentLifecycleManager`] manages agent state transitions through a
//! well-defined state machine: Pending → Scheduled → Running → Completed/Failed.
//! State transitions are validated, events are published, and lifecycle hooks
//! are invoked at key transition points.

use anyhow::Result;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::events::{AgentEvent, AgentEventEnvelope, EventBus};

/// Lifecycle states for an agent managed by the controller.
///
/// Transitions form a directed graph:
/// ```text
/// Pending → Scheduled → Running → Completed
///                        Running → Failed → Scheduled (recovery)
///                        Running → Terminated
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleState {
    /// Agent created but not yet scheduled.
    Pending,
    /// Agent scheduled to run on a node.
    Scheduled,
    /// Agent is actively running.
    Running,
    /// Agent completed successfully (terminal).
    Completed,
    /// Agent failed; may be recoverable via Scheduled.
    Failed,
    /// Agent was forcibly terminated (terminal).
    Terminated,
}

impl LifecycleState {
    /// Returns the set of valid next states from this state.
    pub fn valid_transitions(&self) -> Vec<LifecycleState> {
        match self {
            LifecycleState::Pending => vec![LifecycleState::Scheduled],
            LifecycleState::Scheduled => vec![LifecycleState::Running, LifecycleState::Failed],
            LifecycleState::Running => vec![
                LifecycleState::Completed,
                LifecycleState::Failed,
                LifecycleState::Terminated,
            ],
            LifecycleState::Completed => vec![],
            LifecycleState::Failed => vec![LifecycleState::Scheduled],
            LifecycleState::Terminated => vec![],
        }
    }

    /// Check if a transition to `target` is valid.
    pub fn can_transition_to(&self, target: &LifecycleState) -> bool {
        self.valid_transitions().contains(target)
    }

    /// Whether this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(self, LifecycleState::Completed | LifecycleState::Terminated)
    }
}

/// A lifecycle hook that executes at a specific point in the agent lifecycle.
///
/// Hooks are `Send + Sync` and return a boxed future so they can be stored
/// in trait-object collections.
pub trait LifecycleHook: Send + Sync {
    /// Human-readable hook name for logging.
    fn name(&self) -> &str;

    /// Execute the hook for the given agent.
    fn execute(&self, agent_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// Named collections of lifecycle hooks.
#[derive(Default)]
pub struct LifecycleHooks {
    /// Run before an agent enters the Running state.
    pub pre_start: Vec<Arc<dyn LifecycleHook>>,
    /// Run after an agent enters the Running state.
    pub post_start: Vec<Arc<dyn LifecycleHook>>,
    /// Run before an agent enters a terminal/stop state (Completed, Failed, Terminated).
    pub pre_stop: Vec<Arc<dyn LifecycleHook>>,
    /// Run after an agent enters a terminal/stop state.
    pub post_stop: Vec<Arc<dyn LifecycleHook>>,
}

/// Orchestrates agent lifecycle through validated state transitions,
/// event publishing, and hook execution.
pub struct AgentLifecycleManager {
    agents: HashMap<String, LifecycleState>,
    event_bus: Arc<EventBus>,
    hooks: LifecycleHooks,
}

impl AgentLifecycleManager {
    /// Create a new lifecycle manager.
    pub fn new(event_bus: Arc<EventBus>, hooks: LifecycleHooks) -> Self {
        Self {
            agents: HashMap::new(),
            event_bus,
            hooks,
        }
    }

    /// Register a new agent, placing it in `Pending` state and publishing
    /// a `Created` event.
    pub async fn register_agent(&mut self, agent_id: &str) -> Result<()> {
        self.agents
            .insert(agent_id.to_string(), LifecycleState::Pending);

        let envelope = AgentEventEnvelope::new(AgentEvent::Created, agent_id, "lifecycle-manager");
        self.event_bus.publish(&envelope).await;

        tracing::info!(agent_id = %agent_id, "Agent registered in Pending state");
        Ok(())
    }

    /// Get the current lifecycle state of an agent.
    pub fn state(&self, agent_id: &str) -> Option<&LifecycleState> {
        self.agents.get(agent_id)
    }

    /// Transition an agent to a new lifecycle state.
    ///
    /// 1. Validates the transition against the state machine.
    /// 2. Runs pre-transition hooks (`pre_start` for Running, `pre_stop` for terminal).
    /// 3. Updates state.
    /// 4. Publishes the corresponding event.
    /// 5. Runs post-transition hooks (`post_start` for Running, `post_stop` for terminal).
    ///
    /// Returns an error if the transition is invalid or a hook fails.
    pub async fn transition(&mut self, agent_id: &str, new_state: LifecycleState) -> Result<()> {
        let current = self
            .agents
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?
            .clone();

        if !current.can_transition_to(&new_state) {
            anyhow::bail!(
                "Invalid transition for agent {}: {:?} -> {:?}",
                agent_id,
                current,
                new_state
            );
        }

        // Map lifecycle state to event type.
        let event = match &new_state {
            LifecycleState::Scheduled => AgentEvent::Scheduled,
            LifecycleState::Running => AgentEvent::Running,
            LifecycleState::Completed => AgentEvent::Completed,
            LifecycleState::Failed => AgentEvent::Failed,
            LifecycleState::Terminated => AgentEvent::Terminated,
            LifecycleState::Pending => AgentEvent::Created,
        };

        let entering_running = new_state == LifecycleState::Running;
        let entering_terminal = matches!(
            new_state,
            LifecycleState::Completed | LifecycleState::Failed | LifecycleState::Terminated
        );

        // Pre-transition hooks.
        if entering_running {
            let hooks: Vec<_> = self.hooks.pre_start.clone();
            for hook in &hooks {
                tracing::debug!(agent_id = %agent_id, hook = hook.name(), "Running pre_start hook");
                hook.execute(agent_id).await?;
            }
        }
        if entering_terminal {
            let hooks: Vec<_> = self.hooks.pre_stop.clone();
            for hook in &hooks {
                tracing::debug!(agent_id = %agent_id, hook = hook.name(), "Running pre_stop hook");
                hook.execute(agent_id).await?;
            }
        }

        // Update state.
        self.agents.insert(agent_id.to_string(), new_state.clone());

        // Publish transition event.
        let envelope = AgentEventEnvelope::new(event, agent_id, "lifecycle-manager")
            .with_metadata("from", format!("{:?}", current))
            .with_metadata("to", format!("{:?}", new_state));
        self.event_bus.publish(&envelope).await;

        // Post-transition hooks.
        if entering_running {
            let hooks: Vec<_> = self.hooks.post_start.clone();
            for hook in &hooks {
                tracing::debug!(agent_id = %agent_id, hook = hook.name(), "Running post_start hook");
                hook.execute(agent_id).await?;
            }
        }
        if entering_terminal {
            let hooks: Vec<_> = self.hooks.post_stop.clone();
            for hook in &hooks {
                tracing::debug!(agent_id = %agent_id, hook = hook.name(), "Running post_stop hook");
                hook.execute(agent_id).await?;
            }
        }

        tracing::info!(
            agent_id = %agent_id,
            from = ?current,
            to = ?self.agents.get(agent_id),
            "Agent lifecycle transition"
        );

        Ok(())
    }

    /// Force-terminate an agent from any non-terminal state.
    ///
    /// Unlike `transition`, this bypasses normal state-machine validation
    /// and moves the agent directly to `Terminated`.  Pre/post-stop hooks
    /// are still executed.
    pub async fn force_terminate(&mut self, agent_id: &str, reason: &str) -> Result<()> {
        let current = self
            .agents
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?
            .clone();

        if current.is_terminal() {
            anyhow::bail!(
                "Cannot force-terminate agent {} in terminal state {:?}",
                agent_id,
                current
            );
        }

        // Pre-stop hooks.
        let hooks: Vec<_> = self.hooks.pre_stop.clone();
        for hook in &hooks {
            tracing::debug!(
                agent_id = %agent_id,
                hook = hook.name(),
                "Running pre_stop hook (force terminate)"
            );
            hook.execute(agent_id).await?;
        }

        self.agents
            .insert(agent_id.to_string(), LifecycleState::Terminated);

        let envelope =
            AgentEventEnvelope::new(AgentEvent::Terminated, agent_id, "lifecycle-manager")
                .with_metadata("reason", reason)
                .with_metadata("from", format!("{:?}", current));
        self.event_bus.publish(&envelope).await;

        // Post-stop hooks.
        let hooks: Vec<_> = self.hooks.post_stop.clone();
        for hook in &hooks {
            tracing::debug!(
                agent_id = %agent_id,
                hook = hook.name(),
                "Running post_stop hook (force terminate)"
            );
            hook.execute(agent_id).await?;
        }

        tracing::warn!(
            agent_id = %agent_id,
            reason = %reason,
            from = ?current,
            "Agent force-terminated"
        );

        Ok(())
    }

    /// Number of agents currently being managed.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventType;
    use std::sync::Mutex;

    /// A hook that records the agent IDs it was called with.
    struct RecordingHook {
        label: String,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingHook {
        fn new(label: &str, calls: Arc<Mutex<Vec<String>>>) -> Self {
            Self {
                label: label.to_string(),
                calls,
            }
        }
    }

    impl LifecycleHook for RecordingHook {
        fn name(&self) -> &str {
            &self.label
        }

        fn execute(&self, agent_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            let id = agent_id.to_string();
            let calls = self.calls.clone();
            Box::pin(async move {
                calls.lock().unwrap().push(id);
                Ok(())
            })
        }
    }

    /// A hook that always fails.
    struct FailingHook;

    impl LifecycleHook for FailingHook {
        fn name(&self) -> &str {
            "failing"
        }

        fn execute(
            &self,
            _agent_id: &str,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async { anyhow::bail!("hook failure") })
        }
    }

    fn make_manager() -> AgentLifecycleManager {
        let bus = Arc::new(EventBus::new());
        AgentLifecycleManager::new(bus, LifecycleHooks::default())
    }

    // -- Registration & basic state -----------------------------------------

    #[tokio::test]
    async fn test_register_agent_creates_pending() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Pending));
        assert_eq!(mgr.agent_count(), 1);
    }

    #[tokio::test]
    async fn test_register_agent_publishes_created_event() {
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus.clone(), LifecycleHooks::default());
        let mut rx = bus.subscribe(None, EventType::Specific(AgentEvent::Created));

        mgr.register_agent("a1").await.unwrap();

        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.event, AgentEvent::Created);
        assert_eq!(ev.agent_id, "a1");
    }

    // -- Valid transitions ---------------------------------------------------

    #[tokio::test]
    async fn test_transition_pending_to_scheduled() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Scheduled));
    }

    #[tokio::test]
    async fn test_full_lifecycle_happy_path() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Completed)
            .await
            .unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Completed));
    }

    #[tokio::test]
    async fn test_failed_recovery_path() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Failed).await.unwrap();

        // Recovery: Failed -> Scheduled -> Running
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Running));
    }

    // -- Invalid transitions -------------------------------------------------

    #[tokio::test]
    async fn test_invalid_transition_rejected() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();

        // Pending -> Running is invalid (must go through Scheduled).
        let result = mgr.transition("a1", LifecycleState::Running).await;
        assert!(result.is_err());
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Pending));
    }

    #[tokio::test]
    async fn test_terminal_state_no_transitions() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Completed)
            .await
            .unwrap();

        // Completed is terminal — any transition should fail.
        let result = mgr.transition("a1", LifecycleState::Scheduled).await;
        assert!(result.is_err());
        let result = mgr.transition("a1", LifecycleState::Running).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transition_unknown_agent_fails() {
        let mut mgr = make_manager();
        let result = mgr.transition("ghost", LifecycleState::Scheduled).await;
        assert!(result.is_err());
    }

    // -- Force termination ---------------------------------------------------

    #[tokio::test]
    async fn test_force_terminate_from_running() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();

        mgr.force_terminate("a1", "operator requested")
            .await
            .unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Terminated));
    }

    #[tokio::test]
    async fn test_force_terminate_from_failed() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Failed).await.unwrap();

        mgr.force_terminate("a1", "unrecoverable").await.unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Terminated));
    }

    #[tokio::test]
    async fn test_force_terminate_from_scheduled() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();

        mgr.force_terminate("a1", "cancelled").await.unwrap();
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Terminated));
    }

    #[tokio::test]
    async fn test_force_terminate_terminal_state_fails() {
        let mut mgr = make_manager();
        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Completed)
            .await
            .unwrap();

        let result = mgr.force_terminate("a1", "test").await;
        assert!(result.is_err());
    }

    // -- Lifecycle hooks -----------------------------------------------------

    #[tokio::test]
    async fn test_post_start_hook_on_running() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook = Arc::new(RecordingHook::new("post_start", calls.clone()));
        let hooks = LifecycleHooks {
            post_start: vec![hook],
            ..Default::default()
        };
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus, hooks);

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        assert!(calls.lock().unwrap().is_empty());

        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["a1".to_string()]);
    }

    #[tokio::test]
    async fn test_pre_stop_hook_on_completion() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook = Arc::new(RecordingHook::new("pre_stop", calls.clone()));
        let hooks = LifecycleHooks {
            pre_stop: vec![hook],
            ..Default::default()
        };
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus, hooks);

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Completed)
            .await
            .unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["a1".to_string()]);
    }

    #[tokio::test]
    async fn test_post_stop_hook_on_failure() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook = Arc::new(RecordingHook::new("post_stop", calls.clone()));
        let hooks = LifecycleHooks {
            post_stop: vec![hook],
            ..Default::default()
        };
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus, hooks);

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.transition("a1", LifecycleState::Failed).await.unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["a1".to_string()]);
    }

    #[tokio::test]
    async fn test_hooks_run_on_force_terminate() {
        let pre_calls = Arc::new(Mutex::new(Vec::new()));
        let post_calls = Arc::new(Mutex::new(Vec::new()));
        let hooks = LifecycleHooks {
            pre_stop: vec![Arc::new(RecordingHook::new("pre", pre_calls.clone()))],
            post_stop: vec![Arc::new(RecordingHook::new("post", post_calls.clone()))],
            ..Default::default()
        };
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus, hooks);

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.force_terminate("a1", "test").await.unwrap();

        assert_eq!(*pre_calls.lock().unwrap(), vec!["a1".to_string()]);
        assert_eq!(*post_calls.lock().unwrap(), vec!["a1".to_string()]);
    }

    #[tokio::test]
    async fn test_failing_hook_aborts_transition() {
        let hooks = LifecycleHooks {
            pre_start: vec![Arc::new(FailingHook)],
            ..Default::default()
        };
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus, hooks);

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();

        let result = mgr.transition("a1", LifecycleState::Running).await;
        assert!(result.is_err());
        // State should remain Scheduled since the hook failed.
        assert_eq!(mgr.state("a1"), Some(&LifecycleState::Scheduled));
    }

    // -- Events on force terminate -------------------------------------------

    #[tokio::test]
    async fn test_force_terminate_publishes_event() {
        let bus = Arc::new(EventBus::new());
        let mut mgr = AgentLifecycleManager::new(bus.clone(), LifecycleHooks::default());
        let mut rx = bus.subscribe(None, EventType::Specific(AgentEvent::Terminated));

        mgr.register_agent("a1").await.unwrap();
        mgr.transition("a1", LifecycleState::Scheduled)
            .await
            .unwrap();
        mgr.transition("a1", LifecycleState::Running).await.unwrap();
        mgr.force_terminate("a1", "operator kill").await.unwrap();

        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.agent_id, "a1");
        assert_eq!(ev.metadata.get("reason").unwrap(), "operator kill");
    }

    // -- LifecycleState helpers ----------------------------------------------

    #[test]
    fn test_lifecycle_state_is_terminal() {
        assert!(!LifecycleState::Pending.is_terminal());
        assert!(!LifecycleState::Scheduled.is_terminal());
        assert!(!LifecycleState::Running.is_terminal());
        assert!(LifecycleState::Completed.is_terminal());
        assert!(!LifecycleState::Failed.is_terminal());
        assert!(LifecycleState::Terminated.is_terminal());
    }

    #[test]
    fn test_lifecycle_state_valid_transitions() {
        assert_eq!(
            LifecycleState::Pending.valid_transitions(),
            vec![LifecycleState::Scheduled]
        );
        assert!(LifecycleState::Completed.valid_transitions().is_empty());
        assert!(LifecycleState::Terminated.valid_transitions().is_empty());
    }
}
