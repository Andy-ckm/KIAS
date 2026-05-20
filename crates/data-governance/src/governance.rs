//! # Agent Governance Framework (Paper-Driven: arXiv 2508.15014)
//!
//! Implements a multi-layered governance framework for ethical and safe agentic AI.
//! Based on "Towards Safe and Ethical Agentic AI: A Multi-Layered Framework
//! for Governance" (2025).
//!
//! ## Core Mechanisms
//!
//! - **Ethical Decision Points (EDPs)**: Mandatory checkpoints before high-stakes actions
//! - **Agent Governance Chains (AGCs)**: Multi-agent interaction logging for cross-agent accountability
//! - **Safety Constraint Propagation (SCP)**: Safety rules propagate across connected agents
//!
//! ## Paper Claims
//!
//! - 4-layer governance: Risk Assessment → Preventive Controls → Real-time Monitoring → Accountability
//! - EDPs reduce harmful outputs by requiring explicit approval at decision boundaries
//! - AGCs enable full traceability in multi-agent delegation chains
//! - SCP ensures safety constraints are inherited and cannot be bypassed through delegation

use std::collections::HashMap;
use std::fmt;

use crate::accountability::{ActionId, ActionSeverity, AgentId};

/// Governance layer in the 4-layer framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GovernanceLayer {
    /// Layer 1: Evaluate potential harm before action.
    RiskAssessment,
    /// Layer 2: Policy-based controls (allowlists, blocklists).
    PreventiveControls,
    /// Layer 3: Real-time interception during execution.
    RealTimeMonitoring,
    /// Layer 4: Post-hoc audit and attribution.
    Accountability,
}

impl fmt::Display for GovernanceLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GovernanceLayer::RiskAssessment => write!(f, "RiskAssessment"),
            GovernanceLayer::PreventiveControls => write!(f, "PreventiveControls"),
            GovernanceLayer::RealTimeMonitoring => write!(f, "RealTimeMonitoring"),
            GovernanceLayer::Accountability => write!(f, "Accountability"),
        }
    }
}

/// Decision made at an Ethical Decision Point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EDPDecision {
    /// Action approved to proceed.
    Approved,
    /// Action denied — violates ethical/safety constraints.
    Denied,
    /// Action deferred — requires human review.
    Deferred,
}

impl fmt::Display for EDPDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EDPDecision::Approved => write!(f, "Approved"),
            EDPDecision::Denied => write!(f, "Denied"),
            EDPDecision::Deferred => write!(f, "Deferred"),
        }
    }
}

/// An Ethical Decision Point — mandatory checkpoint before high-stakes actions.
///
/// EDPs sit at decision boundaries where an agent's action could cause harm.
/// They evaluate the action against safety constraints and either approve,
/// deny, or defer to human review.
#[derive(Debug, Clone)]
pub struct EthicalDecisionPoint {
    /// Unique EDP identifier.
    pub id: String,
    /// Which governance layer this EDP belongs to.
    pub layer: GovernanceLayer,
    /// Minimum severity that triggers this EDP.
    pub severity_threshold: ActionSeverity,
    /// Human-readable description of what this EDP checks.
    pub description: String,
    /// Whether this EDP is currently active.
    pub active: bool,
}

impl EthicalDecisionPoint {
    /// Create a new EDP.
    pub fn new(
        id: impl Into<String>,
        layer: GovernanceLayer,
        severity_threshold: ActionSeverity,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            layer,
            severity_threshold,
            description: description.into(),
            active: true,
        }
    }

    /// Check if this EDP should trigger for the given severity.
    pub fn should_trigger(&self, severity: ActionSeverity) -> bool {
        self.active && severity >= self.severity_threshold
    }
}

