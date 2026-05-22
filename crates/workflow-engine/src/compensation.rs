
// ---------------------------------------------------------------------
// Stub for the external crate – replace with real import in production.
// ---------------------------------------------------------------------
pub mod kias_common {
    use std::fmt;
    use std::error;

    /// Minimal version of the error type used by the rest of the system.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KiasError {
        pub code: i32,
        pub message: String,
    }

    impl fmt::Display for KiasError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "KiasError {}: {}", self.code, self.message)
        }
    }

    impl error::Error for KiasError {}

    impl KiasError {
        /// Helper constructor.
        pub fn new(code: i32, msg: impl Into<String>) -> Self {
            KiasError { code, message: msg.into() }
        }
    }
}

// ---------------------------------------------------------------------
// Core imports (replace the stub with the real one if available)
// ---------------------------------------------------------------------
use kias_common::KiasError;

// ---------------------------------------------------------------------
// 1️⃣  CompensationHandler trait
// ---------------------------------------------------------------------
/// Units of work that can be executed forward and undone.
/// All implementations must be `Send + Sync` so the saga can be shared
/// across threads.
pub trait CompensationHandler: Send + Sync {
    /// Human‑readable name of the step.
    fn name(&self) -> &str;

    /// Execute the forward action.
    fn execute(&self) -> Result<(), KiasError>;

    /// Undo / compensate the forward action.
    fn compensate(&self) -> Result<(), KiasError>;
}

// ---------------------------------------------------------------------
// 2️⃣  Status tracking for a single step
// ---------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Executed,
    Compensated,
    Failed,
}

// ---------------------------------------------------------------------
// 3️⃣  Step wrapper that ties a handler to its status
// ---------------------------------------------------------------------
/// A saga step that knows how to run forward and how to roll back.
pub struct Step<H: CompensationHandler> {
    name: String,
    handler: H,
    status: StepStatus,
}

impl<H: CompensationHandler> Step<H> {
    pub fn new(handler: H) -> Self {
        Step {
            name: handler.name().to_owned(),
            handler,
            status: StepStatus::Pending,
        }
    }

    /// Returns a reference to the underlying handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Current execution status.
    pub fn status(&self) -> StepStatus {
        self.status
    }

    /// Execute the forward action and mark the step as `Executed`.
    pub fn execute(&mut self) -> Result<(), KiasError> {
        if self.status != StepStatus::Pending {
            // Idempotent – if already executed we can treat it as success.
            return Ok(());
        }
        self.handler.execute()?;
        self.status = StepStatus::Executed;
        Ok(())
    }

    /// Undo the step and mark it as `Compensated`.
    /// The forward action must have been executed before compensation.
    pub fn compensate(&mut self) -> Result<(), KiasError> {
        if self.status == StepStatus::Compensated {
            // Idempotent compensation – already rolled back.
            return Ok(());
        }
        if self.status != StepStatus::Executed {
            // Guard: we cannot compensate a step that hasn't been executed.
            return Err(KiasError::new(
                500,
                format!("Cannot compensate step '{}' – status is {:?}",
                        self.name, self.status),
            ));
        }
        self.handler.compensate()?;
        self.status = StepStatus::Compensated;
        Ok(())
    }
}

// ---------------------------------------------------------------------
// 4️⃣  RollbackChain – a list of compensating actions
// ---------------------------------------------------------------------
/// Simple wrapper around a list of compensating closures.
/// Each closure is `Send + Sync` and returns `Result<(), KiasError>`.
pub struct RollbackChain {
    actions: Vec<Box<dyn Fn() -> Result<(), KiasError> + Send + Sync>>,
}

impl Default for RollbackChain {
    fn default() -> Self {
        RollbackChain { actions: Vec::new() }
    }
}

impl RollbackChain {
    /// Create an empty chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new compensating action.
    /// The closure will be called **in reverse order** when the chain runs.
    pub fn push<F>(&mut self, action: F)
    where
        F: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
    {
        self.actions.push(Box::new(action));
    }

    /// Execute all stored compensating actions in reverse order.
    /// The first error aborts the chain and is returned.
    pub fn execute_all(&mut self) -> Result<(), KiasError> {
        for action in self.actions.iter().rev() {
            action()?;
        }
        Ok(())
    }

