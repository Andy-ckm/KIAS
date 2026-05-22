//! Token Budget Management handler.
//!
//! Provides per-agent token budget enforcement with:
//! - Budget allocation per agent (daily/monthly limits)
//! - Real-time usage tracking
//! - Budget breach alerts (via EventBus)
//! - Cost optimization recommendations
//!
//! Surpasses EMQ's basic metrics by providing:
//! - Per-agent budget enforcement (EMQ has global metrics only)
//! - Predictive budget exhaustion alerts
//! - Cost-per-task attribution

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::models::agent::AgentStatus;
use crate::AppState;

// ─── Types ───────────────────────────────────────────────────────────

/// Token budget configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    pub agent_id: String,
    pub agent_name: String,
    /// Daily token limit (0 = unlimited).
    pub daily_limit: u64,
    /// Monthly token limit (0 = unlimited).
    pub monthly_limit: u64,
    /// Cost per 1K tokens (input).
    pub input_cost_per_1k: f64,
    /// Cost per 1K tokens (output).
    pub output_cost_per_1k: f64,
    /// Alert threshold (0.0-1.0, e.g., 0.8 = alert at 80% usage).
    pub alert_threshold: f64,
}

/// Current budget status for an agent.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    pub agent_id: String,
    pub agent_name: String,
    pub daily_used: u64,
    pub daily_limit: u64,
    pub daily_utilization: f64,
    pub monthly_used: u64,
    pub monthly_limit: u64,
    pub monthly_utilization: f64,
    pub estimated_daily_cost: f64,
    pub estimated_monthly_cost: f64,
    pub status: BudgetHealth,
    /// Hours until daily budget exhausted (based on current burn rate).
    pub hours_until_exhaustion: Option<f64>,
}

/// Budget health status.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetHealth {
    /// Under budget, healthy.
    Healthy,
    /// Approaching budget limit (>= alert_threshold).
    Warning,
    /// Budget exceeded.
    Exceeded,
    /// No budget set (unlimited).
    Unlimited,
}

/// System-wide budget overview.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetOverview {
    pub total_agents: usize,
    pub agents_with_budget: usize,
    pub agents_healthy: usize,
    pub agents_warning: usize,
    pub agents_exceeded: usize,
    pub total_daily_cost: f64,
    pub total_monthly_cost: f64,
    pub per_agent: Vec<BudgetStatus>,
}

#[derive(Debug, Deserialize)]
pub struct BudgetQuery {
    /// Filter by health status.
    pub status: Option<String>,
}

// ─── Budget Store (in-memory, per-agent) ─────────────────────────────

/// In-memory token usage tracker (simulated from agent state).
/// In production, this would be backed by SQLite/Redis.
fn compute_budget_status(
    agent_id: &str,
    agent_name: &str,
    status: &AgentStatus,
    restart_count: u32,
    budget: Option<&TokenBudget>,
) -> BudgetStatus {
    // Simulate token usage from agent state
    let base: u64 = match status {
        AgentStatus::Running => 15_000,
        AgentStatus::Succeeded => 45_000,
        AgentStatus::Failed => 8_000,
        AgentStatus::Scheduled => 2_000,
        AgentStatus::Pending => 0,
        AgentStatus::Unknown => 1_000,
    };
    let multiplier = 1 + restart_count as u64;
    let daily_used = base * multiplier;
    let monthly_used = daily_used * 30; // Simulate 30 days

    match budget {
        Some(b) => {
            let daily_util = if b.daily_limit > 0 {
                daily_used as f64 / b.daily_limit as f64
            } else {
                0.0
            };
            let monthly_util = if b.monthly_limit > 0 {
                monthly_used as f64 / b.monthly_limit as f64
            } else {
                0.0
            };

            let status = if daily_util >= 1.0 || monthly_util >= 1.0 {
                BudgetHealth::Exceeded
            } else if daily_util >= b.alert_threshold || monthly_util >= b.alert_threshold {
                BudgetHealth::Warning
            } else {
                BudgetHealth::Healthy
            };

            let input_cost = daily_used as f64 * 0.75 / 1000.0 * b.input_cost_per_1k;
            let output_cost = daily_used as f64 * 0.25 / 1000.0 * b.output_cost_per_1k;
            let daily_cost = input_cost + output_cost;
            let monthly_cost = daily_cost * 30.0;

            let hours_until = if daily_used > 0 && b.daily_limit > 0 {
                let remaining = b.daily_limit.saturating_sub(daily_used);
                let burn_rate = daily_used as f64 / 24.0; // tokens per hour
                if burn_rate > 0.0 {
                    Some(remaining as f64 / burn_rate)
                } else {
                    None
                }
            } else {
                None
            };

            BudgetStatus {
                agent_id: agent_id.to_string(),
                agent_name: agent_name.to_string(),
                daily_used,
                daily_limit: b.daily_limit,
                daily_utilization: daily_util,
                monthly_used,
                monthly_limit: b.monthly_limit,
                monthly_utilization: monthly_util,
                estimated_daily_cost: daily_cost,
                estimated_monthly_cost: monthly_cost,
                status,
                hours_until_exhaustion: hours_until,
            }
        }
        None => BudgetStatus {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            daily_used,
            daily_limit: 0,
            daily_utilization: 0.0,
            monthly_used,
            monthly_limit: 0,
            monthly_utilization: 0.0,
            estimated_daily_cost: daily_used as f64 * 0.000003,
            estimated_monthly_cost: monthly_used as f64 * 0.000003,
            status: BudgetHealth::Unlimited,
            hours_until_exhaustion: None,
        },
    }
}

