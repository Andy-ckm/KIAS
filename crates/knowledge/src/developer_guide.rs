//! 扩展开发手册数据
//!
//! 提供快速入门/API参考/最佳实践/示例，支持 hello world 到生产模板。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 手册章节类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuideSectionType {
    QuickStart,
    ApiReference,
    BestPractices,
    Examples,
    Troubleshooting,
}

/// 手册章节
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideSection {
    pub id: String,
    pub title: String,
    pub section_type: GuideSectionType,
    pub content: String,
    pub order: u32,
    pub examples: Vec<String>,
    pub related_sections: Vec<String>,
}

impl GuideSection {
    pub fn new(id: &str, title: &str, section_type: GuideSectionType, order: u32) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            section_type,
            content: String::new(),
            order,
            examples: Vec::new(),
            related_sections: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.content = content.to_string();
        self
    }

    pub fn with_example(mut self, example: &str) -> Self {
        self.examples.push(example.to_string());
        self
    }
}

/// 示例项目定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub difficulty: Difficulty,
    pub category: String,
    pub files: Vec<ExampleFile>,
    pub prerequisites: Vec<String>,
    pub estimated_time_minutes: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Beginner => write!(f, "Beginner"),
            Difficulty::Intermediate => write!(f, "Intermediate"),
            Difficulty::Advanced => write!(f, "Advanced"),
            Difficulty::Expert => write!(f, "Expert"),
        }
    }
}

/// 示例文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleFile {
    pub path: String,
    pub content: String,
    pub language: String,
}

/// 开发手册
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperGuide {
    pub id: String,
    pub title: String,
    pub version: String,
    pub sections: Vec<GuideSection>,
    pub example_projects: Vec<ExampleProject>,
    pub metadata: HashMap<String, String>,
}

impl DeveloperGuide {
    pub fn new(id: &str, title: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            version: version.to_string(),
            sections: Vec::new(),
            example_projects: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_section(&mut self, section: GuideSection) {
        self.sections.push(section);
    }

    pub fn add_example_project(&mut self, project: ExampleProject) {
        self.example_projects.push(project);
    }

    /// 获取按类型分组的章节
    pub fn sections_by_type(&self, section_type: GuideSectionType) -> Vec<&GuideSection> {
        self.sections
            .iter()
            .filter(|s| s.section_type == section_type)
            .collect()
    }

    /// 获取按难度分组的示例
    pub fn examples_by_difficulty(&self, difficulty: Difficulty) -> Vec<&ExampleProject> {
        self.example_projects
            .iter()
            .filter(|p| p.difficulty == difficulty)
            .collect()
    }
}

/// 手册注册表
pub struct GuideRegistry {
    guides: HashMap<String, DeveloperGuide>,
    by_tag: HashMap<String, Vec<String>>,
}

impl Default for GuideRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GuideRegistry {
    pub fn new() -> Self {
        Self {
            guides: HashMap::new(),
            by_tag: HashMap::new(),
        }
    }

    /// 注册手册
    pub fn register(&mut self, guide: DeveloperGuide) -> Result<(), GuideError> {
        let id = guide.id.clone();
        if self.guides.contains_key(&id) {
            return Err(GuideError::AlreadyRegistered(id));
        }
        // 索引标签
        for project in &guide.example_projects {
            for tag in &project.tags {
                self.by_tag.entry(tag.clone()).or_default().push(id.clone());
            }
        }
        self.guides.insert(id, guide);
        Ok(())
    }

    /// 获取手册
    pub fn get(&self, id: &str) -> Option<&DeveloperGuide> {
        self.guides.get(id)
    }

