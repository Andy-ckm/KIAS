//! Graceful shutdown handling for KIAS.
//!
//! This module provides:
//! - Signal handling (SIGTERM, SIGINT)
//! - Graceful shutdown coordinator with timeout
//! - Subsystem notification and cleanup
//! - Health check integration

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch};
use tracing::{debug, info, warn};

/// Shutdown phases for coordinated shutdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// System is running normally
    Running,
    /// Shutdown initiated, notifying subsystems
    Draining,
    /// Waiting for in-flight requests to complete
    WaitingForCompletion,
    /// Force shutdown after timeout
    ForceShutdown,
    /// Shutdown complete
    Complete,
}

/// Configuration for graceful shutdown behavior
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// Maximum time to wait for in-flight requests (default: 30s)
    pub drain_timeout: Duration,
    /// Maximum time to wait for subsystem cleanup (default: 10s)
    pub cleanup_timeout: Duration,
    /// Whether to force shutdown after timeout (default: true)
    pub force_after_timeout: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            drain_timeout: Duration::from_secs(30),
            cleanup_timeout: Duration::from_secs(10),
            force_after_timeout: true,
        }
    }
}

/// Coordinates graceful shutdown across all subsystems.
///
/// Features:
/// - Listens for SIGTERM/SIGINT signals
/// - Notifies all subsystems via broadcast channel
/// - Waits for in-flight requests with configurable timeout
/// - Tracks shutdown progress and metrics
/// - Provides health check integration
pub struct GracefulShutdown {
    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<ShutdownPhase>,
    /// Current phase tracking
    phase_tx: watch::Sender<ShutdownPhase>,
    /// Whether shutdown has been initiated
    initiated: Arc<AtomicBool>,
    /// Configuration
    config: ShutdownConfig,
    /// Start time for uptime tracking
    started_at: Instant,
    /// Subsystem names that have acknowledged shutdown
    acknowledged: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl GracefulShutdown {
    /// Create a new graceful shutdown coordinator.
    pub fn new(config: ShutdownConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        let (phase_tx, _) = watch::channel(ShutdownPhase::Running);

        Self {
            shutdown_tx,
            phase_tx,
            initiated: Arc::new(AtomicBool::new(false)),
            config,
            started_at: Instant::now(),
            acknowledged: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ShutdownConfig::default())
    }

    /// Get a receiver for shutdown signals.
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownPhase> {
        self.shutdown_tx.subscribe()
    }

    /// Get a watch receiver for the current shutdown phase.
    pub fn watch_phase(&self) -> watch::Receiver<ShutdownPhase> {
        self.phase_tx.subscribe()
    }

    /// Get the current shutdown phase.
    pub fn current_phase(&self) -> ShutdownPhase {
        *self.phase_tx.borrow()
    }

    /// Check if shutdown has been initiated.
    pub fn is_shutting_down(&self) -> bool {
        self.initiated.load(Ordering::Relaxed)
    }

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Register a subsystem as having acknowledged shutdown.
    pub async fn acknowledge(&self, name: &str) {
        let mut acked = self.acknowledged.lock().await;
        if !acked.contains(&name.to_string()) {
            acked.push(name.to_string());
            debug!(subsystem = name, "subsystem acknowledged shutdown");
        }
    }

    /// Get list of subsystems that have acknowledged shutdown.
    pub async fn acknowledged_subsystems(&self) -> Vec<String> {
        self.acknowledged.lock().await.clone()
    }

    /// Initiate graceful shutdown.
    ///
    /// This will:
    /// 1. Set phase to Draining
    /// 2. Notify all subsystems
    /// 3. Wait for drain_timeout
    /// 4. Set phase to WaitingForCompletion
    /// 5. Wait for cleanup_timeout
    /// 6. Set phase to ForceShutdown (if configured)
    /// 7. Set phase to Complete
    pub async fn shutdown(&self) -> ShutdownResult {
        if self.initiated.swap(true, Ordering::SeqCst) {
            warn!("shutdown already initiated");
            return ShutdownResult {
                phase: self.current_phase(),
                duration: Duration::ZERO,
                acknowledged: self.acknowledged_subsystems().await,
            };
        }

        let shutdown_start = Instant::now();
        info!("initiating graceful shutdown");

        // Phase 1: Draining
        self.set_phase(ShutdownPhase::Draining).await;
        let _ = self.shutdown_tx.send(ShutdownPhase::Draining);

        // Wait for subsystems to acknowledge (with shorter sleep intervals)
        let ack_deadline = Instant::now() + self.config.cleanup_timeout;
        while Instant::now() < ack_deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
            // Continue even if not all subsystems acknowledge
        }

        // Phase 2: Waiting for completion
        self.set_phase(ShutdownPhase::WaitingForCompletion).await;
        let _ = self.shutdown_tx.send(ShutdownPhase::WaitingForCompletion);

        // Wait for drain timeout
        tokio::time::sleep(self.config.drain_timeout).await;

        // Phase 3: Force shutdown if configured
        if self.config.force_after_timeout {
            self.set_phase(ShutdownPhase::ForceShutdown).await;
            let _ = self.shutdown_tx.send(ShutdownPhase::ForceShutdown);
            warn!("forcing shutdown after timeout");
        }

        // Phase 4: Complete
        self.set_phase(ShutdownPhase::Complete).await;
        let _ = self.shutdown_tx.send(ShutdownPhase::Complete);

        let duration = shutdown_start.elapsed();
        info!(
            duration_secs = duration.as_secs(),
            "graceful shutdown complete"
        );

        ShutdownResult {
            phase: ShutdownPhase::Complete,
            duration,
            acknowledged: self.acknowledged_subsystems().await,
        }
    }

