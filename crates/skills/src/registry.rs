use super::skill::{Skill, SkillConfig};
use super::skill::{SkillDependency, SkillPermission};
use std::collections::HashMap;

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
        self.skills
            .values()
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

    /// Find skills by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| s.config().tags.iter().any(|t| t == tag))
            .map(|s| s.as_ref())
            .collect()
    }

    /// Find skills by multiple tags (any match)
    pub fn find_by_any_tag(&self, tags: &[&str]) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| {
                let skill_tags = &s.config().tags;
                tags.iter().any(|t| skill_tags.iter().any(|st| st == t))
            })
            .map(|s| s.as_ref())
            .collect()
    }

    /// Find skills by multiple tags (all match)
    pub fn find_by_all_tags(&self, tags: &[&str]) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| {
                let skill_tags = &s.config().tags;
                tags.iter().all(|t| skill_tags.iter().any(|st| st == t))
            })
            .map(|s| s.as_ref())
            .collect()
    }

    /// Find skills that require a specific permission.
    pub fn find_by_permission(&self, perm: &SkillPermission) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| s.config().requires_permission(perm))
            .map(|s| s.as_ref())
            .collect()
    }

    /// Find skills that require any of the given permissions.
    pub fn find_by_any_permission(&self, perms: &[SkillPermission]) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| {
                let skill_perms = &s.config().permissions;
                perms.iter().any(|p| skill_perms.contains(p))
            })
            .map(|s| s.as_ref())
            .collect()
    }

    /// List all unique permissions required by registered skills.
    pub fn all_permissions(&self) -> Vec<SkillPermission> {
        let mut perms: Vec<SkillPermission> = self
            .skills
            .values()
            .flat_map(|s| s.config().permissions)
            .collect();
        perms.sort_by_cached_key(|p| p.to_string());
        perms.dedup_by(|a, b| a == b);
        perms
    }

    /// Find skills that depend on the given skill name.
    pub fn find_dependents_of(&self, skill_name: &str) -> Vec<&dyn Skill> {
        self.skills
            .values()
            .filter(|s| s.config().dependencies.iter().any(|d| d.name == skill_name))
            .map(|s| s.as_ref())
            .collect()
    }

    /// Validate that all required dependencies of a skill are present in the registry.
    /// Returns `Ok(())` if all non-optional dependencies are satisfied,
    /// or `Err` with the list of missing required dependency names.
    pub fn validate_dependencies(&self, skill_name: &str) -> Result<(), Vec<String>> {
        let skill = match self.skills.get(skill_name) {
            Some(s) => s,
            None => return Err(vec![format!("Skill '{}' not found", skill_name)]),
        };

        let config = skill.config();
        let missing: Vec<String> = config
            .dependencies
            .iter()
            .filter(|d| !d.optional && !self.has(&d.name))
            .map(|d| d.name.clone())
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Get all declared dependencies (including optional) for a skill.
    pub fn get_dependencies(&self, skill_name: &str) -> Vec<SkillDependency> {
        self.skills
            .get(skill_name)
            .map(|s| s.config().dependencies)
            .unwrap_or_default()
    }

    /// List all unique tags
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.skills.values().flat_map(|s| s.config().tags).collect();
        tags.sort();
        tags.dedup();
        tags
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
            Self {
                name: name.to_string(),
                desc: desc.to_string(),
            }
        }
    }

    #[async_trait]
    impl Skill for MockSkill {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            &self.desc
        }
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

    #[test]
    fn test_find_by_tag() {
        let mut registry = SkillRegistry::new();
        let skill1 = MockSkill::new("http_call", "HTTP");
        // Can't set tags on MockSkill directly, but we can test the method exists
        registry.register(Box::new(skill1));

        // find_by_tag should return empty for non-existent tag
        let results = registry.find_by_tag("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_all_tags_empty() {
        let registry = SkillRegistry::new();
        let tags = registry.all_tags();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_find_by_any_tag_empty() {
        let registry = SkillRegistry::new();
        let results = registry.find_by_any_tag(&["tag1", "tag2"]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_by_all_tags_empty() {
        let registry = SkillRegistry::new();
        let results = registry.find_by_all_tags(&["tag1", "tag2"]);
        assert!(results.is_empty());
    }

    // ===== Permission-based registry tests =====

    #[test]
    fn test_find_by_permission_empty() {
        let registry = SkillRegistry::new();
        let results = registry.find_by_permission(&SkillPermission::Network);
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_by_any_permission_empty() {
        let registry = SkillRegistry::new();
        let results = registry.find_by_any_permission(&[SkillPermission::Network]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_all_permissions_empty() {
        let registry = SkillRegistry::new();
        assert!(registry.all_permissions().is_empty());
    }

    #[test]
    fn test_find_dependents_of_empty() {
        let registry = SkillRegistry::new();
        let results = registry.find_dependents_of("any");
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_dependencies_skill_not_found() {
        let registry = SkillRegistry::new();
        let result = registry.validate_dependencies("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("not found"));
    }

    #[test]
    fn test_get_dependencies_empty() {
        let registry = SkillRegistry::new();
        let deps = registry.get_dependencies("nonexistent");
        assert!(deps.is_empty());
    }
}
