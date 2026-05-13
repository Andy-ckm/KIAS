use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

/// Performance trend direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Trend {
    Improving,
    Stable,
    Degrading,
}

/// Agent performance profile aggregated over a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    pub agent_id: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: u64,
    pub avg_tokens_per_task: f64,
    pub cost_estimate: f64,
    pub uptime_percent: f64,
    pub error_rate: f64,
    pub throughput_per_minute: f64,
    pub reliability_score: f64,
    pub efficiency_score: f64,
    pub overall_score: f64,
    pub trend: Trend,
    pub recommendations: Vec<String>,
}

/// Calculates performance profiles for agents
pub struct PerformanceAnalyzer {
    /// Token cost per 1K tokens (configurable)
    token_cost_per_1k: f64,
    /// Window size for analysis
    window_hours: u32,
}

impl Default for PerformanceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceAnalyzer {
    pub fn new() -> Self {
        Self {
            token_cost_per_1k: 0.002, // $0.002 per 1K tokens default
            window_hours: 24,
        }
    }

    pub fn with_token_cost(mut self, cost_per_1k: f64) -> Self {
        self.token_cost_per_1k = cost_per_1k;
        self
    }

    pub fn with_window_hours(mut self, hours: u32) -> Self {
        self.window_hours = hours;
        self
    }

    /// Build a performance profile from task data
    pub fn analyze(
        &self,
        agent_id: &str,
        task_count: u64,
        failed_count: u64,
        avg_latency_ms: f64,
        p95_latency_ms: u64,
        total_tokens: u64,
        uptime_percent: f64,
    ) -> PerformanceProfile {
        let window_end = Utc::now();
        let window_start = window_end - Duration::hours(self.window_hours as i64);
        let tasks_completed = task_count - failed_count;
        let error_rate = if task_count > 0 { failed_count as f64 / task_count as f64 } else { 0.0 };
        let avg_tokens_per_task = if task_count > 0 { total_tokens as f64 / task_count as f64 } else { 0.0 };
        let cost_estimate = total_tokens as f64 / 1000.0 * self.token_cost_per_1k;
        let minutes = self.window_hours as f64 * 60.0;
        let throughput = if minutes > 0.0 { task_count as f64 / minutes } else { 0.0 };

        // Scores (0.0 - 1.0)
        let reliability_score = (1.0 - error_rate).max(0.0) * (uptime_percent / 100.0);
        let efficiency_score = if avg_latency_ms > 0.0 {
            (1000.0 / avg_latency_ms).min(1.0)
        } else {
            1.0
        };
        let overall_score = reliability_score * 0.4 + efficiency_score * 0.3 + (uptime_percent / 100.0) * 0.3;

        let trend = if error_rate > 0.2 {
            Trend::Degrading
        } else if reliability_score > 0.9 && efficiency_score > 0.7 {
            Trend::Improving
        } else {
            Trend::Stable
        };

        let mut recommendations = Vec::new();
        if error_rate > 0.1 {
            recommendations.push(format!("High error rate ({:.1}%) — investigate failure causes", error_rate * 100.0));
        }
        if avg_latency_ms > 5000.0 {
            recommendations.push("High latency — consider caching or task decomposition".to_string());
        }
        if avg_tokens_per_task > 50_000.0 {
            recommendations.push("High token usage — optimize prompts or enable compression".to_string());
        }
        if uptime_percent < 99.0 {
            recommendations.push(format!("Uptime {:.2}% — check health monitoring", uptime_percent));
        }
        if cost_estimate > 10.0 {
            recommendations.push(format!("Cost ${:.2} — consider model downgrade for simple tasks", cost_estimate));
        }

        PerformanceProfile {
            agent_id: agent_id.to_string(),
            window_start,
            window_end,
            tasks_completed,
            tasks_failed: failed_count,
            avg_latency_ms,
            p95_latency_ms,
            avg_tokens_per_task,
            cost_estimate,
            uptime_percent,
            error_rate,
            throughput_per_minute: throughput,
            reliability_score,
            efficiency_score,
            overall_score,
            trend,
            recommendations,
        }
    }
}

