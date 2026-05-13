use std::collections::HashMap;
use super::skill::{Skill, SkillConfig};

pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn register(&mut self, skill: Box<dyn Skill>) {
        tracing::info!(name = %skill.name(), "Registering skill");
        self.skills.insert(skill.name().to_string(), skill);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills.get(name).map(|s| s.as_ref())
    }

    pub fn list_skills(&self) -> Vec<&str> {
        self.skills.values().map(|s| s.name()).collect()
    }

    /// List all skill configurations
    pub fn list_configs(&self) -> Vec<SkillConfig> {
        self.skills.values().map(|s| s.config()).collect()
    }

    /// Search skills by name substring
    pub fn search_by_name(&self, query: &str) -> Vec<&dyn Skill> {
        let query_lower = query.to_lowercase();
        self.skills.values()
            .filter(|s| s.name().to_lowercase().contains(&query_lower))
            .map(|s| s.as_ref())
            .collect()
    }

    /// Get the number of registered skills
    pub fn count(&self) -> usize {
        self.skills.len()
    }

    /// Check if a skill is registered
    pub fn has(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    /// Unregister a skill
    pub fn unregister(&mut self, name: &str) -> bool {
        self.skills.remove(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use kias_common::KiasResult;

    struct MockSkill {
        name: String,
        desc: String,
    }

    impl MockSkill {
        fn new(name: &str, desc: &str) -> Self {
            Self { name: name.to_string(), desc: desc.to_string() }
        }
    }

    #[async_trait]
    impl Skill for MockSkill {
        fn name(&self) -> &str { &self.name }
        fn description(&self) -> &str { &self.desc }
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"status": "ok"}))
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = SkillRegistry::new();
        assert!(registry.get("any").is_none());
        assert_eq!(registry.list_skills().len(), 0);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("test-skill", "A test skill")));

        let skill = registry.get("test-skill");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name(), "test-skill");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = SkillRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list_skills() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("skill-a", "A")));
        registry.register(Box::new(MockSkill::new("skill-b", "B")));

        let names = registry.list_skills();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"skill-a"));
        assert!(names.contains(&"skill-b"));
    }

    #[tokio::test]
    async fn test_skill_execution() {
        let skill = MockSkill::new("mock", "test");
        let result = skill.execute(serde_json::json!({})).await.unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[test]
    fn test_registry_list_configs() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("s1", "Skill 1")));
        registry.register(Box::new(MockSkill::new("s2", "Skill 2")));

        let configs = registry.list_configs();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_registry_search() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("http_call", "HTTP")));
        registry.register(Box::new(MockSkill::new("shell_command", "Shell")));
        registry.register(Box::new(MockSkill::new("http_proxy", "Proxy")));

        let results = registry.search_by_name("http");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_registry_count() {
        let mut registry = SkillRegistry::new();
        assert_eq!(registry.count(), 0);
        registry.register(Box::new(MockSkill::new("s1", "A")));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_registry_has() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("s1", "A")));
        assert!(registry.has("s1"));
        assert!(!registry.has("s2"));
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(MockSkill::new("s1", "A")));
        assert!(registry.has("s1"));
        assert!(registry.unregister("s1"));
        assert!(!registry.has("s1"));
        assert!(!registry.unregister("s1"));
    }
}
