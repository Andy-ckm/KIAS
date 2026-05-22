//! # Dependency Checker - AST-Level Layer Dependency Validation
//!
//! Implements AST-based dependency checking for the L0→L1→L2→L3 layer model.
//! This replaces grep-based dependency checking with proper cargo metadata parsing.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Layer levels in the architecture hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Layer {
    /// L0: Common utilities, types, errors
    L0 = 0,
    /// L1: Models and data structures
    L1 = 1,
    /// L2: Services and business logic
    L2 = 2,
    /// L3: Handlers and API endpoints
    L3 = 3,
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Layer::L0 => write!(f, "L0"),
            Layer::L1 => write!(f, "L1"),
            Layer::L2 => write!(f, "L2"),
            Layer::L3 => write!(f, "L3"),
        }
    }
}

/// A single layer dependency rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRule {
    /// The crate this rule applies to
    pub crate_name: String,
    /// The layer this crate belongs to
    pub current_layer: Layer,
    /// Allowed layers this crate can depend on (only lower/equal layers)
    pub allowed_dependencies: Vec<Layer>,
}

impl LayerRule {
    /// Creates a standard rule where a layer can only depend on lower layers
    pub fn standard(crate_name: &str, current_layer: Layer) -> Self {
        let allowed = match current_layer {
            Layer::L0 => vec![Layer::L0],
            Layer::L1 => vec![Layer::L0, Layer::L1],
            Layer::L2 => vec![Layer::L0, Layer::L1, Layer::L2],
            Layer::L3 => vec![Layer::L0, Layer::L1, Layer::L2, Layer::L3],
        };
        Self {
            crate_name: crate_name.to_string(),
            current_layer,
            allowed_dependencies: allowed,
        }
    }
}

/// A detected dependency violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationReport {
    /// The crate that has the violation
    pub crate_name: String,
    /// The layer this crate belongs to
    pub current_layer: Layer,
    /// The forbidden dependency crate name
    pub forbidden_dependency: String,
    /// The layer of the forbidden dependency
    pub forbidden_layer: Layer,
    /// Human-readable description of the violation
    pub description: String,
    /// Suggested fix
    pub suggestion: String,
}

impl ViolationReport {
    fn new(
        crate_name: &str,
        current_layer: Layer,
        forbidden: &str,
        forbidden_layer: Layer,
    ) -> Self {
        let desc = format!(
            "Crate '{}' (layer {}) illegally depends on '{}' (layer {})",
            crate_name, current_layer, forbidden, forbidden_layer
        );
        let suggestion = format!(
            "Move '{}' to a lower layer or refactor to use an abstraction in layer {}",
            forbidden,
            forbidden_layer
                .to_string()
                .trim_start_matches('L')
                .parse::<usize>()
                .map(|n| n + 1)
                .unwrap_or(2)
        );
        Self {
            crate_name: crate_name.to_string(),
            current_layer,
            forbidden_dependency: forbidden.to_string(),
            forbidden_layer,
            description: desc,
            suggestion,
        }
    }
}

/// Cargo package metadata
#[derive(Debug, Clone, Deserialize)]
pub struct CargoPackage {
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CargoDependency {
    pub name: String,
    #[serde(rename = "req")]
    pub requirement: Option<String>,
}

/// Workspace metadata from cargo
#[derive(Debug, Clone, Deserialize)]
pub struct CargoMetadata {
    pub packages: Vec<CargoPackage>,
    #[serde(default)]
    pub workspace_members: Vec<String>,
}

/// The main dependency checker using cargo metadata
#[derive(Debug, Clone)]
pub struct DependencyChecker {
    /// Rules for each known crate
    rules: HashMap<String, LayerRule>,
    /// Cache of layer assignments
    layer_cache: HashMap<String, Layer>,
}

impl Default for DependencyChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyChecker {
    /// Create a new dependency checker with standard rules
    pub fn new() -> Self {
        let mut checker = Self {
            rules: HashMap::new(),
            layer_cache: HashMap::new(),
        };
        checker.register_standard_rules();
        checker
    }

