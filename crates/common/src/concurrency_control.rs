//! Adaptive Concurrency Control
//!
//! Provides three complementary concurrency control mechanisms:
//! - **AIMDController**: Additive Increase / Multiplicative Decrease for dynamic concurrency
//! - **TokenBucket**: Classic token-bucket rate limiting
//! - **GradientLimiter**: Gradient-based rate limiting (adjusts based on latency changes)

use crate::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use tokio::sync::RwLock;

// ── AIMD Controller ─────────────────────────────────────────────────────────

/// AIMD Controller: Additive Increase / Multiplicative Decrease
#[derive(Debug, Clone)]
pub struct AIMDController {
    concurrency: f64,
    min_concurrency: f64,
    max_concurrency: f64,
    additive_increase: f64,
    multiplicative_decrease: f64,
    consecutive_successes: u64,
    consecutive_failures: u64,
    success_threshold: u64,
}

impl Default for AIMDController {
    fn default() -> Self { Self::new(10.0, 1000.0, 0.75) }
}

impl AIMDController {
    pub fn new(min_concurrency: f64, max_concurrency: f64, multiplicative_decrease: f64) -> Self {
        Self { concurrency: min_concurrency, min_concurrency, max_concurrency, additive_increase: 1.0, multiplicative_decrease, consecutive_successes: 0, consecutive_failures: 0, success_threshold: 1 }
    }
    pub fn record_success(&mut self) {
        self.consecutive_successes += 1; self.consecutive_failures = 0;
        if self.consecutive_successes >= self.success_threshold {
            self.concurrency = (self.concurrency + self.additive_increase).min(self.max_concurrency);
            self.consecutive_successes = 0;
        }
    }
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1; self.consecutive_successes = 0;
        self.concurrency = (self.concurrency * self.multiplicative_decrease).max(self.min_concurrency);
    }
    pub fn try_acquire(&self) -> bool { self.concurrency >= 1.0 }
    pub fn current_concurrency(&self) -> f64 { self.concurrency }
    pub fn concurrency_as_u64(&self) -> u64 { self.concurrency.floor() as u64 }
    pub fn reset(&mut self) { self.concurrency = self.min_concurrency; self.consecutive_successes = 0; self.consecutive_failures = 0; }
}

// ── Token Bucket ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TokenBucket { max_capacity: f64, tokens: f64, refill_rate: f64, last_refill: f64, blocking: bool }

impl Default for TokenBucket { fn default() -> Self { Self::new(100.0, 10.0) } }

impl TokenBucket {
    pub fn new(max_capacity: f64, refill_rate: f64) -> Self {
        Self { max_capacity, tokens: max_capacity, refill_rate, last_refill: std::time::Instant::now().elapsed().as_secs_f64(), blocking: false }
    }
    pub fn with_blocking(mut self) -> Self { self.blocking = true; self }
    pub fn try_acquire(&mut self, tokens_needed: f64) -> bool {
        self.refill();
        if self.tokens >= tokens_needed { self.tokens -= tokens_needed; true } else { false }
    }
    pub async fn acquire(&mut self, tokens_needed: f64) -> KiasResult<()> {
        if self.try_acquire(tokens_needed) { return Ok(()); }
        if !self.blocking { return Err(KiasError::InsufficientResources("Token bucket empty".to_string())); }
        let needed = tokens_needed - self.tokens;
        let wait_secs = needed / self.refill_rate;
        tokio::time::sleep(std::time::Duration::from_secs_f64(wait_secs)).await;
        self.refill(); self.tokens = (self.tokens - tokens_needed).max(0.0);
        Ok(())
    }
    fn refill(&mut self) {
        let now = std::time::Instant::now().elapsed().as_secs_f64();
        let elapsed = now - self.last_refill;
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_capacity);
        self.last_refill = now;
    }
    pub fn available_tokens(&self) -> f64 { self.tokens }
    pub fn reset(&mut self) { self.tokens = self.max_capacity; self.last_refill = std::time::Instant::now().elapsed().as_secs_f64(); }
    pub fn set_refill_rate(&mut self, rate: f64) { self.refill_rate = rate; }
    pub fn set_capacity(&mut self, capacity: f64) { self.max_capacity = capacity; self.tokens = self.tokens.min(capacity); }
}

