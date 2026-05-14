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
}
