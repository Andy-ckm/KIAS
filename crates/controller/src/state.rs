use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub replicas: u32,
    pub agent_config: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub image: String,
    pub resources: ResourceRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu: String,
    pub memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualState {
    pub running_replicas: u32,
    pub agent_status: AgentStatus,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Pending,
    Running,
    Failed,
    Succeeded,
    Unresponsive,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Pending => write!(f, "Pending"),
            AgentStatus::Running => write!(f, "Running"),
            AgentStatus::Failed => write!(f, "Failed"),
            AgentStatus::Succeeded => write!(f, "Succeeded"),
            AgentStatus::Unresponsive => write!(f, "Unresponsive"),
        }
    }
}

/// Per-agent tracking information for heartbeat and recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub retry_count: u32,
    pub last_recovery_attempt: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
}

impl AgentInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status: AgentStatus::Pending,
            last_heartbeat: Utc::now(),
            retry_count: 0,
            last_recovery_attempt: None,
            consecutive_failures: 0,
        }
    }

    /// Check if this agent has exceeded the maximum retry count.
    pub fn has_exceeded_retries(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }

    /// Check if this agent is in a recoverable state (Failed or Unresponsive,
    /// but hasn't exceeded max retries).
    pub fn is_recoverable(&self, max_retries: u32) -> bool {
        matches!(self.status, AgentStatus::Failed | AgentStatus::Unresponsive)
            && !self.has_exceeded_retries(max_retries)
    }

    /// Time since last heartbeat.
    pub fn time_since_heartbeat(&self) -> chrono::Duration {
        Utc::now() - self.last_heartbeat
    }
}

pub struct ControllerState {
    pub desired: DesiredState,
    pub actual: ActualState,
    /// Per-agent tracking for heartbeat monitoring and recovery.
    pub agents: HashMap<String, AgentInfo>,
}

impl ControllerState {
    /// Count agents by status.
    pub fn count_by_status(&self, status: &AgentStatus) -> usize {
        self.agents.values().filter(|a| a.status == *status).count()
    }

    /// Get all agents matching a given status.
    pub fn agents_with_status(&self, status: &AgentStatus) -> Vec<&AgentInfo> {
        self.agents
            .values()
            .filter(|a| a.status == *status)
            .collect()
    }

    /// Update running replica count based on actual agent statuses.
    pub fn sync_running_replicas(&mut self) {
        self.actual.running_replicas = self
            .agents
            .values()
            .filter(|a| a.status == AgentStatus::Running)
            .count() as u32;
        self.actual.last_updated = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent_info(id: &str, status: AgentStatus) -> AgentInfo {
        let mut info = AgentInfo::new(id, format!("agent-{id}"));
        info.status = status;
        info
    }

    #[test]
    fn test_agent_status_display() {
        assert_eq!(format!("{}", AgentStatus::Pending), "Pending");
        assert_eq!(format!("{}", AgentStatus::Running), "Running");
        assert_eq!(format!("{}", AgentStatus::Failed), "Failed");
        assert_eq!(format!("{}", AgentStatus::Unresponsive), "Unresponsive");
        assert_eq!(format!("{}", AgentStatus::Succeeded), "Succeeded");
    }

    #[test]
    fn test_agent_status_equality() {
        assert_eq!(AgentStatus::Running, AgentStatus::Running);
        assert_ne!(AgentStatus::Running, AgentStatus::Failed);
        assert_ne!(AgentStatus::Failed, AgentStatus::Unresponsive);
    }

    #[test]
    fn test_agent_info_new() {
        let info = AgentInfo::new("a1", "test-agent");
        assert_eq!(info.id, "a1");
        assert_eq!(info.name, "test-agent");
        assert_eq!(info.status, AgentStatus::Pending);
        assert_eq!(info.retry_count, 0);
        assert!(info.last_recovery_attempt.is_none());
        assert_eq!(info.consecutive_failures, 0);
    }

    #[test]
    fn test_has_exceeded_retries() {
        let mut info = AgentInfo::new("a1", "test");
        assert!(!info.has_exceeded_retries(3));

        info.retry_count = 2;
        assert!(!info.has_exceeded_retries(3));

        info.retry_count = 3;
        assert!(info.has_exceeded_retries(3));
    }

    #[test]
    fn test_is_recoverable() {
        let mut info = AgentInfo::new("a1", "test");

        // Pending is not recoverable
        assert!(!info.is_recoverable(3));

        // Failed is recoverable
        info.status = AgentStatus::Failed;
        assert!(info.is_recoverable(3));

        // Unresponsive is recoverable
        info.status = AgentStatus::Unresponsive;
        assert!(info.is_recoverable(3));

        // Running is not recoverable
        info.status = AgentStatus::Running;
        assert!(!info.is_recoverable(3));

        // Failed but exceeded retries is not recoverable
        info.status = AgentStatus::Failed;
        info.retry_count = 3;
        assert!(!info.is_recoverable(3));
    }

    #[test]
    fn test_controller_state_count_by_status() {
        let mut state = ControllerState {
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
        };

        state
            .agents
            .insert("a1".into(), make_agent_info("a1", AgentStatus::Running));
        state
            .agents
            .insert("a2".into(), make_agent_info("a2", AgentStatus::Running));
        state
            .agents
            .insert("a3".into(), make_agent_info("a3", AgentStatus::Failed));

        assert_eq!(state.count_by_status(&AgentStatus::Running), 2);
        assert_eq!(state.count_by_status(&AgentStatus::Failed), 1);
        assert_eq!(state.count_by_status(&AgentStatus::Pending), 0);
    }

    #[test]
    fn test_agents_with_status() {
        let mut state = ControllerState {
            desired: DesiredState {
                replicas: 2,
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
        };

        state
            .agents
            .insert("a1".into(), make_agent_info("a1", AgentStatus::Running));
        state.agents.insert(
            "a2".into(),
            make_agent_info("a2", AgentStatus::Unresponsive),
        );

        let running = state.agents_with_status(&AgentStatus::Running);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, "a1");
    }

    #[test]
    fn test_sync_running_replicas() {
        let mut state = ControllerState {
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
        };

        state
            .agents
            .insert("a1".into(), make_agent_info("a1", AgentStatus::Running));
        state
            .agents
            .insert("a2".into(), make_agent_info("a2", AgentStatus::Running));
        state
            .agents
            .insert("a3".into(), make_agent_info("a3", AgentStatus::Failed));

        state.sync_running_replicas();
        assert_eq!(state.actual.running_replicas, 2);
    }

    #[test]
    fn test_time_since_heartbeat() {
        let info = AgentInfo::new("a1", "test");
        let elapsed = info.time_since_heartbeat();
        // Should be very small (nearly zero)
        assert!(elapsed.num_milliseconds() < 1000);
    }
}
