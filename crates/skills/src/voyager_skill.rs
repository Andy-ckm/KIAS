//! Voyager-style Continuous Learning Skill Library
//!
//! Based on the Voyager paper (NVIDIA, 2023):
//! - Successful tasks are automatically extracted as reusable skills
//! - Skills are stored in a growing library
//! - When facing new tasks, relevant skills are retrieved
//! - Skills can be composed from simpler skills
//!
//! ## Key Concepts
//! 1. **Skill Extraction**: Analyze successful task executions to create skills
//! 2. **Skill Storage**: Persistent storage with metadata and versioning
//! 3. **Skill Retrieval**: Find relevant skills based on task similarity
//! 4. **Skill Composition**: Build complex skills from simpler ones

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A skill extracted from successful task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSkill {
    /// Unique skill identifier
    pub id: String,
    /// Human-readable skill name
    pub name: String,
    /// Description of what the skill does
    pub description: String,
    /// Category (e.g., "coding", "data-analysis", "system-admin")
    pub category: String,
    /// Tags for searchability
    pub tags: Vec<String>,
    /// The code/commands that implement this skill
    pub implementation: SkillImplementation,
    /// Parameters the skill accepts
    pub parameters: Vec<SkillParameter>,
    /// When this skill was extracted
    pub created_at: DateTime<Utc>,
    /// How many times this skill has been successfully used
    pub success_count: u64,
    /// How many times this skill has failed
    pub failure_count: u64,
    /// Average execution time in milliseconds
    pub avg_execution_ms: f64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Source task that generated this skill
    pub source_task_id: Option<String>,
    /// Version for skill evolution
    pub version: u32,
    /// Parent skills this was composed from
    pub composed_from: Vec<String>,
}

/// Implementation details of a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillImplementation {
    /// Shell command(s) to execute
    Shell(Vec<String>),
    /// Rust code snippet
    RustCode(String),
    /// Python code snippet
    PythonCode(String),
    /// API call template
    ApiCall {
        method: String,
        url_template: String,
        headers: HashMap<String, String>,
        body_template: Option<String>,
    },
    /// Composite skill - sequence of sub-skills
    Composite(Vec<SubSkillCall>),
    /// LLM prompt template
    PromptTemplate(String),
}

/// A call to a sub-skill in a composite skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubSkillCall {
    /// Skill to call
    pub skill_id: String,
    /// Parameters to pass (can reference parent params)
    pub params: HashMap<String, serde_json::Value>,
    /// Whether to continue on failure
    pub continue_on_failure: bool,
}

/// Parameter definition for a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type (string, number, boolean, etc.)
    pub param_type: String,
    /// Whether this parameter is required
    pub required: bool,
    /// Default value if not provided
    pub default: Option<serde_json::Value>,
    /// Description of the parameter
    pub description: String,
}

/// Result of executing a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Output from the execution
    pub output: String,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_ms: u64,
    /// Metadata about the execution
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Task execution record for skill extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    /// Task identifier
    pub task_id: String,
    /// What the task was trying to accomplish
    pub goal: String,
    /// The commands/code that were executed
    pub actions: Vec<TaskAction>,
    /// Whether the task succeeded
    pub success: bool,
    /// Final output
    pub output: String,
    /// Execution time in milliseconds
    pub execution_ms: u64,
    /// When the task was executed
    pub executed_at: DateTime<Utc>,
}

/// A single action within a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAction {
    /// Type of action (shell, code, api, etc.)
    pub action_type: String,
    /// The actual command/code
    pub content: String,
    /// Output from this action
    pub output: String,
    /// Whether this action succeeded
    pub success: bool,
}

/// Voyager-style skill library
pub struct SkillLibrary {
    /// All extracted skills
    skills: HashMap<String, ExtractedSkill>,
    /// Index by category
    category_index: HashMap<String, Vec<String>>,
    /// Index by tag
    tag_index: HashMap<String, Vec<String>>,
    /// Task execution history for skill extraction
    execution_history: Vec<TaskExecution>,
    /// Minimum success rate to keep a skill
    min_success_rate: f64,
    /// Minimum confidence to use a skill
    min_confidence: f64,
}

