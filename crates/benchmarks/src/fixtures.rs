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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_nodes_count() {
        let nodes = make_nodes(5);
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn test_make_nodes_zero() {
        let nodes = make_nodes(0);
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_make_nodes_ids_sequential() {
        let nodes = make_nodes(3);
        assert_eq!(nodes[0].id, "node-0");
        assert_eq!(nodes[1].id, "node-1");
        assert_eq!(nodes[2].id, "node-2");
    }

    #[test]
    fn test_make_nodes_uniform_resources() {
        let nodes = make_nodes(4);
        for n in &nodes {
            assert_eq!(n.total_resources.cpu, 8.0);
            assert_eq!(n.total_resources.memory_bytes, 16 * 1024 * 1024 * 1024);
            assert_eq!(n.total_resources.gpu, 2);
            assert_eq!(n.status, NodeStatus::Ready);
            assert!(n.allocated_agents.is_empty());
            assert!(n.labels.is_empty());
        }
    }

    #[test]
    fn test_make_nodes_available_equals_total() {
        let nodes = make_nodes(3);
        for n in &nodes {
            assert_eq!(n.available_resources.cpu, n.total_resources.cpu);
            assert_eq!(n.available_resources.memory_bytes, n.total_resources.memory_bytes);
            assert_eq!(n.available_resources.gpu, n.total_resources.gpu);
        }
    }

    #[test]
    fn test_make_heterogeneous_nodes_count() {
        let nodes = make_heterogeneous_nodes(10);
        assert_eq!(nodes.len(), 10);
    }

    #[test]
    fn test_make_heterogeneous_nodes_varying_resources() {
        let nodes = make_heterogeneous_nodes(20);
        let cpus: Vec<f64> = nodes.iter().map(|n| n.total_resources.cpu).collect();
        let first = cpus[0];
        assert!(cpus.iter().any(|c| *c != first), "resources should vary");
    }

    #[test]
    fn test_make_heterogeneous_nodes_cpu_range() {
        let nodes = make_heterogeneous_nodes(10);
        for n in &nodes {
            assert!(n.total_resources.cpu >= 4.0, "cpu should be >= 4.0");
            assert!(n.total_resources.cpu <= 16.0, "cpu should be <= 16.0");
        }
    }

    #[test]
    fn test_make_heterogeneous_nodes_some_have_zone_labels() {
        let nodes = make_heterogeneous_nodes(12);
        let with_zone = nodes.iter().filter(|n| n.labels.contains_key("zone")).count();
        assert!(with_zone > 0, "some nodes should have zone labels");
        assert!(with_zone >= 4);
    }

    #[test]
    fn test_make_heterogeneous_nodes_ids_sequential() {
        let nodes = make_heterogeneous_nodes(3);
        assert_eq!(nodes[0].id, "node-0");
        assert_eq!(nodes[2].id, "node-2");
    }

    #[test]
    fn test_make_agents_count() {
        let agents = make_agents(8);
        assert_eq!(agents.len(), 8);
    }

    #[test]
    fn test_make_agents_zero() {
        let agents = make_agents(0);
        assert!(agents.is_empty());
    }

    #[test]
    fn test_make_agents_round_robin_priority() {
        let agents = make_agents(8);
        assert_eq!(agents[0].priority, Priority::Critical);
        assert_eq!(agents[1].priority, Priority::High);
        assert_eq!(agents[2].priority, Priority::Medium);
        assert_eq!(agents[3].priority, Priority::Low);
        assert_eq!(agents[4].priority, Priority::Critical);
    }

    #[test]
    fn test_make_agents_ids_sequential() {
        let agents = make_agents(3);
        assert_eq!(agents[0].id, "agent-0");
        assert_eq!(agents[1].id, "agent-1");
        assert_eq!(agents[2].id, "agent-2");
    }

    #[test]
    fn test_make_agents_uniform_resources() {
        let agents = make_agents(4);
        for a in &agents {
            assert_eq!(a.resource_request.cpu, 0.5);
            assert_eq!(a.resource_request.memory_bytes, 512 * 1024 * 1024);
            assert_eq!(a.resource_request.gpu, 0);
        }
    }

    #[test]
    fn test_make_agents_prompt_hash_mod_10() {
        let agents = make_agents(15);
        for (i, a) in agents.iter().enumerate() {
            assert_eq!(a.system_prompt_hash, Some((i as u64) % 10));
        }
    }

    #[test]
    fn test_make_gpu_nodes_count() {
        let nodes = make_gpu_nodes(6);
        assert_eq!(nodes.len(), 6);
    }

    #[test]
    fn test_make_gpu_nodes_have_gpu_type_label() {
        let nodes = make_gpu_nodes(4);
        for n in &nodes {
            assert!(n.labels.contains_key("gpu-type"));
            assert!(n.labels.contains_key("gpu-interconnect"));
            assert!(n.labels.contains_key("gpu-memory-mb"));
        }
    }

    #[test]
    fn test_make_gpu_nodes_alternate_types() {
        let nodes = make_gpu_nodes(4);
        assert_eq!(nodes[0].labels["gpu-type"], "nvidia-a100");
        assert_eq!(nodes[1].labels["gpu-type"], "nvidia-h100");
        assert_eq!(nodes[2].labels["gpu-type"], "nvidia-a100");
    }

    #[test]
    fn test_make_gpu_nodes_interconnect() {
        let nodes = make_gpu_nodes(6);
        assert_eq!(nodes[0].labels["gpu-interconnect"], "nvlink");
        assert_eq!(nodes[1].labels["gpu-interconnect"], "pcie");
        assert_eq!(nodes[3].labels["gpu-interconnect"], "nvlink");
    }

    #[test]
    fn test_make_gpu_nodes_gpu_count_range() {
        let nodes = make_gpu_nodes(10);
        for n in &nodes {
            assert!(n.total_resources.gpu >= 2);
            assert!(n.total_resources.gpu <= 8);
        }
    }

    #[test]
    fn test_make_gpu_nodes_ids() {
        let nodes = make_gpu_nodes(3);
        assert_eq!(nodes[0].id, "gpu-node-0");
        assert_eq!(nodes[2].id, "gpu-node-2");
    }

    #[test]
    fn test_make_gpu_agents_count() {
        let agents = make_gpu_agents(5);
        assert_eq!(agents.len(), 5);
    }

    #[test]
    fn test_make_gpu_agents_request_one_gpu() {
        let agents = make_gpu_agents(3);
        for a in &agents {
            assert_eq!(a.resource_request.gpu, 1);
            assert_eq!(a.resource_request.cpu, 4.0);
            assert_eq!(a.resource_request.memory_bytes, 8 * 1024 * 1024 * 1024);
        }
    }

    #[test]
    fn test_make_gpu_agents_ids() {
        let agents = make_gpu_agents(3);
        assert_eq!(agents[0].id, "gpu-agent-0");
        assert_eq!(agents[2].id, "gpu-agent-2");
    }

    #[test]
    fn test_make_gpu_agents_medium_priority() {
        let agents = make_gpu_agents(4);
        for a in &agents {
            assert_eq!(a.priority, Priority::Medium);
        }
    }

    #[test]
    fn test_make_gpu_agents_no_prompt_hash() {
        let agents = make_gpu_agents(3);
        for a in &agents {
            assert!(a.system_prompt_hash.is_none());
        }
    }

    #[test]
    fn test_make_zone_nodes_count() {
        let nodes = make_zone_nodes(9);
        assert_eq!(nodes.len(), 9);
    }

    #[test]
    fn test_make_zone_nodes_distributed() {
        let nodes = make_zone_nodes(6);
        assert_eq!(nodes[0].labels["topology.kubernetes.io/zone"], "zone-a");
        assert_eq!(nodes[1].labels["topology.kubernetes.io/zone"], "zone-b");
        assert_eq!(nodes[2].labels["topology.kubernetes.io/zone"], "zone-c");
        assert_eq!(nodes[3].labels["topology.kubernetes.io/zone"], "zone-a");
    }

    #[test]
    fn test_make_zone_nodes_ids() {
        let nodes = make_zone_nodes(3);
        assert_eq!(nodes[0].id, "zone-node-0");
        assert_eq!(nodes[2].id, "zone-node-2");
    }

    #[test]
    fn test_make_zone_nodes_uniform_resources() {
        let nodes = make_zone_nodes(4);
        for n in &nodes {
            assert_eq!(n.total_resources.cpu, 8.0);
            assert_eq!(n.total_resources.gpu, 0);
        }
    }

    #[test]
    fn test_make_agents_with_affinity_count() {
        let agents = make_agents_with_affinity(10);
        assert_eq!(agents.len(), 10);
    }

    #[test]
    fn test_make_agents_with_affinity_some_have_affinity() {
        let agents = make_agents_with_affinity(12);
        let with_affinity = agents.iter().filter(|a| a.affinity.is_some()).count();
        assert!(with_affinity >= 4);
    }

    #[test]
    fn test_make_agents_with_affinity_target_us_east() {
        let agents = make_agents_with_affinity(6);
        for a in &agents {
            if let Some(ref aff) = a.affinity {
                assert_eq!(aff.required.get("zone"), Some(&"us-east".to_string()));
                assert!(aff.preferred.is_empty());
            }
        }
    }

    #[test]
    fn test_make_agents_with_affinity_ids() {
        let agents = make_agents_with_affinity(3);
        assert_eq!(agents[0].id, "agent-0");
        assert_eq!(agents[2].id, "agent-2");
    }

    #[test]
    fn test_make_agents_with_affinity_uniform_resources() {
        let agents = make_agents_with_affinity(5);
        for a in &agents {
            assert_eq!(a.resource_request.cpu, 0.5);
            assert_eq!(a.resource_request.memory_bytes, 256 * 1024 * 1024);
        }
    }
}