/// Record of an EDP evaluation.
#[derive(Debug, Clone)]
pub struct EDPRecord {
    /// Which EDP was evaluated.
    pub edp_id: String,
    /// The action being evaluated.
    pub action_id: ActionId,
    /// Agent performing the action.
    pub agent_id: AgentId,
    /// Action severity.
    pub severity: ActionSeverity,
    /// Decision made.
    pub decision: EDPDecision,
    /// Reason for the decision.
    pub reason: String,
    /// Unix timestamp of the evaluation.
    pub evaluated_at: u64,
}

/// Manager for Ethical Decision Points.
#[derive(Debug, Default)]
pub struct EDPManager {
    /// Registered EDPs.
    edps: Vec<EthicalDecisionPoint>,
    /// History of EDP evaluations.
    records: Vec<EDPRecord>,
}

impl EDPManager {
    /// Create a new EDP manager.
    pub fn new() -> Self {
        Self {
            edps: Vec::new(),
            records: Vec::new(),
        }
    }

    /// Register a new EDP.
    pub fn register(&mut self, edp: EthicalDecisionPoint) {
        self.edps.push(edp);
    }

    /// Evaluate an action against all applicable EDPs.
    ///
    /// Returns the most restrictive decision (Denied > Deferred > Approved).
    pub fn evaluate(
        &mut self,
        action_id: &str,
        agent_id: &str,
        severity: ActionSeverity,
    ) -> EDPDecision {
        let mut final_decision = EDPDecision::Approved;

        for edp in &self.edps {
            if edp.should_trigger(severity) {
                // Default: defer to human for high/critical
                let decision = match severity {
                    ActionSeverity::Critical => EDPDecision::Deferred,
                    ActionSeverity::High => EDPDecision::Deferred,
                    _ => EDPDecision::Approved,
                };

                // Most restrictive wins
                if decision == EDPDecision::Denied {
                    final_decision = EDPDecision::Denied;
                } else if decision == EDPDecision::Deferred && final_decision != EDPDecision::Denied
                {
                    final_decision = EDPDecision::Deferred;
                }

                let record = EDPRecord {
                    edp_id: edp.id.clone(),
                    action_id: action_id.to_string(),
                    agent_id: agent_id.to_string(),
                    severity,
                    decision,
                    reason: format!("EDP '{}' triggered for severity {}", edp.id, severity),
                    evaluated_at: now_secs(),
                };
                self.records.push(record);
            }
        }

        final_decision
    }

    /// Get all EDP records for an action.
    pub fn get_records_for_action(&self, action_id: &str) -> Vec<&EDPRecord> {
        self.records
            .iter()
            .filter(|r| r.action_id == action_id)
            .collect()
    }

    /// Get total number of registered EDPs.
    pub fn edp_count(&self) -> usize {
        self.edps.len()
    }

    /// Get total number of evaluation records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Get all registered EDPs.
    pub fn all_edps(&self) -> &[EthicalDecisionPoint] {
        &self.edps
    }

    /// Get all evaluation records.
    pub fn all_records(&self) -> &[EDPRecord] {
        &self.records
    }
}

/// A link in the Agent Governance Chain — records one agent delegating to another.
#[derive(Debug, Clone)]
pub struct GovernanceLink {
    /// Agent that delegated.
    pub from_agent: AgentId,
    /// Agent that received the delegation.
    pub to_agent: AgentId,
    /// Action that was delegated.
    pub action_id: ActionId,
    /// Safety constraints propagated with the delegation.
    pub propagated_constraints: Vec<String>,
    /// Unix timestamp of the delegation.
    pub delegated_at: u64,
}

/// Agent Governance Chain — tracks multi-agent interaction chains.
///
/// When Agent A delegates to Agent B, which delegates to Agent C,
/// the AGC records the full chain for accountability.
#[derive(Debug, Default)]
pub struct AgentGovernanceChain {
    /// All governance links.
    links: Vec<GovernanceLink>,
    /// Agent-to-agent interaction counts.
    interaction_counts: HashMap<(AgentId, AgentId), u32>,
}

impl AgentGovernanceChain {
    /// Create a new empty governance chain.
    pub fn new() -> Self {
        Self {
            links: Vec::new(),
            interaction_counts: HashMap::new(),
        }
    }