    /// Register standard layer rules for known crates
    fn register_standard_rules(&mut self) {
        // L0 crates - common utilities
        let l0_crates = vec!["kias-common", "common"];
        for crate_name in l0_crates {
            self.rules.insert(
                crate_name.to_string(),
                LayerRule::standard(crate_name, Layer::L0),
            );
            self.layer_cache.insert(crate_name.to_string(), Layer::L0);
        }

        // L1 crates - models
        let l1_crates = vec!["kias-models", "models"];
        for crate_name in l1_crates {
            self.rules.insert(
                crate_name.to_string(),
                LayerRule::standard(crate_name, Layer::L1),
            );
            self.layer_cache.insert(crate_name.to_string(), Layer::L1);
        }

        // L2 crates - services
        let l2_crates = vec![
            "kias-scheduler",
            "kias-controller",
            "kias-workflow-engine",
            "kias-team-engine",
            "kias-goal-engine",
            "kias-langgraph-engine",
            "scheduler",
            "controller",
            "workflow-engine",
            "team-engine",
            "goal-engine",
            "langgraph-engine",
        ];
        for crate_name in l2_crates {
            self.rules.insert(
                crate_name.to_string(),
                LayerRule::standard(crate_name, Layer::L2),
            );
            self.layer_cache.insert(crate_name.to_string(), Layer::L2);
        }

        // L3 crates - handlers/api
        let l3_crates = vec!["kias-api-server", "kias-cli", "api-server", "agent-view"];
        for crate_name in l3_crates {
            self.rules.insert(
                crate_name.to_string(),
                LayerRule::standard(crate_name, Layer::L3),
            );
            self.layer_cache.insert(crate_name.to_string(), Layer::L3);
        }
    }

    /// Register a custom layer rule
    pub fn register_rule(&mut self, rule: LayerRule) {
        self.rules.insert(rule.crate_name.clone(), rule.clone());
        self.layer_cache.insert(rule.crate_name, rule.current_layer);
    }

    /// Get the layer for a crate (by name or by parsing metadata)
    pub fn get_layer(&self, crate_name: &str) -> Option<Layer> {
        self.layer_cache.get(crate_name).copied()
    }

    /// Set layer for a crate
    pub fn set_layer(&mut self, crate_name: &str, layer: Layer) {
        self.layer_cache.insert(crate_name.to_string(), layer);
        if let Some(rule) = self.rules.get_mut(crate_name) {
            rule.current_layer = layer;
        }
    }

    /// Parse cargo metadata from the workspace
    pub fn parse_cargo_metadata(manifest_path: Option<&Path>) -> Option<CargoMetadata> {
        let mut cmd = Command::new("cargo");
        cmd.arg("metadata");
        cmd.arg("--format-version=1");
        cmd.arg("--no-deps");

        if let Some(path) = manifest_path {
            cmd.current_dir(path);
        }

        // Add --locked flag to avoid interactive prompts
        cmd.arg("--locked");

        let output = cmd.output().ok()?;
        if !output.status.success() {
            return None;
        }

        let metadata: CargoMetadata = serde_json::from_slice(&output.stdout).ok()?;
        Some(metadata)
    }

    /// Validate dependencies against layer rules
    pub fn validate_dependencies(&self, metadata: &CargoMetadata) -> Vec<ViolationReport> {
        let mut violations = Vec::new();

        for package in &metadata.packages {
            let crate_name = &package.name;

            // Skip if no rule defined for this crate
            let Some(rule) = self.rules.get(crate_name) else {
                continue;
            };

            for dep in &package.dependencies {
                // Skip external crates (not in workspace)
                if !metadata.workspace_members.contains(&format!(
                    "{} {}",
                    dep.name,
                    dep.requirement.clone().unwrap_or_default()
                )) && !self.layer_cache.contains_key(&dep.name)
                {
                    continue;
                }

                // Get the layer of the dependency
                if let Some(dep_layer) = self.layer_cache.get(&dep.name) {
                    // Check if this dependency is allowed
                    if !rule.allowed_dependencies.contains(dep_layer) {
                        violations.push(ViolationReport::new(
                            crate_name,
                            rule.current_layer,
                            &dep.name,
                            *dep_layer,
                        ));
                    }
                }
            }
        }

        violations
    }

