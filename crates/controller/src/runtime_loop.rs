//! # Runtime Loop — Execute → Observe → Adjust → Re-Execute
//!
//! Controller-level orchestration loop that runs iterative execute→observe→adjust
//! cycles until a goal is achieved or limits are reached.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │   Execute    │───▶│   Observe   │───▶│   Adjust    │
//! │  (Executor)  │    │ (Evaluator) │    │  (Feedback) │
//! └─────────────┘    └─────────────┘    └─────────────┘
//!       ▲                                        │
//!       └────────────────────────────────────────┘
//!                    until goal achieved or max rounds
//! ```

use chrono::Utc;
use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

// ──────────────────────────────────────────────────────────────────────────────
// Traits
// ──────────────────────────────────────────────────────────────────────────────

/// Executor — performs one round of work, optionally using feedback from the previous round.
#[async_trait::async_trait]
pub trait RuntimeExecutor: Send + Sync {
    /// Execute one round. `feedback` contains adjustment guidance from the previous round.
    async fn execute_round(
        &self,
        goal: &str,
        round: u32,
        feedback: Option<&str>,
    ) -> KiasResult<String>;
}

/// Evaluator — scores the output of a round (0.0 – 1.0).
#[async_trait::async_trait]
pub trait RuntimeEvaluator: Send + Sync {
    /// Evaluate the quality of `output` for the given `goal`. Returns a score in [0.0, 1.0].
    async fn evaluate(&self, goal: &str, output: &str, round: u32) -> KiasResult<f64>;
}

/// Observer — receives lifecycle events for observability / logging.
#[async_trait::async_trait]
pub trait RuntimeLoopObserver: Send + Sync {
    async fn on_round_start(&self, round: u32, goal: &str);
    async fn on_round_complete(&self, round: u32, output: &str, quality: f64);
    async fn on_status_change(&self, goal: &str, old: &RuntimeLoopStatus, new: &RuntimeLoopStatus);
    async fn on_feedback(&self, round: u32, feedback: &str);
}

// ──────────────────────────────────────────────────────────────────────────────
// Default Implementations
// ──────────────────────────────────────────────────────────────────────────────

/// Simple executor that echoes back the goal with round number (for testing / demo).
pub struct SimpleRuntimeExecutor;

#[async_trait::async_trait]
impl RuntimeExecutor for SimpleRuntimeExecutor {
    async fn execute_round(
        &self,
        goal: &str,
        round: u32,
        _feedback: Option<&str>,
    ) -> KiasResult<String> {
        Ok(format!("Round {} output for: {}", round, goal))
    }
}

/// Simple evaluator that returns 0.5 for all outputs (baseline / testing).
pub struct SimpleRuntimeEvaluator;

#[async_trait::async_trait]
impl RuntimeEvaluator for SimpleRuntimeEvaluator {
    async fn evaluate(&self, _goal: &str, _output: &str, _round: u32) -> KiasResult<f64> {
        Ok(0.5)
    }
}

/// No-op observer — silently ignores all events.
pub struct NoOpObserver;

#[async_trait::async_trait]
impl RuntimeLoopObserver for NoOpObserver {
    async fn on_round_start(&self, _round: u32, _goal: &str) {}
    async fn on_round_complete(&self, _round: u32, _output: &str, _quality: f64) {}
    async fn on_status_change(
        &self,
        _goal: &str,
        _old: &RuntimeLoopStatus,
        _new: &RuntimeLoopStatus,
    ) {
    }
    async fn on_feedback(&self, _round: u32, _feedback: &str) {}
}

/// Observer that logs events via `tracing`.
pub struct TracingObserver;

