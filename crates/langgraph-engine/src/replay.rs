//! # Time-Travel Debugger
//!
//! Enables full state reconstruction from checkpoints with step-through navigation.
//! Works with the existing [`CheckpointStore`] and [`ExecutionEvent`] infrastructure
//! to provide replay, diff, and breakpoint capabilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use crate::state::GraphState;
use crate::stream::ExecutionEvent;

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ReplayError {
    RunNotFound(String),
    StepOutOfBounds { step: usize, total: usize },
    NoEvents,
    StateReconstructionFailed(String),
    BreakpointExists(usize),
    StorageError(String),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunNotFound(id) => write!(f, "Run not found: {id}"),
            Self::StepOutOfBounds { step, total } => {
                write!(f, "Step {step} out of bounds (total: {total})")
            }
            Self::NoEvents => write!(f, "No events recorded"),
            Self::StateReconstructionFailed(msg) => {
                write!(f, "State reconstruction failed: {msg}")
            }
            Self::BreakpointExists(step) => write!(f, "Breakpoint already at step {step}"),
            Self::StorageError(msg) => write!(f, "Storage error: {msg}"),
        }
    }
}

impl std::error::Error for ReplayError {}

// ── Event Store ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: usize,
    pub event: ExecutionEvent,
    pub stored_at: DateTime<Utc>,
    pub state_snapshot: Option<GraphState>,
}

pub trait EventStore: Send + Sync {
    fn append(&self, run_id: &str, event: ExecutionEvent) -> Result<(), ReplayError>;
    fn get_events(&self, run_id: &str) -> Result<Vec<StoredEvent>, ReplayError>;
    fn get_events_range(
        &self,
        run_id: &str,
        from_seq: usize,
        to_seq: usize,
    ) -> Result<Vec<StoredEvent>, ReplayError>;
    fn event_count(&self, run_id: &str) -> Result<usize, ReplayError>;
    fn list_runs(&self) -> Result<Vec<String>, ReplayError>;
}

#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    runs: RwLock<HashMap<String, Vec<StoredEvent>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            runs: RwLock::new(HashMap::new()),
        }
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, run_id: &str, event: ExecutionEvent) -> Result<(), ReplayError> {
        let mut runs = self
            .runs
            .write()
            .map_err(|e| ReplayError::StorageError(e.to_string()))?;
        let events = runs.entry(run_id.to_string()).or_default();
        events.push(StoredEvent {
            seq: events.len(),
            event,
            stored_at: Utc::now(),
            state_snapshot: None,
        });
        Ok(())
    }

    fn get_events(&self, run_id: &str) -> Result<Vec<StoredEvent>, ReplayError> {
        let runs = self
            .runs
            .read()
            .map_err(|e| ReplayError::StorageError(e.to_string()))?;
        runs.get(run_id)
            .cloned()
            .ok_or_else(|| ReplayError::RunNotFound(run_id.to_string()))
    }

    fn get_events_range(
        &self,
        run_id: &str,
        from_seq: usize,
        to_seq: usize,
    ) -> Result<Vec<StoredEvent>, ReplayError> {
        let events = self.get_events(run_id)?;
        Ok(events
            .into_iter()
            .filter(|e| e.seq >= from_seq && e.seq <= to_seq)
            .collect())
    }

    fn event_count(&self, run_id: &str) -> Result<usize, ReplayError> {
        let runs = self
            .runs
            .read()
            .map_err(|e| ReplayError::StorageError(e.to_string()))?;
        runs.get(run_id)
            .map(|e| e.len())
            .ok_or_else(|| ReplayError::RunNotFound(run_id.to_string()))
    }

    fn list_runs(&self) -> Result<Vec<String>, ReplayError> {
        let runs = self
            .runs
            .read()
            .map_err(|e| ReplayError::StorageError(e.to_string()))?;
        Ok(runs.keys().cloned().collect())
    }
}

// ── Replay State ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayState {
    pub step: usize,
    pub event: ExecutionEvent,
    pub graph_state: Option<GraphState>,
    pub timestamp: DateTime<Utc>,
    pub has_breakpoint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub from_step: usize,
    pub to_step: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<(String, String, String)>,
}

impl fmt::Display for StateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "State diff: step {} -> {}", self.from_step, self.to_step)?;
        if !self.added.is_empty() {
            writeln!(f, "  + added: {}", self.added.join(", "))?;
        }
        if !self.removed.is_empty() {
            writeln!(f, "  - removed: {}", self.removed.join(", "))?;
        }
        for (key, old, new) in &self.modified {
            writeln!(f, "  ~ {key}: {old} -> {new}")?;
        }
        Ok(())
    }
}

// ── Replay Session ──────────────────────────────────────────────────────

pub struct ReplaySession<'a> {
    run_id: String,
    store: &'a dyn EventStore,
    events: Vec<StoredEvent>,
    current_step: usize,
    breakpoints: Vec<usize>,
}

