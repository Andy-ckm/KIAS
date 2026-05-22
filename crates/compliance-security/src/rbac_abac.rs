//! # RBAC/ABAC Dual-Model Permission System
//!
//! Provides both Role-Based Access Control (RBAC) and Attribute-Based Access Control (ABAC)
//! with a unified PolicyEvaluator.

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── RBAC: Roles & Permissions ─────────────────────────────────────────────────

/// A named permission
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Permission {
    pub resource: String,
    pub action: String,
    pub description: String,
}

impl Permission {
    pub fn new(resource: &str, action: &str) -> Self {
        Self { resource: resource.to_string(), action: action.to_string(), description: format!("{}:{}", resource, action) }
    }
}

/// A role with a set of permissions
#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    pub permissions: HashSet<Permission>,
    pub inherits_from: Vec<String>,
}

impl Role {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), permissions: HashSet::new(), inherits_from: Vec::new() }
    }
    pub fn add_permission(&mut self, permission: Permission) { self.permissions.insert(permission); }
    pub fn inherit(&mut self, role_name: &str) { self.inherits_from.push(role_name.to_string()); }
}

// ── ABAC: Attributes ─────────────────────────────────────────────────────────

/// Subject attributes (who is making the request)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubjectAttributes {
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub roles: Vec<String>,
    pub department: Option<String>,
    pub clearance_level: Option<u32>,
    pub custom: HashMap<String, String>,
}

/// Resource attributes (what is being accessed)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceAttributes {
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub owner_tenant: Option<String>,
    pub sensitivity_level: Option<u32>,
    pub tags: HashMap<String, String>,
    pub custom: HashMap<String, String>,
}

/// Action attributes (what action is being performed)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionAttributes {
    pub action_type: String,
    pub is_read: bool,
    pub is_write: bool,
    pub is_delete: bool,
    pub is_admin: bool,
}

/// Context attributes (environmental conditions)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextAttributes {
    pub ip_address: Option<String>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub location: Option<String>,
    pub device_trust: Option<f32>,
}

/// Combined ABAC request context
#[derive(Debug, Clone, Default)]
pub struct AbacContext {
    pub subject: SubjectAttributes,
    pub resource: ResourceAttributes,
    pub action: ActionAttributes,
    pub environment: ContextAttributes,
}

impl AbacContext {
    pub fn new(subject: SubjectAttributes, resource: ResourceAttributes, action: ActionAttributes, environment: ContextAttributes) -> Self {
        Self { subject, resource, action, environment }
    }
}

// ── Policy ─────────────────────────────────────────────────────────────────

/// An ABAC policy rule
#[derive(Debug, Clone)]
pub struct AbacPolicy {
    pub name: String,
    pub effect: PolicyEffect,
    pub conditions: Vec<PolicyCondition>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyEffect { Allow, Deny }

#[derive(Debug, Clone)]
pub enum PolicyCondition {
    SubjectAttrEq { key: String, value: String },
    SubjectAttrGt { key: String, value: u32 },
    ResourceTypeEq { value: String },
    ResourceOwnerEq { value: String },
    ResourceTagExists { key: String },
    ActionTypeEq { value: String },
    EnvironmentIpIn { cidr: String },
    EnvironmentTrustGt { value: f32 },
    Custom { key: String, operator: String, value: String },
}

impl AbacPolicy {
    pub fn new_allow(name: &str) -> Self {
        Self { name: name.to_string(), effect: PolicyEffect::Allow, conditions: Vec::new(), description: String::new() }
    }
    pub fn with_condition(mut self, cond: PolicyCondition) -> Self { self.conditions.push(cond); self }
    pub fn deny(name: &str) -> Self {
        Self { name: name.to_string(), effect: PolicyEffect::Deny, conditions: Vec::new(), description: String::new() }
    }
}

// ── RBAC Checker ─────────────────────────────────────────────────────────────

pub struct RbacChecker {
    roles: HashMap<String, Role>,
    user_roles: HashMap<String, HashSet<String>>,
}

impl Default for RbacChecker {
    fn default() -> Self { Self::new() }
}

impl RbacChecker {
    pub fn new() -> Self {
        Self { roles: HashMap::new(), user_roles: HashMap::new() }
    }
    pub fn register_role(&mut self, role: Role) { self.roles.insert(role.name.clone(), role); }
    pub fn assign_role(&mut self, user_id: &str, role_name: &str) { self.user_roles.entry(user_id.to_string()).or_insert_with(HashSet::new).insert(role_name.to_string()); }

    pub fn check_permission(&self, user_id: &str, resource: &str, action: &str) -> bool {
        let roles = match self.user_roles.get(user_id) {
            Some(r) => r,
            None => return false,
        };
        let perm = Permission::new(resource, action);
        for role_name in roles {
            if let Some(role) = self.roles.get(role_name) {
                if role.permissions.contains(&perm) { return true; }
                // Check inherited roles
                for inherited in &role.inherits_from {
                    if let Some(inherited_role) = self.roles.get(inherited) {
                        if inherited_role.permissions.contains(&perm) { return true; }
                    }
                }
            }
        }
        false
    }

