use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

    mod delivery_tests {
        use super::*;

        #[test]
        fn test_delivery_log_basic_read_write() {
            let mut log = DeliveryLog::new();
            log.begin_operation("agent-1", "op-1");
            log.record_read("agent-1", "config.timeout");
            log.commit_write("agent-1", "config.timeout");
            log.end_operation(
                "agent-1",
                HashSet::from(["config.timeout".to_string()]),
                false,
            );
            assert_eq!(log.get_version("config.timeout"), 1);
            assert_eq!(log.completed_count(), 1);
        }

        #[test]
        fn test_delivery_log_no_conflict_different_keys() {
            let mut log = DeliveryLog::new();
            log.begin_operation("agent-1", "op-1");
            log.record_read("agent-1", "key-a");

            log.begin_operation("agent-2", "op-2");
            log.record_read("agent-2", "key-b");

            // agent-1 writes key-a: no conflict (agent-2 didn't read it)
            assert_eq!(log.check_write("agent-1", "key-a"), ConflictCheck::Safe);
        }

        #[test]
        fn test_delivery_log_conflict_detected() {
            let mut log = DeliveryLog::new();
            // Set initial version to 1
            log.commit_write("system", "shared-state");

            // agent-1 reads version 1
            log.begin_operation("agent-1", "op-1");
            log.record_read("agent-1", "shared-state");

            // System updates the key to version 2
            log.commit_write("system", "shared-state");

            // agent-2 reads version 2
            log.begin_operation("agent-2", "op-2");
            log.record_read("agent-2", "shared-state");

            // agent-1 tries to write: conflict because agent-1 read version 1,
            // but current version is 2 (agent-2 read the newer version)
            let result = log.check_write("agent-1", "shared-state");
            assert!(matches!(result, ConflictCheck::Conflict { .. }));
        }

        #[test]
        fn test_delivery_log_read_set_keys() {
            let mut log = DeliveryLog::new();
            log.begin_operation("agent-1", "op-1");
            log.record_read("agent-1", "a");
            log.record_read("agent-1", "b");
            log.record_read("agent-1", "a"); // duplicate read

            let keys = log.agent_read_keys("agent-1");
            assert!(keys.contains("a"));
            assert!(keys.contains("b"));
        }

        #[test]
        fn test_delivery_log_version_tracking() {
            let mut log = DeliveryLog::new();
            assert_eq!(log.get_version("new-key"), 0);

            log.commit_write("agent-1", "counter");
            assert_eq!(log.get_version("counter"), 1);

            log.commit_write("agent-2", "counter");
            assert_eq!(log.get_version("counter"), 2);
        }

        #[test]
        fn test_delivery_log_default_trait() {
            let log = DeliveryLog::default();
            assert_eq!(log.completed_count(), 0);
        }
    }
}

// ── DeliveryLog: Observable-Read Isolation for multi-agent state ──────
//
// Tracks per-agent read-sets to prevent structural race conditions.
// When agent A reads key "X", we record it. When agent B writes "X",
// we detect the conflict and can intervene (block, notify, merge).

/// A single read observation by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadEntry {
    /// Which key/path was read.
    pub key: String,
    /// Version of the value at read time.
    pub version: u64,
    /// When the read happened.
    pub read_at: DateTime<Utc>,
}

/// Per-agent read-set: everything this agent has read during its current operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadSet {
    pub agent_id: String,
    pub entries: Vec<ReadEntry>,
    pub operation_id: String,
    pub started_at: DateTime<Utc>,
}

impl ReadSet {
    pub fn new(agent_id: impl Into<String>, operation_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            entries: Vec::new(),
            operation_id: operation_id.into(),
            started_at: Utc::now(),
        }
    }

    pub fn record_read(&mut self, key: impl Into<String>, version: u64) {
        self.entries.push(ReadEntry {
            key: key.into(),
            version,
            read_at: Utc::now(),
        });
    }

    pub fn keys(&self) -> HashSet<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }
}

