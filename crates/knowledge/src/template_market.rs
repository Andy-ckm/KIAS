//! Workflow 模板市场
//!
//! 提供模板注册、搜索、分类功能，支持金融/政企/制造/客服/通用行业。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 模板类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateCategory {
    Finance,         // 金融
    Government,      // 政企
    Manufacturing,   // 制造
    CustomerService, // 客服
    General,         // 通用
}

impl std::fmt::Display for TemplateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateCategory::Finance => write!(f, "Finance"),
            TemplateCategory::Government => write!(f, "Government"),
            TemplateCategory::Manufacturing => write!(f, "Manufacturing"),
            TemplateCategory::CustomerService => write!(f, "CustomerService"),
            TemplateCategory::General => write!(f, "General"),
        }
    }
}

/// Workflow 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub entry_point: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<String>,
}

/// 模板定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub industry: TemplateCategory,
    pub template_type: String,
    pub workflow: WorkflowDefinition,
    pub tags: Vec<String>,
    pub version: String,
}

/// 模板注册表
pub struct TemplateRegistry {
    templates: HashMap<String, Template>,
    by_industry: HashMap<TemplateCategory, Vec<String>>,
    by_type: HashMap<String, Vec<String>>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            by_industry: HashMap::new(),
            by_type: HashMap::new(),
        }
    }

    /// 注册模板
    pub fn register(&mut self, template: Template) -> Result<(), TemplateError> {
        let id = template.id.clone();
        if self.templates.contains_key(&id) {
            return Err(TemplateError::AlreadyRegistered(id));
        }
        let industry = template.industry;
        let template_type = template.template_type.clone();
        self.templates.insert(id.clone(), template);
        self.by_industry
            .entry(industry)
            .or_default()
            .push(id.clone());
        self.by_type.entry(template_type).or_default().push(id);
        Ok(())
    }

    /// 获取模板
    pub fn get(&self, id: &str) -> Option<&Template> {
        self.templates.get(id)
    }

    /// 搜索模板
    pub fn search(&self, query: &str) -> Vec<&Template> {
        let query_lower = query.to_lowercase();
        self.templates
            .values()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower)
                    || t.description.to_lowercase().contains(&query_lower)
                    || t.tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// 按行业获取模板
    pub fn by_industry(&self, industry: TemplateCategory) -> Vec<&Template> {
        self.by_industry
            .get(&industry)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// 按类型获取模板
    pub fn by_type(&self, template_type: &str) -> Vec<&Template> {
        self.by_type
            .get(template_type)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// 列出所有模板
    pub fn list_all(&self) -> Vec<&Template> {
        self.templates.values().collect()
    }

    /// 获取模板数量
    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// 删除模板
    pub fn unregister(&mut self, id: &str) -> Result<(), TemplateError> {
        let template = self
            .templates
            .remove(id)
            .ok_or_else(|| TemplateError::NotFound(id.to_string()))?;
        // 从索引中移除
        if let Some(ids) = self.by_industry.get_mut(&template.industry) {
            ids.retain(|i| i != id);
        }
        if let Some(ids) = self.by_type.get_mut(&template.template_type) {
            ids.retain(|i| i != id);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("Template `{0}` not found")]
    NotFound(String),
    #[error("Template `{0}` already registered")]
    AlreadyRegistered(String),
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: "node1".to_string(),
                    name: "Start".to_string(),
                    node_type: "start".to_string(),
                    config: HashMap::new(),
                },
                WorkflowNode {
                    id: "node2".to_string(),
                    name: "Process".to_string(),
                    node_type: "llm".to_string(),
                    config: HashMap::new(),
                },
            ],
            edges: vec![WorkflowEdge {
                from: "node1".to_string(),
                to: "node2".to_string(),
                condition: None,
            }],
            entry_point: "node1".to_string(),
        }
    }

    fn create_test_template() -> Template {
        Template {
            id: "test-1".to_string(),
            name: "Test Template".to_string(),
            description: "A test workflow template".to_string(),
            industry: TemplateCategory::General,
            template_type: "chat".to_string(),
            workflow: create_test_workflow(),
            tags: vec!["test".to_string(), "demo".to_string()],
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn test_template_registry_register_and_get() {
        let mut registry = TemplateRegistry::new();
        let template = create_test_template();
        registry.register(template).unwrap();
        let found = registry.get("test-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test Template");
    }

    #[test]
    fn test_template_registry_duplicate_registration() {
        let mut registry = TemplateRegistry::new();
        let template = create_test_template();
        registry.register(template.clone()).unwrap();
        let result = registry.register(template);
        assert!(result.is_err());
    }

    #[test]
    fn test_template_registry_search() {
        let mut registry = TemplateRegistry::new();
        registry.register(create_test_template()).unwrap();
        let results = registry.search("test");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Test Template");
    }

    #[test]
    fn test_template_registry_by_industry() {
        let mut registry = TemplateRegistry::new();
        registry.register(create_test_template()).unwrap();
        let results = registry.by_industry(TemplateCategory::General);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_template_registry_list_all() {
        let mut registry = TemplateRegistry::new();
        registry.register(create_test_template()).unwrap();
        let all = registry.list_all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_template_registry_unregister() {
        let mut registry = TemplateRegistry::new();
        registry.register(create_test_template()).unwrap();
        registry.unregister("test-1").unwrap();
        assert!(registry.get("test-1").is_none());
    }

    #[test]
    fn test_template_category_display() {
        assert_eq!(TemplateCategory::Finance.to_string(), "Finance");
        assert_eq!(TemplateCategory::Government.to_string(), "Government");
        assert_eq!(TemplateCategory::Manufacturing.to_string(), "Manufacturing");
        assert_eq!(
            TemplateCategory::CustomerService.to_string(),
            "CustomerService"
        );
        assert_eq!(TemplateCategory::General.to_string(), "General");
    }
}
