//! Fault detection and automatic recovery for failed agents.
//!
//! The [`RecoveryManager`] detects agents that have entered a `Failed` or
//! `Unresponsive` state and attempts to restart them using exponential backoff
//! to avoid thundering-herd problems.

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::state::{AgentStatus, ControllerState};

/// Configuration for the recovery manager.
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum number of recovery attempts before giving up.
    pub max_retries: u32,
    /// Base delay (in seconds) for the first retry.
    pub base_backoff_secs: u64,
    /// Maximum backoff delay (in seconds) to cap exponential growth.
    pub max_backoff_secs: u64,
    /// Multiplier for exponential backoff (typically 2.0).
    pub backoff_multiplier: f64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_backoff_secs: 5,
            max_backoff_secs: 300, // 5 minutes
            backoff_multiplier: 2.0,
        }
    }
}

/// Outcome of a recovery attempt for a single agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Agent was restarted successfully.
    Restarted,
    /// Agent is waiting for backoff delay before next retry.
    WaitingForBackoff,
    /// Agent has exceeded max retries and is permanently failed.
    PermanentlyFailed,
    /// Agent is not in a recoverable state (Running, Pending, Succeeded).
    NoAction,
}

/// Tracks per-agent recovery state.
#[derive(Debug, Clone)]
struct RecoveryRecord {
    retry_count: u32,
    last_attempt: Option<DateTime<Utc>>,
    current_backoff: Duration,
}

impl RecoveryRecord {
    fn new(base_backoff_secs: u64) -> Self {
        Self {
            retry_count: 0,
            last_attempt: None,
            current_backoff: Duration::seconds(base_backoff_secs as i64),
        }
    }
}

/// Manages fault detection and automatic recovery of failed agents.
#[derive(Debug)]
pub struct RecoveryManager {
    config: RecoveryConfig,
    records: HashMap<String, RecoveryRecord>,
}

impl RecoveryManager {
    pub fn new(config: RecoveryConfig) -> Self {
        Self {
            config,
            records: HashMap::new(),
        }
    }

    /// Register an agent for recovery tracking.
    pub fn register_agent(&mut self, agent_id: &str) {
        self.records.insert(
            agent_id.to_string(),
            RecoveryRecord::new(self.config.base_backoff_secs),
        );
    }

    /// Unregister an agent.
    pub fn unregister_agent(&mut self, agent_id: &str) {
        self.records.remove(agent_id);
    }

    /// Calculate the backoff duration for a given retry count using exponential backoff.
    ///
    /// `backoff = min(base * multiplier^retry_count, max_backoff)`
    pub fn calculate_backoff(&self, retry_count: u32) -> Duration {
        let base = self.config.base_backoff_secs as f64;
        let multiplier = self.config.backoff_multiplier;
        let max_secs = self.config.max_backoff_secs as f64;

        let backoff_secs = base * multiplier.powi(retry_count as i32);
        let capped = backoff_secs.min(max_secs) as i64;
        Duration::seconds(capped)
    }

    /// Detect and process recovery for all failed/unresponsive agents.
    ///
    /// Returns a list of (agent_id, action) for each agent processed.
    pub fn process_recovery(
        &mut self,
        state: &mut ControllerState,
    ) -> Vec<(String, RecoveryAction)> {
        let now = Utc::now();
        let mut actions = Vec::new();

        // Collect agent IDs to avoid borrow conflicts.
        let agent_ids: Vec<String> = state.agents.keys().cloned().collect();

        for agent_id in agent_ids {
            let action = self.process_agent_recovery(&agent_id, state, now);
            actions.push((agent_id, action));
        }

        actions
    }

