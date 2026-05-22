//! Process Supervisor — unified lifecycle management for agent processes.
//!
//! ## Features
//! 1. **Process Lifecycle** — Starting → Running → Stopping → Crashed → Stopped
//! 2. **Crash拉起** — automatic restart with configurable retry policy
//! 3. **指数退避** — back-off between restarts to prevent crash loops
//! 4. **熔断隔离** — circuit breaker trips after `consecutive_failures` and pauses restarts
//!
//! ## State Machine
//! ```text
//!  [Start] ──→ Starting ──→ Running ──→ Stopping ──→ Stopped
//!                  │             │            ▲
//!                  │             ▼            │
//!                  │         Crashed ─────────┤
//!                  │             │            │
//!                  ▼             ▼            │
//!              (retry)    (exponential         │
//!                          backoff)           │
//!                  │             │            │
//!                  ▼             ▼            ▼
//!              [CircuitOpen] ──────────────→ Stopped
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Process lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProcessLifecycle {
    #[default]
    Starting,
    Running,
    Stopping,
    Crashed,
    Stopped,
    /// Circuit is open — no restarts will be attempted.
    CircuitOpen,
}

impl std::fmt::Display for ProcessLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessLifecycle::Starting => write!(f, "Starting"),
            ProcessLifecycle::Running => write!(f, "Running"),
            ProcessLifecycle::Stopping => write!(f, "Stopping"),
            ProcessLifecycle::Crashed => write!(f, "Crashed"),
            ProcessLifecycle::Stopped => write!(f, "Stopped"),
            ProcessLifecycle::CircuitOpen => write!(f, "CircuitOpen"),
        }
    }
}

/// Configuration for the process supervisor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSupervisorConfig {
    /// Maximum number of restart attempts before circuit breaker opens.
    pub max_restart_attempts: u32,
    /// Initial delay between restart attempts.
    pub initial_backoff_secs: u64,
    /// Maximum backoff delay (caps exponential growth).
    pub max_backoff_secs: u64,
    /// After how many consecutive successes to reset failure counter.
    pub success_reset_threshold: u32,
}

impl Default for ProcessSupervisorConfig {
    fn default() -> Self {
        Self {
            max_restart_attempts: 5,
            initial_backoff_secs: 1,
            max_backoff_secs: 300,
            success_reset_threshold: 3,
        }
    }
}

/// Runtime state for a supervised process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    pub pid: Option<u32>,
    pub lifecycle: ProcessLifecycle,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub current_backoff_secs: u64,
    pub restart_count: u32,
    pub last_crash_at: Option<DateTime<Utc>>,
    pub last_started_at: Option<DateTime<Utc>>,
    pub last_exit_code: Option<i32>,
    /// Key-value metadata (cmdline, workdir, env, etc.)
    pub metadata: HashMap<String, String>,
}

impl Default for ProcessState {
    fn default() -> Self {
        Self {
            pid: None,
            lifecycle: ProcessLifecycle::Starting,
            consecutive_failures: 0,
            consecutive_successes: 0,
            current_backoff_secs: 0,
            restart_count: 0,
            last_crash_at: None,
            last_started_at: None,
            last_exit_code: None,
            metadata: HashMap::new(),
        }
    }
}

/// The Process Supervisor itself.
#[derive(Debug, Clone)]
pub struct ProcessSupervisor {
    config: ProcessSupervisorConfig,
    state: ProcessState,
}

impl ProcessSupervisor {
    pub fn new(config: ProcessSupervisorConfig) -> Self {
        Self {
            config,
            state: ProcessState::default(),
        }
    }

    pub fn state(&self) -> &ProcessState {
        &self.state
    }

    // ── Lifecycle transitions ────────────────────────────────────────────────

    /// Call when the process has been started by the OS (pid assigned).
    pub fn started(&mut self, pid: u32) {
        self.state.pid = Some(pid);
        self.state.lifecycle = ProcessLifecycle::Running;
        self.state.last_started_at = Some(Utc::now());
        self.state.current_backoff_secs = self.config.initial_backoff_secs;
    }

    /// Call when the supervisor decides to stop the process gracefully.
    pub fn stopping(&mut self) {
        self.state.lifecycle = ProcessLifecycle::Stopping;
    }

    /// Call when the process exits normally.
    pub fn exited_normally(&mut self, exit_code: i32) {
        self.state.last_exit_code = Some(exit_code);
        self.state.lifecycle = ProcessLifecycle::Stopped;
        self.state.consecutive_failures = 0;
        self.state.consecutive_successes += 1;
        if self.state.consecutive_successes >= self.config.success_reset_threshold {
            self.state.current_backoff_secs = self.config.initial_backoff_secs;
        }
    }

    /// Call when the process crashes (non-zero exit or signal).
    pub fn crashed(&mut self, exit_code: i32) -> CrashAction {
        self.state.last_exit_code = Some(exit_code);
        self.state.last_crash_at = Some(Utc::now());
        self.state.consecutive_failures += 1;
        self.state.consecutive_successes = 0;
        self.state.restart_count += 1;

        if self.state.consecutive_failures >= self.config.max_restart_attempts {
            self.state.lifecycle = ProcessLifecycle::CircuitOpen;
            return CrashAction::CircuitOpen;
        }

        self.state.lifecycle = ProcessLifecycle::Crashed;

        // Exponential back-off
        let next = self
            .state
            .current_backoff_secs
            .saturating_mul(2)
            .min(self.config.max_backoff_secs);
        self.state.current_backoff_secs = next.max(1);

        CrashAction::RetryAfter(Duration::from_secs(self.state.current_backoff_secs))
    }