impl SkillLibrary {
    /// Create a new empty skill library
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            category_index: HashMap::new(),
            tag_index: HashMap::new(),
            execution_history: Vec::new(),
            min_success_rate: 0.7,
            min_confidence: 0.5,
        }
    }

    /// Create a skill library with custom thresholds
    pub fn with_thresholds(min_success_rate: f64, min_confidence: f64) -> Self {
        Self {
            min_success_rate,
            min_confidence,
            ..Self::new()
        }
    }

    /// Add a skill to the library
    pub fn add_skill(&mut self, skill: ExtractedSkill) {
        // Update category index
        self.category_index
            .entry(skill.category.clone())
            .or_insert_with(Vec::new)
            .push(skill.id.clone());

        // Update tag index
        for tag in &skill.tags {
            self.tag_index
                .entry(tag.clone())
                .or_insert_with(Vec::new)
                .push(skill.id.clone());
        }

        self.skills.insert(skill.id.clone(), skill);
    }

    /// Get a skill by ID
    pub fn get_skill(&self, id: &str) -> Option<&ExtractedSkill> {
        self.skills.get(id)
    }

    /// Get all skills in a category
    pub fn get_by_category(&self, category: &str) -> Vec<&ExtractedSkill> {
        self.category_index
            .get(category)
            .map(|ids| ids.iter().filter_map(|id| self.skills.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all skills with a specific tag
    pub fn get_by_tag(&self, tag: &str) -> Vec<&ExtractedSkill> {
        self.tag_index
            .get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.skills.get(id)).collect())
            .unwrap_or_default()
    }

    /// Search skills by name or description
    pub fn search(&self, query: &str) -> Vec<&ExtractedSkill> {
        let query_lower = query.to_lowercase();
        self.skills
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
                    || s.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Record a task execution
    pub fn record_execution(&mut self, execution: TaskExecution) {
        self.execution_history.push(execution);
    }

    /// Extract skills from successful task executions
    pub fn extract_skills(&mut self) -> Vec<ExtractedSkill> {
        let mut extracted = Vec::new();

        // Collect successful executions first to avoid borrow conflict
        let successful: Vec<TaskExecution> = self
            .execution_history
            .drain(..)
            .filter(|e| e.success)
            .collect();

        for execution in successful {
            // Try to extract a skill from this execution
            if let Some(skill) = self.extract_from_execution(&execution) {
                extracted.push(skill.clone());
                self.add_skill(skill);
            }
        }

        extracted
    }

    /// Extract a skill from a single execution
    fn extract_from_execution(&self, execution: &TaskExecution) -> Option<ExtractedSkill> {
        // Only extract from successful executions with actions
        if !execution.success || execution.actions.is_empty() {
            return None;
        }

        // Check if all actions succeeded
        if !execution.actions.iter().all(|a| a.success) {
            return None;
        }

        // Determine implementation type
        let implementation = self.detect_implementation(&execution.actions);

        // Generate skill name from goal
        let name = self.generate_skill_name(&execution.goal);

        // Extract tags from goal and actions
        let tags = self.extract_tags(&execution.goal, &execution.actions);

        Some(ExtractedSkill {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: execution.goal.clone(),
            category: self.detect_category(&execution.goal),
            tags,
            implementation,
            parameters: self.detect_parameters(&execution.actions),
            created_at: Utc::now(),
            success_count: 1,
            failure_count: 0,
            avg_execution_ms: execution.execution_ms as f64,
            confidence: 0.8, // Initial confidence
            source_task_id: Some(execution.task_id.clone()),
            version: 1,
            composed_from: Vec::new(),
        })
    }

    /// Detect implementation type from actions
    fn detect_implementation(&self, actions: &[TaskAction]) -> SkillImplementation {
        // Check if all actions are shell commands
        if actions.iter().all(|a| a.action_type == "shell") {
            let commands: Vec<String> = actions.iter().map(|a| a.content.clone()).collect();
            return SkillImplementation::Shell(commands);
        }

        // Check if all actions are Python code
        if actions.iter().all(|a| a.action_type == "python") {
            let code: Vec<String> = actions.iter().map(|a| a.content.clone()).collect();
            return SkillImplementation::PythonCode(code.join("\n"));
        }

        // Check if all actions are Rust code
        if actions.iter().all(|a| a.action_type == "rust") {
            let code: Vec<String> = actions.iter().map(|a| a.content.clone()).collect();
            return SkillImplementation::RustCode(code.join("\n"));
        }

        // Default to shell
        let commands: Vec<String> = actions.iter().map(|a| a.content.clone()).collect();
        SkillImplementation::Shell(commands)
    }

    /// Generate a skill name from a goal
    fn generate_skill_name(&self, goal: &str) -> String {
        // Take first 50 chars and clean up
        let name = goal
            .chars()
            .take(50)
            .collect::<String>()
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
            .replace(' ', "-")
            .trim_matches('-')
            .to_string();

        format!("skill-{}", name)
    }

    /// Extract tags from goal and actions
    fn extract_tags(&self, goal: &str, actions: &[TaskAction]) -> Vec<String> {
        let mut tags = Vec::new();

        // Add action types as tags
        for action in actions {
            if !tags.contains(&action.action_type) {
                tags.push(action.action_type.clone());
            }
        }

        // Extract keywords from goal
        let keywords: Vec<&str> = goal.split_whitespace().filter(|w| w.len() > 3).collect();

        for keyword in keywords {
            let tag = keyword
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), "");
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }

        tags
    }

    /// Detect category from goal
    fn detect_category(&self, goal: &str) -> String {
        let goal_lower = goal.to_lowercase();

        if goal_lower.contains("code")
            || goal_lower.contains("function")
            || goal_lower.contains("implement")
        {
            "coding".to_string()
        } else if goal_lower.contains("test") || goal_lower.contains("verify") {
            "testing".to_string()
        } else if goal_lower.contains("deploy") || goal_lower.contains("build") {
            "devops".to_string()
        } else if goal_lower.contains("data") || goal_lower.contains("analyze") {
            "data-analysis".to_string()
        } else if goal_lower.contains("file")
            || goal_lower.contains("read")
            || goal_lower.contains("write")
        {
            "file-operations".to_string()
        } else {
            "general".to_string()
        }
    }

    /// Detect parameters from actions
    fn detect_parameters(&self, actions: &[TaskAction]) -> Vec<SkillParameter> {
        let mut params = Vec::new();

        // Look for patterns like ${param} or {param} in actions
        for action in actions {
            let content = &action.content;
            let mut start = 0;

            while let Some(pos) = content[start..].find('{') {
                let end = content[start + pos..].find('}');
                if let Some(end) = end {
                    let param_name = &content[start + pos + 1..start + pos + end];
                    if !param_name.is_empty() && !param_name.contains('{') {
                        let param = SkillParameter {
                            name: param_name.to_string(),
                            param_type: "string".to_string(),
                            required: true,
                            default: None,
                            description: format!("Parameter extracted from action: {}", param_name),
                        };

                        if !params.iter().any(|p: &SkillParameter| p.name == param_name) {
                            params.push(param);
                        }
                    }
                    start = start + pos + end + 1;
                } else {
                    break;
                }
            }
        }

        params
    }

    /// Update skill success/failure counts
    pub fn record_skill_usage(&mut self, skill_id: &str, success: bool, execution_ms: u64) {
        if let Some(skill) = self.skills.get_mut(skill_id) {
            if success {
                skill.success_count += 1;
            } else {
                skill.failure_count += 1;
            }

            // Update average execution time
            let total_count = skill.success_count + skill.failure_count;
            skill.avg_execution_ms = (skill.avg_execution_ms * (total_count - 1) as f64
                + execution_ms as f64)
                / total_count as f64;

            // Update confidence based on success rate
            let success_rate = skill.success_count as f64 / total_count as f64;
            skill.confidence = success_rate * 0.9 + 0.1; // Min confidence 0.1
        }
    }

    /// Get skills that are reliable enough to use
    pub fn get_reliable_skills(&self) -> Vec<&ExtractedSkill> {
        self.skills
            .values()
            .filter(|s| {
                let total = s.success_count + s.failure_count;
                if total == 0 {
                    return true; // New skills are considered reliable
                }
                let success_rate = s.success_count as f64 / total as f64;
                success_rate >= self.min_success_rate && s.confidence >= self.min_confidence
            })
            .collect()
    }

    /// Compose a new skill from existing skills
    pub fn compose_skill(
        &mut self,
        name: String,
        description: String,
        sub_skills: Vec<SubSkillCall>,
    ) -> ExtractedSkill {
        let skill = ExtractedSkill {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            category: "composite".to_string(),
            tags: vec!["composite".to_string()],
            implementation: SkillImplementation::Composite(sub_skills.clone()),
            parameters: Vec::new(),
            created_at: Utc::now(),
            success_count: 0,
            failure_count: 0,
            avg_execution_ms: 0.0,
            confidence: 0.7,
            source_task_id: None,
            version: 1,
            composed_from: sub_skills.iter().map(|s| s.skill_id.clone()).collect(),
        };

        self.add_skill(skill.clone());
        skill
    }

    /// Prune low-performing skills
    pub fn prune_skills(&mut self) -> Vec<String> {
        let mut pruned = Vec::new();

        let skill_ids: Vec<String> = self.skills.keys().cloned().collect();
        for id in skill_ids {
            if let Some(skill) = self.skills.get(&id) {
                let total = skill.success_count + skill.failure_count;
                if total >= 5 {
                    let success_rate = skill.success_count as f64 / total as f64;
                    if success_rate < self.min_success_rate {
                        pruned.push(id.clone());
                        self.remove_skill(&id);
                    }
                }
            }
        }

        pruned
    }

    /// Remove a skill from the library
    fn remove_skill(&mut self, id: &str) {
        if let Some(skill) = self.skills.remove(id) {
            // Remove from category index
            if let Some(ids) = self.category_index.get_mut(&skill.category) {
                ids.retain(|i| i != id);
            }

            // Remove from tag index
            for tag in &skill.tags {
                if let Some(ids) = self.tag_index.get_mut(tag) {
                    ids.retain(|i| i != id);
                }
            }
        }
    }

    /// Get library statistics
    pub fn stats(&self) -> SkillLibraryStats {
        let total_skills = self.skills.len();
        let reliable_skills = self.get_reliable_skills().len();
        let categories = self.category_index.len();
        let tags = self.tag_index.len();

        let total_usage: u64 = self
            .skills
            .values()
            .map(|s| s.success_count + s.failure_count)
            .sum();
        let avg_confidence = if total_skills > 0 {
            self.skills.values().map(|s| s.confidence).sum::<f64>() / total_skills as f64
        } else {
            0.0
        };

        SkillLibraryStats {
            total_skills,
            reliable_skills,
            categories,
            tags,
            total_usage,
            avg_confidence,
        }
    }

    /// Export skills to JSON
    pub fn export_json(&self) -> String {
        let skills: Vec<&ExtractedSkill> = self.skills.values().collect();
        serde_json::to_string_pretty(&skills).unwrap_or_default()
    }

    /// Import skills from JSON
    pub fn import_json(&mut self, json: &str) -> Result<usize, String> {
        let skills: Vec<ExtractedSkill> =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let count = skills.len();
        for skill in skills {
            self.add_skill(skill);
        }

        Ok(count)
    }
}

