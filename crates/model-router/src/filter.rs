//! Layered filter pipeline for model routing.
//!
//! Inspired by LiteLLM's 8-layer filtering approach:
//! 1. Access group filtering
//! 2. Health check filtering
//! 3. Cooldown filtering
//! 4. Pre-call checks (context window)
//! 5. Order sorting
//! 6. Strategy selection

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{RouterError, RouterResult};
use crate::provider::Provider;
use crate::types::*;

// ---------------------------------------------------------------------------
// Filter Trait
// ---------------------------------------------------------------------------

/// Trait for routing filters.
#[async_trait::async_trait]
pub trait RoutingFilter: Send + Sync {
    /// Filter name for logging.
    fn name(&self) -> &str;

    /// Apply filter to candidate providers.
    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        request: &ChatRequest,
    ) -> RouterResult<Vec<usize>>;
}

// ---------------------------------------------------------------------------
// Health Check Filter
// ---------------------------------------------------------------------------

/// Filters out unhealthy providers.
pub struct HealthCheckFilter;

#[async_trait::async_trait]
impl RoutingFilter for HealthCheckFilter {
    fn name(&self) -> &str {
        "health-check"
    }

    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        _request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        let mut healthy = Vec::new();

        for idx in candidates {
            let health = providers[idx].health().await;
            if health.healthy {
                healthy.push(idx);
            }
        }

        Ok(healthy)
    }
}

// ---------------------------------------------------------------------------
// Cooldown Filter
// ---------------------------------------------------------------------------

/// Filters out providers in cooldown (recently failed).
pub struct CooldownFilter {
    /// Cooldown duration after failure.
    cooldown_duration: Duration,
    /// Provider cooldown timestamps.
    cooldowns: Arc<RwLock<HashMap<String, Instant>>>,
}

impl CooldownFilter {
    pub fn new(cooldown_duration: Duration) -> Self {
        Self {
            cooldown_duration,
            cooldowns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Put a provider in cooldown.
    pub async fn cooldown(&self, provider_name: &str) {
        let mut cooldowns = self.cooldowns.write().await;
        cooldowns.insert(provider_name.to_string(), Instant::now());
    }

    /// Check if a provider is in cooldown.
    pub async fn is_in_cooldown(&self, provider_name: &str) -> bool {
        let cooldowns = self.cooldowns.read().await;
        if let Some(timestamp) = cooldowns.get(provider_name) {
            timestamp.elapsed() < self.cooldown_duration
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
impl RoutingFilter for CooldownFilter {
    fn name(&self) -> &str {
        "cooldown"
    }

    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        _request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        let mut available = Vec::new();
        let original_candidates = candidates.clone();

        for idx in candidates {
            let provider_name = providers[idx].name();
            if !self.is_in_cooldown(provider_name).await {
                available.push(idx);
            }
        }

        // If all providers are in cooldown, return all (cooldown bypass)
        if available.is_empty() {
            return Ok(original_candidates);
        }

        Ok(available)
    }
}

// ---------------------------------------------------------------------------
// Capability Filter
// ---------------------------------------------------------------------------

/// Filters providers based on required capabilities.
pub struct CapabilityFilter;

#[async_trait::async_trait]
impl RoutingFilter for CapabilityFilter {
    fn name(&self) -> &str {
        "capability"
    }

    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        // Check if request requires specific capabilities
        let required_caps = request
            .routing
            .as_ref()
            .map(|r| &r.required_capabilities)
            .cloned()
            .unwrap_or_default();

        if required_caps.is_empty() {
            return Ok(candidates);
        }

        let mut capable = Vec::new();

        for idx in candidates {
            let provider = &providers[idx];
            // Check if provider supports the model (basic capability check)
            if provider.supports_model(&request.model) {
                capable.push(idx);
            }
        }

        Ok(capable)
    }
}

// ---------------------------------------------------------------------------
// Latency Filter
// ---------------------------------------------------------------------------

/// Filters providers by maximum latency requirement.
pub struct LatencyFilter;

#[async_trait::async_trait]
impl RoutingFilter for LatencyFilter {
    fn name(&self) -> &str {
        "latency"
    }

    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        let max_latency = request
            .routing
            .as_ref()
            .and_then(|r| r.max_latency_ms);

        let max_latency = match max_latency {
            Some(max) => max,
            None => return Ok(candidates),
        };

        let mut fast_enough = Vec::new();
        let original_candidates = candidates.clone();

        for idx in candidates {
            let health = providers[idx].health().await;
            if health.avg_latency_ms <= max_latency {
                fast_enough.push(idx);
            }
        }

        // If no provider meets latency requirement, return all
        if fast_enough.is_empty() {
            return Ok(original_candidates);
        }

        Ok(fast_enough)
    }
}

// ---------------------------------------------------------------------------
// Cost Filter
// ---------------------------------------------------------------------------

/// Filters providers by maximum cost requirement.
pub struct CostFilter;

#[async_trait::async_trait]
impl RoutingFilter for CostFilter {
    fn name(&self) -> &str {
        "cost"
    }

    async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        let max_cost = request
            .routing
            .as_ref()
            .and_then(|r| r.max_cost);

        let _max_cost = match max_cost {
            Some(max) => max,
            None => return Ok(candidates),
        };

        // For now, return all candidates as we don't have per-provider pricing
        // In a real implementation, we'd check provider-specific costs
        Ok(candidates)
    }
}

// ---------------------------------------------------------------------------
// Filter Pipeline
// ---------------------------------------------------------------------------

/// Pipeline of routing filters.
pub struct FilterPipeline {
    filters: Vec<Box<dyn RoutingFilter>>,
}

impl FilterPipeline {
    /// Create a new filter pipeline with default filters.
    pub fn new() -> Self {
        Self {
            filters: vec![
                Box::new(HealthCheckFilter),
                Box::new(CooldownFilter::new(Duration::from_secs(60))),
                Box::new(CapabilityFilter),
                Box::new(LatencyFilter),
                Box::new(CostFilter),
            ],
        }
    }

    /// Create a custom filter pipeline.
    pub fn with_filters(filters: Vec<Box<dyn RoutingFilter>>) -> Self {
        Self { filters }
    }

    /// Apply all filters in sequence.
    pub async fn apply(
        &self,
        candidates: Vec<usize>,
        providers: &[Box<dyn Provider>],
        request: &ChatRequest,
    ) -> RouterResult<Vec<usize>> {
        let mut current = candidates;

        for filter in &self.filters {
            let before = current.len();
            current = filter.apply(current, providers, request).await?;
            let after = current.len();

            if before != after {
                tracing::debug!(
                    filter = filter.name(),
                    before = before,
                    after = after,
                    "Filter reduced candidates"
                );
            }

            if current.is_empty() {
                return Err(RouterError::NoAvailableProvider(format!(
                    "All providers filtered out by {}",
                    filter.name()
                )));
            }
        }

        Ok(current)
    }
}

impl Default for FilterPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_pipeline_creation() {
        let pipeline = FilterPipeline::new();
        assert_eq!(pipeline.filters.len(), 5);
    }

    #[test]
    fn test_cooldown_filter_creation() {
        let filter = CooldownFilter::new(Duration::from_secs(30));
        assert_eq!(filter.name(), "cooldown");
    }
}
