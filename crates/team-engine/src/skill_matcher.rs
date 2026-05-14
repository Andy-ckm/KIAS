//! # Skill Matcher
//!
//! Intelligent agent selection based on capability matching.
//!
//! Given a set of required capabilities, the skill matcher ranks available
//! agents by how well they match. This enables the delegation protocol
//! to make informed decisions about which agent to delegate to.
//!
//! ## Scoring Algorithm
//!
//! ```text
//! score = capability_match * 0.6
//!       + availability * 0.2
//!       + (1.0 - load) * 0.15
//!       + historical_success * 0.05
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent profile for skill matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Agent ID
    pub agent_id: String,
    /// Agent name
    pub name: String,
    /// Capabilities the agent has (with proficiency 0.0 - 1.0)
    pub capabilities: HashMap<String, f32>,
    /// Whether the agent is currently available
    pub available: bool,
    /// Current load (0.0 = idle, 1.0 = fully loaded)
    pub load: f32,
    /// Historical success rate (0.0 - 1.0)
    pub success_rate: f32,
    /// Total tasks completed
    pub tasks_completed: u32,
    /// Specializations (high-level domains)
    pub specializations: Vec<String>,
}

impl AgentProfile {
    /// Create a new agent profile
    pub fn new(agent_id: &str, name: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            capabilities: HashMap::new(),
            available: true,
            load: 0.0,
            success_rate: 1.0,
            tasks_completed: 0,
            specializations: Vec::new(),
        }
    }

    /// Add a capability with proficiency level
    pub fn with_capability(mut self, name: &str, proficiency: f32) -> Self {
        self.capabilities
            .insert(name.to_string(), proficiency.clamp(0.0, 1.0));
        self
    }

    /// Add a specialization
    pub fn with_specialization(mut self, spec: &str) -> Self {
        self.specializations.push(spec.to_string());
        self
    }

    /// Set availability
    pub fn with_availability(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Set load
    pub fn with_load(mut self, load: f32) -> Self {
        self.load = load.clamp(0.0, 1.0);
        self
    }

    /// Set success rate
    pub fn with_success_rate(mut self, rate: f32) -> Self {
        self.success_rate = rate.clamp(0.0, 1.0);
        self
    }
}

/// Match result with scoring details
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Agent ID
    pub agent_id: String,
    /// Overall match score (0.0 - 1.0)
    pub score: f32,
    /// Capability match score
    pub capability_score: f32,
    /// Whether all required capabilities are present
    pub all_capabilities_met: bool,
    /// Missing capabilities
    pub missing_capabilities: Vec<String>,
    /// Agent's load
    pub agent_load: f32,
    /// Agent's success rate
    pub agent_success_rate: f32,
}

/// Skill matcher configuration
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// Weight for capability match (default: 0.6)
    pub capability_weight: f32,
    /// Weight for availability (default: 0.2)
    pub availability_weight: f32,
    /// Weight for low load (default: 0.15)
    pub load_weight: f32,
    /// Weight for historical success (default: 0.05)
    pub success_weight: f32,
    /// Minimum score threshold for a valid match
    pub min_score_threshold: f32,
    /// Whether to require ALL capabilities or allow partial matches
    pub require_all_capabilities: bool,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            capability_weight: 0.6,
            availability_weight: 0.2,
            load_weight: 0.15,
            success_weight: 0.05,
            min_score_threshold: 0.0,
            require_all_capabilities: false,
        }
    }
}

/// Skill matcher - finds the best agent for a given set of capabilities
pub struct SkillMatcher {
    config: MatcherConfig,
}

impl SkillMatcher {
    pub fn new(config: MatcherConfig) -> Self {
        Self { config }
    }

