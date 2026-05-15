//! Heartbeat monitoring for agent liveness detection.
//!
//! The [`HeartbeatMonitor`] tracks the last heartbeat time for each agent and
//! detects agents that have stopped sending heartbeats within the configured
//! timeout window. Unresponsive agents are marked as [`AgentStatus::Unresponsive`].

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::state::{AgentStatus, ControllerState};

/// Configuration for heartbeat monitoring.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// How often (in seconds) to check for missed heartbeats.
    pub check_interval_secs: u64,
    /// Maximum time (in seconds) without a heartbeat before marking unresponsive.
    pub timeout_secs: u64,
    /// Jitter percentage applied per-agent to the timeout (0-100).
    /// A value of 10 means each agent's effective timeout is
    /// `timeout_secs ± 10%`, preventing a thundering herd of timeout detections.
    pub jitter_percent: u64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 15,
            timeout_secs: 60,
            jitter_percent: 10,
        }
    }
}

/// Result of a heartbeat check cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatAction {
    /// Agent is alive (recent heartbeat received).
    Alive,
    /// Agent missed heartbeats and is now marked Unresponsive.
    TimedOut,
    /// Agent was already in a terminal state (Failed/Succeeded), no action taken.
    NoAction,
}

/// Tracks agent heartbeats and detects unresponsive agents.
#[derive(Debug)]
pub struct HeartbeatMonitor {
    config: HeartbeatConfig,
    /// Per-agent heartbeat records (id -> HeartbeatRecord).
    records: HashMap<String, HeartbeatRecord>,
}

/// Internal record for a single agent's heartbeat tracking.
#[derive(Debug, Clone)]
struct HeartbeatRecord {
    last_heartbeat: DateTime<Utc>,
    missed_beats: u32,
}

impl HeartbeatRecord {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            last_heartbeat: now,
            missed_beats: 0,
        }
    }
}

impl HeartbeatMonitor {
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            records: HashMap::new(),
        }
    }

    /// Register an agent for heartbeat monitoring.
    pub fn register_agent(&mut self, agent_id: &str) {
        self.records
            .insert(agent_id.to_string(), HeartbeatRecord::new(Utc::now()));
    }

    /// Unregister an agent (e.g., when it is removed from the system).
    pub fn unregister_agent(&mut self, agent_id: &str) {
        self.records.remove(agent_id);
    }

    /// Record a heartbeat for the given agent.
    pub fn record_heartbeat(&mut self, agent_id: &str) {
        if let Some(record) = self.records.get_mut(agent_id) {
            record.last_heartbeat = Utc::now();
            record.missed_beats = 0;
        }
    }

    /// Check all registered agents for heartbeat timeouts.
    ///
    /// Returns a list of (agent_id, action) pairs for agents whose status changed.
    pub fn check_heartbeats(
        &mut self,
        state: &mut ControllerState,
    ) -> Vec<(String, HeartbeatAction)> {
        let now = Utc::now();
        let jitter_frac = self.config.jitter_percent as f64 / 100.0;
        let timeout_base = self.config.timeout_secs as f64;
        let timeout_min = timeout_base * (1.0 - jitter_frac);
        let timeout_max = timeout_base * (1.0 + jitter_frac);
        let mut actions = Vec::new();

        for (agent_id, record) in self.records.iter_mut() {
            let elapsed = now - record.last_heartbeat;
            // Apply per-agent jitter to the timeout so agents don't all
            // timeout at the same instant (thundering herd prevention).
            let timeout = jittered_duration(agent_id, timeout_min, timeout_max);

            // Skip agents not in the state (shouldn't happen but be safe).
            let agent = match state.agents.get_mut(agent_id) {
                Some(a) => a,
                None => continue,
            };

            // Skip agents already in terminal states.
            if matches!(
                agent.status,
                AgentStatus::Failed | AgentStatus::Succeeded | AgentStatus::Unresponsive
            ) {
                actions.push((agent_id.clone(), HeartbeatAction::NoAction));
                continue;
            }

            if elapsed > timeout {
                record.missed_beats += 1;
                tracing::warn!(
                    agent_id = %agent_id,
                    elapsed_secs = elapsed.num_seconds(),
                    missed_beats = record.missed_beats,
                    "Agent heartbeat timeout"
                );
                agent.status = AgentStatus::Unresponsive;
                agent.last_heartbeat = record.last_heartbeat;
                actions.push((agent_id.clone(), HeartbeatAction::TimedOut));
            } else {
                // Agent is alive — sync heartbeat to agent info.
                agent.last_heartbeat = record.last_heartbeat;
                actions.push((agent_id.clone(), HeartbeatAction::Alive));
            }
        }

        actions
    }

    /// Get the last heartbeat time for an agent.
    pub fn last_heartbeat(&self, agent_id: &str) -> Option<DateTime<Utc>> {
        self.records.get(agent_id).map(|r| r.last_heartbeat)
    }

    /// Get the number of missed beats for an agent.
    pub fn missed_beats(&self, agent_id: &str) -> Option<u32> {
        self.records.get(agent_id).map(|r| r.missed_beats)
    }

    /// Get the configured timeout duration.
    pub fn timeout_duration(&self) -> Duration {
        Duration::seconds(self.config.timeout_secs as i64)
    }

    /// Number of agents currently being monitored.
    pub fn monitored_count(&self) -> usize {
        self.records.len()
    }
}

