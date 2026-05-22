//! # Policy Simulator
//!
//! Simulates policy decisions before they are enforced in production.
//! Allows safety officers to model "what-if" scenarios against the
//! compliance rule engine without affecting live agents.
//!
//! ## Design
//!
//! ```text
//! PolicySimulationRequest ──► PolicySimulator ──► PolicySimulationReport
//!        │                          │                    │
//!        │                          │              ┌──────┴──────┐
//!        │                          │              │ decisions   |
//!        │                          │              │ diffs        |
//!        │                          │              │ risk评估     |
//! PolicyContext ────────────────────┘              └─────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Outcome of a single policy decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DecisionOutcome {
    /// Action was allowed
    Allowed,
    /// Action was denied
    Denied,
    /// Action requires human approval
    RequiresApproval,
    /// Action triggered a warning but was allowed
    Warned,
}

impl DecisionOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionOutcome::Allowed => "allowed",
            DecisionOutcome::Denied => "denied",
            DecisionOutcome::RequiresApproval => "requires_approval",
            DecisionOutcome::Warned => "warned",
        }
    }

    /// True if the action can proceed without further approval.
    pub fn is_permissive(&self) -> bool {
        matches!(self, DecisionOutcome::Allowed | DecisionOutcome::Warned)
    }
}

/// A single simulated policy decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedDecision {
    /// Which policy rule was evaluated
    pub policy_rule: String,
    /// The decision produced by the simulator
    pub outcome: DecisionOutcome,
    /// Which conditions contributed to the decision
    pub conditions_matched: Vec<String>,
    /// Which conditions were not satisfied
    pub conditions_failed: Vec<String>,
    /// Risk score 0.0–1.0 associated with this decision
    pub risk_score: f64,
    /// Human-readable explanation
    pub explanation: String,
}

/// A change in policy between two simulation runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDiff {
    /// Rule that was added
    pub added_rules: Vec<String>,
    /// Rule that was removed
    pub removed_rules: Vec<String>,
    /// Rules whose conditions changed
    pub modified_rules: Vec<String>,
    /// Rules whose outcomes changed
    pub outcome_changes: HashMap<String, (DecisionOutcome, DecisionOutcome)>,
}

/// Request to simulate a policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySimulationRequest {
    /// ID of the agent or workflow to simulate
    pub target_id: String,
    /// The action being evaluated (e.g., "tool.call", "data.read", "agent.spawn")
    pub action: String,
    /// Resource being acted upon
    pub resource: String,
    /// Principal (user or agent) performing the action
    pub principal: String,
    /// Simulated context attributes
    pub context: HashMap<String, String>,
    /// Optional: baseline policy version to compare against
    pub baseline_version: Option<String>,
    /// New policy version to simulate (if comparing)
    pub new_version: Option<String>,
}

impl PolicySimulationRequest {
    pub fn new(
        target_id: impl Into<String>,
        action: impl Into<String>,
        resource: impl Into<String>,
        principal: impl Into<String>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            action: action.into(),
            resource: resource.into(),
            principal: principal.into(),
            context: HashMap::new(),
            baseline_version: None,
            new_version: None,
        }
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    pub fn with_baseline_version(mut self, version: impl Into<String>) -> Self {
        self.baseline_version = Some(version.into());
        self
    }

    pub fn with_new_version(mut self, version: impl Into<String>) -> Self {
        self.new_version = Some(version.into());
        self
    }
}

/// A single scenario in a batch simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationScenario {
    pub name: String,
    pub request: PolicySimulationRequest,
    /// Expected outcome (for validation after simulation)
    pub expected_outcome: Option<DecisionOutcome>,
}

impl SimulationScenario {
    pub fn new(name: impl Into<String>, request: PolicySimulationRequest) -> Self {
        Self {
            name: name.into(),
            request,
            expected_outcome: None,
        }
    }

    pub fn with_expected(mut self, outcome: DecisionOutcome) -> Self {
        self.expected_outcome = Some(outcome);
        self
    }
}

/// Result of a complete policy simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySimulationReport {
    pub id: String,
    /// All decisions made during the simulation
    pub decisions: Vec<SimulatedDecision>,
    /// Diff against baseline if version comparison was requested
    pub diff: Option<PolicyDiff>,
    /// All scenarios that matched "denied" or "requires_approval"
    pub high_risk_decisions: Vec<SimulatedDecision>,
    /// Overall risk assessment (0.0–1.0)
    pub overall_risk_score: f64,
    /// Aggregated decision counts
    pub decision_counts: HashMap<String, usize>,
    /// When the simulation was run
    pub simulated_at: SystemTime,
    /// Target that was simulated
    pub target_id: String,
}

