//! Scheduler Performance Benchmarks
//!
//! Measures single-agent and batch scheduling latency across all 4 algorithms
//! (round-robin, least-loaded, resource-aware, cache-aware) with varying cluster
//! and workload sizes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_benchmarks::fixtures;
use kias_common::{Agent, Priority, Resources};
use kias_scheduler::algorithms::{
    AffinityScheduler, GpuAwareScheduler, PriorityAwareScheduler, SchedulingAlgorithm,
};
use kias_scheduler::config::SchedulerConfig;
use kias_scheduler::scheduler::Scheduler;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn scheduler_config(algorithm: &str) -> SchedulerConfig {
    SchedulerConfig {
        algorithm: algorithm.to_string(),
        cache_weight: 0.3,
        preemption_enabled: false,
        ..Default::default()
    }
}

fn make_agent(id: &str) -> Agent {
    Agent {
        id: id.to_string(),
        name: id.to_string(),
        resource_request: Resources {
            cpu: 0.5,
            memory_bytes: 512 * 1024 * 1024,
            gpu: 0,
            ..Default::default()
        },
        priority: Priority::Medium,
        system_prompt_hash: Some(42),
        affinity: None,
        anti_affinity: None,
    }
}

// ── Single-agent scheduling ─────────────────────────────────────────────────

fn bench_single_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/single_agent");
    let nodes = fixtures::make_nodes(10);

    for algo in &[
        "round-robin",
        "least-loaded",
        "resource-aware",
        "cache-aware",
    ] {
        let config = scheduler_config(algo);
        let scheduler = Scheduler::new(config);
        let agent = make_agent("bench-agent");

        group.bench_with_input(BenchmarkId::from_parameter(algo), algo, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                rt.block_on(async { black_box(scheduler.schedule_agent(&agent, &nodes).await) })
            });
        });
    }
    group.finish();
}

// ── Batch scheduling throughput ─────────────────────────────────────────────

fn bench_batch_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/batch");
    let nodes = fixtures::make_nodes(20);

    for batch_size in &[10, 50, 100, 500] {
        let config = scheduler_config("round-robin");
        let scheduler = Scheduler::new(config);
        let agents = fixtures::make_agents(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("round-robin", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let mut agents = agents.clone();
                    rt.block_on(async {
                        black_box(scheduler.schedule_batch(&mut agents, &nodes).await)
                    });
                });
            },
        );
    }

    for batch_size in &[10, 50, 100, 500] {
        let config = scheduler_config("least-loaded");
        let scheduler = Scheduler::new(config);
        let agents = fixtures::make_agents(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("least-loaded", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let mut agents = agents.clone();
                    rt.block_on(async {
                        black_box(scheduler.schedule_batch(&mut agents, &nodes).await)
                    });
                });
            },
        );
    }

    for batch_size in &[10, 50, 100, 500] {
        let config = scheduler_config("cache-aware");
        let scheduler = Scheduler::new(config);
        let agents = fixtures::make_agents(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("cache-aware", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let mut agents = agents.clone();
                    rt.block_on(async {
                        black_box(scheduler.schedule_batch(&mut agents, &nodes).await)
                    });
                });
            },
        );
    }

    group.finish();
}

// ── Affinity filtering ──────────────────────────────────────────────────────

fn bench_affinity(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/affinity");
    let nodes = fixtures::make_heterogeneous_nodes(20);

    for batch_size in &[50, 200] {
        let config = scheduler_config("round-robin");
        let scheduler = Scheduler::new(config);
        let agents = fixtures::make_agents_with_affinity(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("with_affinity", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let mut agents = agents.clone();
                    rt.block_on(async {
                        black_box(scheduler.schedule_batch(&mut agents, &nodes).await)
                    });
                });
            },
        );
    }
    group.finish();
}

// ── Cluster scaling ─────────────────────────────────────────────────────────

fn bench_cluster_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/cluster_scaling");
    let agent = make_agent("scaling-agent");

    for node_count in &[5, 20, 50, 100] {
        let config = scheduler_config("resource-aware");
        let scheduler = Scheduler::new(config);
        let nodes = fixtures::make_nodes(*node_count);

        group.bench_with_input(
            BenchmarkId::new("resource-aware", node_count),
            node_count,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let _ = rt.block_on(async {
                        black_box(scheduler.schedule_agent(&agent, &nodes).await)
                    });
                });
            },
        );
    }
    group.finish();
}

// ── Affinity algorithm (direct) ────────────────────────────────────────────

fn bench_affinity_algorithm(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/affinity_algorithm");
    let nodes = fixtures::make_zone_nodes(15);

    for batch_size in &[10, 50, 100] {
        let scheduler = AffinityScheduler::new();
        let agents = fixtures::make_agents_with_affinity(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("zone_spread", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    for agent in &agents {
                        let _ = rt.block_on(async {
                            black_box(scheduler.schedule(agent, &nodes).await)
                        });
                    }
                });
            },
        );
    }
    group.finish();
}

// ── Priority-aware scheduling ──────────────────────────────────────────────

fn bench_priority_aware(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/priority_aware");
    let nodes = fixtures::make_nodes(20);

    for batch_size in &[10, 50, 100, 500] {
        let scheduler = PriorityAwareScheduler::new();
        let agents = fixtures::make_agents(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("priority_aware", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    for agent in &agents {
                        let _ = rt.block_on(async {
                            black_box(scheduler.schedule(agent, &nodes).await)
                        });
                    }
                });
            },
        );
    }
    group.finish();
}

// ── GPU-aware scheduling ───────────────────────────────────────────────────

fn bench_gpu_aware(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler/gpu_aware");
    let nodes = fixtures::make_gpu_nodes(20);

    for batch_size in &[10, 50, 100] {
        let scheduler = GpuAwareScheduler::new();
        let agents = fixtures::make_gpu_agents(*batch_size);

        group.bench_with_input(
            BenchmarkId::new("gpu_aware", batch_size),
            batch_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    for agent in &agents {
                        let _ = rt.block_on(async {
                            black_box(scheduler.schedule(agent, &nodes).await)
                        });
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_single_agent,
    bench_batch_scheduling,
    bench_affinity,
    bench_cluster_scaling,
    bench_affinity_algorithm,
    bench_priority_aware,
    bench_gpu_aware,
);
criterion_main!(benches);
