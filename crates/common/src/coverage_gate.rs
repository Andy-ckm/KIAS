//! # Coverage Gate - Module-Level Coverage Thresholds
//!
//! Implements coverage gates with per-module thresholds, trend tracking,
//! and automated pass/fail decisions for CI/CD integration.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use chrono::{DateTime, Utc, Duration};

/// Coverage threshold configuration for a module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageThreshold {
    pub module: String,
    pub line_threshold: f64,
    pub branch_threshold: f64,
    pub function_threshold: f64,
    pub overall_threshold: f64,
}

impl CoverageThreshold {
    pub fn new(module: &str, overall: f64) -> Self {
        Self {
            module: module.to_string(),
            line_threshold: overall * 0.9,      // 90% of overall
            branch_threshold: overall * 0.85,   // 85% of overall
            function_threshold: overall * 0.95,  // 95% of overall
            overall_threshold: overall,
        }
    }

    pub fn with_custom(
        module: &str,
        line: f64,
        branch: f64,
        function: f64,
        overall: f64,
    ) -> Self {
        Self {
            module: module.to_string(),
            line_threshold: line,
            branch_threshold: branch,
            function_threshold: function,
            overall_threshold: overall,
        }
    }

    pub fn strict(module: &str) -> Self {
        Self::with_custom(module, 80.0, 75.0, 85.0, 80.0)
    }

    pub fn moderate(module: &str) -> Self {
        Self::with_custom(module, 60.0, 55.0, 70.0, 65.0)
    }

    pub fn lenient(module: &str) -> Self {
        Self::with_custom(module, 40.0, 35.0, 50.0, 45.0)
    }
}

/// Coverage measurement for a single module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMeasurement {
    pub module: String,
    pub line_coverage: f64,
    pub branch_coverage: f64,
    pub function_coverage: f64,
    pub measured_at: DateTime<Utc>,
}

impl CoverageMeasurement {
    pub fn overall(&self) -> f64 {
        (self.line_coverage + self.branch_coverage + self.function_coverage) / 3.0
    }
}

/// Result of checking coverage against a threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageCheckResult {
    pub module: String,
    pub passed: bool,
    pub line_ok: bool,
    pub branch_ok: bool,
    pub function_ok: bool,
    pub overall_ok: bool,
    pub line_actual: f64,
    pub branch_actual: f64,
    pub function_actual: f64,
    pub overall_actual: f64,
    pub line_threshold: f64,
    pub branch_threshold: f64,
    pub function_threshold: f64,
    pub overall_threshold: f64,
}

impl CoverageCheckResult {
    pub fn pass(module: &str, measurement: &CoverageMeasurement, threshold: &CoverageThreshold) -> Self {
        Self {
            module: module.to_string(),
            passed: true,
            line_ok: true,
            branch_ok: true,
            function_ok: true,
            overall_ok: true,
            line_actual: measurement.line_coverage,
            branch_actual: measurement.branch_coverage,
            function_actual: measurement.function_coverage,
            overall_actual: measurement.overall(),
            line_threshold: threshold.line_threshold,
            branch_threshold: threshold.branch_threshold,
            function_threshold: threshold.function_threshold,
            overall_threshold: threshold.overall_threshold,
        }
    }

    pub fn fail(module: &str, measurement: &CoverageMeasurement, threshold: &CoverageThreshold) -> Self {
        let overall = measurement.overall();
        Self {
            module: module.to_string(),
            passed: false,
            line_ok: measurement.line_coverage >= threshold.line_threshold,
            branch_ok: measurement.branch_coverage >= threshold.branch_threshold,
            function_ok: measurement.function_coverage >= threshold.function_threshold,
            overall_ok: overall >= threshold.overall_threshold,
            line_actual: measurement.line_coverage,
            branch_actual: measurement.branch_coverage,
            function_actual: measurement.function_coverage,
            overall_actual: overall,
            line_threshold: threshold.line_threshold,
            branch_threshold: threshold.branch_threshold,
            function_threshold: threshold.function_threshold,
            overall_threshold: threshold.overall_threshold,
        }
    }

