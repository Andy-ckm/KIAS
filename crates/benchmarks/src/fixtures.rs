//! Shared test fixtures for benchmarks.
//!
//! Provides deterministic agent and node generators with configurable parameters
//! so each benchmark can tune cluster size, resource distribution, and affinity rules.

use kias_common::{Affinity, Agent, Node, NodeStatus, Priority, Resources};
use std::collections::HashMap;

/// Generate `n` nodes with uniform resources.
pub fn make_nodes(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| Node {
            id: format!("node-{}", i),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16 * 1024 * 1024 * 1024,
                gpu: 2,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16 * 1024 * 1024 * 1024,
                gpu: 2,
                ..Default::default()
            },
            allocated_agents: vec![],
            labels: HashMap::new(),
        })
        .collect()
}

/// Generate `n` nodes with heterogeneous resource profiles.
///
/// Nodes have varying CPU (4-16 cores), memory (8-64 GB), and GPU (0-4) counts
/// to exercise resource-aware scheduling logic.
pub fn make_heterogeneous_nodes(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| {
            let cpu = 4.0 + ((i as f64 * 1.7) % 12.0);
            let mem = (8 + ((i * 7) % 56)) as u64 * 1024 * 1024 * 1024;
            let gpu = (i % 5) as u32;
            Node {
                id: format!("node-{}", i),
                status: NodeStatus::Ready,
                total_resources: Resources {
                    cpu,
                    memory_bytes: mem,
                    gpu,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu,
                    memory_bytes: mem,
                    gpu,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels: if i % 3 == 0 {
                    let mut m = HashMap::new();
                    m.insert("zone".to_string(), "us-east".to_string());
                    m
                } else {
                    HashMap::new()
                },
            }
        })
        .collect()
}

/// Generate `n` agents with round-robin priority and small resource requests.
pub fn make_agents(n: usize) -> Vec<Agent> {
    (0..n)
        .map(|i| {
            let priority = match i % 4 {
                0 => Priority::Critical,
                1 => Priority::High,
                2 => Priority::Medium,
                _ => Priority::Low,
            };
            Agent {
                id: format!("agent-{}", i),
                name: format!("agent-{}", i),
                resource_request: Resources {
                    cpu: 0.5,
                    memory_bytes: 512 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                priority,
                system_prompt_hash: Some((i as u64) % 10),
                affinity: None,
                anti_affinity: None,
                tenant_id: None,
            }
        })
        .collect()
}

/// Generate `n` GPU nodes with heterogeneous GPU types and NVLink labels.
///
/// Nodes alternate between nvidia-a100 and nvidia-h100 GPU types, with some
/// having NVLink interconnects. Each node has 2-8 GPUs.
pub fn make_gpu_nodes(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| {
            let gpu_count = (2 + (i % 4) * 2) as u32;
            let gpu_type = if i % 2 == 0 {
                "nvidia-a100"
            } else {
                "nvidia-h100"
            };
            let interconnect = if i % 3 == 0 { "nvlink" } else { "pcie" };
            let mut labels = HashMap::new();
            labels.insert("gpu-type".to_string(), gpu_type.to_string());
            labels.insert("gpu-interconnect".to_string(), interconnect.to_string());
            labels.insert(
                "gpu-memory-mb".to_string(),
                if gpu_type == "nvidia-a100" {
                    "81920".to_string()
                } else {
                    "80000".to_string()
                },
            );
            Node {
                id: format!("gpu-node-{}", i),
                status: NodeStatus::Ready,
                total_resources: Resources {
                    cpu: 16.0 + (i as f64 * 2.0) % 32.0,
                    memory_bytes: (64 + ((i * 7) % 128)) as u64 * 1024 * 1024 * 1024,
                    gpu: gpu_count,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu: 16.0 + (i as f64 * 2.0) % 32.0,
                    memory_bytes: (64 + ((i * 7) % 128)) as u64 * 1024 * 1024 * 1024,
                    gpu: gpu_count,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels,
            }
        })
        .collect()
}

/// Generate `n` agents requesting 1 GPU each.
pub fn make_gpu_agents(n: usize) -> Vec<Agent> {
    (0..n)
        .map(|i| Agent {
            id: format!("gpu-agent-{}", i),
            name: format!("gpu-agent-{}", i),
            resource_request: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 1,
                ..Default::default()
            },
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        })
        .collect()
}

/// Generate `n` zone-aware nodes spread across 3 zones.
pub fn make_zone_nodes(n: usize) -> Vec<Node> {
    let zones = ["zone-a", "zone-b", "zone-c"];
    (0..n)
        .map(|i| {
            let zone = zones[i % zones.len()];
            let mut labels = HashMap::new();
            labels.insert("topology.kubernetes.io/zone".to_string(), zone.to_string());
            Node {
                id: format!("zone-node-{}", i),
                status: NodeStatus::Ready,
                total_resources: Resources {
                    cpu: 8.0,
                    memory_bytes: 16 * 1024 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu: 8.0,
                    memory_bytes: 16 * 1024 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels,
            }
        })
        .collect()
}

/// Generate `n` agents, some with affinity constraints targeting zone=us-east.
pub fn make_agents_with_affinity(n: usize) -> Vec<Agent> {
    (0..n)
        .map(|i| {
            let affinity = if i % 3 == 0 {
                let mut required = HashMap::new();
                required.insert("zone".to_string(), "us-east".to_string());
                Some(Affinity {
                    required,
                    preferred: vec![],
                })
            } else {
                None
            };
            Agent {
                id: format!("agent-{}", i),
                name: format!("agent-{}", i),
                resource_request: Resources {
                    cpu: 0.5,
                    memory_bytes: 256 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                priority: Priority::Medium,
                system_prompt_hash: None,
                affinity,
                anti_affinity: None,
                tenant_id: None,
            }
        })
        .collect()
}
