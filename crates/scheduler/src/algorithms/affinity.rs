//! Affinity scheduler: node affinity, pod anti-affinity, and zone-aware scheduling.
//!
//! Implements K8S-style scheduling rules:
//! - **Node affinity**: prefer or require nodes matching specific labels.
//! - **Pod anti-affinity**: avoid co-locating agents with matching labels.
//! - **Zone-aware**: spread agents across availability zones.

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};
use std::collections::HashMap;

use super::SchedulingAlgorithm;

// ─── Public types ───────────────────────────────────────────────────────

/// Whether an affinity rule is a hard requirement or a soft preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffinityType {
    /// Node must match — if no node matches the scheduling fails.
    Required,
    /// Node should match — non-matching nodes receive a penalty but are still eligible.
    Preferred,
}

/// A single affinity rule: match nodes whose label `label` == `value`.
#[derive(Debug, Clone)]
pub struct AffinityRule {
    /// The label key to match on the node.
    pub label: String,
    /// The required value for that label.
    pub value: String,
    /// Required or Preferred.
    pub affinity_type: AffinityType,
    /// Weight for scoring (only meaningful for Preferred rules).
    /// Higher weight = stronger preference.
    pub weight: f64,
}

/// Topology key for zone-aware spreading (e.g. `"topology.kubernetes.io/zone"`).
const ZONE_LABEL: &str = "topology.kubernetes.io/zone";

// ─── Scheduler ──────────────────────────────────────────────────────────

/// Affinity-aware scheduler implementing node affinity, anti-affinity, and zone spreading.
pub struct AffinityScheduler {
    /// Extra affinity rules beyond what the agent carries.
    /// In practice these come from the agent; this field lets us configure
    /// global defaults.
    extra_rules: Vec<AffinityRule>,
    /// Whether zone-aware spreading is enabled.
    zone_aware: bool,
}

impl AffinityScheduler {
    pub fn new() -> Self {
        Self {
            extra_rules: Vec::new(),
            zone_aware: true,
        }
    }

    /// Create with explicit zone-awareness toggle.
    pub fn with_zone_awareness(zone_aware: bool) -> Self {
        Self {
            extra_rules: Vec::new(),
            zone_aware,
        }
    }

    /// Create with extra affinity rules.
    pub fn with_rules(rules: Vec<AffinityRule>) -> Self {
        Self {
            extra_rules: rules,
            zone_aware: true,
        }
    }

    /// Collect all affinity rules for an agent (agent-defined + extra).
    fn all_rules(&self, agent: &Agent) -> Vec<AffinityRule> {
        let mut rules = self.extra_rules.clone();

        // Convert agent's affinity into rules
        if let Some(ref affinity) = agent.affinity {
            for (key, value) in &affinity.required {
                rules.push(AffinityRule {
                    label: key.clone(),
                    value: value.clone(),
                    affinity_type: AffinityType::Required,
                    weight: 100.0,
                });
            }
            for pref in &affinity.preferred {
                rules.push(AffinityRule {
                    label: pref.label.clone(),
                    value: pref.value.clone(),
                    affinity_type: AffinityType::Preferred,
                    weight: pref.weight,
                });
            }
        }

        rules
    }

    /// Check if a node satisfies all Required rules.
    fn satisfies_required(&self, node: &Node, rules: &[AffinityRule]) -> bool {
        rules
            .iter()
            .filter(|r| r.affinity_type == AffinityType::Required)
            .all(|r| node.labels.get(&r.label) == Some(&r.value))
    }

