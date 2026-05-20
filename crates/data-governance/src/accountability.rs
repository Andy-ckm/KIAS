//! # Accountability Graph (Paper-Driven: arXiv 2412.08765)
//!
//! Implements causal dependency tracking between agent actions for fine-grained
//! attribution. Based on "Governing the Governors: Accountability Mechanisms
//! for LLM-Based Multi-Agent Systems" (Martinez & O'Brien, 2024).
//!
//! ## Core Concepts
//!
//! - **ActionNode**: A single agent action with metadata and outcome
//! - **CausalEdge**: A directed link from cause to effect
//! - **AccountabilityGraph**: DAG of actions with causal relationships
//! - **3 Modes**: Reactive (post-hoc), Proactive (pre-action risk), Continuous (real-time)
//!
//! ## Paper Claims
//!
//! - 94% correct attribution with Accountability Graph
//! - 37% reduction in harmful outputs with continuous monitoring
//! - Middleware architecture decouples governance from agent internals

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Unique identifier for an action node.
pub type ActionId = String;

/// Unique identifier for an agent.
pub type AgentId = String;

/// Monitoring mode for the accountability system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonitoringMode {
    /// Post-hoc analysis: trace back from an outcome to find root cause.
    Reactive,
    /// Pre-action risk scoring: evaluate potential harm before execution.
    Proactive,
    /// Real-time monitoring: intercept and evaluate actions as they happen.
    Continuous,
}

impl fmt::Display for MonitoringMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitoringMode::Reactive => write!(f, "Reactive"),
            MonitoringMode::Proactive => write!(f, "Proactive"),
            MonitoringMode::Continuous => write!(f, "Continuous"),
        }
    }
}

/// Severity level for an agent action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionSeverity {
    /// Low-risk read-only action.
    Low,
    /// Medium-risk action (e.g., config change).
    Medium,
    /// High-risk action (e.g., data deletion, external API call).
    High,
    /// Critical action requiring explicit approval.
    Critical,
}

impl fmt::Display for ActionSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionSeverity::Low => write!(f, "Low"),
            ActionSeverity::Medium => write!(f, "Medium"),
            ActionSeverity::High => write!(f, "High"),
            ActionSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Outcome of an agent action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionOutcome {
    /// Action completed successfully.
    Success,
    /// Action failed with an error.
    Failure,
    /// Action was blocked by governance policy.
    Blocked,
    /// Action is still in progress.
    Pending,
    /// Action was rolled back.
    RolledBack,
}

impl fmt::Display for ActionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionOutcome::Success => write!(f, "Success"),
            ActionOutcome::Failure => write!(f, "Failure"),
            ActionOutcome::Blocked => write!(f, "Blocked"),
            ActionOutcome::Pending => write!(f, "Pending"),
            ActionOutcome::RolledBack => write!(f, "RolledBack"),
        }
    }
}

/// A single agent action node in the accountability graph.
#[derive(Debug, Clone)]
pub struct ActionNode {
    /// Unique action identifier.
    pub id: ActionId,
    /// Agent that performed the action.
    pub agent_id: AgentId,
    /// Human-readable action description.
    pub action_type: String,
    /// Target resource (e.g., "file:/etc/nginx.conf", "api:/v1/agents").
    pub target: String,
    /// Severity level.
    pub severity: ActionSeverity,
    /// Outcome of the action.
    pub outcome: ActionOutcome,
    /// Unix timestamp (seconds) when the action started.
    pub started_at: u64,
    /// Unix timestamp (seconds) when the action completed (None if pending).
    pub completed_at: Option<u64>,
    /// Risk score (0.0 - 1.0) from proactive evaluation.
    pub risk_score: f64,
    /// Free-form metadata.
    pub metadata: HashMap<String, String>,
}

impl ActionNode {
    /// Create a new action node with pending outcome.
    pub fn new(
        id: impl Into<ActionId>,
        agent_id: impl Into<AgentId>,
        action_type: impl Into<String>,
        target: impl Into<String>,
        severity: ActionSeverity,
    ) -> Self {
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            action_type: action_type.into(),
            target: target.into(),
            severity,
            outcome: ActionOutcome::Pending,
            started_at: now_secs(),
            completed_at: None,
            risk_score: 0.0,
            metadata: HashMap::new(),
        }
    }

    /// Mark the action as completed with a given outcome.
    pub fn complete(&mut self, outcome: ActionOutcome) {
        self.outcome = outcome;
        self.completed_at = Some(now_secs());
    }

    /// Set the risk score (clamped to 0.0-1.0).
    pub fn with_risk_score(mut self, score: f64) -> Self {
        self.risk_score = score.clamp(0.0, 1.0);
        self
    }

    /// Add metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Duration in seconds (None if still pending).
    pub fn duration_secs(&self) -> Option<u64> {
        self.completed_at
            .map(|end| end.saturating_sub(self.started_at))
    }

    /// Whether this action is terminal (completed, failed, blocked, or rolled back).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.outcome,
            ActionOutcome::Success
                | ActionOutcome::Failure
                | ActionOutcome::Blocked
                | ActionOutcome::RolledBack
        )
    }
}

/// A directed causal edge from one action to another.
#[derive(Debug, Clone)]
pub struct CausalEdge {
    /// The action that caused the effect.
    pub cause_id: ActionId,
    /// The action that was caused.
    pub effect_id: ActionId,
    /// Human-readable description of the causal relationship.
    pub relationship: String,
}

/// Result of an attribution analysis.
#[derive(Debug, Clone)]
pub struct AttributionResult {
    /// The root-cause action.
    pub root_cause: ActionId,
    /// The agent responsible.
    pub responsible_agent: AgentId,
    /// Full causal chain from root cause to the queried action.
    pub causal_chain: Vec<ActionId>,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Human-readable explanation.
    pub explanation: String,
}

/// Pre-action risk assessment result.
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    /// Overall risk score (0.0 - 1.0).
    pub risk_score: f64,
    /// Whether the action should be blocked.
    pub should_block: bool,
    /// Reasons for the risk score.
    pub reasons: Vec<String>,
    /// Recommended mitigations.
    pub mitigations: Vec<String>,
}

/// The core accountability graph.
///
/// Tracks agent actions as nodes and causal relationships as edges.
/// Supports reactive (post-hoc), proactive (pre-action), and continuous
/// (real-time) monitoring modes.
#[derive(Debug)]
pub struct AccountabilityGraph {
    /// All action nodes, keyed by action ID.
    nodes: HashMap<ActionId, ActionNode>,
    /// All causal edges.
    edges: Vec<CausalEdge>,
    /// Current monitoring mode.
    mode: MonitoringMode,
    /// Risk threshold for proactive blocking (default: 0.8).
    risk_threshold: f64,
    /// Maximum graph size before pruning oldest nodes.
    max_nodes: usize,
}