    pub fn failures(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if !self.line_ok {
            failures.push(format!(
                "Line coverage {:.1}% < threshold {:.1}%",
                self.line_actual, self.line_threshold
            ));
        }
        if !self.branch_ok {
            failures.push(format!(
                "Branch coverage {:.1}% < threshold {:.1}%",
                self.branch_actual, self.branch_threshold
            ));
        }
        if !self.function_ok {
            failures.push(format!(
                "Function coverage {:.1}% < threshold {:.1}%",
                self.function_actual, self.function_threshold
            ));
        }
        if !self.overall_ok {
            failures.push(format!(
                "Overall coverage {:.1}% < threshold {:.1}%",
                self.overall_actual, self.overall_threshold
            ));
        }
        failures
    }
}

/// Coverage trend data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageTrendPoint {
    pub timestamp: DateTime<Utc>,
    pub overall: f64,
    pub line: f64,
    pub branch: f64,
    pub function: f64,
}

/// Coverage trend tracking for a module
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageTrend {
    pub module: String,
    pub points: Vec<CoverageTrendPoint>,
    pub window_size: usize,
}

impl CoverageTrend {
    pub fn new(module: &str, window_size: usize) -> Self {
        Self {
            module: module.to_string(),
            points: Vec::new(),
            window_size,
        }
    }

    pub fn add_point(&mut self, measurement: &CoverageMeasurement) {
        self.points.push(CoverageTrendPoint {
            timestamp: measurement.measured_at,
            overall: measurement.overall(),
            line: measurement.line_coverage,
            branch: measurement.branch_coverage,
            function: measurement.function_coverage,
        });

        // Keep only the most recent window_size points
        if self.points.len() > self.window_size {
            self.points.remove(0);
        }
    }

    pub fn slope(&self) -> Option<f64> {
        if self.points.len() < 2 {
            return None;
        }

        // Simple linear regression slope
        let n = self.points.len() as f64;
        let sum_x = (0..self.points.len()).map(|i| i as f64).sum::<f64>();
        let sum_y = self.points.iter().map(|p| p.overall).sum::<f64>();
        let sum_xy = self.points.iter().enumerate().map(|(i, p)| i as f64 * p.overall).sum::<f64>();
        let sum_x2 = self.points.iter().enumerate().map(|(i, _)| (i as f64) * (i as f64)).sum::<f64>();

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < f64::EPSILON {
            return None;
        }

        Some((n * sum_xy - sum_x * sum_y) / denominator)
    }

    pub fn direction(&self) -> Option<&'static str> {
        self.slope().map(|s| {
            if s > 0.1 { "improving" }
            else if s < -0.1 { "declining" }
            else { "stable" }
        })
    }

    pub fn average(&self) -> Option<f64> {
        if self.points.is_empty() {
            return None;
        }
        Some(self.points.iter().map(|p| p.overall).sum::<f64>() / self.points.len() as f64)
    }
}

/// Coverage checker that validates measurements against thresholds
#[derive(Debug, Clone)]
pub struct CoverageChecker {
    thresholds: BTreeMap<String, CoverageThreshold>,
    trends: BTreeMap<String, CoverageTrend>,
}

impl Default for CoverageChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageChecker {
    pub fn new() -> Self {
        Self {
            thresholds: BTreeMap::new(),
            trends: BTreeMap::new(),
        }
    }

    pub fn add_threshold(&mut self, threshold: CoverageThreshold) {
        self.thresholds.insert(threshold.module.clone(), threshold);
    }

    pub fn set_threshold(&mut self, module: &str, overall: f64) {
        self.thresholds.insert(module.to_string(), CoverageThreshold::new(module, overall));
    }

    pub fn get_threshold(&self, module: &str) -> Option<&CoverageThreshold> {
        self.thresholds.get(module)
    }

