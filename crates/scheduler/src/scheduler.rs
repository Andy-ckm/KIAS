use std::collections::HashMap;
use std::sync::Arc;

use kias_common::{Agent, KiasError, Node, ScheduleResult};
use tokio::sync::RwLock;
use tracing;

use crate::algorithms::{
    CacheAwareScheduler, LeastLoadedScheduler, ResourceAwareScheduler, RoundRobinScheduler,
    SchedulingAlgorithm,
};

/// Tenant context for multi-tenant scheduling isolation.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub namespace: String,
    pub resource_quota: ResourceQuota,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceQuota {
    pub max_agents: u32,
    pub max_nodes: u32,
    pub cpu_limit: f64,
    pub memory_limit_mb: u64,
}

impl TenantContext {
    pub fn new(tenant_id: impl Into<String>) -> Self {
        let tid = tenant_id.into();
        Self {
            tenant_id: tid.clone(),
            namespace: format!("tenant-{}", tid),
            resource_quota: ResourceQuota::default(),
        }
    }

    pub fn with_quota(mut self, quota: ResourceQuota) -> Self {
        self.resource_quota = quota;
        self
    }
}

/// Per-tenant scheduling statistics and resource accounting.
#[derive(Debug, Clone, Default)]
pub struct TenantStats {
    /// Current number of active (scheduled) agents for this tenant.
    pub active_agents: u32,
    /// Total CPU cores currently allocated to this tenant.
    pub total_cpu: f64,
    /// Total memory bytes currently allocated to this tenant.
    pub total_memory_bytes: u64,
    /// Total scheduling attempts for this tenant.
    pub schedules_attempted: u64,
    /// Successful scheduling decisions for this tenant.
    pub schedules_succeeded: u64,
    /// Scheduling attempts rejected due to quota limits.
    pub schedules_rejected_quota: u64,
}

use crate::config::SchedulerConfig;
use crate::optimizer::CacheOptimizer;
use crate::policies::{AffinityFilter, PrioritySorter};

/// The main scheduler entry point.
///
/// Orchestrates the full scheduling pipeline:
/// 1. Sort agents by priority
/// 2. Filter nodes by affinity / anti-affinity
/// 3. Apply the configured scheduling algorithm
/// 4. Track cache state via the optimizer
/// 5. Enforce multi-tenant quotas and namespace isolation
pub struct Scheduler {
    config: SchedulerConfig,
    algorithm: Box<dyn SchedulingAlgorithm>,
    cache_optimizer: Arc<CacheOptimizer>,
    /// Per-tenant state for multi-tenant isolation.
    tenant_states: Arc<RwLock<HashMap<String, TenantStats>>>,
    /// Index for fair round-robin scheduling across tenants.
    fair_schedule_index: Arc<RwLock<usize>>,
}

