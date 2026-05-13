//! Health checking loop that integrates heartbeat monitoring, fault detection,
//! and automatic recovery into a single periodic loop.
//!
//! The [`HealthChecker`] runs at a configurable interval and orchestrates:
//! 1. Heartbeat checking — detect unresponsive agents
//! 2. Fault detection — identify failed/unresponsive agents
//! 3. Auto-recovery — restart failed agents with exponential backoff
//! 4. State synchronization — update running replica counts

use crate::heartbeat::{HeartbeatAction, HeartbeatConfig, HeartbeatMonitor};
use crate::recovery::{RecoveryAction, RecoveryConfig, RecoveryManager};
use crate::state::ControllerState;

/// Configuration for the health checking loop.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Interval between health check cycles in milliseconds.
    pub check_interval_ms: u64,
    /// Heartbeat monitor configuration.
    pub heartbeat: HeartbeatConfig,
    /// Recovery manager configuration.
    pub recovery: RecoveryConfig,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 15_000,
            heartbeat: HeartbeatConfig::default(),
            recovery: RecoveryConfig::default(),
        }
    }
}

/// Summary of a single health check cycle.
#[derive(Debug, Clone, Default)]
pub struct HealthCheckSummary {
    /// Number of agents checked.
    pub agents_checked: usize,
    /// Number of agents that are alive.
    pub alive: usize,
    /// Number of agents that timed out (became unresponsive).
    pub timed_out: usize,
    /// Number of agents restarted.
    pub restarted: usize,
    /// Number of agents waiting for backoff.
    pub waiting_for_backoff: usize,
    /// Number of agents permanently failed.
    pub permanently_failed: usize,
    /// Number of agents with no recovery action needed.
    pub no_action: usize,
}

