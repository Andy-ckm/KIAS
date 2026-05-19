//! # Resource-Level Access Control Policies
//!
//! Extends the existing RBAC system (`Admin > Operator > Viewer`) with fine-grained
//! resource-type-level policies. A [`ResourcePolicy`] specifies which role can perform
//! which action on which resource type.
//!
//! The [`PolicyEngine`] evaluates a set of policies against a request context to produce
//! an [`AccessDecision`].

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;

/// A single access control policy.
///
/// Example: "Operators can Read agents but not Delete them."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourcePolicy {
    /// Unique policy ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// The role this policy applies to.
    pub role: String,
    /// The resource type this policy governs (e.g. "agent", "node", "config").
    pub resource_type: String,
    /// The action allowed by this policy (stored as string for serialization).
    pub action: String,
    /// Whether the policy grants or denies access.
    pub effect: PolicyEffect,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Whether this policy is currently active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Whether a policy grants or denies access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    /// Allow the action.
    Allow,
    /// Deny the action (overrides Allow).
    Deny,
}

impl std::fmt::Display for PolicyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Deny => write!(f, "deny"),
        }
    }
}

/// The result of evaluating a policy against a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    /// Access is granted.
    Allow,
    /// Access is denied with a reason.
    Deny(String),
}

impl AccessDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// Engine that evaluates resource-level access policies.
///
/// Policies are stored in-memory and can be updated at runtime.
/// The engine uses a "Deny overrides Allow" strategy:
/// - If any matching Deny policy exists → Deny
/// - If at least one matching Allow policy exists → Allow
/// - If no matching policies exist → Deny (default-deny)
#[derive(Debug)]
pub struct PolicyEngine {
    policies: RwLock<Vec<ResourcePolicy>>,
}

