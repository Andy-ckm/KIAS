use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use tokio::task::JoinHandle;

/// Duration after which an inactive entry is considered stale (5 minutes).
const STALE_THRESHOLD: Duration = Duration::from_secs(300);

/// Interval for the background cleanup task (60 seconds).
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

// ── Result types ────────────────────────────────────────────────────────────

/// Result of a rate-limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Request is allowed. Contains the number of remaining tokens.
    Allow { remaining: u32 },
    /// Request is denied. Contains the number of seconds the client should wait.
    Deny { retry_after: u64 },
}

// ── Token bucket ────────────────────────────────────────────────────────────

/// A single token bucket for one client IP.
#[derive(Debug)]
pub struct TokenBucket {
    /// Current number of available tokens.
    tokens: f64,
    /// Maximum number of tokens the bucket can hold.
    max_tokens: f64,
    /// Number of tokens added per second.
    refill_rate: f64,
    /// Last time tokens were refilled.
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time since last refill.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Try to consume one token. Returns `true` if allowed.
    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Return the number of seconds until at least one token is available.
    fn retry_after_secs(&self) -> u64 {
        if self.tokens >= 1.0 {
            0
        } else {
            let deficit = 1.0 - self.tokens;
            (deficit / self.refill_rate).ceil() as u64
        }
    }

    /// Return the current token count (after refill).
    fn remaining(&mut self) -> u32 {
        self.refill();
        self.tokens.floor() as u32
    }

    /// Return the timestamp of the last refill.
    fn last_refill(&self) -> Instant {
        self.last_refill
    }
}

// ── Rate limiter ────────────────────────────────────────────────────────────

/// Configuration for the rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Sustained requests per second per IP.
    pub requests_per_second: f64,
    /// Maximum burst size per IP.
    pub burst_size: f64,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 10.0,
            burst_size: 20.0,
        }
    }
}

/// Per-IP token-bucket rate limiter backed by a concurrent hash map.
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<IpAddr, TokenBucket>>,
    config: RateLimiterConfig,
    cleanup_handle: Arc<Option<JoinHandle<()>>>,
}

impl RateLimiter {
    /// Create a new rate limiter and spawn the background cleanup task.
    pub fn new(config: RateLimiterConfig) -> Self {
        let buckets: Arc<DashMap<IpAddr, TokenBucket>> = Arc::new(DashMap::new());
        let cleanup_buckets = buckets.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                cleanup_buckets
                    .retain(|_, bucket| bucket.last_refill().elapsed() < STALE_THRESHOLD);
            }
        });

        Self {
            buckets,
            config,
            cleanup_handle: Arc::new(Some(handle)),
        }
    }

    /// Create a rate limiter without spawning the background cleanup task
    /// (useful for testing).
    pub fn new_without_cleanup(config: RateLimiterConfig) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
            cleanup_handle: Arc::new(None),
        }
    }

    /// Check whether a request from the given IP is allowed.
    pub fn check(&self, ip: IpAddr) -> RateLimitResult {
        let mut bucket = self.buckets.entry(ip).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size, self.config.requests_per_second)
        });

        if bucket.try_consume() {
            let remaining = bucket.remaining();
            RateLimitResult::Allow { remaining }
        } else {
            let retry_after = bucket.retry_after_secs().max(1);
            RateLimitResult::Deny { retry_after }
        }
    }

    /// Return the configured burst size (used for the X-RateLimit-Limit header).
    pub fn burst_size(&self) -> u32 {
        self.config.burst_size as u32
    }

    /// Number of tracked IPs (for testing / observability).
    pub fn tracked_ips(&self) -> usize {
        self.buckets.len()
    }
}

impl Drop for RateLimiter {
    fn drop(&mut self) {
        // Abort the cleanup task if we own the last reference.
        if let Some(handle) = self.cleanup_handle.as_ref() {
            handle.abort();
        }
    }
}

// ── Axum middleware ─────────────────────────────────────────────────────────