    pub fn get_user_permissions(&self, user_id: &str) -> HashSet<Permission> {
        let mut perms = HashSet::new();
        let roles = match self.user_roles.get(user_id) {
            Some(r) => r,
            None => return perms,
        };
        for role_name in roles {
            if let Some(role) = self.roles.get(role_name) {
                perms.extend(role.permissions.clone());
                for inherited in &role.inherits_from {
                    if let Some(inherited_role) = self.roles.get(inherited) {
                        perms.extend(inherited_role.permissions.clone());
                    }
                }
            }
        }
        perms
    }
}

// ── ABAC Evaluator ──────────────────────────────────────────────────────────

pub struct AbacEvaluator {
    policies: Vec<AbacPolicy>,
}

impl Default for AbacEvaluator {
    fn default() -> Self { Self::new() }
}

impl AbacEvaluator {
    pub fn new() -> Self { Self { policies: Vec::new() } }
    pub fn add_policy(&mut self, policy: AbacPolicy) { self.policies.push(policy); }

    pub fn evaluate(&self, ctx: &AbacContext) -> bool {
        for policy in &self.policies {
            if self.policy_matches(policy, ctx) {
                return policy.effect == PolicyEffect::Allow;
            }
        }
        false // Default deny
    }

    fn policy_matches(&self, policy: &AbacPolicy, ctx: &AbacContext) -> bool {
        for cond in &policy.conditions {
            if !self.condition_matches(cond, ctx) { return false; }
        }
        true
    }

    fn condition_matches(&self, cond: &PolicyCondition, ctx: &AbacContext) -> bool {
        match cond {
            PolicyCondition::SubjectAttrEq { key, value } => {
                ctx.subject.custom.get(key).map(|v| v == value).unwrap_or(false)
            }
            PolicyCondition::SubjectAttrGt { key, value } => {
                ctx.subject.custom.get(key).and_then(|v| v.parse::<u32>().ok()).map(|v| v > *value).unwrap_or(false)
            }
            PolicyCondition::ResourceTypeEq { value } => ctx.resource.resource_type == *value,
            PolicyCondition::ResourceOwnerEq { value } => ctx.resource.owner_tenant.as_ref().map(|t| t == value).unwrap_or(false),
            PolicyCondition::ResourceTagExists { key } => ctx.resource.tags.contains_key(key),
            PolicyCondition::ActionTypeEq { value } => ctx.action.action_type == *value,
            PolicyCondition::EnvironmentIpIn { .. } => true, // Simplified
            PolicyCondition::EnvironmentTrustGt { value } => ctx.environment.device_trust.map(|t| t > *value).unwrap_or(false),
            PolicyCondition::Custom { .. } => true, // Custom conditions require implementation
        }
    }
}

// ── Unified Policy Evaluator ─────────────────────────────────────────────────

pub struct PolicyEvaluator {
    rbac: RbacChecker,
    abac: AbacEvaluator,
    default_effect: PolicyEffect,
}

impl Default for PolicyEvaluator {
    fn default() -> Self { Self::new() }
}

impl PolicyEvaluator {
    pub fn new() -> Self {
        Self { rbac: RbacChecker::new(), abac: AbacEvaluator::new(), default_effect: PolicyEffect::Deny }
    }
    pub fn with_default_allow(mut self) -> Self { self.default_effect = PolicyEffect::Allow; self }

    // RBAC interface
    pub fn rbac_checker_mut(&mut self) -> &mut RbacChecker { &mut self.rbac }
    pub fn rbac_checker(&self) -> &RbacChecker { &self.rbac }

    // ABAC interface
    pub fn abac_evaluator_mut(&mut self) -> &mut AbacEvaluator { &mut self.abac }
    pub fn abac_evaluator(&self) -> &AbacEvaluator { &self.abac }

    /// Check access using both RBAC and ABAC
    pub fn check(&self, ctx: &AbacContext, resource: &str, action: &str) -> AccessDecision {
        // RBAC check if user_id present
        if let Some(user_id) = &ctx.subject.user_id {
            if self.rbac.check_permission(user_id, resource, action) {
                return AccessDecision::Allowed { reason: "RBAC".to_string() };
            }
        }

        // ABAC check
        if self.abac.evaluate(ctx) {
            return AccessDecision::Allowed { reason: "ABAC".to_string() };
        }

        match self.default_effect {
            PolicyEffect::Allow => AccessDecision::Allowed { reason: "default".to_string() },
            PolicyEffect::Deny => AccessDecision::Denied { reason: "no matching policy".to_string() },
        }
    }

    /// Simple RBAC-only check
    pub fn check_rbac(&self, user_id: &str, resource: &str, action: &str) -> AccessDecision {
        if self.rbac.check_permission(user_id, resource, action) {
            AccessDecision::Allowed { reason: "RBAC".to_string() }
        } else {
            AccessDecision::Denied { reason: "insufficient permissions".to_string() }
        }
    }

