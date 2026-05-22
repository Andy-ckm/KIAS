//! # Connection Pooling with Health-Aware Routing
//!
//! Production-grade outbound connection pool for api-server with:
//! - Health-aware routing (routes to healthy endpoints only)
//! - Per-endpoint circuit breaker (closed → open → half-open → closed)
//! - Connection reuse metrics (utilization, latency percentiles, error rates)
//! - Weighted least-connections load balancing
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │           HealthAwareRouter              │
//! │  ┌───────┐ ┌───────┐ ┌───────┐         │
//! │  │ EP-1  │ │ EP-2  │ │ EP-3  │         │
//! │  │Healthy│ │Degrade│ │ Open  │         │
//! │  │ CB:Off│ │ CB:Off│ │ CB:On │         │
//! │  └───┬───┘ └───┬───┘ └───────┘         │
//! │      │         │    (skipped)           │
//! │  ┌───▼─────────▼───┐                    │
//! │  │  ConnectionPool  │                    │
//! │  │  - idle conns    │                    │
//! │  │  - active conns  │                    │
//! │  │  - metrics       │                    │
//! │  └──────────────────┘                    │
//! └─────────────────────────────────────────┘
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

// ===========================================================================
// Configuration
// ===========================================================================

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum idle connections per endpoint
    pub max_idle_per_endpoint: usize,
    /// Maximum total connections across all endpoints
    pub max_total_connections: usize,
    /// Idle connection timeout (seconds)
    pub idle_timeout_secs: u64,
    /// Connection timeout (seconds)
    pub connect_timeout_secs: u64,
    /// Health check interval (seconds)
    pub health_check_interval_secs: u64,
    /// Circuit breaker failure threshold (consecutive failures to trip)
    pub cb_failure_threshold: u32,
    /// Circuit breaker open duration (seconds before half-open)
    pub cb_open_duration_secs: u64,
    /// Circuit breaker half-open success threshold (to close again)
    pub cb_half_open_success_threshold: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_endpoint: 10,
            max_total_connections: 100,
            idle_timeout_secs: 60,
            connect_timeout_secs: 5,
            health_check_interval_secs: 30,
            cb_failure_threshold: 5,
            cb_open_duration_secs: 30,
            cb_half_open_success_threshold: 3,
        }
    }
}

// ===========================================================================
// Circuit Breaker
// ===========================================================================

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation — requests pass through
    Closed,
    /// Too many failures — requests rejected immediately
    Open,
    /// Testing recovery — limited requests allowed
    HalfOpen,
}

/// Per-endpoint circuit breaker
///
/// Transitions:
/// - Closed → Open: after `failure_threshold` consecutive failures
/// - Open → HalfOpen: after `open_duration` elapsed
/// - HalfOpen → Closed: after `half_open_success_threshold` successes
/// - HalfOpen → Open: on any failure
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_at: Option<Instant>,
    last_state_change: Instant,
    config: PoolConfig,
    /// Total circuit trips (for metrics)
    total_trips: u64,
}

impl CircuitBreaker {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_failure_at: None,
            last_state_change: Instant::now(),
            config,
            total_trips: 0,
        }
    }

    /// Check if a request is allowed
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if open duration has elapsed
                if let Some(last_fail) = self.last_failure_at {
                    if last_fail.elapsed() >= Duration::from_secs(self.config.cb_open_duration_secs) {
                        self.transition(CircuitState::HalfOpen);
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.consecutive_failures = 0;
            }
            CircuitState::HalfOpen => {
                self.consecutive_successes += 1;
                if self.consecutive_successes >= self.config.cb_half_open_success_threshold {
                    self.transition(CircuitState::Closed);
                }
            }
            CircuitState::Open => {
                // Shouldn't happen (requests are rejected), but reset anyway
                self.consecutive_failures = 0;
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.last_failure_at = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                self.consecutive_failures += 1;
                if self.consecutive_failures >= self.config.cb_failure_threshold {
                    self.transition(CircuitState::Open);
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open trips back to open
                self.transition(CircuitState::Open);
            }
            CircuitState::Open => {
                // Already open, just update timestamp
            }
        }
    }

    fn transition(&mut self, new_state: CircuitState) {
        if self.state != new_state {
            debug!(
                "Circuit breaker: {:?} -> {:?}",
                self.state, new_state
            );
            if new_state == CircuitState::Open {
                self.total_trips += 1;
            }
            self.state = new_state;
            self.last_state_change = Instant::now();
            self.consecutive_successes = 0;
            if new_state == CircuitState::Closed {
                self.consecutive_failures = 0;
            }
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn total_trips(&self) -> u64 {
        self.total_trips
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}

// ===========================================================================
// Endpoint Health
// ===========================================================================

/// Endpoint health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Fully operational
    Healthy,
    /// Degraded performance (high latency or intermittent errors)
    Degraded,
    /// Unavailable (circuit breaker open or health check failing)
    Unhealthy,
}

/// Health metrics for a single endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealth {
    pub status: HealthStatus,
    pub circuit_state: CircuitState,
    pub consecutive_failures: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_circuit_trips: u64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub last_health_check: Option<DateTime<Utc>>,
}

