//! Controller State Benchmarks
//!
//! Measures throughput of core controller operations:
//! - State management (count_by_status, sync_running_replicas)
//! - Agent tracking at scale (100–10,000 agents)
//! - Heartbeat timeout detection
//! - Recovery eligibility filtering

use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_controller::reconciler::{DefaultReconciler, NoOpSpawner, Reconciler};
use kias_controller::state::{
    ActualState, AgentConfig, AgentInfo, AgentStatus, ControllerState, DesiredState,
    ResourceRequirements,
};
use std::collections::HashMap;

fn make_controller_state(agent_count: usize) -> ControllerState {
    let mut agents = HashMap::with_capacity(agent_count);
    for i in 0..agent_count {
        let status = match i % 5 {
            0 => AgentStatus::Running,
            1 => AgentStatus::Failed,
            2 => AgentStatus::Pending,
            3 => AgentStatus::Unresponsive,
            _ => AgentStatus::Succeeded,
        };
        let mut info = AgentInfo::new(format!("agent-{}", i), format!("agent-{}", i));
        info.status = status;
        info.retry_count = (i % 4) as u32;
        agents.insert(format!("agent-{}", i), info);
    }

    ControllerState {
        desired: DesiredState {
            replicas: agent_count as u32,
            agent_config: AgentConfig {
                name: "bench".to_string(),
                image: "bench:latest".to_string(),
                resources: ResourceRequirements {
                    cpu: "100m".to_string(),
                    memory: "128Mi".to_string(),
                },
            },
        },
        actual: ActualState {
            running_replicas: 0,
            agent_status: AgentStatus::Pending,
            last_updated: Utc::now(),
        },
        agents,
    }
}

// ── count_by_status ─────────────────────────────────────────────────────────

fn bench_count_by_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/count_by_status");

    for count in &[100, 1_000, 5_000, 10_000] {
        let state = make_controller_state(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                black_box(state.count_by_status(&AgentStatus::Running));
                black_box(state.count_by_status(&AgentStatus::Failed));
            });
        });
    }
    group.finish();
}

// ── agents_with_status ──────────────────────────────────────────────────────

fn bench_agents_with_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/agents_with_status");

    for count in &[100, 1_000, 5_000, 10_000] {
        let state = make_controller_state(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let running = state.agents_with_status(&AgentStatus::Running);
                black_box(running.len());
            });
        });
    }
    group.finish();
}

// ── sync_running_replicas ───────────────────────────────────────────────────

fn bench_sync_running(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/sync_replicas");

    for count in &[100, 1_000, 5_000, 10_000] {
        let mut state = make_controller_state(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                state.sync_running_replicas();
                black_box(state.actual.running_replicas);
            });
        });
    }
    group.finish();
}

// ── Recovery eligibility scan ───────────────────────────────────────────────

fn bench_recovery_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/recovery_scan");

    for count in &[100, 1_000, 5_000, 10_000] {
        let state = make_controller_state(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                // Scan all agents for recovery eligibility (max_retries = 3)
                let recoverable: Vec<&AgentInfo> = state
                    .agents
                    .values()
                    .filter(|a| a.is_recoverable(3))
                    .collect();
                black_box(recoverable.len());
            });
        });
    }
    group.finish();
}

// ── AgentInfo operations ────────────────────────────────────────────────────

fn bench_agent_info_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/agent_info");

    group.bench_function("new", |b| {
        b.iter(|| {
            black_box(AgentInfo::new("a1", "test-agent"));
        });
    });

    group.bench_function("has_exceeded_retries", |b| {
        let mut info = AgentInfo::new("a1", "test");
        info.retry_count = 2;
        b.iter(|| {
            black_box(info.has_exceeded_retries(3));
        });
    });

    group.bench_function("is_recoverable", |b| {
        let mut info = AgentInfo::new("a1", "test");
        info.status = AgentStatus::Failed;
        info.retry_count = 1;
        b.iter(|| {
            black_box(info.is_recoverable(3));
        });
    });

    group.finish();
}

fn make_controller_state_with_running(agent_count: usize) -> ControllerState {
    let mut agents = HashMap::with_capacity(agent_count);
    for i in 0..agent_count {
        let mut info = AgentInfo::new(format!("agent-{}", i), format!("agent-{}", i));
        info.status = AgentStatus::Running;
        agents.insert(format!("agent-{}", i), info);
    }

    ControllerState {
        desired: DesiredState {
            replicas: agent_count as u32,
            agent_config: AgentConfig {
                name: "bench".to_string(),
                image: "bench:latest".to_string(),
                resources: ResourceRequirements {
                    cpu: "100m".to_string(),
                    memory: "128Mi".to_string(),
                },
            },
        },
        actual: ActualState {
            running_replicas: agent_count as u32,
            agent_status: AgentStatus::Running,
            last_updated: Utc::now(),
        },
        agents,
    }
}

// ── Reconciliation loop ───────────────────────────────────────────────────

fn bench_reconcile_scale_up(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/reconcile_scale_up");
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();

    for count in &[5, 20, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                let mut state = make_controller_state(*count);
                // Set desired higher than actual to trigger scale-up
                state.desired.replicas = (*count as u32) + 10;
                rt.block_on(async {
                    black_box(reconciler.reconcile(&mut state).await.unwrap());
                });
            });
        });
    }
    group.finish();
}

fn bench_reconcile_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("controller/reconcile_noop");
    let reconciler = DefaultReconciler::<NoOpSpawner>::default();

    for count in &[50, 500, 1_000, 5_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                // State already at desired → reconcile should be a no-op
                let mut state = make_controller_state_with_running(*count);
                rt.block_on(async {
                    black_box(reconciler.reconcile(&mut state).await.unwrap());
                });
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_count_by_status,
    bench_agents_with_status,
    bench_sync_running,
    bench_recovery_scan,
    bench_agent_info_ops,
    bench_reconcile_scale_up,
    bench_reconcile_noop,
);
criterion_main!(benches);