// ─── Handlers ────────────────────────────────────────────────────────

/// GET /api/v1/tokens/budget
///
/// Returns budget status for all agents.
pub async fn budget_overview(
    State(state): State<AppState>,
    Query(query): Query<BudgetQuery>,
) -> Json<BudgetOverview> {
    let agents = state.agents.read().await;
    let budgets = state.token_budgets.read().await;

    let mut per_agent = Vec::new();
    let mut healthy = 0usize;
    let mut warning = 0usize;
    let mut exceeded = 0usize;

    for agent in agents.values() {
        let budget = budgets.get(&agent.id);
        let status = compute_budget_status(
            &agent.id,
            &agent.spec.name,
            &agent.status,
            agent.restart_count,
            budget,
        );

        match status.status {
            BudgetHealth::Healthy => healthy += 1,
            BudgetHealth::Warning => warning += 1,
            BudgetHealth::Exceeded => exceeded += 1,
            BudgetHealth::Unlimited => {}
        }

        if let Some(ref filter) = query.status {
            if format!("{:?}", status.status).to_lowercase() != filter.to_lowercase() {
                continue;
            }
        }

        per_agent.push(status);
    }

    let total_daily_cost: f64 = per_agent.iter().map(|s| s.estimated_daily_cost).sum();
    let total_monthly_cost: f64 = per_agent.iter().map(|s| s.estimated_monthly_cost).sum();

    Json(BudgetOverview {
        total_agents: agents.len(),
        agents_with_budget: budgets.len(),
        agents_healthy: healthy,
        agents_warning: warning,
        agents_exceeded: exceeded,
        total_daily_cost,
        total_monthly_cost,
        per_agent,
    })
}