impl PolicyEngine {
    /// Create a new policy engine with no policies (default-deny for everything).
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(Vec::new()),
        }
    }

    /// Create a policy engine pre-loaded with the given policies.
    pub fn with_policies(policies: Vec<ResourcePolicy>) -> Self {
        Self {
            policies: RwLock::new(policies),
        }
    }

    /// Add a policy. Overwrites any existing policy with the same ID.
    pub async fn add_policy(&self, policy: ResourcePolicy) {
        let mut policies = self.policies.write().await;
        // Remove existing policy with same ID
        policies.retain(|p| p.id != policy.id);
        debug!(
            policy_id = %policy.id,
            role = %policy.role,
            resource = %policy.resource_type,
            action = %policy.action,
            effect = %policy.effect,
            "Added resource policy"
        );
        policies.push(policy);
    }

    /// Remove a policy by ID.
    pub async fn remove_policy(&self, id: &str) -> bool {
        let mut policies = self.policies.write().await;
        let before = policies.len();
        policies.retain(|p| p.id != id);
        policies.len() < before
    }

    /// List all policies.
    pub async fn list_policies(&self) -> Vec<ResourcePolicy> {
        self.policies.read().await.clone()
    }

    /// Evaluate access for a given role, resource type, and action.
    ///
    /// Strategy: Deny overrides Allow. Default is Deny.
    pub async fn evaluate(&self, role: &str, resource_type: &str, action: &str) -> AccessDecision {
        let policies = self.policies.read().await;
        let matching: Vec<&ResourcePolicy> = policies
            .iter()
            .filter(|p| p.enabled)
            .filter(|p| p.role == role)
            .filter(|p| p.resource_type == resource_type || p.resource_type == "*")
            .filter(|p| p.action.eq_ignore_ascii_case(action))
            .collect();

        // Check for explicit Deny first (Deny overrides Allow)
        if matching.iter().any(|p| p.effect == PolicyEffect::Deny) {
            return AccessDecision::Deny(format!(
                "Explicit deny for role={role} resource={resource_type} action={action}"
            ));
        }

        // Check for Allow
        if matching.iter().any(|p| p.effect == PolicyEffect::Allow) {
            return AccessDecision::Allow;
        }

        // Default deny
        AccessDecision::Deny(format!(
            "No matching policy for role={role} resource={resource_type} action={action}"
        ))
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default KIAS policy set.
///
/// Mirrors the RBAC hierarchy:
/// - Admin: all actions on all resources
/// - Operator: CRUD (no Delete) on agents, tasks, workflows, knowledge
/// - Viewer: Read-only on agents, tasks, workflows, knowledge
pub fn default_policies() -> Vec<ResourcePolicy> {
    let mut policies = Vec::new();
    let resource_types = ["agent", "node", "workflow", "task", "config", "knowledge"];
    let all_actions = [
        "Create",
        "Read",
        "Update",
        "Delete",
        "Schedule",
        "Execute",
        "ConfigChange",
    ];
    let operator_actions = ["Create", "Read", "Update", "Schedule", "Execute"];

    let mut id_counter = 0u32;
    let mut next_id = || {
        id_counter += 1;
        format!("pol-{id_counter:04}")
    };

    // Admin: full access to everything
    for rt in &resource_types {
        for action in &all_actions {
            policies.push(ResourcePolicy {
                id: next_id(),
                name: format!("admin-{rt}-{action}"),
                role: "Admin".to_string(),
                resource_type: rt.to_string(),
                action: action.to_string(),
                effect: PolicyEffect::Allow,
                description: format!("Admin can {action} {rt}"),
                enabled: true,
            });
        }
    }

    // Operator: read + create + update (no delete) on most resources
    for rt in &resource_types {
        for action in &operator_actions {
            policies.push(ResourcePolicy {
                id: next_id(),
                name: format!("operator-{rt}-{action}"),
                role: "Operator".to_string(),
                resource_type: rt.to_string(),
                action: action.to_string(),
                effect: PolicyEffect::Allow,
                description: format!("Operator can {action} {rt}"),
                enabled: true,
            });
        }
        // Deny delete for operators
        policies.push(ResourcePolicy {
            id: next_id(),
            name: format!("operator-{rt}-Delete-deny"),
            role: "Operator".to_string(),
            resource_type: rt.to_string(),
            action: "Delete".to_string(),
            effect: PolicyEffect::Deny,
            description: format!("Operator cannot delete {rt}"),
            enabled: true,
        });
    }

    // Viewer: read-only
    for rt in &resource_types {
        policies.push(ResourcePolicy {
            id: next_id(),
            name: format!("viewer-{rt}-Read"),
            role: "Viewer".to_string(),
            resource_type: rt.to_string(),
            action: "Read".to_string(),
            effect: PolicyEffect::Allow,
            description: format!("Viewer can read {rt}"),
            enabled: true,
        });
    }

    policies
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_policy(
        id: &str,
        role: &str,
        resource: &str,
        action: &str,
        effect: PolicyEffect,
    ) -> ResourcePolicy {
        ResourcePolicy {
            id: id.to_string(),
            name: format!("{id}-name"),
            role: role.to_string(),
            resource_type: resource.to_string(),
            action: action.to_string(),
            effect,
            description: String::new(),
            enabled: true,
        }
    }

    #[tokio::test]
    async fn test_allow_policy() {
        let engine = PolicyEngine::with_policies(vec![make_policy(
            "p1",
            "Admin",
            "agent",
            "Read",
            PolicyEffect::Allow,
        )]);

        let decision = engine.evaluate("Admin", "agent", "Read").await;
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_overrides_allow() {
        let engine = PolicyEngine::with_policies(vec![
            make_policy("p1", "Operator", "agent", "Delete", PolicyEffect::Allow),
            make_policy("p2", "Operator", "agent", "Delete", PolicyEffect::Deny),
        ]);

        let decision = engine.evaluate("Operator", "agent", "Delete").await;
        assert!(!decision.is_allowed());
    }

    #[tokio::test]
    async fn test_default_deny_no_matching_policy() {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate("Viewer", "agent", "Create").await;
        assert!(!decision.is_allowed());
    }

    #[tokio::test]
    async fn test_wildcard_resource_type() {
        let engine = PolicyEngine::with_policies(vec![make_policy(
            "p1",
            "Admin",
            "*",
            "Read",
            PolicyEffect::Allow,
        )]);

        let decision = engine.evaluate("Admin", "agent", "Read").await;
        assert!(decision.is_allowed());

        let decision2 = engine.evaluate("Admin", "workflow", "Read").await;
        assert!(decision2.is_allowed());
    }

    #[tokio::test]
    async fn test_disabled_policy_ignored() {
        let mut policy = make_policy("p1", "Admin", "agent", "Read", PolicyEffect::Allow);
        policy.enabled = false;

        let engine = PolicyEngine::with_policies(vec![policy]);
        let decision = engine.evaluate("Admin", "agent", "Read").await;
        assert!(!decision.is_allowed());
    }

    #[tokio::test]
    async fn test_add_and_remove_policy() {
        let engine = PolicyEngine::new();
        let policy = make_policy("p1", "Admin", "agent", "Read", PolicyEffect::Allow);

        engine.add_policy(policy).await;
        assert!(engine.evaluate("Admin", "agent", "Read").await.is_allowed());

        assert!(engine.remove_policy("p1").await);
        assert!(!engine.evaluate("Admin", "agent", "Read").await.is_allowed());
    }

    #[tokio::test]
    async fn test_default_policies_admin_full_access() {
        let engine = PolicyEngine::with_policies(default_policies());

        // Admin can do everything
        for action in ["Create", "Read", "Update", "Delete"] {
            assert!(
                engine.evaluate("Admin", "agent", action).await.is_allowed(),
                "Admin should be able to {action} agents"
            );
        }
    }

    #[tokio::test]
    async fn test_default_policies_viewer_read_only() {
        let engine = PolicyEngine::with_policies(default_policies());

        assert!(engine
            .evaluate("Viewer", "agent", "Read")
            .await
            .is_allowed());
        assert!(!engine
            .evaluate("Viewer", "agent", "Create")
            .await
            .is_allowed());
        assert!(!engine
            .evaluate("Viewer", "agent", "Delete")
            .await
            .is_allowed());
    }

    #[tokio::test]
    async fn test_default_policies_operator_no_delete() {
        let engine = PolicyEngine::with_policies(default_policies());

        assert!(engine
            .evaluate("Operator", "agent", "Read")
            .await
            .is_allowed());
        assert!(engine
            .evaluate("Operator", "agent", "Create")
            .await
            .is_allowed());
        assert!(!engine
            .evaluate("Operator", "agent", "Delete")
            .await
            .is_allowed());
    }

    #[test]
    fn test_policy_effect_display() {
        assert_eq!(PolicyEffect::Allow.to_string(), "allow");
        assert_eq!(PolicyEffect::Deny.to_string(), "deny");
    }

    #[test]
    fn test_default_policies_count() {
        let policies = default_policies();
        // 6 resources × 7 actions for admin + 6 resources × 6 actions for operator + 6 resources for viewer
        let expected = 6 * 7 + 6 * 6 + 6;
        assert_eq!(policies.len(), expected);
    }

    #[test]
    fn test_policy_serde_roundtrip() {
        let policy = make_policy("p1", "Admin", "agent", "Read", PolicyEffect::Allow);
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ResourcePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, deserialized);
    }

    #[test]
    fn test_access_decision_is_allowed() {
        assert!(AccessDecision::Allow.is_allowed());
        assert!(!AccessDecision::Deny("reason".to_string()).is_allowed());
    }

    #[test]
    fn test_access_decision_debug() {
        let d = AccessDecision::Deny("test reason".to_string());
        let debug = format!("{:?}", d);
        assert!(debug.contains("test reason"));
    }

    #[tokio::test]
    async fn test_policy_engine_list_policies() {
        let engine = PolicyEngine::new();
        assert!(engine.list_policies().await.is_empty());

        engine
            .add_policy(make_policy("p1", "Admin", "agent", "Read", PolicyEffect::Allow))
            .await;
        assert_eq!(engine.list_policies().await.len(), 1);
    }

    #[tokio::test]
    async fn test_policy_engine_remove_nonexistent() {
        let engine = PolicyEngine::new();
        assert!(!engine.remove_policy("nonexistent").await);
    }

    #[tokio::test]
    async fn test_evaluate_case_insensitive_action() {
        let engine = PolicyEngine::with_policies(vec![make_policy(
            "p1",
            "Admin",
            "agent",
            "Read",
            PolicyEffect::Allow,
        )]);

        // Should match regardless of case
        assert!(engine.evaluate("Admin", "agent", "read").await.is_allowed());
        assert!(engine.evaluate("Admin", "agent", "READ").await.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_overrides_allow_both_present() {
        let engine = PolicyEngine::with_policies(vec![
            make_policy("p1", "Admin", "agent", "Read", PolicyEffect::Allow),
            make_policy("p2", "Admin", "agent", "Read", PolicyEffect::Deny),
        ]);

        let decision = engine.evaluate("Admin", "agent", "Read").await;
        assert!(!decision.is_allowed());
    }

    #[tokio::test]
    async fn test_evaluate_unknown_role() {
        let engine = PolicyEngine::with_policies(default_policies());
        let decision = engine.evaluate("UnknownRole", "agent", "Read").await;
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_resource_policy_default_enabled() {
        let json = r#"{"id":"p1","name":"test","role":"Admin","resource_type":"agent","action":"Read","effect":"allow"}"#;
        let policy: ResourcePolicy = serde_json::from_str(json).unwrap();
        assert!(policy.enabled); // default_enabled should be true
        assert!(policy.description.is_empty());
    }

    #[tokio::test]
    async fn test_default_policies_operator_all_resources() {
        let engine = PolicyEngine::with_policies(default_policies());
        // Operator can Read all resource types
        for rt in &["agent", "node", "workflow", "task", "config", "knowledge"] {
            assert!(
                engine.evaluate("Operator", rt, "Read").await.is_allowed(),
                "Operator should be able to Read {rt}"
            );
        }
    }
}