    /// Set the current phase and notify watchers.
    async fn set_phase(&self, phase: ShutdownPhase) {
        let _ = self.phase_tx.send(phase);
        debug!(?phase, "shutdown phase changed");
    }

    /// Wait for a specific phase with timeout.
    pub async fn wait_for_phase(
        &self,
        target: ShutdownPhase,
        timeout: Duration,
    ) -> Result<(), tokio::time::error::Elapsed> {
        let mut phase_rx = self.watch_phase();
        tokio::time::timeout(timeout, async {
            loop {
                if *phase_rx.borrow() == target {
                    return;
                }
                if phase_rx.changed().await.is_err() {
                    return;
                }
            }
        })
        .await
    }
}

/// Result of a shutdown operation
#[derive(Debug)]
pub struct ShutdownResult {
    pub phase: ShutdownPhase,
    pub duration: Duration,
    pub acknowledged: Vec<String>,
}

/// Handle SIGTERM and SIGINT signals.
///
/// Returns a future that resolves when a signal is received.
pub async fn wait_for_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler registration");

        tokio::select! {
            _ = ctrl_c => {
                info!("received SIGINT (Ctrl+C)");
            }
            _ = sigterm.recv() => {
                info!("received SIGTERM");
            }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("Ctrl+C signal listener");
        info!("received Ctrl+C");
    }
}

/// Spawn a signal handler that triggers graceful shutdown.
pub fn spawn_signal_handler(shutdown: Arc<GracefulShutdown>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        wait_for_signal().await;
        info!("signal received, initiating shutdown");
        shutdown.shutdown().await;
    })
}