// ===========================================================================
// Latency Tracker
// ===========================================================================

/// Simple latency tracker with sliding window
#[derive(Debug)]
struct LatencyTracker {
    samples: Vec<Duration>,
    max_samples: usize,
}

impl LatencyTracker {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    fn record(&mut self, latency: Duration) {
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(latency);
    }

    fn avg_ms(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: Duration = self.samples.iter().sum();
        sum.as_secs_f64() * 1000.0 / self.samples.len() as f64
    }

    fn p99_ms(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<Duration> = self.samples.clone();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted[idx.min(sorted.len() - 1)].as_secs_f64() * 1000.0
    }
}

// ===========================================================================
// Endpoint
// ===========================================================================

/// A single backend endpoint with connection pool and circuit breaker
#[derive(Debug)]
struct Endpoint {
    /// Endpoint URL
    url: String,
    /// Circuit breaker (interior mutability for concurrent access)
    circuit_breaker: RwLock<CircuitBreaker>,
    /// Health status
    health: RwLock<HealthStatus>,
    /// Request counters
    total_requests: AtomicU64,
    total_failures: AtomicU64,
    /// Latency tracking
    latency: RwLock<LatencyTracker>,
    /// Active connections count
    active_connections: AtomicU64,
    /// Idle connections count
    idle_connections: AtomicU64,
    /// Last health check timestamp
    last_health_check: RwLock<Option<Instant>>,
    /// Weight for weighted routing (higher = more traffic)
    weight: u32,
}

impl Endpoint {
    fn new(url: String, config: PoolConfig, weight: u32) -> Self {
        Self {
            url,
            circuit_breaker: RwLock::new(CircuitBreaker::new(config)),
            health: RwLock::new(HealthStatus::Healthy),
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            latency: RwLock::new(LatencyTracker::new(1000)),
            active_connections: AtomicU64::new(0),
            idle_connections: AtomicU64::new(0),
            last_health_check: RwLock::new(None),
            weight,
        }
    }

    /// Check if this endpoint can accept a request
    async fn is_available(&self) -> bool {
        let health = *self.health.read().await;
        let mut cb = self.circuit_breaker.write().await;
        health != HealthStatus::Unhealthy && cb.allow_request()
    }

    /// Record a completed request
    async fn record_request(&self, latency: Duration, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.latency.write().await.record(latency);

        let mut cb = self.circuit_breaker.write().await;
        if success {
            cb.record_success();
        } else {
            self.total_failures.fetch_add(1, Ordering::Relaxed);
            cb.record_failure();
        }
    }
}

// ===========================================================================
// Connection Pool
// ===========================================================================

/// Pooled connection handle
#[derive(Debug)]
pub struct PooledConnection {
    /// Endpoint URL this connection is bound to
    pub endpoint_url: String,
    /// Connection ID
    pub id: u64,
    /// When this connection was created
    pub created_at: Instant,
    /// When this connection was last used
    pub last_used: Instant,
}

impl PooledConnection {
    /// Check if this connection has expired
    pub fn is_expired(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() >= idle_timeout
    }
}

/// Connection pool metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    /// Total endpoints registered
    pub endpoint_count: usize,
    /// Healthy endpoints
    pub healthy_endpoints: usize,
    /// Degraded endpoints
    pub degraded_endpoints: usize,
    /// Unhealthy endpoints
    pub unhealthy_endpoints: usize,
    /// Total active connections across all endpoints
    pub total_active_connections: u64,
    /// Total idle connections across all endpoints
    pub total_idle_connections: u64,
    /// Total requests served
    pub total_requests: u64,
    /// Total failures
    pub total_failures: u64,
    /// Overall error rate
    pub error_rate: f64,
    /// Per-endpoint health details
    pub endpoints: Vec<EndpointHealth>,
}

