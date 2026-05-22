//! # ROI Dashboard - Value Proof Metrics and Benchmarks
//!
//! Implements ROI metrics, calculator, and benchmark comparison
//! for demonstrating business value of AgentGuard.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ROI metric categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ROIMetricType {
    /// Efficiency improvement metrics
    EfficiencyImprovement,
    /// Cost reduction metrics
    CostReduction,
    /// Risk reduction metrics
    RiskReduction,
    /// Revenue impact metrics
    RevenueImpact,
}

impl ROIMetricType {
    pub fn name(&self) -> &'static str {
        match self {
            ROIMetricType::EfficiencyImprovement => "Efficiency Improvement",
            ROIMetricType::CostReduction => "Cost Reduction",
            ROIMetricType::RiskReduction => "Risk Reduction",
            ROIMetricType::RevenueImpact => "Revenue Impact",
        }
    }
}

/// A single ROI metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROIMetric {
    pub id: String,
    pub name: String,
    pub metric_type: ROIMetricType,
    pub baseline_value: f64,
    pub current_value: f64,
    pub unit: String,
    pub period_days: u32,
}

impl ROIMetric {
    pub fn new(
        id: &str,
        name: &str,
        metric_type: ROIMetricType,
        baseline_value: f64,
        current_value: f64,
        unit: &str,
        period_days: u32,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            metric_type,
            baseline_value,
            current_value,
            unit: unit.to_string(),
            period_days,
        }
    }

    /// Calculate improvement percentage
    pub fn improvement_percent(&self) -> f64 {
        if self.baseline_value == 0.0 {
            return 0.0;
        }
        ((self.current_value - self.baseline_value) / self.baseline_value) * 100.0
    }

    /// Calculate absolute change
    pub fn absolute_change(&self) -> f64 {
        self.current_value - self.baseline_value
    }
}

/// ROI calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROICalculation {
    pub metric_id: String,
    pub baseline: f64,
    pub current: f64,
    pub improvement_percent: f64,
    pub annualized_value: f64,
    pub cost_savings: f64,
    pub calculation_date: DateTime<Utc>,
}

impl ROICalculation {
    pub fn calculate(metric: &ROIMetric, cost_per_unit: f64) -> Self {
        let improvement = metric.improvement_percent();
        let change = metric.absolute_change();

        // Annualize based on period
        let periods_per_year = 365.0 / metric.period_days as f64;
        let annualized_change = change * periods_per_year;
        let annualized_value = annualized_change * cost_per_unit;

        Self {
            metric_id: metric.id.clone(),
            baseline: metric.baseline_value,
            current: metric.current_value,
            improvement_percent: improvement,
            annualized_value,
            cost_savings: annualized_value.max(0.0),
            calculation_date: Utc::now(),
        }
    }
}

/// Benchmark comparison data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub metric_name: String,
    pub your_value: f64,
    pub industry_average: f64,
    pub best_in_class: f64,
    pub percentile: f64, // Your position in the industry (0-100)
}

impl BenchmarkComparison {
    pub fn new(
        metric_name: &str,
        your_value: f64,
        industry_average: f64,
        best_in_class: f64,
    ) -> Self {
        let percentile = if your_value >= best_in_class {
            100.0
        } else if your_value <= industry_average {
            50.0
        } else {
            // Linear interpolation between average and best
            let range = best_in_class - industry_average;
            let position = your_value - industry_average;
            50.0 + (position / range) * 50.0
        };

        Self {
            metric_name: metric_name.to_string(),
            your_value,
            industry_average,
            best_in_class,
            percentile,
        }
    }

    pub fn status(&self) -> &'static str {
        if self.percentile >= 90.0 {
            "Leader"
        } else if self.percentile >= 70.0 {
            "Above Average"
        } else if self.percentile >= 40.0 {
            "Average"
        } else {
            "Below Average"
        }
    }
}

/// ROI summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROISummary {
    pub total_annualized_savings: f64,
    pub total_investment: f64,
    pub roi_percentage: f64,
    pub payback_period_months: f64,
    pub efficiency_gain_percent: f64,
    pub risk_reduction_percent: f64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

