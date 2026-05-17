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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status_default() {
        assert_eq!(NodeStatus::default(), NodeStatus::Unknown);
    }

    #[test]
    fn test_node_status_serialization() {
        assert_eq!(serde_json::to_string(&NodeStatus::Ready).unwrap(), ""Ready"");
        assert_eq!(serde_json::to_string(&NodeStatus::NotReady).unwrap(), ""NotReady"");
        assert_eq!(serde_json::to_string(&NodeStatus::Draining).unwrap(), ""Draining"");
    }

    #[test]
    fn test_node_status_deserialization() {
        let status: NodeStatus = serde_json::from_str("\"Ready\"").unwrap();
        assert_eq!(status, NodeStatus::Ready);
        let status: NodeStatus = serde_json::from_str("\"Draining\"").unwrap();
        assert_eq!(status, NodeStatus::Draining);
    }

    #[test]
    fn test_resource_capacity_default() {
        let rc = ResourceCapacity::default();
        assert!(rc.cpu.is_empty());
        assert!(rc.memory.is_empty());
        assert!(rc.gpu.is_empty());
    }

    #[test]
    fn test_node_serialization_roundtrip() {
        let node = Node {
            id: "node-1".to_string(),
            name: "worker-1".to_string(),
            status: NodeStatus::Ready,
            resources: ResourceCapacity { cpu: "4".to_string(), memory: "8Gi".to_string(), gpu: "0".to_string() },
            allocatable: ResourceCapacity { cpu: "4".to_string(), memory: "8Gi".to_string(), gpu: "0".to_string() },
            labels: HashMap::from([("zone".to_string(), "us-east".to_string())]),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_heartbeat: "2026-01-01T00:01:00Z".to_string(),
        };
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "node-1");
        assert_eq!(deserialized.name, "worker-1");
        assert_eq!(deserialized.status, NodeStatus::Ready);
        assert_eq!(deserialized.labels.get("zone").unwrap(), "us-east");
    }

    #[test]
    fn test_node_deserialization_from_json() {
        let json = r#"{"id":"n1","name":"node-1","status":"Ready","resources":{"cpu":"8","memory":"16Gi","gpu":"1"},"allocatable":{"cpu":"8","memory":"16Gi","gpu":"1"},"labels":{},"created_at":"2026-01-01","last_heartbeat":"2026-01-01"}"#;
        let node: Node = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "n1");
        assert_eq!(node.status, NodeStatus::Ready);
        assert_eq!(node.resources.cpu, "8");
    }
}