/// The main health checker that orchestrates heartbeat monitoring, fault
/// detection, and recovery.
pub struct HealthChecker {
    config: HealthCheckConfig,
    heartbeat_monitor: HeartbeatMonitor,
    recovery_manager: RecoveryManager,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        let heartbeat_monitor = HeartbeatMonitor::new(config.heartbeat.clone());
        let recovery_manager = RecoveryManager::new(config.recovery.clone());
        Self {
            config,
            heartbeat_monitor,
            recovery_manager,
        }
    }

    /// Register an agent for health monitoring.
    pub fn register_agent(&mut self, agent_id: &str) {
        self.heartbeat_monitor.register_agent(agent_id);
        self.recovery_manager.register_agent(agent_id);
    }

    /// Unregister an agent from health monitoring.
    pub fn unregister_agent(&mut self, agent_id: &str) {
        self.heartbeat_monitor.unregister_agent(agent_id);
        self.recovery_manager.unregister_agent(agent_id);
    }

    /// Record a heartbeat for the given agent.
    pub fn record_heartbeat(&mut self, agent_id: &str) {
        self.heartbeat_monitor.record_heartbeat(agent_id);
        // Reset recovery state when agent is healthy again.
        self.recovery_manager.reset(agent_id);
    }

    /// Run a single health check cycle.
    ///
    /// This performs:
    /// 1. Heartbeat checking to detect unresponsive agents
    /// 2. Recovery processing for failed/unresponsive agents
    /// 3. State synchronization
    pub fn check(&mut self, state: &mut ControllerState) -> HealthCheckSummary {
        let mut summary = HealthCheckSummary::default();

        // Phase 1: Check heartbeats — detect unresponsive agents.
        let heartbeat_actions = self.heartbeat_monitor.check_heartbeats(state);
        summary.agents_checked = heartbeat_actions.len();

        for (_, action) in &heartbeat_actions {
            match action {
                HeartbeatAction::Alive => summary.alive += 1,
                HeartbeatAction::TimedOut => summary.timed_out += 1,
                HeartbeatAction::NoAction => {} // counted in recovery phase
            }
        }

        // Phase 2: Process recovery for failed/unresponsive agents.
        let recovery_actions = self.recovery_manager.process_recovery(state);

        for (_, action) in &recovery_actions {
            match action {
                RecoveryAction::Restarted => summary.restarted += 1,
                RecoveryAction::WaitingForBackoff => summary.waiting_for_backoff += 1,
                RecoveryAction::PermanentlyFailed => summary.permanently_failed += 1,
                RecoveryAction::NoAction => summary.no_action += 1,
            }
        }

        // Phase 3: Synchronize running replica count.
        state.sync_running_replicas();

        if summary.timed_out > 0 || summary.restarted > 0 || summary.permanently_failed > 0 {
            tracing::info!(
                agents_checked = summary.agents_checked,
                alive = summary.alive,
                timed_out = summary.timed_out,
                restarted = summary.restarted,
                waiting_for_backoff = summary.waiting_for_backoff,
                permanently_failed = summary.permanently_failed,
                running_replicas = state.actual.running_replicas,
                "Health check completed"
            );
        } else {
            tracing::debug!(
                agents_checked = summary.agents_checked,
                alive = summary.alive,
                "Health check completed — all healthy"
            );
        }

        summary
    }

    /// Get the configured check interval in milliseconds.
    pub fn check_interval_ms(&self) -> u64 {
        self.config.check_interval_ms
    }

    /// Get a reference to the heartbeat monitor.
    pub fn heartbeat_monitor(&self) -> &HeartbeatMonitor {
        &self.heartbeat_monitor
    }

    /// Get a reference to the recovery manager.
    pub fn recovery_manager(&self) -> &RecoveryManager {
        &self.recovery_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ActualState, AgentConfig, AgentInfo, AgentStatus, DesiredState, ResourceRequirements,
    };
    use chrono::Utc;
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

    fn add_agent(state: &mut ControllerState, id: &str, status: AgentStatus) {
        let mut agent = AgentInfo::new(id, format!("agent-{id}"));
        agent.status = status;
        state.agents.insert(id.to_string(), agent);
    }

    fn make_config(timeout_secs: u64, max_retries: u32) -> HealthCheckConfig {
        HealthCheckConfig {
            check_interval_ms: 1000,
            heartbeat: HeartbeatConfig {
                check_interval_secs: 5,
                timeout_secs,
            },
            recovery: RecoveryConfig {
                max_retries,
                base_backoff_secs: 1,
                max_backoff_secs: 60,
                backoff_multiplier: 2.0,
            },
        }
    }

    #[test]
    fn test_default_config() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.check_interval_ms, 15_000);
        assert_eq!(config.heartbeat.check_interval_secs, 15);
        assert_eq!(config.heartbeat.timeout_secs, 60);
        assert_eq!(config.recovery.max_retries, 3);
    }

    #[test]
    fn test_register_unregister() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        checker.register_agent("a1");
        checker.register_agent("a2");
        assert_eq!(checker.heartbeat_monitor().monitored_count(), 2);
        assert_eq!(checker.recovery_manager().tracked_count(), 2);

        checker.unregister_agent("a1");
        assert_eq!(checker.heartbeat_monitor().monitored_count(), 1);
        assert_eq!(checker.recovery_manager().tracked_count(), 1);
    }

    #[test]
    fn test_check_all_healthy() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        let mut state = make_state();

        let mut a1 = AgentInfo::new("a1", "agent-1");
        a1.status = AgentStatus::Running;
        state.agents.insert("a1".into(), a1);

        let mut a2 = AgentInfo::new("a2", "agent-2");
        a2.status = AgentStatus::Running;
        state.agents.insert("a2".into(), a2);

        checker.register_agent("a1");
        checker.register_agent("a2");

        let summary = checker.check(&mut state);

        assert_eq!(summary.agents_checked, 2);
        assert_eq!(summary.alive, 2);
        assert_eq!(summary.timed_out, 0);
        assert_eq!(summary.restarted, 0);
        assert_eq!(summary.no_action, 2); // both Running, recovery has NoAction
        assert_eq!(state.actual.running_replicas, 2);
    }

    #[test]
    fn test_check_detects_unresponsive_and_recovers() {
        // Use 0 timeout so heartbeat check immediately detects timeout.
        let mut checker = HealthChecker::new(make_config(0, 3));
        let mut state = make_state();

        let mut a1 = AgentInfo::new("a1", "agent-1");
        a1.status = AgentStatus::Running;
        a1.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        state.agents.insert("a1".into(), a1);

        checker.register_agent("a1");
        std::thread::sleep(std::time::Duration::from_millis(5));

        let summary = checker.check(&mut state);

        // Heartbeat detects timeout (timed_out=1), then recovery restarts (restarted=1)
        assert_eq!(summary.timed_out, 1);

        // Phase 2: recovery restarts the agent
        assert_eq!(summary.restarted, 1);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_check_handles_failed_agent_recovery() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        let mut state = make_state();

        // Agent is Failed but heartbeat hasn't timed out
        let mut a1 = AgentInfo::new("a1", "agent-1");
        a1.status = AgentStatus::Failed;
        state.agents.insert("a1".into(), a1);

        checker.register_agent("a1");

        let summary = checker.check(&mut state);

        // Heartbeat: agent is Failed (terminal), so NoAction
        // Recovery: agent is Failed, so Restarted
        assert_eq!(summary.restarted, 1);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_check_permanently_failed() {
        let mut checker = HealthChecker::new(make_config(60, 1)); // max 1 retry
        let mut state = make_state();

        add_agent(&mut state, "a1", AgentStatus::Failed);
        checker.register_agent("a1");

        // First check: restart
        let summary = checker.check(&mut state);
        assert_eq!(summary.restarted, 1);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);

        // Fail again
        state.agents.get_mut("a1").unwrap().status = AgentStatus::Failed;
        std::thread::sleep(std::time::Duration::from_millis(1100)); // wait for backoff

        // Second check: permanently failed
        let summary = checker.check(&mut state);
        assert_eq!(summary.permanently_failed, 1);
    }

    #[test]
    fn test_check_syncs_running_replicas() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        let mut state = make_state();

        add_agent(&mut state, "a1", AgentStatus::Running);
        add_agent(&mut state, "a2", AgentStatus::Running);
        add_agent(&mut state, "a3", AgentStatus::Failed);

        checker.register_agent("a1");
        checker.register_agent("a2");
        checker.register_agent("a3");

        checker.check(&mut state);

        // a1 and a2 are Running, a3 was Failed and got restarted to Running
        assert_eq!(state.actual.running_replicas, 3);
    }

    #[test]
    fn test_heartbeat_recording_resets_recovery() {
        let mut checker = HealthChecker::new(make_config(0, 3));
        let mut state = make_state();

        let mut a1 = AgentInfo::new("a1", "agent-1");
        a1.status = AgentStatus::Running;
        a1.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        state.agents.insert("a1".into(), a1);

        checker.register_agent("a1");
        std::thread::sleep(std::time::Duration::from_millis(5));

        // First check: timeout + recovery
        checker.check(&mut state);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
        assert_eq!(checker.recovery_manager().retry_count("a1"), 1);

        // Record a new heartbeat — resets recovery
        checker.record_heartbeat("a1");
        assert_eq!(checker.recovery_manager().retry_count("a1"), 0);
    }

    #[test]
    fn test_check_interval_ms() {
        let mut config = make_config(60, 3);
        config.check_interval_ms = 5000;
        let checker = HealthChecker::new(config);
        assert_eq!(checker.check_interval_ms(), 5000);
    }

    #[test]
    fn test_empty_state_check() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        let mut state = make_state();

        let summary = checker.check(&mut state);

        assert_eq!(summary.agents_checked, 0);
        assert_eq!(summary.alive, 0);
        assert_eq!(summary.timed_out, 0);
        assert_eq!(summary.restarted, 0);
        assert_eq!(state.actual.running_replicas, 0);
    }

    #[test]
    fn test_mixed_agent_statuses() {
        let mut checker = HealthChecker::new(make_config(60, 3));
        let mut state = make_state();

        add_agent(&mut state, "running1", AgentStatus::Running);
        add_agent(&mut state, "running2", AgentStatus::Running);
        add_agent(&mut state, "failed1", AgentStatus::Failed);
        add_agent(&mut state, "succeeded1", AgentStatus::Succeeded);
        add_agent(&mut state, "pending1", AgentStatus::Pending);

        checker.register_agent("running1");
        checker.register_agent("running2");
        checker.register_agent("failed1");
        checker.register_agent("succeeded1");
        checker.register_agent("pending1");

        let summary = checker.check(&mut state);

        assert_eq!(summary.agents_checked, 5);
        assert_eq!(summary.alive, 3); // running1, running2, pending1 (alive heartbeat)
        assert_eq!(summary.restarted, 1); // failed1
                                          // succeeded1: heartbeat NoAction + recovery NoAction
    }
}
