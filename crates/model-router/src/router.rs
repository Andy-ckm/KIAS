//! Model router with intelligent load balancing and failover.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{RouterError, RouterResult};
use crate::provider::{OpenAICompatibleProvider, Provider, ProviderConfig};
use crate::types::*;

// ---------------------------------------------------------------------------
// Router Configuration
// ---------------------------------------------------------------------------

/// Configuration for the model router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Default routing strategy.
    pub default_strategy: RoutingStrategy,
    /// Maximum retries on failure.
    pub max_retries: u32,
    /// Enable request caching.
    pub cache_enabled: bool,
    /// Cache TTL (seconds).
    pub cache_ttl_secs: u64,
    /// Enable circuit breaker.
    pub circuit_breaker_enabled: bool,
    /// Circuit breaker failure threshold.
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker recovery timeout (seconds).
    pub circuit_breaker_recovery_secs: u64,
    /// Global monthly budget (USD).
    pub global_budget: Option<f64>,
    /// Enable request logging.
    pub logging_enabled: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            default_strategy: RoutingStrategy::RoundRobin,
            max_retries: 3,
            cache_enabled: false,
            cache_ttl_secs: 300,
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_recovery_secs: 60,
            global_budget: None,
            logging_enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Request Cache
// ---------------------------------------------------------------------------

/// Cached response entry.
struct CacheEntry {
    response: ChatResponse,
    created_at: Instant,
    ttl: Duration,
}

/// Simple request cache.
struct RequestCache {
    entries: DashMap<String, CacheEntry>,
}

impl RequestCache {
    fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<ChatResponse> {
        let expired = {
            if let Some(entry) = self.entries.get(key) {
                if entry.created_at.elapsed() < entry.ttl {
                    return Some(entry.response.clone());
                }
                true
            } else {
                false
            }
        }; // read guard dropped here
        if expired {
            self.entries.remove(key);
        }
        None
    }

    fn set(&self, key: String, response: ChatResponse, ttl: Duration) {
        self.entries.insert(
            key,
            CacheEntry {
                response,
                created_at: Instant::now(),
                ttl,
            },
        );
    }

    fn clear(&self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Circuit Breaker
// ---------------------------------------------------------------------------

/// Circuit breaker state per provider.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CircuitState {
    /// Normal operation — requests flow through.
    Closed,
    /// Too many failures — requests are blocked.
    Open,
    /// Testing recovery — limited requests allowed.
    HalfOpen,
}

/// Per-provider circuit breaker tracker.
struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    threshold: u32,
    last_failure: Option<Instant>,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: u32, recovery_secs: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            threshold,
            last_failure: None,
            recovery_timeout: Duration::from_secs(recovery_secs),
        }
    }

    /// Check if the circuit allows a request through.
    fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = self.last_failure {
                    if last.elapsed() >= self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request.
    fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
    }

    /// Record a failed request.
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        if self.failure_count >= self.threshold {
            self.state = CircuitState::Open;
        }
    }
}

// ---------------------------------------------------------------------------
// Model Router
// ---------------------------------------------------------------------------

/// Intelligent multi-model router.
pub struct ModelRouter {
    /// Router configuration.
    config: RouterConfig,
    /// Registered providers.
    providers: Arc<RwLock<Vec<Box<dyn Provider>>>>,
    /// Round-robin counter.
    round_robin_counter: Arc<RwLock<u64>>,
    /// Request cache.
    cache: RequestCache,
    /// Cost tracking per user.
    user_costs: Arc<DashMap<String, f64>>,
    /// Total cost.
    total_cost: Arc<RwLock<f64>>,
    /// Per-provider circuit breakers.
    circuit_breakers: DashMap<usize, std::sync::Mutex<CircuitBreaker>>,
}

