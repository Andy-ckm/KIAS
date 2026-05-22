//! # Test Framework - Testing Pyramid Implementation
//!
//! Implements a comprehensive test pyramid framework with multiple test levels,
//! test suite management, coverage reporting, and test matrices.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Test levels in the testing pyramid
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TestLevel {
    /// Unit tests - individual function/component testing
    Unit = 0,
    /// Contract tests - API contract verification
    Contract = 1,
    /// Integration tests - component interaction testing
    Integration = 2,
    /// End-to-end tests - full system testing
    E2E = 3,
    /// Chaos tests - failure injection and resilience testing
    Chaos = 4,
}

impl std::fmt::Display for TestLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestLevel::Unit => write!(f, "Unit"),
            TestLevel::Contract => write!(f, "Contract"),
            TestLevel::Integration => write!(f, "Integration"),
            TestLevel::E2E => write!(f, "E2E"),
            TestLevel::Chaos => write!(f, "Chaos"),
        }
    }
}

/// A single test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub level: TestLevel,
    pub module: String,
    pub tags: Vec<String>,
    pub enabled: bool,
}

impl TestCase {
    pub fn new(id: &str, name: &str, level: TestLevel, module: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            level,
            module: module.to_string(),
            tags: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// Coverage data for a module
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleCoverage {
    pub module: String,
    pub lines_covered: u32,
    pub lines_total: u32,
    pub branches_covered: u32,
    pub branches_total: u32,
    pub functions_covered: u32,
    pub functions_total: u32,
}

impl ModuleCoverage {
    pub fn new(module: &str) -> Self {
        Self {
            module: module.to_string(),
            ..Default::default()
        }
    }

    pub fn line_coverage_percent(&self) -> f64 {
        if self.lines_total == 0 {
            return 100.0;
        }
        (self.lines_covered as f64 / self.lines_total as f64) * 100.0
    }

    pub fn branch_coverage_percent(&self) -> f64 {
        if self.branches_total == 0 {
            return 100.0;
        }
        (self.branches_covered as f64 / self.branches_total as f64) * 100.0
    }

    pub fn function_coverage_percent(&self) -> f64 {
        if self.functions_total == 0 {
            return 100.0;
        }
        (self.functions_covered as f64 / self.functions_total as f64) * 100.0
    }

    pub fn overall_coverage_percent(&self) -> f64 {
        (self.line_coverage_percent() + self.branch_coverage_percent() + self.function_coverage_percent()) / 3.0
    }
}

/// Coverage report aggregating all modules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageReport {
    pub module_coverages: HashMap<String, ModuleCoverage>,
    pub total_lines_covered: u32,
    pub total_lines_total: u32,
    pub total_branches_covered: u32,
    pub total_branches_total: u32,
    pub total_functions_covered: u32,
    pub total_functions_total: u32,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl CoverageReport {
    pub fn new() -> Self {
        Self {
            module_coverages: HashMap::new(),
            generated_at: chrono::Utc::now(),
            ..Default::default()
        }
    }

    pub fn add_module(&mut self, coverage: ModuleCoverage) {
        self.total_lines_covered += coverage.lines_covered;
        self.total_lines_total += coverage.lines_total;
        self.total_branches_covered += coverage.branches_covered;
        self.total_branches_total += coverage.branches_total;
        self.total_functions_covered += coverage.functions_covered;
        self.total_functions_total += coverage.functions_total;
        self.module_coverages.insert(coverage.module.clone(), coverage);
    }

    pub fn overall_line_coverage(&self) -> f64 {
        if self.total_lines_total == 0 {
            return 100.0;
        }
        (self.total_lines_covered as f64 / self.total_lines_total as f64) * 100.0
    }

    pub fn overall_branch_coverage(&self) -> f64 {
        if self.total_branches_total == 0 {
            return 100.0;
        }
        (self.total_branches_covered as f64 / self.total_branches_total as f64) * 100.0
    }

    pub fn overall_function_coverage(&self) -> f64 {
        if self.total_functions_total == 0 {
            return 100.0;
        }
        (self.total_functions_covered as f64 / self.total_functions_total as f64) * 100.0
    }

    pub fn overall_coverage(&self) -> f64 {
        (self.overall_line_coverage() + self.overall_branch_coverage() + self.overall_function_coverage()) / 3.0
    }
}

/// A test suite containing multiple test cases
#[derive(Debug, Clone, Default)]
pub struct TestSuite {
    pub name: String,
    pub test_cases: Vec<TestCase>,
    pub by_level: BTreeMap<TestLevel, Vec<TestCase>>,
    by_module: HashMap<String, Vec<TestCase>>,
}

impl TestSuite {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            test_cases: Vec::new(),
            by_level: BTreeMap::new(),
            by_module: HashMap::new(),
        }
    }

