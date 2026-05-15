//! Agent Creation & Lifecycle Benchmarks
//!
//! Measures performance of agent creation, task lifecycle, and team coordination
//! across the TeamEngine and Controller reconciler.
//!
//! Tests:
//! - Engine creation with varying worker/verifier counts
//! - Task creation, assignment, completion, verification throughput
//! - Full task lifecycle (assign → complete → verify) throughput
//! - Reconciliation scale-up with NoOpSpawner

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_team_engine::TeamEngine;

// ── Engine creation ──────────────────────────────────────────────────────────

fn bench_engine_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/engine_creation");

    group.bench_function("new_engine", |b| {
        b.iter(|| {
            black_box(TeamEngine::new("owner"));
        });
    });

    group.finish();
}

// ── Worker/Verifier registration ─────────────────────────────────────────────

fn bench_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/registration");

    for count in &[1, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("add_workers", count), count, |b, &n| {
            b.iter(|| {
                let mut engine = TeamEngine::new("owner");
                for i in 0..n {
                    engine.add_worker(&format!("worker-{}", i));
                }
                black_box(engine.get_state().workers.len());
            });
        });
    }

    for count in &[1, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("add_verifiers", count), count, |b, &n| {
            b.iter(|| {
                let mut engine = TeamEngine::new("owner");
                for i in 0..n {
                    engine.add_verifier(&format!("verifier-{}", i));
                }
                black_box(engine.get_state().verifiers.len());
            });
        });
    }

    group.finish();
}

// ── Task creation throughput ─────────────────────────────────────────────────

fn bench_task_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/task_creation");

    for count in &[10, 100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &n| {
            b.iter(|| {
                let mut engine = TeamEngine::new("owner");
                for i in 0..n {
                    black_box(engine.create_task(&format!("task-{}", i), "benchmark task"));
                }
            });
        });
    }

    group.finish();
}

// ── Task assignment throughput ───────────────────────────────────────────────

fn bench_task_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/task_assignment");

    for count in &[10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &n| {
            b.iter(|| {
                let mut engine = TeamEngine::new("owner");
                let mut worker_ids = Vec::new();
                for i in 0..n {
                    worker_ids.push(engine.add_worker(&format!("w-{}", i)));
                }
                let mut task_ids = Vec::new();
                for i in 0..n {
                    task_ids.push(engine.create_task(&format!("t-{}", i), "bench"));
                }
                for (task_id, worker_id) in task_ids.iter().zip(worker_ids.iter()) {
                    black_box(engine.assign_task(task_id, worker_id).unwrap());
                }
            });
        });
    }

    group.finish();
}

// ── Full task lifecycle throughput ────────────────────────────────────────────

fn bench_task_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/task_lifecycle");

    for count in &[10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &n| {
            b.iter(|| {
                let mut engine = TeamEngine::new("owner");
                let worker_id = engine.add_worker("worker");
                let verifier_id = engine.add_verifier("verifier");

                for i in 0..n {
                    let task_id = engine.create_task(&format!("t-{}", i), "bench");
                    engine.assign_task(&task_id, &worker_id).unwrap();
                    engine.complete_task(&task_id).unwrap();
                    engine.verify_task(&task_id, &verifier_id, true).unwrap();
                }
                black_box(engine.get_state().tasks.len());
            });
        });
    }

    group.finish();
}

// ── Execute with retry ──────────────────────────────────────────────────────

fn bench_execute_with_retry(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/execute_with_retry");

    // All pass on first try
    group.bench_function("first_try_pass", |b| {
        b.iter(|| {
            let mut engine = TeamEngine::new("owner");
            let worker_id = engine.add_worker("w");
            let verifier_id = engine.add_verifier("v");
            let task_id = engine.create_task("t", "d");
            black_box(
                engine
                    .execute_with_retry(&task_id, &worker_id, &verifier_id, |_| true)
                    .unwrap(),
            );
        });
    });

    // Fail once, pass on retry
    group.bench_function("retry_once", |b| {
        b.iter(|| {
            let mut engine = TeamEngine::new("owner");
            let worker_id = engine.add_worker("w");
            let verifier_id = engine.add_verifier("v");
            let task_id = engine.create_task("t", "d");
            let mut attempt = 0;
            black_box(
                engine
                    .execute_with_retry(&task_id, &worker_id, &verifier_id, |_| {
                        attempt += 1;
                        attempt > 1
                    })
                    .unwrap(),
            );
        });
    });

    group.finish();
}

// ── State query throughput ───────────────────────────────────────────────────

fn bench_state_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent/state_queries");

    for count in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("get_task_status", count),
            count,
            |b, &n| {
                let mut engine = TeamEngine::new("owner");
                let mut task_ids = Vec::new();
                for i in 0..n {
                    task_ids.push(engine.create_task(&format!("t-{}", i), "bench"));
                }
                let mid = task_ids[n / 2].clone();

                b.iter(|| {
                    black_box(engine.get_task_status(&mid));
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("get_state", count), count, |b, &n| {
            let mut engine = TeamEngine::new("owner");
            for i in 0..n {
                engine.add_worker(&format!("w-{}", i));
            }

            b.iter(|| {
                let state = engine.get_state();
                black_box(state.workers.len());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_engine_creation,
    bench_registration,
    bench_task_creation,
    bench_task_assignment,
    bench_task_lifecycle,
    bench_execute_with_retry,
    bench_state_queries,
);
criterion_main!(benches);