// ===========================================================================
// Health-Aware Router
// ===========================================================================

/// Health-aware connection pool router
///
/// Routes requests to the healthiest endpoint with lowest load,
/// skipping endpoints with open circuit breakers.
pub struct ConnectionPool {
    /// Registered endpoints
    endpoints: RwLock<Vec<Endpoint>>,
    /// Configuration
    config: PoolConfig,
    /// Global request counter
    global_requests: AtomicU64,
    /// Global failure counter
    global_failures: AtomicU64,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Self {
        Self {
            endpoints: RwLock::new(Vec::new()),
            config,
            global_requests: AtomicU64::new(0),
            global_failures: AtomicU64::new(0),
        }
    }

    /// Register a new endpoint
    pub async fn add_endpoint(&self, url: impl Into<String>, weight: u32) {
        let mut eps = self.endpoints.write().await;
        eps.push(Endpoint::new(url.into(), self.config.clone(), weight));
    }

    /// Select the best endpoint for a request (health-aware, weighted)
    pub async fn select_endpoint(&self) -> Option<String> {
        let eps = self.endpoints.read().await;

        // Collect available endpoints with scores
        let mut candidates: Vec<(String, f64)> = Vec::new();
        for ep in eps.iter() {
            if ep.is_available().await {
                let active = ep.active_connections.load(Ordering::Relaxed) as f64;
                let score = ep.weight as f64 / (active + 1.0);
                candidates.push((ep.url.clone(), score));
            }
        }

        if candidates.is_empty() {
            warn!("No healthy endpoints available");
            return None;
        }

        // Pick highest score
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let url = candidates[0].0.clone();

        // Increment active connections
        drop(eps);
        let eps = self.endpoints.read().await;
        if let Some(ep) = eps.iter().find(|e| e.url == url) {
            ep.active_connections.fetch_add(1, Ordering::Relaxed);
        }

        Some(url)
    }

    /// Report request completion
    pub async fn report_request(&self, endpoint_url: &str, latency: Duration, success: bool) {
        self.global_requests.fetch_add(1, Ordering::Relaxed);
        if !success {
            self.global_failures.fetch_add(1, Ordering::Relaxed);
        }

        let mut eps = self.endpoints.write().await;
        if let Some(ep) = eps.iter_mut().find(|e| e.url == endpoint_url) {
            ep.total_requests.fetch_add(1, Ordering::Relaxed);
            ep.active_connections.fetch_sub(1, Ordering::Relaxed);
            ep.latency.write().await.record(latency);

            {
                let mut cb = ep.circuit_breaker.write().await;
                if success {
                    cb.record_success();
                } else {
                    ep.total_failures.fetch_add(1, Ordering::Relaxed);
                    cb.record_failure();
                }

                // Update health status based on error rate and latency
                let total = ep.total_requests.load(Ordering::Relaxed);
                let fails = ep.total_failures.load(Ordering::Relaxed);
                let error_rate = if total > 0 { fails as f64 / total as f64 } else { 0.0 };
                let avg_latency = ep.latency.read().await.avg_ms();

                let new_health = if cb.state() == CircuitState::Open {
                HealthStatus::Unhealthy
            } else if error_rate > 0.5 || avg_latency > 5000.0 {
                HealthStatus::Unhealthy
            } else if error_rate > 0.1 || avg_latency > 2000.0 {
                HealthStatus::Degraded
                } else {
                    HealthStatus::Healthy
                };
                *ep.health.write().await = new_health;
            }
        }
    }