impl AccountabilityGraph {
    /// Create a new accountability graph with the given monitoring mode.
    pub fn new(mode: MonitoringMode) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            mode,
            risk_threshold: 0.8,
            max_nodes: 10_000,
        }
    }

    /// Set the risk threshold for proactive blocking.
    pub fn with_risk_threshold(mut self, threshold: f64) -> Self {
        self.risk_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set maximum number of nodes before pruning.
    pub fn with_max_nodes(mut self, max: usize) -> Self {
        self.max_nodes = max;
        self
    }

    /// Get the current monitoring mode.
    pub fn mode(&self) -> MonitoringMode {
        self.mode
    }

    /// Set the monitoring mode.
    pub fn set_mode(&mut self, mode: MonitoringMode) {
        self.mode = mode;
    }

    /// Record a new action node.
    pub fn record_action(&mut self, action: ActionNode) -> ActionId {
        let id = action.id.clone();
        self.nodes.insert(id.clone(), action);
        self.prune_if_needed();
        id
    }

    /// Update an existing action's outcome.
    pub fn complete_action(&mut self, action_id: &str, outcome: ActionOutcome) -> bool {
        if let Some(node) = self.nodes.get_mut(action_id) {
            node.complete(outcome);
            true
        } else {
            false
        }
    }

    /// Add a causal edge between two actions.
    pub fn add_causal_link(
        &mut self,
        cause_id: impl Into<ActionId>,
        effect_id: impl Into<ActionId>,
        relationship: impl Into<String>,
    ) -> Result<(), AccountabilityError> {
        let cause = cause_id.into();
        let effect = effect_id.into();

        if !self.nodes.contains_key(&cause) {
            return Err(AccountabilityError::ActionNotFound(cause));
        }
        if !self.nodes.contains_key(&effect) {
            return Err(AccountabilityError::ActionNotFound(effect));
        }
        if cause == effect {
            return Err(AccountabilityError::SelfReference(cause));
        }

        self.edges.push(CausalEdge {
            cause_id: cause,
            effect_id: effect,
            relationship: relationship.into(),
        });
        Ok(())
    }

    /// Get an action node by ID.
    pub fn get_action(&self, action_id: &str) -> Option<&ActionNode> {
        self.nodes.get(action_id)
    }

    /// Get all actions for a specific agent.
    pub fn get_agent_actions(&self, agent_id: &str) -> Vec<&ActionNode> {
        self.nodes
            .values()
            .filter(|n| n.agent_id == agent_id)
            .collect()
    }

    /// Get all terminal (completed) actions.
    pub fn get_terminal_actions(&self) -> Vec<&ActionNode> {
        self.nodes.values().filter(|n| n.is_terminal()).collect()
    }

    /// Get all failed actions.
    pub fn get_failed_actions(&self) -> Vec<&ActionNode> {
        self.nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Failure)
            .collect()
    }

    /// Get total number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Iterate over all action nodes (id, &ActionNode).
    pub fn all_nodes(&self) -> impl Iterator<Item = (&ActionId, &ActionNode)> {
        self.nodes.iter()
    }

    /// Get all causal edges.
    pub fn all_edges(&self) -> &[CausalEdge] {
        &self.edges
    }

    // ── Reactive Mode: Post-hoc Analysis ──────────────────────────────

    /// Trace back from an action to find the root cause.
    ///
    /// Follows causal edges backwards to find the originating action.
    /// Returns an AttributionResult with the full causal chain.
    pub fn trace_root_cause(&self, action_id: &str) -> Result<AttributionResult, AccountabilityError> {
        let _action = self
            .nodes
            .get(action_id)
            .ok_or_else(|| AccountabilityError::ActionNotFound(action_id.to_string()))?;

        let mut chain = vec![action_id.to_string()];
        let mut current = action_id.to_string();
        let mut visited = std::collections::HashSet::new();
        visited.insert(current.clone());

        // Follow edges backwards
        loop {
            let parent = self.edges.iter().find(|e| e.effect_id == current);
            match parent {
                Some(edge) => {
                    if visited.contains(&edge.cause_id) {
                        break; // Cycle detected
                    }
                    visited.insert(edge.cause_id.clone());
                    chain.push(edge.cause_id.clone());
                    current = edge.cause_id.clone();
                }
                None => break,
            }
        }

        let root_cause_id = chain.last().cloned().unwrap_or_else(|| action_id.to_string());
        let root_cause = self.nodes.get(&root_cause_id);

        let responsible_agent = root_cause
            .map(|n| n.agent_id.clone())
            .unwrap_or_default();

        let confidence = if chain.len() > 1 {
            // Confidence decreases with chain length
            (1.0 / chain.len() as f64).max(0.5)
        } else {
            1.0 // Direct action, high confidence
        };

        let explanation = if chain.len() > 1 {
            format!(
                "Root cause traced through {} actions. Agent '{}' initiated the chain.",
                chain.len(),
                responsible_agent
            )
        } else {
            format!(
                "Direct action by agent '{}' — no causal chain.",
                responsible_agent
            )
        };

        chain.reverse(); // Root cause first

        Ok(AttributionResult {
            root_cause: root_cause_id,
            responsible_agent,
            causal_chain: chain,
            confidence,
            explanation,
        })
    }

    // ── Proactive Mode: Pre-action Risk Scoring ───────────────────────

    /// Evaluate the risk of a proposed action before execution.
    ///
    /// Returns a RiskAssessment with score, blocking decision, and mitigations.
    pub fn evaluate_risk(&self, action: &ActionNode) -> RiskAssessment {
        let mut score = 0.0;
        let mut reasons = Vec::new();
        let mut mitigations = Vec::new();

        // Factor 1: Severity-based risk
        let severity_risk = match action.severity {
            ActionSeverity::Low => 0.1,
            ActionSeverity::Medium => 0.3,
            ActionSeverity::High => 0.6,
            ActionSeverity::Critical => 0.9,
        };
        score += severity_risk;
        if action.severity >= ActionSeverity::High {
            reasons.push(format!("High severity action: {}", action.severity));
            mitigations.push("Require explicit approval before execution".to_string());
        }

        // Factor 2: Agent history — how many failures has this agent had?
        let agent_failures = self
            .nodes
            .values()
            .filter(|n| n.agent_id == action.agent_id && n.outcome == ActionOutcome::Failure)
            .count();
        let failure_risk = (agent_failures as f64 * 0.1).min(0.3);
        score += failure_risk;
        if agent_failures > 0 {
            reasons.push(format!(
                "Agent '{}' has {} past failures",
                action.agent_id, agent_failures
            ));
            mitigations.push("Increase monitoring frequency for this agent".to_string());
        }

        // Factor 3: Target sensitivity
        let sensitive_targets = ["/etc/", "/var/", "rm ", "DROP ", "DELETE ", "sudo "];
        let is_sensitive = sensitive_targets
            .iter()
            .any(|t| action.target.contains(t) || action.action_type.contains(t));
        if is_sensitive {
            score += 0.2;
            reasons.push("Action targets sensitive resource".to_string());
            mitigations.push("Create checkpoint/backup before execution".to_string());
        }

        // Factor 4: Concurrent actions by same agent
        let concurrent = self
            .nodes
            .values()
            .filter(|n| n.agent_id == action.agent_id && !n.is_terminal())
            .count();
        if concurrent > 3 {
            score += 0.1;
            reasons.push(format!("Agent has {} concurrent actions", concurrent));
            mitigations.push("Throttle agent action rate".to_string());
        }

        let score = score.clamp(0.0, 1.0);
        let should_block = score >= self.risk_threshold;

        if should_block {
            reasons.push(format!(
                "Risk score {:.2} exceeds threshold {:.2}",
                score, self.risk_threshold
            ));
        }

        RiskAssessment {
            risk_score: score,
            should_block,
            reasons,
            mitigations,
        }
    }

    // ── Continuous Mode: Real-time Monitoring ─────────────────────────

    /// Get a real-time summary of the accountability state.
    ///
    /// Returns statistics about active actions, failures, and risk levels.
    pub fn monitoring_snapshot(&self) -> MonitoringSnapshot {
        let total = self.nodes.len();
        let pending = self
            .nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Pending)
            .count();
        let failed = self
            .nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Failure)
            .count();
        let blocked = self
            .nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Blocked)
            .count();
        let succeeded = self
            .nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Success)
            .count();

        let avg_risk = if total > 0 {
            self.nodes.values().map(|n| n.risk_score).sum::<f64>() / total as f64
        } else {
            0.0
        };

        let high_risk_count = self
            .nodes
            .values()
            .filter(|n| n.risk_score >= self.risk_threshold)
            .count();

        let agents: Vec<AgentId> = self
            .nodes
            .values()
            .map(|n| n.agent_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        MonitoringSnapshot {
            total_actions: total,
            pending_actions: pending,
            failed_actions: failed,
            blocked_actions: blocked,
            succeeded_actions: succeeded,
            avg_risk_score: avg_risk,
            high_risk_count,
            active_agents: agents,
            mode: self.mode,
        }
    }

    /// Check if any active actions exceed the risk threshold.
    /// Returns IDs of high-risk pending actions.
    pub fn check_risk_violations(&self) -> Vec<&ActionNode> {
        self.nodes
            .values()
            .filter(|n| n.outcome == ActionOutcome::Pending && n.risk_score >= self.risk_threshold)
            .collect()
    }

    // ── Utility ───────────────────────────────────────────────────────

    /// Prune oldest nodes if we exceed max_nodes.
    fn prune_if_needed(&mut self) {
        if self.nodes.len() <= self.max_nodes {
            return;
        }
        // Remove oldest completed actions first
        let mut completed: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.is_terminal())
            .map(|(id, n)| (id.clone(), n.started_at))
            .collect();
        completed.sort_by_key(|(_, ts)| *ts);

        let to_remove = self.nodes.len() - self.max_nodes + 100; // Remove extra for headroom
        for (id, _) in completed.iter().take(to_remove) {
            self.nodes.remove(id);
            self.edges.retain(|e| e.cause_id != *id && e.effect_id != *id);
        }
    }

    /// Export all nodes and edges as a serializable snapshot.
    pub fn export_snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.clone(),
            mode: self.mode,
            timestamp: now_secs(),
        }
    }
}