// ── Gradient Limiter ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GradientLimiter {
    rate: f64, min_rate: f64, max_rate: f64, smoothing: f64, gradient_threshold: f64,
    adjustment_factor: f64, last_latency_us: Option<f64>, ema_latency: Option<f64>, ema_gradient: f64, sample_count: u64,
}

impl Default for GradientLimiter { fn default() -> Self { Self::new(1.0, 100.0, 10.0, 0.3) } }

impl GradientLimiter {
    pub fn new(min_rate: f64, max_rate: f64, gradient_threshold: f64, adjustment_factor: f64) -> Self {
        Self { rate: (min_rate + max_rate) / 2.0, min_rate, max_rate, smoothing: 0.3, gradient_threshold, adjustment_factor, last_latency_us: None, ema_latency: None, ema_gradient: 0.0, sample_count: 0 }
    }
    pub fn record_latency(&mut self, latency_us: f64) {
        self.sample_count += 1;
        if let Some(last) = self.last_latency_us {
            let raw_gradient = latency_us - last;
            self.ema_gradient = self.smoothing * raw_gradient + (1.0 - self.smoothing) * self.ema_gradient;
            self.ema_latency = Some(self.smoothing * latency_us + (1.0 - self.smoothing) * self.ema_latency.unwrap_or(latency_us));
            self.last_latency_us = Some(latency_us);
            if self.ema_gradient.abs() > self.gradient_threshold {
                if self.ema_gradient > 0.0 { self.rate = (self.rate * (1.0 - self.adjustment_factor)).max(self.min_rate); }
                else { self.rate = (self.rate * (1.0 + self.adjustment_factor)).min(self.max_rate); }
            }
        } else { self.last_latency_us = Some(latency_us); self.ema_latency = Some(latency_us); }
    }
    pub fn is_allowed(&self) -> bool { self.rate > self.min_rate }
    pub fn current_rate(&self) -> f64 { self.rate }
    pub fn current_gradient(&self) -> f64 { self.ema_gradient }
    pub fn ema_latency(&self) -> Option<f64> { self.ema_latency }
    pub fn reset(&mut self) { self.last_latency_us = None; self.ema_latency = None; self.ema_gradient = 0.0; self.sample_count = 0; self.rate = (self.min_rate + self.max_rate) / 2.0; }
    pub fn set_rate(&mut self, rate: f64) { self.rate = rate.clamp(self.min_rate, self.max_rate); }
}

// ── Thread-safe wrappers ─────────────────────────────────────────────────────

pub struct ThreadSafeAIMD { inner: Arc<RwLock<AIMDController>> }
impl Default for ThreadSafeAIMD { fn default() -> Self { Self::new() } }
impl ThreadSafeAIMD {
    pub fn new() -> Self { Self { inner: Arc::new(RwLock::new(AIMDController::default())) } }
    pub async fn record_success(&self) { self.inner.write().await.record_success(); }
    pub async fn record_failure(&self) { self.inner.write().await.record_failure(); }
    pub async fn try_acquire(&self) -> bool { self.inner.read().await.try_acquire() }
    pub async fn current_concurrency(&self) -> f64 { self.inner.read().await.current_concurrency() }
    pub async fn reset(&self) { self.inner.write().await.reset(); }
}

pub struct ThreadSafeTokenBucket { inner: Arc<RwLock<TokenBucket>> }
impl Default for ThreadSafeTokenBucket { fn default() -> Self { Self::new() } }
impl ThreadSafeTokenBucket {
    pub fn new() -> Self { Self { inner: Arc::new(RwLock::new(TokenBucket::default())) } }
    pub async fn try_acquire(&self, tokens: f64) -> bool { self.inner.write().await.try_acquire(tokens) }
    pub async fn acquire(&self, tokens: f64) -> KiasResult<()> { self.inner.write().await.acquire(tokens).await }
    pub async fn available_tokens(&self) -> f64 { self.inner.read().await.available_tokens() }
    pub async fn reset(&self) { self.inner.write().await.reset(); }
}