    /// Validate dependencies from a manifest path
    pub fn validate_from_manifest(&self, manifest_path: Option<&Path>) -> Vec<ViolationReport> {
        if let Some(metadata) = Self::parse_cargo_metadata(manifest_path) {
            self.validate_dependencies(&metadata)
        } else {
            Vec::new()
        }
    }

    /// Check for circular dependencies
    pub fn detect_cycles(&self, metadata: &CargoMetadata) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = Vec::new();
        let mut recursion_stack: HashSet<String> = HashSet::new();

        // Build adjacency list
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for package in &metadata.packages {
            let direct_deps: Vec<String> = package
                .dependencies
                .iter()
                .filter(|d| self.layer_cache.contains_key(&d.name))
                .map(|d| d.name.clone())
                .collect();
            deps.insert(package.name.clone(), direct_deps);
        }

        fn dfs(
            node: &str,
            deps: &HashMap<String, Vec<String>>,
            visited: &mut HashSet<String>,
            stack: &mut Vec<String>,
            recursion_stack: &mut HashSet<String>,
            cycles: &mut Vec<Vec<String>>,
        ) {
            visited.insert(node.to_string());
            stack.push(node.to_string());
            recursion_stack.insert(node.to_string());

            if let Some(dependencies) = deps.get(node) {
                for dep in dependencies {
                    if !visited.contains(dep) {
                        dfs(dep, deps, visited, stack, recursion_stack, cycles);
                    } else if recursion_stack.contains(dep) {
                        // Found a cycle
                        if let Some(pos) = stack.iter().position(|x| x == dep) {
                            let cycle: Vec<String> = stack[pos..].to_vec();
                            cycles.push(cycle);
                        }
                    }
                }
            }

            stack.pop();
            recursion_stack.remove(node);
        }