    /// Returns how many actions are queued.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// True if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

// ---------------------------------------------------------------------
// 5️⃣  SagaError – the error type used by the saga orchestrator
// ---------------------------------------------------------------------
#[derive(Debug)]
pub enum SagaError {
    /// Wraps a raw `KiasError` coming from a handler.
    Kias(KiasError),
    /// A forward step failed.
    StepFailed(String),
    /// A compensation step failed.
    CompensationFailed(String),
    /// Attempted to start a saga that has already been rolled back.
    AlreadyRolledBack,
    /// Requested an invalid step index.
    InvalidIndex,
}

impl fmt::Display for SagaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SagaError::Kias(e) => write!(f, "Kias error: {}", e),
            SagaError::StepFailed(name) => write!(f, "Step '{}' failed", name),
            SagaError::CompensationFailed(name) => {
                write!(f, "Compensation for step '{}' failed", name)
            }
            SagaError::AlreadyRolledBack => write!(f, "Saga already rolled back"),
            SagaError::InvalidIndex => write!(f, "Invalid step index"),
        }
    }
}

impl std::error::Error for SagaError {}

impl From<KiasError> for SagaError {
    fn from(e: KiasError) -> Self {
        SagaError::Kias(e)
    }
}

// ---------------------------------------------------------------------
// 6️⃣  Saga – the orchestrator
// ---------------------------------------------------------------------
/// The saga holds a collection of steps and a rollback chain.
/// It can either run to completion or, on failure, roll back already‑executed steps.
pub struct Saga {
    steps: Vec<StepEntry>,
    /// Index of the first step that has **not** been executed.
    next_index: usize,
    rollback_chain: RollbackChain,
    /// Set to true when the saga has been fully rolled back.
    rolled_back: bool,
}

/// Internal representation of a single saga step.
struct StepEntry {
    name: String,
    /// Forward action.
    execute: Box<dyn Fn() -> Result<(), KiasError> + Send + Sync>,
    /// Compensation action.
    compensate: Box<dyn Fn() -> Result<(), KiasError> + Send + Sync>,
    status: StepStatus,
}

impl Default for Saga {
    fn default() -> Self {
        Saga {
            steps: Vec::new(),
            next_index: 0,
            rollback_chain: RollbackChain::new(),
            rolled_back: false,
        }
    }
}

impl Saga {
    /// Create a fresh saga.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new step to the saga.
    ///
    /// The closures `forward` and `compensation` must be `Send + Sync`
    /// and may capture environment variables.
    pub fn add_step<F, G>(&mut self, name: impl Into<String>, forward: F, compensation: G)
    where
        F: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
        G: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
    {
        self.steps.push(StepEntry {
            name: name.into(),
            execute: Box::new(forward),
            compensate: Box::new(compensation),
            status: StepStatus::Pending,
        });
    }

    /// Returns the number of steps in the saga.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Returns true if the saga has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Execute the saga forward.
    ///
    /// If any step fails, the saga automatically rolls back all
    /// already‑executed steps using the stored compensation closures.
    pub fn execute(&mut self) -> Result<(), SagaError> {
        if self.rolled_back {
            return Err(SagaError::AlreadyRolledBack);
        }

        while self.next_index < self.steps.len() {
            let idx = self.next_index;
            let step = &mut self.steps[idx];

            // Run forward action.
            (step.execute)().map_err(|e| {
                // Forward failed – rollback everything before this step.
                self.rollback_to(idx)?;
                SagaError::StepFailed(step.name.clone())
            })?;

            step.status = StepStatus::Executed;

            // Register the compensation action for later rollback.
            let name = step.name.clone();
            let comp = step.compensate.clone();
            self.rollback_chain.push(move || {
                comp().map_err(|e| {
                    // In a real system you may want to log this.
                    e
                })
            });

            self.next_index += 1;
        }

        // All steps succeeded – clear the rollback chain (no need to keep it).
        self.rollback_chain = RollbackChain::new();
        Ok(())
    }

    /// Roll back to (and including) the step at `target_index`.
    /// All steps with indices `0..=target_index` are compensated.
    ///
    /// If `target_index` is greater than `next_index` (i.e. the step has not
    /// been executed yet) this is a no‑op.
    pub fn rollback_to(&mut self, target_index: usize) -> Result<(), SagaError> {
        if target_index >= self.steps.len() {
            return Err(SagaError::InvalidIndex);
        }

        // Run the rollback chain (which holds actions for already‑executed steps).
        // The chain stores actions in forward order; executing it reversed
        // yields the correct rollback order.
        if let Err(e) = self.rollback_chain.execute_all() {
            // Compensation failure is considered fatal – we still mark the saga
            // as rolled back, but we return the error so the caller knows.
            self.rolled_back = true;
            return Err(SagaError::CompensationFailed(
                self.steps
                    .get(target_index)
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
            ));
        }

        // Update status of each rolled‑back step.
        for step in self.steps.iter_mut().take(target_index + 1) {
            step.status = StepStatus::Compensated;
        }

        // Clear the chain – no further rollback is possible.
        self.rollback_chain = RollbackChain::new();
        self.rolled_back = true;
        Ok(())
    }

