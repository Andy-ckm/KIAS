
use crate::error::{KiasError};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{info!, warn!};

/// Represents a single entry in the cache with metadata
#[derive(Debug)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    last_accessed: Instant,
    ttl: Option<Duration>,
    access_count: u64,
}

/// Statistics for a single cache layer
#[derive(Debug, Default, Clone)]
pub struct LayerStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub insertions: u64,
    pub invalidations: u64,
}

/// Configuration for a single cache layer
#[derive(Debug, Clone)]
pub struct LayerConfig {
    pub name: String,
    pub capacity: usize,
    pub ttl: Option<Duration>,
    pub eviction_threshold: f64,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            capacity: 100,
            ttl: Some(Duration::from_secs(300)),
            eviction_threshold: 0.8,
        }
    }
}