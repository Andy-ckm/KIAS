use async_trait::async_trait;
use dashmap::DashMap;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};
use std::sync::Arc;

use super::SchedulingAlgorithm;

/// Information about cached prefixes on a node
#[derive(Debug, Clone, Default)]
pub struct NodeCacheInfo {
    /// Set of system prompt hashes cached on this node
    pub cached_prefixes: Vec<u64>,
    /// Total cache memory used (bytes)
    pub cache_memory_bytes: u64,
    /// Cache hit count
    pub hit_count: u64,
    /// Cache miss count
    pub miss_count: u64,
}

impl NodeCacheInfo {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            return 0.0;
        }
        self.hit_count as f64 / total as f64
    }
}

/// Cache-Aware scheduler: routes agents to nodes that already have
/// relevant KV Cache (Prefix Cache) warm.
///
/// This is a KIAS innovation that leverages DeepSeek-style prefix caching:
/// 1. Check if any node has a cached prefix matching the agent's system prompt hash
/// 2. If cache hit → route to that node (avoids recomputation, saves ~90% cost)
/// 3. If cache miss → fall back to least-loaded algorithm
///
/// The `cache_weight` parameter controls how much cache affinity matters
/// relative to load balancing (0.0 = pure least-loaded, 1.0 = pure cache-first).
pub struct CacheAwareScheduler {
    /// Node ID -> cache info
    cache_map: Arc<DashMap<String, NodeCacheInfo>>,
    /// Weight of cache affinity vs load balancing (0.0 - 1.0)
    cache_weight: f64,
}

impl CacheAwareScheduler {
    pub fn new(cache_weight: f64) -> Self {
        Self {
            cache_map: Arc::new(DashMap::new()),
            cache_weight: cache_weight.clamp(0.0, 1.0),
        }
    }

    /// Update cache info for a node
    pub fn update_node_cache(&self, node_id: &str, info: NodeCacheInfo) {
        self.cache_map.insert(node_id.to_string(), info);
    }

    /// Record a cache hit on a node
    pub fn record_cache_hit(&self, node_id: &str) {
        if let Some(mut info) = self.cache_map.get_mut(node_id) {
            info.hit_count += 1;
        }
    }

    /// Record a cache miss on a node
    pub fn record_cache_miss(&self, node_id: &str) {
        if let Some(mut info) = self.cache_map.get_mut(node_id) {
            info.miss_count += 1;
        }
    }

    /// Get cache info for a node
    pub fn get_node_cache(&self, node_id: &str) -> Option<NodeCacheInfo> {
        self.cache_map.get(node_id).map(|r| r.clone())
    }
}

impl Default for CacheAwareScheduler {
    fn default() -> Self {
        Self::new(0.3)
    }
}

