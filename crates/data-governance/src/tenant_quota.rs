
pub struct TenantQuota {
    tenant_id: Uuid,
    quotas: QuotaLimits,
    usage: CurrentUsage,
    parent_tenant_id: Option<Uuid>,
}