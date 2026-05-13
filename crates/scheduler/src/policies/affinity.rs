use kias_common::{Agent, Node};

/// Affinity filter: filters and scores nodes based on agent affinity/anti-affinity rules.
pub struct AffinityFilter;

impl AffinityFilter {
    /// Filter nodes that satisfy hard affinity requirements.
    /// Returns only nodes whose labels contain all required affinity labels.
    pub fn filter_by_affinity<'a>(agent: &Agent, nodes: &'a [Node]) -> Vec<&'a Node> {
        let affinity = match &agent.affinity {
            Some(a) => a,
            None => return nodes.iter().collect(),
        };

        nodes
            .iter()
            .filter(|node| {
                // Check hard requirements: node must have all required labels
                affinity.required.iter().all(|(key, value)| {
                    node.labels.get(key.as_str()) == Some(value)
                })
            })
            .collect()
    }

    /// Remove nodes that match anti-affinity rules.
    pub fn filter_by_anti_affinity<'a>(agent: &Agent, nodes: &[&'a Node]) -> Vec<&'a Node> {
        let anti = match &agent.anti_affinity {
            Some(a) => a,
            None => return nodes.to_vec(),
        };

        nodes
            .iter()
            .copied()
            .filter(|node| {
                // Node must NOT match any avoid_labels
                !anti.avoid_labels.iter().any(|(key, value)| {
                    node.labels.get(key.as_str()) == Some(value)
                })
            })
            .collect()
    }

    /// Score a node based on soft (preferred) affinity.
    /// Returns a score from 0.0 to 1.0.
    pub fn affinity_score(agent: &Agent, node: &Node) -> f64 {
        let affinity = match &agent.affinity {
            Some(a) => a,
            None => return 0.5, // neutral
        };

        if affinity.preferred.is_empty() {
            return 0.5;
        }

        let total_weight: f64 = affinity.preferred.iter().map(|p| p.weight).sum();
        if total_weight == 0.0 {
            return 0.5;
        }

        let matched_weight: f64 = affinity
            .preferred
            .iter()
            .filter(|pref| {
                node.labels
                    .get(pref.label.as_str()) == Some(&pref.value)
            })
            .map(|p| p.weight)
            .sum();

        matched_weight / total_weight
    }

    /// Full pipeline: filter by hard constraints, then score by soft preferences.
    pub fn apply<'a>(agent: &Agent, nodes: &'a [Node]) -> Vec<(&'a Node, f64)> {
        let hard_filtered = Self::filter_by_affinity(agent, nodes);
        let anti_filtered = Self::filter_by_anti_affinity(agent, &hard_filtered);

        anti_filtered
            .iter()
            .map(|node| {
                let score = Self::affinity_score(agent, node);
                (*node, score)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Affinity, AntiAffinity, LabelPreference, Resources};
    use std::collections::HashMap;

    fn make_node(id: &str, labels: HashMap<String, String>) -> Node {
        Node {
            id: id.to_string(),
            status: kias_common::NodeStatus::Ready,
            total_resources: Resources::default(),
            available_resources: Resources::default(),
            allocated_agents: vec![],
            labels,
        }
    }

    #[test]
    fn test_hard_affinity_filter() {
        let mut labels = HashMap::new();
        labels.insert("zone".to_string(), "us-east".to_string());
        let nodes = vec![
            make_node("n1", labels.clone()),
            make_node("n2", HashMap::new()),
        ];

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: labels,
                preferred: vec![],
            }),
            anti_affinity: None,
        };

        let filtered = AffinityFilter::filter_by_affinity(&agent, &nodes);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "n1");
    }

    #[test]
    fn test_anti_affinity_filter() {
        let mut avoid = HashMap::new();
        avoid.insert("zone".to_string(), "eu-west".to_string());

        let mut labels1 = HashMap::new();
        labels1.insert("zone".to_string(), "eu-west".to_string());
        let mut labels2 = HashMap::new();
        labels2.insert("zone".to_string(), "us-east".to_string());

        let nodes = vec![make_node("n1", labels1), make_node("n2", labels2)];
        let node_refs: Vec<&Node> = nodes.iter().collect();

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: Some(AntiAffinity {
                avoid_labels: avoid,
                avoid_agent_types: vec![],
            }),
        };

        let filtered = AffinityFilter::filter_by_anti_affinity(&agent, &node_refs);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "n2");
    }

    #[test]
    fn test_soft_affinity_score() {
        let mut labels = HashMap::new();
        labels.insert("gpu".to_string(), "a100".to_string());

        let node = make_node("n1", labels);
        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: HashMap::new(),
                preferred: vec![LabelPreference {
                    label: "gpu".to_string(),
                    value: "a100".to_string(),
                    weight: 1.0,
                }],
            }),
            anti_affinity: None,
        };

        let score = AffinityFilter::affinity_score(&agent, &node);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }
}