    pub fn register(&mut self, test: TestCase) {
        // Add to by_level index
        self.by_level
            .entry(test.level)
            .or_insert_with(Vec::new)
            .push(test.clone());

        // Add to by_module index
        self.by_module
            .entry(test.module.clone())
            .or_insert_with(Vec::new)
            .push(test.clone());

        // Add to main collection
        self.test_cases.push(test);
    }

    pub fn by_level(&self, level: TestLevel) -> Option<&Vec<TestCase>> {
        self.by_level.get(&level)
    }

    pub fn by_module(&self, module: &str) -> Option<&Vec<TestCase>> {
        self.by_module.get(module)
    }

    pub fn total_count(&self) -> usize {
        self.test_cases.len()
    }

    pub fn level_counts(&self) -> HashMap<TestLevel, usize> {
        self.by_level
            .iter()
            .map(|(k, v)| (*k, v.len()))
            .collect()
    }

    pub fn module_counts(&self) -> HashMap<String, usize> {
        self.by_module.iter().map(|(k, v)| (k.clone(), v.len())).collect()
    }

    pub fn enabled_tests(&self) -> Vec<&TestCase> {
        self.test_cases.iter().filter(|t| t.enabled).collect()
    }
}

/// A cell in the test matrix
#[derive(Debug, Clone)]
pub struct TestMatrixCell {
    pub level: TestLevel,
    pub module: String,
    pub test_count: usize,
    pub coverage: Option<f64>,
}

/// Test matrix showing test distribution across levels and modules
#[derive(Debug, Clone)]
pub struct TestMatrix {
    pub cells: Vec<TestMatrixCell>,
    pub levels: Vec<TestLevel>,
    pub modules: Vec<String>,
}

impl TestMatrix {
    pub fn from_suite(suite: &TestSuite, coverages: &HashMap<String, f64>) -> Self {
        let levels: Vec<TestLevel> = suite.by_level.keys().cloned().collect();
        let modules: Vec<String> = suite.by_module.keys().cloned().collect();

        let mut cells = Vec::new();
        for (module, tests) in &suite.by_module {
            let test_count = tests.len();
            let coverage = coverages.get(module).copied();
            for level in suite.by_level.keys() {
                let level_test_count = tests.iter().filter(|t| t.level == *level).count();
                if level_test_count > 0 {
                    cells.push(TestMatrixCell {
                        level: *level,
                        module: module.clone(),
                        test_count: level_test_count,
                        coverage,
                    });
                }
            }
        }

        Self {
            cells,
            levels,
            modules,
        }
    }

    pub fn total_tests(&self) -> usize {
        self.cells.iter().map(|c| c.test_count).sum()
    }
}

/// The main test pyramid framework
#[derive(Debug, Clone)]
pub struct TestPyramid {
    pub suite: TestSuite,
    pub coverage_report: Option<CoverageReport>,
}

impl Default for TestPyramid {
    fn default() -> Self {
        Self::new()
    }
}

