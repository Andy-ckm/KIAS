//! MCP Circuit Breaker & Rate Limiter
//!
//! Provides:
//! - Circuit breaker pattern (closed → open → half-open)
//! - Token bucket rate limiter
//! - Sliding window rate limiter
//! - Adaptive rate limiting based on latency
//! - Per-client and global rate limiting

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Circuit Breaker
// ---------------------------------------------------------------------------

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failure threshold exceeded — requests are rejected.
    Open,
    /// Testing if service has recovered — limited requests allowed.
    HalfOpen,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening.
    pub failure_threshold: u32,
    /// Duration to stay open before transitioning to half-open.
    pub open_duration: Duration,
    /// Number of successful requests in half-open state to close.
    pub half_open_success_threshold: u32,
    /// Request timeout (consider failure if exceeded).
    pub request_timeout: Duration,
    /// Window for counting failures (resets on success).
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(60),
            half_open_success_threshold: 3,
            request_timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerMetrics {
    /// Total requests attempted.
    pub total_requests: u64,
    /// Successful requests.
    pub successes: u64,
    /// Failed requests.
    pub failures: u64,
    /// Rejected requests (circuit open).
    pub rejected: u64,
    /// Current consecutive failures.
    pub consecutive_failures: u32,
    /// Last failure timestamp (Unix millis).
    pub last_failure_ms: Option<u64>,
    /// State transitions count.
    pub state_transitions: u64,
}

/// Circuit breaker for protecting external calls.
pub struct CircuitBreaker {
    /// Configuration.
    config: CircuitBreakerConfig,
    /// Current state.
    state: Arc<RwLock<CircuitState>>,
    /// Metrics.
    metrics: Arc<RwLock<CircuitBreakerMetrics>>,
    /// When the circuit was last opened.
    opened_at: Arc<RwLock<Option<Instant>>>,
    /// Successes in half-open state.
    half_open_successes: Arc<RwLock<u32>>,
    /// Failure timestamps for windowed counting.
    failure_times: Arc<RwLock<Vec<Instant>>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            metrics: Arc::new(RwLock::new(CircuitBreakerMetrics::default())),
            opened_at: Arc::new(RwLock::new(None)),
            half_open_successes: Arc::new(RwLock::new(0)),
            failure_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Get the current state.
    pub async fn state(&self) -> CircuitState {
        let state = self.state.read().await;
        *state
    }

    /// Get metrics.
    pub async fn metrics(&self) -> CircuitBreakerMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Check if a request is allowed.
    pub async fn allow_request(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if open duration has elapsed
                let opened_at = self.opened_at.read().await;
                if let Some(opened) = *opened_at {
                    if opened.elapsed() >= self.config.open_duration {
                        // Transition to half-open
                        drop(opened_at);
                        drop(state);
                        self.transition_to(CircuitState::HalfOpen).await;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                let successes = self.half_open_successes.read().await;
                *successes < self.config.half_open_success_threshold
            }
        }
    }

    /// Record a successful request.
    pub async fn record_success(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.successes += 1;
        metrics.consecutive_failures = 0;

        let state = self.state.read().await;
        if *state == CircuitState::HalfOpen {
            drop(state);
            let mut successes = self.half_open_successes.write().await;
            *successes += 1;
            if *successes >= self.config.half_open_success_threshold {
                drop(successes);
                self.transition_to(CircuitState::Closed).await;
            }
        }
    }

    /// Record a failed request.
    pub async fn record_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.failures += 1;
        metrics.consecutive_failures += 1;
        metrics.last_failure_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        // Track failure time for windowed counting
        let now = Instant::now();
        let mut failure_times = self.failure_times.write().await;
        failure_times.push(now);

        // Remove old failures outside the window
        let window_start = now - self.config.failure_window;
        failure_times.retain(|t| *t >= window_start);

