//! # AgentSight - Cost Analytics & Observability
//!
//! Provides cost attribution, token tracking, and explainable cost analytics.

pub mod cost_model;
pub mod roi_dashboard;

pub use cost_model::{
    CostAggregator, CostAlert, CostBreakdown, CostDimension, CostRecord, CostSummary,
    ExceedanceTier, TenantCostSnapshot, TimeWindow,
};

pub use roi_dashboard::{
    BenchmarkComparison, DashboardDataPoint, ROICalculation, ROICalculator, ROIDashboard,
    ROIMetric, ROIMetricType, ROISummary,
};