impl<'a> ReplaySession<'a> {
    pub fn new(run_id: &str, store: &'a dyn EventStore) -> Result<Self, ReplayError> {
        let events = store.get_events(run_id)?;
        if events.is_empty() {
            return Err(ReplayError::NoEvents);
        }
        Ok(Self {
            run_id: run_id.to_string(),
            store,
            events,
            current_step: 0,
            breakpoints: Vec::new(),
        })
    }

    pub fn total_steps(&self) -> usize {
        self.events.len()
    }

    pub fn current_step(&self) -> usize {
        self.current_step
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn current_state(&self) -> ReplayState {
        let event = &self.events[self.current_step];
        ReplayState {
            step: self.current_step,
            event: event.event.clone(),
            graph_state: event.state_snapshot.clone(),
            timestamp: event.stored_at,
            has_breakpoint: self.breakpoints.contains(&self.current_step),
        }
    }

    pub fn step_forward(&mut self) -> Result<ReplayState, ReplayError> {
        if self.current_step + 1 >= self.events.len() {
            return Err(ReplayError::StepOutOfBounds {
                step: self.current_step + 1,
                total: self.events.len(),
            });
        }
        self.current_step += 1;
        Ok(self.current_state())
    }

    pub fn step_backward(&mut self) -> Result<ReplayState, ReplayError> {
        if self.current_step == 0 {
            return Err(ReplayError::StepOutOfBounds {
                step: 0,
                total: self.events.len(),
            });
        }
        self.current_step -= 1;
        Ok(self.current_state())
    }

    pub fn goto_step(&mut self, step: usize) -> Result<ReplayState, ReplayError> {
        if step >= self.events.len() {
            return Err(ReplayError::StepOutOfBounds {
                step,
                total: self.events.len(),
            });
        }
        self.current_step = step;
        Ok(self.current_state())
    }

    pub fn set_breakpoint(&mut self, step: usize) -> Result<(), ReplayError> {
        if step >= self.events.len() {
            return Err(ReplayError::StepOutOfBounds {
                step,
                total: self.events.len(),
            });
        }
        if self.breakpoints.contains(&step) {
            return Err(ReplayError::BreakpointExists(step));
        }
        self.breakpoints.push(step);
        self.breakpoints.sort();
        Ok(())
    }

    pub fn remove_breakpoint(&mut self, step: usize) {
        self.breakpoints.retain(|&s| s != step);
    }

    pub fn breakpoints(&self) -> &[usize] {
        &self.breakpoints
    }

    pub fn continue_to_breakpoint(&mut self) -> Result<Vec<ReplayState>, ReplayError> {
        let mut states = Vec::new();
        loop {
            if self.current_step + 1 >= self.events.len() {
                break;
            }
            let state = self.step_forward()?;
            states.push(state);
            if self.breakpoints.contains(&self.current_step) {
                break;
            }
        }
        Ok(states)
    }

    pub fn diff(&self, from_step: usize, to_step: usize) -> Result<StateDiff, ReplayError> {
        if from_step >= self.events.len() {
            return Err(ReplayError::StepOutOfBounds {
                step: from_step,
                total: self.events.len(),
            });
        }
        if to_step >= self.events.len() {
            return Err(ReplayError::StepOutOfBounds {
                step: to_step,
                total: self.events.len(),
            });
        }

        let from_state = self.events[from_step].state_snapshot.as_ref();
        let to_state = self.events[to_step].state_snapshot.as_ref();

        let (added, removed, modified) = match (from_state, to_state) {
            (Some(from), Some(to)) => diff_graph_states(from, to),
            _ => (Vec::new(), Vec::new(), Vec::new()),
        };

        Ok(StateDiff {
            from_step,
            to_step,
            added,
            removed,
            modified,
        })
    }

    pub fn timeline(&self) -> Vec<TimelineEntry> {
        self.events
            .iter()
            .map(|e| {
                let (node, event_type) = match &e.event {
                    ExecutionEvent::NodeStart { node, .. } => (node.clone(), "start"),
                    ExecutionEvent::NodeComplete { node, .. } => (node.clone(), "complete"),
                    ExecutionEvent::NodeError { node, .. } => (node.clone(), "error"),
                    ExecutionEvent::EdgeTaken { from, to, .. } => {
                        (format!("{from} -> {to}"), "edge")
                    }
                    ExecutionEvent::Interrupted { node, .. } => (node.clone(), "interrupted"),
                    ExecutionEvent::Completed { .. } => ("*".into(), "completed"),
                    ExecutionEvent::Failed { node, .. } => (node.clone(), "failed"),
                    ExecutionEvent::CheckpointSaved { node, .. } => (node.clone(), "checkpoint"),
                    ExecutionEvent::Resumed { node, .. } => (node.clone(), "resumed"),
                    ExecutionEvent::BranchStart { source, .. } => (source.clone(), "branch-start"),
                    ExecutionEvent::BranchComplete { source, .. } => {
                        (source.clone(), "branch-complete")
                    }
                };
                TimelineEntry {
                    seq: e.seq,
                    node,
                    event_type: event_type.to_string(),
                    timestamp: e.stored_at,
                    has_state: e.state_snapshot.is_some(),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub seq: usize,
    pub node: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub has_state: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn diff_graph_states(
    from: &GraphState,
    to: &GraphState,
) -> (Vec<String>, Vec<String>, Vec<(String, String, String)>) {
    let from_keys: std::collections::HashSet<String> = from.channels.keys().cloned().collect();
    let to_keys: std::collections::HashSet<String> = to.channels.keys().cloned().collect();

    let added: Vec<String> = to_keys.difference(&from_keys).cloned().collect();
    let removed: Vec<String> = from_keys.difference(&to_keys).cloned().collect();

    let mut modified = Vec::new();
    for key in from_keys.intersection(&to_keys) {
        let old_val = format!("{:?}", from.channels.get(key));
        let new_val = format!("{:?}", to.channels.get(key));
        if old_val != new_val {
            modified.push((key.clone(), old_val, new_val));
        }
    }

    (added, removed, modified)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(step: usize) -> ExecutionEvent {
        ExecutionEvent::NodeStart {
            node: format!("node-{step}"),
            step,
            timestamp_ms: step as i64 * 1000,
        }
    }

    fn make_complete_event(steps: usize) -> ExecutionEvent {
        ExecutionEvent::Completed {
            total_steps: steps,
            total_duration_ms: steps as u64 * 1000,
        }
    }

    #[test]
    fn test_event_store_append_and_get() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();
        store.append("run-1", make_event(1)).unwrap();
        store.append("run-1", make_complete_event(2)).unwrap();

        let events = store.get_events("run-1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
        assert_eq!(events[2].seq, 2);
    }

    #[test]
    fn test_event_store_range() {
        let store = InMemoryEventStore::new();
        for i in 0..10 {
            store.append("run-1", make_event(i)).unwrap();
        }

        let range = store.get_events_range("run-1", 3, 6).unwrap();
        assert_eq!(range.len(), 4);
        assert_eq!(range[0].seq, 3);
        assert_eq!(range[3].seq, 6);
    }

    #[test]
    fn test_event_store_count() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();
        store.append("run-1", make_event(1)).unwrap();

        assert_eq!(store.event_count("run-1").unwrap(), 2);
        assert_eq!(store.list_runs().unwrap().len(), 1);
    }

    #[test]
    fn test_event_store_run_not_found() {
        let store = InMemoryEventStore::new();
        assert!(store.get_events("nonexistent").is_err());
        assert!(store.event_count("nonexistent").is_err());
    }

    #[test]
    fn test_replay_session_navigation() {
        let store = InMemoryEventStore::new();
        for i in 0..5 {
            store.append("run-1", make_event(i)).unwrap();
        }
        store.append("run-1", make_complete_event(5)).unwrap();

        let mut session = ReplaySession::new("run-1", &store).unwrap();
        assert_eq!(session.total_steps(), 6);
        assert_eq!(session.current_step(), 0);

        let state = session.step_forward().unwrap();
        assert_eq!(state.step, 1);

        let state = session.step_forward().unwrap();
        assert_eq!(state.step, 2);

        let state = session.step_backward().unwrap();
        assert_eq!(state.step, 1);

        let state = session.goto_step(4).unwrap();
        assert_eq!(state.step, 4);
    }

    #[test]
    fn test_replay_out_of_bounds() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();

        let mut session = ReplaySession::new("run-1", &store).unwrap();
        assert!(session.step_backward().is_err());
        assert!(session.step_forward().is_err());
        assert!(session.goto_step(5).is_err());
    }

    #[test]
    fn test_breakpoints() {
        let store = InMemoryEventStore::new();
        for i in 0..10 {
            store.append("run-1", make_event(i)).unwrap();
        }

        let mut session = ReplaySession::new("run-1", &store).unwrap();

        session.set_breakpoint(3).unwrap();
        session.set_breakpoint(7).unwrap();
        assert_eq!(session.breakpoints(), &[3, 7]);

        assert!(session.set_breakpoint(3).is_err());

        let states = session.continue_to_breakpoint().unwrap();
        assert_eq!(states.len(), 3);
        assert_eq!(session.current_step(), 3);

        session.remove_breakpoint(7);
        assert_eq!(session.breakpoints(), &[3]);
    }

    #[test]
    fn test_timeline() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();
        store.append("run-1", make_event(1)).unwrap();
        store.append("run-1", make_complete_event(2)).unwrap();

        let session = ReplaySession::new("run-1", &store).unwrap();
        let timeline = session.timeline();
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].event_type, "start");
        assert_eq!(timeline[1].event_type, "start");
        assert_eq!(timeline[2].event_type, "completed");
    }

    #[test]
    fn test_no_events_error() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();
        assert!(ReplaySession::new("run-empty", &store).is_err());
    }

    #[test]
    fn test_multiple_runs() {
        let store = InMemoryEventStore::new();
        store.append("run-1", make_event(0)).unwrap();
        store.append("run-2", make_event(0)).unwrap();
        store.append("run-2", make_event(1)).unwrap();

        assert_eq!(store.event_count("run-1").unwrap(), 1);
        assert_eq!(store.event_count("run-2").unwrap(), 2);
        assert_eq!(store.list_runs().unwrap().len(), 2);
    }
}