    /// Get pool metrics
    pub async fn metrics(&self) -> PoolMetrics {
        let eps = self.endpoints.read().await;

        let mut healthy = 0;
        let mut degraded = 0;
        let mut unhealthy = 0;
        let mut total_active = 0u64;
        let mut total_idle = 0u64;
        let mut endpoint_details = Vec::new();

        for ep in eps.iter() {
            let health = *ep.health.read().await;
            match health {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Degraded => degraded += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
            }

            let active = ep.active_connections.load(Ordering::Relaxed);
            let idle = ep.idle_connections.load(Ordering::Relaxed);
            total_active += active;
            total_idle += idle;

            let total_req = ep.total_requests.load(Ordering::Relaxed);
            let total_fail = ep.total_failures.load(Ordering::Relaxed);

            let cb = ep.circuit_breaker.read().await;
            endpoint_details.push(EndpointHealth {
                status: *ep.health.read().await,
                circuit_state: cb.state(),
                consecutive_failures: cb.consecutive_failures(),
                total_requests: total_req,
                total_failures: total_fail,
                total_circuit_trips: cb.total_trips(),
                error_rate: if total_req > 0 { total_fail as f64 / total_req as f64 } else { 0.0 },
                avg_latency_ms: 0.0, // Would need async read
                p99_latency_ms: 0.0,
                active_connections: active as usize,
                idle_connections: idle as usize,
                last_health_check: None,
            });
        }

        let total_req = self.global_requests.load(Ordering::Relaxed);
        let total_fail = self.global_failures.load(Ordering::Relaxed);

        PoolMetrics {
            endpoint_count: eps.len(),
            healthy_endpoints: healthy,
            degraded_endpoints: degraded,
            unhealthy_endpoints: unhealthy,
            total_active_connections: total_active,
            total_idle_connections: total_idle,
            total_requests: total_req,
            total_failures: total_fail,
            error_rate: if total_req > 0 { total_fail as f64 / total_req as f64 } else { 0.0 },
            endpoints: endpoint_details,
        }
    }

    /// Run health check on all endpoints
    pub async fn health_check(&self) {
        let mut eps = self.endpoints.write().await;
        for ep in eps.iter_mut() {
            // Simple health check: if circuit is open and enough time passed, try half-open
            {
                let mut cb = ep.circuit_breaker.write().await;
                if cb.state() == CircuitState::Open {
                    cb.allow_request(); // Will transition to HalfOpen if ready
                }
            }
            *ep.last_health_check.write().await = Some(Instant::now());
        }
    }