    /// Record a delegation from one agent to another.
    pub fn record_delegation(
        &mut self,
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
        action_id: impl Into<ActionId>,
        constraints: Vec<String>,
    ) {
        let from_id = from.into();
        let to_id = to.into();
        let key = (from_id.clone(), to_id.clone());

        *self.interaction_counts.entry(key).or_insert(0) += 1;

        self.links.push(GovernanceLink {
            from_agent: from_id,
            to_agent: to_id,
            action_id: action_id.into(),
            propagated_constraints: constraints,
            delegated_at: now_secs(),
        });
    }

    /// Get the full delegation chain for an agent (who delegated to him).
    pub fn get_delegators(&self, agent_id: &str) -> Vec<&GovernanceLink> {
        self.links
            .iter()
            .filter(|l| l.to_agent == agent_id)
            .collect()
    }

    /// Get all delegations made by an agent.
    pub fn get_delegations(&self, agent_id: &str) -> Vec<&GovernanceLink> {
        self.links
            .iter()
            .filter(|l| l.from_agent == agent_id)
            .collect()
    }

    /// Get interaction count between two agents.
    pub fn interaction_count(&self, from: &str, to: &str) -> u32 {
        self.interaction_counts
            .get(&(from.to_string(), to.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Get total number of governance links.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Get all links.
    pub fn all_links(&self) -> &[GovernanceLink] {
        &self.links
    }
}

/// A safety constraint that can be propagated across agents.
#[derive(Debug, Clone)]
pub struct SafetyConstraint {
    /// Unique constraint identifier.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Severity level of violating this constraint.
    pub violation_severity: ActionSeverity,
    /// Whether child agents inherit this constraint.
    pub propagates: bool,
    /// Constraint parameters (e.g., max_file_size, allowed_commands).
    pub params: HashMap<String, String>,
}

impl SafetyConstraint {
    /// Create a new safety constraint.
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        violation_severity: ActionSeverity,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            violation_severity,
            propagates: true,
            params: HashMap::new(),
        }
    }

    /// Create a non-propagating constraint.
    pub fn non_propagating(mut self) -> Self {
        self.propagates = false;
        self
    }

    /// Add a parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

/// Safety Constraint Propagator — ensures constraints flow across agent delegations.
#[derive(Debug, Default)]
pub struct SafetyConstraintPropagator {
    /// Global constraints that apply to all agents.
    global_constraints: Vec<SafetyConstraint>,
    /// Per-agent constraint overrides.
    agent_constraints: HashMap<AgentId, Vec<SafetyConstraint>>,
    /// Propagation log: which constraints were propagated to which agents.
    propagation_log: Vec<PropagationRecord>,
}

/// Record of a constraint propagation event.
#[derive(Debug, Clone)]
pub struct PropagationRecord {
    /// Source agent.
    pub from_agent: AgentId,
    /// Target agent.
    pub to_agent: AgentId,
    /// Constraints that were propagated.
    pub constraint_ids: Vec<String>,
    /// Unix timestamp.
    pub propagated_at: u64,
}

impl SafetyConstraintPropagator {
    /// Create a new propagator.
    pub fn new() -> Self {
        Self {
            global_constraints: Vec::new(),
            agent_constraints: HashMap::new(),
            propagation_log: Vec::new(),
        }
    }

    /// Add a global constraint (applies to all agents).
    pub fn add_global_constraint(&mut self, constraint: SafetyConstraint) {
        self.global_constraints.push(constraint);
    }

    /// Add an agent-specific constraint.
    pub fn add_agent_constraint(
        &mut self,
        agent_id: impl Into<AgentId>,
        constraint: SafetyConstraint,
    ) {
        self.agent_constraints
            .entry(agent_id.into())
            .or_default()
            .push(constraint);
    }

    /// Propagate constraints from one agent to another during delegation.
    ///
    /// Returns the list of constraint IDs that were propagated.
    pub fn propagate(&mut self, from_agent: &str, to_agent: &str) -> Vec<String> {
        let mut propagated = Vec::new();

        // Propagate global constraints
        for c in &self.global_constraints {
            if c.propagates {
                propagated.push(c.id.clone());
            }
        }

        // Propagate source agent's constraints
        if let Some(constraints) = self.agent_constraints.get(from_agent) {
            for c in constraints {
                if c.propagates && !propagated.contains(&c.id) {
                    propagated.push(c.id.clone());
                }
            }
        }

        if !propagated.is_empty() {
            self.propagation_log.push(PropagationRecord {
                from_agent: from_agent.to_string(),
                to_agent: to_agent.to_string(),
                constraint_ids: propagated.clone(),
                propagated_at: now_secs(),
            });
        }

        propagated
    }

    /// Get all constraints applicable to an agent (global + propagated + specific).
    pub fn get_effective_constraints(&self, agent_id: &str) -> Vec<&SafetyConstraint> {
        let mut result: Vec<&SafetyConstraint> = Vec::new();

        // Global constraints
        for c in &self.global_constraints {
            result.push(c);
        }

        // Agent-specific constraints
        if let Some(constraints) = self.agent_constraints.get(agent_id) {
            for c in constraints {
                result.push(c);
            }
        }

        result
    }

    /// Get propagation log.
    pub fn propagation_log(&self) -> &[PropagationRecord] {
        &self.propagation_log
    }

    /// Get global constraint count.
    pub fn global_constraint_count(&self) -> usize {
        self.global_constraints.len()
    }
}

/// Get current Unix timestamp in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    // === EDP Manager Tests ===

    #[test]
    fn test_edp_new() {
        let edp = EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "Check before destructive ops",
        );
        assert_eq!(edp.id, "edp-1");
        assert_eq!(edp.layer, GovernanceLayer::PreventiveControls);
        assert_eq!(edp.severity_threshold, ActionSeverity::High);
        assert!(edp.active);
    }

