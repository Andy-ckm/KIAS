//! Rolling Update — zero-downtime agent version upgrades.
//!
//! Architecture reference:
//! - Kubernetes RollingUpdate strategy (maxUnavailable, maxSurge)
//! - Netflix Atlas canary deployment pattern
//! - Google SRE Book Chapter 14: "Simplicity and Reliability"
//!
//! Pattern: Progressive delivery with automatic rollback on failure threshold.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Rolling update strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingUpdateConfig {
    /// Maximum number of agents that can be unavailable during update.
    pub max_unavailable: u32,
    /// Maximum number of extra agents above desired count during update.
    pub max_surge: u32,
    /// Number of consecutive health checks required before marking ready.
    pub min_ready_checks: u32,
    /// Timeout for the entire rolling update (ms).
    pub update_timeout_ms: u64,
    /// Failure threshold: if this many agents fail health check, abort and rollback.
    pub failure_threshold: u32,
    /// Pause duration between batches (ms).
    pub batch_pause_ms: u64,
}

impl Default for RollingUpdateConfig {
    fn default() -> Self {
        Self {
            max_unavailable: 1,
            max_surge: 1,
            min_ready_checks: 3,
            update_timeout_ms: 600_000,
            failure_threshold: 3,
            batch_pause_ms: 2_000,
        }
    }
}

/// State of a single agent during rolling update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateAgentState {
    Pending,
    Updating,
    HealthChecking { consecutive_ok: u32 },
    Updated,
    Failed,
    RolledBack,
}

/// A single agent's rolling update status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdateStatus {
    pub agent_id: String,
    pub old_version: String,
    pub new_version: String,
    pub state: UpdateAgentState,
    pub health_checks_passed: u32,
    pub last_health_check_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
    pub error: Option<String>,
}

/// Overall rolling update status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingUpdateStatus {
    pub update_id: String,
    pub target_version: String,
    pub total_agents: u32,
    pub updated: u32,
    pub updating: u32,
    pub pending: u32,
    pub failed: u32,
    pub rolled_back: u32,
    pub phase: UpdatePhase,
    pub started_at_ms: u64,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdatePhase {
    NotStarted,
    InProgress,
    Completed,
    RolledBack,
    Aborted,
}

/// Manages rolling updates for a group of agents.
pub struct RollingUpdateManager {
    config: RollingUpdateConfig,
    updates: std::sync::Mutex<HashMap<String, RollingUpdateState>>,
}

struct RollingUpdateState {
    status: RollingUpdateStatus,
    agents: Vec<AgentUpdateStatus>,
    pending_queue: VecDeque<String>,
    rollback_version: String,
}

