//! Task Decomposition and Planning Module
//!
//! Decomposes complex goals into executable task sequences with dependency tracking,
//! cost estimation, and risk assessment. Learns from historical execution to improve plans.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique step identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Prerequisites (IDs of steps that must complete first)
    pub dependencies: Vec<String>,
    /// Estimated time cost in milliseconds
    pub estimated_cost_ms: u64,
    /// Estimated compute cost in tokens
    pub estimated_token_cost: u64,
    /// Risk level 0-10
    pub risk_level: u8,
    /// Whether this step can be parallelized with others
    pub parallelizable: bool,
    /// Tags for categorization
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Step ID that was executed
    pub step_id: String,
    /// Actual time taken
    pub actual_cost_ms: u64,
    /// Whether step succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    /// Template name (e.g., "code_review", "data_migration")
    pub name: String,
    /// Template steps
    pub steps: Vec<PlanStep>,
    /// Number of times this template was used
    pub usage_count: u32,
    /// Historical success rate
    pub success_rate: f64,
}

pub struct TaskPlanner {
    /// Historical execution records for learning
    history: Vec<ExecutionRecord>,
    /// Reusable plan templates
    templates: HashMap<String, PlanTemplate>,
    /// Learning rate for cost prediction (0.0 - 1.0)
    learning_rate: f64,
}