    #[test]
    fn test_edp_should_trigger() {
        let edp = EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::RiskAssessment,
            ActionSeverity::High,
            "High-risk check",
        );
        assert!(!edp.should_trigger(ActionSeverity::Low));
        assert!(!edp.should_trigger(ActionSeverity::Medium));
        assert!(edp.should_trigger(ActionSeverity::High));
        assert!(edp.should_trigger(ActionSeverity::Critical));
    }

    #[test]
    fn test_edp_inactive_does_not_trigger() {
        let mut edp = EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::RiskAssessment,
            ActionSeverity::Low,
            "Always check",
        );
        edp.active = false;
        assert!(!edp.should_trigger(ActionSeverity::Critical));
    }

    #[test]
    fn test_edp_manager_register() {
        let mut mgr = EDPManager::new();
        assert_eq!(mgr.edp_count(), 0);

        mgr.register(EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "test",
        ));
        assert_eq!(mgr.edp_count(), 1);
    }

    #[test]
    fn test_edp_manager_evaluate_approves_low() {
        let mut mgr = EDPManager::new();
        mgr.register(EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "High-risk gate",
        ));

        let decision = mgr.evaluate("action-1", "agent-1", ActionSeverity::Low);
        assert_eq!(decision, EDPDecision::Approved);
        // Low severity doesn't trigger the EDP, so no records
        assert_eq!(mgr.record_count(), 0);
    }

    #[test]
    fn test_edp_manager_evaluate_defers_critical() {
        let mut mgr = EDPManager::new();
        mgr.register(EthicalDecisionPoint::new(
            "edp-critical",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "High-risk gate",
        ));

        let decision = mgr.evaluate("action-2", "agent-1", ActionSeverity::Critical);
        assert_eq!(decision, EDPDecision::Deferred);
        assert_eq!(mgr.record_count(), 1);
    }

    #[test]
    fn test_edp_manager_evaluate_defers_high() {
        let mut mgr = EDPManager::new();
        mgr.register(EthicalDecisionPoint::new(
            "edp-high",
            GovernanceLayer::RiskAssessment,
            ActionSeverity::High,
            "High check",
        ));

        let decision = mgr.evaluate("action-3", "agent-2", ActionSeverity::High);
        assert_eq!(decision, EDPDecision::Deferred);
    }

    #[test]
    fn test_edp_manager_get_records_for_action() {
        let mut mgr = EDPManager::new();
        mgr.register(EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "test",
        ));
        mgr.evaluate("action-x", "agent-1", ActionSeverity::Critical);
        mgr.evaluate("action-y", "agent-1", ActionSeverity::Critical);

        let records = mgr.get_records_for_action("action-x");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action_id, "action-x");
    }

    #[test]
    fn test_edp_manager_multiple_edps() {
        let mut mgr = EDPManager::new();
        mgr.register(EthicalDecisionPoint::new(
            "edp-1",
            GovernanceLayer::RiskAssessment,
            ActionSeverity::Medium,
            "Medium gate",
        ));
        mgr.register(EthicalDecisionPoint::new(
            "edp-2",
            GovernanceLayer::PreventiveControls,
            ActionSeverity::High,
            "High gate",
        ));

        // Critical triggers both EDPs
        let decision = mgr.evaluate("action-1", "agent-1", ActionSeverity::Critical);
        assert_eq!(decision, EDPDecision::Deferred);
        assert_eq!(mgr.record_count(), 2);
    }

    #[test]
    fn test_edp_display() {
        assert_eq!(format!("{}", EDPDecision::Approved), "Approved");
        assert_eq!(format!("{}", EDPDecision::Denied), "Denied");
        assert_eq!(format!("{}", EDPDecision::Deferred), "Deferred");
    }

    // === Agent Governance Chain Tests ===

    #[test]
    fn test_agc_record_delegation() {
        let mut agc = AgentGovernanceChain::new();
        agc.record_delegation("agent-a", "agent-b", "action-1", vec![]);
        assert_eq!(agc.link_count(), 1);
        assert_eq!(agc.interaction_count("agent-a", "agent-b"), 1);
    }

    #[test]
    fn test_agc_multiple_delegations() {
        let mut agc = AgentGovernanceChain::new();
        agc.record_delegation("a", "b", "act-1", vec![]);
        agc.record_delegation("a", "b", "act-2", vec![]);
        agc.record_delegation("b", "c", "act-3", vec![]);

        assert_eq!(agc.link_count(), 3);
        assert_eq!(agc.interaction_count("a", "b"), 2);
        assert_eq!(agc.interaction_count("b", "c"), 1);
        assert_eq!(agc.interaction_count("a", "c"), 0);
    }

    #[test]
    fn test_agc_get_delegators() {
        let mut agc = AgentGovernanceChain::new();
        agc.record_delegation("a", "b", "act-1", vec![]);
        agc.record_delegation("c", "b", "act-2", vec![]);

        let delegators = agc.get_delegators("b");
        assert_eq!(delegators.len(), 2);
    }

    #[test]
    fn test_agc_get_delegations() {
        let mut agc = AgentGovernanceChain::new();
        agc.record_delegation("a", "b", "act-1", vec![]);
        agc.record_delegation("a", "c", "act-2", vec![]);

        let delegations = agc.get_delegations("a");
        assert_eq!(delegations.len(), 2);
    }

    #[test]
    fn test_agc_with_constraints() {
        let mut agc = AgentGovernanceChain::new();
        agc.record_delegation(
            "a",
            "b",
            "act-1",
            vec!["no_rm_rf".to_string(), "read_only_db".to_string()],
        );

        let link = &agc.all_links()[0];
        assert_eq!(link.propagated_constraints.len(), 2);
        assert!(link
            .propagated_constraints
            .contains(&"no_rm_rf".to_string()));
    }

    // === Safety Constraint Propagation Tests ===

    #[test]
    fn test_safety_constraint_new() {
        let c = SafetyConstraint::new("sc-1", "No rm -rf", ActionSeverity::Critical);
        assert_eq!(c.id, "sc-1");
        assert!(c.propagates);
        assert!(c.params.is_empty());
    }

    #[test]
    fn test_safety_constraint_non_propagating() {
        let c =
            SafetyConstraint::new("sc-1", "Local only", ActionSeverity::Medium).non_propagating();
        assert!(!c.propagates);
    }

    #[test]
    fn test_safety_constraint_with_param() {
        let c = SafetyConstraint::new("sc-1", "Max size", ActionSeverity::Low)
            .with_param("max_bytes", "1048576");
        assert_eq!(c.params.get("max_bytes").unwrap(), "1048576");
    }

    #[test]
    fn test_propagator_global_constraints() {
        let mut prop = SafetyConstraintPropagator::new();
        prop.add_global_constraint(SafetyConstraint::new(
            "global-1",
            "No deletion",
            ActionSeverity::Critical,
        ));
        assert_eq!(prop.global_constraint_count(), 1);
    }

    #[test]
    fn test_propagator_propagate_global() {
        let mut prop = SafetyConstraintPropagator::new();
        prop.add_global_constraint(SafetyConstraint::new(
            "global-1",
            "No deletion",
            ActionSeverity::Critical,
        ));

        let propagated = prop.propagate("agent-a", "agent-b");
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0], "global-1");
        assert_eq!(prop.propagation_log().len(), 1);
    }

    #[test]
    fn test_propagator_propagate_agent_constraints() {
        let mut prop = SafetyConstraintPropagator::new();
        prop.add_agent_constraint(
            "agent-a",
            SafetyConstraint::new("a-only", "Agent A rule", ActionSeverity::High),
        );

        let propagated = prop.propagate("agent-a", "agent-b");
        assert!(propagated.contains(&"a-only".to_string()));
    }

    #[test]
    fn test_propagator_no_propagate_when_empty() {
        let mut prop = SafetyConstraintPropagator::new();
        let propagated = prop.propagate("agent-a", "agent-b");
        assert!(propagated.is_empty());
        assert_eq!(prop.propagation_log().len(), 0);
    }

    #[test]
    fn test_propagator_non_propagating_not_inherited() {
        let mut prop = SafetyConstraintPropagator::new();
        prop.add_global_constraint(
            SafetyConstraint::new("local-1", "Local only", ActionSeverity::Medium)
                .non_propagating(),
        );

        let propagated = prop.propagate("agent-a", "agent-b");
        assert!(propagated.is_empty());
    }

    #[test]
    fn test_propagator_effective_constraints() {
        let mut prop = SafetyConstraintPropagator::new();
        prop.add_global_constraint(SafetyConstraint::new(
            "global-1",
            "Global rule",
            ActionSeverity::High,
        ));
        prop.add_agent_constraint(
            "agent-x",
            SafetyConstraint::new("x-rule", "Agent X rule", ActionSeverity::Medium),
        );

        let effective = prop.get_effective_constraints("agent-x");
        assert_eq!(effective.len(), 2); // global + agent-specific

        let effective_other = prop.get_effective_constraints("agent-y");
        assert_eq!(effective_other.len(), 1); // only global
    }

    // === GovernanceLayer Display ===

    #[test]
    fn test_governance_layer_display() {
        assert_eq!(
            format!("{}", GovernanceLayer::RiskAssessment),
            "RiskAssessment"
        );
        assert_eq!(
            format!("{}", GovernanceLayer::PreventiveControls),
            "PreventiveControls"
        );
        assert_eq!(
            format!("{}", GovernanceLayer::RealTimeMonitoring),
            "RealTimeMonitoring"
        );
        assert_eq!(
            format!("{}", GovernanceLayer::Accountability),
            "Accountability"
        );
    }
}