pub struct ThreadSafeGradientLimiter { inner: Arc<RwLock<GradientLimiter>> }
impl Default for ThreadSafeGradientLimiter { fn default() -> Self { Self::new() } }
impl ThreadSafeGradientLimiter {
    pub fn new() -> Self { Self { inner: Arc::new(RwLock::new(GradientLimiter::default())) } }
    pub async fn record_latency(&self, latency_us: f64) { self.inner.write().await.record_latency(latency_us); }
    pub async fn is_allowed(&self) -> bool { self.inner.read().await.is_allowed() }
    pub async fn current_rate(&self) -> f64 { self.inner.read().await.current_rate() }
    pub async fn reset(&self) { self.inner.write().await.reset(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aimd_default_state() { let aimd = AIMDController::default(); assert_eq!(aimd.current_concurrency(), 10.0); assert!(aimd.try_acquire()); }

    #[test]
    fn test_aimd_success_increases() {
        let mut aimd = AIMDController::new(1.0, 100.0, 0.75);
        aimd.record_success(); aimd.record_success();
        assert!(aimd.current_concurrency() >= 11.0);
    }

    #[test]
    fn test_aimd_failure_decreases() {
        let mut aimd = AIMDController::new(1.0, 100.0, 0.75);
        let initial = aimd.current_concurrency();
        aimd.record_failure();
        assert!(aimd.current_concurrency() < initial);
        assert!(aimd.current_concurrency() >= 1.0);
    }

    #[test]
    fn test_aimd_respects_max() {
        let mut aimd = AIMDController::new(1.0, 50.0, 0.75);
        for _ in 0..100 { aimd.record_success(); }
        assert!(aimd.current_concurrency() <= 50.0);
    }

    #[test]
    fn test_aimd_respects_min() {
        let mut aimd = AIMDController::new(5.0, 100.0, 0.75);
        for _ in 0..20 { aimd.record_failure(); }
        assert!(aimd.current_concurrency() >= 5.0);
    }

    #[test]
    fn test_aimd_reset() {
        let mut aimd = AIMDController::new(5.0, 100.0, 0.75);
        aimd.record_success(); aimd.record_failure();
        aimd.reset();
        assert_eq!(aimd.current_concurrency(), 5.0);
    }

    #[tokio::test]
    async fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(10.0, 1.0);
        assert!(bucket.try_acquire(5.0));
        assert!(bucket.try_acquire(5.0));
        assert!(!bucket.try_acquire(1.0));
    }

    #[tokio::test]
    async fn test_token_bucket_acquire_blocks() {
        let mut bucket = TokenBucket::new(5.0, 10.0).with_blocking();
        bucket.try_acquire(5.0);
        let start = std::time::Instant::now();
        bucket.acquire(5.0).await.unwrap();
        assert!(start.elapsed().as_millis() >= 400);
    }

    #[test]
    fn test_token_bucket_reset() {
        let mut bucket = TokenBucket::new(10.0, 5.0);
        bucket.try_acquire(7.0);
        bucket.reset();
        assert_eq!(bucket.available_tokens(), 10.0);
    }

    #[test]
    fn test_gradient_limiter_initial() {
        let limiter = GradientLimiter::default();
        assert!(limiter.current_rate() > 0.0);
    }

    #[test]
    fn test_gradient_limiter_records_latency() {
        let mut limiter = GradientLimiter::new(1.0, 100.0, 10.0, 0.1);
        limiter.record_latency(1000.0);
        limiter.record_latency(1000.0);
        assert!(limiter.ema_latency().is_some());
    }

    #[test]
    fn test_gradient_limiter_decreases_rate_on_increasing_latency() {
        let mut limiter = GradientLimiter::new(1.0, 100.0, 1.0, 0.1);
        limiter.set_rate(50.0);
        limiter.record_latency(1000.0);
        limiter.record_latency(2000.0);
        limiter.record_latency(3000.0);
        limiter.record_latency(4000.0);
        assert!(limiter.current_rate() <= 50.0);
    }

    #[test]
    fn test_gradient_limiter_resets() {
        let mut limiter = GradientLimiter::new(5.0, 100.0, 10.0, 0.2);
        limiter.record_latency(5000.0);
        limiter.reset();
        assert!(limiter.ema_latency().is_none());
    }

    #[tokio::test]
    async fn test_thread_safe_aimd() {
        let aimd = ThreadSafeAIMD::new();
        aimd.record_success().await;
        aimd.record_failure().await;
        assert!(aimd.current_concurrency().await > 0.0);
    }

    #[tokio::test]
    async fn test_thread_safe_token_bucket() {
        let bucket = ThreadSafeTokenBucket::new();
        assert!(bucket.try_acquire(1.0).await);
    }

    #[tokio::test]
    async fn test_thread_safe_gradient_limiter() {
        let limiter = ThreadSafeGradientLimiter::new();
        limiter.record_latency(1000.0).await;
        assert!(limiter.current_rate().await > 0.0);
    }
}