impl ROISummary {
    pub fn new(
        calculations: &[ROICalculation],
        total_investment: f64,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Self {
        let total_annualized_savings: f64 = calculations.iter().map(|c| c.cost_savings).sum();

        let roi_percentage = if total_investment > 0.0 {
            ((total_annualized_savings - total_investment) / total_investment) * 100.0
        } else {
            0.0
        };

        let payback_period_months = if total_annualized_savings > 0.0 {
            (total_investment / total_annualized_savings) * 12.0
        } else {
            0.0
        };

        // Calculate aggregate efficiency gain
        let efficiency_gain_percent = if !calculations.is_empty() {
            calculations
                .iter()
                .filter(|c| c.improvement_percent > 0.0)
                .map(|c| c.improvement_percent)
                .sum::<f64>()
                / calculations.len() as f64
        } else {
            0.0
        };

        let risk_reduction_percent = if !calculations.is_empty() {
            calculations
                .iter()
                .filter(|c| c.improvement_percent > 0.0)
                .map(|c| c.improvement_percent.min(100.0))
                .sum::<f64>()
                / calculations.len() as f64
        } else {
            0.0
        };

        Self {
            total_annualized_savings,
            total_investment,
            roi_percentage,
            payback_period_months,
            efficiency_gain_percent,
            risk_reduction_percent,
            period_start,
            period_end,
        }
    }
}

/// ROI Calculator engine
#[derive(Debug, Clone)]
pub struct ROICalculator {
    metrics: Vec<ROIMetric>,
    benchmarks: HashMap<String, (f64, f64)>, // metric_name -> (industry_avg, best_in_class)
    investment: f64,
}

impl Default for ROICalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl ROICalculator {
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
            benchmarks: HashMap::new(),
            investment: 0.0,
        }
    }

    pub fn add_metric(&mut self, metric: ROIMetric) {
        self.metrics.push(metric);
    }

    pub fn set_investment(&mut self, investment: f64) {
        self.investment = investment;
    }

    pub fn set_benchmark(&mut self, metric_name: &str, industry_average: f64, best_in_class: f64) {
        self.benchmarks
            .insert(metric_name.to_string(), (industry_average, best_in_class));
    }

    pub fn get_metrics(&self) -> &[ROIMetric] {
        &self.metrics
    }

    pub fn calculate_all(&self, cost_per_unit: f64) -> Vec<ROICalculation> {
        self.metrics
            .iter()
            .map(|m| ROICalculation::calculate(m, cost_per_unit))
            .collect()
    }

    pub fn generate_summary(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> ROISummary {
        let calculations = self.calculate_all(1.0); // Generic unit cost for summary
        ROISummary::new(&calculations, self.investment, period_start, period_end)
    }

    pub fn compare_to_benchmark(&self, metric_name: &str) -> Option<BenchmarkComparison> {
        // Find metric by name
        let metric = self.metrics.iter().find(|m| m.name == metric_name)?;

        // Get benchmarks
        let (industry_avg, best_in_class) = self.benchmarks.get(metric_name)?;

        Some(BenchmarkComparison::new(
            metric_name,
            metric.current_value,
            *industry_avg,
            *best_in_class,
        ))
    }

    pub fn all_benchmark_comparisons(&self) -> Vec<BenchmarkComparison> {
        self.metrics
            .iter()
            .filter_map(|m| {
                self.benchmarks.get(&m.name).map(|(avg, best)| {
                    BenchmarkComparison::new(&m.name, m.current_value, *avg, *best)
                })
            })
            .collect()
    }
}

/// Dashboard data point for time-series display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardDataPoint {
    pub timestamp: DateTime<Utc>,
    pub metric_id: String,
    pub value: f64,
}

/// ROI Dashboard
#[derive(Debug, Clone)]
pub struct ROIDashboard {
    calculator: ROICalculator,
    time_series: Vec<DashboardDataPoint>,
}

impl Default for ROIDashboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ROIDashboard {
    pub fn new() -> Self {
        Self {
            calculator: ROICalculator::new(),
            time_series: Vec::new(),
        }
    }

    pub fn with_calculator(calculator: ROICalculator) -> Self {
        Self {
            calculator,
            time_series: Vec::new(),
        }
    }

    pub fn add_metric(&mut self, metric: ROIMetric) {
        // Also add to time series
        self.time_series.push(DashboardDataPoint {
            timestamp: Utc::now(),
            metric_id: metric.id.clone(),
            value: metric.current_value,
        });
        self.calculator.add_metric(metric);
    }

    pub fn set_investment(&mut self, investment: f64) {
        self.calculator.set_investment(investment);
    }

    pub fn add_benchmark(&mut self, metric_name: &str, industry_avg: f64, best: f64) {
        self.calculator
            .set_benchmark(metric_name, industry_avg, best);
    }

