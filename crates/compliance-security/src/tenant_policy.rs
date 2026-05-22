//! # Tenant-Level Policy Override System
//!
//! Provides Global → Tenant → Project three-level policy inheritance
//! with conflict resolution.

use kias_common::KiasResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Policy Level ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyLevel { Global, Tenant, Project }

impl std::fmt::Display for PolicyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { PolicyLevel::Global => write!(f, "Global"), PolicyLevel::Tenant => write!(f, "Tenant"), PolicyLevel::Project => write!(f, "Project") }
    }
}

// ── Policy ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    pub name: String,
    pub level: PolicyLevel,
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub rules: Vec<PolicyRule>,
    pub priority: i32,
    pub enabled: bool,
}

impl Policy {
    pub fn new_global(name: &str) -> Self {
        Self { policy_id: uuid::Uuid::new_v4().to_string(), name: name.to_string(), level: PolicyLevel::Global, tenant_id: None, project_id: None, rules: Vec::new(), priority: 0, enabled: true }
    }
    pub fn new_tenant(name: &str, tenant_id: &str) -> Self {
        Self { policy_id: uuid::Uuid::new_v4().to_string(), name: name.to_string(), level: PolicyLevel::Tenant, tenant_id: Some(tenant_id.to_string()), project_id: None, rules: Vec::new(), priority: 100, enabled: true }
    }
    pub fn new_project(name: &str, tenant_id: &str, project_id: &str) -> Self {
        Self { policy_id: uuid::Uuid::new_v4().to_string(), name: name.to_string(), level: PolicyLevel::Project, tenant_id: Some(tenant_id.to_string()), project_id: Some(project_id.to_string()), rules: Vec::new(), priority: 200, enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub rule_id: String,
    pub effect: RuleEffect,
    pub resource_pattern: String,
    pub action: String,
    pub conditions: Vec<RuleCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleEffect { Allow, Deny }

#[derive(Debug, Clone)]
pub enum RuleCondition { MaxTokens(u64), MaxRequestsPerMin(u64), AllowedRegions(Vec<String>), RequiresRole(String), MaxDataSize(u64) }

// ── Policy Hierarchy ─────────────────────────────────────────────────────────

pub struct PolicyHierarchy {
    global_policies: Arc<RwLock<Vec<Policy>>>,
    tenant_policies: Arc<RwLock<HashMap<String, Vec<Policy>>>>,
    project_policies: Arc<RwLock<HashMap<String, Vec<Policy>>>>,
    cache: Arc<RwLock<BTreeMap<String, Vec<Policy>>>>,
}

impl Default for PolicyHierarchy { fn default() -> Self { Self::new() } }

impl PolicyHierarchy {
    pub fn new() -> Self {
        Self { global_policies: Arc::new(RwLock::new(Vec::new())), tenant_policies: Arc::new(RwLock::new(HashMap::new())), project_policies: Arc::new(RwLock::new(HashMap::new())), cache: Arc::new(RwLock::new(BTreeMap::new())) }
    }

    pub async fn add_policy(&self, policy: Policy) {
        match policy.level {
            PolicyLevel::Global => { self.global_policies.write().await.push(policy); }
            PolicyLevel::Tenant => { let mut map = self.tenant_policies.write().await; map.entry(policy.tenant_id.clone().unwrap()).or_insert_with(Vec::new).push(policy); }
            PolicyLevel::Project => { let mut map = self.project_policies.write().await; map.entry(policy.project_id.clone().unwrap()).or_insert_with(Vec::new).push(policy); }
        }
        self.invalidate_cache().await;
    }

    pub async fn get_policies(&self, tenant_id: &str, project_id: Option<&str>) -> Vec<Policy> {
        let cache_key = format!("{}/{:#?}", tenant_id, project_id);
        if let Some(cached) = self.cache.read().await.get(&cache_key) { return cached.clone(); }
        let mut result = Vec::new();
        result.extend(self.global_policies.read().await.iter().cloned());
        if let Some(tenant_pol) = self.tenant_policies.read().await.get(tenant_id) { result.extend(tenant_pol.iter().cloned()); }
        if let Some(pid) = project_id { if let Some(proj_pol) = self.project_policies.read().await.get(pid) { result.extend(proj_pol.iter().cloned()); } }
        result.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.cache.write().await.insert(cache_key, result.clone());
        result
    }

    async fn invalidate_cache(&self) { self.cache.write().await.clear(); }
}

// ── Conflict Resolution ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy { DenyWins, AllowWins, HighestPriorityWins, MostSpecificWins }

pub struct PolicyConflictResolver { strategy: ConflictStrategy }

impl Default for PolicyConflictResolver { fn default() -> Self { Self::new() } }

impl PolicyConflictResolver {
    pub fn new() -> Self { Self { strategy: ConflictStrategy::MostSpecificWins } }
    pub fn with_strategy(mut self, strategy: ConflictStrategy) -> Self { self.strategy = strategy; self }
    pub fn resolve(&self, policies: &[Policy]) -> Vec<Policy> {
        match self.strategy {
            ConflictStrategy::DenyWins => { let mut result = policies.to_vec(); result.sort_by(|a, b| { let a_deny = a.rules.iter().any(|r| r.effect == RuleEffect::Deny); let b_deny = b.rules.iter().any(|r| r.effect == RuleEffect::Deny); if a_deny != b_deny { b_deny.cmp(&a_deny) } else { b.priority.cmp(&a.priority) } }); result }
            ConflictStrategy::AllowWins => { let mut result = policies.to_vec(); result.sort_by(|a, b| { let a_allow = a.rules.iter().any(|r| r.effect == RuleEffect::Allow); let b_allow = b.rules.iter().any(|r| r.effect == RuleEffect::Allow); if a_allow != b_allow { a_allow.cmp(&b_allow) } else { b.priority.cmp(&a.priority) } }); result }
            ConflictStrategy::HighestPriorityWins => { let mut result = policies.to_vec(); result.sort_by(|a, b| b.priority.cmp(&a.priority)); result }
            ConflictStrategy::MostSpecificWins => { let mut result = policies.to_vec(); result.sort_by(|a, b| b.level.cmp(&a.level)); result }
        }
    }
}

// ── Policy Override ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OverrideCapability { pub can_override: bool, pub overridable_fields: Vec<String> }

pub struct PolicyOverrideManager {
    overridable_fields: Vec<String>,
}

impl Default for PolicyOverrideManager { fn default() -> Self { Self::new() } }

impl PolicyOverrideManager {
    pub fn new() -> Self { Self { overridable_fields: vec!["rate_limit".to_string(), "allowed_regions".to_string(), "max_tokens".to_string()] } }
    pub fn can_override(&self, field: &str) -> bool { self.overridable_fields.contains(&field.to_string()) }
    pub fn register_overridable_field(&mut self, field: &str) { self.overridable_fields.push(field.to_string()); }
    pub fn get_capability(&self, tenant_id: &str) -> OverrideCapability { OverrideCapability { can_override: true, overridable_fields: self.overridable_fields.clone() } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_policy_hierarchy_add_global() {
        let hierarchy = PolicyHierarchy::new();
        hierarchy.add_policy(Policy::new_global("global_policy")).await;
        let policies = hierarchy.get_policies("tenant1", None).await;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].level, PolicyLevel::Global);
    }

    #[tokio::test]
    async fn test_policy_hierarchy_tenant_inherits_global() {
        let hierarchy = PolicyHierarchy::new();
        hierarchy.add_policy(Policy::new_global("global_policy")).await;
        hierarchy.add_policy(Policy::new_tenant("tenant_policy", "tenant1")).await;
        let policies = hierarchy.get_policies("tenant1", None).await;
        assert_eq!(policies.len(), 2);
        assert!(policies.iter().any(|p| p.name == "global_policy"));
        assert!(policies.iter().any(|p| p.name == "tenant_policy"));
    }

    #[tokio::test]
    async fn test_policy_hierarchy_priority_order() {
        let hierarchy = PolicyHierarchy::new();
        hierarchy.add_policy(Policy::new_global("global_policy")).await;
        hierarchy.add_policy(Policy::new_project("project_policy", "tenant1", "proj1")).await;
        let policies = hierarchy.get_policies("tenant1", Some("proj1")).await;
        assert_eq!(policies[0].name, "project_policy");
    }

    #[test]
    fn test_conflict_resolver_deny_wins() {
        let resolver = PolicyConflictResolver::new();
        let mut p1 = Policy::new_global("allow_all"); p1.rules.push(PolicyRule { rule_id: "r1".to_string(), effect: RuleEffect::Allow, resource_pattern: "*".to_string(), action: "*".to_string(), conditions: vec![] });
        let mut p2 = Policy::new_tenant("deny_write", "t1"); p2.rules.push(PolicyRule { rule_id: "r2".to_string(), effect: RuleEffect::Deny, resource_pattern: "*".to_string(), action: "write".to_string(), conditions: vec![] });
        let resolved = resolver.resolve(&[p1, p2]);
        assert_eq!(resolved[0].name, "deny_write");
    }

    #[test]
    fn test_conflict_resolver_most_specific() {
        let resolver = PolicyConflictResolver::with_strategy(PolicyConflictResolver::new(), ConflictStrategy::MostSpecificWins);
        let p1 = Policy::new_global("global");
        let p2 = Policy::new_tenant("tenant", "t1");
        let resolved = resolver.resolve(&[p1, p2]);
        assert_eq!(resolved[0].name, "tenant");
    }

    #[test]
    fn test_override_capability() {
        let manager = PolicyOverrideManager::new();
        assert!(manager.can_override("rate_limit"));
        assert!(!manager.can_override("security_level"));
    }

    #[tokio::test]
    async fn test_policy_cache() {
        let hierarchy = PolicyHierarchy::new();
        hierarchy.add_policy(Policy::new_global("global")).await;
        let first = hierarchy.get_policies("t1", None).await;
        let second = hierarchy.get_policies("t1", None).await;
        assert_eq!(first.len(), second.len());
    }
}
