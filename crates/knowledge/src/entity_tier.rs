//! Entity tier management — Tier 1/2/3 layering per GBrain design.
//!
//! - **Tier 3** — stub page (first mention, only name + source)
//! - **Tier 2** — basic enrichment (≥ 3 distinct sources)
//! - **Tier 1** — full enrichment (≥ 8 distinct sources **or** attended a meeting)

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Entity tier level — mirrors GBrain Tier 1/2/3 concept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityTier {
    /// Stub page — first mention, only name and source.
    Tier3,
    /// Basic enrichment — appeared across 3+ distinct sources.
    Tier2,
    /// Full enrichment — appeared across 8+ sources or attended a meeting.
    Tier1,
}

/// Manages entity tier promotions based on source diversity and meeting participation.
pub struct EntityTierManager {
    /// entity_id → set of distinct sources.
    source_counts: HashMap<String, HashSet<String>>,
    /// Entities that have participated in a meeting (instant Tier 1).
    meeting_participants: HashSet<String>,
}

impl EntityTierManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            source_counts: HashMap::new(),
            meeting_participants: HashSet::new(),
        }
    }

    /// Register an entity mention from a given source.
    ///
    /// Duplicate mentions from the same source are deduplicated.
    /// Returns the entity's **current** tier after registration.
    pub fn register_mention(&mut self, entity_id: &str, source: &str) -> EntityTier {
        self.source_counts
            .entry(entity_id.to_string())
            .or_default()
            .insert(source.to_string());
        self.calculate_tier(entity_id)
    }

    /// Register an entity as a meeting participant.
    ///
    /// Meeting participation promotes the entity directly to Tier 1.
    /// Returns the entity's current tier.
    pub fn register_meeting(&mut self, entity_id: &str) -> EntityTier {
        self.meeting_participants.insert(entity_id.to_string());
        // Ensure the entity exists in source_counts as well.
        self.source_counts
            .entry(entity_id.to_string())
            .or_default();
        self.calculate_tier(entity_id)
    }

    /// Calculate the tier for a given entity based on current state.
    pub fn calculate_tier(&self, entity_id: &str) -> EntityTier {
        let source_count = self
            .source_counts
            .get(entity_id)
            .map(|s| s.len())
            .unwrap_or(0);
        let in_meeting = self.meeting_participants.contains(entity_id);

        if source_count >= 8 || in_meeting {
            EntityTier::Tier1
        } else if source_count >= 3 {
            EntityTier::Tier2
        } else {
            EntityTier::Tier3
        }
    }

    /// Return entities that *just crossed* a tier boundary:
    /// - exactly 3 sources → Tier3 → Tier2
    /// - exactly 8 sources → Tier2 → Tier1
    ///
    /// Meeting participants are excluded (they jump straight to Tier1).
    pub fn get_upgradable_entities(&self) -> Vec<(String, EntityTier, EntityTier)> {
        let mut result = Vec::new();
        for (entity_id, sources) in &self.source_counts {
            if self.meeting_participants.contains(entity_id) {
                continue;
            }
            match sources.len() {
                3 => result.push((
                    entity_id.clone(),
                    EntityTier::Tier3,
                    EntityTier::Tier2,
                )),
                8 => result.push((
                    entity_id.clone(),
                    EntityTier::Tier2,
                    EntityTier::Tier1,
                )),
                _ => {}
            }
        }
        result
    }

    /// Number of distinct sources for a given entity.
    pub fn source_count(&self, entity_id: &str) -> usize {
        self.source_counts
            .get(entity_id)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// List all tracked entity IDs.
    pub fn all_entities(&self) -> Vec<String> {
        self.source_counts.keys().cloned().collect()
    }
}