impl Default for TaskPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskPlanner {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            templates: HashMap::new(),
            learning_rate: 0.2,
        }
    }

    /// Create planner with custom learning rate
    pub fn with_learning_rate(mut self, rate: f64) -> Self {
        self.learning_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Register a plan template
    pub fn register_template(&mut self, template: PlanTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Decompose a complex goal into plan steps using template matching
    pub fn decompose(&self, goal: &str) -> Vec<PlanStep> {
        // Simple keyword-based decomposition (production would use LLM)
        let goal_lower = goal.to_lowercase();
        
        if goal_lower.contains("code review") || goal_lower.contains("review code") {
            vec![
                PlanStep {
                    id: "cr_1".to_string(),
                    description: "Fetch code changes".to_string(),
                    dependencies: vec![],
                    estimated_cost_ms: 500,
                    estimated_token_cost: 100,
                    risk_level: 1,
                    parallelizable: false,
                    tags: vec!["fetch".to_string()],
                },
                PlanStep {
                    id: "cr_2".to_string(),
                    description: "Static analysis scan".to_string(),
                    dependencies: vec!["cr_1".to_string()],
                    estimated_cost_ms: 2000,
                    estimated_token_cost: 500,
                    risk_level: 2,
                    parallelizable: false,
                    tags: vec!["analysis".to_string()],
                },
                PlanStep {
                    id: "cr_3".to_string(),
                    description: "Security vulnerability check".to_string(),
                    dependencies: vec!["cr_1".to_string()],
                    estimated_cost_ms: 3000,
                    estimated_token_cost: 800,
                    risk_level: 4,
                    parallelizable: true,
                    tags: vec!["security".to_string()],
                },
                PlanStep {
                    id: "cr_4".to_string(),
                    description: "Generate review report".to_string(),
                    dependencies: vec!["cr_2".to_string(), "cr_3".to_string()],
                    estimated_cost_ms: 1000,
                    estimated_token_cost: 300,
                    risk_level: 1,
                    parallelizable: false,
                    tags: vec!["report".to_string()],
                },
            ]
        } else if goal_lower.contains("deploy") || goal_lower.contains("release") {
            vec![
                PlanStep {
                    id: "dp_1".to_string(),
                    description: "Build artifacts".to_string(),
                    dependencies: vec![],
                    estimated_cost_ms: 10000,
                    estimated_token_cost: 200,
                    risk_level: 3,
                    parallelizable: false,
                    tags: vec!["build".to_string()],
                },
                PlanStep {
                    id: "dp_2".to_string(),
                    description: "Run test suite".to_string(),
                    dependencies: vec!["dp_1".to_string()],
                    estimated_cost_ms: 30000,
                    estimated_token_cost: 1000,
                    risk_level: 5,
                    parallelizable: false,
                    tags: vec!["test".to_string()],
                },
                PlanStep {
                    id: "dp_3".to_string(),
                    description: "Security scan".to_string(),
                    dependencies: vec!["dp_1".to_string()],
                    estimated_cost_ms: 5000,
                    estimated_token_cost: 500,
                    risk_level: 4,
                    parallelizable: true,
                    tags: vec!["security".to_string()],
                },
                PlanStep {
                    id: "dp_4".to_string(),
                    description: "Deploy to staging".to_string(),
                    dependencies: vec!["dp_2".to_string(), "dp_3".to_string()],
                    estimated_cost_ms: 5000,
                    estimated_token_cost: 100,
                    risk_level: 6,
                    parallelizable: false,
                    tags: vec!["deploy".to_string()],
                },
                PlanStep {
                    id: "dp_5".to_string(),
                    description: "Smoke tests".to_string(),
                    dependencies: vec!["dp_4".to_string()],
                    estimated_cost_ms: 3000,
                    estimated_token_cost: 200,
                    risk_level: 5,
                    parallelizable: false,
                    tags: vec!["test".to_string()],
                },
                PlanStep {
                    id: "dp_6".to_string(),
                    description: "Promote to production".to_string(),
                    dependencies: vec!["dp_5".to_string()],
                    estimated_cost_ms: 2000,
                    estimated_token_cost: 50,
                    risk_level: 8,
                    parallelizable: false,
                    tags: vec!["deploy".to_string(), "critical".to_string()],
                },
            ]
        } else {
            // Generic decomposition
            vec![
                PlanStep {
                    id: "gen_1".to_string(),
                    description: "Understand requirements".to_string(),
                    dependencies: vec![],
                    estimated_cost_ms: 1000,
                    estimated_token_cost: 500,
                    risk_level: 2,
                    parallelizable: false,
                    tags: vec!["analysis".to_string()],
                },
                PlanStep {
                    id: "gen_2".to_string(),
                    description: "Plan execution steps".to_string(),
                    dependencies: vec!["gen_1".to_string()],
                    estimated_cost_ms: 500,
                    estimated_token_cost: 300,
                    risk_level: 1,
                    parallelizable: false,
                    tags: vec!["planning".to_string()],
                },
                PlanStep {
                    id: "gen_3".to_string(),
                    description: "Execute plan".to_string(),
                    dependencies: vec!["gen_2".to_string()],
                    estimated_cost_ms: 5000,
                    estimated_token_cost: 1000,
                    risk_level: 3,
                    parallelizable: false,
                    tags: vec!["execution".to_string()],
                },
                PlanStep {
                    id: "gen_4".to_string(),
                    description: "Verify results".to_string(),
                    dependencies: vec!["gen_3".to_string()],
                    estimated_cost_ms: 1000,
                    estimated_token_cost: 200,
                    risk_level: 2,
                    parallelizable: false,
                    tags: vec!["verification".to_string()],
                },
            ]
        }
    }

    /// Compute execution order using topological sort
    pub fn compute_execution_order(steps: &[PlanStep]) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        
        // Initialize
        for step in steps {
            in_degree.insert(step.id.clone(), 0);
            graph.insert(step.id.clone(), Vec::new());
        }
        
        // Build graph and compute in-degrees
        for step in steps {
            for dep in &step.dependencies {
                if !in_degree.contains_key(dep) {
                    return Err(format!("Unknown dependency: {}", dep));
                }
                graph.get_mut(dep).unwrap().push(step.id.clone());
                *in_degree.get_mut(&step.id).unwrap() += 1;
            }
        }
        
        // Kahn's algorithm
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| k.clone())
            .collect();
        queue.sort(); // Deterministic ordering
        
        let mut result = Vec::new();
        while !queue.is_empty() {
            let node = queue.remove(0);
            result.push(node.clone());
            
            for neighbor in graph.get(&node).unwrap() {
                *in_degree.get_mut(neighbor).unwrap() -= 1;
                if in_degree[neighbor] == 0 {
                    queue.push(neighbor.clone());
                    queue.sort();
                }
            }
        }
        
        if result.len() != steps.len() {
            return Err("Circular dependency detected".to_string());
        }
        
        Ok(result)
    }

    /// Compute total estimated cost for a plan
    pub fn compute_total_cost(steps: &[PlanStep]) -> (u64, u64, u8) {
        let total_ms: u64 = steps.iter().map(|s| s.estimated_cost_ms).sum();
        let total_tokens: u64 = steps.iter().map(|s| s.estimated_token_cost).sum();
        let max_risk = steps.iter().map(|s| s.risk_level).max().unwrap_or(0);
        (total_ms, total_tokens, max_risk)
    }

    /// Identify critical path (longest path through the plan)
    pub fn find_critical_path(steps: &[PlanStep]) -> Vec<String> {
        let order = match Self::compute_execution_order(steps) {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        
        let mut dist: HashMap<String, (i64, Option<String>)> = HashMap::new();
        for step in steps {
            dist.insert(step.id.clone(), (step.estimated_cost_ms as i64, None));
        }
        
        for step_id in &order {
            let current_dist = dist.get(step_id).map(|(d, _)| *d).unwrap_or(0);
            for other in steps.iter().filter(|s| s.dependencies.contains(step_id)) {
                let new_dist = current_dist + other.estimated_cost_ms as i64;
                let existing = dist.get(&other.id).map(|(d, _)| *d).unwrap_or(0);
                if new_dist > existing {
                    dist.insert(other.id.clone(), (new_dist, Some(step_id.clone())));
                }
            }
        }
        
        // Find max
        let max_entry = dist.iter().max_by_key(|(_, (d, _))| *d);
        if let Some((end, _)) = max_entry {
            let mut path = Vec::new();
            let mut current = Some(end.clone());
            while let Some(id) = current {
                path.push(id.clone());
                current = dist.get(&id).unwrap().1.clone();
            }
            path.reverse();
            return path;
        }
        
        Vec::new()
    }

    /// Record execution result for learning
    pub fn record_execution(&mut self, record: ExecutionRecord) {
        self.history.push(record);
        // Keep history bounded
        if self.history.len() > 10000 {
            self.history.drain(0..1000);
        }
    }

    /// Refine cost estimates based on history
    pub fn refine_estimates(&self, step_id: &str, steps: &mut [PlanStep]) {
        if self.history.is_empty() {
            return;
        }
        
        let recent_records: Vec<_> = self.history.iter()
            .filter(|r| r.step_id == step_id)
            .rev()
            .take(10)
            .collect();
        
        if recent_records.is_empty() {
            return;
        }
        
        let avg_actual: f64 = recent_records.iter()
            .map(|r| r.actual_cost_ms as f64)
            .sum::<f64>() / recent_records.len() as f64;
        
        if let Some(step) = steps.iter_mut().find(|s| s.id == step_id) {
            let current = step.estimated_cost_ms as f64;
            step.estimated_cost_ms = ((1.0 - self.learning_rate) * current 
                + self.learning_rate * avg_actual) as u64;
        }
    }

    /// Get parallelizable groups (steps that can run concurrently)
    pub fn get_parallel_groups(steps: &[PlanStep]) -> Vec<Vec<String>> {
        let order = match Self::compute_execution_order(steps) {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut completed: HashSet<String> = HashSet::new();
        let mut remaining: Vec<_> = steps.iter().collect();
        
        while !remaining.is_empty() {
            let mut group = Vec::new();
            remaining.retain(|step| {
                let deps_done = step.dependencies.iter().all(|d| completed.contains(d));
                if deps_done && step.parallelizable {
                    group.push(step.id.clone());
                    false
                } else if deps_done && group.is_empty() {
                    // Non-parallel step goes alone
                    group.push(step.id.clone());
                    false
                } else {
                    true
                }
            });
            
            for id in &group {
                completed.insert(id.clone());
            }
            groups.push(group);
        }
        
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompose_code_review() {
        let planner = TaskPlanner::new();
        let steps = planner.decompose("Review code changes for security");
        assert!(!steps.is_empty());
        assert!(steps.iter().all(|s| !s.id.is_empty()));
    }

    #[test]
    fn test_decompose_deploy() {
        let planner = TaskPlanner::new();
        let steps = planner.decompose("Deploy application to production");
        assert!(steps.len() >= 5);
        // Last step should depend on earlier ones
        let last = steps.last().unwrap();
        assert!(!last.dependencies.is_empty() || last.id.starts_with("dp_"));
    }

    #[test]
    fn test_compute_execution_order() {
        let steps = vec![
            PlanStep {
                id: "a".to_string(),
                description: "Step A".to_string(),
                dependencies: vec![],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "b".to_string(),
                description: "Step B".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 200,
                estimated_token_cost: 20,
                risk_level: 2,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "c".to_string(),
                description: "Step C".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 150,
                estimated_token_cost: 15,
                risk_level: 1,
                parallelizable: true,
                tags: vec![],
            },
        ];
        
        let order = TaskPlanner::compute_execution_order(&steps).unwrap();
        assert_eq!(order[0], "a");
        // b and c can be in any order after a
        assert!(order.contains(&"b".to_string()));
        assert!(order.contains(&"c".to_string()));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let steps = vec![
            PlanStep {
                id: "a".to_string(),
                description: "A".to_string(),
                dependencies: vec!["b".to_string()],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "b".to_string(),
                description: "B".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
        ];
        
        assert!(TaskPlanner::compute_execution_order(&steps).is_err());
    }

    #[test]
    fn test_compute_total_cost() {
        let steps = vec![
            PlanStep {
                id: "a".to_string(),
                description: "A".to_string(),
                dependencies: vec![],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 3,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "b".to_string(),
                description: "B".to_string(),
                dependencies: vec![],
                estimated_cost_ms: 200,
                estimated_token_cost: 20,
                risk_level: 5,
                parallelizable: false,
                tags: vec![],
            },
        ];
        
        let (ms, tokens, risk) = TaskPlanner::compute_total_cost(&steps);
        assert_eq!(ms, 300);
        assert_eq!(tokens, 30);
        assert_eq!(risk, 5);
    }

    #[test]
    fn test_find_critical_path() {
        let steps = vec![
            PlanStep {
                id: "a".to_string(),
                description: "A".to_string(),
                dependencies: vec![],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "b".to_string(),
                description: "B".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 500,
                estimated_token_cost: 50,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "c".to_string(),
                description: "C".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
        ];
        
        let path = TaskPlanner::find_critical_path(&steps);
        assert_eq!(path.first(), Some(&"a".to_string()));
        assert_eq!(path.last(), Some(&"b".to_string()));
    }

    #[test]
    fn test_record_and_refine() {
        let mut planner = TaskPlanner::new();
        planner.record_execution(ExecutionRecord {
            step_id: "test_1".to_string(),
            actual_cost_ms: 500,
            success: true,
            error: None,
            timestamp: chrono::Utc::now(),
        });
        
        let mut steps = vec![PlanStep {
            id: "test_1".to_string(),
            description: "Test".to_string(),
            dependencies: vec![],
            estimated_cost_ms: 400,
            estimated_token_cost: 10,
            risk_level: 1,
            parallelizable: false,
            tags: vec![],
        }];
        
        planner.refine_estimates("test_1", &mut steps);
        // After learning, estimate should move toward 500
        assert!(steps[0].estimated_cost_ms >= 400);
    }

    #[test]
    fn test_parallel_groups() {
        let steps = vec![
            PlanStep {
                id: "a".to_string(),
                description: "A".to_string(),
                dependencies: vec![],
                estimated_cost_ms: 100,
                estimated_token_cost: 10,
                risk_level: 1,
                parallelizable: false,
                tags: vec![],
            },
            PlanStep {
                id: "b".to_string(),
                description: "B".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 200,
                estimated_token_cost: 20,
                risk_level: 1,
                parallelizable: true,
                tags: vec![],
            },
            PlanStep {
                id: "c".to_string(),
                description: "C".to_string(),
                dependencies: vec!["a".to_string()],
                estimated_cost_ms: 150,
                estimated_token_cost: 15,
                risk_level: 1,
                parallelizable: true,
                tags: vec![],
            },
        ];
        
        let groups = TaskPlanner::get_parallel_groups(&steps);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec!["a"]);
        assert!(groups[1].contains(&"b".to_string()));
        assert!(groups[1].contains(&"c".to_string()));
    }
}