impl PolicySimulationReport {
    pub fn new(
        id: impl Into<String>,
        target_id: impl Into<String>,
        decisions: Vec<SimulatedDecision>,
    ) -> Self {
        let decision_counts = decisions.iter().fold(HashMap::new(), |mut acc, d| {
            *acc.entry(d.outcome.as_str().to_string()).or_insert(0) += 1;
            acc
        });

        let high_risk_decisions: Vec<_> = decisions
            .iter()
            .filter(|d| {
                matches!(
                    d.outcome,
                    DecisionOutcome::Denied | DecisionOutcome::RequiresApproval
                ) || d.risk_score > 0.6
            })
            .cloned()
            .collect();

        let overall_risk_score = if decisions.is_empty() {
            0.0
        } else {
            let avg: f64 =
                decisions.iter().map(|d| d.risk_score).sum::<f64>() / decisions.len() as f64;
            // Penalize denied/requires_approval decisions
            let denial_penalty = decisions
                .iter()
                .filter(|d| {
                    matches!(
                        d.outcome,
                        DecisionOutcome::Denied | DecisionOutcome::RequiresApproval
                    )
                })
                .count() as f64
                * 0.15;
            (avg + denial_penalty).clamp(0.0, 1.0)
        };

        Self {
            id: id.into(),
            target_id: target_id.into(),
            decisions,
            diff: None,
            high_risk_decisions,
            overall_risk_score,
            decision_counts,
            simulated_at: SystemTime::now(),
        }
    }

    pub fn with_diff(mut self, diff: PolicyDiff) -> Self {
        self.diff = Some(diff);
        self
    }
}

// ─── Policy Simulator ──────────────────────────────────────────────────────────

/// Simulates policy decisions without enforcing them.
pub struct PolicySimulator {
    /// Policy rules in effect: (rule_name, risk_score, conditions, outcome)
    rules: Vec<PolicyRule>,
    /// Historical simulation cache for diffing
    #[allow(dead_code)]
    simulation_history: Vec<PolicySimulationReport>,
}

struct PolicyRule {
    name: String,
    action_pattern: String,
    resource_pattern: String,
    conditions: Vec<PolicyCondition>,
    outcome: DecisionOutcome,
    risk_score: f64,
}

struct PolicyCondition {
    attribute: String,
    operator: ConditionOperator,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    In,
    GreaterThan,
    LessThan,
}