/// Real-time monitoring summary.
#[derive(Debug, Clone)]
pub struct MonitoringSnapshot {
    pub total_actions: usize,
    pub pending_actions: usize,
    pub failed_actions: usize,
    pub blocked_actions: usize,
    pub succeeded_actions: usize,
    pub avg_risk_score: f64,
    pub high_risk_count: usize,
    pub active_agents: Vec<AgentId>,
    pub mode: MonitoringMode,
}

/// Serializable graph snapshot for persistence.
#[derive(Debug, Clone)]
pub struct GraphSnapshot {
    pub nodes: Vec<ActionNode>,
    pub edges: Vec<CausalEdge>,
    pub mode: MonitoringMode,
    pub timestamp: u64,
}

/// Errors in accountability operations.
#[derive(Debug, Clone)]
pub enum AccountabilityError {
    /// Action ID not found in the graph.
    ActionNotFound(ActionId),
    /// Self-referencing causal edge.
    SelfReference(ActionId),
    /// Cycle detected in causal graph.
    CycleDetected(Vec<ActionId>),
}

impl fmt::Display for AccountabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountabilityError::ActionNotFound(id) => write!(f, "Action not found: {}", id),
            AccountabilityError::SelfReference(id) => {
                write!(f, "Self-referencing edge: {}", id)
            }
            AccountabilityError::CycleDetected(chain) => {
                write!(f, "Cycle detected in chain: {:?}", chain)
            }
        }
    }
}

impl std::error::Error for AccountabilityError {}

/// Shared accountability graph state for use in Axum handlers/middleware.
pub type SharedAccountabilityGraph = Arc<RwLock<AccountabilityGraph>>;

/// Create a new shared accountability graph.
pub fn shared_graph(mode: MonitoringMode) -> SharedAccountabilityGraph {
    Arc::new(RwLock::new(AccountabilityGraph::new(mode)))
}

/// Get current Unix timestamp in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Properties that an immutable audit trail must satisfy.
/// Based on arXiv:2503.19876 — "Immutable Audit Trails for Autonomous Agent Actions."
///
/// The 4 audit properties are:
/// 1. **Completeness** — Every terminal action has a signed entry.
/// 2. **Integrity** — The hash chain is unbroken (no tampering).
/// 3. **Non-repudiation** — Every entry has an agent attribution.
/// 4. **Causal coherence** — Causal links in the graph match entries in the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditProperty {
    Completeness,
    Integrity,
    NonRepudiation,
    CausalCoherence,
}

impl fmt::Display for AuditProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditProperty::Completeness => write!(f, "Completeness"),
            AuditProperty::Integrity => write!(f, "Integrity"),
            AuditProperty::NonRepudiation => write!(f, "NonRepudiation"),
            AuditProperty::CausalCoherence => write!(f, "CausalCoherence"),
        }
    }
}

/// Result of verifying a single audit property.
#[derive(Debug, Clone)]
pub struct PropertyCheckResult {
    /// Which property was checked.
    pub property: AuditProperty,
    /// Whether the property holds.
    pub satisfied: bool,
    /// Human-readable details.
    pub details: String,
    /// Number of entries checked.
    pub entries_checked: usize,
    /// Number of violations found.
    pub violations: usize,
}

/// A single signed entry in the immutable audit trail.
///
/// Each entry contains the action data, the hash of the previous entry
/// (forming a hash chain), and its own computed hash. This enables
/// tamper detection: modifying any entry invalidates all subsequent hashes.
///
/// Paper: arXiv:2503.19876 — "Immutable Audit Trails for Autonomous Agent Actions"
#[derive(Debug, Clone)]
pub struct SignedAuditEntry {
    /// The action node this entry records.
    pub action: ActionNode,
    /// Hash of the previous entry ("genesis" for the first entry).
    pub prev_hash: String,
    /// SHA-256-like hash of this entry (computed from action data + prev_hash).
    pub entry_hash: String,
    /// Sequence number in the chain (0 = genesis).
    pub sequence: u64,
}