#[async_trait::async_trait]
impl RuntimeLoopObserver for TracingObserver {
    async fn on_round_start(&self, round: u32, goal: &str) {
        tracing::info!(round, goal, "Runtime loop round starting");
    }
    async fn on_round_complete(&self, round: u32, output: &str, quality: f64) {
        tracing::info!(round, quality, output_len = output.len(), "Round complete");
    }
    async fn on_status_change(
        &self,
        goal: &str,
        _old: &RuntimeLoopStatus,
        new: &RuntimeLoopStatus,
    ) {
        tracing::info!(goal, ?new, "Loop status changed");
    }
    async fn on_feedback(&self, round: u32, feedback: &str) {
        tracing::debug!(round, feedback, "Feedback generated");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────────────────────────────────────

/// Configuration for a runtime loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLoopConfig {
    /// Maximum number of rounds before forced stop.
    pub max_rounds: u32,
    /// Timeout for the entire loop execution.
    pub loop_timeout: Duration,
    /// Timeout per individual round.
    pub round_timeout: Duration,
    /// Minimum acceptable quality score (0.0 – 1.0) to consider the goal achieved.
    pub quality_threshold: f64,
    /// Whether to stop as soon as the quality threshold is met.
    pub stop_on_achieve: bool,
    /// Cooldown between rounds (for rate limiting).
    pub cooldown: Duration,
}

impl Default for RuntimeLoopConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            loop_timeout: Duration::from_secs(300),
            round_timeout: Duration::from_secs(60),
            quality_threshold: 0.8,
            stop_on_achieve: true,
            cooldown: Duration::from_millis(100),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Status & Metrics
// ──────────────────────────────────────────────────────────────────────────────

/// Status of a runtime loop execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeLoopStatus {
    Ready,
    Running,
    Achieved,
    MaxRoundsReached,
    TimedOut,
    Cancelled,
    Failed(String),
}

/// Metrics collected during loop execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLoopMetrics {
    pub rounds_executed: u32,
    pub achieved_on_round: Option<u32>,
    pub total_duration: Duration,
    pub round_durations: Vec<Duration>,
    pub quality_scores: Vec<f64>,
    pub feedback_chain: Vec<String>,
    pub status: RuntimeLoopStatus,
}