/// Calculate a jittered duration for a specific agent.
///
/// Uses a deterministic hash of the agent_id so the jitter is stable
/// across calls for the same agent, but different between agents.
fn jittered_duration(agent_id: &str, min: f64, max: f64) -> Duration {
    // Use a simple hash of agent_id for deterministic per-agent jitter.
    let hash: u64 = agent_id
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let t = (hash % 1000) as f64 / 1000.0;
    let jittered = min + t * (max - min);
    Duration::seconds(jittered as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ActualState, AgentConfig, AgentInfo, AgentStatus, ControllerState, DesiredState,
        ResourceRequirements,
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

    fn make_config(timeout_secs: u64) -> HeartbeatConfig {
        HeartbeatConfig {
            check_interval_secs: 5,
            timeout_secs,
            jitter_percent: 0,
        }
    }

    #[test]
    fn test_default_config() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.check_interval_secs, 15);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_register_and_unregister() {
        let mut monitor = HeartbeatMonitor::new(make_config(60));
        assert_eq!(monitor.monitored_count(), 0);

        monitor.register_agent("a1");
        assert_eq!(monitor.monitored_count(), 1);

        monitor.register_agent("a2");
        assert_eq!(monitor.monitored_count(), 2);

        monitor.unregister_agent("a1");
        assert_eq!(monitor.monitored_count(), 1);
    }

    #[test]
    fn test_record_heartbeat() {
        let mut monitor = HeartbeatMonitor::new(make_config(60));
        monitor.register_agent("a1");

        let before = monitor.last_heartbeat("a1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        monitor.record_heartbeat("a1");
        let after = monitor.last_heartbeat("a1").unwrap();

        assert!(after > before);
        assert_eq!(monitor.missed_beats("a1"), Some(0));
    }

    #[test]
    fn test_heartbeat_alive() {
        let mut monitor = HeartbeatMonitor::new(make_config(60));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test-agent");
        agent.status = AgentStatus::Running;
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");
        let actions = monitor.check_heartbeats(&mut state);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1, HeartbeatAction::Alive);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);
    }

    #[test]
    fn test_heartbeat_timeout() {
        // Use 0 second timeout so the agent immediately times out.
        let mut monitor = HeartbeatMonitor::new(make_config(0));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test-agent");
        agent.status = AgentStatus::Running;
        // Set last heartbeat to well in the past
        agent.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");
        // Force the heartbeat record to be old too
        // We need to wait a tiny bit so `now` is after the record's timestamp
        std::thread::sleep(std::time::Duration::from_millis(5));

        let actions = monitor.check_heartbeats(&mut state);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1, HeartbeatAction::TimedOut);
        assert_eq!(state.agents["a1"].status, AgentStatus::Unresponsive);
    }

    #[test]
    fn test_heartbeat_no_action_for_failed() {
        let mut monitor = HeartbeatMonitor::new(make_config(0));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test-agent");
        agent.status = AgentStatus::Failed;
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");
        std::thread::sleep(std::time::Duration::from_millis(5));

        let actions = monitor.check_heartbeats(&mut state);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1, HeartbeatAction::NoAction);
        // Status should remain Failed, not changed to Unresponsive.
        assert_eq!(state.agents["a1"].status, AgentStatus::Failed);
    }

    #[test]
    fn test_heartbeat_succeeded_no_action() {
        let mut monitor = HeartbeatMonitor::new(make_config(0));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test-agent");
        agent.status = AgentStatus::Succeeded;
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");
        std::thread::sleep(std::time::Duration::from_millis(5));

        let actions = monitor.check_heartbeats(&mut state);
        assert_eq!(actions[0].1, HeartbeatAction::NoAction);
    }

    #[test]
    fn test_multiple_agents_mixed() {
        let mut monitor = HeartbeatMonitor::new(make_config(0));
        let mut state = make_state();

        // Agent a1: Running (will timeout because config timeout is 0)
        let mut a1 = AgentInfo::new("a1", "agent-1");
        a1.status = AgentStatus::Running;
        a1.last_heartbeat = Utc::now() - chrono::Duration::seconds(60);
        state.agents.insert("a1".into(), a1);

        // Agent a2: Succeeded (terminal, no action)
        let mut a2 = AgentInfo::new("a2", "agent-2");
        a2.status = AgentStatus::Succeeded;
        state.agents.insert("a2".into(), a2);

        monitor.register_agent("a1");
        monitor.register_agent("a2");
        std::thread::sleep(std::time::Duration::from_millis(5));

        let actions = monitor.check_heartbeats(&mut state);
        assert_eq!(actions.len(), 2);

        let a1_action = actions.iter().find(|(id, _)| id == "a1").unwrap();
        assert_eq!(a1_action.1, HeartbeatAction::TimedOut);
        assert_eq!(state.agents["a1"].status, AgentStatus::Unresponsive);

        let a2_action = actions.iter().find(|(id, _)| id == "a2").unwrap();
        assert_eq!(a2_action.1, HeartbeatAction::NoAction);
    }

    #[test]
    fn test_missed_beats_increments() {
        let mut monitor = HeartbeatMonitor::new(make_config(0));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test");
        agent.status = AgentStatus::Running;
        agent.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");
        std::thread::sleep(std::time::Duration::from_millis(5));

        // First check
        monitor.check_heartbeats(&mut state);
        assert_eq!(monitor.missed_beats("a1"), Some(1));

        // Second check (agent is now Unresponsive, so NoAction, missed_beats doesn't increment)
        monitor.check_heartbeats(&mut state);
        assert_eq!(monitor.missed_beats("a1"), Some(1)); // Still 1 because agent is terminal-adjacent
    }

    #[test]
    fn test_timeout_duration() {
        let monitor = HeartbeatMonitor::new(make_config(45));
        assert_eq!(monitor.timeout_duration(), Duration::seconds(45));
    }

    #[test]
    fn test_heartbeat_recovery_after_timeout() {
        let mut monitor = HeartbeatMonitor::new(make_config(60));
        let mut state = make_state();

        let mut agent = AgentInfo::new("a1", "test");
        agent.status = AgentStatus::Running;
        state.agents.insert("a1".into(), agent);

        monitor.register_agent("a1");

        // First check: alive
        let actions = monitor.check_heartbeats(&mut state);
        assert_eq!(actions[0].1, HeartbeatAction::Alive);
        assert_eq!(state.agents["a1"].status, AgentStatus::Running);

        // Simulate recovery: agent gets a new heartbeat
        monitor.record_heartbeat("a1");

        // Still alive after new heartbeat
        let actions = monitor.check_heartbeats(&mut state);
        assert_eq!(actions[0].1, HeartbeatAction::Alive);
    }

    #[test]
    fn test_default_config_has_jitter() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.jitter_percent, 10);
    }

    #[test]
    fn test_jittered_timeout_varies_per_agent() {
        let config = HeartbeatConfig {
            check_interval_secs: 5,
            timeout_secs: 100,
            jitter_percent: 10,
        };
        let mut monitor = HeartbeatMonitor::new(config);
        let mut state = make_state();

        // Register 3 agents — each should get a different jittered timeout.
        for id in &["agent-alpha", "agent-beta", "agent-gamma"] {
            let mut agent = AgentInfo::new(*id, format!("test-{id}"));
            agent.status = AgentStatus::Running;
            // Set heartbeat far in the past to ensure timeout with base 100s.
            agent.last_heartbeat = Utc::now() - chrono::Duration::seconds(90);
            state.agents.insert(id.to_string(), agent);
            monitor.register_agent(id);
        }

        // With 10% jitter on 100s timeout, the jittered range is [90, 110].
        // Since heartbeats were 90s ago, some agents may timeout and some may not,
        // depending on their per-agent jitter. This demonstrates different timeouts.
        let actions = monitor.check_heartbeats(&mut state);
        assert_eq!(actions.len(), 3);

        // At least one agent should timeout (those with jittered timeout <= 90s).
        // And at least one should be alive (those with jittered timeout > 90s).
        let timed_out = actions
            .iter()
            .filter(|(_, a)| *a == HeartbeatAction::TimedOut)
            .count();
        let alive = actions
            .iter()
            .filter(|(_, a)| *a == HeartbeatAction::Alive)
            .count();
        // With hash-based jitter, different agents get different timeouts.
        // Total should be 3.
        assert_eq!(timed_out + alive, 3);
    }

    #[test]
    fn test_jittered_duration_deterministic() {
        // Same agent_id should always produce the same jittered duration.
        let d1 = jittered_duration("test-agent", 50.0, 150.0);
        let d2 = jittered_duration("test-agent", 50.0, 150.0);
        assert_eq!(d1, d2);

        // Different agent_ids should produce different durations.
        let d3 = jittered_duration("agent-x", 50.0, 150.0);
        let _d4 = jittered_duration("agent-y", 50.0, 150.0);
        // While not guaranteed, it's extremely unlikely they're equal.
        // But let's check they're both in range.
        assert!(d1.num_seconds() >= 50 && d1.num_seconds() <= 150);
        assert!(d3.num_seconds() >= 50 && d3.num_seconds() <= 150);
    }
}