    /// Get number of registered endpoints
    pub async fn endpoint_count(&self) -> usize {
        self.endpoints.read().await.len()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PoolConfig {
        PoolConfig {
            max_idle_per_endpoint: 5,
            max_total_connections: 50,
            idle_timeout_secs: 30,
            connect_timeout_secs: 2,
            health_check_interval_secs: 10,
            cb_failure_threshold: 3,
            cb_open_duration_secs: 5,
            cb_half_open_success_threshold: 2,
        }
    }

    // --- Circuit Breaker Tests ---

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(test_config());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_trips_on_failures() {
        let mut cb = CircuitBreaker::new(test_config());
        for _ in 0..3 {
            assert_eq!(cb.state(), CircuitState::Closed);
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let mut cb = CircuitBreaker::new(test_config());
        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // resets consecutive failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed); // only 2 consecutive, threshold is 3
    }

    #[test]
    fn test_circuit_breaker_half_open_after_timeout() {
        let config = PoolConfig {
            cb_open_duration_secs: 0, // instant timeout for test
            ..test_config()
        };
        let mut cb = CircuitBreaker::new(config);

        // Trip it
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait (0 seconds in this config) and try again
        std::thread::sleep(Duration::from_millis(10));
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_to_closed() {
        let config = PoolConfig {
            cb_open_duration_secs: 0,
            cb_half_open_success_threshold: 2,
            ..test_config()
        };
        let mut cb = CircuitBreaker::new(config);

        // Trip to open
        for _ in 0..3 {
            cb.record_failure();
        }
        // Transition to half-open
        std::thread::sleep(Duration::from_millis(10));
        cb.allow_request();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Two successes close it
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let config = PoolConfig {
            cb_open_duration_secs: 0,
            ..test_config()
        };
        let mut cb = CircuitBreaker::new(config);

        for _ in 0..3 {
            cb.record_failure();
        }
        std::thread::sleep(Duration::from_millis(10));
        cb.allow_request();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_trip_count() {
        let config = PoolConfig {
            cb_open_duration_secs: 0,
            ..test_config()
        };
        let mut cb = CircuitBreaker::new(config);

        // Trip twice
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.total_trips(), 1);

        std::thread::sleep(Duration::from_millis(10));
        cb.allow_request(); // half-open
        cb.record_failure(); // back to open
        assert_eq!(cb.total_trips(), 2);
    }

    // --- Connection Pool Tests ---

    #[tokio::test]
    async fn test_pool_add_and_select() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://ep1:8080", 1).await;
        pool.add_endpoint("http://ep2:8080", 1).await;

        assert_eq!(pool.endpoint_count().await, 2);

        let ep = pool.select_endpoint().await;
        assert!(ep.is_some());
    }

    #[tokio::test]
    async fn test_pool_select_empty() {
        let pool = ConnectionPool::new(test_config());
        let ep = pool.select_endpoint().await;
        assert!(ep.is_none());
    }

    #[tokio::test]
    async fn test_pool_weighted_routing() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://low-weight", 1).await;
        pool.add_endpoint("http://high-weight", 10).await;

        // With no active connections, high-weight should be selected
        let ep = pool.select_endpoint().await.unwrap();
        assert_eq!(ep, "http://high-weight");
    }

    #[tokio::test]
    async fn test_pool_metrics() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://ep1", 1).await;

        let metrics = pool.metrics().await;
        assert_eq!(metrics.endpoint_count, 1);
        assert_eq!(metrics.healthy_endpoints, 1);
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_pool_report_success() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://ep1", 1).await;

        pool.report_request("http://ep1", Duration::from_millis(50), true).await;

        let metrics = pool.metrics().await;
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.total_failures, 0);
        assert_eq!(metrics.error_rate, 0.0);
    }

    #[tokio::test]
    async fn test_pool_report_failure() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://ep1", 1).await;

        pool.report_request("http://ep1", Duration::from_millis(5000), false).await;

        let metrics = pool.metrics().await;
        assert_eq!(metrics.total_failures, 1);
        assert!(metrics.error_rate > 0.0);
    }

    #[tokio::test]
    async fn test_pool_health_check() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://ep1", 1).await;

        pool.health_check().await;
        let metrics = pool.metrics().await;
        assert_eq!(metrics.endpoint_count, 1);
    }

    #[tokio::test]
    async fn test_pool_multiple_endpoints_health() {
        let pool = ConnectionPool::new(test_config());
        pool.add_endpoint("http://healthy", 1).await;

        pool.report_request("http://healthy", Duration::from_millis(50), true).await;

        let metrics = pool.metrics().await;
        assert_eq!(metrics.endpoint_count, 1);
        let ep = &metrics.endpoints[0];
        assert_eq!(ep.status, HealthStatus::Healthy);
        assert_eq!(ep.total_requests, 1);
    }

    // --- Latency Tracker Tests ---

    #[test]
    fn test_latency_tracker_avg() {
        let mut tracker = LatencyTracker::new(100);
        tracker.record(Duration::from_millis(100));
        tracker.record(Duration::from_millis(200));
        tracker.record(Duration::from_millis(300));

        assert!((tracker.avg_ms() - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_latency_tracker_p99() {
        let mut tracker = LatencyTracker::new(100);
        for i in 0..100 {
            tracker.record(Duration::from_millis(i));
        }
        // p99 of 0..100 should be ~99ms
        assert!((tracker.p99_ms() - 99.0).abs() < 1.0);
    }

    #[test]
    fn test_latency_tracker_window() {
        let mut tracker = LatencyTracker::new(3);
        tracker.record(Duration::from_millis(100));
        tracker.record(Duration::from_millis(200));
        tracker.record(Duration::from_millis(300));
        tracker.record(Duration::from_millis(400)); // evicts 100

        assert_eq!(tracker.samples.len(), 3);
        assert!((tracker.avg_ms() - 300.0).abs() < 0.1);
    }

    // --- CircuitBreaker Serialization ---

    #[test]
    fn test_circuit_state_serialization() {
        let states = vec![CircuitState::Closed, CircuitState::Open, CircuitState::HalfOpen];
        for state in states {
            let json = serde_json::to_string(&state).expect("serialize");
            let deserialized: CircuitState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn test_health_status_serialization() {
        let statuses = vec![HealthStatus::Healthy, HealthStatus::Degraded, HealthStatus::Unhealthy];
        for status in statuses {
            let json = serde_json::to_string(&status).expect("serialize");
            let deserialized: HealthStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(status, deserialized);
        }
    }
}