    /// Simple ABAC-only check
    pub fn check_abac(&self, ctx: &AbacContext) -> AccessDecision {
        if self.abac.evaluate(ctx) {
            AccessDecision::Allowed { reason: "ABAC".to_string() }
        } else {
            AccessDecision::Denied { reason: "ABAC policy denied".to_string() }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessDecision {
    Allowed { reason: String },
    Denied { reason: String },
}

impl AccessDecision {
    pub fn is_allowed(&self) -> bool { matches!(self, AccessDecision::Allowed { .. }) }
    pub fn is_denied(&self) -> bool { matches!(self, AccessDecision::Denied { .. }) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rbac_basic() {
        let mut checker = RbacChecker::new();
        let mut admin = Role::new("admin");
        admin.add_permission(Permission::new("agents", "read"));
        admin.add_permission(Permission::new("agents", "write"));
        admin.add_permission(Permission::new("agents", "delete"));
        checker.register_role(admin);
        checker.assign_role("user1", "admin");

        assert!(checker.check_permission("user1", "agents", "read"));
        assert!(checker.check_permission("user1", "agents", "delete"));
        assert!(!checker.check_permission("user1", "agents", "not_exist"));
        assert!(!checker.check_permission("unknown_user", "agents", "read"));
    }

    #[test]
    fn test_rbac_role_inheritance() {
        let mut checker = RbacChecker::new();
        let mut viewer = Role::new("viewer");
        viewer.add_permission(Permission::new("agents", "read"));
        let mut editor = Role::new("editor");
        editor.add_permission(Permission::new("agents", "write"));
        editor.inherit("viewer");
        checker.register_role(viewer);
        checker.register_role(editor);
        checker.assign_role("user1", "editor");

        assert!(checker.check_permission("user1", "agents", "read")); // inherited
        assert!(checker.check_permission("user1", "agents", "write")); // direct
        assert!(!checker.check_permission("user1", "agents", "delete"));
    }

    #[test]
    fn test_abac_basic() {
        let mut evaluator = AbacEvaluator::new();
        evaluator.add_policy(AbacPolicy::new_allow("allow_tenant_access")
            .with_condition(PolicyCondition::ResourceOwnerEq { value: "tenant_a".to_string() })
            .with_condition(PolicyCondition::ActionTypeEq { value: "read".to_string() }));

        let ctx = AbacContext {
            subject: SubjectAttributes { user_id: Some("u1".to_string()), tenant_id: Some("tenant_a".to_string()), ..Default::default() },
            resource: ResourceAttributes { resource_type: "document".to_string(), owner_tenant: Some("tenant_a".to_string()), ..Default::default() },
            action: ActionAttributes { action_type: "read".to_string(), is_read: true, ..Default::default() },
            environment: ContextAttributes::default(),
        };

        assert!(evaluator.evaluate(&ctx));
    }

    #[test]
    fn test_abac_deny() {
        let mut evaluator = AbacEvaluator::new();
        evaluator.add_policy(AbacPolicy::deny("deny_high_security")
            .with_condition(PolicyCondition::SubjectAttrGt { key: "clearance".to_string(), value: 3 }));

        let mut ctx = AbacContext::default();
        ctx.subject.custom.insert("clearance".to_string(), "5".to_string());
        ctx.resource.resource_type = "secure_doc".to_string();

        assert!(!evaluator.evaluate(&ctx)); // deny matches
    }

    #[test]
    fn test_policy_evaluator_rbac_abac() {
        let mut evaluator = PolicyEvaluator::new();
        let mut admin = Role::new("admin");
        admin.add_permission(Permission::new("agents", "read"));
        evaluator.rbac_checker_mut().register_role(admin);
        evaluator.rbac_checker_mut().assign_role("admin_user", "admin");

        // RBAC allows
        let ctx = AbacContext {
            subject: SubjectAttributes { user_id: Some("admin_user".to_string()), ..Default::default() },
            resource: ResourceAttributes::default(),
            action: ActionAttributes::default(),
            environment: ContextAttributes::default(),
        };
        assert!(evaluator.check(&ctx, "agents", "read").is_allowed());
    }

    #[test]
    fn test_access_decision() {
        let allowed = AccessDecision::Allowed { reason: "test".to_string() };
        let denied = AccessDecision::Denied { reason: "test".to_string() };
        assert!(allowed.is_allowed());
        assert!(!allowed.is_denied());
        assert!(denied.is_denied());
        assert!(!denied.is_allowed());
    }

    #[test]
    fn test_abac_evaluator_default_deny() {
        let evaluator = AbacEvaluator::new();
        let ctx = AbacContext::default();
        assert!(!evaluator.evaluate(&ctx)); // No policies = deny
    }

    #[test]
    fn test_policy_evaluator_default_deny() {
        let evaluator = PolicyEvaluator::new();
        let ctx = AbacContext {
            subject: SubjectAttributes { user_id: Some("nobody".to_string()), ..Default::default() },
            resource: ResourceAttributes::default(),
            action: ActionAttributes::default(),
            environment: ContextAttributes::default(),
        };
        assert!(evaluator.check(&ctx, "any", "any").is_denied());
    }
}
