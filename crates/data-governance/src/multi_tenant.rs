//! Multi-Tenant Isolation Engine
//!
//! Provides namespace-based hard isolation + resource quota soft isolation
//! for enterprise multi-tenant deployments.

use kias_common::KiasError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tenant identifier
pub type TenantId = String;

/// Namespace isolates resources per tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub tenant_id: TenantId,
    pub name: String,
    pub labels: HashMap<String, String>,
    pub network_policy: NetworkPolicy,
    pub created_at: String,
}

/// Network isolation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Allow ingress only from these namespaces
    pub ingress_allow: Vec<TenantId>,
    /// Allow egress only to these namespaces
    pub egress_allow: Vec<TenantId>,
    /// Default deny all traffic
    pub default_deny: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            ingress_allow: Vec::new(),
            egress_allow: Vec::new(),
            default_deny: true,
        }
    }
}

/// Resource quota per tenant (soft limits)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub tenant_id: TenantId,
    /// Max queries per second
    pub max_qps: u32,
    /// Max tokens per day
    pub max_tokens_per_day: u64,
    /// Max concurrent agents
    pub max_agents: u32,
    /// Max storage in bytes
    pub max_storage_bytes: u64,
    /// Max tool invocations per hour
    pub max_tool_calls_per_hour: u32,
    /// Max workflow executions per day
    pub max_workflows_per_day: u32,
}

impl ResourceQuota {
    pub fn standard(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            max_qps: 100,
            max_tokens_per_day: 1_000_000,
            max_agents: 10,
            max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10GB
            max_tool_calls_per_hour: 1000,
            max_workflows_per_day: 100,
        }
    }

    pub fn enterprise(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            max_qps: 1000,
            max_tokens_per_day: 50_000_000,
            max_agents: 100,
            max_storage_bytes: 100 * 1024 * 1024 * 1024, // 100GB
            max_tool_calls_per_hour: 10000,
            max_workflows_per_day: 1000,
        }
    }
}

/// Current resource usage for a tenant
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub current_qps: u32,
    pub tokens_used_today: u64,
    pub active_agents: u32,
    pub storage_used_bytes: u64,
    pub tool_calls_this_hour: u32,
    pub workflows_today: u32,
}

/// Quota check result
#[derive(Debug, Clone)]
pub struct QuotaCheck {
    pub allowed: bool,
    pub resource: String,
    pub current: u64,
    pub limit: u64,
    pub message: String,
}

/// Multi-tenant manager
pub struct TenantManager {
    tenants: Arc<RwLock<HashMap<TenantId, TenantState>>>,
}

#[derive(Debug, Clone)]
struct TenantState {
    namespace: Namespace,
    quota: ResourceQuota,
    usage: ResourceUsage,
    active: bool,
}

/// Tenant info returned to callers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantInfo {
    pub tenant_id: TenantId,
    pub namespace_name: String,
    pub active: bool,
    pub quota: ResourceQuota,
    pub usage: ResourceUsage,
}

impl TenantManager {
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new tenant with namespace and quota
    pub async fn register_tenant(
        &self,
        tenant_id: TenantId,
        namespace_name: String,
        quota: ResourceQuota,
    ) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        if tenants.contains_key(&tenant_id) {
            return Err(KiasError::Validation(format!(
                "Tenant '{}' already exists",
                tenant_id
            )));
        }