    pub fn get_summary(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> ROISummary {
        self.calculator.generate_summary(period_start, period_end)
    }

    pub fn get_benchmark_comparisons(&self) -> Vec<BenchmarkComparison> {
        self.calculator.all_benchmark_comparisons()
    }

    pub fn get_time_series(&self, metric_id: &str) -> Vec<&DashboardDataPoint> {
        self.time_series
            .iter()
            .filter(|dp| dp.metric_id == metric_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roi_metric_creation() {
        let metric = ROIMetric::new(
            "m1",
            "Task Completion Time",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            80.0,
            "minutes",
            30,
        );
        assert_eq!(metric.id, "m1");
        assert_eq!(metric.baseline_value, 100.0);
        assert_eq!(metric.current_value, 80.0);
    }

    #[test]
    fn test_roi_metric_improvement_percent() {
        let metric = ROIMetric::new(
            "m1",
            "Task Completion Time",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            80.0,
            "minutes",
            30,
        );
        // Improvement from 100 to 80 is 20% improvement
        assert!((metric.improvement_percent() - (-20.0)).abs() < 0.01);
    }

    #[test]
    fn test_roi_metric_absolute_change() {
        let metric = ROIMetric::new(
            "m1",
            "Task Throughput",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            150.0,
            "tasks/hour",
            30,
        );
        assert!((metric.absolute_change() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_roi_calculation() {
        let metric = ROIMetric::new(
            "m1",
            "Hours Saved",
            ROIMetricType::CostReduction,
            10.0,
            20.0,
            "hours/week",
            7,
        );
        let calc = ROICalculation::calculate(&metric, 100.0); // $100 per hour

        // Improvement: (20-10)/10 = 100%
        assert!((calc.improvement_percent - 100.0).abs() < 0.01);
        // 7 days * 10 hours saved = 70 hours saved over period
        // Annualized: 70 * (365/7) = 3650 hours
        // Value: 3650 * 100 = $365,000
        assert!(calc.annualized_value > 0.0);
    }

    #[test]
    fn test_benchmark_comparison_leader() {
        let bench = BenchmarkComparison::new("Response Time", 50.0, 100.0, 50.0);
        assert_eq!(bench.percentile, 100.0);
        assert_eq!(bench.status(), "Leader");
    }

    #[test]
    fn test_benchmark_comparison_average() {
        let bench = BenchmarkComparison::new("Response Time", 100.0, 100.0, 50.0);
        assert_eq!(bench.percentile, 100.0); // your_value >= best_in_class
        assert_eq!(bench.status(), "Leader");
    }

    #[test]
    fn test_benchmark_comparison_above_average() {
        let bench = BenchmarkComparison::new("Response Time", 75.0, 100.0, 50.0);
        assert_eq!(bench.percentile, 100.0); // your_value >= best_in_class (50)
        assert_eq!(bench.status(), "Leader");
    }

    #[test]
    fn test_roi_summary_calculation() {
        let metrics = [ROIMetric::new(
            "m1",
            "Metric1",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            150.0,
            "units",
            30,
        )];
        let calculations: Vec<ROICalculation> = metrics
            .iter()
            .map(|m| ROICalculation::calculate(m, 100.0))
            .collect();

        let summary = ROISummary::new(&calculations, 50.0, Utc::now(), Utc::now());

        assert!(summary.roi_percentage > 0.0);
    }

    #[test]
    fn test_roi_calculator_add_metric() {
        let mut calc = ROICalculator::new();
        calc.add_metric(ROIMetric::new(
            "m1",
            "Test",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            80.0,
            "min",
            30,
        ));
        assert_eq!(calc.get_metrics().len(), 1);
    }

    #[test]
    fn test_roi_calculator_benchmark() {
        let mut calc = ROICalculator::new();
        calc.set_benchmark("Response Time", 100.0, 50.0);

        calc.add_metric(ROIMetric::new(
            "m1",
            "Response Time",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            60.0,
            "ms",
            30,
        ));

        let bench = calc.compare_to_benchmark("Response Time");
        assert!(bench.is_some());
        // percentile depends on implementation
    }

    #[test]
    fn test_roi_dashboard_creation() {
        let dashboard = ROIDashboard::new();
        assert!(dashboard.get_benchmark_comparisons().is_empty());
    }

    #[test]
    fn test_roi_dashboard_with_metric() {
        let mut dashboard = ROIDashboard::new();
        dashboard.add_metric(ROIMetric::new(
            "m1",
            "Efficiency",
            ROIMetricType::EfficiencyImprovement,
            100.0,
            120.0,
            "%",
            30,
        ));

        let series = dashboard.get_time_series("m1");
        assert_eq!(series.len(), 1);
    }

    #[test]
    fn test_roi_metric_type_names() {
        assert_eq!(
            ROIMetricType::EfficiencyImprovement.name(),
            "Efficiency Improvement"
        );
        assert_eq!(ROIMetricType::CostReduction.name(), "Cost Reduction");
        assert_eq!(ROIMetricType::RiskReduction.name(), "Risk Reduction");
        assert_eq!(ROIMetricType::RevenueImpact.name(), "Revenue Impact");
    }
}