impl Default for EntityTierManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entity_defaults_to_tier3() {
        let mut mgr = EntityTierManager::new();
        let tier = mgr.register_mention("alice", "doc1");
        assert_eq!(tier, EntityTier::Tier3);
        assert_eq!(mgr.calculate_tier("alice"), EntityTier::Tier3);
    }

    #[test]
    fn three_sources_upgrade_to_tier2() {
        let mut mgr = EntityTierManager::new();
        mgr.register_mention("bob", "src1");
        mgr.register_mention("bob", "src2");
        let tier = mgr.register_mention("bob", "src3");
        assert_eq!(tier, EntityTier::Tier2);
    }

    #[test]
    fn eight_sources_upgrade_to_tier1() {
        let mut mgr = EntityTierManager::new();
        for i in 1..=8 {
            let src = format!("src{i}");
            mgr.register_mention("carol", &src);
        }
        assert_eq!(mgr.calculate_tier("carol"), EntityTier::Tier1);
    }

    #[test]
    fn meeting_participation_direct_tier1() {
        let mut mgr = EntityTierManager::new();
        // Only one source → would normally be Tier3
        mgr.register_mention("dave", "doc1");
        assert_eq!(mgr.calculate_tier("dave"), EntityTier::Tier3);

        let tier = mgr.register_meeting("dave");
        assert_eq!(tier, EntityTier::Tier1);
    }

    #[test]
    fn duplicate_source_dedup() {
        let mut mgr = EntityTierManager::new();
        mgr.register_mention("eve", "src1");
        mgr.register_mention("eve", "src1"); // duplicate
        mgr.register_mention("eve", "src1"); // duplicate
        assert_eq!(mgr.source_count("eve"), 1);
        assert_eq!(mgr.calculate_tier("eve"), EntityTier::Tier3);
    }

    #[test]
    fn get_upgradable_entities_correct() {
        let mut mgr = EntityTierManager::new();
        // Tier3 entity (2 sources) — not upgradable
        mgr.register_mention("a", "s1");
        mgr.register_mention("a", "s2");
        // Exactly 3 sources → Tier3→Tier2 upgradable
        mgr.register_mention("b", "s1");
        mgr.register_mention("b", "s2");
        mgr.register_mention("b", "s3");
        // 5 sources → already Tier2, not at boundary
        for i in 1..=5 {
            mgr.register_mention("c", &format!("s{i}"));
        }

        let upgradable = mgr.get_upgradable_entities();
        assert_eq!(upgradable.len(), 1);
        assert_eq!(upgradable[0].0, "b");
        assert_eq!(upgradable[0].1, EntityTier::Tier3);
        assert_eq!(upgradable[0].2, EntityTier::Tier2);
    }

    #[test]
    fn source_count_returns_correct_value() {
        let mut mgr = EntityTierManager::new();
        assert_eq!(mgr.source_count("unknown"), 0);

        mgr.register_mention("x", "s1");
        mgr.register_mention("x", "s2");
        assert_eq!(mgr.source_count("x"), 2);

        mgr.register_mention("x", "s1"); // dup
        assert_eq!(mgr.source_count("x"), 2);
    }

    #[test]
    fn all_entities_returns_all() {
        let mut mgr = EntityTierManager::new();
        mgr.register_mention("alpha", "s1");
        mgr.register_mention("beta", "s1");
        mgr.register_mention("gamma", "s1");

        let mut entities = mgr.all_entities();
        entities.sort();
        assert_eq!(entities, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn multiple_entities_independent_tiers() {
        let mut mgr = EntityTierManager::new();

        // Entity "low" → Tier3 (1 source)
        mgr.register_mention("low", "s1");
        // Entity "mid" → Tier2 (3 sources)
        for i in 1..=3 {
            mgr.register_mention("mid", &format!("s{i}"));
        }
        // Entity "high" → Tier1 via meeting
        mgr.register_mention("high", "s1");
        mgr.register_meeting("high");

        assert_eq!(mgr.calculate_tier("low"), EntityTier::Tier3);
        assert_eq!(mgr.calculate_tier("mid"), EntityTier::Tier2);
        assert_eq!(mgr.calculate_tier("high"), EntityTier::Tier1);
    }
}
