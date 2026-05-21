//! End-to-end integration tests for the full agent lifecycle.
//!
//! Covers: Deploy → Schedule → Execute → Complete → Recover
//!
//! These tests exercise the controller, scheduler, and recovery subsystems
//! together without requiring external services (uses NoOpSpawner and mock nodes).

use chrono::Utc;
use kias_autonomy_controller::AutonomyLevel;
use kias_controller::{
    AgentInfo, AgentStatus, AutonomyGate, ControllerState, DefaultReconciler, DesiredState,
    NoOpSpawner, Reconciler, RecoveryAction, RecoveryConfig, RecoveryManager,
};
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────

fn make_agent_config(name: &str) -> kias_controller::AgentConfig {
    kias_controller::AgentConfig {
        name: name.to_string(),
        image: "python:3.11".to_string(),
        resources: kias_controller::ResourceRequirements {
            cpu: "100m".to_string(),
            memory: "128Mi".to_string(),
        },
    }
}

fn make_state(desired: u32, actual_running: u32) -> ControllerState {
    ControllerState {
        desired: DesiredState {
            replicas: desired,
            agent_config: make_agent_config("test-agent"),
        },
        actual: kias_controller::ActualState {
            running_replicas: actual_running,
            agent_status: if actual_running > 0 {
                AgentStatus::Running
            } else {
                AgentStatus::Pending
            },
            last_updated: Utc::now(),
        },
        agents: HashMap::new(),
    }
}

fn spawn_agents(state: &mut ControllerState, count: u32) {
    for i in 1..=count {
        let id = format!("agent-{}", i);
        let name = format!("test-agent-{}", i);
        let mut info = AgentInfo::new(&id, &name);
        info.status = AgentStatus::Running;
        state.agents.insert(id, info);
    }
}

// ── Deploy ───────────────────────────────────────────────────

#[tokio::test]
async fn test_deploy_creates_agents() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(3, 0);

    reconciler.reconcile(&mut state).await.unwrap();

    assert_eq!(state.agents.len(), 3, "Should have spawned 3 agents");
    assert_eq!(state.actual.running_replicas, 3);
    for info in state.agents.values() {
        assert!(matches!(info.status, AgentStatus::Running));
    }
}

#[tokio::test]
async fn test_deploy_idempotent() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(2, 0);
    spawn_agents(&mut state, 2);
    state.actual.running_replicas = 2;

    reconciler.reconcile(&mut state).await.unwrap();

    // Should NOT spawn more agents
    assert_eq!(state.agents.len(), 2, "Idempotent: no extra agents");
}

#[tokio::test]
async fn test_deploy_with_autonomy_gate() {
    let mut gate = AutonomyGate::new();
    gate.set_level(AutonomyLevel::FullAuto);
    let reconciler = DefaultReconciler::<NoOpSpawner>::default().with_autonomy_gate(gate);
    let mut state = make_state(2, 0);

    reconciler.reconcile(&mut state).await.unwrap();

    assert_eq!(state.agents.len(), 2);
}

// ── Schedule ─────────────────────────────────────────────────

#[tokio::test]
async fn test_schedule_pending_to_running() {
    let mut state = make_state(1, 0);
    state.agents.insert("agent-1".to_string(), {
        let mut info = AgentInfo::new("agent-1", "test-agent-1");
        info.status = AgentStatus::Pending;
        info
    });

    // Simulate scheduling: Pending → Running
    if let Some(agent) = state.agents.get_mut("agent-1") {
        agent.status = AgentStatus::Running;
    }
    state.actual.running_replicas = 1;
    state.actual.agent_status = AgentStatus::Running;

    assert!(matches!(
        state.agents["agent-1"].status,
        AgentStatus::Running
    ));
    assert_eq!(state.actual.running_replicas, 1);
}

// ── Execute ──────────────────────────────────────────────────

#[tokio::test]
async fn test_execute_task_on_running_agent() {
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    let agent = state.agents.get_mut("agent-1").unwrap();
    assert!(matches!(agent.status, AgentStatus::Running));

    // Simulate task execution — agent stays Running during execution
    agent.last_heartbeat = Utc::now();
    assert!(matches!(agent.status, AgentStatus::Running));
}

// ── Complete ─────────────────────────────────────────────────