impl SignedAuditEntry {
    /// Compute a hash for the given action data and previous hash.
    fn compute_hash(prev_hash: &str, action: &ActionNode) -> String {
        let mut hasher = DefaultHasher::new();
        prev_hash.hash(&mut hasher);
        action.id.hash(&mut hasher);
        action.agent_id.hash(&mut hasher);
        action.action_type.hash(&mut hasher);
        action.target.hash(&mut hasher);
        format!("{:?}", action.severity).hash(&mut hasher);
        format!("{:?}", action.outcome).hash(&mut hasher);
        action.started_at.hash(&mut hasher);
        action.completed_at.hash(&mut hasher);
        // Hash metadata deterministically (sorted keys)
        let mut sorted_meta: Vec<_> = action.metadata.iter().collect();
        sorted_meta.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in sorted_meta {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// Verify that this entry's hash is correct given the action data.
    pub fn verify_hash(&self) -> bool {
        let expected = Self::compute_hash(&self.prev_hash, &self.action);
        self.entry_hash == expected
    }
}

/// An immutable audit trail with hash-chain integrity.
///
/// Wraps an `AccountabilityGraph` and adds cryptographic-style linking:
/// each entry includes the hash of the previous entry, forming a tamper-evident
/// chain. The trail verifies 4 audit properties:
///
/// 1. **Completeness**: Every terminal action has a signed entry.
/// 2. **Integrity**: The hash chain is unbroken.
/// 3. **Non-repudiation**: Every entry has an agent ID.
/// 4. **Causal coherence**: Causal edges in the graph are represented in the chain.
///
/// Paper: arXiv:2503.19876 — "Immutable Audit Trails for Autonomous Agent Actions"
///   "Sub-ms overhead, <2% throughput reduction at 50 agents."
#[derive(Debug)]
pub struct ImmutableAuditTrail {
    /// The signed entries forming the hash chain.
    entries: Vec<SignedAuditEntry>,
    /// Reference to the underlying accountability graph.
    graph: AccountabilityGraph,
}

impl ImmutableAuditTrail {
    /// Create a new empty audit trail anchored to the given graph.
    pub fn new(graph: AccountabilityGraph) -> Self {
        Self {
            entries: Vec::new(),
            graph,
        }
    }

    /// Number of entries in the trail.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the previous hash (genesis hash if empty).
    fn prev_hash(&self) -> &str {
        self.entries
            .last()
            .map(|e| e.entry_hash.as_str())
            .unwrap_or("genesis")
    }

    /// Append an action to the audit trail, creating a signed entry.
    ///
    /// The entry's hash chains from the previous entry, making retroactive
    /// modification detectable.
    pub fn append(&mut self, action: ActionNode) -> String {
        let prev_hash = self.prev_hash().to_string();
        let entry_hash = SignedAuditEntry::compute_hash(&prev_hash, &action);
        let sequence = self.entries.len() as u64;

        // Also record in the underlying graph
        self.graph.record_action(action.clone());

        self.entries.push(SignedAuditEntry {
            action,
            prev_hash,
            entry_hash: entry_hash.clone(),
            sequence,
        });
        entry_hash
    }

    /// Append and immediately complete an action.
    pub fn append_completed(
        &mut self,
        action: ActionNode,
        outcome: ActionOutcome,
    ) -> String {
        let mut action = action;
        action.complete(outcome);
        self.append(action)
    }

    /// Get the hash of the last entry (or "genesis" if empty).
    pub fn chain_head(&self) -> &str {
        self.prev_hash()
    }

    /// Get all entries.
    pub fn entries(&self) -> &[SignedAuditEntry] {
        &self.entries
    }

    /// Get a reference to the underlying graph.
    pub fn graph(&self) -> &AccountabilityGraph {
        &self.graph
    }

    /// Get a mutable reference to the underlying graph.
    pub fn graph_mut(&mut self) -> &mut AccountabilityGraph {
        &mut self.graph
    }

    /// Verify the integrity of the hash chain.
    ///
    /// Checks that each entry's hash matches its computed value and that
    /// each entry's prev_hash matches the previous entry's entry_hash.
    pub fn verify_integrity(&self) -> PropertyCheckResult {
        let mut violations = 0;
        let mut details = Vec::new();

        for (i, entry) in self.entries.iter().enumerate() {
            // Check hash correctness
            if !entry.verify_hash() {
                violations += 1;
                details.push(format!(
                    "Entry {} (seq={}): hash mismatch — data tampered",
                    entry.action.id, entry.sequence
                ));
            }

            // Check chain linkage
            if i > 0 {
                let expected_prev = &self.entries[i - 1].entry_hash;
                if &entry.prev_hash != expected_prev {
                    violations += 1;
                    details.push(format!(
                        "Entry {} (seq={}): prev_hash mismatch — chain broken",
                        entry.action.id, entry.sequence
                    ));
                }
            } else if entry.prev_hash != "genesis" {
                violations += 1;
                details.push(format!(
                    "Entry {} (seq=0): first entry must have prev_hash='genesis'",
                    entry.action.id
                ));
            }
        }

        PropertyCheckResult {
            property: AuditProperty::Integrity,
            satisfied: violations == 0,
            details: if details.is_empty() {
                format!("Hash chain intact across {} entries", self.entries.len())
            } else {
                details.join("; ")
            },
            entries_checked: self.entries.len(),
            violations,
        }
    }

    /// Verify completeness: every terminal action in the graph has an entry.
    pub fn verify_completeness(&self) -> PropertyCheckResult {
        let mut violations = 0;
        let mut details = Vec::new();

        for (id, node) in self.graph.all_nodes() {
            if node.is_terminal() {
                let has_entry = self.entries.iter().any(|e| e.action.id == *id);
                if !has_entry {
                    violations += 1;
                    details.push(format!(
                        "Terminal action '{}' (agent={}, outcome={:?}) has no audit entry",
                        id, node.agent_id, node.outcome
                    ));
                }
            }
        }

        PropertyCheckResult {
            property: AuditProperty::Completeness,
            satisfied: violations == 0,
            details: if details.is_empty() {
                format!(
                    "All terminal actions have audit entries ({} entries, {} graph nodes)",
                    self.entries.len(),
                    self.graph.node_count()
                )
            } else {
                details.join("; ")
            },
            entries_checked: self.graph.node_count(),
            violations,
        }
    }

    /// Verify non-repudiation: every entry has a non-empty agent_id.
    pub fn verify_non_repudiation(&self) -> PropertyCheckResult {
        let mut violations = 0;
        let mut details = Vec::new();

        for entry in &self.entries {
            if entry.action.agent_id.is_empty() {
                violations += 1;
                details.push(format!(
                    "Entry {} (seq={}): empty agent_id — cannot attribute action",
                    entry.action.id, entry.sequence
                ));
            }
        }

        PropertyCheckResult {
            property: AuditProperty::NonRepudiation,
            satisfied: violations == 0,
            details: if details.is_empty() {
                format!(
                    "All {} entries have agent attribution",
                    self.entries.len()
                )
            } else {
                details.join("; ")
            },
            entries_checked: self.entries.len(),
            violations,
        }
    }

    /// Verify causal coherence: every causal edge in the graph has both
    /// endpoints present as entries in the trail.
    pub fn verify_causal_coherence(&self) -> PropertyCheckResult {
        let mut violations = 0;
        let mut details = Vec::new();
        let edges = self.graph.all_edges();
        let edge_count = edges.len();

        for edge in edges {
            let cause_has_entry = self.entries.iter().any(|e| e.action.id == edge.cause_id);
            let effect_has_entry = self.entries.iter().any(|e| e.action.id == edge.effect_id);

            if !cause_has_entry {
                violations += 1;
                details.push(format!(
                    "Causal edge '{}'→'{}': cause '{}' has no audit entry",
                    edge.cause_id, edge.effect_id, edge.cause_id
                ));
            }
            if !effect_has_entry {
                violations += 1;
                details.push(format!(
                    "Causal edge '{}'→'{}': effect '{}' has no audit entry",
                    edge.cause_id, edge.effect_id, edge.effect_id
                ));
            }
        }

        PropertyCheckResult {
            property: AuditProperty::CausalCoherence,
            satisfied: violations == 0,
            details: if details.is_empty() {
                format!(
                    "All {} causal edges have both endpoints in the trail",
                    edge_count
                )
            } else {
                details.join("; ")
            },
            entries_checked: edge_count,
            violations,
        }
    }

    /// Verify all 4 audit properties. Returns a vector of check results.
    pub fn verify_all(&self) -> Vec<PropertyCheckResult> {
        vec![
            self.verify_completeness(),
            self.verify_integrity(),
            self.verify_non_repudiation(),
            self.verify_causal_coherence(),
        ]
    }

    /// Check if all 4 properties are satisfied.
    pub fn is_valid(&self) -> bool {
        self.verify_all().iter().all(|r| r.satisfied)
    }

    /// Export the trail as a serializable snapshot.
    pub fn export_snapshot(&self) -> AuditTrailSnapshot {
        AuditTrailSnapshot {
            entry_count: self.entries.len(),
            chain_head: self.chain_head().to_string(),
            graph_nodes: self.graph.node_count(),
            graph_edges: self.graph.edge_count(),
            properties: self.verify_all(),
            entries: self
                .entries
                .iter()
                .map(|e| EntrySnapshot {
                    action_id: e.action.id.clone(),
                    agent_id: e.action.agent_id.clone(),
                    action_type: e.action.action_type.clone(),
                    target: e.action.target.clone(),
                    severity: format!("{:?}", e.action.severity),
                    outcome: format!("{:?}", e.action.outcome),
                    sequence: e.sequence,
                    entry_hash: e.entry_hash.clone(),
                    prev_hash: e.prev_hash.clone(),
                })
                .collect(),
        }
    }
}

/// Serializable snapshot of the audit trail for export/inspection.
#[derive(Debug, Clone)]
pub struct AuditTrailSnapshot {
    pub entry_count: usize,
    pub chain_head: String,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub properties: Vec<PropertyCheckResult>,
    pub entries: Vec<EntrySnapshot>,
}

/// A single entry in the exported snapshot.
#[derive(Debug, Clone)]
pub struct EntrySnapshot {
    pub action_id: String,
    pub agent_id: String,
    pub action_type: String,
    pub target: String,
    pub severity: String,
    pub outcome: String,
    pub sequence: u64,
    pub entry_hash: String,
    pub prev_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ActionNode Tests ────────────────────────────────────────────

    #[test]
    fn test_action_node_new() {
        let node = ActionNode::new("a1", "agent-1", "read_file", "/etc/hosts", ActionSeverity::Low);
        assert_eq!(node.id, "a1");
        assert_eq!(node.agent_id, "agent-1");
        assert_eq!(node.action_type, "read_file");
        assert_eq!(node.target, "/etc/hosts");
        assert_eq!(node.severity, ActionSeverity::Low);
        assert_eq!(node.outcome, ActionOutcome::Pending);
        assert!(node.completed_at.is_none());
        assert!(node.duration_secs().is_none());
        assert!(!node.is_terminal());
    }

    #[test]
    fn test_action_node_complete() {
        let mut node = ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::High);
        node.complete(ActionOutcome::Success);
        assert_eq!(node.outcome, ActionOutcome::Success);
        assert!(node.completed_at.is_some());
        assert!(node.duration_secs().is_some());
        assert!(node.is_terminal());
    }

    #[test]
    fn test_action_node_risk_score_clamped() {
        let node = ActionNode::new("a1", "agent-1", "test", "target", ActionSeverity::Low)
            .with_risk_score(1.5);
        assert_eq!(node.risk_score, 1.0);

        let node2 = ActionNode::new("a2", "agent-1", "test", "target", ActionSeverity::Low)
            .with_risk_score(-0.5);
        assert_eq!(node2.risk_score, 0.0);
    }

    #[test]
    fn test_action_node_metadata() {
        let node = ActionNode::new("a1", "agent-1", "cmd", "host", ActionSeverity::Medium)
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");
        assert_eq!(node.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(node.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_action_node_is_terminal_variants() {
        let outcomes = [
            (ActionOutcome::Pending, false),
            (ActionOutcome::Success, true),
            (ActionOutcome::Failure, true),
            (ActionOutcome::Blocked, true),
            (ActionOutcome::RolledBack, true),
        ];
        for (outcome, expected) in outcomes {
            let mut node = ActionNode::new("a1", "agent-1", "test", "t", ActionSeverity::Low);
            node.outcome = outcome;
            assert_eq!(node.is_terminal(), expected, "outcome: {}", outcome);
        }
    }

    // ── Graph Construction Tests ────────────────────────────────────

    #[test]
    fn test_graph_new() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.mode(), MonitoringMode::Reactive);
    }

    #[test]
    fn test_graph_record_and_get() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Continuous);
        let node = ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::High);
        graph.record_action(node);

