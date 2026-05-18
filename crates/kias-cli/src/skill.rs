//! 技能管理模块

use serde::{Deserialize, Serialize};

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub parameters: Option<serde_json::Value>,
    /// Fine-grained permissions this skill requires.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Dependencies on other skills.
    #[serde(default)]
    pub dependencies: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_definition_parse() {
        let json = r#"{
            "name": "summarization",
            "description": "Text summarization skill",
            "version": "1.0.0",
            "tags": ["nlp", "text"],
            "parameters": {"max_length": 500}
        }"#;
        let skill: SkillDefinition = serde_json::from_str(json).expect("should parse");
        assert_eq!(skill.name, "summarization");
        assert_eq!(skill.version, "1.0.0");
        assert_eq!(skill.tags, vec!["nlp", "text"]);
        assert!(skill.parameters.is_some());
    }

    #[test]
    fn test_skill_definition_no_params() {
        let json = r#"{
            "name": "codegen",
            "description": "Code generation",
            "version": "0.1.0",
            "tags": [],
            "parameters": null
        }"#;
        let skill: SkillDefinition = serde_json::from_str(json).expect("should parse");
        assert!(skill.parameters.is_none());
        assert!(skill.tags.is_empty());
    }

    #[test]
    fn test_skill_clone_debug() {
        let json = r#"{
            "name": "test-skill",
            "description": "A test skill",
            "version": "2.0.0",
            "tags": ["test"],
            "parameters": null
        }"#;
        let skill: SkillDefinition = serde_json::from_str(json).unwrap();
        let cloned = skill.clone();
        assert_eq!(cloned.name, "test-skill");
        assert_eq!(cloned.version, "2.0.0");
        let _debug = format!("{:?}", cloned);
    }

    #[test]
    fn test_skill_serialization_roundtrip() {
        let json = r#"{
            "name": "roundtrip",
            "description": "test",
            "version": "1.0.0",
            "tags": ["a", "b"],
            "parameters": {"x": 1}
        }"#;
        let skill: SkillDefinition = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string(&skill).unwrap();
        let deserialized: SkillDefinition = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, "roundtrip");
        assert_eq!(deserialized.tags.len(), 2);
        assert!(deserialized.parameters.is_some());
    }

    #[test]
    fn test_skill_many_tags() {
        let json = r#"{
            "name": "tagged",
            "description": "many tags",
            "version": "1.0.0",
            "tags": ["t1", "t2", "t3", "t4", "t5"],
            "parameters": null
        }"#;
        let skill: SkillDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(skill.tags.len(), 5);
        assert_eq!(skill.tags[4], "t5");
    }
}