impl TestPyramid {
    pub fn new() -> Self {
        Self {
            suite: TestSuite::new("default"),
            coverage_report: None,
        }
    }

    pub fn with_name(name: &str) -> Self {
        Self {
            suite: TestSuite::new(name),
            coverage_report: None,
        }
    }

    pub fn register_test(&mut self, test: TestCase) {
        self.suite.register(test);
    }

    pub fn set_coverage_report(&mut self, report: CoverageReport) {
        self.coverage_report = Some(report);
    }

    pub fn get_coverage(&self, module: &str) -> Option<f64> {
        self.coverage_report
            .as_ref()
            .and_then(|r| r.module_coverages.get(module))
            .map(|c| c.overall_coverage_percent())
    }

    /// Check if the pyramid is balanced (proper distribution across levels)
    pub fn check_balance(&self) -> HashMap<TestLevel, f64> {
        let total = self.suite.total_count() as f64;
        if total == 0.0 {
            return HashMap::new();
        }

        let mut balance = HashMap::new();
        for (level, tests) in &self.suite.by_level {
            let percent = (tests.len() as f64 / total) * 100.0;
            balance.insert(*level, percent);
        }
        balance
    }

    /// Recommended distribution for a healthy pyramid
    pub fn recommended_distribution() -> HashMap<TestLevel, (f64, f64)> {
        let mut dist = HashMap::new();
        dist.insert(TestLevel::Unit, (60.0, 80.0));          // 60-80% unit tests
        dist.insert(TestLevel::Contract, (10.0, 20.0));      // 10-20% contract tests
        dist.insert(TestLevel::Integration, (5.0, 15.0));    // 5-15% integration tests
        dist.insert(TestLevel::E2E, (2.0, 10.0));           // 2-10% E2E tests
        dist.insert(TestLevel::Chaos, (0.0, 5.0));           // 0-5% chaos tests
        dist
    }

    /// Check if the test distribution is healthy
    pub fn is_balanced(&self) -> bool {
        let balance = self.check_balance();
        let recommended = Self::recommended_distribution();

        for (level, (min, max)) in recommended {
            if let Some(actual) = balance.get(&level) {
                if *actual < min || *actual > max {
                    return false;
                }
            }
        }
        true
    }

