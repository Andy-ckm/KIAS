use crate::resource::ResourceSnapshot;
use crate::task_history::TaskStats;
use crate::view::AgentView;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Dashboard summary for the entire system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub generated_at: DateTime<Utc>,
    pub total_agents: usize,
    pub active_agents: usize,
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub total_tasks: u64,
    pub overall_success_rate: f64,
    pub avg_latency_ms: f64,
    pub total_tokens_used: u64,
    pub estimated_cost: f64,
    pub agents: Vec<AgentSummary>,
    pub alerts: Vec<Alert>,
}

/// Per-agent summary for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub agent_id: String,
    pub status: String,
    pub active_sessions: usize,
    pub total_sessions: usize,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub tokens_used: u64,
    pub health_score: f64,
}

/// Dashboard alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub level: AlertLevel,
    pub agent_id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// Generates dashboard data from various data sources
pub struct DashboardGenerator;

impl DashboardGenerator {
    /// Generate a full dashboard summary
    pub fn generate(
        views: &[AgentView],
        resources: &HashMap<String, ResourceSnapshot>,
        task_stats: &HashMap<String, TaskStats>,
    ) -> DashboardSummary {
        let mut agents = Vec::new();
        let mut alerts = Vec::new();
        let mut total_tasks = 0u64;
        let mut total_success = 0u64;
        let mut total_latency_weighted = 0.0f64;
        let mut total_latency_tasks = 0u64;
        let mut total_tokens = 0u64;

        for view in views {
            let active_sessions = view.get_active_sessions().len();
            let res = resources.get(&view.agent_id);
            let stats = task_stats.get(&view.agent_id);

            let cpu = res.map_or(0.0, |r| r.cpu_usage_percent);
            let mem = res.map_or(0.0, |r| r.memory_usage_percent());
            let tokens = stats.map_or(0, |s| s.total_tokens);
            let tasks = stats.map_or(0, |s| s.total_tasks);
            let failed = stats.map_or(0, |s| s.failed);
            let success_rate = stats.map_or(1.0, |s| s.success_rate);
            let avg_lat = stats.map_or(0.0, |s| s.avg_duration_ms);

            total_tasks += tasks;
            total_success += tasks - failed;
            if tasks > 0 {
                total_latency_weighted += avg_lat * tasks as f64;
                total_latency_tasks += tasks;
            }
            total_tokens += tokens;

            let health_score = compute_health_score(cpu, mem, success_rate, avg_lat);

            // Generate alerts
            if cpu > 90.0 {
                alerts.push(Alert {
                    level: AlertLevel::Critical,
                    agent_id: view.agent_id.clone(),
                    message: format!("CPU usage critical: {:.1}%", cpu),
                    timestamp: Utc::now(),
                });
            } else if cpu > 70.0 {
                alerts.push(Alert {
                    level: AlertLevel::Warning,
                    agent_id: view.agent_id.clone(),
                    message: format!("CPU usage high: {:.1}%", cpu),
                    timestamp: Utc::now(),
                });
            }

            if mem > 85.0 {
                alerts.push(Alert {
                    level: AlertLevel::Critical,
                    agent_id: view.agent_id.clone(),
                    message: format!("Memory usage critical: {:.1}%", mem),
                    timestamp: Utc::now(),
                });
            }

            if success_rate < 0.8 && tasks > 0 {
                alerts.push(Alert {
                    level: AlertLevel::Warning,
                    agent_id: view.agent_id.clone(),
                    message: format!("Low success rate: {:.1}%", success_rate * 100.0),
                    timestamp: Utc::now(),
                });
            }

            if health_score < 0.5 {
                alerts.push(Alert {
                    level: AlertLevel::Critical,
                    agent_id: view.agent_id.clone(),
                    message: format!("Health score low: {:.2}", health_score),
                    timestamp: Utc::now(),
                });
            }

            let status = if active_sessions > 0 {
                "active"
            } else {
                "idle"
            };

            agents.push(AgentSummary {
                agent_id: view.agent_id.clone(),
                status: status.to_string(),
                active_sessions,
                total_sessions: view.sessions.len(),
                cpu_usage_percent: cpu,
                memory_usage_percent: mem,
                tasks_completed: tasks - failed,
                tasks_failed: failed,
                success_rate,
                avg_latency_ms: avg_lat,
                tokens_used: tokens,
                health_score,
            });
        }

        let active_agents = views
            .iter()
            .filter(|v| !v.get_active_sessions().is_empty())
            .count();

        let total_sessions: usize = views.iter().map(|v| v.sessions.len()).sum();
        let active_sessions: usize = views.iter().map(|v| v.get_active_sessions().len()).sum();
        let overall_success_rate = if total_tasks > 0 {
            total_success as f64 / total_tasks as f64
        } else {
            1.0
        };
        let avg_latency = if total_latency_tasks > 0 {
            total_latency_weighted / total_latency_tasks as f64
        } else {
            0.0
        };
        let estimated_cost = total_tokens as f64 / 1000.0 * 0.002;

        DashboardSummary {
            generated_at: Utc::now(),
            total_agents: views.len(),
            active_agents,
            total_sessions,
            active_sessions,
            total_tasks,
            overall_success_rate,
            avg_latency_ms: avg_latency,
            total_tokens_used: total_tokens,
            estimated_cost,
            agents,
            alerts,
        }
    }
}