    /// Find and rank agents matching the required capabilities
    pub fn find_matches(
        &self,
        agents: &[AgentProfile],
        required_capabilities: &[String],
    ) -> Vec<MatchResult> {
        let mut results: Vec<MatchResult> = agents
            .iter()
            .map(|agent| self.score_agent(agent, required_capabilities))
            .filter(|r| r.score >= self.config.min_score_threshold)
            .filter(|r| !self.config.require_all_capabilities || r.all_capabilities_met)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Find the single best agent
    pub fn find_best(
        &self,
        agents: &[AgentProfile],
        required_capabilities: &[String],
    ) -> Option<MatchResult> {
        self.find_matches(agents, required_capabilities)
            .into_iter()
            .next()
    }

    /// Score a single agent against requirements
    fn score_agent(&self, agent: &AgentProfile, required_capabilities: &[String]) -> MatchResult {
        let mut matched_proficiencies = Vec::new();
        let mut missing = Vec::new();

        for cap in required_capabilities {
            if let Some(&proficiency) = agent.capabilities.get(cap) {
                matched_proficiencies.push(proficiency);
            } else {
                missing.push(cap.clone());
            }
        }

        // Capability score: average proficiency of matched capabilities,
        // weighted by how many were matched
        let capability_score = if required_capabilities.is_empty() {
            1.0
        } else if matched_proficiencies.is_empty() {
            0.0
        } else {
            let avg_proficiency: f32 =
                matched_proficiencies.iter().sum::<f32>() / matched_proficiencies.len() as f32;
            let coverage = matched_proficiencies.len() as f32 / required_capabilities.len() as f32;
            avg_proficiency * coverage
        };

        let availability_score = if agent.available { 1.0 } else { 0.0 };
        let load_score = 1.0 - agent.load;
        let success_score = agent.success_rate;

        let score = capability_score * self.config.capability_weight
            + availability_score * self.config.availability_weight
            + load_score * self.config.load_weight
            + success_score * self.config.success_weight;

        MatchResult {
            agent_id: agent.agent_id.clone(),
            score,
            capability_score,
            all_capabilities_met: missing.is_empty(),
            missing_capabilities: missing,
            agent_load: agent.load,
            agent_success_rate: agent.success_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agents() -> Vec<AgentProfile> {
        vec![
            AgentProfile::new("a1", "Code Expert")
                .with_capability("code_generation", 0.95)
                .with_capability("code_review", 0.9)
                .with_capability("testing", 0.7)
                .with_load(0.3)
                .with_success_rate(0.95),
            AgentProfile::new("a2", "Research Specialist")
                .with_capability("web_search", 0.9)
                .with_capability("document_analysis", 0.85)
                .with_capability("summarization", 0.8)
                .with_load(0.1)
                .with_success_rate(0.9),
            AgentProfile::new("a3", "Full Stack")
                .with_capability("code_generation", 0.7)
                .with_capability("web_search", 0.6)
                .with_capability("testing", 0.8)
                .with_load(0.8)
                .with_success_rate(0.85),
        ]
    }

    #[test]
    fn test_skill_matcher_finds_best_agent() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let agents = make_agents();

        let best = matcher
            .find_best(
                &agents,
                &["code_generation".to_string(), "code_review".to_string()],
            )
            .unwrap();

        assert_eq!(best.agent_id, "a1");
        assert!(best.all_capabilities_met);
    }

    #[test]
    fn test_skill_matcher_partial_match() {
        let matcher = SkillMatcher::new(MatcherConfig {
            require_all_capabilities: false,
            ..MatcherConfig::default()
        });
        let agents = make_agents();

        let results = matcher.find_matches(
            &agents,
            &["code_generation".to_string(), "web_search".to_string()],
        );

        assert!(!results.is_empty());
        // a3 should score well since it has both (even if lower proficiency)
        let a3 = results.iter().find(|r| r.agent_id == "a3").unwrap();
        assert!(a3.all_capabilities_met);
    }

    #[test]
    fn test_skill_matcher_require_all_capabilities() {
        let matcher = SkillMatcher::new(MatcherConfig {
            require_all_capabilities: true,
            ..MatcherConfig::default()
        });
        let agents = make_agents();

        let results = matcher.find_matches(
            &agents,
            &["code_generation".to_string(), "nonexistent".to_string()],
        );

        // No agent should match all capabilities
        assert!(results.is_empty());
    }

    #[test]
    fn test_skill_matcher_availability_filter() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let agents = vec![
            AgentProfile::new("a1", "Available")
                .with_capability("task", 1.0)
                .with_availability(true),
            AgentProfile::new("a2", "Unavailable")
                .with_capability("task", 1.0)
                .with_availability(false),
        ];

        let results = matcher.find_matches(&agents, &["task".to_string()]);
        assert_eq!(results.len(), 2);

        // Available agent should score higher
        assert!(results[0].score > results[1].score);
        assert_eq!(results[0].agent_id, "a1");
    }

    #[test]
    fn test_skill_matcher_load_affects_ranking() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let agents = vec![
            AgentProfile::new("a1", "Busy")
                .with_capability("task", 0.9)
                .with_load(0.9),
            AgentProfile::new("a2", "Idle")
                .with_capability("task", 0.9)
                .with_load(0.0),
        ];

        let results = matcher.find_matches(&agents, &["task".to_string()]);
        assert_eq!(results.len(), 2);
        // Idle agent should rank higher
        assert_eq!(results[0].agent_id, "a2");
    }

    #[test]
    fn test_skill_matcher_empty_requirements() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let agents = make_agents();

        let results = matcher.find_matches(&agents, &[]);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_skill_matcher_min_score_threshold() {
        let matcher = SkillMatcher::new(MatcherConfig {
            min_score_threshold: 0.9,
            ..MatcherConfig::default()
        });
        let agents = make_agents();

        let results = matcher.find_matches(&agents, &["code_generation".to_string()]);
        // Very high threshold should filter most agents
        for result in &results {
            assert!(result.score >= 0.9);
        }
    }

    #[test]
    fn test_skill_matcher_no_agents() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let results = matcher.find_matches(&[], &["task".to_string()]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_agent_profile_builder() {
        let agent = AgentProfile::new("a1", "Test")
            .with_capability("cap1", 0.9)
            .with_capability("cap2", 0.5)
            .with_specialization("testing")
            .with_availability(true)
            .with_load(0.3)
            .with_success_rate(0.95);

        assert_eq!(agent.capabilities.len(), 2);
        assert!(agent.available);
        assert!((agent.load - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_match_result_details() {
        let matcher = SkillMatcher::new(MatcherConfig::default());
        let agents = make_agents();

        let result = matcher
            .find_best(&agents, &["code_generation".to_string()])
            .unwrap();

        assert!(result.capability_score > 0.0);
        assert!(result.agent_success_rate > 0.0);
    }

    #[test]
    fn test_custom_config_weights() {
        let config = MatcherConfig {
            capability_weight: 0.0,
            availability_weight: 0.0,
            load_weight: 1.0,
            success_weight: 0.0,
            min_score_threshold: 0.0,
            require_all_capabilities: false,
        };
        let matcher = SkillMatcher::new(config);
        let agents = make_agents();

        let best = matcher
            .find_best(&agents, &["anything".to_string()])
            .unwrap();
        // With load_weight=1.0, least loaded agent wins
        assert_eq!(best.agent_id, "a2"); // load = 0.1
    }
}
