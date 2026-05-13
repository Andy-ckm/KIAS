use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub enum NodeStatus {
    Ready,
    NotReady,
    #[default]
    Unknown,
    Draining,
}

/// Node resource capacity
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceCapacity {
    pub cpu: String,
    pub memory: String,
    pub gpu: String,
}

/// Full Node object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub status: NodeStatus,
    pub resources: ResourceCapacity,
    pub allocatable: ResourceCapacity,
    pub labels: HashMap<String, String>,
    pub created_at: String,
    pub last_heartbeat: String,
}