impl ModelRouter {
    /// Create a new model router.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config,
            providers: Arc::new(RwLock::new(Vec::new())),
            round_robin_counter: Arc::new(RwLock::new(0)),
            cache: RequestCache::new(),
            user_costs: Arc::new(DashMap::new()),
            total_cost: Arc::new(RwLock::new(0.0)),
            circuit_breakers: DashMap::new(),
        }
    }

    /// Add a provider from configuration.
    pub async fn add_provider(&self, config: ProviderConfig) -> RouterResult<()> {
        let provider = OpenAICompatibleProvider::new(config)?;
        let mut providers = self.providers.write().await;
        let idx = providers.len();
        providers.push(Box::new(provider));
        if self.config.circuit_breaker_enabled {
            self.circuit_breakers.insert(
                idx,
                std::sync::Mutex::new(CircuitBreaker::new(
                    self.config.circuit_breaker_threshold,
                    self.config.circuit_breaker_recovery_secs,
                )),
            );
        }
        Ok(())
    }

    /// Add a custom provider.
    pub async fn add_custom_provider(&self, provider: Box<dyn Provider>) {
        let mut providers = self.providers.write().await;
        let idx = providers.len();
        providers.push(provider);
        if self.config.circuit_breaker_enabled {
            self.circuit_breakers.insert(
                idx,
                std::sync::Mutex::new(CircuitBreaker::new(
                    self.config.circuit_breaker_threshold,
                    self.config.circuit_breaker_recovery_secs,
                )),
            );
        }
    }

    /// Get available providers for a model.
    async fn get_available_providers(&self, model: &str) -> Vec<usize> {
        let providers = self.providers.read().await;
        let mut available = Vec::new();

        for (i, provider) in providers.iter().enumerate() {
            if provider.supports_model(model) {
                // Check circuit breaker
                if self.config.circuit_breaker_enabled {
                    if let Some(cb) = self.circuit_breakers.get(&i) {
                        if let Ok(mut breaker) = cb.lock() {
                            if !breaker.allow_request() {
                                debug!(provider = i, "Circuit breaker open, skipping");
                                continue;
                            }
                        }
                    }
                }
                let health = provider.health().await;
                if health.healthy {
                    available.push(i);
                }
            }
        }

        available
    }

    /// Select a provider based on routing strategy.
    async fn select_provider(
        &self,
        model: &str,
        preference: &Option<RoutingPreference>,
    ) -> RouterResult<usize> {
        let available = self.get_available_providers(model).await;

        if available.is_empty() {
            return Err(RouterError::NoAvailableProvider(model.to_string()));
        }

        let strategy = preference
            .as_ref()
            .and_then(|p| p.strategy.clone())
            .unwrap_or_else(|| self.config.default_strategy.clone());

        let providers = self.providers.read().await;

        match strategy {
            RoutingStrategy::RoundRobin => {
                let mut counter = self.round_robin_counter.write().await;
                let idx = (*counter % available.len() as u64) as usize;
                *counter += 1;
                Ok(available[idx])
            }
            RoutingStrategy::LeastLatency => {
                let mut best_idx = available[0];
                let mut best_latency = u64::MAX;

                for &idx in &available {
                    let health = providers[idx].health().await;
                    if health.avg_latency_ms < best_latency {
                        best_latency = health.avg_latency_ms;
                        best_idx = idx;
                    }
                }

                Ok(best_idx)
            }
            RoutingStrategy::CostOptimized => {
                // For cost optimization, we'd need model pricing info
                // For now, fall back to round-robin
                let mut counter = self.round_robin_counter.write().await;
                let idx = (*counter % available.len() as u64) as usize;
                *counter += 1;
                Ok(available[idx])
            }
            RoutingStrategy::WeightedRandom => {
                // Weighted random selection
                let total_weight: f64 = available
                    .iter()
                    .map(|&idx| providers[idx].config().weight)
                    .sum();

                let mut rand_val = rand::random::<f64>() * total_weight;

                for &idx in &available {
                    rand_val -= providers[idx].config().weight;
                    if rand_val <= 0.0 {
                        return Ok(idx);
                    }
                }

                Ok(available[0])
            }
            RoutingStrategy::Pinned(ref provider_name) => {
                for &idx in &available {
                    if providers[idx].name() == provider_name {
                        return Ok(idx);
                    }
                }
                Err(RouterError::NoAvailableProvider(format!(
                    "Pinned provider {} not available",
                    provider_name
                )))
            }
            RoutingStrategy::CapabilityBased => {
                // Check required capabilities
                if let Some(_pref) = preference {
                    // In a real implementation, we'd check model capabilities
                    // For now, return first available
                    if let Some(&idx) = available.first() {
                        return Ok(idx);
                    }
                }
                Ok(available[0])
            }
            RoutingStrategy::LeastBusy => {
                // Route to provider with fewest active requests
                let mut best_idx = available[0];
                let mut best_requests = u64::MAX;

                for &idx in &available {
                    let health = providers[idx].health().await;
                    if health.total_requests < best_requests {
                        best_requests = health.total_requests;
                        best_idx = idx;
                    }
                }

                Ok(best_idx)
            }
            RoutingStrategy::UsageBased => {
                // Route based on usage (TPM/RPM) - select provider with lowest usage
                let mut best_idx = available[0];
                let mut best_usage = u64::MAX;

                for &idx in &available {
                    let health = providers[idx].health().await;
                    // Use total_requests as proxy for usage
                    if health.total_requests < best_usage {
                        best_usage = health.total_requests;
                        best_idx = idx;
                    }
                }

                Ok(best_idx)
            }
        }
    }

    /// Generate cache key for a request.
    fn cache_key(&self, request: &ChatRequest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        for msg in &request.messages {
            msg.role.hash(&mut hasher);
            msg.content.hash(&mut hasher);
        }
        if let Some(t) = request.temperature {
            t.to_bits().hash(&mut hasher);
        }
        request.max_tokens.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Execute a chat completion request.
    pub async fn chat(&self, request: ChatRequest) -> RouterResult<ChatResponse> {
        // Check cache
        if self.config.cache_enabled {
            let key = self.cache_key(&request);
            if let Some(cached) = self.cache.get(&key) {
                debug!("Cache hit for request");
                return Ok(cached);
            }
        }

        // Check budget
        if let Some(budget) = self.config.global_budget {
            let total = self.total_cost.read().await;
            if *total >= budget {
                return Err(RouterError::BudgetExceeded {
                    spent: *total,
                    limit: budget,
                });
            }
        }

        // Try with retries
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            let provider_idx = self
                .select_provider(&request.model, &request.routing)
                .await?;

            let providers = self.providers.read().await;
            let provider = &providers[provider_idx];

            match provider.chat(&request).await {
                Ok(response) => {
                    // Update cost tracking
                    {
                        let mut total = self.total_cost.write().await;
                        *total += response.cost_usd;
                    }

                    if let Some(ref user) = request.user {
                        self.user_costs
                            .entry(user.clone())
                            .and_modify(|c| *c += response.cost_usd)
                            .or_insert(response.cost_usd);
                    }

                    // Cache response
                    if self.config.cache_enabled {
                        let key = self.cache_key(&request);
                        self.cache.set(
                            key,
                            response.clone(),
                            Duration::from_secs(self.config.cache_ttl_secs),
                        );
                    }

                    if self.config.logging_enabled {
                        info!(
                            model = %request.model,
                            provider = %response.provider,
                            latency_ms = response.latency_ms,
                            tokens = response.usage.total_tokens,
                            cost_usd = response.cost_usd,
                            "Chat completion successful"
                        );
                    }

                    // Record success in circuit breaker
                    if self.config.circuit_breaker_enabled {
                        if let Some(cb) = self.circuit_breakers.get(&provider_idx) {
                            if let Ok(mut breaker) = cb.lock() {
                                breaker.record_success();
                            }
                        }
                    }

                    return Ok(response);
                }
                Err(e) => {
                    warn!(
                        provider = provider.name(),
                        attempt = attempt + 1,
                        error = %e,
                        "Provider failed, retrying"
                    );
                    last_error = Some(e);
                    // Record failure in circuit breaker
                    if self.config.circuit_breaker_enabled {
                        if let Some(cb) = self.circuit_breakers.get(&provider_idx) {
                            if let Ok(mut breaker) = cb.lock() {
                                breaker.record_failure();
                            }
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            RouterError::AllProvidersFailed("No providers available".to_string())
        }))
    }

    /// Execute an embedding request.
    pub async fn embedding(&self, request: EmbeddingRequest) -> RouterResult<EmbeddingResponse> {
        let available = self.get_available_providers(&request.model).await;

        if available.is_empty() {
            return Err(RouterError::NoAvailableProvider(request.model.clone()));
        }

        let providers = self.providers.read().await;
        let provider = &providers[available[0]];

        provider.embedding(&request).await
    }

    /// Get router statistics.
    pub async fn stats(&self) -> RouterStats {
        let providers = self.providers.read().await;
        let mut provider_stats = Vec::new();

        for provider in providers.iter() {
            let health = provider.health().await;
            provider_stats.push(ProviderStats {
                name: provider.name().to_string(),
                healthy: health.healthy,
                success_rate: health.success_rate,
                avg_latency_ms: health.avg_latency_ms,
                total_requests: health.total_requests,
                current_cost: provider.current_cost().await,
            });
        }

        let total_cost = self.total_cost.read().await;

        RouterStats {
            providers: provider_stats,
            total_cost: *total_cost,
            cache_size: self.cache.entries.len(),
        }
    }

    /// Get user cost.
    pub async fn user_cost(&self, user: &str) -> f64 {
        self.user_costs.get(user).map(|c| *c).unwrap_or(0.0)
    }

    /// Clear cache.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

/// Router statistics.
#[derive(Debug, Clone, Serialize)]
pub struct RouterStats {
    /// Provider statistics.
    pub providers: Vec<ProviderStats>,
    /// Total cost (USD).
    pub total_cost: f64,
    /// Cache size.
    pub cache_size: usize,
}

/// Provider statistics.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderStats {
    /// Provider name.
    pub name: String,
    /// Whether provider is healthy.
    pub healthy: bool,
    /// Success rate.
    pub success_rate: f64,
    /// Average latency (ms).
    pub avg_latency_ms: u64,
    /// Total requests.
    pub total_requests: u64,
    /// Current cost (USD).
    pub current_cost: f64,
}

// Simple random number generator for weighted random
mod rand {
    use std::cell::Cell;

    thread_local! {
        static RNG: Cell<u64> = const { Cell::new(1) };
    }

    pub fn random<T: FromRandom>() -> T {
        T::from_random()
    }

    pub trait FromRandom {
        fn from_random() -> Self;
    }

    impl FromRandom for f64 {
        fn from_random() -> Self {
            RNG.with(|rng| {
                let mut state = rng.get();
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                rng.set(state);
                (state as f64) / (u64::MAX as f64)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_config_default() {
        let config = RouterConfig::default();
        assert_eq!(config.default_strategy, RoutingStrategy::RoundRobin);
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_model_router_creation() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let stats = router.stats().await;
        assert_eq!(stats.providers.len(), 0);
    }

    #[tokio::test]
    async fn test_model_router_add_provider() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let provider_config = ProviderConfig::openai("test", "sk-test", vec!["gpt-4".to_string()]);

        router.add_provider(provider_config).await.unwrap();

        let stats = router.stats().await;
        assert_eq!(stats.providers.len(), 1);
    }

    #[test]
    fn test_cache_key_generation() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            user: None,
            routing: None,
        };

        let key = router.cache_key(&request);
        assert!(!key.is_empty());
    }

    // ── Circuit Breaker Tests ─────────────────────────────────────

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let mut cb = CircuitBreaker::new(3, 60);
        assert_eq!(cb.state, CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(3, 60);

        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let mut cb = CircuitBreaker::new(3, 60);

        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.failure_count, 0);
    }

    #[test]
    fn test_circuit_breaker_half_open_after_recovery() {
        let mut cb = CircuitBreaker::new(2, 0); // 0s recovery = immediate

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        // With 0s recovery timeout, should transition to HalfOpen
        assert!(cb.allow_request());
        assert_eq!(cb.state, CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_allows_request() {
        let mut cb = CircuitBreaker::new(2, 0);

        cb.record_failure();
        cb.record_failure();
        // Force to half-open
        let _ = cb.allow_request();
        assert_eq!(cb.state, CircuitState::HalfOpen);
        // Half-open still allows requests
        assert!(cb.allow_request());
    }

    #[test]
    fn test_router_config_circuit_breaker_defaults() {
        let config = RouterConfig::default();
        assert!(config.circuit_breaker_enabled);
        assert_eq!(config.circuit_breaker_threshold, 5);
        assert_eq!(config.circuit_breaker_recovery_secs, 60);
    }

    // ── RequestCache Tests ────────────────────────────────────────

    #[test]
    fn test_request_cache_set_and_get() {
        let cache = RequestCache::new();
        let response = ChatResponse {
            id: "test-1".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![],
            usage: Usage::default(),
            latency_ms: 100,
            cost_usd: 0.01,
        };

        cache.set("key1".to_string(), response.clone(), Duration::from_secs(60));
        let cached = cache.get("key1");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, "test-1");
    }

    #[test]
    fn test_request_cache_miss() {
        let cache = RequestCache::new();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_request_cache_expired() {
        let cache = RequestCache::new();
        let response = ChatResponse {
            id: "test-2".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![],
            usage: Usage::default(),
            latency_ms: 100,
            cost_usd: 0.01,
        };

        // Set with very short TTL
        cache.set("key2".to_string(), response, Duration::from_millis(1));

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("key2").is_none());
    }

    #[test]
    fn test_request_cache_clear() {
        let cache = RequestCache::new();
        let response = ChatResponse {
            id: "test-3".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![],
            usage: Usage::default(),
            latency_ms: 100,
            cost_usd: 0.01,
        };

        cache.set("key3".to_string(), response, Duration::from_secs(60));
        assert!(cache.get("key3").is_some());

        cache.clear();
        assert!(cache.get("key3").is_none());
    }

    #[test]
    fn test_request_cache_overwrite() {
        let cache = RequestCache::new();
        let response1 = ChatResponse {
            id: "first".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![],
            usage: Usage::default(),
            latency_ms: 100,
            cost_usd: 0.01,
        };
        let response2 = ChatResponse {
            id: "second".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            choices: vec![],
            usage: Usage::default(),
            latency_ms: 200,
            cost_usd: 0.02,
        };

        cache.set("key".to_string(), response1, Duration::from_secs(60));
        cache.set("key".to_string(), response2, Duration::from_secs(60));

        let cached = cache.get("key").unwrap();
        assert_eq!(cached.id, "second");
    }

    // ── Circuit Breaker Additional Tests ──────────────────────────

    #[test]
    fn test_circuit_breaker_half_open_success_closes() {
        let mut cb = CircuitBreaker::new(2, 0);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        // Transition to half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state, CircuitState::HalfOpen);

        // Success in half-open closes the circuit
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.failure_count, 0);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(2, 0);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        // Transition to half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state, CircuitState::HalfOpen);

        // Failure in half-open reopens
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_threshold_boundary() {
        let mut cb = CircuitBreaker::new(3, 60);

        // Exactly at threshold - 1
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Closed);

        // At threshold
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_open_blocks_requests() {
        let mut cb = CircuitBreaker::new(1, 60);

        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.allow_request());
    }

    // ── RouterConfig Tests ────────────────────────────────────────

    #[test]
    fn test_router_config_custom() {
        let config = RouterConfig {
            default_strategy: RoutingStrategy::LeastLatency,
            max_retries: 5,
            cache_enabled: true,
            cache_ttl_secs: 600,
            circuit_breaker_enabled: false,
            circuit_breaker_threshold: 10,
            circuit_breaker_recovery_secs: 120,
            global_budget: Some(100.0),
            logging_enabled: false,
        };

        assert_eq!(config.default_strategy, RoutingStrategy::LeastLatency);
        assert_eq!(config.max_retries, 5);
        assert!(config.cache_enabled);
        assert_eq!(config.cache_ttl_secs, 600);
        assert!(!config.circuit_breaker_enabled);
        assert_eq!(config.global_budget, Some(100.0));
    }

    #[test]
    fn test_router_config_serialization() {
        let config = RouterConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RouterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.default_strategy, RoutingStrategy::RoundRobin);
        assert_eq!(deserialized.max_retries, 3);
    }

    // ── Cache Key Tests ───────────────────────────────────────────

    #[test]
    fn test_cache_key_different_messages() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let req1 = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            user: None,
            routing: None,
        };

        let req2 = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "World".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            user: None,
            routing: None,
        };

        let key1 = router.cache_key(&req1);
        let key2 = router.cache_key(&req2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_key_same_input_same_key() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let make_request = || ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            user: None,
            routing: None,
        };

        let key1 = router.cache_key(&make_request());
        let key2 = router.cache_key(&make_request());
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_temperature_sensitive() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let make_request = |temp: Option<f64>| ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_call_id: None,
                tool_calls: None,
            }],
            temperature: temp,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            user: None,
            routing: None,
        };

        let key1 = router.cache_key(&make_request(Some(0.5)));
        let key2 = router.cache_key(&make_request(Some(0.9)));
        assert_ne!(key1, key2);
    }

    // ── RouterStats Tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_router_stats_empty() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let stats = router.stats().await;
        assert_eq!(stats.providers.len(), 0);
        assert_eq!(stats.total_cost, 0.0);
        assert_eq!(stats.cache_size, 0);
    }

    #[tokio::test]
    async fn test_router_user_cost_default() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let cost = router.user_cost("nonexistent-user").await;
        assert_eq!(cost, 0.0);
    }

    #[tokio::test]
    async fn test_router_multiple_providers() {
        let config = RouterConfig::default();
        let router = ModelRouter::new(config);

        let config1 = ProviderConfig::openai("openai", "sk-1", vec!["gpt-4".to_string()]);
        let config2 = ProviderConfig::anthropic("anthropic", "sk-ant-1", vec!["claude-3".to_string()]);

        router.add_provider(config1).await.unwrap();
        router.add_provider(config2).await.unwrap();

        let stats = router.stats().await;
        assert_eq!(stats.providers.len(), 2);
        assert_eq!(stats.providers[0].name, "openai");
        assert_eq!(stats.providers[1].name, "anthropic");
    }
}