    pub fn check(&self, measurement: &CoverageMeasurement) -> CoverageCheckResult {
        if let Some(threshold) = self.thresholds.get(&measurement.module) {
            if measurement.overall() >= threshold.overall_threshold
                && measurement.line_coverage >= threshold.line_threshold
                && measurement.branch_coverage >= threshold.branch_threshold
                && measurement.function_coverage >= threshold.function_threshold
            {
                CoverageCheckResult::pass(&measurement.module, measurement, threshold)
            } else {
                CoverageCheckResult::fail(&measurement.module, measurement, threshold)
            }
        } else {
            // No threshold defined - auto-pass with default 70%
            let default = CoverageThreshold::moderate(&measurement.module);
            if measurement.overall() >= default.overall_threshold {
                CoverageCheckResult::pass(&measurement.module, measurement, &default)
            } else {
                CoverageCheckResult::fail(&measurement.module, measurement, &default)
            }
        }
    }

    pub fn check_all(&self, measurements: &[CoverageMeasurement]) -> Vec<CoverageCheckResult> {
        measurements.iter().map(|m| self.check(m)).collect()
    }

    pub fn add_trend_point(&mut self, measurement: &CoverageMeasurement) {
        let module = &measurement.module;
        if !self.trends.contains_key(module) {
            self.trends.insert(module.clone(), CoverageTrend::new(module, 10));
        }
        if let Some(trend) = self.trends.get_mut(module) {
            trend.add_point(measurement);
        }
    }

    pub fn get_trend(&self, module: &str) -> Option<&CoverageTrend> {
        self.trends.get(module)
    }

    pub fn get_all_trends(&self) -> &BTreeMap<String, CoverageTrend> {
        &self.trends
    }
}

/// Coverage gate that combines thresholds and checker
#[derive(Debug, Clone)]
pub struct CoverageGate {
    checker: CoverageChecker,
    strict_mode: bool,
}