    fn process_agent_recovery(
        &mut self,
        agent_id: &str,
        state: &mut ControllerState,
        now: DateTime<Utc>,
    ) -> RecoveryAction {
        let agent = match state.agents.get(agent_id) {
            Some(a) => a,
            None => return RecoveryAction::NoAction,
        };

        // Only process Failed or Unresponsive agents.
        if !matches!(
            agent.status,
            AgentStatus::Failed | AgentStatus::Unresponsive
        ) {
            return RecoveryAction::NoAction;
        }

        // Ensure we have a recovery record.
        if !self.records.contains_key(agent_id) {
            self.register_agent(agent_id);
        }

        let record = self.records.get(agent_id).unwrap();

        // Check if we've exceeded max retries.
        if record.retry_count >= self.config.max_retries {
            tracing::error!(
                agent_id = %agent_id,
                retry_count = record.retry_count,
                max_retries = self.config.max_retries,
                "Agent permanently failed — max retries exceeded"
            );
            // Update agent status.
            if let Some(agent) = state.agents.get_mut(agent_id) {
                agent.status = AgentStatus::Failed;
                agent.retry_count = record.retry_count;
            }
            return RecoveryAction::PermanentlyFailed;
        }

        // Check if we're still in backoff period.
        if let Some(last_attempt) = record.last_attempt {
            let elapsed = now - last_attempt;
            let backoff = record.current_backoff;
            if elapsed < backoff {
                let remaining = backoff - elapsed;
                tracing::debug!(
                    agent_id = %agent_id,
                    remaining_secs = remaining.num_seconds(),
                    "Agent waiting for backoff"
                );
                return RecoveryAction::WaitingForBackoff;
            }
        }

        // Attempt recovery (restart).
        let retry_count = record.retry_count + 1;
        let backoff = self.calculate_backoff(retry_count);

        tracing::info!(
            agent_id = %agent_id,
            attempt = retry_count,
            max_retries = self.config.max_retries,
            next_backoff_secs = backoff.num_seconds(),
            "Attempting agent recovery"
        );

        // Update the recovery record.
        let record = self.records.get_mut(agent_id).unwrap();
        record.retry_count = retry_count;
        record.last_attempt = Some(now);
        record.current_backoff = backoff;

        // Simulate restart: reset agent to Running.
        if let Some(agent) = state.agents.get_mut(agent_id) {
            agent.status = AgentStatus::Running;
            agent.retry_count = retry_count;
            agent.last_recovery_attempt = Some(now);
            agent.last_heartbeat = now;
            agent.consecutive_failures = 0;
        }

        RecoveryAction::Restarted
    }

    /// Get the current retry count for an agent.
    pub fn retry_count(&self, agent_id: &str) -> u32 {
        self.records.get(agent_id).map_or(0, |r| r.retry_count)
    }

    /// Reset the recovery state for an agent (e.g., after it stabilizes).
    pub fn reset(&mut self, agent_id: &str) {
        self.records.insert(
            agent_id.to_string(),
            RecoveryRecord::new(self.config.base_backoff_secs),
        );
    }

    /// Number of agents being tracked for recovery.
    pub fn tracked_count(&self) -> usize {
        self.records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ActualState, AgentConfig, AgentInfo, AgentStatus, DesiredState, ResourceRequirements,
    };
    use std::collections::HashMap;

    fn make_state() -> ControllerState {
        ControllerState {
            desired: DesiredState {
                replicas: 3,
                agent_config: AgentConfig {
                    name: "test".to_string(),
                    image: "test:latest".to_string(),
                    resources: ResourceRequirements {
                        cpu: "100m".to_string(),
                        memory: "128Mi".to_string(),
                    },
                },
            },
            actual: ActualState {
                running_replicas: 0,
                agent_status: AgentStatus::Pending,
                last_updated: Utc::now(),
            },
            agents: HashMap::new(),
        }
    }

    fn make_config(max_retries: u32, base_backoff_secs: u64) -> RecoveryConfig {
        RecoveryConfig {
            max_retries,
            base_backoff_secs,
            max_backoff_secs: 300,
            backoff_multiplier: 2.0,
        }
    }

    fn add_agent(state: &mut ControllerState, id: &str, status: AgentStatus) {
        let mut agent = AgentInfo::new(id, format!("agent-{id}"));
        agent.status = status;
        state.agents.insert(id.to_string(), agent);
    }

    #[test]
    fn test_default_config() {
        let config = RecoveryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_backoff_secs, 5);
        assert_eq!(config.max_backoff_secs, 300);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let manager = RecoveryManager::new(make_config(5, 10));