        let ns = Namespace {
            tenant_id: tenant_id.clone(),
            name: namespace_name,
            labels: HashMap::new(),
            network_policy: NetworkPolicy::default(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        tenants.insert(
            tenant_id,
            TenantState {
                namespace: ns,
                quota,
                usage: ResourceUsage::default(),
                active: true,
            },
        );
        Ok(())
    }

    /// Remove a tenant and all its resources
    pub async fn remove_tenant(&self, tenant_id: &str) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        tenants
            .remove(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;
        Ok(())
    }

    /// Get tenant info
    pub async fn get_tenant(&self, tenant_id: &str) -> Result<TenantInfo, KiasError> {
        let tenants = self.tenants.read().await;
        let state = tenants
            .get(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;
        Ok(TenantInfo {
            tenant_id: tenant_id.to_string(),
            namespace_name: state.namespace.name.clone(),
            active: state.active,
            quota: state.quota.clone(),
            usage: state.usage.clone(),
        })
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Vec<TenantInfo> {
        let tenants = self.tenants.read().await;
        tenants
            .iter()
            .map(|(id, state)| TenantInfo {
                tenant_id: id.clone(),
                namespace_name: state.namespace.name.clone(),
                active: state.active,
                quota: state.quota.clone(),
                usage: state.usage.clone(),
            })
            .collect()
    }

    /// Check if a request is within quota
    pub async fn check_quota(
        &self,
        tenant_id: &str,
        resource: &str,
        requested: u64,
    ) -> Result<QuotaCheck, KiasError> {
        let tenants = self.tenants.read().await;
        let state = tenants
            .get(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;

        if !state.active {
            return Err(KiasError::Validation(format!(
                "Tenant '{}' is deactivated",
                tenant_id
            )));
        }

        let (current, limit) = match resource {
            "qps" => (state.usage.current_qps as u64, state.quota.max_qps as u64),
            "tokens" => (
                state.usage.tokens_used_today,
                state.quota.max_tokens_per_day,
            ),
            "agents" => (state.usage.active_agents as u64, state.quota.max_agents as u64),
            "storage" => (
                state.usage.storage_used_bytes,
                state.quota.max_storage_bytes,
            ),
            "tool_calls" => (
                state.usage.tool_calls_this_hour as u64,
                state.quota.max_tool_calls_per_hour as u64,
            ),
            "workflows" => (
                state.usage.workflows_today as u64,
                state.quota.max_workflows_per_day,
            ),
            _ => {
                return Err(KiasError::Validation(format!(
                    "Unknown resource type '{}'",
                    resource
                )))
            }
        };

        let new_total = current + requested;
        let allowed = new_total <= limit;

        Ok(QuotaCheck {
            allowed,
            resource: resource.to_string(),
            current,
            limit,
            message: if allowed {
                format!("{}: {}/{} (within quota)", resource, new_total, limit)
            } else {
                format!(
                    "{}: {}/{} (quota exceeded, requested {})",
                    resource, new_total, limit, requested
                )
            },
        })
    }

    /// Record resource usage
    pub async fn record_usage(
        &self,
        tenant_id: &str,
        resource: &str,
        amount: u64,
    ) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        let state = tenants
            .get_mut(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;

        match resource {
            "tokens" => state.usage.tokens_used_today += amount,
            "storage" => state.usage.storage_used_bytes += amount,
            "tool_calls" => state.usage.tool_calls_this_hour += amount as u32,
            "workflows" => state.usage.workflows_today += amount as u32,
            "qps" => state.usage.current_qps += amount as u32,
            "agents" => state.usage.active_agents += amount as u32,
            _ => {
                return Err(KiasError::Validation(format!(
                    "Unknown resource '{}'",
                    resource
                )))
            }
        }
        Ok(())
    }

    /// Check cross-tenant access (is source allowed to access target?)
    pub async fn check_cross_tenant_access(
        &self,
        source_tenant: &str,
        target_tenant: &str,
    ) -> Result<bool, KiasError> {
        let tenants = self.tenants.read().await;
        let source = tenants
            .get(source_tenant)
            .ok_or_else(|| KiasError::NotFound(format!("Source tenant '{}' not found", source_tenant)))?;

        // Same tenant = always allowed
        if source_tenant == target_tenant {
            return Ok(true);
        }

        // Check network policy
        if source.namespace.network_policy.default_deny {
            return Ok(source
                .namespace
                .network_policy
                .egress_allow
                .contains(&target_tenant.to_string()));
        }

        Ok(true)
    }

    /// Update network policy for a tenant
    pub async fn set_network_policy(
        &self,
        tenant_id: &str,
        policy: NetworkPolicy,
    ) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        let state = tenants
            .get_mut(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;
        state.namespace.network_policy = policy;
        Ok(())
    }

    /// Deactivate a tenant (soft delete)
    pub async fn deactivate(&self, tenant_id: &str) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        let state = tenants
            .get_mut(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;
        state.active = false;
        Ok(())
    }

    /// Activate a tenant
    pub async fn activate(&self, tenant_id: &str) -> Result<(), KiasError> {
        let mut tenants = self.tenants.write().await;
        let state = tenants
            .get_mut(tenant_id)
            .ok_or_else(|| KiasError::NotFound(format!("Tenant '{}' not found", tenant_id)))?;
        state.active = true;
        Ok(())
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get_tenant() {
        let mgr = TenantManager::new();
        let quota = ResourceQuota::standard("t1");
        mgr.register_tenant("t1".into(), "ns-t1".into(), quota)
            .await
            .unwrap();

        let info = mgr.get_tenant("t1").await.unwrap();
        assert_eq!(info.tenant_id, "t1");
        assert_eq!(info.namespace_name, "ns-t1");
        assert!(info.active);
    }

    #[tokio::test]
    async fn test_duplicate_tenant_rejected() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();
        let result = mgr
            .register_tenant("t1".into(), "ns2".into(), ResourceQuota::standard("t1"))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_tenant() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();
        mgr.remove_tenant("t1").await.unwrap();
        assert!(mgr.get_tenant("t1").await.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_tenant() {
        let mgr = TenantManager::new();
        assert!(mgr.remove_tenant("ghost").await.is_err());
    }

    #[tokio::test]
    async fn test_list_tenants() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();
        mgr.register_tenant(
            "t2".into(),
            "ns2".into(),
            ResourceQuota::enterprise("t2"),
        )
        .await
        .unwrap();

        let list = mgr.list_tenants().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_quota_within_limit() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        let check = mgr.check_quota("t1", "tokens", 500).await.unwrap();
        assert!(check.allowed);
        assert_eq!(check.resource, "tokens");
    }

    #[tokio::test]
    async fn test_quota_exceeded() {
        let mgr = TenantManager::new();
        let mut quota = ResourceQuota::standard("t1");
        quota.max_tokens_per_day = 1000;
        mgr.register_tenant("t1".into(), "ns1".into(), quota)
            .await
            .unwrap();

        // Request 1500 tokens when limit is 1000
        let check = mgr.check_quota("t1", "tokens", 1500).await.unwrap();
        assert!(!check.allowed);
        assert!(check.message.contains("exceeded"));
    }

    #[tokio::test]
    async fn test_quota_unknown_resource() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        assert!(mgr.check_quota("t1", "gpu_hours", 1).await.is_err());
    }

    #[tokio::test]
    async fn test_record_usage() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        mgr.record_usage("t1", "tokens", 5000).await.unwrap();
        let info = mgr.get_tenant("t1").await.unwrap();
        assert_eq!(info.usage.tokens_used_today, 5000);
    }

    #[tokio::test]
    async fn test_cross_tenant_same_tenant_allowed() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        assert!(mgr.check_cross_tenant_access("t1", "t1").await.unwrap());
    }

    #[tokio::test]
    async fn test_cross_tenant_default_deny() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();
        mgr.register_tenant(
            "t2".into(),
            "ns2".into(),
            ResourceQuota::standard("t2"),
        )
        .await
        .unwrap();

        // Default deny = cross-tenant blocked
        assert!(!mgr.check_cross_tenant_access("t1", "t2").await.unwrap());
    }

    #[tokio::test]
    async fn test_cross_tenant_with_explicit_allow() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();
        mgr.register_tenant(
            "t2".into(),
            "ns2".into(),
            ResourceQuota::standard("t2"),
        )
        .await
        .unwrap();

        // Allow t1 -> t2
        let policy = NetworkPolicy {
            ingress_allow: vec![],
            egress_allow: vec!["t2".to_string()],
            default_deny: true,
        };
        mgr.set_network_policy("t1", policy).await.unwrap();

        assert!(mgr.check_cross_tenant_access("t1", "t2").await.unwrap());
        // t2 -> t1 still blocked
        assert!(!mgr.check_cross_tenant_access("t2", "t1").await.unwrap());
    }