fn compute_health_score(cpu: f64, mem: f64, success_rate: f64, latency_ms: f64) -> f64 {
    let cpu_score = (1.0 - cpu / 100.0).max(0.0);
    let mem_score = (1.0 - mem / 100.0).max(0.0);
    let lat_score = if latency_ms > 0.0 {
        (1000.0 / latency_ms).min(1.0)
    } else {
        1.0
    };
    cpu_score * 0.2 + mem_score * 0.2 + success_rate * 0.3 + lat_score * 0.3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::ResourceSnapshot;
    use crate::task_history::TaskStats;
    use crate::view::AgentView;

    fn make_task_stats(total: u64, failed: u64, avg_lat: f64, tokens: u64) -> TaskStats {
        TaskStats {
            total_tasks: total,
            successful: total - failed,
            failed,
            timed_out: 0,
            cancelled: 0,
            success_rate: if total > 0 {
                (total - failed) as f64 / total as f64
            } else {
                1.0
            },
            avg_duration_ms: avg_lat,
            p50_duration_ms: avg_lat as u64,
            p95_duration_ms: (avg_lat * 2.0) as u64,
            p99_duration_ms: (avg_lat * 3.0) as u64,
            total_tokens: tokens,
            avg_tokens_per_task: if total > 0 {
                tokens as f64 / total as f64
            } else {
                0.0
            },
            total_retries: 0,
        }
    }

    #[test]
    fn test_dashboard_empty() {
        let summary = DashboardGenerator::generate(&[], &HashMap::new(), &HashMap::new());
        assert_eq!(summary.total_agents, 0);
        assert_eq!(summary.total_tasks, 0);
    }

    #[test]
    fn test_dashboard_with_agents() {
        let view = AgentView::new("a1");
        let views = vec![view];
        let mut resources = HashMap::new();
        resources.insert("a1".to_string(), ResourceSnapshot::new("a1"));
        let mut stats = HashMap::new();
        stats.insert("a1".to_string(), make_task_stats(100, 5, 200.0, 50000));

        let summary = DashboardGenerator::generate(&views, &resources, &stats);
        assert_eq!(summary.total_agents, 1);
        assert_eq!(summary.total_tasks, 100);
        assert!((summary.overall_success_rate - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_alerts_generated_for_high_cpu() {
        let view = AgentView::new("a1");
        let views = vec![view];
        let mut resources = HashMap::new();
        let mut snap = ResourceSnapshot::new("a1");
        snap.cpu_usage_percent = 95.0;
        resources.insert("a1".to_string(), snap);
        let stats = HashMap::new();

        let summary = DashboardGenerator::generate(&views, &resources, &stats);
        assert!(summary
            .alerts
            .iter()
            .any(|a| a.level == AlertLevel::Critical));
    }

    #[test]
    fn test_health_score() {
        let score = compute_health_score(10.0, 20.0, 0.99, 100.0);
        assert!(score > 0.8);

        let bad_score = compute_health_score(95.0, 90.0, 0.5, 10000.0);
        assert!(bad_score < 0.3);
    }

    #[test]
    fn test_agent_summary_fields() {
        let mut view = AgentView::new("a1");
        view.add_session(crate::session::Session::new("s1", "a1"));
        let views = vec![view];
        let mut resources = HashMap::new();
        resources.insert("a1".to_string(), ResourceSnapshot::new("a1"));
        let mut stats = HashMap::new();
        stats.insert("a1".to_string(), make_task_stats(50, 0, 100.0, 25000));

        let summary = DashboardGenerator::generate(&views, &resources, &stats);
        let agent = &summary.agents[0];
        assert_eq!(agent.agent_id, "a1");
        assert_eq!(agent.status, "active");
        assert_eq!(agent.active_sessions, 1);
        assert!((agent.success_rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_estimated_cost() {
        let view = AgentView::new("a1");
        let views = vec![view];
        let mut stats = HashMap::new();
        stats.insert("a1".to_string(), make_task_stats(10, 0, 100.0, 100_000));
        let summary = DashboardGenerator::generate(&views, &HashMap::new(), &stats);
        assert!((summary.estimated_cost - 0.2).abs() < 0.01);
    }
}