    /// Roll back the entire saga (all executed steps).
    pub fn rollback_all(&mut self) -> Result<(), SagaError> {
        if self.next_index == 0 {
            // Nothing has been executed – nothing to roll back.
            return Ok(());
        }
        // next_index - 1 is the index of the last executed step.
        self.rollback_to(self.next_index.saturating_sub(1))
    }

    /// Returns a snapshot of the current status of each step.
    pub fn status_snapshot(&self) -> Vec<(String, StepStatus)> {
        self.steps
            .iter()
            .map(|s| (s.name.clone(), s.status))
            .collect()
    }
}

// ---------------------------------------------------------------------
// 7️⃣  Convenience helpers for building sagas with less boilerplate
// ---------------------------------------------------------------------
/// Helper trait to allow chaining `and_then` style steps.
pub trait SagaBuilder {
    fn and_then<F, G>(self, name: &'static str, forward: F, compensation: G) -> Self
    where
        F: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
        G: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
        Self: Sized;
}

impl SagaBuilder for Saga {
    fn and_then<F, G>(mut self, name: &'static str, forward: F, compensation: G) -> Self
    where
        F: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
        G: Fn() -> Result<(), KiasError> + 'static + Send + Sync,
    {
        self.add_step(name, forward, compensation);
        self
    }
}

// ---------------------------------------------------------------------
// 8️⃣  Example usage (can be removed in production)
// ---------------------------------------------------------------------
#[cfg(test)]
mod usage_example {
    use super::*;

    /// Simple mock handler that records calls.
    pub struct MockHandler {
        pub name: String,
        pub exec_count: std::sync::atomic::AtomicUsize,
        pub comp_count: std::sync::atomic::AtomicUsize,
        pub should_fail: std::sync::atomic::AtomicBool,
    }