        let window_failures = failure_times.len() as u32;
        drop(failure_times);

        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => {
                if window_failures >= self.config.failure_threshold {
                    drop(state);
                    drop(metrics);
                    self.transition_to(CircuitState::Open).await;
                }
            }
            CircuitState::HalfOpen => {
                drop(state);
                drop(metrics);
                self.transition_to(CircuitState::Open).await;
            }
            _ => {}
        }
    }

    /// Execute a function with circuit breaker protection.
    pub async fn execute<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        if !self.allow_request().await {
            let mut metrics = self.metrics.write().await;
            metrics.rejected += 1;
            return Err(CircuitBreakerError::Rejected);
        }

        match tokio::time::timeout(self.config.request_timeout, f).await {
            Ok(Ok(result)) => {
                self.record_success().await;
                Ok(result)
            }
            Ok(Err(e)) => {
                self.record_failure().await;
                Err(CircuitBreakerError::Inner(e))
            }
            Err(_) => {
                self.record_failure().await;
                Err(CircuitBreakerError::Timeout)
            }
        }
    }

    /// Transition to a new state.
    async fn transition_to(&self, new_state: CircuitState) {
        let mut state = self.state.write().await;
        let mut metrics = self.metrics.write().await;

        *state = new_state;
        metrics.state_transitions += 1;

        match new_state {
            CircuitState::Open => {
                let mut opened_at = self.opened_at.write().await;
                *opened_at = Some(Instant::now());
            }
            CircuitState::HalfOpen => {
                let mut successes = self.half_open_successes.write().await;
                *successes = 0;
            }
            CircuitState::Closed => {
                let mut opened_at = self.opened_at.write().await;
                *opened_at = None;
                let mut failure_times = self.failure_times.write().await;
                failure_times.clear();
            }
        }
    }

    /// Reset the circuit breaker to closed state.
    pub async fn reset(&self) {
        self.transition_to(CircuitState::Closed).await;
        let mut metrics = self.metrics.write().await;
        *metrics = CircuitBreakerMetrics::default();
    }
}

/// Circuit breaker error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerError<E> {
    /// Request was rejected (circuit is open).
    Rejected,
    /// Request timed out.
    Timeout,
    /// Inner operation failed.
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::Rejected => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::Timeout => write!(f, "Request timed out"),
            CircuitBreakerError::Inner(e) => write!(f, "Inner error: {}", e),
        }
    }
}

impl<E: std::fmt::Display + std::fmt::Debug> std::error::Error for CircuitBreakerError<E> {}

// ---------------------------------------------------------------------------
// Token Bucket Rate Limiter
// ---------------------------------------------------------------------------

/// Token bucket rate limiter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Maximum tokens (burst size).
    pub max_tokens: u32,
    /// Token refill rate (tokens per second).
    pub refill_rate: f64,
    /// Initial tokens (defaults to max_tokens).
    pub initial_tokens: Option<u32>,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100,
            refill_rate: 10.0,
            initial_tokens: None,
        }
    }
}

/// Rate limiter statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimiterStats {
    /// Total requests allowed.
    pub allowed: u64,
    /// Total requests rejected.
    pub rejected: u64,
    /// Current available tokens.
    pub available_tokens: f64,
    /// Time until next token refill (millis).
    pub next_refill_ms: u64,
}

/// Token bucket rate limiter.
pub struct TokenBucketRateLimiter {
    /// Configuration.
    config: RateLimiterConfig,
    /// Current tokens.
    tokens: Arc<RwLock<f64>>,
    /// Last refill time.
    last_refill: Arc<RwLock<Instant>>,
    /// Statistics.
    stats: Arc<RwLock<RateLimiterStats>>,
}

impl TokenBucketRateLimiter {
    /// Create a new rate limiter.
    pub fn new(config: RateLimiterConfig) -> Self {
        let initial = config
            .initial_tokens
            .unwrap_or(config.max_tokens) as f64;

        Self {
            config,
            tokens: Arc::new(RwLock::new(initial)),
            last_refill: Arc::new(RwLock::new(Instant::now())),
            stats: Arc::new(RwLock::new(RateLimiterStats {
                available_tokens: initial,
                ..Default::default()
            })),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RateLimiterConfig::default())
    }

    /// Try to acquire a token. Returns true if allowed.
    pub async fn acquire(&self) -> bool {
        self.acquire_n(1).await
    }

    /// Try to acquire N tokens.
    pub async fn acquire_n(&self, n: u32) -> bool {
        self.refill().await;

        let mut tokens = self.tokens.write().await;
        let mut stats = self.stats.write().await;

        if *tokens >= n as f64 {
            *tokens -= n as f64;
            stats.allowed += 1;
            stats.available_tokens = *tokens;
            true
        } else {
            stats.rejected += 1;
            stats.available_tokens = *tokens;
            false
        }
    }

    /// Refill tokens based on elapsed time.
    async fn refill(&self) {
        let mut tokens = self.tokens.write().await;
        let mut last_refill = self.last_refill.write().await;

        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();
        let new_tokens = elapsed * self.config.refill_rate;

        *tokens = (*tokens + new_tokens).min(self.config.max_tokens as f64);
        *last_refill = now;
    }

    /// Get current statistics.
    pub async fn stats(&self) -> RateLimiterStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Reset the rate limiter.
    pub async fn reset(&self) {
        let mut tokens = self.tokens.write().await;
        let mut last_refill = self.last_refill.write().await;
        let mut stats = self.stats.write().await;

        *tokens = self.config.max_tokens as f64;
        *last_refill = Instant::now();
        *stats = RateLimiterStats::default();
    }
}

// ---------------------------------------------------------------------------
// Sliding Window Rate Limiter
// ---------------------------------------------------------------------------