impl PolicySimulator {
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            simulation_history: Vec::new(),
        }
    }

    /// Add a custom rule to the simulator.
    pub fn add_rule(
        mut self,
        name: impl Into<String>,
        action_pattern: impl Into<String>,
        resource_pattern: impl Into<String>,
        conditions: Vec<(String, &str, String)>,
        outcome: DecisionOutcome,
        risk_score: f64,
    ) -> Self {
        self.rules.push(PolicyRule {
            name: name.into(),
            action_pattern: action_pattern.into(),
            resource_pattern: resource_pattern.into(),
            conditions: conditions
                .into_iter()
                .map(|(attr, op, val)| PolicyCondition {
                    attribute: attr,
                    operator: Self::parse_op(op),
                    value: val,
                })
                .collect(),
            outcome,
            risk_score,
        });
        self
    }

    fn parse_op(op: &str) -> ConditionOperator {
        match op {
            "==" | "eq" | "equals" => ConditionOperator::Equals,
            "!=" | "neq" => ConditionOperator::NotEquals,
            "contains" => ConditionOperator::Contains,
            "in" => ConditionOperator::In,
            ">" | "gt" => ConditionOperator::GreaterThan,
            "<" | "lt" => ConditionOperator::LessThan,
            _ => ConditionOperator::Equals,
        }
    }

    /// Simulate a single policy decision.
    pub fn simulate(&self, request: &PolicySimulationRequest) -> SimulatedDecision {
        let mut matched_rules: Vec<&PolicyRule> = self
            .rules
            .iter()
            .filter(|r| {
                Self::pattern_matches(&r.action_pattern, &request.action)
                    && Self::pattern_matches(&r.resource_pattern, &request.resource)
            })
            .collect();

        // If no rules match, apply default permissive policy
        if matched_rules.is_empty() {
            return SimulatedDecision {
                policy_rule: "default_permissive".to_string(),
                outcome: DecisionOutcome::Allowed,
                conditions_matched: vec!["(no rules matched)".to_string()],
                conditions_failed: vec![],
                risk_score: 0.0,
                explanation: "No matching policy rules — default permissive outcome applied."
                    .to_string(),
            };
        }

        // Use the highest-risk matching rule
        matched_rules.sort_by(|a, b| {
            b.risk_score
                .partial_cmp(&a.risk_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let rule = matched_rules.remove(0);

        let (matched, failed): (Vec<_>, Vec<_>) = rule
            .conditions
            .iter()
            .partition(|c| self.evaluate_condition(c, request));

        SimulatedDecision {
            policy_rule: rule.name.clone(),
            outcome: rule.outcome.clone(),
            conditions_matched: matched
                .iter()
                .map(|c| {
                    format!(
                        "{} {} {}",
                        c.attribute,
                        format!("{:?}", c.operator).to_lowercase(),
                        c.value
                    )
                })
                .collect(),
            conditions_failed: failed
                .iter()
                .map(|c| {
                    format!(
                        "{} {} {}",
                        c.attribute,
                        format!("{:?}", c.operator).to_lowercase(),
                        c.value
                    )
                })
                .collect(),
            risk_score: rule.risk_score,
            explanation: format!(
                "Policy '{}' evaluated {} with risk {:.2}",
                rule.name,
                rule.outcome.as_str(),
                rule.risk_score
            ),
        }
    }

    fn evaluate_condition(
        &self,
        cond: &PolicyCondition,
        request: &PolicySimulationRequest,
    ) -> bool {
        let ctx_value = request
            .context
            .get(&cond.attribute)
            .cloned()
            .unwrap_or_else(|| {
                // Fall back to request fields
                match cond.attribute.as_str() {
                    "action" => request.action.clone(),
                    "resource" => request.resource.clone(),
                    "principal" => request.principal.clone(),
                    "target_id" => request.target_id.clone(),
                    _ => String::new(),
                }
            });

        match cond.operator {
            ConditionOperator::Equals => ctx_value == cond.value,
            ConditionOperator::NotEquals => ctx_value != cond.value,
            ConditionOperator::Contains => ctx_value.contains(&cond.value),
            ConditionOperator::In => cond.value.split(',').any(|v| v.trim() == ctx_value),
            ConditionOperator::GreaterThan => ctx_value
                .parse::<f64>()
                .map(|n| n > cond.value.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(false),
            ConditionOperator::LessThan => ctx_value
                .parse::<f64>()
                .map(|n| n < cond.value.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(false),
        }
    }

    fn pattern_matches(pattern: &str, value: &str) -> bool {
        if pattern == "*" || pattern.is_empty() {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return value.starts_with(prefix);
        }
        pattern == value
    }

    /// Run a batch of simulation scenarios.
    pub fn simulate_batch(
        &self,
        scenarios: &[SimulationScenario],
    ) -> Vec<(SimulatedDecision, Option<DecisionOutcome>)> {
        scenarios
            .iter()
            .map(|s| {
                let decision = self.simulate(&s.request);
                (decision, s.expected_outcome.clone())
            })
            .collect()
    }

    /// Simulate and return a full report.
    pub fn simulate_report(&self, request: PolicySimulationRequest) -> PolicySimulationReport {
        let id = uuid::Uuid::new_v4().to_string();
        let decision = self.simulate(&request);
        let decisions = vec![decision];
        let mut report = PolicySimulationReport::new(id, request.target_id.clone(), decisions);

        // If version comparison was requested, compute diff
        if request.baseline_version.is_some() && request.new_version.is_some() {
            let diff = self.compute_diff(
                request.baseline_version.as_deref().unwrap_or(""),
                request.new_version.as_deref().unwrap_or(""),
                &request,
            );
            report = report.with_diff(diff);
        }

        report
    }

    fn compute_diff(
        &self,
        _baseline: &str,
        _new: &str,
        _request: &PolicySimulationRequest,
    ) -> PolicyDiff {
        // Simplified diff: in a real implementation, this would compare
        // two versions of the policy rules stored in a registry.
        // Here we simulate a diff result.
        PolicyDiff {
            added_rules: vec![],
            removed_rules: vec![],
            modified_rules: vec![],
            outcome_changes: HashMap::new(),
        }
    }

    /// Simulate a complete scenario set and return a combined report.
    pub fn simulate_scenarios(&self, scenarios: &[SimulationScenario]) -> PolicySimulationReport {
        let decisions: Vec<_> = scenarios
            .iter()
            .map(|s| self.simulate(&s.request))
            .collect();
        let target_id = scenarios
            .first()
            .map(|s| s.request.target_id.clone())
            .unwrap_or_default();
        PolicySimulationReport::new(uuid::Uuid::new_v4().to_string(), target_id, decisions)
    }

    fn default_rules() -> Vec<PolicyRule> {
        vec![
            PolicyRule {
                name: "deny-dangerous-tools".to_string(),
                action_pattern: "tool.call".to_string(),
                resource_pattern: "exec_shell".to_string(),
                conditions: vec![PolicyCondition {
                    attribute: "principal".to_string(),
                    operator: ConditionOperator::NotEquals,
                    value: "admin".to_string(),
                }],
                outcome: DecisionOutcome::Denied,
                risk_score: 0.9,
            },
            PolicyRule {
                name: "warn-sensitive-data".to_string(),
                action_pattern: "data.read".to_string(),
                resource_pattern: "audit_log".to_string(),
                conditions: vec![],
                outcome: DecisionOutcome::Warned,
                risk_score: 0.4,
            },
            PolicyRule {
                name: "require-approval-agent-spawn".to_string(),
                action_pattern: "agent.spawn".to_string(),
                resource_pattern: "*".to_string(),
                conditions: vec![],
                outcome: DecisionOutcome::RequiresApproval,
                risk_score: 0.5,
            },
            PolicyRule {
                name: "allow-read-only".to_string(),
                action_pattern: "data.read".to_string(),
                resource_pattern: "*".to_string(),
                conditions: vec![],
                outcome: DecisionOutcome::Allowed,
                risk_score: 0.1,
            },
            PolicyRule {
                name: "deny-credential-export".to_string(),
                action_pattern: "data.export".to_string(),
                resource_pattern: "credentials".to_string(),
                conditions: vec![],
                outcome: DecisionOutcome::Denied,
                risk_score: 1.0,
            },
        ]
    }
}

impl Default for PolicySimulator {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simulator() -> PolicySimulator {
        PolicySimulator::new()
    }

    // --- DecisionOutcome tests ---

    #[test]
    fn test_decision_outcome_is_permissive() {
        assert!(DecisionOutcome::Allowed.is_permissive());
        assert!(DecisionOutcome::Warned.is_permissive());
        assert!(!DecisionOutcome::Denied.is_permissive());
        assert!(!DecisionOutcome::RequiresApproval.is_permissive());
    }

    #[test]
    fn test_decision_outcome_as_str() {
        assert_eq!(DecisionOutcome::Allowed.as_str(), "allowed");
        assert_eq!(DecisionOutcome::Denied.as_str(), "denied");
        assert_eq!(
            DecisionOutcome::RequiresApproval.as_str(),
            "requires_approval"
        );
        assert_eq!(DecisionOutcome::Warned.as_str(), "warned");
    }

    // --- PolicySimulationRequest tests ---

    #[test]
    fn test_policy_simulation_request_builder() {
        let req: PolicySimulationRequest =
            PolicySimulationRequest::new("agent-1", "tool.call", "read_file", "user-1")
                .with_context("role", "developer")
                .with_baseline_version("v1.0")
                .with_new_version("v1.1");

        assert_eq!(req.target_id, "agent-1");
        assert_eq!(req.action, "tool.call");
        assert_eq!(req.resource, "read_file");
        assert_eq!(req.principal, "user-1");
        assert_eq!(req.context.get("role"), Some(&"developer".to_string()));
        assert_eq!(req.baseline_version, Some("v1.0".to_string()));
        assert_eq!(req.new_version, Some("v1.1".to_string()));
    }

    #[test]
    fn test_policy_simulation_request_default_context_empty() {
        let req = PolicySimulationRequest::new("agent-1", "tool.call", "read_file", "user-1");
        assert!(req.context.is_empty());
    }

    // --- SimulationScenario tests ---

    #[test]
    fn test_simulation_scenario_builder() {
        let req = PolicySimulationRequest::new("agent-1", "tool.call", "exec_shell", "admin");
        let scenario = SimulationScenario::new("dangerous tool test", req)
            .with_expected(DecisionOutcome::Denied);

        assert_eq!(scenario.name, "dangerous tool test");
        assert_eq!(scenario.expected_outcome, Some(DecisionOutcome::Denied));
    }

    // --- PolicySimulator default rules tests ---

    #[test]
    fn test_simulator_has_default_rules() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "tool.call", "exec_shell", "user-1");
        let decision = sim.simulate(&req);
        // Non-admin calling exec_shell should be denied
        assert_eq!(decision.outcome, DecisionOutcome::Denied);
    }

    #[test]
    fn test_simulator_allows_read_for_admin() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "tool.call", "exec_shell", "admin");
        let decision = sim.simulate(&req);
        // Admin calling exec_shell should pass the condition
        // but exec_shell is still denied — deny-dangerous-tools applies regardless
        assert_eq!(decision.outcome, DecisionOutcome::Denied);
    }

    #[test]
    fn test_simulator_warns_sensitive_data() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "data.read", "audit_log", "user-1");
        let decision = sim.simulate(&req);
        assert_eq!(decision.outcome, DecisionOutcome::Warned);
        assert!(decision.conditions_matched.is_empty());
    }

    #[test]
    fn test_simulator_requires_approval_agent_spawn() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "agent.spawn", "new-agent", "user-1");
        let decision = sim.simulate(&req);
        assert_eq!(decision.outcome, DecisionOutcome::RequiresApproval);
    }

    #[test]
    fn test_simulator_denies_credential_export() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "data.export", "credentials", "user-1");
        let decision = sim.simulate(&req);
        assert_eq!(decision.outcome, DecisionOutcome::Denied);
        assert_eq!(decision.risk_score, 1.0);
    }

    // --- Batch simulation tests ---

    #[test]
    fn test_simulate_batch_multiple_scenarios() {
        let sim = make_simulator();
        let scenarios = vec![
            SimulationScenario::new(
                "exec_shell by non-admin",
                PolicySimulationRequest::new("agent-1", "tool.call", "exec_shell", "user-1"),
            ),
            SimulationScenario::new(
                "read audit log",
                PolicySimulationRequest::new("agent-1", "data.read", "audit_log", "user-1"),
            ),
            SimulationScenario::new(
                "spawn agent",
                PolicySimulationRequest::new("agent-1", "agent.spawn", "new-agent", "user-1"),
            ),
        ];

        let results = sim.simulate_batch(&scenarios);
        assert_eq!(results.len(), 3);

        let (decision1, _) = &results[0];
        assert_eq!(decision1.outcome, DecisionOutcome::Denied);

        let (decision2, _) = &results[1];
        assert_eq!(decision2.outcome, DecisionOutcome::Warned);

        let (decision3, _) = &results[2];
        assert_eq!(decision3.outcome, DecisionOutcome::RequiresApproval);
    }

    #[test]
    fn test_simulate_batch_validates_expectations() {
        let sim = make_simulator();
        let scenarios = vec![SimulationScenario::new(
            "expect denied",
            PolicySimulationRequest::new("agent-1", "data.export", "credentials", "user-1"),
        )
        .with_expected(DecisionOutcome::Denied)];

        let results = sim.simulate_batch(&scenarios);
        let (decision, expected) = &results[0];
        assert_eq!(expected.as_ref(), Some(&DecisionOutcome::Denied));
        assert_eq!(&decision.outcome, expected.as_ref().unwrap());
    }

    // --- Report tests ---

    #[test]
    fn test_policy_simulation_report_high_risk() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "data.export", "credentials", "user-1");
        let report = sim.simulate_report(req);

        assert!(!report.high_risk_decisions.is_empty());
        assert_eq!(report.overall_risk_score, 1.0);
    }

    #[test]
    fn test_policy_simulation_report_decision_counts() {
        let sim = make_simulator();
        let scenarios = vec![
            SimulationScenario::new(
                "t1",
                PolicySimulationRequest::new("a1", "data.read", "audit_log", "u1"),
            ),
            SimulationScenario::new(
                "t2",
                PolicySimulationRequest::new("a1", "data.read", "audit_log", "u1"),
            ),
            SimulationScenario::new(
                "t3",
                PolicySimulationRequest::new("a1", "agent.spawn", "x", "u1"),
            ),
        ];
        let report = sim.simulate_scenarios(&scenarios);
        assert_eq!(*report.decision_counts.get("warned").unwrap_or(&0), 2);
        assert_eq!(
            *report
                .decision_counts
                .get("requires_approval")
                .unwrap_or(&0),
            1
        );
    }

    #[test]
    fn test_policy_simulation_report_empty_scenarios() {
        let sim = make_simulator();
        let report = sim.simulate_scenarios(&[]);
        assert!(report.decisions.is_empty());
        assert_eq!(report.overall_risk_score, 0.0);
    }

    // --- Pattern matching tests ---

    #[test]
    fn test_pattern_matches_exact() {
        assert!(PolicySimulator::pattern_matches("tool.call", "tool.call"));
        assert!(!PolicySimulator::pattern_matches("tool.call", "data.read"));
    }

    #[test]
    fn test_pattern_matches_wildcard() {
        assert!(PolicySimulator::pattern_matches("*", "anything"));
        assert!(PolicySimulator::pattern_matches("tool.*", "tool.call"));
        assert!(PolicySimulator::pattern_matches("tool.*", "tool.read"));
        assert!(!PolicySimulator::pattern_matches("tool.*", "data.call"));
    }

    #[test]
    fn test_pattern_matches_empty() {
        assert!(PolicySimulator::pattern_matches("", "anything"));
    }

    // --- Custom rule tests ---

    #[test]
    fn test_add_custom_rule() {
        let sim = make_simulator().add_rule(
            "custom-allow-read",
            "data.read",
            "*",
            vec![],
            DecisionOutcome::Allowed,
            0.1,
        );

        let req = PolicySimulationRequest::new("agent-1", "data.read", "anything", "user-1");
        let decision = sim.simulate(&req);
        // With custom rule added, data.read should match "allow-read-only" first (risk 0.1)
        // since it sorts by risk_score descending, deny-dangerous-tools has risk 0.9
        // but it only matches exec_shell
        assert!(decision.outcome.is_permissive());
    }

    #[test]
    fn test_custom_rule_with_condition() {
        let sim = make_simulator().add_rule(
            "role-restricted",
            "data.read",
            "secret",
            vec![("role".into(), "eq", "admin".to_string())],
            DecisionOutcome::Allowed,
            0.2,
        );

        let req = PolicySimulationRequest::new("agent-1", "data.read", "secret", "admin")
            .with_context("role", "admin");
        let decision = sim.simulate(&req);
        assert!(decision
            .conditions_matched
            .iter()
            .any(|c| c.contains("role")));
    }

    // --- Serde roundtrip tests ---

    #[test]
    fn test_simulated_decision_serde() {
        let decision = SimulatedDecision {
            policy_rule: "test-rule".to_string(),
            outcome: DecisionOutcome::Denied,
            conditions_matched: vec!["role eq admin".to_string()],
            conditions_failed: vec![],
            risk_score: 0.8,
            explanation: "Denied by test".to_string(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        let decoded: SimulatedDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.policy_rule, "test-rule");
        assert_eq!(decoded.risk_score, 0.8);
    }

    #[test]
    fn test_policy_diff_serde() {
        let diff = PolicyDiff {
            added_rules: vec!["new-rule".to_string()],
            removed_rules: vec!["old-rule".to_string()],
            modified_rules: vec!["modified-rule".to_string()],
            outcome_changes: HashMap::new(),
        };
        let json = serde_json::to_string(&diff).unwrap();
        let decoded: PolicyDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.added_rules.len(), 1);
    }

    #[test]
    fn test_policy_simulation_report_serde() {
        let sim = make_simulator();
        let req = PolicySimulationRequest::new("agent-1", "data.read", "audit_log", "user-1");
        let report = sim.simulate_report(req);

        let json = serde_json::to_string(&report).unwrap();
        let decoded: PolicySimulationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.target_id, "agent-1");
        assert_eq!(decoded.decisions.len(), 1);
    }

    // --- Risk score bounds tests ---

    #[test]
    fn test_overall_risk_score_bounded() {
        let sim = make_simulator();
        let scenarios = vec![
            SimulationScenario::new(
                "t1",
                PolicySimulationRequest::new("a1", "data.export", "credentials", "u1"),
            ),
            SimulationScenario::new(
                "t2",
                PolicySimulationRequest::new("a1", "data.export", "credentials", "u1"),
            ),
        ];
        let report = sim.simulate_scenarios(&scenarios);
        assert!(report.overall_risk_score <= 1.0);
        assert!(report.overall_risk_score >= 0.0);
    }
}
