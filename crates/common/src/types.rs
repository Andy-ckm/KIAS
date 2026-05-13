use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a node
pub type NodeId = String;

/// Unique identifier for an agent
pub type AgentId = String;

/// Resource quantities (CPU in cores, memory in bytes)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resources {
    /// CPU cores (e.g., 0.5, 4.0)
    pub cpu: f64,
    /// Memory in bytes
    pub memory_bytes: u64,
    /// GPU count
    pub gpu: u32,
    /// Custom resources
    #[serde(default)]
    pub custom: HashMap<String, f64>,
}

impl Resources {
    /// Check if this resource set can satisfy the given request
    pub fn can_satisfy(&self, request: &Resources) -> bool {
        self.cpu >= request.cpu
            && self.memory_bytes >= request.memory_bytes
            && self.gpu >= request.gpu
    }

    /// Subtract requested resources (returns None if insufficient)
    pub fn subtract(&self, request: &Resources) -> Option<Resources> {
        if !self.can_satisfy(request) {
            return None;
        }
        Some(Resources {
            cpu: self.cpu - request.cpu,
            memory_bytes: self.memory_bytes - request.memory_bytes,
            gpu: self.gpu - request.gpu,
            custom: self.custom.clone(),
        })
    }
}

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
}

/// Represents a cluster node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub status: NodeStatus,
    pub total_resources: Resources,
    pub available_resources: Resources,
    pub allocated_agents: Vec<AgentId>,
    /// Node labels for affinity/anti-affinity
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

impl Node {
    /// Current load factor (0.0 = idle, 1.0 = fully loaded)
    pub fn load_factor(&self) -> f64 {
        if self.total_resources.cpu == 0.0 {
            return 1.0;
        }
        let cpu_used = self.total_resources.cpu - self.available_resources.cpu;
        cpu_used / self.total_resources.cpu
    }

    /// Number of running agents
    pub fn agent_count(&self) -> usize {
        self.allocated_agents.len()
    }
}

/// Scheduling priority
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Default)]
pub enum Priority {
    Low = 10,
    #[default]
    Medium = 50,
    High = 100,
    Critical = 200,
}


/// Represents an agent to be scheduled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub resource_request: Resources,
    #[serde(default)]
    pub priority: Priority,
    /// System prompt hash for cache-aware scheduling
    #[serde(default)]
    pub system_prompt_hash: Option<u64>,
    /// Node affinity rules
    #[serde(default)]
    pub affinity: Option<Affinity>,
    /// Node anti-affinity rules
    #[serde(default)]
    pub anti_affinity: Option<AntiAffinity>,
}

/// Affinity: prefer/require nodes matching labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Affinity {
    /// Required labels (hard constraint)
    #[serde(default)]
    pub required: HashMap<String, String>,
    /// Preferred labels (soft constraint, with weight)
    #[serde(default)]
    pub preferred: Vec<LabelPreference>,
}

/// Anti-affinity: avoid nodes matching labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiAffinity {
    /// Avoid nodes with these labels
    #[serde(default)]
    pub avoid_labels: HashMap<String, String>,
    /// Avoid co-locating with agents of these types
    #[serde(default)]
    pub avoid_agent_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelPreference {
    pub label: String,
    pub value: String,
    pub weight: f64,
}

/// Result of a scheduling decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    pub agent_id: AgentId,
    pub node_id: NodeId,
    pub algorithm: String,
    pub score: f64,
}