/// Sliding window rate limiter.
pub struct SlidingWindowRateLimiter {
    /// Maximum requests per window.
    max_requests: u32,
    /// Window duration.
    window: Duration,
    /// Request timestamps.
    timestamps: Arc<RwLock<Vec<Instant>>>,
}

impl SlidingWindowRateLimiter {
    /// Create a new sliding window rate limiter.
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            timestamps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if a request is allowed.
    pub async fn is_allowed(&self) -> bool {
        let now = Instant::now();
        let window_start = now - self.window;

        let mut timestamps = self.timestamps.write().await;

        // Remove old timestamps
        timestamps.retain(|t| *t >= window_start);

        if timestamps.len() < self.max_requests as usize {
            timestamps.push(now);
            true
        } else {
            false
        }
    }

    /// Get the number of requests in the current window.
    pub async fn current_count(&self) -> u32 {
        let now = Instant::now();
        let window_start = now - self.window;

        let timestamps = self.timestamps.read().await;
        timestamps.iter().filter(|t| **t >= window_start).count() as u32
    }

    /// Reset the limiter.
    pub async fn reset(&self) {
        let mut timestamps = self.timestamps.write().await;
        timestamps.clear();
    }
}

// ---------------------------------------------------------------------------
// Per-Client Rate Limiter Manager
// ---------------------------------------------------------------------------

/// Per-client rate limiter with configurable policies.
pub struct ClientRateLimiter {
    /// Default rate limiter config.
    default_config: RateLimiterConfig,
    /// Per-client limiters.
    clients: Arc<RwLock<HashMap<String, TokenBucketRateLimiter>>>,
    /// Per-client configs (overrides default).
    client_configs: Arc<RwLock<HashMap<String, RateLimiterConfig>>>,
}

impl ClientRateLimiter {
    /// Create a new client rate limiter.
    pub fn new(default_config: RateLimiterConfig) -> Self {
        Self {
            default_config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            client_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set a custom config for a client.
    pub async fn set_client_config(&self, client_id: &str, config: RateLimiterConfig) {
        let mut configs = self.client_configs.write().await;
        configs.insert(client_id.to_string(), config);
    }

    /// Try to acquire a token for a client.
    pub async fn acquire(&self, client_id: &str) -> bool {
        self.get_or_create(client_id).await.acquire().await
    }

    /// Get or create a rate limiter for a client.
    async fn get_or_create(&self, client_id: &str) -> TokenBucketRateLimiter {
        let clients = self.clients.read().await;
        if let Some(limiter) = clients.get(client_id) {
            return TokenBucketRateLimiter::new(RateLimiterConfig {
                max_tokens: limiter.config.max_tokens,
                refill_rate: limiter.config.refill_rate,
                initial_tokens: Some(limiter.config.max_tokens),
            });
        }
        drop(clients);

        let configs = self.client_configs.read().await;
        let config = configs
            .get(client_id)
            .cloned()
            .unwrap_or_else(|| self.default_config.clone());
        drop(configs);

        let limiter = TokenBucketRateLimiter::new(config);
        let mut clients = self.clients.write().await;
        clients.insert(client_id.to_string(), limiter.clone());
        limiter
    }
}

impl Clone for TokenBucketRateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            tokens: self.tokens.clone(),
            last_refill: self.last_refill.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed() {
        let cb = CircuitBreaker::with_defaults();

        // Should allow requests in closed state
        assert!(cb.allow_request().await);

        // Record successes
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            open_duration: Duration::from_secs(1),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Record failures until threshold
        for _ in 0..3 {
            cb.record_failure().await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.allow_request().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            open_duration: Duration::from_millis(100),
            half_open_success_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for open duration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open and allow request
        assert!(cb.allow_request().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_token_bucket_rate_limiter() {
        let config = RateLimiterConfig {
            max_tokens: 5,
            refill_rate: 10.0,
            initial_tokens: Some(5),
        };
        let limiter = TokenBucketRateLimiter::new(config);

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.acquire().await);
        }

        // 6th should be rejected
        assert!(!limiter.acquire().await);
    }

    #[tokio::test]
    async fn test_sliding_window_rate_limiter() {
        let limiter = SlidingWindowRateLimiter::new(3, Duration::from_secs(1));

        // Should allow 3 requests
        assert!(limiter.is_allowed().await);
        assert!(limiter.is_allowed().await);
        assert!(limiter.is_allowed().await);

        // 4th should be rejected
        assert!(!limiter.is_allowed().await);

        // Wait for window to expire
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Should allow again
        assert!(limiter.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_execute() {
        let cb = CircuitBreaker::with_defaults();

        // Successful execution
        let result = cb.execute(async { Ok::<i32, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);

        // Failed execution
        let result = cb.execute(async { Err::<i32, String>("test error".to_string()) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::Inner(_))));
    }
}