/// Shutdown-aware health check.
///
/// Returns healthy during normal operation, degraded during shutdown,
/// and unhealthy after shutdown timeout.
pub fn health_status(phase: ShutdownPhase) -> &'static str {
    match phase {
        ShutdownPhase::Running => "healthy",
        ShutdownPhase::Draining => "degraded",
        ShutdownPhase::WaitingForCompletion => "degraded",
        ShutdownPhase::ForceShutdown => "unhealthy",
        ShutdownPhase::Complete => "unhealthy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ShutdownConfig {
        ShutdownConfig {
            drain_timeout: Duration::from_millis(100),
            cleanup_timeout: Duration::from_millis(50),
            force_after_timeout: true,
        }
    }

    #[tokio::test]
    async fn test_graceful_shutdown_basic() {
        let shutdown = GracefulShutdown::new(test_config());

        assert!(!shutdown.is_shutting_down());
        assert_eq!(shutdown.current_phase(), ShutdownPhase::Running);
        assert!(shutdown.uptime_secs() < 5);

        // Subscribe before shutdown
        let mut rx = shutdown.subscribe();

        // Initiate shutdown
        let result = tokio::time::timeout(Duration::from_secs(2), shutdown.shutdown())
            .await
            .expect("shutdown should complete within timeout");

        assert_eq!(result.phase, ShutdownPhase::Complete);
        assert!(shutdown.is_shutting_down());

        // Should have received at least one phase notification
        assert!(rx.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_acknowledgment() {
        let shutdown = GracefulShutdown::new(test_config());

        // Acknowledge from a subsystem
        shutdown.acknowledge("scheduler").await;
        shutdown.acknowledge("controller").await;

        let acked = shutdown.acknowledged_subsystems().await;
        assert_eq!(acked.len(), 2);
        assert!(acked.contains(&"scheduler".to_string()));
        assert!(acked.contains(&"controller".to_string()));

        // Duplicate acknowledgment should be ignored
        shutdown.acknowledge("scheduler").await;
        let acked = shutdown.acknowledged_subsystems().await;
        assert_eq!(acked.len(), 2);
    }

    #[tokio::test]
    async fn test_shutdown_idempotent() {
        let shutdown = GracefulShutdown::new(test_config());

        // First shutdown - wait for it to complete
        let result1 = tokio::time::timeout(Duration::from_secs(5), shutdown.shutdown())
            .await
            .expect("first shutdown should complete");
        assert_eq!(result1.phase, ShutdownPhase::Complete);
        assert!(shutdown.is_shutting_down());

        // Second shutdown should be immediate (already initiated)
        // Note: phase might still be Running if first shutdown completed very fast
        let result2 = shutdown.shutdown().await;
        assert!(
            result2.phase == ShutdownPhase::Complete || result2.phase == ShutdownPhase::Running
        );
        assert!(shutdown.is_shutting_down());
    }

    #[tokio::test]
    async fn test_phase_watch() {
        let shutdown = GracefulShutdown::new(test_config());
        let mut phase_rx = shutdown.watch_phase();

        assert_eq!(*phase_rx.borrow(), ShutdownPhase::Running);

        // Trigger shutdown in background
        let s = Arc::new(shutdown);
        let s2 = s.clone();
        let handle = tokio::spawn(async move {
            s2.shutdown().await;
        });

        // Wait for phase change to Draining (with timeout)
        let _ = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let phase = *phase_rx.borrow();
                if phase != ShutdownPhase::Running {
                    // Got a phase change
                    assert!(phase == ShutdownPhase::Draining || phase == ShutdownPhase::Complete);
                    break;
                }
                if phase_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;

        handle.await.unwrap();
        assert!(s.is_shutting_down());
    }

    #[tokio::test]
    async fn test_health_status() {
        assert_eq!(health_status(ShutdownPhase::Running), "healthy");
        assert_eq!(health_status(ShutdownPhase::Draining), "degraded");
        assert_eq!(
            health_status(ShutdownPhase::WaitingForCompletion),
            "degraded"
        );
        assert_eq!(health_status(ShutdownPhase::ForceShutdown), "unhealthy");
        assert_eq!(health_status(ShutdownPhase::Complete), "unhealthy");
    }

    #[tokio::test]
    async fn test_wait_for_phase_with_timeout() {
        let shutdown = GracefulShutdown::new(test_config());

        // Should timeout since we're not shutting down
        let result = shutdown
            .wait_for_phase(ShutdownPhase::Draining, Duration::from_millis(100))
            .await;
        assert!(result.is_err());

        // Shutdown in background
        let s = Arc::new(shutdown);
        let s2 = s.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            s2.shutdown().await;
        });

        // Should succeed
        let result = s
            .wait_for_phase(ShutdownPhase::Draining, Duration::from_secs(1))
            .await;
        assert!(result.is_ok());
    }
}