    /// 搜索手册
    pub fn search(&self, query: &str) -> Vec<&DeveloperGuide> {
        let query_lower = query.to_lowercase();
        self.guides
            .values()
            .filter(|g| {
                g.title.to_lowercase().contains(&query_lower)
                    || g.sections
                        .iter()
                        .any(|s| s.title.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// 按标签查找
    pub fn find_by_tag(&self, tag: &str) -> Vec<&DeveloperGuide> {
        self.by_tag
            .get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.guides.get(id)).collect())
            .unwrap_or_default()
    }

    /// 列出所有手册
    pub fn list_all(&self) -> Vec<&DeveloperGuide> {
        self.guides.values().collect()
    }

    /// 创建默认开发手册
    pub fn create_default_guide(&mut self) -> Result<&DeveloperGuide, GuideError> {
        let mut guide = DeveloperGuide::new("kias-guide", "KIAS Developer Guide", "1.0.0");

        // 快速入门章节
        guide.add_section(
            GuideSection::new("qs-1", "Getting Started", GuideSectionType::QuickStart, 1)
                .with_content("Learn how to set up your first KIAS agent in 5 minutes.")
                .with_example("hello_world.rs"),
        );

        guide.add_section(
            GuideSection::new("qs-2", "First Workflow", GuideSectionType::QuickStart, 2)
                .with_content("Create your first workflow with visual editor.")
                .with_example("first_workflow.rs"),
        );

        // API 参考章节
        guide.add_section(
            GuideSection::new("api-1", "Agent API", GuideSectionType::ApiReference, 1)
                .with_content("Complete reference for Agent lifecycle management."),
        );

        guide.add_section(
            GuideSection::new("api-2", "Workflow API", GuideSectionType::ApiReference, 2)
                .with_content("Workflow definition and execution APIs."),
        );

        // 最佳实践章节
        guide.add_section(
            GuideSection::new("bp-1", "Error Handling", GuideSectionType::BestPractices, 1)
                .with_content("Best practices for error handling in KIAS agents."),
        );

        guide.add_section(
            GuideSection::new(
                "bp-2",
                "Performance Tuning",
                GuideSectionType::BestPractices,
                2,
            )
            .with_content("Optimization tips for production workloads."),
        );

        // 添加示例项目
        guide.add_example_project(ExampleProject {
            id: "hello-world".to_string(),
            name: "Hello World Agent".to_string(),
            description: "Your first KIAS agent".to_string(),
            difficulty: Difficulty::Beginner,
            category: "Fundamentals".to_string(),
            files: vec![ExampleFile {
                path: "src/main.rs".to_string(),
                content: "fn main() { println!(\"Hello from KIAS!\"); }".to_string(),
                language: "rust".to_string(),
            }],
            prerequisites: vec!["Rust 1.70+".to_string()],
            estimated_time_minutes: 5,
            tags: vec!["beginner".to_string(), "quick-start".to_string()],
        });

        guide.add_example_project(ExampleProject {
            id: "workflow-basic".to_string(),
            name: "Basic Workflow".to_string(),
            description: "Create a simple workflow with multiple steps".to_string(),
            difficulty: Difficulty::Beginner,
            category: "Workflows".to_string(),
            files: vec![
                ExampleFile {
                    path: "workflow.yaml".to_string(),
                    content: "nodes:\n  - id: start\n  - id: process\nedges:\n  - from: start\n    to: process".to_string(),
                    language: "yaml".to_string(),
                },
            ],
            prerequisites: vec!["Hello World completed".to_string()],
            estimated_time_minutes: 15,
            tags: vec!["beginner".to_string(), "workflow".to_string()],
        });

        let id = guide.id.clone();
        self.guides.insert(id.clone(), guide);
        self.by_tag.insert("beginner".to_string(), vec![id.clone()]);
        self.guides
            .get(&id)
            .ok_or_else(|| GuideError::NotFound(id))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GuideError {
    #[error("Guide `{0}` already registered")]
    AlreadyRegistered(String),
    #[error("Guide `{0}` not found")]
    NotFound(String),
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guide_section_creation() {
        let section = GuideSection::new("sec-1", "Test Section", GuideSectionType::QuickStart, 1)
            .with_content("Test content")
            .with_example("example.rs");
        assert_eq!(section.id, "sec-1");
        assert_eq!(section.title, "Test Section");
        assert_eq!(section.section_type, GuideSectionType::QuickStart);
        assert_eq!(section.order, 1);
        assert_eq!(section.examples.len(), 1);
    }

    #[test]
    fn test_developer_guide_sections_by_type() {
        let mut guide = DeveloperGuide::new("test", "Test Guide", "1.0.0");
        guide.add_section(GuideSection::new(
            "qs-1",
            "Quick Start",
            GuideSectionType::QuickStart,
            1,
        ));
        guide.add_section(GuideSection::new(
            "api-1",
            "API Ref",
            GuideSectionType::ApiReference,
            1,
        ));
        let qs_sections = guide.sections_by_type(GuideSectionType::QuickStart);
        assert_eq!(qs_sections.len(), 1);
    }

    #[test]
    fn test_developer_guide_examples_by_difficulty() {
        let mut guide = DeveloperGuide::new("test", "Test Guide", "1.0.0");
        guide.add_example_project(ExampleProject {
            id: "p1".to_string(),
            name: "Project 1".to_string(),
            description: "".to_string(),
            difficulty: Difficulty::Beginner,
            category: "".to_string(),
            files: vec![],
            prerequisites: vec![],
            estimated_time_minutes: 5,
            tags: vec![],
        });
        let beginner = guide.examples_by_difficulty(Difficulty::Beginner);
        assert_eq!(beginner.len(), 1);
    }

    #[test]
    fn test_guide_registry_register_and_get() {
        let mut registry = GuideRegistry::new();
        let guide = DeveloperGuide::new("test-guide", "Test Guide", "1.0.0");
        registry.register(guide).unwrap();
        assert!(registry.get("test-guide").is_some());
    }

    #[test]
    fn test_guide_registry_search() {
        let mut registry = GuideRegistry::new();
        let guide = DeveloperGuide::new("test-guide", "Test Guide", "1.0.0");
        registry.register(guide).unwrap();
        let results = registry.search("Test");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_guide_registry_find_by_tag() {
        let mut registry = GuideRegistry::new();
        registry.create_default_guide();
        let results = registry.find_by_tag("beginner");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_difficulty_display() {
        assert_eq!(Difficulty::Beginner.to_string(), "Beginner");
        assert_eq!(Difficulty::Intermediate.to_string(), "Intermediate");
        assert_eq!(Difficulty::Advanced.to_string(), "Advanced");
        assert_eq!(Difficulty::Expert.to_string(), "Expert");
    }

    #[test]
    fn test_example_project_serialization() {
        let project = ExampleProject {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test description".to_string(),
            difficulty: Difficulty::Intermediate,
            category: "Test".to_string(),
            files: vec![ExampleFile {
                path: "main.rs".to_string(),
                content: "fn main() {}".to_string(),
                language: "rust".to_string(),
            }],
            prerequisites: vec!["Rust".to_string()],
            estimated_time_minutes: 10,
            tags: vec!["test".to_string()],
        };
        let json = serde_json::to_string(&project).unwrap();
        let deserialized: ExampleProject = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test");
    }

    #[test]
    fn test_guide_section_type_serialization() {
        let st = GuideSectionType::QuickStart;
        let json = serde_json::to_string(&st).unwrap();
        let deserialized: GuideSectionType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, st);
    }

    #[test]
    fn test_guide_registry_list_all() {
        let mut registry = GuideRegistry::new();
        registry.create_default_guide();
        let all = registry.list_all();
        assert!(!all.is_empty());
    }
}