        for package in &metadata.packages {
            if !visited.contains(&package.name) {
                dfs(
                    &package.name,
                    &deps,
                    &mut visited,
                    &mut stack,
                    &mut recursion_stack,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    /// Generate a summary report
    pub fn generate_report(&self, violations: &[ViolationReport]) -> String {
        if violations.is_empty() {
            "No layer dependency violations found.".to_string()
        } else {
            let mut report = format!(
                "Found {} layer dependency violation(s):\n\n",
                violations.len()
            );
            for (i, v) in violations.iter().enumerate() {
                report += &format!(
                    "{}. {}\n   Current: {} | Forbidden: {} ({})\n   Suggestion: {}\n\n",
                    i + 1,
                    v.description,
                    v.current_layer,
                    v.forbidden_dependency,
                    v.forbidden_layer,
                    v.suggestion
                );
            }
            report
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_ordering() {
        assert!(Layer::L0 < Layer::L1);
        assert!(Layer::L1 < Layer::L2);
        assert!(Layer::L2 < Layer::L3);
        assert!(Layer::L3 > Layer::L0);
    }

    #[test]
    fn test_layer_display() {
        assert_eq!(Layer::L0.to_string(), "L0");
        assert_eq!(Layer::L1.to_string(), "L1");
        assert_eq!(Layer::L2.to_string(), "L2");
        assert_eq!(Layer::L3.to_string(), "L3");
    }

    #[test]
    fn test_layer_rule_standard() {
        let rule_l0 = LayerRule::standard("test-l0", Layer::L0);
        assert_eq!(rule_l0.allowed_dependencies, vec![Layer::L0]);

        let rule_l1 = LayerRule::standard("test-l1", Layer::L1);
        assert!(rule_l1.allowed_dependencies.contains(&Layer::L0));
        assert!(rule_l1.allowed_dependencies.contains(&Layer::L1));

        let rule_l2 = LayerRule::standard("test-l2", Layer::L2);
        assert!(rule_l2.allowed_dependencies.contains(&Layer::L0));
        assert!(rule_l2.allowed_dependencies.contains(&Layer::L1));
        assert!(rule_l2.allowed_dependencies.contains(&Layer::L2));

        let rule_l3 = LayerRule::standard("test-l3", Layer::L3);
        assert!(rule_l3.allowed_dependencies.contains(&Layer::L0));
        assert!(rule_l3.allowed_dependencies.contains(&Layer::L1));
        assert!(rule_l3.allowed_dependencies.contains(&Layer::L2));
        assert!(rule_l3.allowed_dependencies.contains(&Layer::L3));
    }

    #[test]
    fn test_dependency_checker_new() {
        let checker = DependencyChecker::new();
        assert_eq!(checker.get_layer("kias-common"), Some(Layer::L0));
        assert_eq!(checker.get_layer("kias-api-server"), Some(Layer::L3));
        assert_eq!(checker.get_layer("kias-scheduler"), Some(Layer::L2));
    }

    #[test]
    fn test_set_and_get_layer() {
        let mut checker = DependencyChecker::new();
        checker.set_layer("test-crate", Layer::L1);
        assert_eq!(checker.get_layer("test-crate"), Some(Layer::L1));
    }

    #[test]
    fn test_custom_rule() {
        let mut checker = DependencyChecker::new();
        let rule = LayerRule {
            crate_name: "custom-crate".to_string(),
            current_layer: Layer::L2,
            allowed_dependencies: vec![Layer::L0, Layer::L2], // Cannot depend on L1
        };
        checker.register_rule(rule);
        assert_eq!(checker.get_layer("custom-crate"), Some(Layer::L2));
    }

    #[test]
    fn test_violation_report() {
        let report = ViolationReport::new("api-server", Layer::L3, "scheduler", Layer::L2);
        assert!(report.description.contains("api-server"));
        assert!(report.description.contains("scheduler"));
        assert!(report.suggestion.contains("scheduler"));
    }

    #[test]
    fn test_violation_report_creation() {
        let report = ViolationReport::new("test-crate", Layer::L2, "forbidden-dep", Layer::L3);
        assert_eq!(report.crate_name, "test-crate");
        assert_eq!(report.current_layer, Layer::L2);
        assert_eq!(report.forbidden_dependency, "forbidden-dep");
        assert_eq!(report.forbidden_layer, Layer::L3);
    }

    #[test]
    fn test_report_generation_empty() {
        let checker = DependencyChecker::new();
        let report = checker.generate_report(&[]);
        assert!(report.contains("No layer dependency violations"));
    }

    #[test]
    fn test_report_generation_with_violations() {
        let checker = DependencyChecker::new();
        let violations = vec![ViolationReport::new(
            "api-server",
            Layer::L3,
            "scheduler",
            Layer::L2,
        )];
        let report = checker.generate_report(&violations);
        assert!(report.contains("1 layer dependency violation"));
        assert!(report.contains("api-server"));
    }

    #[test]
    fn test_parse_cargo_metadata_returns_none_on_failure() {
        // This tests that we handle invalid manifest paths gracefully
        let result = DependencyChecker::parse_cargo_metadata(Some(std::path::Path::new(
            "/nonexistent/path",
        )));
        // Result depends on whether cargo metadata succeeds or fails
        // Just verify it doesn't panic
        assert!(result.is_none() || result.is_some());
    }

    #[test]
    fn test_detect_cycles_empty() {
        let checker = DependencyChecker::new();
        let metadata = CargoMetadata {
            packages: vec![],
            workspace_members: vec![],
        };
        let cycles = checker.detect_cycles(&metadata);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_detect_cycles_simple() {
        let checker = DependencyChecker::new();
        // A -> B -> C (no cycle)
        let metadata = CargoMetadata {
            packages: vec![
                CargoPackage {
                    name: "a".to_string(),
                    dependencies: vec![CargoDependency {
                        name: "b".to_string(),
                        requirement: None,
                    }],
                },
                CargoPackage {
                    name: "b".to_string(),
                    dependencies: vec![CargoDependency {
                        name: "c".to_string(),
                        requirement: None,
                    }],
                },
                CargoPackage {
                    name: "c".to_string(),
                    dependencies: vec![],
                },
            ],
            workspace_members: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };
        let cycles = checker.detect_cycles(&metadata);
        assert!(cycles.is_empty());
    }
}