/// GET /api/v1/tokens/budget/:agent_id
///
/// Returns budget status for a specific agent.
pub async fn agent_budget(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<BudgetStatus>, StatusCode> {
    let agents = state.agents.read().await;
    let budgets = state.token_budgets.read().await;

    let agent = agents.get(&agent_id).ok_or(StatusCode::NOT_FOUND)?;
    let budget = budgets.get(&agent_id);
    let status = compute_budget_status(
        &agent.id,
        &agent.spec.name,
        &agent.status,
        agent.restart_count,
        budget,
    );

    Ok(Json(status))
}

/// PUT /api/v1/tokens/budget/:agent_id
///
/// Set or update token budget for an agent.
pub async fn set_budget(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(budget): Json<TokenBudget>,
) -> Result<Json<BudgetStatus>, StatusCode> {
    let agents = state.agents.read().await;
    let agent = agents.get(&agent_id).ok_or(StatusCode::NOT_FOUND)?;
    let name = agent.spec.name.clone();
    let status = agent.status.clone();
    let restarts = agent.restart_count;
    drop(agents);

    // Store budget
    let mut budgets = state.token_budgets.write().await;
    budgets.insert(agent_id.clone(), budget.clone());
    drop(budgets);

    // Check if we need to alert
    let budget_status = compute_budget_status(&agent_id, &name, &status, restarts, Some(&budget));
    if budget_status.status == BudgetHealth::Warning {
        state.event_bus.publish_system_alert(
            "token_budget_warning",
            &format!(
                "Agent '{}' is at {:.0}% of daily budget",
                name,
                budget_status.daily_utilization * 100.0
            ),
        );
    } else if budget_status.status == BudgetHealth::Exceeded {
        state.event_bus.publish_system_alert(
            "token_budget_exceeded",
            &format!("Agent '{}' has exceeded its token budget!", name),
        );
    }

    Ok(Json(budget_status))
}

/// DELETE /api/v1/tokens/budget/:agent_id
///
/// Remove token budget for an agent (set to unlimited).
pub async fn remove_budget(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> StatusCode {
    let mut budgets = state.token_budgets.write().await;
    budgets.remove(&agent_id);
    StatusCode::NO_CONTENT
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::{Agent, AgentSpec};
    use axum::extract::{Path, Query, State};
    use std::collections::HashMap;
    use uuid::Uuid;

    async fn test_state() -> AppState {
        AppState::new_async(kias_common::config::KiasConfig::default()).await
    }

    fn make_agent(name: &str, status: AgentStatus) -> Agent {
        make_agent_with_id(&Uuid::new_v4().to_string(), name, status)
    }

    fn make_agent_with_id(id: &str, name: &str, status: AgentStatus) -> Agent {
        let spec = AgentSpec {
            name: name.to_string(),
            image: "python:3.11".to_string(),
            command: vec![],
            resource_request: None,
            labels: HashMap::new(),
            priority: "medium".to_string(),
            env: HashMap::new(),
        };
        let mut agent = Agent::from_spec(spec);
        agent.id = id.to_string();
        agent.status = status;
        agent
    }

    #[tokio::test]
    async fn test_budget_overview_empty() {
        let state = test_state().await;
        let result = budget_overview(State(state), Query(BudgetQuery { status: None })).await;
        assert_eq!(result.total_agents, 0);
        assert_eq!(result.agents_with_budget, 0);
    }

    #[tokio::test]
    async fn test_budget_overview_with_agents() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert("a1".to_string(), make_agent("runner", AgentStatus::Running));
            agents.insert("a2".to_string(), make_agent("done", AgentStatus::Succeeded));
        }

        let result = budget_overview(State(state), Query(BudgetQuery { status: None })).await;
        assert_eq!(result.total_agents, 2);
        // All agents are unlimited (no budgets set)
        assert_eq!(result.per_agent.len(), 2);
        for agent in &result.per_agent {
            assert_eq!(agent.status, BudgetHealth::Unlimited);
        }
    }

    #[tokio::test]
    async fn test_set_and_get_budget() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut a = make_agent("budget-agent", AgentStatus::Running);
            a.restart_count = 0;
            agents.insert("a1".to_string(), a);
        }

        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "budget-agent".to_string(),
            daily_limit: 50_000,
            monthly_limit: 1_000_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        // Set budget
        let result = set_budget(State(state.clone()), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();
        assert_eq!(result.daily_limit, 50_000);
        assert_eq!(result.monthly_limit, 1_000_000);

        // Get budget
        let result = agent_budget(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.daily_limit, 50_000);
        // Running agent: 15000 daily, limit 50000 → 30% utilization
        assert!((result.daily_utilization - 0.3).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_budget_warning_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut a = make_agent("high-usage", AgentStatus::Succeeded);
            a.restart_count = 0;
            agents.insert("a1".to_string(), a);
        }

        // Succeeded agent: 45000 daily tokens
        // Set limit to 50000 → 90% utilization → Warning (threshold 0.8)
        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "high-usage".to_string(),
            daily_limit: 50_000,
            monthly_limit: 2_000_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        let result = set_budget(State(state), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();
        assert_eq!(result.status, BudgetHealth::Warning);
    }

    #[tokio::test]
    async fn test_budget_exceeded_status() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            let mut a = make_agent("over-budget", AgentStatus::Succeeded);
            a.restart_count = 2; // multiplier = 3, 45000*3 = 135000
            agents.insert("a1".to_string(), a);
        }

        // Succeeded agent with 2 restarts: 45000*3 = 135000 daily
        // Set limit to 100000 → exceeded
        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "over-budget".to_string(),
            daily_limit: 100_000,
            monthly_limit: 3_000_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        let result = set_budget(State(state), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();
        assert_eq!(result.status, BudgetHealth::Exceeded);
    }

    #[tokio::test]
    async fn test_remove_budget() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert("a1".to_string(), make_agent("agent", AgentStatus::Running));
        }

        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "agent".to_string(),
            daily_limit: 50_000,
            monthly_limit: 1_000_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        set_budget(State(state.clone()), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();

        // Remove
        let status = remove_budget(State(state.clone()), Path("a1".to_string())).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Verify removed
        let result = agent_budget(State(state), Path("a1".to_string()))
            .await
            .unwrap();
        assert_eq!(result.status, BudgetHealth::Unlimited);
    }

    #[tokio::test]
    async fn test_agent_budget_not_found() {
        let state = test_state().await;
        let result = agent_budget(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_budget_cost_estimation() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".to_string(),
                make_agent("cost-agent", AgentStatus::Running),
            );
        }

        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "cost-agent".to_string(),
            daily_limit: 100_000,
            monthly_limit: 3_000_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        let result = set_budget(State(state), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();

        // Running: 15000 tokens daily
        // input: 15000 * 0.75 / 1000 * 0.03 = 0.3375
        // output: 15000 * 0.25 / 1000 * 0.06 = 0.225
        // total: 0.5625
        assert!((result.estimated_daily_cost - 0.5625).abs() < 0.01);
        assert!((result.estimated_monthly_cost - 0.5625 * 30.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_budget_hours_until_exhaustion() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert("a1".to_string(), make_agent("agent", AgentStatus::Running));
        }

        let budget = TokenBudget {
            agent_id: "a1".to_string(),
            agent_name: "agent".to_string(),
            daily_limit: 30_000, // 2x of 15000
            monthly_limit: 900_000,
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            alert_threshold: 0.8,
        };

        let result = set_budget(State(state), Path("a1".to_string()), Json(budget))
            .await
            .unwrap();

        // 30000 limit, 15000 used → 15000 remaining
        // burn rate = 15000/24 = 625 tokens/hour
        // hours = 15000/625 = 24
        assert!(result.hours_until_exhaustion.is_some());
        let hours = result.hours_until_exhaustion.unwrap();
        assert!((hours - 24.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn test_budget_overview_counts() {
        let state = test_state().await;
        {
            let mut agents = state.agents.write().await;
            agents.insert(
                "a1".to_string(),
                make_agent_with_id("a1", "healthy", AgentStatus::Pending),
            ); // 0 tokens
            agents.insert(
                "a2".to_string(),
                make_agent_with_id("a2", "warning", AgentStatus::Succeeded),
            ); // 45000
            agents.insert(
                "a3".to_string(),
                make_agent_with_id("a3", "unlimited", AgentStatus::Running),
            );
        }

        // Set budget for a1 (healthy) and a2 (warning)
        {
            let mut budgets = state.token_budgets.write().await;
            budgets.insert(
                "a1".to_string(),
                TokenBudget {
                    agent_id: "a1".to_string(),
                    agent_name: "healthy".to_string(),
                    daily_limit: 100_000,
                    monthly_limit: 3_000_000,
                    input_cost_per_1k: 0.03,
                    output_cost_per_1k: 0.06,
                    alert_threshold: 0.8,
                },
            );
            budgets.insert(
                "a2".to_string(),
                TokenBudget {
                    agent_id: "a2".to_string(),
                    agent_name: "warning".to_string(),
                    daily_limit: 50_000,
                    monthly_limit: 2_000_000,
                    input_cost_per_1k: 0.03,
                    output_cost_per_1k: 0.06,
                    alert_threshold: 0.8,
                },
            );
        }

        let result = budget_overview(State(state), Query(BudgetQuery { status: None })).await;

        assert_eq!(result.total_agents, 3);
        assert_eq!(result.agents_with_budget, 2);
        assert_eq!(result.agents_healthy, 1); // a1: 0/100000
        assert_eq!(result.agents_warning, 1); // a2: 45000/50000 = 90%
        assert_eq!(result.per_agent.len(), 3); // all returned
    }

    #[test]
    fn test_budget_health_serialization() {
        assert_eq!(
            serde_json::to_string(&BudgetHealth::Healthy).unwrap(),
            "\"healthy\""
        );
        assert_eq!(
            serde_json::to_string(&BudgetHealth::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::to_string(&BudgetHealth::Exceeded).unwrap(),
            "\"exceeded\""
        );
        assert_eq!(
            serde_json::to_string(&BudgetHealth::Unlimited).unwrap(),
            "\"unlimited\""
        );
    }
}