impl RollingUpdateManager {
    pub fn new(config: RollingUpdateConfig) -> Self {
        Self {
            config,
            updates: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Start a rolling update for a set of agents.
    pub fn start_update(
        &self,
        update_id: String,
        agent_ids: Vec<String>,
        current_version: String,
        target_version: String,
    ) -> Result<(), String> {
        if agent_ids.is_empty() {
            return Err("No agents to update".to_string());
        }

        let total = agent_ids.len() as u32;
        let agent_statuses: Vec<AgentUpdateStatus> = agent_ids
            .iter()
            .map(|id| AgentUpdateStatus {
                agent_id: id.clone(),
                old_version: current_version.clone(),
                new_version: target_version.clone(),
                state: UpdateAgentState::Pending,
                health_checks_passed: 0,
                last_health_check_ms: None,
                updated_at_ms: None,
                error: None,
            })
            .collect();

        let pending_queue: VecDeque<String> = agent_ids.into();

        let state = RollingUpdateState {
            status: RollingUpdateStatus {
                update_id: update_id.clone(),
                target_version,
                total_agents: total,
                updated: 0,
                updating: 0,
                pending: total,
                failed: 0,
                rolled_back: 0,
                phase: UpdatePhase::InProgress,
                started_at_ms: now_ms(),
                elapsed_ms: 0,
                error: None,
            },
            agents: agent_statuses,
            pending_queue,
            rollback_version: current_version,
        };

        let mut updates = self.updates.lock().map_err(|e| e.to_string())?;
        updates.insert(update_id, state);
        Ok(())
    }

    /// Get the status of a rolling update.
    pub fn get_status(&self, update_id: &str) -> Option<RollingUpdateStatus> {
        let updates = self.updates.lock().ok()?;
        updates.get(update_id).map(|s| {
            let mut status = s.status.clone();
            status.elapsed_ms = now_ms() - status.started_at_ms;
            status
        })
    }

    /// Mark an agent as successfully updated.
    pub fn mark_agent_updated(&self, update_id: &str, agent_id: &str) -> Result<(), String> {
        let mut updates = self.updates.lock().map_err(|e| e.to_string())?;
        let state = updates
            .get_mut(update_id)
            .ok_or_else(|| format!("Update {} not found", update_id))?;

        if let Some(agent) = state.agents.iter_mut().find(|a| a.agent_id == agent_id) {
            agent.state = UpdateAgentState::Updated;
            agent.updated_at_ms = Some(now_ms());
            state.status.updated += 1;
            state.status.updating = state.status.updating.saturating_sub(1);
            if state.status.updated >= state.status.total_agents {
                state.status.phase = UpdatePhase::Completed;
            }
        }
        Ok(())
    }

    /// Report a health check result for an agent.
    pub fn report_health_check(
        &self,
        update_id: &str,
        agent_id: &str,
        healthy: bool,
    ) -> Result<(), String> {
        let mut updates = self.updates.lock().map_err(|e| e.to_string())?;
        let state = updates
            .get_mut(update_id)
            .ok_or_else(|| format!("Update {} not found", update_id))?;

        if let Some(agent) = state.agents.iter_mut().find(|a| a.agent_id == agent_id) {
            agent.last_health_check_ms = Some(now_ms());
            if healthy {
                match &agent.state {
                    UpdateAgentState::HealthChecking { consecutive_ok } => {
                        let new_count = consecutive_ok + 1;
                        if new_count >= self.config.min_ready_checks {
                            agent.state = UpdateAgentState::Updated;
                            agent.health_checks_passed = new_count;
                            agent.updated_at_ms = Some(now_ms());
                            state.status.updated += 1;
                            state.status.updating = state.status.updating.saturating_sub(1);
                        } else {
                            agent.state = UpdateAgentState::HealthChecking {
                                consecutive_ok: new_count,
                            };
                            agent.health_checks_passed = new_count;
                        }
                    }
                    _ => {
                        agent.state = UpdateAgentState::HealthChecking { consecutive_ok: 1 };
                        agent.health_checks_passed = 1;
                    }
                }
            } else {
                agent.state = UpdateAgentState::Failed;
                agent.error = Some("Health check failed".to_string());
                state.status.failed += 1;
                state.status.updating = state.status.updating.saturating_sub(1);

                if state.status.failed >= self.config.failure_threshold {
                    state.status.phase = UpdatePhase::Aborted;
                    state.status.error = Some(format!(
                        "Failure threshold ({}) reached, aborting update",
                        self.config.failure_threshold
                    ));
                }
            }

            if state.status.updated >= state.status.total_agents {
                state.status.phase = UpdatePhase::Completed;
            }
        }
        Ok(())
    }

    /// Get the next batch of agents to update.
    pub fn get_next_batch(&self, update_id: &str) -> Result<Vec<String>, String> {
        let mut updates = self.updates.lock().map_err(|e| e.to_string())?;
        let state = updates
            .get_mut(update_id)
            .ok_or_else(|| format!("Update {} not found", update_id))?;

        if state.status.phase != UpdatePhase::InProgress {
            return Ok(Vec::new());
        }

        let available_slots = self.config.max_unavailable.max(self.config.max_surge) as usize;
        let mut batch = Vec::new();

        for _ in 0..available_slots {
            if let Some(agent_id) = state.pending_queue.pop_front() {
                if let Some(agent) = state.agents.iter_mut().find(|a| a.agent_id == agent_id) {
                    agent.state = UpdateAgentState::Updating;
                    state.status.updating += 1;
                    state.status.pending = state.status.pending.saturating_sub(1);
                }
                batch.push(agent_id);
            }
        }
        Ok(batch)
    }

    /// Abort a rolling update and rollback all updated agents.
    pub fn abort_and_rollback(&self, update_id: &str) -> Result<(), String> {
        let mut updates = self.updates.lock().map_err(|e| e.to_string())?;
        let state = updates
            .get_mut(update_id)
            .ok_or_else(|| format!("Update {} not found", update_id))?;

        state.status.phase = UpdatePhase::RolledBack;
        let rollback_version = state.rollback_version.clone();

        for agent in state.agents.iter_mut() {
            if matches!(
                agent.state,
                UpdateAgentState::Updated | UpdateAgentState::HealthChecking { .. }
            ) {
                agent.state = UpdateAgentState::RolledBack;
                agent.new_version = rollback_version.clone();
                state.status.rolled_back += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_update() {
        let mgr = RollingUpdateManager::new(RollingUpdateConfig::default());
        mgr.start_update(
            "u1".to_string(),
            vec!["a1".into(), "a2".into(), "a3".into()],
            "v1.0".into(),
            "v2.0".into(),
        )
        .unwrap();
        let status = mgr.get_status("u1").unwrap();
        assert_eq!(status.total_agents, 3);
        assert_eq!(status.phase, UpdatePhase::InProgress);
    }

    #[test]
    fn test_get_next_batch() {
        let config = RollingUpdateConfig {
            max_unavailable: 2,
            max_surge: 1,
            ..Default::default()
        };
        let mgr = RollingUpdateManager::new(config);
        mgr.start_update(
            "u1".to_string(),
            vec!["a1".into(), "a2".into(), "a3".into(), "a4".into()],
            "v1.0".into(),
            "v2.0".into(),
        )
        .unwrap();
        let batch = mgr.get_next_batch("u1").unwrap();
        assert!(batch.len() <= 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_health_check_and_complete() {
        let config = RollingUpdateConfig {
            min_ready_checks: 2,
            ..Default::default()
        };
        let mgr = RollingUpdateManager::new(config);
        mgr.start_update(
            "u1".to_string(),
            vec!["a1".into()],
            "v1.0".into(),
            "v2.0".into(),
        )
        .unwrap();
        mgr.get_next_batch("u1").unwrap();
        mgr.report_health_check("u1", "a1", true).unwrap();
        mgr.report_health_check("u1", "a1", true).unwrap();
        let status = mgr.get_status("u1").unwrap();
        assert_eq!(status.phase, UpdatePhase::Completed);
        assert_eq!(status.updated, 1);
    }

    #[test]
    fn test_failure_threshold_aborts() {
        let config = RollingUpdateConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let mgr = RollingUpdateManager::new(config);
        mgr.start_update(
            "u1".to_string(),
            vec!["a1".into()],
            "v1.0".into(),
            "v2.0".into(),
        )
        .unwrap();
        mgr.get_next_batch("u1").unwrap();
        mgr.report_health_check("u1", "a1", false).unwrap();
        let status = mgr.get_status("u1").unwrap();
        assert_eq!(status.phase, UpdatePhase::Aborted);
    }

    #[test]
    fn test_abort_and_rollback() {
        let mgr = RollingUpdateManager::new(RollingUpdateConfig::default());
        mgr.start_update(
            "u1".to_string(),
            vec!["a1".into(), "a2".into()],
            "v1.0".into(),
            "v2.0".into(),
        )
        .unwrap();
        mgr.get_next_batch("u1").unwrap();
        mgr.mark_agent_updated("u1", "a1").unwrap();
        mgr.abort_and_rollback("u1").unwrap();
        let status = mgr.get_status("u1").unwrap();
        assert_eq!(status.phase, UpdatePhase::RolledBack);
        assert_eq!(status.rolled_back, 1);
    }
}