/// Tracks performance profiles over time for trend analysis
pub struct PerformanceTracker {
    profiles: HashMap<String, Vec<PerformanceProfile>>,
    max_history: usize,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceTracker {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            max_history: 100,
        }
    }

    pub fn record(&mut self, profile: PerformanceProfile) {
        let history = self.profiles.entry(profile.agent_id.clone()).or_default();
        history.push(profile);
        if history.len() > self.max_history {
            let drain = history.len() - self.max_history;
            history.drain(0..drain);
        }
    }

    pub fn latest(&self, agent_id: &str) -> Option<&PerformanceProfile> {
        self.profiles.get(agent_id)?.last()
    }

    pub fn history(&self, agent_id: &str) -> &[PerformanceProfile] {
        self.profiles.get(agent_id).map_or(&[], |h| h.as_slice())
    }

    pub fn all_agents(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    /// Get agents sorted by overall score (best first)
    pub fn rank_agents(&self) -> Vec<(String, f64)> {
        let mut rankings: Vec<(String, f64)> = self.profiles.iter()
            .filter_map(|(id, h)| h.last().map(|p| (id.clone(), p.overall_score)))
            .collect();
        rankings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        rankings
    }

    /// Get agents that need attention (low scores)
    pub fn agents_needing_attention(&self, threshold: f64) -> Vec<String> {
        self.profiles.iter()
            .filter_map(|(id, h)| {
                h.last().filter(|p| p.overall_score < threshold).map(|_| id.clone())
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.profiles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_basic() {
        let analyzer = PerformanceAnalyzer::new();
        let profile = analyzer.analyze("a1", 100, 5, 200.0, 500, 500_000, 99.9);
        assert_eq!(profile.agent_id, "a1");
        assert_eq!(profile.tasks_completed, 95);
        assert_eq!(profile.tasks_failed, 5);
        assert!((profile.error_rate - 0.05).abs() < 0.01);
    }

    #[test]
    fn test_analyzer_cost() {
        let analyzer = PerformanceAnalyzer::new().with_token_cost(0.01);
        let profile = analyzer.analyze("a1", 10, 0, 100.0, 200, 100_000, 100.0);
        assert!((profile.cost_estimate - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_analyzer_throughput() {
        let analyzer = PerformanceAnalyzer::new().with_window_hours(1);
        let profile = analyzer.analyze("a1", 60, 0, 100.0, 200, 100_000, 100.0);
        assert!((profile.throughput_per_minute - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_analyzer_recommendations_high_error() {
        let analyzer = PerformanceAnalyzer::new();
        let profile = analyzer.analyze("a1", 100, 30, 100.0, 200, 100_000, 99.0);
        assert!(!profile.recommendations.is_empty());
    }

    #[test]
    fn test_analyzer_recommendations_high_latency() {
        let analyzer = PerformanceAnalyzer::new();
        let profile = analyzer.analyze("a1", 10, 0, 10000.0, 20000, 100_000, 100.0);
        assert!(profile.recommendations.iter().any(|r| r.contains("latency")));
    }

    #[test]
    fn test_trend_assignment() {
        let analyzer = PerformanceAnalyzer::new();
        let degrading = analyzer.analyze("a1", 100, 25, 100.0, 200, 100_000, 99.0);
        assert_eq!(degrading.trend, Trend::Degrading);

        let improving = analyzer.analyze("a2", 100, 1, 50.0, 100, 50_000, 99.99);
        assert_eq!(improving.trend, Trend::Improving);
    }

    #[test]
    fn test_performance_tracker() {
        let mut tracker = PerformanceTracker::new();
        let analyzer = PerformanceAnalyzer::new();
        let profile = analyzer.analyze("a1", 100, 5, 200.0, 500, 500_000, 99.9);
        tracker.record(profile);
        assert!(tracker.latest("a1").is_some());
        assert_eq!(tracker.all_agents().len(), 1);
    }

    #[test]
    fn test_performance_tracker_ranking() {
        let mut tracker = PerformanceTracker::new();
        let analyzer = PerformanceAnalyzer::new();
        tracker.record(analyzer.analyze("a1", 100, 20, 500.0, 1000, 500_000, 95.0));
        tracker.record(analyzer.analyze("a2", 100, 1, 100.0, 200, 100_000, 99.99));
        let rankings = tracker.rank_agents();
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].0, "a2"); // better score first
    }

    #[test]
    fn test_agents_needing_attention() {
        let mut tracker = PerformanceTracker::new();
        let analyzer = PerformanceAnalyzer::new();
        tracker.record(analyzer.analyze("a1", 100, 50, 10000.0, 20000, 1_000_000, 80.0));
        tracker.record(analyzer.analyze("a2", 100, 0, 50.0, 100, 50_000, 99.99));
        let needs_attention = tracker.agents_needing_attention(0.5);
        assert!(needs_attention.contains(&"a1".to_string()));
    }

    #[test]
    fn test_scores_range() {
        let analyzer = PerformanceAnalyzer::new();
        let profile = analyzer.analyze("a1", 100, 5, 200.0, 500, 500_000, 99.5);
        assert!(profile.reliability_score >= 0.0 && profile.reliability_score <= 1.0);
        assert!(profile.efficiency_score >= 0.0 && profile.efficiency_score <= 1.0);
        assert!(profile.overall_score >= 0.0 && profile.overall_score <= 1.0);
    }
}