    #[tokio::test]
    async fn test_deactivate_blocks_quota_check() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        mgr.deactivate("t1").await.unwrap();
        assert!(mgr.check_quota("t1", "tokens", 1).await.is_err());
    }

    #[tokio::test]
    async fn test_activate_restores_access() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        mgr.deactivate("t1").await.unwrap();
        mgr.activate("t1").await.unwrap();
        assert!(mgr.check_quota("t1", "tokens", 1).await.is_ok());
    }

    #[tokio::test]
    async fn test_enterprise_quota_limits() {
        let q = ResourceQuota::enterprise("ent1");
        assert_eq!(q.max_qps, 1000);
        assert_eq!(q.max_tokens_per_day, 50_000_000);
        assert_eq!(q.max_agents, 100);
    }

    #[tokio::test]
    async fn test_standard_quota_limits() {
        let q = ResourceQuota::standard("s1");
        assert_eq!(q.max_qps, 100);
        assert_eq!(q.max_agents, 10);
    }

    #[tokio::test]
    async fn test_multiple_resource_checks() {
        let mgr = TenantManager::new();
        mgr.register_tenant(
            "t1".into(),
            "ns1".into(),
            ResourceQuota::standard("t1"),
        )
        .await
        .unwrap();

        // Check multiple resources
        assert!(mgr.check_quota("t1", "qps", 50).await.unwrap().allowed);
        assert!(mgr
            .check_quota("t1", "agents", 5)
            .await
            .unwrap()
            .allowed);
        assert!(mgr
            .check_quota("t1", "storage", 1024)
            .await
            .unwrap()
            .allowed);
    }
}