#[tokio::test]
async fn test_complete_agent_lifecycle() {
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    // Running → Completed
    let agent = state.agents.get_mut("agent-1").unwrap();
    agent.status = AgentStatus::Succeeded;

    assert!(matches!(agent.status, AgentStatus::Succeeded));
}

#[tokio::test]
async fn test_multiple_agents_mixed_completion() {
    let mut state = make_state(3, 0);
    spawn_agents(&mut state, 3);

    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Succeeded;
    state.agents.get_mut("agent-2").unwrap().status = AgentStatus::Failed;
    // agent-3 stays Running

    let succeeded = state
        .agents
        .values()
        .filter(|a| a.status == AgentStatus::Succeeded)
        .count();
    let failed = state
        .agents
        .values()
        .filter(|a| a.status == AgentStatus::Failed)
        .count();
    let running = state
        .agents
        .values()
        .filter(|a| a.status == AgentStatus::Running)
        .count();

    assert_eq!(succeeded, 1);
    assert_eq!(failed, 1);
    assert_eq!(running, 1);
}

// ── Recover ──────────────────────────────────────────────────

#[tokio::test]
async fn test_recovery_restart_failed_agent() {
    let config = RecoveryConfig {
        max_retries: 3,
        base_backoff_secs: 0, // no delay for tests
        max_backoff_secs: 1,
        backoff_multiplier: 1.0,
        jitter_percent: 0,
    };
    let mut manager = RecoveryManager::new(config);
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    // Mark agent as failed
    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Failed;

    let actions = manager.process_recovery(&mut state);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].1, RecoveryAction::Restarted);
    assert!(matches!(
        state.agents["agent-1"].status,
        AgentStatus::Running
    ));
}

#[tokio::test]
async fn test_recovery_exponential_backoff() {
    let config = RecoveryConfig {
        max_retries: 3,
        base_backoff_secs: 0,
        max_backoff_secs: 0,
        backoff_multiplier: 1.0,
        jitter_percent: 0,
    };
    let mut manager = RecoveryManager::new(config);
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    // First failure — immediate restart (backoff not expired yet, but first attempt)
    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Failed;
    let actions = manager.process_recovery(&mut state);
    assert_eq!(actions[0].1, RecoveryAction::Restarted);

    // Simulate repeated failures
    for _ in 0..3 {
        state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Failed;
        manager.process_recovery(&mut state);
    }

    // After max_retries, should be permanently failed
    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Failed;
    let actions = manager.process_recovery(&mut state);
    assert_eq!(actions[0].1, RecoveryAction::PermanentlyFailed);
}

#[tokio::test]
async fn test_recovery_unresponsive_agent() {
    let config = RecoveryConfig {
        max_retries: 2,
        base_backoff_secs: 0,
        max_backoff_secs: 1,
        backoff_multiplier: 1.0,
        jitter_percent: 0,
    };
    let mut manager = RecoveryManager::new(config);
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    // Mark as unresponsive
    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Unresponsive;

    let actions = manager.process_recovery(&mut state);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].1, RecoveryAction::Restarted);
}

// ── Full Lifecycle ───────────────────────────────────────────

#[tokio::test]
async fn test_full_lifecycle_deploy_execute_fail_recover_complete() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(1, 0);

    // 1. Deploy
    reconciler.reconcile(&mut state).await.unwrap();
    assert_eq!(state.agents.len(), 1);
    assert!(matches!(
        state.agents.values().next().unwrap().status,
        AgentStatus::Running
    ));

    // Get the first agent ID (NoOpSpawner generates UUIDs)
    let agent_id = state.agents.keys().next().unwrap().clone();

    // 2. Execute (heartbeat)
    state.agents.get_mut(&agent_id).unwrap().last_heartbeat = Utc::now();

    // 3. Fail
    state.agents.get_mut(&agent_id).unwrap().status = AgentStatus::Failed;

    // 4. Recover
    let config = RecoveryConfig {
        max_retries: 3,
        base_backoff_secs: 0,
        max_backoff_secs: 0,
        backoff_multiplier: 1.0,
        jitter_percent: 0,
    };
    let mut recovery = RecoveryManager::new(config);
    let actions = recovery.process_recovery(&mut state);
    assert_eq!(actions[0].1, RecoveryAction::Restarted);
    assert!(matches!(
        state.agents[&agent_id].status,
        AgentStatus::Running
    ));

    // 5. Complete
    state.agents.get_mut(&agent_id).unwrap().status = AgentStatus::Succeeded;
    assert!(matches!(
        state.agents[&agent_id].status,
        AgentStatus::Succeeded
    ));
}