impl RuntimeLoopMetrics {
    fn new() -> Self {
        Self {
            rounds_executed: 0,
            achieved_on_round: None,
            total_duration: Duration::ZERO,
            round_durations: Vec::new(),
            quality_scores: Vec::new(),
            feedback_chain: Vec::new(),
            status: RuntimeLoopStatus::Ready,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RuntimeLoop
// ──────────────────────────────────────────────────────────────────────────────

/// The runtime loop — orchestrates execute→observe→adjust→re-execute.
pub struct RuntimeLoop {
    config: RuntimeLoopConfig,
    executor: Arc<dyn RuntimeExecutor>,
    evaluator: Arc<dyn RuntimeEvaluator>,
    observer: Arc<dyn RuntimeLoopObserver>,
}

impl RuntimeLoop {
    /// Create with all components specified.
    pub fn new(
        config: RuntimeLoopConfig,
        executor: Arc<dyn RuntimeExecutor>,
        evaluator: Arc<dyn RuntimeEvaluator>,
        observer: Arc<dyn RuntimeLoopObserver>,
    ) -> Self {
        Self {
            config,
            executor,
            evaluator,
            observer,
        }
    }

    /// Create with defaults for testing.
    pub fn with_defaults() -> Self {
        Self {
            config: RuntimeLoopConfig::default(),
            executor: Arc::new(SimpleRuntimeExecutor),
            evaluator: Arc::new(SimpleRuntimeEvaluator),
            observer: Arc::new(TracingObserver),
        }
    }

    /// Create with custom config but default components.
    pub fn with_config(config: RuntimeLoopConfig) -> Self {
        Self {
            config,
            executor: Arc::new(SimpleRuntimeExecutor),
            evaluator: Arc::new(SimpleRuntimeEvaluator),
            observer: Arc::new(TracingObserver),
        }
    }

    /// Run the loop for a goal description.
    pub async fn run(&self, goal: &str) -> KiasResult<RuntimeLoopMetrics> {
        let start = Utc::now();
        let mut metrics = RuntimeLoopMetrics::new();
        let mut previous_feedback: Option<String> = None;

        metrics.status = RuntimeLoopStatus::Running;
        self.observer
            .on_status_change(goal, &RuntimeLoopStatus::Ready, &RuntimeLoopStatus::Running)
            .await;

        for round in 1..=self.config.max_rounds {
            // Check total timeout
            let elapsed = Utc::now().signed_duration_since(start);
            if elapsed.num_seconds() as u64 >= self.config.loop_timeout.as_secs() {
                metrics.status = RuntimeLoopStatus::TimedOut;
                break;
            }

            let round_start = Utc::now();
            self.observer.on_round_start(round, goal).await;

            // ── Execute ──
            let output = tokio::time::timeout(
                self.config.round_timeout,
                self.executor
                    .execute_round(goal, round, previous_feedback.as_deref()),
            )
            .await
            .map_err(|_| {
                KiasError::Internal(anyhow::anyhow!(
                    "Round {} timed out after {:?}",
                    round,
                    self.config.round_timeout
                ))
            })?
            .map_err(|e| {
                KiasError::Internal(anyhow::anyhow!("Round {} execution failed: {}", round, e))
            })?;

            let round_duration = Utc::now()
                .signed_duration_since(round_start)
                .to_std()
                .unwrap_or(Duration::ZERO);
            metrics.round_durations.push(round_duration);
            metrics.rounds_executed = round;

            // ── Observe (evaluate quality) ──
            let quality = self.evaluator.evaluate(goal, &output, round).await?;
            metrics.quality_scores.push(quality);
            self.observer
                .on_round_complete(round, &output, quality)
                .await;

            // ── Check achievement ──
            if self.config.stop_on_achieve && quality >= self.config.quality_threshold {
                metrics.achieved_on_round = Some(round);
                metrics.status = RuntimeLoopStatus::Achieved;
                metrics.feedback_chain.push(format!(
                    "Round {}: achieved (quality={:.2})",
                    round, quality
                ));
                break;
            }

            // ── Adjust (generate feedback) ──
            let feedback = self.generate_feedback(goal, &output, quality, round);
            metrics.feedback_chain.push(feedback.clone());
            previous_feedback = Some(feedback.clone());
            self.observer.on_feedback(round, &feedback).await;

            // Cooldown
            if round < self.config.max_rounds && self.config.cooldown > Duration::ZERO {
                tokio::time::sleep(self.config.cooldown).await;
            }
        }

        // Finalize
        if metrics.status == RuntimeLoopStatus::Running {
            metrics.status = RuntimeLoopStatus::MaxRoundsReached;
        }

        metrics.total_duration = Utc::now()
            .signed_duration_since(start)
            .to_std()
            .unwrap_or(Duration::ZERO);

        self.observer
            .on_status_change(goal, &RuntimeLoopStatus::Running, &metrics.status)
            .await;

        Ok(metrics)
    }

    /// Generate feedback for the next round.
    fn generate_feedback(&self, _goal: &str, output: &str, quality: f64, round: u32) -> String {
        let preview = &output[..output.len().min(200)];
        if quality >= self.config.quality_threshold {
            format!(
                "Round {} quality {:.2} meets threshold {:.2}. Output: {}",
                round, quality, self.config.quality_threshold, preview,
            )
        } else {
            format!(
                "Round {} quality {:.2} below threshold {:.2}. Improve: {}",
                round, quality, self.config.quality_threshold, preview,
            )
        }
    }

    /// Get the current config (for inspection in tests).
    pub fn config(&self) -> &RuntimeLoopConfig {
        &self.config
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Builder
// ──────────────────────────────────────────────────────────────────────────────

/// Fluent builder for `RuntimeLoop`.
pub struct RuntimeLoopBuilder {
    config: RuntimeLoopConfig,
    executor: Option<Arc<dyn RuntimeExecutor>>,
    evaluator: Option<Arc<dyn RuntimeEvaluator>>,
    observer: Option<Arc<dyn RuntimeLoopObserver>>,
}

impl RuntimeLoopBuilder {
    pub fn new() -> Self {
        Self {
            config: RuntimeLoopConfig::default(),
            executor: None,
            evaluator: None,
            observer: None,
        }
    }

    pub fn max_rounds(mut self, n: u32) -> Self {
        self.config.max_rounds = n;
        self
    }

    pub fn loop_timeout(mut self, d: Duration) -> Self {
        self.config.loop_timeout = d;
        self
    }

    pub fn round_timeout(mut self, d: Duration) -> Self {
        self.config.round_timeout = d;
        self
    }

    pub fn quality_threshold(mut self, t: f64) -> Self {
        self.config.quality_threshold = t;
        self
    }

    pub fn stop_on_achieve(mut self, b: bool) -> Self {
        self.config.stop_on_achieve = b;
        self
    }

    pub fn cooldown(mut self, d: Duration) -> Self {
        self.config.cooldown = d;
        self
    }

    pub fn executor(mut self, e: Arc<dyn RuntimeExecutor>) -> Self {
        self.executor = Some(e);
        self
    }

    pub fn evaluator(mut self, e: Arc<dyn RuntimeEvaluator>) -> Self {
        self.evaluator = Some(e);
        self
    }

    pub fn observer(mut self, o: Arc<dyn RuntimeLoopObserver>) -> Self {
        self.observer = Some(o);
        self
    }

    pub fn build(self) -> RuntimeLoop {
        RuntimeLoop {
            config: self.config,
            executor: self
                .executor
                .unwrap_or_else(|| Arc::new(SimpleRuntimeExecutor)),
            evaluator: self
                .evaluator
                .unwrap_or_else(|| Arc::new(SimpleRuntimeEvaluator)),
            observer: self.observer.unwrap_or_else(|| Arc::new(NoOpObserver)),
        }
    }
}

impl Default for RuntimeLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── Test executors ──────────────────────────────────────────────────────

    /// Executor that returns a fixed string.
    struct FixedExecutor(String);
    impl FixedExecutor {
        fn new(s: &str) -> Self {
            Self(s.to_string())
        }
    }
    #[async_trait::async_trait]
    impl RuntimeExecutor for FixedExecutor {
        async fn execute_round(
            &self,
            _goal: &str,
            round: u32,
            _fb: Option<&str>,
        ) -> KiasResult<String> {
            Ok(format!("{} (round {})", self.0, round))
        }
    }

    /// Executor that returns improving output each round.
    struct ImprovingExecutor(AtomicU32);
    impl ImprovingExecutor {
        fn new() -> Self {
            Self(AtomicU32::new(0))
        }
    }
    #[async_trait::async_trait]
    impl RuntimeExecutor for ImprovingExecutor {
        async fn execute_round(
            &self,
            _goal: &str,
            _round: u32,
            _fb: Option<&str>,
        ) -> KiasResult<String> {
            let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(format!("improving v{}", n))
        }
    }

    /// Executor that always fails.
    struct FailingExecutor;
    #[async_trait::async_trait]
    impl RuntimeExecutor for FailingExecutor {
        async fn execute_round(
            &self,
            _goal: &str,
            _round: u32,
            _fb: Option<&str>,
        ) -> KiasResult<String> {
            Err(KiasError::Internal(anyhow::anyhow!("boom")))
        }
    }

    /// Executor that checks feedback is passed through.
    struct FeedbackCheckingExecutor {
        saw_feedback: Arc<tokio::sync::Mutex<bool>>,
    }
    #[async_trait::async_trait]
    impl RuntimeExecutor for FeedbackCheckingExecutor {
        async fn execute_round(
            &self,
            _goal: &str,
            _round: u32,
            feedback: Option<&str>,
        ) -> KiasResult<String> {
            if feedback.is_some() {
                *self.saw_feedback.lock().await = true;
            }
            Ok("output".to_string())
        }
    }

    // ── Test evaluators ─────────────────────────────────────────────────────

    /// Evaluator that returns a fixed score.
    struct FixedEvaluator(f64);
    #[async_trait::async_trait]
    impl RuntimeEvaluator for FixedEvaluator {
        async fn evaluate(&self, _goal: &str, _output: &str, _round: u32) -> KiasResult<f64> {
            Ok(self.0)
        }
    }

    /// Evaluator that increases score each round.
    struct ImprovingEvaluator(AtomicU32);
    impl ImprovingEvaluator {
        fn new() -> Self {
            Self(AtomicU32::new(0))
        }
    }
    #[async_trait::async_trait]
    impl RuntimeEvaluator for ImprovingEvaluator {
        async fn evaluate(&self, _goal: &str, _output: &str, _round: u32) -> KiasResult<f64> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            Ok(0.3 + (n as f64 * 0.25).min(0.7)) // 0.3, 0.55, 0.8, 0.95...
        }
    }

    // ── Helper ──────────────────────────────────────────────────────────────

    #[allow(dead_code)]
    fn fast_config() -> RuntimeLoopConfig {
        RuntimeLoopConfig {
            max_rounds: 5,
            loop_timeout: Duration::from_secs(10),
            round_timeout: Duration::from_secs(5),
            quality_threshold: 0.8,
            stop_on_achieve: true,
            cooldown: Duration::ZERO,
        }
    }

    // ── Tests ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_basic_execution() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(3)
            .executor(Arc::new(FixedExecutor::new("out")))
            .evaluator(Arc::new(FixedEvaluator(0.5)))
            .build();

        let m = lp.run("test goal").await.unwrap();
        assert_eq!(m.rounds_executed, 3);
        assert_eq!(m.status, RuntimeLoopStatus::MaxRoundsReached);
        assert!(m.achieved_on_round.is_none());
        assert_eq!(m.quality_scores.len(), 3);
        assert_eq!(m.feedback_chain.len(), 3);
    }

    #[tokio::test]
    async fn test_achieves_when_quality_high() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(5)
            .quality_threshold(0.8)
            .executor(Arc::new(FixedExecutor::new("good")))
            .evaluator(Arc::new(FixedEvaluator(0.9)))
            .build();

        let m = lp.run("goal").await.unwrap();
        assert_eq!(m.rounds_executed, 1);
        assert_eq!(m.status, RuntimeLoopStatus::Achieved);
        assert_eq!(m.achieved_on_round, Some(1));
    }

    #[tokio::test]
    async fn test_achieves_after_several_rounds() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(10)
            .quality_threshold(0.8)
            .executor(Arc::new(ImprovingExecutor::new()))
            .evaluator(Arc::new(ImprovingEvaluator::new()))
            .build();

        let m = lp.run("goal").await.unwrap();
        // Round 1: quality=0.3, Round 2: 0.55, Round 3: 0.8 → achieved
        assert_eq!(m.status, RuntimeLoopStatus::Achieved);
        assert_eq!(m.achieved_on_round, Some(3));
        assert_eq!(m.rounds_executed, 3);
    }

