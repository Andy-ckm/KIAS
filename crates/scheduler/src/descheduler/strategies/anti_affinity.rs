//! RemoveAgentsViolatingAntiAffinity strategy.
//!
//! Finds agents that are co-located with agents they should avoid
//! (per `anti_affinity.avoid_agent_types`) and proposes evicting
//! the lower-priority agent.

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node};
use std::collections::HashMap;

use super::DeschedulerStrategy;
use crate::descheduler::types::{Eviction, EvictionReason};

/// Proposes evictions for agents violating anti-affinity constraints.
pub struct AntiAffinityViolationStrategy;

impl AntiAffinityViolationStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AntiAffinityViolationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeschedulerStrategy for AntiAffinityViolationStrategy {
    fn name(&self) -> &str {
        "remove-anti-affinity-violations"
    }

    async fn propose_evictions(
        &self,
        nodes: &[Node],
        agents: &[Agent],
    ) -> Result<Vec<Eviction>, KiasError> {
        let agent_map: HashMap<&str, &Agent> = agents.iter().map(|a| (a.id.as_str(), a)).collect();

        let mut evictions = Vec::new();
        let mut evicted_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for node in nodes {
            if node.status != kias_common::NodeStatus::Ready {
                continue;
            }

            let node_agents: Vec<&Agent> = node
                .allocated_agents
                .iter()
                .filter_map(|id| agent_map.get(id.as_str()).copied())
                .collect();

            // Check each pair for anti-affinity violations
            for i in 0..node_agents.len() {
                for j in (i + 1)..node_agents.len() {
                    let a = node_agents[i];
                    let b = node_agents[j];

                    // Check if a wants to avoid b's type
                    let a_violates = a
                        .anti_affinity
                        .as_ref()
                        .map(|aa| aa.avoid_agent_types.contains(&b.name))
                        .unwrap_or(false);

                    // Check if b wants to avoid a's type
                    let b_violates = b
                        .anti_affinity
                        .as_ref()
                        .map(|aa| aa.avoid_agent_types.contains(&a.name))
                        .unwrap_or(false);

                    if a_violates || b_violates {
                        // Evict the lower-priority agent
                        let (victim, _keeper) = if a.priority <= b.priority {
                            (a, b)
                        } else {
                            (b, a)
                        };

                        if !evicted_ids.contains(&victim.id) {
                            evicted_ids.insert(victim.id.clone());
                            evictions.push(Eviction {
                                agent_id: victim.id.clone(),
                                source_node: node.id.clone(),
                                reason: EvictionReason::AntiAffinityViolation {
                                    conflicting_agent_id: if victim.id == a.id {
                                        b.id.clone()
                                    } else {
                                        a.id.clone()
                                    },
                                    constraint: format!(
                                        "avoid_agent_types conflict on node {}",
                                        node.id
                                    ),
                                },
                                priority: victim.priority,
                            });
                        }
                    }
                }
            }
        }

        tracing::info!(
            evictions = evictions.len(),
            "AntiAffinityViolation analysis complete"
        );

        Ok(evictions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{AntiAffinity, Priority, Resources};

    fn make_node(id: &str, agent_ids: Vec<&str>) -> Node {
        Node {
            id: id.to_string(),
            status: kias_common::NodeStatus::Ready,
            total_resources: Resources {
                cpu: 8.0,
                memory_bytes: 16_000_000_000,
                gpu: 0,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: 4.0,
                memory_bytes: 8_000_000_000,
                gpu: 0,
                ..Default::default()
            },
            allocated_agents: agent_ids.into_iter().map(String::from).collect(),
            labels: Default::default(),
        }
    }

    fn make_agent_with_anti_affinity(
        id: &str,
        name: &str,
        priority: Priority,
        avoid_types: Vec<&str>,
    ) -> Agent {
        Agent {
            id: id.to_string(),
            name: name.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: Some(AntiAffinity {
                avoid_labels: Default::default(),
                avoid_agent_types: avoid_types.into_iter().map(String::from).collect(),
            }),
            tenant_id: None,
        }
    }

    fn make_agent_basic(id: &str, name: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: name.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_no_violation_when_no_anti_affinity() {
        let nodes = vec![make_node("n1", vec!["a1", "a2"])];
        let agents = vec![
            make_agent_basic("a1", "type-a", Priority::Medium),
            make_agent_basic("a2", "type-b", Priority::Medium),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_evicts_lower_priority_violator() {
        let nodes = vec![make_node("n1", vec!["a1", "a2"])];
        let agents = vec![
            make_agent_with_anti_affinity("a1", "type-a", Priority::Low, vec!["type-b"]),
            make_agent_basic("a2", "type-b", Priority::High),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert_eq!(evictions.len(), 1);
        assert_eq!(evictions[0].agent_id, "a1"); // lower priority
    }

    #[tokio::test]
    async fn test_no_violation_when_different_nodes() {
        let nodes = vec![make_node("n1", vec!["a1"]), make_node("n2", vec!["a2"])];
        let agents = vec![
            make_agent_with_anti_affinity("a1", "type-a", Priority::Low, vec!["type-b"]),
            make_agent_basic("a2", "type-b", Priority::High),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_bidirectional_anti_affinity() {
        let nodes = vec![make_node("n1", vec!["a1", "a2"])];
        let agents = vec![
            make_agent_with_anti_affinity("a1", "type-a", Priority::High, vec!["type-b"]),
            make_agent_with_anti_affinity("a2", "type-b", Priority::Low, vec!["type-a"]),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        assert_eq!(evictions.len(), 1);
        assert_eq!(evictions[0].agent_id, "a2"); // lower priority evicted
    }

    #[tokio::test]
    async fn test_skips_not_ready_nodes() {
        let mut node = make_node("n1", vec!["a1", "a2"]);
        node.status = kias_common::NodeStatus::NotReady;
        let nodes = vec![node];
        let agents = vec![
            make_agent_with_anti_affinity("a1", "type-a", Priority::Low, vec!["type-b"]),
            make_agent_basic("a2", "type-b", Priority::High),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();
        assert!(evictions.is_empty());
    }

    #[tokio::test]
    async fn test_no_duplicate_evictions() {
        // a1 avoids type-b, a3 also avoids type-b — but a1 should only be evicted once
        let nodes = vec![make_node("n1", vec!["a1", "a2", "a3"])];
        let agents = vec![
            make_agent_with_anti_affinity("a1", "type-a", Priority::Low, vec!["type-b"]),
            make_agent_basic("a2", "type-b", Priority::High),
            make_agent_with_anti_affinity("a3", "type-c", Priority::Medium, vec!["type-b"]),
        ];

        let strategy = AntiAffinityViolationStrategy::new();
        let evictions = strategy.propose_evictions(&nodes, &agents).await.unwrap();

        // a1 conflicts with a2, a3 conflicts with a2
        // a1 (Low) evicted for a2, a3 (Medium) evicted for a2
        let evicted_ids: Vec<&str> = evictions.iter().map(|e| e.agent_id.as_str()).collect();
        assert!(evicted_ids.contains(&"a1"));
        assert!(evicted_ids.contains(&"a3"));
        // No duplicates
        assert_eq!(evicted_ids.len(), evictions.len());
    }
}