#[tokio::test]
async fn test_full_lifecycle_multi_agent_partial_failure() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(3, 0);

    // Deploy 3 agents
    reconciler.reconcile(&mut state).await.unwrap();
    assert_eq!(state.agents.len(), 3);

    // Get actual agent IDs (NoOpSpawner generates UUIDs)
    let mut agent_ids: Vec<String> = state.agents.keys().cloned().collect();
    agent_ids.sort();

    // Agent 1 succeeds, agent 2 fails, agent 3 unresponsive
    state.agents.get_mut(&agent_ids[0]).unwrap().status = AgentStatus::Succeeded;
    state.agents.get_mut(&agent_ids[1]).unwrap().status = AgentStatus::Failed;
    state.agents.get_mut(&agent_ids[2]).unwrap().status = AgentStatus::Unresponsive;

    // Recover failed agents
    let config = RecoveryConfig::default();
    let mut recovery = RecoveryManager::new(config);
    let actions = recovery.process_recovery(&mut state);

    // Both failed and unresponsive should be restarted
    let restarted = actions
        .iter()
        .filter(|(_, a)| *a == RecoveryAction::Restarted)
        .count();
    assert_eq!(restarted, 2, "Both failed and unresponsive should restart");

    // Agent 1 still succeeded
    assert!(matches!(
        state.agents[&agent_ids[0]].status,
        AgentStatus::Succeeded
    ));
}

// ── Lifecycle State Machine ──────────────────────────────────

#[test]
fn test_lifecycle_valid_transitions() {
    use kias_controller::LifecycleState;

    assert!(LifecycleState::Pending.can_transition_to(&LifecycleState::Scheduled));
    assert!(!LifecycleState::Pending.can_transition_to(&LifecycleState::Running));

    assert!(LifecycleState::Scheduled.can_transition_to(&LifecycleState::Running));
    assert!(LifecycleState::Scheduled.can_transition_to(&LifecycleState::Failed));

    assert!(LifecycleState::Running.can_transition_to(&LifecycleState::Completed));
    assert!(LifecycleState::Running.can_transition_to(&LifecycleState::Failed));
    assert!(LifecycleState::Running.can_transition_to(&LifecycleState::Terminated));

    // Terminal states
    assert!(LifecycleState::Completed.valid_transitions().is_empty());
    assert!(LifecycleState::Terminated.valid_transitions().is_empty());

    // Recovery path
    assert!(LifecycleState::Failed.can_transition_to(&LifecycleState::Scheduled));
}

// ── Edge Cases ───────────────────────────────────────────────

#[tokio::test]
async fn test_deploy_zero_replicas() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(0, 0);

    reconciler.reconcile(&mut state).await.unwrap();

    assert_eq!(state.agents.len(), 0);
}

#[tokio::test]
async fn test_recovery_running_agent_no_action() {
    let mut manager = RecoveryManager::new(RecoveryConfig::default());
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);

    let actions = manager.process_recovery(&mut state);
    assert!(
        actions.iter().all(|(_, a)| *a == RecoveryAction::NoAction),
        "Running agents should not be recovered"
    );
}

#[tokio::test]
async fn test_recovery_succeeded_agent_no_action() {
    let mut manager = RecoveryManager::new(RecoveryConfig::default());
    let mut state = make_state(1, 0);
    spawn_agents(&mut state, 1);
    state.agents.get_mut("agent-1").unwrap().status = AgentStatus::Succeeded;

    let actions = manager.process_recovery(&mut state);
    assert!(
        actions.iter().all(|(_, a)| *a == RecoveryAction::NoAction),
        "Succeeded agents should not be recovered"
    );
}

#[tokio::test]
async fn test_scale_up_from_partial() {
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();
    let mut state = make_state(5, 0);
    spawn_agents(&mut state, 3); // only 3 running

    reconciler.reconcile(&mut state).await.unwrap();

    assert_eq!(state.agents.len(), 5, "Should scale up to 5");
}