        // retry 0: 10 * 2^0 = 10
        assert_eq!(manager.calculate_backoff(0), Duration::seconds(10));
        // retry 1: 10 * 2^1 = 20
        assert_eq!(manager.calculate_backoff(1), Duration::seconds(20));
        // retry 2: 10 * 2^2 = 40
        assert_eq!(manager.calculate_backoff(2), Duration::seconds(40));
        // retry 3: 10 * 2^3 = 80
        assert_eq!(manager.calculate_backoff(3), Duration::seconds(80));
        // retry 4: 10 * 2^4 = 160
        assert_eq!(manager.calculate_backoff(4), Duration::seconds(160));
        // retry 5: 10 * 2^5 = 320 -> capped at 300
        assert_eq!(manager.calculate_backoff(5), Duration::seconds(300));
    }

    #[test]
    fn test_register_unregister() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        assert_eq!(manager.tracked_count(), 0);

        manager.register_agent("a1");
        assert_eq!(manager.tracked_count(), 1);

        manager.unregister_agent("a1");
        assert_eq!(manager.tracked_count(), 0);
    }

    #[test]
    fn test_no_action_for_running_agent() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Running);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::NoAction);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_no_action_for_pending_agent() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Pending);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::NoAction);
    }

    #[test]
    fn test_no_action_for_succeeded_agent() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Succeeded);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::NoAction);
    }

    #[test]
    fn test_restart_failed_agent() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1, RecoveryAction::Restarted);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
        assert_eq!(state.agents["a1"].retry_count, 1);
        assert!(state.agents["a1"].last_recovery_attempt.is_some());
    }

    #[test]
    fn test_restart_unresponsive_agent() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Unresponsive);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::Restarted);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_backoff_waiting_period() {
        let mut manager = RecoveryManager::new(make_config(3, 300)); // 5 min backoff
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        // First attempt: immediate restart
        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::Restarted);

        // Agent fails again
        state.agents.get_mut("a1").unwrap().status = AgentStatus::Failed;

        // Second attempt: should be waiting for backoff (300s hasn't elapsed)
        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::WaitingForBackoff);
    }

    #[test]
    fn test_permanently_failed_after_max_retries() {
        let mut manager = RecoveryManager::new(make_config(2, 0)); // max 2 retries, no backoff
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        // First attempt
        manager.process_recovery(&mut state);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);

        // Fail again
        state.agents.get_mut("a1").unwrap().status = AgentStatus::Failed;

        // Second attempt
        manager.process_recovery(&mut state);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);

        // Fail again
        state.agents.get_mut("a1").unwrap().status = AgentStatus::Failed;
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // Third attempt: exceeds max retries
        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions[0].1, RecoveryAction::PermanentlyFailed);
        assert_eq!(state.agents["a1"].status, AgentStatus::Failed);
        assert_eq!(state.agents["a1"].retry_count, 2);
    }

    #[test]
    fn test_retry_count_tracking() {
        let mut manager = RecoveryManager::new(make_config(5, 1));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        assert_eq!(manager.retry_count("a1"), 0);

        manager.process_recovery(&mut state);
        assert_eq!(manager.retry_count("a1"), 1);
    }

    #[test]
    fn test_reset_recovery_state() {
        let mut manager = RecoveryManager::new(make_config(3, 1));
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        manager.process_recovery(&mut state);
        assert_eq!(manager.retry_count("a1"), 1);

        manager.reset("a1");
        assert_eq!(manager.retry_count("a1"), 0);
    }

    #[test]
    fn test_multiple_agents_mixed_states() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();

        add_agent(&mut state, "a1", AgentStatus::Running);
        add_agent(&mut state, "a2", AgentStatus::Failed);
        add_agent(&mut state, "a3", AgentStatus::Unresponsive);
        add_agent(&mut state, "a4", AgentStatus::Succeeded);

        let actions = manager.process_recovery(&mut state);
        assert_eq!(actions.len(), 4);

        let get_action = |id: &str| -> RecoveryAction {
            actions.iter().find(|(aid, _)| aid == id).unwrap().1.clone()
        };

        assert_eq!(get_action("a1"), RecoveryAction::NoAction);
        assert_eq!(get_action("a2"), RecoveryAction::Restarted);
        assert_eq!(get_action("a3"), RecoveryAction::Restarted);
        assert_eq!(get_action("a4"), RecoveryAction::NoAction);

        assert_eq!(state.agents["a2"].status, AgentStatus::Running);
        assert_eq!(state.agents["a3"].status, AgentStatus::Running);
    }

    #[test]
    fn test_consecutive_restarts_increment_retry() {
        let mut manager = RecoveryManager::new(make_config(5, 0)); // 0 second backoff
        let mut state = make_state();
        add_agent(&mut state, "a1", AgentStatus::Failed);

        // Attempt 1
        manager.process_recovery(&mut state);
        assert_eq!(manager.retry_count("a1"), 1);

        // Fail again
        state.agents.get_mut("a1").unwrap().status = AgentStatus::Failed;

        // Attempt 2 (immediate since backoff is 0)
        manager.process_recovery(&mut state);
        assert_eq!(manager.retry_count("a1"), 2);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_auto_register_on_recovery() {
        let mut manager = RecoveryManager::new(make_config(3, 5));
        let mut state = make_state();

        // Add agent without registering with recovery manager
        add_agent(&mut state, "a1", AgentStatus::Failed);

        // Recovery should auto-register the agent
        assert_eq!(manager.tracked_count(), 0);
        manager.process_recovery(&mut state);
        assert_eq!(manager.tracked_count(), 1);
    }
}