    #[tokio::test]
    async fn test_max_rounds_stops_loop() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(3)
            .quality_threshold(0.99) // unreachable
            .executor(Arc::new(FixedExecutor::new("out")))
            .evaluator(Arc::new(FixedEvaluator(0.5)))
            .build();

        let m = lp.run("goal").await.unwrap();
        assert_eq!(m.rounds_executed, 3);
        assert_eq!(m.status, RuntimeLoopStatus::MaxRoundsReached);
    }

    #[tokio::test]
    async fn test_failed_executor_returns_error() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(3)
            .executor(Arc::new(FailingExecutor))
            .build();

        let result = lp.run("goal").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_feedback_passed_to_next_round() {
        let saw = Arc::new(tokio::sync::Mutex::new(false));
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(2)
            .quality_threshold(0.99)
            .executor(Arc::new(FeedbackCheckingExecutor {
                saw_feedback: saw.clone(),
            }))
            .evaluator(Arc::new(FixedEvaluator(0.5)))
            .build();

        let _ = lp.run("goal").await.unwrap();
        assert!(
            *saw.lock().await,
            "Second round should have received feedback"
        );
    }

    #[tokio::test]
    async fn test_cooldown_adds_delay() {
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(3)
            .cooldown(Duration::from_millis(50))
            .executor(Arc::new(FixedExecutor::new("out")))
            .evaluator(Arc::new(FixedEvaluator(0.5)))
            .build();

        let start = std::time::Instant::now();
        let _ = lp.run("goal").await.unwrap();
        // 3 rounds, 2 cooldowns of 50ms each = ~100ms minimum
        assert!(start.elapsed() >= Duration::from_millis(80));
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = RuntimeLoopConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RuntimeLoopConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_rounds, 10);
        assert!((deserialized.quality_threshold - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_status_serialization() {
        for status in &[
            RuntimeLoopStatus::Ready,
            RuntimeLoopStatus::Running,
            RuntimeLoopStatus::Achieved,
            RuntimeLoopStatus::MaxRoundsReached,
            RuntimeLoopStatus::TimedOut,
            RuntimeLoopStatus::Cancelled,
            RuntimeLoopStatus::Failed("test".to_string()),
        ] {
            let json = serde_json::to_string(status).unwrap();
            let deserialized: RuntimeLoopStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&deserialized, status);
        }
    }

    #[tokio::test]
    async fn test_metrics_serialization() {
        let m = RuntimeLoopMetrics {
            rounds_executed: 3,
            achieved_on_round: Some(2),
            total_duration: Duration::from_secs(5),
            round_durations: vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(2),
            ],
            quality_scores: vec![0.3, 0.7, 0.9],
            feedback_chain: vec!["f1".into(), "f2".into()],
            status: RuntimeLoopStatus::Achieved,
        };
        let json = serde_json::to_string(&m).unwrap();
        let d: RuntimeLoopMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(d.rounds_executed, 3);
        assert_eq!(d.achieved_on_round, Some(2));
    }

    #[tokio::test]
    async fn test_builder_defaults() {
        let lp = RuntimeLoopBuilder::new().build();
        assert_eq!(lp.config().max_rounds, 10);
        assert!((lp.config().quality_threshold - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_with_defaults_factory() {
        let lp = RuntimeLoop::with_defaults();
        assert_eq!(lp.config().max_rounds, 10);
    }

    #[tokio::test]
    async fn test_with_config_factory() {
        let config = RuntimeLoopConfig {
            max_rounds: 7,
            ..Default::default()
        };
        let lp = RuntimeLoop::with_config(config);
        assert_eq!(lp.config().max_rounds, 7);
    }

    #[tokio::test]
    async fn test_custom_observer() {
        struct RecordingObserver {
            events: Arc<tokio::sync::Mutex<Vec<String>>>,
        }
        #[async_trait::async_trait]
        impl RuntimeLoopObserver for RecordingObserver {
            async fn on_round_start(&self, round: u32, _goal: &str) {
                self.events.lock().await.push(format!("start:{}", round));
            }
            async fn on_round_complete(&self, round: u32, _: &str, _: f64) {
                self.events.lock().await.push(format!("complete:{}", round));
            }
            async fn on_status_change(
                &self,
                _: &str,
                _: &RuntimeLoopStatus,
                new: &RuntimeLoopStatus,
            ) {
                self.events.lock().await.push(format!("status:{:?}", new));
            }
            async fn on_feedback(&self, round: u32, _: &str) {
                self.events.lock().await.push(format!("feedback:{}", round));
            }
        }

        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let lp = RuntimeLoopBuilder::new()
            .max_rounds(2)
            .executor(Arc::new(FixedExecutor::new("out")))
            .evaluator(Arc::new(FixedEvaluator(0.5)))
            .observer(Arc::new(RecordingObserver {
                events: events.clone(),
            }))
            .build();

        let _ = lp.run("goal").await.unwrap();

        let recorded = events.lock().await;
        assert!(recorded.iter().any(|e| e.starts_with("start:")));
        assert!(recorded.iter().any(|e| e.starts_with("complete:")));
        assert!(recorded.iter().any(|e| e.starts_with("status:")));
        assert!(recorded.iter().any(|e| e.starts_with("feedback:")));
    }
}