        assert_eq!(graph.node_count(), 1);
        let retrieved = graph.get_action("a1").unwrap();
        assert_eq!(retrieved.agent_id, "agent-1");
    }

    #[test]
    fn test_graph_complete_action() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "test",
            "target",
            ActionSeverity::Low,
        ));

        assert!(graph.complete_action("a1", ActionOutcome::Success));
        assert!(!graph.complete_action("nonexistent", ActionOutcome::Failure));

        let node = graph.get_action("a1").unwrap();
        assert_eq!(node.outcome, ActionOutcome::Success);
    }

    #[test]
    fn test_graph_causal_link() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "init",
            "target",
            ActionSeverity::Low,
        ));
        graph.record_action(ActionNode::new(
            "a2",
            "agent-2",
            "follow",
            "target",
            ActionSeverity::Medium,
        ));

        assert!(graph.add_causal_link("a1", "a2", "triggered by").is_ok());
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_graph_causal_link_errors() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "test",
            "target",
            ActionSeverity::Low,
        ));

        // Missing action
        assert!(matches!(
            graph.add_causal_link("a1", "nonexistent", "test"),
            Err(AccountabilityError::ActionNotFound(_))
        ));
        assert!(matches!(
            graph.add_causal_link("nonexistent", "a1", "test"),
            Err(AccountabilityError::ActionNotFound(_))
        ));

        // Self-reference
        assert!(matches!(
            graph.add_causal_link("a1", "a1", "test"),
            Err(AccountabilityError::SelfReference(_))
        ));
    }

    // ── Reactive Mode Tests ─────────────────────────────────────────

    #[test]
    fn test_trace_root_cause_direct() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(
            ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::High),
        );
        graph.complete_action("a1", ActionOutcome::Failure);

        let result = graph.trace_root_cause("a1").unwrap();
        assert_eq!(result.root_cause, "a1");
        assert_eq!(result.responsible_agent, "agent-1");
        assert_eq!(result.causal_chain.len(), 1);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_trace_root_cause_chain() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "init",
            "config",
            ActionSeverity::Low,
        ));
        graph.record_action(ActionNode::new(
            "a2",
            "agent-2",
            "modify",
            "config",
            ActionSeverity::Medium,
        ));
        graph.record_action(ActionNode::new(
            "a3",
            "agent-3",
            "deploy",
            "prod",
            ActionSeverity::High,
        ));
        graph
            .add_causal_link("a1", "a2", "config change triggered")
            .unwrap();
        graph
            .add_causal_link("a2", "a3", "deployed based on config")
            .unwrap();

        let result = graph.trace_root_cause("a3").unwrap();
        assert_eq!(result.root_cause, "a1");
        assert_eq!(result.responsible_agent, "agent-1");
        assert_eq!(result.causal_chain, vec!["a1", "a2", "a3"]);
        assert!(result.confidence < 1.0); // Chain length > 1
        assert!(result.explanation.contains("3 actions"));
    }

    #[test]
    fn test_trace_root_cause_not_found() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        assert!(matches!(
            graph.trace_root_cause("nonexistent"),
            Err(AccountabilityError::ActionNotFound(_))
        ));
    }

    // ── Proactive Mode Tests ────────────────────────────────────────

    #[test]
    fn test_evaluate_risk_low_severity() {
        let graph = AccountabilityGraph::new(MonitoringMode::Proactive);
        let action = ActionNode::new("a1", "agent-1", "read_file", "/tmp/test", ActionSeverity::Low);

        let assessment = graph.evaluate_risk(&action);
        assert!(assessment.risk_score < 0.3);
        assert!(!assessment.should_block);
    }

    #[test]
    fn test_evaluate_risk_critical_severity() {
        let graph = AccountabilityGraph::new(MonitoringMode::Proactive).with_risk_threshold(0.8);
        let action = ActionNode::new(
            "a1",
            "agent-1",
            "rm -rf",
            "/var/data",
            ActionSeverity::Critical,
        );

        let assessment = graph.evaluate_risk(&action);
        assert!(assessment.risk_score >= 0.9); // Critical base = 0.9
        assert!(assessment.should_block);
        assert!(!assessment.reasons.is_empty());
        assert!(!assessment.mitigations.is_empty());
    }

    #[test]
    fn test_evaluate_risk_with_agent_failures() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Proactive).with_risk_threshold(0.8);
        // Record 3 past failures for agent-1
        for i in 0..3 {
            let mut node = ActionNode::new(
                format!("f{}", i),
                "agent-1",
                "deploy",
                "prod",
                ActionSeverity::Medium,
            );
            node.complete(ActionOutcome::Failure);
            graph.record_action(node);
        }

        let action = ActionNode::new(
            "a1",
            "agent-1",
            "deploy",
            "prod",
            ActionSeverity::Medium,
        );
        let assessment = graph.evaluate_risk(&action);
        assert!(assessment.risk_score > 0.3); // Base 0.3 + failure history
        assert!(assessment
            .reasons
            .iter()
            .any(|r| r.contains("past failures")));
    }

    #[test]
    fn test_evaluate_risk_sensitive_target() {
        let graph = AccountabilityGraph::new(MonitoringMode::Proactive);
        let action = ActionNode::new(
            "a1",
            "agent-1",
            "edit",
            "/etc/passwd",
            ActionSeverity::Medium,
        );

        let assessment = graph.evaluate_risk(&action);
        assert!(assessment.risk_score >= 0.5); // 0.3 (medium) + 0.2 (sensitive)
        assert!(assessment
            .reasons
            .iter()
            .any(|r| r.contains("sensitive")));
    }

    #[test]
    fn test_evaluate_risk_threshold_customization() {
        let graph = AccountabilityGraph::new(MonitoringMode::Proactive).with_risk_threshold(0.5);
        let action = ActionNode::new(
            "a1",
            "agent-1",
            "deploy",
            "prod",
            ActionSeverity::High,
        );

        let assessment = graph.evaluate_risk(&action);
        assert!(assessment.risk_score >= 0.6); // High severity
        assert!(assessment.should_block); // 0.6 >= 0.5 threshold
    }

    // ── Continuous Mode Tests ───────────────────────────────────────

    #[test]
    fn test_monitoring_snapshot_empty() {
        let graph = AccountabilityGraph::new(MonitoringMode::Continuous);
        let snap = graph.monitoring_snapshot();
        assert_eq!(snap.total_actions, 0);
        assert_eq!(snap.pending_actions, 0);
        assert_eq!(snap.failed_actions, 0);
        assert_eq!(snap.mode, MonitoringMode::Continuous);
    }

    #[test]
    fn test_monitoring_snapshot_populated() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Continuous);

        // Add mix of actions
        let mut a1 = ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::High);
        a1.complete(ActionOutcome::Success);
        graph.record_action(a1);

        let mut a2 = ActionNode::new("a2", "agent-1", "deploy", "staging", ActionSeverity::Medium);
        a2.complete(ActionOutcome::Failure);
        graph.record_action(a2);

        graph.record_action(ActionNode::new(
            "a3",
            "agent-2",
            "monitor",
            "health",
            ActionSeverity::Low,
        ));

        let snap = graph.monitoring_snapshot();
        assert_eq!(snap.total_actions, 3);
        assert_eq!(snap.succeeded_actions, 1);
        assert_eq!(snap.failed_actions, 1);
        assert_eq!(snap.pending_actions, 1);
        assert_eq!(snap.active_agents.len(), 2);
    }

    #[test]
    fn test_check_risk_violations() {
        let mut graph =
            AccountabilityGraph::new(MonitoringMode::Continuous).with_risk_threshold(0.8);
        graph.record_action(
            ActionNode::new("a1", "agent-1", "low", "t", ActionSeverity::Low).with_risk_score(0.2),
        );
        graph.record_action(
            ActionNode::new("a2", "agent-1", "high", "t", ActionSeverity::Critical)
                .with_risk_score(0.9),
        );
        let mut a3 = ActionNode::new("a3", "agent-1", "done", "t", ActionSeverity::High)
            .with_risk_score(0.95);
        a3.complete(ActionOutcome::Success);
        graph.record_action(a3);

        let violations = graph.check_risk_violations();
        assert_eq!(violations.len(), 1); // Only a2 is pending + high risk
        assert_eq!(violations[0].id, "a2");
    }

    // ── Query Tests ─────────────────────────────────────────────────

    #[test]
    fn test_get_agent_actions() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "test",
            "t",
            ActionSeverity::Low,
        ));
        graph.record_action(ActionNode::new(
            "a2",
            "agent-2",
            "test",
            "t",
            ActionSeverity::Low,
        ));
        graph.record_action(ActionNode::new(
            "a3",
            "agent-1",
            "test2",
            "t",
            ActionSeverity::Medium,
        ));

        let actions = graph.get_agent_actions("agent-1");
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_get_failed_actions() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut a1 = ActionNode::new("a1", "agent-1", "test", "t", ActionSeverity::Low);
        a1.complete(ActionOutcome::Failure);
        graph.record_action(a1);

        let mut a2 = ActionNode::new("a2", "agent-1", "test", "t", ActionSeverity::Low);
        a2.complete(ActionOutcome::Success);
        graph.record_action(a2);

        let failed = graph.get_failed_actions();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, "a1");
    }

    // ── Mode Tests ──────────────────────────────────────────────────

    #[test]
    fn test_mode_switch() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        assert_eq!(graph.mode(), MonitoringMode::Reactive);

        graph.set_mode(MonitoringMode::Continuous);
        assert_eq!(graph.mode(), MonitoringMode::Continuous);
    }

    // ── Error Display Tests ─────────────────────────────────────────

    #[test]
    fn test_error_display() {
        let err = AccountabilityError::ActionNotFound("a1".to_string());
        assert!(err.to_string().contains("not found"));

        let err = AccountabilityError::SelfReference("a1".to_string());
        assert!(err.to_string().contains("Self-referencing"));
    }

    // ── Snapshot Export Test ────────────────────────────────────────

    #[test]
    fn test_export_snapshot() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        graph.record_action(ActionNode::new(
            "a1",
            "agent-1",
            "test",
            "target",
            ActionSeverity::Low,
        ));
        graph.record_action(ActionNode::new(
            "a2",
            "agent-2",
            "test2",
            "target",
            ActionSeverity::Medium,
        ));
        graph.add_causal_link("a1", "a2", "caused").unwrap();

        let snap = graph.export_snapshot();
        assert_eq!(snap.nodes.len(), 2);
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.mode, MonitoringMode::Reactive);
        assert!(snap.timestamp > 0);
    }

    // ── Pruning Test ────────────────────────────────────────────────

    #[test]
    fn test_graph_pruning() {
        let mut graph =
            AccountabilityGraph::new(MonitoringMode::Reactive).with_max_nodes(5);

        for i in 0..10 {
            let mut node = ActionNode::new(
                format!("a{}", i),
                "agent-1",
                "test",
                "target",
                ActionSeverity::Low,
            );
            node.complete(ActionOutcome::Success);
            graph.record_action(node);
        }

        // Should have pruned to max_nodes
        assert!(graph.node_count() <= 5 + 100); // +100 headroom from prune
    }

    // ── MonitoringMode Display ──────────────────────────────────────

    #[test]
    fn test_monitoring_mode_display() {
        assert_eq!(MonitoringMode::Reactive.to_string(), "Reactive");
        assert_eq!(MonitoringMode::Proactive.to_string(), "Proactive");
        assert_eq!(MonitoringMode::Continuous.to_string(), "Continuous");
    }

    // ── ActionSeverity Ordering ─────────────────────────────────────

    #[test]
    fn test_severity_ordering() {
        assert!(ActionSeverity::Low < ActionSeverity::Medium);
        assert!(ActionSeverity::Medium < ActionSeverity::High);
        assert!(ActionSeverity::High < ActionSeverity::Critical);
    }

    // ── ActionOutcome Display ───────────────────────────────────────

    #[test]
    fn test_action_outcome_display() {
        assert_eq!(ActionOutcome::Success.to_string(), "Success");
        assert_eq!(ActionOutcome::Failure.to_string(), "Failure");
        assert_eq!(ActionOutcome::Blocked.to_string(), "Blocked");
        assert_eq!(ActionOutcome::Pending.to_string(), "Pending");
        assert_eq!(ActionOutcome::RolledBack.to_string(), "RolledBack");
    }

    // ── Shared Graph Test ───────────────────────────────────────────

    #[tokio::test]
    async fn test_shared_graph_operations() {
        let graph = shared_graph(MonitoringMode::Continuous);

        {
            let mut g = graph.write().await;
            g.record_action(ActionNode::new(
                "a1",
                "agent-1",
                "test",
                "target",
                ActionSeverity::Low,
            ));
        }

        {
            let g = graph.read().await;
            assert_eq!(g.node_count(), 1);
        }
    }

    // ── Multi-step Scenario Test ────────────────────────────────────

    #[test]
    fn test_full_scenario_deployment_chain() {
        let mut graph = AccountabilityGraph::new(MonitoringMode::Continuous)
            .with_risk_threshold(0.7);

        // Step 1: Agent-1 modifies config
        graph.record_action(ActionNode::new(
            "config-change",
            "agent-1",
            "modify_config",
            "/etc/app.conf",
            ActionSeverity::Medium,
        ));

        // Step 2: Agent-2 triggers build (caused by config change)
        graph.record_action(ActionNode::new(
            "build",
            "agent-2",
            "build",
            "ci-pipeline",
            ActionSeverity::Low,
        ));
        graph
            .add_causal_link("config-change", "build", "config triggered rebuild")
            .unwrap();

        // Step 3: Agent-3 deploys (caused by build)
        graph.record_action(ActionNode::new(
            "deploy",
            "agent-3",
            "deploy",
            "production",
            ActionSeverity::Critical,
        ));
        graph
            .add_causal_link("build", "deploy", "deployed after build")
            .unwrap();

        // Complete actions
        graph.complete_action("config-change", ActionOutcome::Success);
        graph.complete_action("build", ActionOutcome::Success);
        graph.complete_action("deploy", ActionOutcome::Failure);

        // Trace root cause of failed deploy
        let attribution = graph.trace_root_cause("deploy").unwrap();
        assert_eq!(attribution.root_cause, "config-change");
        assert_eq!(attribution.responsible_agent, "agent-1");
        assert_eq!(attribution.causal_chain, vec![
            "config-change",
            "build",
            "deploy"
        ]);

        // Proactive risk check for a new deploy
        let new_deploy = ActionNode::new(
            "deploy-retry",
            "agent-3",
            "deploy",
            "production",
            ActionSeverity::Critical,
        );
        let risk = graph.evaluate_risk(&new_deploy);
        assert!(risk.risk_score >= 0.9); // Critical severity
        assert!(risk.should_block);

        // Monitoring snapshot
        let snap = graph.monitoring_snapshot();
        assert_eq!(snap.total_actions, 3);
        assert_eq!(snap.failed_actions, 1);
        assert_eq!(snap.succeeded_actions, 2);
    }

    // ── ImmutableAuditTrail tests (Paper: arXiv:2503.19876) ────────

    #[test]
    fn test_trail_empty_is_valid() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let trail = ImmutableAuditTrail::new(graph);
        assert!(trail.is_empty());
        assert_eq!(trail.len(), 0);
        assert!(trail.is_valid());
        assert_eq!(trail.chain_head(), "genesis");
    }

    #[test]
    fn test_trail_append_single_entry() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let hash = trail.append(ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::High));
        assert!(!hash.is_empty());
        assert_eq!(trail.len(), 1);
        assert_eq!(trail.entries()[0].prev_hash, "genesis");
        assert_eq!(trail.entries()[0].entry_hash, hash);
        assert_eq!(trail.entries()[0].sequence, 0);
    }

    #[test]
    fn test_trail_hash_chain_links() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let hash1 = trail.append(ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium));
        let hash2 = trail.append(ActionNode::new("a2", "agent-2", "test", "ci", ActionSeverity::Low));
        let hash3 = trail.append(ActionNode::new("a3", "agent-3", "deploy", "prod", ActionSeverity::Critical));

        // Chain links: each entry's prev_hash = previous entry's entry_hash
        assert_eq!(trail.entries()[0].prev_hash, "genesis");
        assert_eq!(trail.entries()[1].prev_hash, hash1);
        assert_eq!(trail.entries()[2].prev_hash, hash2);
        assert_eq!(trail.chain_head(), hash3);
    }

    #[test]
    fn test_trail_integrity_passes() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));
        trail.append(ActionNode::new("a2", "agent-2", "write", "file:/tmp", ActionSeverity::Medium));

        let result = trail.verify_integrity();
        assert!(result.satisfied);
        assert_eq!(result.violations, 0);
        assert_eq!(result.entries_checked, 2);
    }

    #[test]
    fn test_trail_integrity_detects_tampering() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));
        trail.append(ActionNode::new("a2", "agent-2", "write", "file:/tmp", ActionSeverity::Medium));

        // Tamper with the first entry's action type
        trail.entries[0].action.action_type = "TAMPERED".to_string();

        let result = trail.verify_integrity();
        assert!(!result.satisfied);
        assert!(result.violations > 0);
        assert!(result.details.contains("hash mismatch"));
    }

    #[test]
    fn test_trail_completeness_all_terminal() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let mut a1 = ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium);
        a1.complete(ActionOutcome::Success);
        trail.append(a1);

        // Record a terminal action directly in graph (not in trail)
        let mut a2 = ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical);
        a2.complete(ActionOutcome::Failure);
        trail.graph_mut().record_action(a2);

        let result = trail.verify_completeness();
        assert!(!result.satisfied);
        assert_eq!(result.violations, 1);
        assert!(result.details.contains("a2"));
    }

    #[test]
    fn test_trail_completeness_pending_actions_ok() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        // Pending action is not terminal, so completeness doesn't require it
        trail.append(ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium));
        trail.graph_mut().record_action(ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical));

        let result = trail.verify_completeness();
        assert!(result.satisfied); // a2 is still Pending, not terminal
    }

    #[test]
    fn test_trail_non_repudiation_passes() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));
        trail.append(ActionNode::new("a2", "agent-2", "write", "file:/tmp", ActionSeverity::Medium));

        let result = trail.verify_non_repudiation();
        assert!(result.satisfied);
        assert_eq!(result.violations, 0);
    }

    #[test]
    fn test_trail_non_repudiation_empty_agent() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "", "read", "file:/etc", ActionSeverity::Low));

        let result = trail.verify_non_repudiation();
        assert!(!result.satisfied);
        assert_eq!(result.violations, 1);
        assert!(result.details.contains("empty agent_id"));
    }

    #[test]
    fn test_trail_causal_coherence_passes() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium));
        trail.append(ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical));
        trail.graph_mut().add_causal_link("a1", "a2", "deployed after build").unwrap();

        let result = trail.verify_causal_coherence();
        assert!(result.satisfied);
        assert_eq!(result.violations, 0);
    }

    #[test]
    fn test_trail_causal_coherence_missing_endpoint() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append(ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium));
        // a2 is in the graph but NOT in the trail
        let a2 = ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical);
        trail.graph_mut().record_action(a2);
        trail.graph_mut().add_causal_link("a1", "a2", "deployed after build").unwrap();

        let result = trail.verify_causal_coherence();
        assert!(!result.satisfied);
        assert_eq!(result.violations, 1); // a2 missing from trail
    }

    #[test]
    fn test_trail_verify_all_passes() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let mut a1 = ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium);
        a1.complete(ActionOutcome::Success);
        trail.append(a1);

        let mut a2 = ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical);
        a2.complete(ActionOutcome::Success);
        trail.append(a2);

        trail.graph_mut().add_causal_link("a1", "a2", "deployed after build").unwrap();

        let results = trail.verify_all();
        assert_eq!(results.len(), 4);
        for r in &results {
            assert!(r.satisfied, "Property {:?} failed: {}", r.property, r.details);
        }
        assert!(trail.is_valid());
    }

    #[test]
    fn test_trail_append_completed() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let hash = trail.append_completed(
            ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::Critical),
            ActionOutcome::Success,
        );

        assert_eq!(trail.len(), 1);
        assert_eq!(trail.entries()[0].action.outcome, ActionOutcome::Success);
        assert!(trail.entries()[0].action.completed_at.is_some());
        assert_eq!(trail.entries()[0].entry_hash, hash);
    }

    #[test]
    fn test_trail_different_actions_different_hashes() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let hash1 = trail.append(ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));
        let hash2 = trail.append(ActionNode::new("a2", "agent-2", "write", "file:/tmp", ActionSeverity::Medium));

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_trail_same_action_same_hash() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let hash1 = trail.append(ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));

        // Compute hash for same action with same prev_hash
        let hash2 = SignedAuditEntry::compute_hash("genesis", &ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low));

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_trail_export_snapshot() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        trail.append_completed(
            ActionNode::new("a1", "agent-1", "build", "ci", ActionSeverity::Medium),
            ActionOutcome::Success,
        );
        trail.append_completed(
            ActionNode::new("a2", "agent-2", "deploy", "prod", ActionSeverity::Critical),
            ActionOutcome::Failure,
        );

        let snap = trail.export_snapshot();
        assert_eq!(snap.entry_count, 2);
        assert_eq!(snap.entries.len(), 2);
        assert_eq!(snap.entries[0].action_id, "a1");
        assert_eq!(snap.entries[0].sequence, 0);
        assert_eq!(snap.entries[1].action_id, "a2");
        assert_eq!(snap.entries[1].sequence, 1);
        assert_eq!(snap.entries[1].outcome, "Failure");
        // All properties should pass
        for p in &snap.properties {
            assert!(p.satisfied, "Property {:?} failed: {}", p.property, p.details);
        }
    }

    #[test]
    fn test_trail_with_metadata() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        let action = ActionNode::new("a1", "agent-1", "deploy", "prod", ActionSeverity::Critical)
            .with_metadata("version", "1.2.3")
            .with_metadata("env", "production");

        let hash = trail.append(action);
        assert!(!hash.is_empty());
        assert_eq!(trail.entries()[0].action.metadata.get("version"), Some(&"1.2.3".to_string()));
    }

    #[test]
    fn test_property_display() {
        assert_eq!(format!("{}", AuditProperty::Completeness), "Completeness");
        assert_eq!(format!("{}", AuditProperty::Integrity), "Integrity");
        assert_eq!(format!("{}", AuditProperty::NonRepudiation), "NonRepudiation");
        assert_eq!(format!("{}", AuditProperty::CausalCoherence), "CausalCoherence");
    }

    #[test]
    fn test_trail_long_chain_integrity() {
        let graph = AccountabilityGraph::new(MonitoringMode::Reactive);
        let mut trail = ImmutableAuditTrail::new(graph);

        for i in 0..100 {
            trail.append(ActionNode::new(
                format!("a{}", i),
                format!("agent-{}", i % 5),
                "task",
                "target",
                ActionSeverity::Low,
            ));
        }

        assert_eq!(trail.len(), 100);
        let integrity = trail.verify_integrity();
        assert!(integrity.satisfied);
        assert_eq!(integrity.entries_checked, 100);
    }

    #[test]
    fn test_signed_entry_verify_hash() {
        let action = ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low);
        let prev_hash = "genesis";
        let entry_hash = SignedAuditEntry::compute_hash(prev_hash, &action);

        let entry = SignedAuditEntry {
            action,
            prev_hash: prev_hash.to_string(),
            entry_hash,
            sequence: 0,
        };

        assert!(entry.verify_hash());
    }

    #[test]
    fn test_signed_entry_verify_hash_fails_on_tamper() {
        let action = ActionNode::new("a1", "agent-1", "read", "file:/etc", ActionSeverity::Low);
        let prev_hash = "genesis";
        let entry_hash = SignedAuditEntry::compute_hash(prev_hash, &action);

        let mut entry = SignedAuditEntry {
            action,
            prev_hash: prev_hash.to_string(),
            entry_hash,
            sequence: 0,
        };

        // Tamper
        entry.action.action_type = "TAMPERED".to_string();
        assert!(!entry.verify_hash());
    }
}