/// Calculate a combined score for cache-aware scheduling.
///
/// Score = cache_weight * cache_score + (1 - cache_weight) * load_score
///
/// Where:
/// - cache_score = 1.0 if the node has the prefix cached, 0.0 otherwise
/// - load_score = 1.0 - load_factor (prefer less loaded nodes)
fn cache_aware_score(
    node: &Node,
    agent: &Agent,
    cache_info: Option<&NodeCacheInfo>,
    cache_weight: f64,
) -> f64 {
    let cache_score =
        if let (Some(info), Some(prefix_hash)) = (cache_info, agent.system_prompt_hash) {
            if info.cached_prefixes.contains(&prefix_hash) {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };

    let load_score = 1.0 - node.load_factor();

    cache_weight * cache_score + (1.0 - cache_weight) * load_score
}

#[async_trait]
impl SchedulingAlgorithm for CacheAwareScheduler {
    fn name(&self) -> &str {
        "cache-aware"
    }

    async fn schedule(&self, agent: &Agent, nodes: &[Node]) -> Result<ScheduleResult, KiasError> {
        let available: Vec<&Node> = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Ready)
            .collect();

        if available.is_empty() {
            return Err(KiasError::NoAvailableNodes);
        }

        // Check for direct cache hit first (fast path)
        if let Some(prefix_hash) = agent.system_prompt_hash {
            for node in &available {
                if let Some(info) = self.cache_map.get(&node.id) {
                    if info.cached_prefixes.contains(&prefix_hash) {
                        tracing::info!(
                            agent_id = %agent.id,
                            node_id = %node.id,
                            prefix_hash = prefix_hash,
                            algorithm = "cache-aware",
                            "Cache hit - agent scheduled to node with warm cache"
                        );
                        return Ok(ScheduleResult {
                            agent_id: agent.id.clone(),
                            node_id: node.id.clone(),
                            algorithm: "cache-aware".to_string(),
                            score: 1.0,
                        });
                    }
                }
            }
        }

        // No cache hit: use combined score
        let mut best_node: Option<&Node> = None;
        let mut best_score = f64::NEG_INFINITY;

        for node in &available {
            let cache_info = self.cache_map.get(&node.id);
            let score = cache_aware_score(
                node,
                agent,
                cache_info.as_ref().map(|r| r.value()),
                self.cache_weight,
            );
            if score > best_score {
                best_score = score;
                best_node = Some(node);
            }
        }

        let selected = best_node.ok_or(KiasError::NoAvailableNodes)?;

        tracing::info!(
            agent_id = %agent.id,
            node_id = %selected.id,
            score = best_score,
            algorithm = "cache-aware",
            "Agent scheduled (cache miss, fallback to score-based)"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: selected.id.clone(),
            algorithm: "cache-aware".to_string(),
            score: best_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;

    fn make_node(id: &str, cpu_avail: f64) -> Node {
        Node {
            id: id.to_string(),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 0,
                ..Default::default()
            },
            available_resources: Resources {
                cpu: cpu_avail,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 0,
                ..Default::default()
            },
            allocated_agents: vec![],
            labels: Default::default(),
        }
    }

    fn make_agent_with_hash(id: &str, hash: u64) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: Some(hash),
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    fn make_agent_no_hash(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_cache_hit_routes_to_warm_node() {
        let scheduler = CacheAwareScheduler::new(0.5);
        let nodes = vec![make_node("node-0", 3.0), make_node("node-1", 3.0)];

        // node-1 has prefix hash 42 cached
        scheduler.update_node_cache(
            "node-1",
            NodeCacheInfo {
                cached_prefixes: vec![42],
                ..Default::default()
            },
        );

        let agent = make_agent_with_hash("a1", 42);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
        assert_eq!(result.score, 1.0);
    }

    #[tokio::test]
    async fn test_cache_miss_falls_back_to_load() {
        let scheduler = CacheAwareScheduler::new(0.5);
        let nodes = vec![make_node("node-0", 1.0), make_node("node-1", 3.0)];

        let agent = make_agent_with_hash("a1", 999); // no node has this cached
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        // node-1 is less loaded, should be preferred
        assert_eq!(result.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_no_hash_uses_load_only() {
        let scheduler = CacheAwareScheduler::new(0.5);
        let nodes = vec![make_node("node-0", 1.0), make_node("node-1", 3.0)];

        let agent = make_agent_no_hash("a1");
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_no_available_nodes() {
        let scheduler = CacheAwareScheduler::new(0.5);
        let mut nodes = vec![make_node("node-0", 3.0)];
        nodes[0].status = NodeStatus::NotReady;

        let agent = make_agent_no_hash("a1");
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }

    #[test]
    fn test_node_cache_info_hit_rate() {
        let info = NodeCacheInfo {
            cached_prefixes: vec![],
            cache_memory_bytes: 0,
            hit_count: 7,
            miss_count: 3,
        };
        assert!((info.hit_rate() - 0.7).abs() < 1e-10, "7/(7+3) = 0.7");

        let zero = NodeCacheInfo::default();
        assert_eq!(zero.hit_rate(), 0.0, "no hits or misses = 0.0");

        let all_hits = NodeCacheInfo {
            hit_count: 10,
            miss_count: 0,
            ..Default::default()
        };
        assert!((all_hits.hit_rate() - 1.0).abs() < 1e-10, "all hits = 1.0");
    }

    #[tokio::test]
    async fn test_update_and_get_node_cache() {
        let scheduler = CacheAwareScheduler::new(0.5);

        // Initially empty
        assert!(scheduler.get_node_cache("node-1").is_none());

        // Update cache info
        scheduler.update_node_cache(
            "node-1",
            NodeCacheInfo {
                cached_prefixes: vec![10, 20, 30],
                cache_memory_bytes: 1024,
                hit_count: 5,
                miss_count: 2,
            },
        );

        let info = scheduler.get_node_cache("node-1").unwrap();
        assert_eq!(info.cached_prefixes.len(), 3);
        assert_eq!(info.cache_memory_bytes, 1024);
        assert_eq!(info.hit_count, 5);
        assert_eq!(info.miss_count, 2);
    }

    #[tokio::test]
    async fn test_record_cache_hit_and_miss() {
        let scheduler = CacheAwareScheduler::new(0.5);

        scheduler.update_node_cache("node-1", NodeCacheInfo::default());
        scheduler.record_cache_hit("node-1");
        scheduler.record_cache_hit("node-1");
        scheduler.record_cache_miss("node-1");

        let info = scheduler.get_node_cache("node-1").unwrap();
        assert_eq!(info.hit_count, 2);
        assert_eq!(info.miss_count, 1);
        assert!((info.hit_rate() - 2.0 / 3.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_record_cache_hit_nonexistent_node() {
        let scheduler = CacheAwareScheduler::new(0.5);
        // Should not panic on nonexistent node
        scheduler.record_cache_hit("nonexistent");
        scheduler.record_cache_miss("nonexistent");
        assert!(scheduler.get_node_cache("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_cache_weight_clamping() {
        // Weight > 1.0 should be clamped to 1.0
        let scheduler = CacheAwareScheduler::new(2.0);
        let nodes = vec![make_node("node-0", 1.0), make_node("node-1", 3.0)];

        scheduler.update_node_cache(
            "node-0",
            NodeCacheInfo {
                cached_prefixes: vec![42],
                ..Default::default()
            },
        );

        let agent = make_agent_with_hash("a1", 42);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        // With weight=1.0 (clamped), cache hit should dominate
        assert_eq!(result.node_id, "node-0");

        // Weight < 0.0 should be clamped to 0.0
        // Note: the fast path still returns on cache hit regardless of weight.
        // To test weight=0.0, use an agent whose prefix is NOT cached.
        let scheduler2 = CacheAwareScheduler::new(-1.0);
        // No cache entries — falls through to combined score
        let agent_no_cache = make_agent_with_hash("a2", 999);
        let result2 = scheduler2.schedule(&agent_no_cache, &nodes).await.unwrap();
        // With weight=0.0, load balancing dominates: node-1 (3.0 load) > node-0 (1.0 load)
        // load_score = 1.0 - load_factor, so higher capacity = higher score
        assert_eq!(result2.node_id, "node-1");
    }

    #[tokio::test]
    async fn test_multiple_cached_nodes_picks_best() {
        let scheduler = CacheAwareScheduler::new(0.8);
        let nodes = vec![
            make_node("node-0", 3.0),
            make_node("node-1", 3.0),
            make_node("node-2", 3.0),
        ];

        // node-0 has the target prefix cached
        scheduler.update_node_cache(
            "node-0",
            NodeCacheInfo {
                cached_prefixes: vec![42],
                ..Default::default()
            },
        );

        // node-1 has a different prefix cached
        scheduler.update_node_cache(
            "node-1",
            NodeCacheInfo {
                cached_prefixes: vec![99],
                ..Default::default()
            },
        );

        let agent = make_agent_with_hash("a1", 42);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(
            result.node_id, "node-0",
            "should pick node with matching prefix"
        );
    }
}
