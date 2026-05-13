use dashmap::DashMap;
use std::sync::Arc;

/// Cache optimizer: tracks prefix cache state across nodes and provides
/// recommendations for cache warming and eviction.
///
/// Works with the cache-aware scheduler to maximize cache hit rates.
pub struct CacheOptimizer {
    /// Node ID -> list of cached prefix hashes
    node_caches: Arc<DashMap<String, Vec<u64>>>,
    /// Prefix hash -> list of node IDs that have it cached
    prefix_locations: Arc<DashMap<u64, Vec<String>>>,
}

impl CacheOptimizer {
    pub fn new() -> Self {
        Self {
            node_caches: Arc::new(DashMap::new()),
            prefix_locations: Arc::new(DashMap::new()),
        }
    }

    /// Register that a node has a prefix cached
    pub fn register_cache(&self, node_id: &str, prefix_hash: u64) {
        self.node_caches
            .entry(node_id.to_string())
            .or_default()
            .push(prefix_hash);

        self.prefix_locations
            .entry(prefix_hash)
            .or_default()
            .push(node_id.to_string());

        tracing::debug!(
            node_id = %node_id,
            prefix_hash = prefix_hash,
            "Cache registered"
        );
    }

    /// Remove a cache entry (e.g., on eviction)
    pub fn evict_cache(&self, node_id: &str, prefix_hash: u64) {
        if let Some(mut caches) = self.node_caches.get_mut(node_id) {
            caches.retain(|&h| h != prefix_hash);
        }
        if let Some(mut locations) = self.prefix_locations.get_mut(&prefix_hash) {
            locations.retain(|n| n != node_id);
        }

        tracing::debug!(
            node_id = %node_id,
            prefix_hash = prefix_hash,
            "Cache evicted"
        );
    }

    /// Find which nodes have a given prefix cached
    pub fn find_prefix_locations(&self, prefix_hash: u64) -> Vec<String> {
        self.prefix_locations
            .get(&prefix_hash)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Get all cached prefixes on a node
    pub fn get_node_prefixes(&self, node_id: &str) -> Vec<u64> {
        self.node_caches
            .get(node_id)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Suggest a node for cache warming: the node with the most free resources
    /// that doesn't already have the prefix.
    pub fn suggest_warm_target(&self, prefix_hash: u64, node_ids: &[String]) -> Option<String> {
        let locations: Vec<String> = self.find_prefix_locations(prefix_hash);
        node_ids
            .iter()
            .find(|id| !locations.contains(id))
            .cloned()
    }
}

impl Default for CacheOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_find() {
        let optimizer = CacheOptimizer::new();
        optimizer.register_cache("node-0", 42);
        optimizer.register_cache("node-1", 42);
        optimizer.register_cache("node-0", 99);

        let locations = optimizer.find_prefix_locations(42);
        assert_eq!(locations.len(), 2);
        assert!(locations.contains(&"node-0".to_string()));
        assert!(locations.contains(&"node-1".to_string()));

        let prefixes = optimizer.get_node_prefixes("node-0");
        assert_eq!(prefixes.len(), 2);
    }

    #[test]
    fn test_evict() {
        let optimizer = CacheOptimizer::new();
        optimizer.register_cache("node-0", 42);
        optimizer.register_cache("node-1", 42);

        optimizer.evict_cache("node-0", 42);

        let locations = optimizer.find_prefix_locations(42);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0], "node-1");
    }

    #[test]
    fn test_suggest_warm_target() {
        let optimizer = CacheOptimizer::new();
        optimizer.register_cache("node-0", 42);

        let all_nodes = vec!["node-0".to_string(), "node-1".to_string()];
        let target = optimizer.suggest_warm_target(42, &all_nodes);
        assert_eq!(target, Some("node-1".to_string()));
    }
}
