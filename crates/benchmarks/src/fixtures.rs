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
            }
        })
        .collect()
}