/// Result of a write-conflict check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictCheck {
    /// No conflict — safe to write.
    Safe,
    /// Conflict detected: another agent read the same key at an older version.
    Conflict {
        key: String,
        conflicting_agents: Vec<String>,
        expected_version: u64,
        actual_version: u64,
    },
}

/// Observable-Read Isolation delivery log.
///
/// Tracks read-sets per agent and detects write conflicts.
pub struct DeliveryLog {
    /// Active read-sets per agent.
    read_sets: HashMap<String, ReadSet>,
    /// Current version per key.
    versions: HashMap<String, u64>,
    /// History of completed operations (for audit).
    completed: Vec<CompletedOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedOperation {
    pub agent_id: String,
    pub operation_id: String,
    pub read_keys: HashSet<String>,
    pub wrote_keys: HashSet<String>,
    pub completed_at: DateTime<Utc>,
    pub had_conflict: bool,
}

impl DeliveryLog {
    pub fn new() -> Self {
        Self {
            read_sets: HashMap::new(),
            versions: HashMap::new(),
            completed: Vec::new(),
        }
    }

    /// Begin tracking reads for an agent operation.
    pub fn begin_operation(&mut self, agent_id: &str, operation_id: &str) {
        let read_set = ReadSet::new(agent_id, operation_id);
        self.read_sets.insert(agent_id.to_string(), read_set);
    }

    /// Record that an agent read a key.
    pub fn record_read(&mut self, agent_id: &str, key: &str) {
        let version = self.versions.get(key).copied().unwrap_or(0);
        if let Some(rs) = self.read_sets.get_mut(agent_id) {
            rs.record_read(key, version);
        }
    }

    /// Check if a write would conflict with any other agent's read-set.
    pub fn check_write(&self, writer_id: &str, key: &str) -> ConflictCheck {
        let current_version = self.versions.get(key).copied().unwrap_or(0);
        let mut conflicting_agents = Vec::new();

        for (agent_id, rs) in &self.read_sets {
            if agent_id == writer_id {
                continue;
            }
            for entry in &rs.entries {
                if entry.key == key {
                    conflicting_agents.push(agent_id.clone());
                }
            }
        }

        if conflicting_agents.is_empty() {
            ConflictCheck::Safe
        } else {
            ConflictCheck::Conflict {
                key: key.to_string(),
                conflicting_agents,
                expected_version: current_version,
                actual_version: current_version + 1,
            }
        }
    }

    /// Commit a write: bump version and record completion.
    pub fn commit_write(&mut self, _agent_id: &str, key: &str) {
        let version = self.versions.entry(key.to_string()).or_insert(0);
        *version += 1;
    }

    /// Complete an agent's operation, moving read-set to history.
    pub fn end_operation(
        &mut self,
        agent_id: &str,
        wrote_keys: HashSet<String>,
        had_conflict: bool,
    ) {
        if let Some(rs) = self.read_sets.remove(agent_id) {
            let read_keys = rs.entries.iter().map(|e| e.key.clone()).collect();
            self.completed.push(CompletedOperation {
                agent_id: agent_id.to_string(),
                operation_id: rs.operation_id,
                read_keys,
                wrote_keys,
                completed_at: Utc::now(),
                had_conflict,
            });
        }
    }

    /// Get current version of a key.
    pub fn get_version(&self, key: &str) -> u64 {
        self.versions.get(key).copied().unwrap_or(0)
    }

    /// Get all keys an agent has read in its current operation.
    pub fn agent_read_keys(&self, agent_id: &str) -> HashSet<String> {
        self.read_sets
            .get(agent_id)
            .map(|rs| rs.keys().into_iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    /// Get number of completed operations.
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }
}

impl Default for DeliveryLog {
    fn default() -> Self {
        Self::new()
    }
}

// ── DeliveryLog tests ──────────────────────────────────────────