    /// Score a node for Preferred rules (higher = better).
    fn preferred_score(&self, node: &Node, rules: &[AffinityRule]) -> f64 {
        rules
            .iter()
            .filter(|r| r.affinity_type == AffinityType::Preferred)
            .map(|r| {
                if node.labels.get(&r.label) == Some(&r.value) {
                    r.weight
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            / 100.0 // normalize
    }

    /// Check anti-affinity: returns true if the node is acceptable.
    fn passes_anti_affinity(&self, node: &Node, agent: &Agent, _all_nodes: &[Node]) -> bool {
        if let Some(ref anti) = agent.anti_affinity {
            // Avoid nodes with specific labels
            for (key, value) in &anti.avoid_labels {
                if node.labels.get(key) == Some(value) {
                    return false;
                }
            }

            // Avoid co-locating with agents of certain types
            if !anti.avoid_agent_types.is_empty() {
                // Check if any agent already on this node is in the avoid list
                for allocated_id in &node.allocated_agents {
                    if anti
                        .avoid_agent_types
                        .iter()
                        .any(|t| allocated_id.contains(t))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Zone spreading score: prefer nodes in zones with fewer existing agents.
    fn zone_spread_score(&self, node: &Node, all_nodes: &[Node]) -> f64 {
        let zone = match node.labels.get(ZONE_LABEL) {
            Some(z) => z,
            None => return 0.0, // no zone label → neutral
        };

        // Count total agents in each zone
        let mut zone_counts: HashMap<&str, usize> = HashMap::new();
        for n in all_nodes {
            if n.status != NodeStatus::Ready {
                continue;
            }
            if let Some(z) = n.labels.get(ZONE_LABEL) {
                *zone_counts.entry(z.as_str()).or_insert(0) += n.agent_count();
            }
        }

        let this_zone_count = zone_counts.get(zone.as_str()).copied().unwrap_or(0);
        let max_count = zone_counts.values().copied().max().unwrap_or(1).max(1);

        // Invert: fewer agents in the zone → higher score
        1.0 - (this_zone_count as f64 / max_count as f64)
    }
}

impl Default for AffinityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchedulingAlgorithm for AffinityScheduler {
    fn name(&self) -> &str {
        "affinity"
    }

    async fn schedule(&self, agent: &Agent, nodes: &[Node]) -> Result<ScheduleResult, KiasError> {
        let available: Vec<&Node> = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Ready)
            .collect();

        if available.is_empty() {
            return Err(KiasError::NoAvailableNodes);
        }

        let rules = self.all_rules(agent);
        let has_required = rules
            .iter()
            .any(|r| r.affinity_type == AffinityType::Required);

        // Step 1: Filter by Required rules and anti-affinity
        let eligible: Vec<&&Node> = available
            .iter()
            .filter(|n| {
                self.satisfies_required(n, &rules) && self.passes_anti_affinity(n, agent, nodes)
            })
            .collect();

        if has_required && eligible.is_empty() {
            return Err(KiasError::Scheduler(format!(
                "No node satisfies required affinity rules for agent {}",
                agent.id
            )));
        }

        // Step 2: Score eligible nodes
        let candidates: Vec<(&&Node, f64)> = if eligible.is_empty() {
            // No required rules — score all available nodes
            available
                .iter()
                .filter(|n| self.passes_anti_affinity(n, agent, nodes))
                .map(|n| {
                    let aff = self.preferred_score(n, &rules);
                    let zone = self.zone_spread_score(n, nodes);
                    let load = 1.0 - n.load_factor();
                    (n, 0.5 * aff + 0.3 * zone + 0.2 * load)
                })
                .collect()
        } else {
            eligible
                .iter()
                .map(|n| {
                    let aff = self.preferred_score(n, &rules);
                    let zone = if self.zone_aware {
                        self.zone_spread_score(n, nodes)
                    } else {
                        0.0
                    };
                    let load = 1.0 - n.load_factor();
                    (n.to_owned(), 0.5 * aff + 0.3 * zone + 0.2 * load)
                })
                .collect()
        };

        if candidates.is_empty() {
            return Err(KiasError::NoAvailableNodes);
        }

        // Pick best scoring node
        let (best_node, best_score) = candidates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();

        tracing::info!(
            agent_id = %agent.id,
            node_id = %best_node.id,
            score = best_score,
            algorithm = "affinity",
            "Agent scheduled"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: best_node.id.clone(),
            algorithm: "affinity".to_string(),
            score: *best_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Affinity, AntiAffinity, LabelPreference, Resources};

    fn make_node(id: &str, labels: HashMap<String, String>, agents: Vec<String>) -> Node {
        Node {
            id: id.to_string(),
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
            allocated_agents: agents,
            labels,
        }
    }

    fn zone_node(id: &str, zone: &str, agent_count: usize) -> Node {
        let mut labels = HashMap::new();
        labels.insert(ZONE_LABEL.to_string(), zone.to_string());
        let agents: Vec<String> = (0..agent_count).map(|i| format!("agent-{}", i)).collect();
        make_node(id, labels, agents)
    }

    fn make_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    fn make_agent_with_affinity(id: &str, affinity: Affinity) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(affinity),
            anti_affinity: None,
            tenant_id: None,
        }
    }

    fn make_agent_with_anti_affinity(id: &str, anti: AntiAffinity) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: Some(anti),
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_required_affinity_matches() {
        let mut labels_a = HashMap::new();
        labels_a.insert("gpu".to_string(), "true".to_string());
        let mut labels_b = HashMap::new();
        labels_b.insert("cpu".to_string(), "fast".to_string());

        let nodes = vec![
            make_node("gpu-node", labels_a, vec![]),
            make_node("cpu-node", labels_b, vec![]),
        ];

        let mut required = HashMap::new();
        required.insert("gpu".to_string(), "true".to_string());
        let agent = make_agent_with_affinity(
            "a1",
            Affinity {
                required,
                preferred: vec![],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "gpu-node");
    }

    #[tokio::test]
    async fn test_required_affinity_no_match_errors() {
        let mut labels = HashMap::new();
        labels.insert("region".to_string(), "us-east".to_string());
        let nodes = vec![make_node("node-1", labels, vec![])];

        let mut required = HashMap::new();
        required.insert("region".to_string(), "eu-west".to_string());
        let agent = make_agent_with_affinity(
            "a1",
            Affinity {
                required,
                preferred: vec![],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::Scheduler(_))));
    }

    #[tokio::test]
    async fn test_preferred_affinity_scores_higher() {
        let mut labels_good = HashMap::new();
        labels_good.insert("ssd".to_string(), "true".to_string());
        let labels_bad = HashMap::new();

        let nodes = vec![
            make_node("ssd-node", labels_good, vec![]),
            make_node("hdd-node", labels_bad, vec![]),
        ];

        let agent = make_agent_with_affinity(
            "a1",
            Affinity {
                required: HashMap::new(),
                preferred: vec![LabelPreference {
                    label: "ssd".to_string(),
                    value: "true".to_string(),
                    weight: 80.0,
                }],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "ssd-node");
    }

    #[tokio::test]
    async fn test_anti_affinity_avoids_co_location() {
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "web".to_string());

        let nodes = vec![
            make_node("node-1", labels.clone(), vec!["web-agent-1".to_string()]),
            make_node("node-2", labels, vec![]),
        ];

        let avoid_types = vec!["web".to_string()];
        let agent = make_agent_with_anti_affinity(
            "a1",
            AntiAffinity {
                avoid_labels: HashMap::new(),
                avoid_agent_types: avoid_types,
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-2");
    }

    #[tokio::test]
    async fn test_anti_affinity_avoids_label_match() {
        let mut labels_bad = HashMap::new();
        labels_bad.insert("env".to_string(), "staging".to_string());
        let mut labels_good = HashMap::new();
        labels_good.insert("env".to_string(), "production".to_string());

        let nodes = vec![
            make_node("staging-node", labels_bad, vec![]),
            make_node("prod-node", labels_good, vec![]),
        ];

        let mut avoid = HashMap::new();
        avoid.insert("env".to_string(), "staging".to_string());
        let agent = make_agent_with_anti_affinity(
            "a1",
            AntiAffinity {
                avoid_labels: avoid,
                avoid_agent_types: vec![],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "prod-node");
    }

    #[tokio::test]
    async fn test_zone_aware_spreads_to_less_loaded_zone() {
        let nodes = vec![
            zone_node("z1-n1", "zone-a", 5), // 5 agents
            zone_node("z1-n2", "zone-a", 3), // 3 agents
            zone_node("z2-n1", "zone-b", 1), // 1 agent → zone-b is less loaded
            zone_node("z2-n2", "zone-b", 0), // 0 agents
        ];

        let agent = make_agent("a1");
        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        // Should prefer zone-b (fewer agents)
        assert!(
            result.node_id.starts_with("z2"),
            "Expected zone-b node, got {}",
            result.node_id
        );
    }

    #[tokio::test]
    async fn test_empty_nodes_error() {
        let scheduler = AffinityScheduler::new();
        let agent = make_agent("a1");
        let result = scheduler.schedule(&agent, &[]).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }

    #[tokio::test]
    async fn test_not_ready_nodes_error() {
        let mut node = make_node("n1", HashMap::new(), vec![]);
        node.status = NodeStatus::NotReady;
        let scheduler = AffinityScheduler::new();
        let agent = make_agent("a1");
        let result = scheduler.schedule(&agent, &[node]).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }

    #[tokio::test]
    async fn test_no_affinity_picks_least_loaded_zone() {
        let nodes = vec![zone_node("n1", "z-a", 4), zone_node("n2", "z-b", 0)];
        let agent = make_agent("a1");
        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "n2"); // z-b has fewer agents
    }

    #[tokio::test]
    async fn test_zone_disabled() {
        let nodes = vec![zone_node("n1", "z-a", 4), zone_node("n2", "z-b", 0)];
        let agent = make_agent("a1");
        let scheduler = AffinityScheduler::with_zone_awareness(false);
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scheduler_name() {
        let scheduler = AffinityScheduler::new();
        assert_eq!(scheduler.name(), "affinity");
    }

    #[tokio::test]
    async fn test_multiple_preferred_rules() {
        let mut labels_best = HashMap::new();
        labels_best.insert("gpu".to_string(), "a100".to_string());
        labels_best.insert("ssd".to_string(), "true".to_string());

        let mut labels_ok = HashMap::new();
        labels_ok.insert("gpu".to_string(), "a100".to_string());

        let labels_worst = HashMap::new();

        let nodes = vec![
            make_node("best", labels_best, vec![]),
            make_node("ok", labels_ok, vec![]),
            make_node("worst", labels_worst, vec![]),
        ];

        let agent = make_agent_with_affinity(
            "a1",
            Affinity {
                required: HashMap::new(),
                preferred: vec![
                    LabelPreference {
                        label: "gpu".to_string(),
                        value: "a100".to_string(),
                        weight: 50.0,
                    },
                    LabelPreference {
                        label: "ssd".to_string(),
                        value: "true".to_string(),
                        weight: 50.0,
                    },
                ],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "best");
    }

    #[tokio::test]
    async fn test_required_plus_preferred() {
        let mut labels_a = HashMap::new();
        labels_a.insert("region".to_string(), "us".to_string());
        labels_a.insert("ssd".to_string(), "true".to_string());

        let mut labels_b = HashMap::new();
        labels_b.insert("region".to_string(), "us".to_string());
        // No SSD

        let nodes = vec![
            make_node("us-ssd", labels_a, vec![]),
            make_node("us-hdd", labels_b, vec![]),
        ];

        let mut required = HashMap::new();
        required.insert("region".to_string(), "us".to_string());
        let agent = make_agent_with_affinity(
            "a1",
            Affinity {
                required,
                preferred: vec![LabelPreference {
                    label: "ssd".to_string(),
                    value: "true".to_string(),
                    weight: 90.0,
                }],
            },
        );

        let scheduler = AffinityScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "us-ssd");
    }
}