impl Default for CoverageGate {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageGate {
    pub fn new() -> Self {
        Self {
            checker: CoverageChecker::new(),
            strict_mode: false,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn add_threshold(&mut self, threshold: CoverageThreshold) {
        self.checker.add_threshold(threshold);
    }

    pub fn set_threshold(&mut self, module: &str, overall: f64) {
        self.checker.set_threshold(module, overall);
    }

    pub fn check(&self, measurement: &CoverageMeasurement) -> CoverageCheckResult {
        self.checker.check(measurement)
    }

    pub fn gate_pass(&self, measurements: &[CoverageMeasurement]) -> bool {
        let results = self.checker.check_all(measurements);
        if self.strict_mode {
            results.iter().all(|r| r.passed)
        } else {
            results.iter().filter(|r| !r.passed).count() <= 1
        }
    }

    pub fn generate_report(&self, measurements: &[CoverageMeasurement]) -> String {
        let results = self.checker.check_all(measurements);
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.iter().filter(|r| !r.passed).count();

        let mut report = format!(
            "Coverage Gate Report\n==================\nTotal: {} | Passed: {} | Failed: {}\n\n",
            results.len(),
            passed,
            failed
        );

        for result in &results {
            let status = if result.passed { "✓ PASS" } else { "✗ FAIL" };
            report += &format!("{}: {}\n", result.module, status);
            if !result.passed {
                for failure in result.failures() {
                    report += &format!("  - {}\n", failure);
                }
            }
        }

        report
    }

    /// Initialize gate with standard thresholds for common modules
    pub fn init_standard(&mut self) {
        self.set_threshold("common", 80.0);
        self.set_threshold("scheduler", 75.0);
        self.set_threshold("controller", 75.0);
        self.set_threshold("api-server", 70.0);
        self.set_threshold("monitor", 70.0);
        self.set_threshold("knowledge", 65.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_measurement(module: &str, line: f64, branch: f64, function: f64) -> CoverageMeasurement {
        CoverageMeasurement {
            module: module.to_string(),
            line_coverage: line,
            branch_coverage: branch,
            function_coverage: function,
            measured_at: Utc::now(),
        }
    }

    #[test]
    fn test_threshold_default_creation() {
        let t = CoverageThreshold::new("test", 80.0);
        assert_eq!(t.module, "test");
        assert_eq!(t.overall_threshold, 80.0);
        assert!((t.line_threshold - 72.0).abs() < 0.01); // 90% of 80
        assert!((t.branch_threshold - 68.0).abs() < 0.01); // 85% of 80
        assert!((t.function_threshold - 76.0).abs() < 0.01); // 95% of 80
    }

    #[test]
    fn test_threshold_custom_creation() {
        let t = CoverageThreshold::with_custom("test", 70.0, 65.0, 80.0, 75.0);
        assert_eq!(t.line_threshold, 70.0);
        assert_eq!(t.branch_threshold, 65.0);
        assert_eq!(t.function_threshold, 80.0);
        assert_eq!(t.overall_threshold, 75.0);
    }

    #[test]
    fn test_threshold_strict() {
        let t = CoverageThreshold::strict("test");
        assert_eq!(t.overall_threshold, 80.0);
        assert_eq!(t.line_threshold, 80.0);
    }

    #[test]
    fn test_threshold_moderate() {
        let t = CoverageThreshold::moderate("test");
        assert_eq!(t.overall_threshold, 65.0);
    }

    #[test]
    fn test_threshold_lenient() {
        let t = CoverageThreshold::lenient("test");
        assert_eq!(t.overall_threshold, 45.0);
    }

    #[test]
    fn test_coverage_measurement_overall() {
        let m = make_measurement("test", 80.0, 70.0, 90.0);
        assert!((m.overall() - 80.0).abs() < 0.01); // (80+70+90)/3 = 80
    }

    #[test]
    fn test_coverage_check_result_pass() {
        let m = make_measurement("test", 90.0, 85.0, 95.0);
        let t = CoverageThreshold::new("test", 80.0);
        let result = CoverageCheckResult::pass("test", &m, &t);
        assert!(result.passed);
        assert!(result.line_ok);
        assert!(result.branch_ok);
        assert!(result.function_ok);
        assert!(result.overall_ok);
    }

    #[test]
    fn test_coverage_check_result_fail() {
        let m = make_measurement("test", 50.0, 40.0, 60.0);
        let t = CoverageThreshold::new("test", 80.0);
        let result = CoverageCheckResult::fail("test", &m, &t);
        assert!(!result.passed);
        assert!(!result.line_ok);
        assert!(!result.branch_ok);
        assert!(!result.function_ok);
        assert!(!result.overall_ok);
    }

    #[test]
    fn test_coverage_check_result_failures() {
        let m = make_measurement("test", 50.0, 40.0, 60.0);
        let t = CoverageThreshold::new("test", 80.0);
        let result = CoverageCheckResult::fail("test", &m, &t);
        let failures = result.failures();
        assert!(!failures.is_empty());
        assert!(failures.len() >= 3);
    }

    #[test]
    fn test_coverage_trend_new() {
        let trend = CoverageTrend::new("test", 5);
        assert_eq!(trend.module, "test");
        assert!(trend.points.is_empty());
        assert_eq!(trend.window_size, 5);
    }

    #[test]
    fn test_coverage_trend_add_point() {
        let mut trend = CoverageTrend::new("test", 3);
        trend.add_point(&make_measurement("test", 70.0, 65.0, 75.0));
        trend.add_point(&make_measurement("test", 75.0, 70.0, 80.0));
        assert_eq!(trend.points.len(), 2);
    }

    #[test]
    fn test_coverage_trend_window_enforcement() {
        let mut trend = CoverageTrend::new("test", 3);
        for i in 0..5 {
            trend.add_point(&make_measurement("test", 70.0 + i as f64, 65.0, 75.0));
        }
        assert_eq!(trend.points.len(), 3); // Only keeps last 3
    }

    #[test]
    fn test_coverage_trend_slope() {
        let mut trend = CoverageTrend::new("test", 10);
        trend.add_point(&make_measurement("test", 60.0, 60.0, 60.0));
        trend.add_point(&make_measurement("test", 70.0, 70.0, 70.0));
        trend.add_point(&make_measurement("test", 80.0, 80.0, 80.0));
        let slope = trend.slope();
        assert!(slope.is_some());
        assert!(slope.unwrap() > 0.0); // Should be positive (improving)
    }

    #[test]
    fn test_coverage_trend_direction() {
        let mut trend = CoverageTrend::new("test", 10);
        trend.add_point(&make_measurement("test", 60.0, 60.0, 60.0));
        trend.add_point(&make_measurement("test", 70.0, 70.0, 70.0));
        let dir = trend.direction();
        assert!(dir.is_some());
        assert_eq!(dir.unwrap(), "improving");
    }

    #[test]
    fn test_coverage_trend_average() {
        let mut trend = CoverageTrend::new("test", 10);
        trend.add_point(&make_measurement("test", 60.0, 60.0, 60.0));
        trend.add_point(&make_measurement("test", 80.0, 80.0, 80.0));
        let avg = trend.average();
        assert!(avg.is_some());
        assert!((avg.unwrap() - 70.0).abs() < 0.01);
    }

    #[test]
    fn test_coverage_checker_new() {
        let checker = CoverageChecker::new();
        assert!(checker.get_threshold("nonexistent").is_none());
    }

    #[test]
    fn test_coverage_checker_add_threshold() {
        let mut checker = CoverageChecker::new();
        checker.add_threshold(CoverageThreshold::new("test", 80.0));
        assert!(checker.get_threshold("test").is_some());
        assert_eq!(checker.get_threshold("test").unwrap().overall_threshold, 80.0);
    }

    #[test]
    fn test_coverage_checker_check_pass() {
        let mut checker = CoverageChecker::new();
        checker.add_threshold(CoverageThreshold::new("test", 60.0));
        let m = make_measurement("test", 80.0, 75.0, 85.0);
        let result = checker.check(&m);
        assert!(result.passed);
    }

    #[test]
    fn test_coverage_checker_check_fail() {
        let mut checker = CoverageChecker::new();
        checker.add_threshold(CoverageThreshold::new("test", 80.0));
        let m = make_measurement("test", 50.0, 40.0, 60.0);
        let result = checker.check(&m);
        assert!(!result.passed);
    }

    #[test]
    fn test_coverage_checker_check_all() {
        let mut checker = CoverageChecker::new();
        checker.add_threshold(CoverageThreshold::new("test1", 80.0));
        checker.add_threshold(CoverageThreshold::new("test2", 80.0));
        let measurements = vec![
            make_measurement("test1", 90.0, 85.0, 95.0),
            make_measurement("test2", 50.0, 40.0, 60.0),
        ];
        let results = checker.check_all(&measurements);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(!results[1].passed);
    }

    #[test]
    fn test_coverage_gate_gate_pass() {
        let mut gate = CoverageGate::new();
        gate.set_threshold("test", 80.0);
        let measurements = vec![
            make_measurement("test", 90.0, 85.0, 95.0),
        ];
        assert!(gate.gate_pass(&measurements));
    }

    #[test]
    fn test_coverage_gate_gate_fail() {
        let mut gate = CoverageGate::new();
        gate.set_threshold("test", 80.0);
        let measurements = vec![
            make_measurement("test", 50.0, 40.0, 60.0),
        ];
        assert!(!gate.gate_pass(&measurements));
    }

    #[test]
    fn test_coverage_gate_strict_mode() {
        let mut gate = CoverageGate::new().with_strict_mode(true);
        gate.set_threshold("test", 80.0);
        let measurements = vec![
            make_measurement("test", 90.0, 85.0, 95.0),
            make_measurement("test2", 50.0, 40.0, 60.0), // No threshold but fails
        ];
        // Strict mode: all must pass
        assert!(!gate.gate_pass(&measurements));
    }

    #[test]
    fn test_coverage_gate_init_standard() {
        let mut gate = CoverageGate::new();
        gate.init_standard();
        assert!(gate.checker.get_threshold("common").is_some());
        assert!(gate.checker.get_threshold("scheduler").is_some());
        assert_eq!(gate.checker.get_threshold("common").unwrap().overall_threshold, 80.0);
    }

    #[test]
    fn test_coverage_gate_generate_report() {
        let mut gate = CoverageGate::new();
        gate.set_threshold("test", 80.0);
        let measurements = vec![
            make_measurement("test", 90.0, 85.0, 95.0),
        ];
        let report = gate.generate_report(&measurements);
        assert!(report.contains("Coverage Gate Report"));
        assert!(report.contains("PASS"));
    }
}