/// Statistics about the skill library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLibraryStats {
    pub total_skills: usize,
    pub reliable_skills: usize,
    pub categories: usize,
    pub tags: usize,
    pub total_usage: u64,
    pub avg_confidence: f64,
}

impl Default for SkillLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_skill(name: &str) -> ExtractedSkill {
        ExtractedSkill {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: format!("Test skill: {}", name),
            category: "testing".to_string(),
            tags: vec!["test".to_string()],
            implementation: SkillImplementation::Shell(vec!["echo hello".to_string()]),
            parameters: Vec::new(),
            created_at: Utc::now(),
            success_count: 0,
            failure_count: 0,
            avg_execution_ms: 100.0,
            confidence: 0.8,
            source_task_id: None,
            version: 1,
            composed_from: Vec::new(),
        }
    }

    fn create_test_execution(goal: &str, success: bool) -> TaskExecution {
        TaskExecution {
            task_id: uuid::Uuid::new_v4().to_string(),
            goal: goal.to_string(),
            actions: vec![TaskAction {
                action_type: "shell".to_string(),
                content: "echo hello".to_string(),
                output: "hello".to_string(),
                success: true,
            }],
            success,
            output: "done".to_string(),
            execution_ms: 100,
            executed_at: Utc::now(),
        }
    }

    #[test]
    fn test_library_creation() {
        let library = SkillLibrary::new();
        assert_eq!(library.stats().total_skills, 0);
    }

    #[test]
    fn test_add_and_get_skill() {
        let mut library = SkillLibrary::new();
        let skill = create_test_skill("test-skill");
        let id = skill.id.clone();

        library.add_skill(skill);
        assert!(library.get_skill(&id).is_some());
        assert_eq!(library.get_skill(&id).unwrap().name, "test-skill");
    }

    #[test]
    fn test_get_by_category() {
        let mut library = SkillLibrary::new();
        library.add_skill(create_test_skill("skill-1"));
        library.add_skill(create_test_skill("skill-2"));

        let skills = library.get_by_category("testing");
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn test_get_by_tag() {
        let mut library = SkillLibrary::new();
        library.add_skill(create_test_skill("skill-1"));

        let skills = library.get_by_tag("test");
        assert_eq!(skills.len(), 1);
    }

    #[test]
    fn test_search() {
        let mut library = SkillLibrary::new();
        library.add_skill(create_test_skill("http-client"));
        library.add_skill(create_test_skill("database-query"));

        let results = library.search("http");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "http-client");
    }

    #[test]
    fn test_extract_skills_from_execution() {
        let mut library = SkillLibrary::new();
        let execution = create_test_execution("Create a hello world script", true);

        library.record_execution(execution);
        let extracted = library.extract_skills();

        assert_eq!(extracted.len(), 1);
        assert!(extracted[0].name.contains("hello"));
    }

    #[test]
    fn test_no_extraction_from_failed_execution() {
        let mut library = SkillLibrary::new();
        let execution = create_test_execution("Failed task", false);

        library.record_execution(execution);
        let extracted = library.extract_skills();

        assert_eq!(extracted.len(), 0);
    }

    #[test]
    fn test_skill_usage_tracking() {
        let mut library = SkillLibrary::new();
        let skill = create_test_skill("test-skill");
        let id = skill.id.clone();

        library.add_skill(skill);

        // Record successful usage
        library.record_skill_usage(&id, true, 100);
        let skill = library.get_skill(&id).unwrap();
        assert_eq!(skill.success_count, 1);
        assert_eq!(skill.failure_count, 0);

        // Record failed usage
        library.record_skill_usage(&id, false, 200);
        let skill = library.get_skill(&id).unwrap();
        assert_eq!(skill.success_count, 1);
        assert_eq!(skill.failure_count, 1);
    }

    #[test]
    fn test_reliable_skills_filtering() {
        let mut library = SkillLibrary::with_thresholds(0.7, 0.5);

        // Add a reliable skill
        let mut skill1 = create_test_skill("reliable");
        skill1.success_count = 10;
        skill1.failure_count = 2;
        skill1.confidence = 0.8;
        library.add_skill(skill1);

        // Add an unreliable skill
        let mut skill2 = create_test_skill("unreliable");
        skill2.success_count = 2;
        skill2.failure_count = 8;
        skill2.confidence = 0.3;
        library.add_skill(skill2);

        let reliable = library.get_reliable_skills();
        assert_eq!(reliable.len(), 1);
        assert_eq!(reliable[0].name, "reliable");
    }

    #[test]
    fn test_compose_skill() {
        let mut library = SkillLibrary::new();

        let sub_skills = vec![
            SubSkillCall {
                skill_id: "skill-1".to_string(),
                params: HashMap::new(),
                continue_on_failure: false,
            },
            SubSkillCall {
                skill_id: "skill-2".to_string(),
                params: HashMap::new(),
                continue_on_failure: true,
            },
        ];

        let composed = library.compose_skill(
            "composed-skill".to_string(),
            "A composed skill".to_string(),
            sub_skills,
        );

        assert_eq!(composed.name, "composed-skill");
        assert_eq!(composed.composed_from.len(), 2);
        assert!(library.get_skill(&composed.id).is_some());
    }

    #[test]
    fn test_prune_skills() {
        let mut library = SkillLibrary::with_thresholds(0.7, 0.5);

        // Add a skill with low success rate
        let mut skill = create_test_skill("bad-skill");
        skill.success_count = 2;
        skill.failure_count = 8;
        library.add_skill(skill);

        let pruned = library.prune_skills();
        assert_eq!(pruned.len(), 1);
        assert!(library.get_skill(&pruned[0]).is_none());
    }

    #[test]
    fn test_library_stats() {
        let mut library = SkillLibrary::new();
        library.add_skill(create_test_skill("skill-1"));
        library.add_skill(create_test_skill("skill-2"));

        let stats = library.stats();
        assert_eq!(stats.total_skills, 2);
        assert_eq!(stats.categories, 1); // Both in "testing"
        assert!(stats.tags > 0);
    }

    #[test]
    fn test_export_import_json() {
        let mut library = SkillLibrary::new();
        library.add_skill(create_test_skill("skill-1"));
        library.add_skill(create_test_skill("skill-2"));

        let json = library.export_json();
        assert!(!json.is_empty());

        let mut new_library = SkillLibrary::new();
        let count = new_library.import_json(&json).unwrap();
        assert_eq!(count, 2);
        assert_eq!(new_library.stats().total_skills, 2);
    }

    #[test]
    fn test_skill_implementation_types() {
        let shell = SkillImplementation::Shell(vec!["echo hello".to_string()]);
        let python = SkillImplementation::PythonCode("print('hello')".to_string());
        let rust = SkillImplementation::RustCode("fn main() { println!(\"hello\"); }".to_string());

        // All should serialize/deserialize
        let shell_json = serde_json::to_string(&shell).unwrap();
        let python_json = serde_json::to_string(&python).unwrap();
        let rust_json = serde_json::to_string(&rust).unwrap();

        assert!(shell_json.contains("Shell"));
        assert!(python_json.contains("PythonCode"));
        assert!(rust_json.contains("RustCode"));
    }

    #[test]
    fn test_parameter_detection() {
        let mut library = SkillLibrary::new();
        let execution = TaskExecution {
            task_id: "t1".to_string(),
            goal: "Deploy service".to_string(),
            actions: vec![TaskAction {
                action_type: "shell".to_string(),
                content: "deploy ${service_name} to ${environment}".to_string(),
                output: "ok".to_string(),
                success: true,
            }],
            success: true,
            output: "done".to_string(),
            execution_ms: 500,
            executed_at: Utc::now(),
        };

        library.record_execution(execution);
        let extracted = library.extract_skills();
        assert_eq!(extracted.len(), 1);
        assert!(extracted[0].parameters.len() >= 2);
        let names: Vec<&str> = extracted[0]
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert!(names.contains(&"service_name"));
        assert!(names.contains(&"environment"));
    }

    #[test]
    fn test_category_detection() {
        let library = SkillLibrary::new();

        assert_eq!(
            library.detect_category("implement a new API endpoint"),
            "coding"
        );
        assert_eq!(
            library.detect_category("write unit tests for module"),
            "testing"
        );
        assert_eq!(library.detect_category("deploy to production"), "devops");
        assert_eq!(
            library.detect_category("analyze sales data"),
            "data-analysis"
        );
        assert_eq!(
            library.detect_category("read config file"),
            "file-operations"
        );
        assert_eq!(
            library.detect_category("send notification email"),
            "general"
        );
    }

    #[test]
    fn test_tag_extraction() {
        let library = SkillLibrary::new();
        let actions = vec![
            TaskAction {
                action_type: "shell".to_string(),
                content: "echo".to_string(),
                output: String::new(),
                success: true,
            },
            TaskAction {
                action_type: "python".to_string(),
                content: "print".to_string(),
                output: String::new(),
                success: true,
            },
        ];

        let tags = library.extract_tags("deploy the application to production", &actions);
        assert!(tags.contains(&"shell".to_string()));
        assert!(tags.contains(&"python".to_string()));
        assert!(tags.contains(&"deploy".to_string()));
        assert!(tags.contains(&"application".to_string()));
        assert!(tags.contains(&"production".to_string()));
        // "the" is only 3 chars, should be filtered out
        assert!(!tags.contains(&"the".to_string()));
    }

    #[test]
    fn test_mixed_implementation_types() {
        let library = SkillLibrary::new();
        let actions = vec![
            TaskAction {
                action_type: "shell".to_string(),
                content: "cargo build".to_string(),
                output: String::new(),
                success: true,
            },
            TaskAction {
                action_type: "python".to_string(),
                content: "print('test')".to_string(),
                output: String::new(),
                success: true,
            },
        ];
        // Mixed types default to Shell
        let impl_type = library.detect_implementation(&actions);
        assert!(matches!(impl_type, SkillImplementation::Shell(_)));
    }

    #[test]
    fn test_empty_actions_no_extraction() {
        let mut library = SkillLibrary::new();
        let execution = TaskExecution {
            task_id: "t1".to_string(),
            goal: "Empty task".to_string(),
            actions: vec![],
            success: true,
            output: "done".to_string(),
            execution_ms: 10,
            executed_at: Utc::now(),
        };

        library.record_execution(execution);
        let extracted = library.extract_skills();
        assert_eq!(extracted.len(), 0);
    }

    #[test]
    fn test_partial_failure_no_extraction() {
        let mut library = SkillLibrary::new();
        let execution = TaskExecution {
            task_id: "t1".to_string(),
            goal: "Partial fail".to_string(),
            actions: vec![
                TaskAction {
                    action_type: "shell".to_string(),
                    content: "echo ok".to_string(),
                    output: "ok".to_string(),
                    success: true,
                },
                TaskAction {
                    action_type: "shell".to_string(),
                    content: "false".to_string(),
                    output: "error".to_string(),
                    success: false,
                },
            ],
            success: true,
            output: "partial".to_string(),
            execution_ms: 100,
            executed_at: Utc::now(),
        };

        library.record_execution(execution);
        let extracted = library.extract_skills();
        assert_eq!(extracted.len(), 0);
    }

    #[test]
    fn test_import_invalid_json() {
        let mut library = SkillLibrary::new();
        let result = library.import_json("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse JSON"));
    }

    #[test]
    fn test_prune_preserves_low_usage_skills() {
        let mut library = SkillLibrary::with_thresholds(0.7, 0.5);

        // Skill with < 5 total uses should NOT be pruned even with low success rate
        let mut skill = create_test_skill("new-skill");
        skill.success_count = 1;
        skill.failure_count = 2;
        let id = skill.id.clone();
        library.add_skill(skill);

        let pruned = library.prune_skills();
        assert_eq!(pruned.len(), 0);
        assert!(library.get_skill(&id).is_some());
    }

    #[test]
    fn test_search_by_description() {
        let mut library = SkillLibrary::new();
        let mut skill = create_test_skill("deploy-skill");
        skill.description = "Automated Kubernetes deployment".to_string();
        library.add_skill(skill);

        let results = library.search("kubernetes");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_tag() {
        let mut library = SkillLibrary::new();
        let mut skill = create_test_skill("test-skill");
        skill.tags = vec!["rust".to_string(), "async".to_string()];
        library.add_skill(skill);

        let results = library.search("async");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_empty_library_stats() {
        let library = SkillLibrary::new();
        let stats = library.stats();
        assert_eq!(stats.total_skills, 0);
        assert_eq!(stats.reliable_skills, 0);
        assert_eq!(stats.categories, 0);
        assert_eq!(stats.tags, 0);
        assert_eq!(stats.total_usage, 0);
        assert_eq!(stats.avg_confidence, 0.0);
    }

    #[test]
    fn test_get_nonexistent_category() {
        let library = SkillLibrary::new();
        let skills = library.get_by_category("nonexistent");
        assert!(skills.is_empty());
    }

    #[test]
    fn test_get_nonexistent_tag() {
        let library = SkillLibrary::new();
        let skills = library.get_by_tag("nonexistent");
        assert!(skills.is_empty());
    }

    #[test]
    fn test_record_usage_nonexistent_skill() {
        let mut library = SkillLibrary::new();
        // Should not panic
        library.record_skill_usage("nonexistent", true, 100);
    }

    #[test]
    fn test_generate_skill_name() {
        let library = SkillLibrary::new();
        let name = library.generate_skill_name("Deploy the Application to Production!");
        assert!(name.starts_with("skill-"));
        assert!(name.contains("deploy"));
        assert!(!name.contains("!"));
        assert!(!name.contains(" "));
    }

    #[test]
    fn test_composite_skill_serialization() {
        let composite = SkillImplementation::Composite(vec![SubSkillCall {
            skill_id: "s1".to_string(),
            params: HashMap::new(),
            continue_on_failure: false,
        }]);
        let json = serde_json::to_string(&composite).unwrap();
        assert!(json.contains("Composite"));

        let deserialized: SkillImplementation = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, SkillImplementation::Composite(_)));
    }

    #[test]
    fn test_api_call_serialization() {
        let api = SkillImplementation::ApiCall {
            method: "POST".to_string(),
            url_template: "https://api.example.com/{id}".to_string(),
            headers: HashMap::new(),
            body_template: Some("{}".to_string()),
        };
        let json = serde_json::to_string(&api).unwrap();
        assert!(json.contains("ApiCall"));
        assert!(json.contains("POST"));
    }

    #[test]
    fn test_prompt_template_serialization() {
        let prompt = SkillImplementation::PromptTemplate("Analyze {{input}}".to_string());
        let json = serde_json::to_string(&prompt).unwrap();
        assert!(json.contains("PromptTemplate"));
    }
}