impl Scheduler {
    /// Create a new scheduler from configuration.
    pub fn new(config: SchedulerConfig) -> Self {
        let algorithm = Self::build_algorithm(&config);
        let cache_optimizer = Arc::new(CacheOptimizer::new());

        tracing::info!(
            algorithm = %config.algorithm,
            cache_weight = config.cache_weight,
            preemption = config.preemption_enabled,
            "Scheduler initialized"
        );

        Self {
            config,
            algorithm,
            cache_optimizer,
            tenant_states: Arc::new(RwLock::new(HashMap::new())),
            fair_schedule_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a scheduler with a shared cache optimizer (for use by other components).
    pub fn with_cache_optimizer(
        config: SchedulerConfig,
        cache_optimizer: Arc<CacheOptimizer>,
    ) -> Self {
        let algorithm = Self::build_algorithm(&config);

        tracing::info!(
            algorithm = %config.algorithm,
            "Scheduler initialized with shared cache optimizer"
        );

        Self {
            config,
            algorithm,
            cache_optimizer,
            tenant_states: Arc::new(RwLock::new(HashMap::new())),
            fair_schedule_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Build the scheduling algorithm from config.
    fn build_algorithm(config: &SchedulerConfig) -> Box<dyn SchedulingAlgorithm> {
        match config.algorithm.as_str() {
            "round-robin" => Box::new(RoundRobinScheduler::new()),
            "least-loaded" => Box::new(LeastLoadedScheduler::new()),
            "resource-aware" => Box::new(ResourceAwareScheduler::new()),
            "cache-aware" => Box::new(CacheAwareScheduler::new(config.cache_weight)),
            other => {
                tracing::warn!(
                    requested = other,
                    "Unknown algorithm, falling back to round-robin"
                );
                Box::new(RoundRobinScheduler::new())
            }
        }
    }

    /// Schedule a single agent onto a node (no tenant context).
    ///
    /// Applies affinity filtering, then delegates to the configured algorithm.
    /// If the agent has a `tenant_id` set, it will be scheduled without quota
    /// enforcement (legacy path).
    pub async fn schedule_agent(
        &self,
        agent: &Agent,
        nodes: &[Node],
    ) -> Result<ScheduleResult, KiasError> {
        self.schedule_agent_with_tenant(agent, nodes, None).await
    }

    /// Schedule a single agent with optional tenant context.
    ///
    /// When a `TenantContext` is provided:
    /// - Validates the agent belongs to the tenant's namespace
    /// - Checks resource quotas (max_agents, CPU, memory)
    /// - Tracks resource usage and scheduling statistics
    /// - Agents from different tenants are isolated (cannot see each other)
    pub async fn schedule_agent_with_tenant(
        &self,
        agent: &Agent,
        nodes: &[Node],
        tenant_ctx: Option<&TenantContext>,
    ) -> Result<ScheduleResult, KiasError> {
        // Validate tenant namespace isolation
        if let Some(ctx) = tenant_ctx {
            // Enforce namespace: agent.tenant_id must match context tenant_id
            if let Some(ref agent_tid) = agent.tenant_id {
                if agent_tid != &ctx.tenant_id {
                    return Err(KiasError::Scheduler(format!(
                        "Tenant namespace violation: agent belongs to tenant '{}', \
                         but context is for tenant '{}'",
                        agent_tid, ctx.tenant_id
                    )));
                }
            }

            // Record scheduling attempt BEFORE quota enforcement
            {
                let mut states = self.tenant_states.write().await;
                let stats = states
                    .entry(ctx.tenant_id.clone())
                    .or_insert_with(TenantStats::default);
                stats.schedules_attempted += 1;
            }

            // Enforce resource quotas
            self.enforce_quota(agent, ctx).await?;
        }

        // Step 1: Filter by affinity / anti-affinity
        let candidates = AffinityFilter::apply(agent, nodes);

        // Step 2: Tenant namespace filtering — restrict to tenant-allowed nodes
        let filtered_candidates = if let Some(ctx) = tenant_ctx {
            self.filter_by_namespace(&candidates, ctx)
        } else {
            candidates
        };

        let candidate_nodes: Vec<Node> = filtered_candidates.iter().map(|(n, _)| (*n).clone()).collect();

        if candidate_nodes.is_empty() {
            tracing::warn!(
                agent_id = %agent.id,
                "No nodes satisfy affinity/namespace constraints"
            );
            return Err(KiasError::NoAvailableNodes);
        }

        // Step 3: Run scheduling algorithm
        let mut result = self.algorithm.schedule(agent, &candidate_nodes).await?;

        // Step 4: Blend affinity score into the result
        if let Some((_, affinity_score)) = filtered_candidates.iter().find(|(n, _)| n.id == result.node_id) {
            // Combine: 70% algorithm score + 30% affinity score
            result.score = 0.7 * result.score + 0.3 * affinity_score;
        }

        // Record successful scheduling and update resource accounting
        if let Some(ctx) = tenant_ctx {
            let mut states = self.tenant_states.write().await;
            let stats = states
                .entry(ctx.tenant_id.clone())
                .or_insert_with(TenantStats::default);
            stats.schedules_succeeded += 1;
            stats.active_agents += 1;
            stats.total_cpu += agent.resource_request.cpu;
            stats.total_memory_bytes += agent.resource_request.memory_bytes;
        }

        tracing::debug!(
            agent_id = %agent.id,
            node_id = %result.node_id,
            algorithm = %result.algorithm,
            score = result.score,
            "Scheduling complete"
        );

        Ok(result)
    }

    /// Enforce tenant resource quotas before scheduling.
    async fn enforce_quota(
        &self,
        agent: &Agent,
        ctx: &TenantContext,
    ) -> Result<(), KiasError> {
        let states = self.tenant_states.read().await;
        let stats = states.get(&ctx.tenant_id);

        let current_agents = stats.map_or(0, |s| s.active_agents);
        let current_cpu = stats.map_or(0.0, |s| s.total_cpu);
        let current_memory = stats.map_or(0, |s| s.total_memory_bytes);

        // Check max_agents
        if ctx.resource_quota.max_agents > 0
            && current_agents >= ctx.resource_quota.max_agents
        {
            tracing::warn!(
                tenant_id = %ctx.tenant_id,
                current = current_agents,
                limit = ctx.resource_quota.max_agents,
                "Tenant agent quota exceeded"
            );
            // Record rejection
            drop(states);
            let mut states = self.tenant_states.write().await;
            let stats = states
                .entry(ctx.tenant_id.clone())
                .or_insert_with(TenantStats::default);
            stats.schedules_rejected_quota += 1;
            return Err(KiasError::TenantQuotaExceeded(format!(
                "tenant '{}': agent quota {}/{} exceeded",
                ctx.tenant_id, current_agents, ctx.resource_quota.max_agents
            )));
        }

        // Check CPU limit
        if ctx.resource_quota.cpu_limit > 0.0
            && current_cpu + agent.resource_request.cpu > ctx.resource_quota.cpu_limit
        {
            tracing::warn!(
                tenant_id = %ctx.tenant_id,
                current_cpu = current_cpu,
                requested = agent.resource_request.cpu,
                limit = ctx.resource_quota.cpu_limit,
                "Tenant CPU quota exceeded"
            );
            drop(states);
            let mut states = self.tenant_states.write().await;
            let stats = states
                .entry(ctx.tenant_id.clone())
                .or_insert_with(TenantStats::default);
            stats.schedules_rejected_quota += 1;
            return Err(KiasError::TenantQuotaExceeded(format!(
                "tenant '{}': CPU quota {:.1}/{:.1} exceeded",
                ctx.tenant_id, current_cpu + agent.resource_request.cpu, ctx.resource_quota.cpu_limit
            )));
        }

        // Check memory limit (quota is in MB, stats in bytes)
        let memory_limit_bytes = ctx.resource_quota.memory_limit_mb * 1024 * 1024;
        if memory_limit_bytes > 0
            && current_memory + agent.resource_request.memory_bytes > memory_limit_bytes
        {
            tracing::warn!(
                tenant_id = %ctx.tenant_id,
                current_memory = current_memory,
                requested = agent.resource_request.memory_bytes,
                limit_mb = ctx.resource_quota.memory_limit_mb,
                "Tenant memory quota exceeded"
            );
            drop(states);
            let mut states = self.tenant_states.write().await;
            let stats = states
                .entry(ctx.tenant_id.clone())
                .or_insert_with(TenantStats::default);
            stats.schedules_rejected_quota += 1;
            return Err(KiasError::TenantQuotaExceeded(format!(
                "tenant '{}': memory quota exceeded",
                ctx.tenant_id
            )));
        }

        Ok(())
    }

    /// Filter candidates by tenant namespace.
    ///
    /// When a tenant context is provided, only nodes that are compatible
    /// with the tenant's namespace are returned. Nodes with a `namespace`
    /// label matching the tenant namespace, or nodes without a namespace
    /// label, are considered available.
    fn filter_by_namespace<'a>(
        &self,
        candidates: &[(&'a Node, f64)],
        ctx: &TenantContext,
    ) -> Vec<(&'a Node, f64)> {
        candidates
            .iter()
            .filter(|(node, _)| {
                // If the node has a namespace label, it must match the tenant's namespace
                // Nodes without a namespace label are shared/available to all tenants
                match node.labels.get("namespace") {
                    Some(ns) => ns == &ctx.namespace,
                    None => true,
                }
            })
            .cloned()
            .collect()
    }

    /// Schedule a batch of agents in priority order.
    ///
    /// Higher-priority agents are scheduled first and get first pick of nodes.
    pub async fn schedule_batch(
        &self,
        agents: &mut [Agent],
        nodes: &[Node],
    ) -> Vec<Result<ScheduleResult, KiasError>> {
        // Sort by priority (high first)
        PrioritySorter::sort_agents(agents);

        let mut results = Vec::with_capacity(agents.len());

        for agent in agents.iter() {
            let result = self.schedule_agent(agent, nodes).await;
            results.push(result);
        }

        let successes = results.iter().filter(|r| r.is_ok()).count();
        tracing::info!(
            total = agents.len(),
            scheduled = successes,
            failed = agents.len() - successes,
            "Batch scheduling complete"
        );

        results
    }

    /// Schedule a batch with fair round-robin across tenants.
    ///
    /// Agents are interleaved across tenants so that no single tenant
    /// monopolizes scheduling. Within each tenant's turn, agents are
    /// scheduled in priority order.
    pub async fn schedule_batch_fair(
        &self,
        agents: &mut [Agent],
        nodes: &[Node],
        tenant_ctxs: &HashMap<String, TenantContext>,
    ) -> Vec<Result<ScheduleResult, KiasError>> {
        // Group agents by tenant_id
        let mut tenant_agents: HashMap<String, Vec<&Agent>> = HashMap::new();
        let mut unassigned: Vec<&Agent> = Vec::new();

        for agent in agents.iter() {
            match &agent.tenant_id {
                Some(tid) => {
                    tenant_agents.entry(tid.clone()).or_default().push(agent);
                }
                None => {
                    unassigned.push(agent);
                }
            }
        }

        // Sort each tenant's agents by priority
        for tenant_list in tenant_agents.values_mut() {
            tenant_list.sort_by_key(|b| std::cmp::Reverse(b.priority));
        }

        // Round-robin across tenants
        let mut results: Vec<Result<ScheduleResult, KiasError>> = Vec::new();
        let tenant_ids: Vec<String> = tenant_agents.keys().cloned().collect();

        if tenant_ids.is_empty() {
            // No tenants, schedule all normally
            for agent in unassigned {
                results.push(self.schedule_agent(agent, nodes).await);
            }
            return results;
        }

        let start_idx = {
            let idx = self.fair_schedule_index.read().await;
            *idx
        };

        let mut has_remaining = true;
        let mut offsets: HashMap<String, usize> = HashMap::new();

        while has_remaining {
            has_remaining = false;
            for i in 0..tenant_ids.len() {
                let tenant_id = &tenant_ids[(start_idx + i) % tenant_ids.len()];
                let offset = offsets.entry(tenant_id.clone()).or_insert(0);
                if let Some(agent_list) = tenant_agents.get(tenant_id) {
                    if *offset < agent_list.len() {
                        let agent = agent_list[*offset];
                        let ctx = tenant_ctxs.get(tenant_id);
                        let result = self
                            .schedule_agent_with_tenant(agent, nodes, ctx)
                            .await;
                        results.push(result);
                        *offset += 1;
                        has_remaining = true;
                    }
                }
            }
        }

        // Schedule unassigned agents
        for agent in unassigned {
            results.push(self.schedule_agent(agent, nodes).await);
        }

        // Update fair schedule index for next call
        {
            let mut idx = self.fair_schedule_index.write().await;
            *idx = (start_idx + 1) % tenant_ids.len().max(1);
        }

        results
    }

    /// Release resources for a tenant's agent (called when an agent is terminated).
    pub async fn release_tenant_agent(
        &self,
        tenant_id: &str,
        cpu: f64,
        memory_bytes: u64,
    ) {
        let mut states = self.tenant_states.write().await;
        if let Some(stats) = states.get_mut(tenant_id) {
            stats.active_agents = stats.active_agents.saturating_sub(1);
            stats.total_cpu = (stats.total_cpu - cpu).max(0.0);
            stats.total_memory_bytes = stats.total_memory_bytes.saturating_sub(memory_bytes);
        }
    }

    /// Get scheduling statistics for a specific tenant.
    pub async fn get_tenant_stats(&self, tenant_id: &str) -> Option<TenantStats> {
        let states = self.tenant_states.read().await;
        states.get(tenant_id).cloned()
    }

    /// Get all tenant scheduling statistics.
    pub async fn get_all_tenant_stats(&self) -> HashMap<String, TenantStats> {
        let states = self.tenant_states.read().await;
        states.clone()
    }

    /// Get a reference to the cache optimizer.
    pub fn cache_optimizer(&self) -> &CacheOptimizer {
        &self.cache_optimizer
    }

    /// Get the current algorithm name.
    pub fn algorithm_name(&self) -> &str {
        self.algorithm.name()
    }

    /// Get the scheduler configuration.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Affinity, AntiAffinity, Priority, Resources};
    use std::collections::HashMap;

    fn make_nodes(n: usize) -> Vec<Node> {
        (0..n)
            .map(|i| Node {
                id: format!("node-{}", i),
                status: kias_common::NodeStatus::Ready,
                total_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels: HashMap::new(),
            })
            .collect()
    }

    fn make_nodes_with_namespace(n: usize, namespace: &str) -> Vec<Node> {
        (0..n)
            .map(|i| {
                let mut labels = HashMap::new();
                labels.insert("namespace".to_string(), namespace.to_string());
                Node {
                    id: format!("node-{}-{}", namespace, i),
                    status: kias_common::NodeStatus::Ready,
                    total_resources: Resources {
                        cpu: 4.0,
                        memory_bytes: 8 * 1024 * 1024 * 1024,
                        gpu: 1,
                        ..Default::default()
                    },
                    available_resources: Resources {
                        cpu: 4.0,
                        memory_bytes: 8 * 1024 * 1024 * 1024,
                        gpu: 1,
                        ..Default::default()
                    },
                    allocated_agents: vec![],
                    labels,
                }
            })
            .collect()
    }

    fn make_agent(id: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    fn make_tenant_agent(id: &str, tenant_id: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: Some(tenant_id.to_string()),
        }
    }

    fn make_tenant_agent_with_resources(
        id: &str,
        tenant_id: &str,
        cpu: f64,
        memory_mb: u64,
    ) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources {
                cpu,
                memory_bytes: memory_mb * 1024 * 1024,
                ..Default::default()
            },
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: Some(tenant_id.to_string()),
        }
    }

    // ─── Existing tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_schedule_single_agent() {
        let config = SchedulerConfig {
            algorithm: "round-robin".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(3);
        let agent = make_agent("a1", Priority::Medium);

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert!(!result.node_id.is_empty());
        assert_eq!(result.algorithm, "round-robin");
    }

    #[tokio::test]
    async fn test_schedule_batch_priority_order() {
        let config = SchedulerConfig {
            algorithm: "least-loaded".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(2);

        let mut agents = vec![
            make_agent("low", Priority::Low),
            make_agent("high", Priority::High),
            make_agent("med", Priority::Medium),
        ];

        let results = scheduler.schedule_batch(&mut agents, &nodes).await;
        assert_eq!(results.len(), 3);
        // All should succeed since we have enough nodes
        assert!(results.iter().all(|r| r.is_ok()));
        // After sorting, high priority agent should be first
        assert_eq!(agents[0].id, "high");
    }

    #[tokio::test]
    async fn test_schedule_with_affinity() {
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);

        let mut labels = HashMap::new();
        labels.insert("zone".to_string(), "us-east".to_string());

        let mut nodes = make_nodes(2);
        nodes[0].labels = labels.clone();
        nodes[1].labels = HashMap::new();

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: labels,
                preferred: vec![],
            }),
            anti_affinity: None,
            tenant_id: None,
        };

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-0");
    }