    /// Generate a summary report
    pub fn summary(&self) -> String {
        let total = self.suite.total_count();
        let balance = self.check_balance();
        let recommended = Self::recommended_distribution();

        let mut report = format!(
            "Test Pyramid Summary: {}\nTotal Tests: {}\n\n",
            self.suite.name, total
        );

        report += "Level Distribution:\n";
        for level in [
            TestLevel::Unit,
            TestLevel::Contract,
            TestLevel::Integration,
            TestLevel::E2E,
            TestLevel::Chaos,
        ] {
            let count = self.suite.by_level.get(&level).map(|v| v.len()).unwrap_or(0);
            let percent = balance.get(&level).copied().unwrap_or(0.0);
            let (min, max) = recommended.get(&level).copied().unwrap_or((0.0, 100.0));
            let status = if percent >= min && percent <= max { "✓" } else { "✗" };
            report += &format!(
                "  {}: {} tests ({:.1}%) [recommended: {:.0}-{:.0}%] {}\n",
                level, count, percent, min, max, status
            );
        }

        if let Some(coverage) = &self.coverage_report {
            report += &format!("\nCoverage Report:\n");
            report += &format!(
                "  Overall: {:.1}%\n",
                coverage.overall_coverage()
            );
            report += &format!(
                "  Lines: {:.1}%\n",
                coverage.overall_line_coverage()
            );
        }

        report += &format!("\nBalanced: {}", if self.is_balanced() { "Yes" } else { "No" });
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_level_ordering() {
        assert!(TestLevel::Unit < TestLevel::Contract);
        assert!(TestLevel::Contract < TestLevel::Integration);
        assert!(TestLevel::Integration < TestLevel::E2E);
        assert!(TestLevel::E2E < TestLevel::Chaos);
    }

    #[test]
    fn test_test_level_display() {
        assert_eq!(TestLevel::Unit.to_string(), "Unit");
        assert_eq!(TestLevel::Contract.to_string(), "Contract");
        assert_eq!(TestLevel::Integration.to_string(), "Integration");
        assert_eq!(TestLevel::E2E.to_string(), "E2E");
        assert_eq!(TestLevel::Chaos.to_string(), "Chaos");
    }

    #[test]
    fn test_test_case_creation() {
        let test = TestCase::new("t1", "My Test", TestLevel::Unit, "module1");
        assert_eq!(test.id, "t1");
        assert_eq!(test.name, "My Test");
        assert_eq!(test.level, TestLevel::Unit);
        assert_eq!(test.module, "module1");
        assert!(test.enabled);
    }

    #[test]
    fn test_test_case_with_tags() {
        let test = TestCase::new("t1", "My Test", TestLevel::Unit, "module1")
            .with_tags(vec!["fast", "critical"]);
        assert_eq!(test.tags, vec!["fast", "critical"]);
    }

    #[test]
    fn test_module_coverage_empty() {
        let cov = ModuleCoverage::new("test");
        assert_eq!(cov.line_coverage_percent(), 100.0);
        assert_eq!(cov.branch_coverage_percent(), 100.0);
        assert_eq!(cov.function_coverage_percent(), 100.0);
    }

    #[test]
    fn test_module_coverage_calculations() {
        let mut cov = ModuleCoverage::new("test");
        cov.lines_covered = 80;
        cov.lines_total = 100;
        cov.branches_covered = 15;
        cov.branches_total = 20;
        cov.functions_covered = 8;
        cov.functions_total = 10;

        assert!((cov.line_coverage_percent() - 80.0).abs() < 0.01);
        assert!((cov.branch_coverage_percent() - 75.0).abs() < 0.01);
        assert!((cov.function_coverage_percent() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_test_suite_registration() {
        let mut suite = TestSuite::new("test");
        suite.register(TestCase::new("t1", "Test 1", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t2", "Test 2", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t3", "Test 3", TestLevel::Integration, "mod2"));

        assert_eq!(suite.total_count(), 3);
        assert_eq!(suite.by_level(&TestLevel::Unit).unwrap().len(), 2);
        assert_eq!(suite.by_level(&TestLevel::Integration).unwrap().len(), 1);
        assert_eq!(suite.by_module("mod1").unwrap().len(), 2);
    }

    #[test]
    fn test_level_counts() {
        let mut suite = TestSuite::new("test");
        suite.register(TestCase::new("t1", "Test 1", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t2", "Test 2", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t3", "Test 3", TestLevel::Chaos, "mod2"));

        let counts = suite.level_counts();
        assert_eq!(counts[&TestLevel::Unit], 2);
        assert_eq!(counts[&TestLevel::Chaos], 1);
    }

    #[test]
    fn test_test_pyramid_creation() {
        let pyramid = TestPyramid::new();
        assert_eq!(pyramid.suite.name, "default");
        assert!(pyramid.coverage_report.is_none());
    }

    #[test]
    fn test_test_pyramid_with_name() {
        let pyramid = TestPyramid::with_name("custom");
        assert_eq!(pyramid.suite.name, "custom");
    }

    #[test]
    fn test_register_and_get_coverage() {
        let mut pyramid = TestPyramid::new();
        pyramid.register_test(TestCase::new("t1", "Test 1", TestLevel::Unit, "mod1"));

        let mut report = CoverageReport::new();
        let mut cov = ModuleCoverage::new("mod1");
        cov.lines_covered = 50;
        cov.lines_total = 100;
        report.add_module(cov);
        pyramid.set_coverage_report(report);

        assert!((pyramid.get_coverage("mod1").unwrap() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_check_balance() {
        let mut pyramid = TestPyramid::new();
        for i in 0..70 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Unit, "mod1"));
        }
        for i in 70..85 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Contract, "mod1"));
        }
        for i in 85..100 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Integration, "mod1"));
        }

        let balance = pyramid.check_balance();
        // With 70 unit, 15 contract, 15 integration = 100 total
        // Unit: 70%, Contract: 15%, Integration: 15%
        assert!((balance[&TestLevel::Unit] - 70.0).abs() < 0.1);
        assert!((balance[&TestLevel::Contract] - 15.0).abs() < 0.1);
        assert!((balance[&TestLevel::Integration] - 15.0).abs() < 0.1);
    }

    #[test]
    fn test_is_balanced_when_healthy() {
        let mut pyramid = TestPyramid::new();
        // 70% Unit
        for i in 0..70 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Unit, "mod1"));
        }
        // 15% Contract
        for i in 70..85 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Contract, "mod1"));
        }
        // 10% Integration
        for i in 85..95 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::Integration, "mod1"));
        }
        // 5% E2E
        for i in 95..100 {
            pyramid.register_test(TestCase::new(&format!("t{}", i), &format!("Test {}", i), TestLevel::E2E, "mod1"));
        }

        assert!(pyramid.is_balanced());
    }

    #[test]
    fn test_recommended_distribution() {
        let dist = TestPyramid::recommended_distribution();
        assert_eq!(dist[&TestLevel::Unit], (60.0, 80.0));
        assert_eq!(dist[&TestLevel::Contract], (10.0, 20.0));
        assert_eq!(dist[&TestLevel::Integration], (5.0, 15.0));
        assert_eq!(dist[&TestLevel::E2E], (2.0, 10.0));
        assert_eq!(dist[&TestLevel::Chaos], (0.0, 5.0));
    }

    #[test]
    fn test_coverage_report_aggregation() {
        let mut report = CoverageReport::new();
        let mut cov1 = ModuleCoverage::new("mod1");
        cov1.lines_covered = 50;
        cov1.lines_total = 100;
        report.add_module(cov1);

        let mut cov2 = ModuleCoverage::new("mod2");
        cov2.lines_covered = 80;
        cov2.lines_total = 100;
        report.add_module(cov2);

        // Total: 130/200 = 65%
        assert!((report.overall_line_coverage() - 65.0).abs() < 0.01);
    }

    #[test]
    fn test_test_matrix_creation() {
        let mut suite = TestSuite::new("test");
        suite.register(TestCase::new("t1", "Test 1", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t2", "Test 2", TestLevel::Unit, "mod2"));
        suite.register(TestCase::new("t3", "Test 3", TestLevel::Integration, "mod1"));

        let coverages: HashMap<String, f64> = HashMap::new();
        let matrix = TestMatrix::from_suite(&suite, &coverages);

        assert_eq!(matrix.levels.len(), 2);
        assert!(matrix.modules.contains(&"mod1".to_string()));
        assert!(matrix.modules.contains(&"mod2".to_string()));
    }

    #[test]
    fn test_test_matrix_total() {
        let mut suite = TestSuite::new("test");
        suite.register(TestCase::new("t1", "Test 1", TestLevel::Unit, "mod1"));
        suite.register(TestCase::new("t2", "Test 2", TestLevel::Unit, "mod2"));
        suite.register(TestCase::new("t3", "Test 3", TestLevel::Integration, "mod1"));

        let coverages: HashMap<String, f64> = HashMap::new();
        let matrix = TestMatrix::from_suite(&suite, &coverages);

        assert_eq!(matrix.total_tests(), 3);
    }

    #[test]
    fn test_summary_generation() {
        let pyramid = TestPyramid::with_name("my-pyramid");
        let summary = pyramid.summary();
        assert!(summary.contains("my-pyramid"));
        assert!(summary.contains("Total Tests: 0"));
    }
}