/// Extract the client IP from the request's `ConnectInfo` or the
/// `X-Forwarded-For` header (first entry), falling back to `127.0.0.1`.
fn extract_client_ip(req: &Request) -> IpAddr {
    // Try ConnectInfo first (works when behind a real TCP listener).
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<std::net::SocketAddr>>() {
        return addr.ip();
    }

    // Fall back to X-Forwarded-For.
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(first) = val.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // Default fallback.
    IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}

static X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
static X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
static X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");

/// Axum-compatible middleware function that enforces rate limiting.
///
/// Intended to be registered via `axum::middleware::from_fn_with_state`.
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&request);
    let burst_limit = limiter.burst_size().to_string();

    match limiter.check(ip) {
        RateLimitResult::Allow { remaining } => {
            let mut response = next.run(request).await;
            let headers = response.headers_mut();
            if let Ok(val) = HeaderValue::from_str(&burst_limit) {
                headers.insert(X_RATELIMIT_LIMIT.clone(), val);
            }
            if let Ok(val) = HeaderValue::from_str(&remaining.to_string()) {
                headers.insert(X_RATELIMIT_REMAINING.clone(), val);
            }
            headers.insert(X_RATELIMIT_RESET.clone(), HeaderValue::from_static("1"));
            response
        }
        RateLimitResult::Deny { retry_after } => {
            let body = serde_json::json!({
                "error": "Too Many Requests",
                "retry_after": retry_after,
            });
            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", retry_after.to_string()),
                    ("X-RateLimit-Limit", burst_limit),
                    ("X-RateLimit-Remaining", "0".to_string()),
                    ("X-RateLimit-Reset", retry_after.to_string()),
                    ("Content-Type", "application/json".to_string()),
                ],
                body.to_string(),
            )
                .into_response()
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn ip_a() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))
    }

    fn ip_b() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))
    }

    fn ip_v6() -> IpAddr {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    }

    #[test]
    fn test_token_bucket_refill_logic() {
        let mut bucket = TokenBucket::new(5.0, 10.0);
        // After creation, tokens == max_tokens
        assert!((bucket.tokens - 5.0).abs() < f64::EPSILON);

        // Consume all tokens
        for _ in 0..5 {
            assert!(bucket.try_consume());
        }
        // Tokens should be at or near zero (may be slightly above due to
        // micro-refills between iterations).
        assert!(
            bucket.tokens <= 1.0,
            "tokens should be near zero, got {}",
            bucket.tokens
        );

        // No tokens available immediately after consuming
        // (refill in try_consume may add a tiny amount, so test via retry_after)
        if bucket.tokens < 1.0 {
            assert!(!bucket.try_consume());
        }

        // Simulate time passing by manipulating last_refill
        bucket.last_refill = Instant::now() - Duration::from_secs(1);
        bucket.refill();
        // After 1 second at 10 tokens/sec → 10 tokens, capped at max_tokens (5)
        assert!((bucket.tokens - 5.0).abs() < 1.0); // within tolerance
    }

    #[test]
    fn test_token_bucket_partial_refill() {
        let mut bucket = TokenBucket::new(10.0, 2.0);
        // Drain all tokens
        for _ in 0..10 {
            bucket.try_consume();
        }
        assert!(bucket.tokens < 0.001);

        // Simulate 0.5 seconds → 2.0 * 0.5 = 1.0 token
        bucket.last_refill = Instant::now() - Duration::from_millis(500);
        bucket.refill();
        assert!((bucket.tokens - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_token_bucket_max_cap() {
        let mut bucket = TokenBucket::new(5.0, 100.0);
        // Even after a very long time, tokens should not exceed max_tokens
        bucket.last_refill = Instant::now() - Duration::from_secs(100);
        bucket.refill();
        assert!(bucket.tokens <= 5.0 + 1e-9);
    }

    #[test]
    fn test_rate_limit_allow_within_burst() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 10.0,
            burst_size: 5.0,
        });
        let ip = ip_a();

        // First 5 requests should be allowed (burst size = 5)
        for i in 0..5 {
            let result = limiter.check(ip);
            match result {
                RateLimitResult::Allow { remaining } => {
                    assert_eq!(remaining, 4 - i as u32);
                }
                other => panic!("Expected Allow, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_rate_limit_deny_after_burst() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 1.0,
            burst_size: 3.0,
        });
        let ip = ip_a();

        // Exhaust the burst
        for _ in 0..3 {
            limiter.check(ip);
        }

        // Next request should be denied
        let result = limiter.check(ip);
        match result {
            RateLimitResult::Deny { retry_after } => {
                assert!(retry_after >= 1);
            }
            other => panic!("Expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_rate_limit_recovery_after_refill() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 10.0,
            burst_size: 2.0,
        });
        let ip = ip_a();

        // Exhaust burst
        limiter.check(ip);
        limiter.check(ip);
        assert!(matches!(limiter.check(ip), RateLimitResult::Deny { .. }));

        // Manually refill by waiting (we can't manipulate Instant easily in integration,
        // so we test via the bucket directly)
        limiter.buckets.entry(ip).and_modify(|b| {
            b.last_refill = Instant::now() - Duration::from_secs(1);
        });

        // Should be allowed again
        assert!(matches!(limiter.check(ip), RateLimitResult::Allow { .. }));
    }

    #[test]
    fn test_per_ip_isolation() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 1.0,
            burst_size: 2.0,
        });
        let a = ip_a();
        let b = ip_b();

        // Exhaust IP A's burst
        limiter.check(a);
        limiter.check(a);
        assert!(matches!(limiter.check(a), RateLimitResult::Deny { .. }));

        // IP B should still be allowed
        assert!(matches!(limiter.check(b), RateLimitResult::Allow { .. }));
        assert!(matches!(limiter.check(b), RateLimitResult::Allow { .. }));
        // Now IP B is also exhausted
        assert!(matches!(limiter.check(b), RateLimitResult::Deny { .. }));
    }

    #[test]
    fn test_per_ip_isolation_ipv6() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 1.0,
            burst_size: 1.0,
        });
        let v4 = ip_a();
        let v6 = ip_v6();

        limiter.check(v4);
        assert!(matches!(limiter.check(v4), RateLimitResult::Deny { .. }));

        // IPv6 should be independent
        assert!(matches!(limiter.check(v6), RateLimitResult::Allow { .. }));
    }

    #[test]
    fn test_burst_size_as_limit_header() {
        let config = RateLimiterConfig {
            requests_per_second: 5.0,
            burst_size: 15.0,
        };
        let limiter = RateLimiter::new_without_cleanup(config);
        assert_eq!(limiter.burst_size(), 15);
    }

    #[test]
    fn test_tracked_ips_count() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig::default());
        assert_eq!(limiter.tracked_ips(), 0);

        limiter.check(ip_a());
        assert_eq!(limiter.tracked_ips(), 1);

        limiter.check(ip_b());
        assert_eq!(limiter.tracked_ips(), 2);

        // Same IP again → still 2
        limiter.check(ip_a());
        assert_eq!(limiter.tracked_ips(), 2);
    }

    #[test]
    fn test_retry_after_value_when_denied() {
        let limiter = RateLimiter::new_without_cleanup(RateLimiterConfig {
            requests_per_second: 1.0,
            burst_size: 1.0,
        });
        let ip = ip_a();

        limiter.check(ip);
        let result = limiter.check(ip);
        match result {
            RateLimitResult::Deny { retry_after } => {
                // With 1 token/sec refill and deficit of 1 token → retry_after ≈ 1
                assert!((1..=2).contains(&retry_after));
            }
            other => panic!("Expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_default_config() {
        let config = RateLimiterConfig::default();
        assert_eq!(config.requests_per_second, 10.0);
        assert_eq!(config.burst_size, 20.0);
    }
}