    #[tokio::test]
    async fn test_schedule_with_anti_affinity() {
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);

        let mut avoid_labels = HashMap::new();
        avoid_labels.insert("zone".to_string(), "eu-west".to_string());

        let mut labels1 = HashMap::new();
        labels1.insert("zone".to_string(), "eu-west".to_string());
        let mut labels2 = HashMap::new();
        labels2.insert("zone".to_string(), "us-east".to_string());

        let mut nodes = make_nodes(2);
        nodes[0].labels = labels1;
        nodes[1].labels = labels2;

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources::default(),
            priority: Priority::Medium,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: Some(AntiAffinity {
                avoid_labels,
                avoid_agent_types: vec![],
            }),
            tenant_id: None,
        };

        let result = scheduler.schedule_agent(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_fallback_algorithm() {
        let config = SchedulerConfig {
            algorithm: "unknown-algo".to_string(),
            ..Default::default()
        };
        let scheduler = Scheduler::new(config);
        assert_eq!(scheduler.algorithm_name(), "round-robin");
    }

    // ─── Multi-tenant tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_tenant_quota_enforcement() {
        // Set up a tenant with max_agents = 2
        let quota = ResourceQuota {
            max_agents: 2,
            max_nodes: 10,
            cpu_limit: 0.0,   // no CPU limit
            memory_limit_mb: 0, // no memory limit
        };
        let ctx = TenantContext::new("tenant-a").with_quota(quota);

        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(10);

        // Schedule first two agents — should succeed
        let a1 = make_tenant_agent("a1", "tenant-a", Priority::Medium);
        let result1 = scheduler.schedule_agent_with_tenant(&a1, &nodes, Some(&ctx)).await;
        assert!(result1.is_ok(), "First agent should succeed");

        let a2 = make_tenant_agent("a2", "tenant-a", Priority::Medium);
        let result2 = scheduler.schedule_agent_with_tenant(&a2, &nodes, Some(&ctx)).await;
        assert!(result2.is_ok(), "Second agent should succeed");

        // Third agent should be rejected — quota exceeded
        let a3 = make_tenant_agent("a3", "tenant-a", Priority::Medium);
        let result3 = scheduler.schedule_agent_with_tenant(&a3, &nodes, Some(&ctx)).await;
        assert!(result3.is_err(), "Third agent should be rejected due to quota");

        match result3 {
            Err(KiasError::TenantQuotaExceeded(_)) => {} // expected
            other => panic!("Expected TenantQuotaExceeded, got {:?}", other),
        }

        // Verify stats
        let stats = scheduler.get_tenant_stats("tenant-a").await.unwrap();
        assert_eq!(stats.active_agents, 2);
        assert_eq!(stats.schedules_attempted, 3);
        assert_eq!(stats.schedules_succeeded, 2);
        assert_eq!(stats.schedules_rejected_quota, 1);
    }

    #[tokio::test]
    async fn test_tenant_namespace_isolation() {
        // Two tenants, each with their own namespace nodes
        let ctx_a = TenantContext::new("alpha");
        let ctx_b = TenantContext::new("beta");

        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);

        // Create namespace-specific nodes
        let mut nodes_a = make_nodes_with_namespace(2, "tenant-alpha");
        let nodes_b = make_nodes_with_namespace(2, "tenant-beta");
        let shared_nodes = make_nodes(2);

        // Combine all nodes
        let mut all_nodes = Vec::new();
        all_nodes.append(&mut nodes_a);
        all_nodes.extend(nodes_b);
        all_nodes.extend(shared_nodes);

        // Agent from tenant-alpha should only land on tenant-alpha or shared nodes
        let agent_a = make_tenant_agent("agent-a", "alpha", Priority::Medium);
        let result_a = scheduler
            .schedule_agent_with_tenant(&agent_a, &all_nodes, Some(&ctx_a))
            .await
            .unwrap();
        assert!(
            result_a.node_id.starts_with("node-") && (
                result_a.node_id.contains("tenant-alpha") ||
                result_a.node_id.starts_with("node-0") || // shared nodes
                result_a.node_id.starts_with("node-1")    // shared nodes
            ),
            "Agent alpha landed on wrong node: {}",
            result_a.node_id
        );

        // Agent from tenant-beta should only land on tenant-beta or shared nodes
        let agent_b = make_tenant_agent("agent-b", "beta", Priority::Medium);
        let result_b = scheduler
            .schedule_agent_with_tenant(&agent_b, &all_nodes, Some(&ctx_b))
            .await
            .unwrap();
        assert!(
            result_b.node_id.starts_with("node-") && (
                result_b.node_id.contains("tenant-beta") ||
                result_b.node_id.starts_with("node-0") || // shared nodes
                result_b.node_id.starts_with("node-1")    // shared nodes
            ),
            "Agent beta landed on wrong node: {}",
            result_b.node_id
        );

        // Agent from tenant-alpha should NOT land on tenant-beta nodes
        assert!(
            !result_a.node_id.contains("tenant-beta"),
            "Tenant alpha agent must not be placed on tenant-beta nodes"
        );
        assert!(
            !result_b.node_id.contains("tenant-alpha"),
            "Tenant beta agent must not be placed on tenant-alpha nodes"
        );
    }

    #[tokio::test]
    async fn test_tenant_namespace_violation_rejected() {
        // Agent claims to be from tenant-beta but context is tenant-alpha
        let ctx_a = TenantContext::new("alpha");
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(3);

        let agent = make_tenant_agent("agent-x", "beta", Priority::Medium);
        let result = scheduler
            .schedule_agent_with_tenant(&agent, &nodes, Some(&ctx_a))
            .await;

        assert!(result.is_err(), "Cross-tenant agent should be rejected");
        match result {
            Err(KiasError::Scheduler(msg)) => {
                assert!(msg.contains("namespace violation"), "Error: {}", msg);
            }
            other => panic!("Expected Scheduler error for namespace violation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_tenant_resource_accounting() {
        // Verify that CPU and memory are tracked per tenant
        let quota = ResourceQuota {
            max_agents: 100,
            max_nodes: 100,
            cpu_limit: 8.0,            // 8 cores total
            memory_limit_mb: 4096,     // 4 GB total
        };
        let ctx = TenantContext::new("accounting-test").with_quota(quota);

        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(10);

        // Schedule agents with specific resource requests
        let a1 = make_tenant_agent_with_resources("a1", "accounting-test", 2.0, 512);
        scheduler.schedule_agent_with_tenant(&a1, &nodes, Some(&ctx)).await.unwrap();

        let a2 = make_tenant_agent_with_resources("a2", "accounting-test", 3.0, 1024);
        scheduler.schedule_agent_with_tenant(&a2, &nodes, Some(&ctx)).await.unwrap();

        let stats = scheduler.get_tenant_stats("accounting-test").await.unwrap();
        assert_eq!(stats.active_agents, 2);
        assert!((stats.total_cpu - 5.0).abs() < f64::EPSILON, "CPU should be 5.0, got {}", stats.total_cpu);
        assert_eq!(stats.total_memory_bytes, (512 + 1024) * 1024 * 1024);

        // Now schedule one more that would exceed CPU limit
        let a3 = make_tenant_agent_with_resources("a3", "accounting-test", 4.0, 512);
        let result = scheduler.schedule_agent_with_tenant(&a3, &nodes, Some(&ctx)).await;
        assert!(result.is_err(), "Should fail: 5.0 + 4.0 > 8.0 CPU limit");

        match result {
            Err(KiasError::TenantQuotaExceeded(msg)) => {
                assert!(msg.contains("CPU"), "Error should mention CPU: {}", msg);
            }
            other => panic!("Expected TenantQuotaExceeded for CPU, got {:?}", other),
        }

        // Stats should reflect the rejection
        let stats = scheduler.get_tenant_stats("accounting-test").await.unwrap();
        assert_eq!(stats.schedules_rejected_quota, 1);
        assert_eq!(stats.schedules_succeeded, 2);

        // Memory limit test — schedule one that would exceed memory
        // Currently: 512 + 1024 = 1536 MB used, limit is 4096 MB
        let a4 = make_tenant_agent_with_resources("a4", "accounting-test", 1.0, 3000);
        let result = scheduler.schedule_agent_with_tenant(&a4, &nodes, Some(&ctx)).await;
        assert!(result.is_err(), "Should fail: 1536 + 3000 > 4096 MB limit");

        match result {
            Err(KiasError::TenantQuotaExceeded(msg)) => {
                assert!(msg.contains("memory"), "Error should mention memory: {}", msg);
            }
            other => panic!("Expected TenantQuotaExceeded for memory, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_multiple_tenants_fair_scheduling() {
        // Three tenants with different priorities, fair scheduling should interleave
        let ctx_a = TenantContext::new("tenant-a").with_quota(ResourceQuota {
            max_agents: 100,
            ..Default::default()
        });
        let ctx_b = TenantContext::new("tenant-b").with_quota(ResourceQuota {
            max_agents: 100,
            ..Default::default()
        });
        let ctx_c = TenantContext::new("tenant-c").with_quota(ResourceQuota {
            max_agents: 100,
            ..Default::default()
        });

        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(100); // plenty of nodes

        // Create agents for each tenant — different counts but all should get fair turns
        let agents = vec![
            make_tenant_agent("a1", "tenant-a", Priority::High),
            make_tenant_agent("a2", "tenant-a", Priority::Medium),
            make_tenant_agent("a3", "tenant-a", Priority::Low),
            make_tenant_agent("b1", "tenant-b", Priority::High),
            make_tenant_agent("b2", "tenant-b", Priority::Medium),
            make_tenant_agent("c1", "tenant-c", Priority::High),
            make_tenant_agent("c2", "tenant-c", Priority::Medium),
            make_tenant_agent("c3", "tenant-c", Priority::Low),
            make_tenant_agent("c4", "tenant-c", Priority::Low),
        ];

        let mut agents = agents;
        let mut tenant_ctxs = HashMap::new();
        tenant_ctxs.insert("tenant-a".to_string(), ctx_a);
        tenant_ctxs.insert("tenant-b".to_string(), ctx_b);
        tenant_ctxs.insert("tenant-c".to_string(), ctx_c);

        let results = scheduler
            .schedule_batch_fair(&mut agents, &nodes, &tenant_ctxs)
            .await;

        // All should succeed
        let successes: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
        assert_eq!(successes.len(), 9, "All 9 agents should be scheduled");

        // Verify tenant stats reflect correct counts
        let stats_a = scheduler.get_tenant_stats("tenant-a").await.unwrap();
        let stats_b = scheduler.get_tenant_stats("tenant-b").await.unwrap();
        let stats_c = scheduler.get_tenant_stats("tenant-c").await.unwrap();

        assert_eq!(stats_a.active_agents, 3, "tenant-a should have 3 agents");
        assert_eq!(stats_b.active_agents, 2, "tenant-b should have 2 agents");
        assert_eq!(stats_c.active_agents, 4, "tenant-c should have 4 agents");

        // Verify total across all tenants
        let all_stats = scheduler.get_all_tenant_stats().await;
        let total_agents: u32 = all_stats.values().map(|s| s.active_agents).sum();
        assert_eq!(total_agents, 9);
    }

    #[tokio::test]
    async fn test_release_tenant_agent() {
        let quota = ResourceQuota {
            max_agents: 2,
            max_nodes: 10,
            cpu_limit: 4.0,
            memory_limit_mb: 2048,
        };
        let ctx = TenantContext::new("release-test").with_quota(quota);

        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(10);

        // Schedule two agents
        let a1 = make_tenant_agent_with_resources("a1", "release-test", 1.0, 512);
        scheduler.schedule_agent_with_tenant(&a1, &nodes, Some(&ctx)).await.unwrap();

        let a2 = make_tenant_agent_with_resources("a2", "release-test", 1.0, 512);
        scheduler.schedule_agent_with_tenant(&a2, &nodes, Some(&ctx)).await.unwrap();

        // Third should fail
        let a3 = make_tenant_agent_with_resources("a3", "release-test", 1.0, 512);
        let result = scheduler.schedule_agent_with_tenant(&a3, &nodes, Some(&ctx)).await;
        assert!(result.is_err(), "Should fail: quota exceeded");

        // Release a1's resources
        scheduler.release_tenant_agent("release-test", 1.0, 512 * 1024 * 1024).await;

        // Now a3 should succeed
        let result = scheduler.schedule_agent_with_tenant(&a3, &nodes, Some(&ctx)).await;
        assert!(result.is_ok(), "Should succeed after release");

        let stats = scheduler.get_tenant_stats("release-test").await.unwrap();
        assert_eq!(stats.active_agents, 2);
    }

    #[tokio::test]
    async fn test_tenant_agent_without_context_succeeds() {
        // An agent with a tenant_id scheduled without a TenantContext
        // should work (legacy path, no quota enforcement)
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(5);

        let agent = make_tenant_agent("a1", "some-tenant", Priority::Medium);
        let result = scheduler.schedule_agent(&agent, &nodes).await;
        assert!(result.is_ok(), "Agent with tenant_id should succeed without context");
    }

    #[tokio::test]
    async fn test_no_quota_means_no_limit() {
        // TenantContext with default (zero) quotas should allow unlimited scheduling
        let ctx = TenantContext::new("unlimited");
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(config);
        let nodes = make_nodes(100);

        for i in 0..20 {
            let agent = make_tenant_agent(&format!("a{}", i), "unlimited", Priority::Medium);
            let result = scheduler.schedule_agent_with_tenant(&agent, &nodes, Some(&ctx)).await;
            assert!(result.is_ok(), "Agent {} should succeed with no limits", i);
        }

        let stats = scheduler.get_tenant_stats("unlimited").await.unwrap();
        assert_eq!(stats.active_agents, 20);
        assert_eq!(stats.schedules_rejected_quota, 0);
    }
}