    impl MockHandler {
        pub fn new(name: &str) -> Self {
            MockHandler {
                name: name.to_owned(),
                exec_count: std::sync::atomic::AtomicUsize::new(0),
                comp_count: std::sync::atomic::AtomicUsize::new(0),
                should_fail: std::sync::atomic::AtomicBool::new(false),
            }
        }
        pub fn set_fail(&self) {
            self.should_fail.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl CompensationHandler for MockHandler {
        fn name(&self) -> &str {
            &self.name
        }
        fn execute(&self) -> Result<(), KiasError> {
            self.exec_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.should_fail.load(std::sync::atomic::Ordering::SeqCst) {
                Err(KiasError::new(1, "Mock failure"))
            } else {
                Ok(())
            }
        }
        fn compensate(&self) -> Result<(), KiasError> {
            self.comp_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------
// 9️⃣  Tests – 6 scenarios
// ---------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -------------------------------------------------------------------------
    // Helper to create a saga that succeeds end‑to‑end.
    // -------------------------------------------------------------------------
    #[test]
    fn saga_all_steps_succeed() {
        let mut saga = Saga::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        saga.add_step("step1", || { c.fetch_add(1, Ordering::SeqCst); Ok(()) }, || Ok(()));
        saga.add_step("step2", || { c.fetch_add(10, Ordering::SeqCst); Ok(()) }, || Ok(()));
        saga.add_step("step3", || { c.fetch_add(100, Ordering::SeqCst); Ok(()) }, || Ok(()));

        saga.execute().expect("Saga should succeed");
        assert_eq!(counter.load(Ordering::SeqCst), 111);

        let snap = saga.status_snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].1, StepStatus::Executed);
        assert_eq!(snap[1].1, StepStatus::Executed);
        assert_eq!(snap[2].1, StepStatus::Executed);
    }

    // -------------------------------------------------------------------------
    // Helper to create a saga that fails on the second step.
    // -------------------------------------------------------------------------
    #[test]
    fn saga_fails_on_second_step_triggers_rollback_of_first() {
        let exec1 = Arc::new(AtomicUsize::new(0));
        let comp1 = Arc::new(AtomicUsize::new(0));
        let exec2 = Arc::new(AtomicUsize::new(0));
        let exec3 = Arc::new(AtomicUsize::new(0)); // should never run

        let e1 = exec1.clone();
        let c1 = comp1.clone();
        let e2 = exec2.clone();

        let mut saga = Saga::new();

        saga.add_step(
            "step1",
            move || { e1.fetch_add(1, Ordering::SeqCst); Ok(()) },
            move || { c1.fetch_add(1, Ordering::SeqCst); Ok(()) },
        );

        saga.add_step(
            "step2",
            move || {
                e2.fetch_add(1, Ordering::SeqCst);
                Err(KiasError::new(42, "Intentional failure"))
            },
            || Ok(()),
        );

        saga.add_step(
            "step3",
            move || { exec3.fetch_add(1, Ordering::SeqCst); Ok(()) },
            || Ok(()),
        );

        let result = saga.execute();
        assert!(result.is_err(), "Saga should have failed");

        // step1 must have been executed, compensated
        assert_eq!(exec1.load(Ordering::SeqCst), 1);
        assert_eq!(comp1.load(Ordering::SeqCst), 1);

        // step2 attempted but failed
        assert_eq!(exec2.load(Ordering::SeqCst), 1);

        // step3 never ran
        assert_eq!(exec3.load(Ordering::SeqCst), 0);

        let snap = saga.status_snapshot();
        assert_eq!(snap[0].1, StepStatus::Compensated);
        assert_eq!(snap[1].1, StepStatus::Failed); // step2 itself didn't succeed
        assert_eq!(snap[2].1, StepStatus::Pending);
    }

    // -------------------------------------------------------------------------
    // Saga fails on first step – no rollback needed.
    // -------------------------------------------------------------------------
    #[test]
    fn saga_fails_on_first_step_no_rollback_needed() {
        let exec1 = Arc::new(AtomicUsize::new(0));
        let comp1 = Arc::new(AtomicUsize::new(0));
        let e1 = exec1.clone();
        let c1 = comp1.clone();

        let mut saga = Saga::new();
        saga.add_step(
            "first",
            move || {
                e1.fetch_add(1, Ordering::SeqCst);
                Err(KiasError::new(99, "First step fails"))
            },
            move || { c1.fetch_add(1, Ordering::SeqCst); Ok(()) },
        );
        saga.add_step("second", || Ok(()), || Ok(()));

        let result = saga.execute();
        assert!(result.is_err());

        // No step succeeded, therefore no compensation should happen.
        assert_eq!(exec1.load(Ordering::SeqCst), 1);
        assert_eq!(comp1.load(Ordering::SeqCst), 0);
    }

    // -------------------------------------------------------------------------
    // Compensation itself fails – saga error propagates.
    // -------------------------------------------------------------------------
    #[test]
    fn compensation_failure_propagates() {
        let exec1 = Arc::new(AtomicUsize::new(0));
        let comp1_fails = Arc::new(AtomicUsize::new(0));
        let exec2 = Arc::new(AtomicUsize::new(0));
        let e1 = exec1.clone();
        let cf = comp1_fails.clone();

        let mut saga = Saga::new();
        saga.add_step(
            "step1",
            move || { e1.fetch_add(1, Ordering::SeqCst); Ok(()) },
            move || {
                cf.fetch_add(1, Ordering::SeqCst);
                Err(KiasError::new(500, "Compensation fails!"))
            },
        );

        saga.add_step(
            "step2",
            move || {
                exec2.fetch_add(1, Ordering::SeqCst);
                Err(KiasError::new(2, "Step2 fails"))
            },
            || Ok(()),
        );

        let result = saga.execute();
        assert!(result.is_err());

        // step1 executed, compensation attempted (and failed)
        assert_eq!(exec1.load(Ordering::SeqCst), 1);
        assert_eq!(comp1_fails.load(Ordering::SeqCst), 1);
    }

    // -------------------------------------------------------------------------
    // Multi‑step saga that succeeds, then explicit rollback_all.
    // -------------------------------------------------------------------------
    #[test]
    fn rollback_all_after_success() {
        let exec1 = Arc::new(AtomicUsize::new(0));
        let comp1 = Arc::new(AtomicUsize::new(0));
        let exec2 = Arc::new(AtomicUsize::new(0));
        let comp2 = Arc::new(AtomicUsize::new(0));
        let e1 = exec1.clone();
        let c1 = comp1.clone();
        let e2 = exec2.clone();
        let c2 = comp2.clone();

        let mut saga = Saga::new();
        saga.add_step(
            "A",
            move || { e1.fetch_add(1, Ordering::SeqCst); Ok(()) },
            move || { c1.fetch_add(1, Ordering::SeqCst); Ok(()) },
        );
        saga.add_step(
            "B",
            move || { e2.fetch_add(1, Ordering::SeqCst); Ok(()) },
            move || { c2.fetch_add(1, Ordering::SeqCst); Ok(()) },
        );

        saga.execute().expect("Saga should succeed");
        assert_eq!(exec1.load(Ordering::SeqCst), 1);
        assert_eq!(exec2.load(Ordering::SeqCst), 1);

        // Now roll back everything.
        saga.rollback_all().expect("rollback_all should succeed");

        assert_eq!(comp1.load(Ordering::SeqCst), 1);
        assert_eq!(comp2.load(Ordering::SeqCst), 1);

        let snap = saga.status_snapshot();
        assert_eq!(snap[0].1, StepStatus::Compensated);
        assert_eq!(snap[1].1, StepStatus::Compensated);
    }

    // -------------------------------------------------------------------------
    // Idempotent compensation – calling compensate twice does not double count.
    // -------------------------------------------------------------------------
    #[test]
    fn idempotent_compensation() {
        let comp_counter = Arc::new(AtomicUsize::new(0));
        let cc = comp_counter.clone();

        // Build a single-step saga that succeeds.
        let mut saga = Saga::new();
        saga.add_step(
            "single",
            || Ok(()),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );

        saga.execute().expect("forward should succeed");
        // First rollback.
        saga.rollback_all().expect("first rollback");
        assert_eq!(comp_counter.load(Ordering::SeqCst), 1);

        // Second rollback – should be idempotent (no extra compensation).
        saga.rollback_all().expect("second rollback should succeed");
        assert_eq!(comp_counter.load(Ordering::SeqCst), 1);
    }

    // -------------------------------------------------------------------------
    // Test rollback_to with exact index.
    // -------------------------------------------------------------------------
    #[test]
    fn rollback_to_specific_index() {
        let exec = (0..5).map(|_| Arc::new(AtomicUsize::new(0))).collect::<Vec<_>>();
        let comp = (0..5).map(|_| Arc::new(AtomicUsize::new(0))).collect::<Vec<_>>();

        let mut saga = Saga::new();
        for i in 0..5 {
            let e = exec[i].clone();
            let c = comp[i].clone();
            saga.add_step(
                format!("step{}", i),
                move || { e.fetch_add(1, Ordering::SeqCst); Ok(()) },
                move || { c.fetch_add(1, Ordering::SeqCst); Ok(()) },
            );
        }

        saga.execute().expect("All steps succeed");

        // Roll back to index 2 (i.e. keep step0‑2 executed, undo step3‑4).
        saga.rollback_to(2).expect("rollback_to should succeed");

        // Steps 0..=2 still executed; 3..=4 compensated.
        for i in 0..=2 {
            assert_eq!(exec[i].load(Ordering::SeqCst), 1, "step{} executed", i);
            assert_eq!(comp[i].load(Ordering::SeqCst), 0, "step{} not compensated", i);
        }
        for i in 3..5 {
            assert_eq!(exec[i].load(Ordering::SeqCst), 1, "step{} executed", i);
            assert_eq!(comp[i].load(Ordering::SeqCst), 1, "step{} compensated", i);
        }
    }

    // -------------------------------------------------------------------------
    // Test that a saga cannot be started after it has been rolled back.
    // -------------------------------------------------------------------------
    #[test]
    fn saga_cannot_restart_after_rollback() {
        let mut saga = Saga::new();
        saga.add_step("a", || Ok(()), || Ok(()));
        saga.add_step("b", || Err(KiasError::new(1, "fail")), || Ok(()));

        let result = saga.execute();
        assert!(result.is_err());

        // Trying to execute again should fail with AlreadyRolledBack.
        let second = saga.execute();
        assert!(matches!(second, Err(SagaError::AlreadyRolledBack)));
    }

    // -------------------------------------------------------------------------
    // Test that an invalid rollback_to index returns InvalidIndex error.
    // -------------------------------------------------------------------------
    #[test]
    fn rollback_to_invalid_index() {
        let mut saga = Saga::new();
        saga.add_step("a", || Ok(()), || Ok(()));

        // Index out of range.
        let res = saga.rollback_to(5);
        assert!(matches!(res, Err(SagaError::InvalidIndex)));
    }
}