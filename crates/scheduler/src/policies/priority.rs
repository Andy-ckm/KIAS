use kias_common::{Agent, Priority};

/// Priority sorter: reorders agents by priority before scheduling.
///
/// Higher priority agents are scheduled first, ensuring critical workloads
/// get the best available nodes.
pub struct PrioritySorter;

impl PrioritySorter {
    /// Sort agents by priority (descending). Higher priority first.
    pub fn sort_agents(agents: &mut [Agent]) {
        agents.sort_by_key(|b| std::cmp::Reverse(b.priority));
    }

    /// Filter agents that can preempt lower-priority workloads.
    /// Returns agents with priority >= threshold.
    pub fn preemptable_agents(agents: &[Agent], threshold: Priority) -> Vec<&Agent> {
        agents.iter().filter(|a| a.priority >= threshold).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;

    fn make_agent(id: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    #[test]
    fn test_sort_by_priority() {
        let mut agents = vec![
            make_agent("low", Priority::Low),
            make_agent("high", Priority::High),
            make_agent("med", Priority::Medium),
        ];

        PrioritySorter::sort_agents(&mut agents);
        assert_eq!(agents[0].id, "high");
        assert_eq!(agents[1].id, "med");
        assert_eq!(agents[2].id, "low");
    }

    #[test]
    fn test_filter_preemptable() {
        let agents = vec![
            make_agent("a1", Priority::Low),
            make_agent("a2", Priority::High),
            make_agent("a3", Priority::Medium),
        ];

        let preemptable = PrioritySorter::preemptable_agents(&agents, Priority::Medium);
        assert_eq!(preemptable.len(), 2);
        assert!(preemptable.iter().any(|a| a.id == "a2"));
        assert!(preemptable.iter().any(|a| a.id == "a3"));
    }
}