    /// Manually reset the circuit breaker (e.g., after an operator intervenes).
    pub fn reset_circuit(&mut self) {
        self.state.lifecycle = ProcessLifecycle::Stopped;
        self.state.consecutive_failures = 0;
        self.state.consecutive_successes = 0;
        self.state.current_backoff_secs = self.config.initial_backoff_secs;
    }

    /// Query whether a restart is allowed right now.
    pub fn can_restart(&self) -> bool {
        !matches!(
            self.state.lifecycle,
            ProcessLifecycle::CircuitOpen | ProcessLifecycle::Running | ProcessLifecycle::Stopping
        )
    }

    /// Return the recommended back-off duration before the next restart attempt.
    pub fn recommended_backoff(&self) -> Duration {
        Duration::from_secs(self.state.current_backoff_secs)
    }
}

/// What to do after a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashAction {
    /// Retry after the given duration.
    RetryAfter(Duration),
    /// Circuit is open — do not restart.
    CircuitOpen,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ProcessSupervisorConfig {
        ProcessSupervisorConfig {
            max_restart_attempts: 3,
            initial_backoff_secs: 1,
            max_backoff_secs: 64,
            success_reset_threshold: 2,
        }
    }

    #[test]
    fn test_starting_to_running() {
        let mut sup = ProcessSupervisor::new(cfg());
        assert_eq!(sup.state().lifecycle, ProcessLifecycle::Starting);
        sup.started(42);
        assert_eq!(sup.state().lifecycle, ProcessLifecycle::Running);
        assert_eq!(sup.state().pid, Some(42));
    }

    #[test]
    fn test_normal_exit_resets_failures() {
        let mut sup = ProcessSupervisor::new(cfg());
        sup.started(1);
        sup.crashed(1);
        sup.crashed(1);
        assert_eq!(sup.state().consecutive_failures, 2);
        sup.exited_normally(0);
        assert_eq!(sup.state().consecutive_failures, 0);
        assert_eq!(sup.state().lifecycle, ProcessLifecycle::Stopped);
    }

    #[test]
    fn test_exponential_backoff_grows() {
        let mut sup = ProcessSupervisor::new(cfg());
        sup.started(1); // initial backoff = 1
        let backoffs: Vec<u64> = (0..4)
            .map(|_| {
                let a = sup.crashed(1);
                if let CrashAction::RetryAfter(d) = a {
                    d.as_secs()
                } else {
                    0
                }
            })
            .collect();
        // started() sets initial_backoff=1, then each crash doubles it:
        // crash 1 → 2, crash 2 → 4, crash 3 → circuit open (0), crash 4 → 0
        assert_eq!(backoffs, &[2, 4, 0, 0]);
    }

    #[test]
    fn test_circuit_breaker_trips() {
        let mut sup = ProcessSupervisor::new(cfg()); // max_restart_attempts = 3
        sup.started(1);
        sup.crashed(1);
        sup.crashed(1);
        assert!(!matches!(
            sup.state().lifecycle,
            ProcessLifecycle::CircuitOpen
        ));
        let action = sup.crashed(1);
        assert!(matches!(action, CrashAction::CircuitOpen));
        assert_eq!(sup.state().lifecycle, ProcessLifecycle::CircuitOpen);
    }

    #[test]
    fn test_reset_circuit_allows_restart() {
        let mut sup = ProcessSupervisor::new(cfg());
        sup.started(1);
        for _ in 0..3 {
            sup.crashed(1);
        }
        assert!(matches!(
            sup.state().lifecycle,
            ProcessLifecycle::CircuitOpen
        ));
        sup.reset_circuit();
        assert!(sup.can_restart());
        assert_eq!(sup.state().lifecycle, ProcessLifecycle::Stopped);
    }

    #[test]
    fn test_success_reset_threshold_cools_backoff() {
        let mut sup = ProcessSupervisor::new(cfg()); // success_reset_threshold = 2
        sup.started(1);
        sup.crashed(1);
        sup.crashed(1);
        // backoff is now 4
        assert_eq!(sup.state().current_backoff_secs, 4);
        // Two successes should reset backoff to initial
        sup.exited_normally(0);
        sup.exited_normally(0);
        assert_eq!(sup.state().current_backoff_secs, 1);
    }

    #[test]
    fn test_max_backoff_cap() {
        let mut sup = ProcessSupervisor::new(cfg()); // max_backoff_secs = 64
        sup.started(1);
        for _ in 0..10 {
            sup.crashed(1);
            if !sup.can_restart() {
                break;
            }
            sup.started(2);
        }
        assert!(sup.state().current_backoff_secs <= 64);
    }

    #[test]
    fn test_cannot_restart_while_running() {
        let mut sup = ProcessSupervisor::new(cfg());
        sup.started(1);
        assert!(!sup.can_restart()); // Running is not restartable state
    }

    #[test]
    fn test_metadata_preserved_across_transitions() {
        let mut sup = ProcessSupervisor::new(cfg());
        sup.state
            .metadata
            .insert("cmd".into(), "python server.py".into());
        sup.started(1);
        sup.crashed(1);
        assert_eq!(
            sup.state.metadata.get("cmd"),
            Some(&"python server.py".into())
        );
    }
}
