//! Concurrent Scheduling Stress Test
//!
//! Simulates high-concurrency scheduling scenarios:
//! - N tasks scheduling agents in parallel onto shared nodes
//! - Measures throughput (agents/sec) and contention under load
//! - Tests cache-aware scheduling with overlapping prompt hashes

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_benchmarks::fixtures;
use kias_common::{Agent, Priority, Resources};
use kias_scheduler::config::SchedulerConfig;
use kias_scheduler::scheduler::Scheduler;
use std::sync::Arc;

fn scheduler_config(algorithm: &str) -> SchedulerConfig {
    SchedulerConfig {
        algorithm: algorithm.to_string(),
        cache_weight: 0.3,
        preemption_enabled: false,
        ..Default::default()
    }
}

// ── Parallel single-agent scheduling ────────────────────────────────────────

fn bench_parallel_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress/parallel_schedule");
    group.sample_size(50);

    for concurrency in &[10, 50, 100, 500] {
        let config = scheduler_config("round-robin");
        let scheduler = Arc::new(Scheduler::new(config));
        let nodes = Arc::new(fixtures::make_nodes(20));
        let agents: Vec<Agent> = (0..*concurrency)
            .map(|i| Agent {
                id: format!("stress-agent-{}", i),
                name: format!("stress-agent-{}", i),
                resource_request: Resources {
                    cpu: 0.25,
                    memory_bytes: 256 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                priority: Priority::Medium,
                system_prompt_hash: Some((i as u64) % 5),
                affinity: None,
                anti_affinity: None,
            tenant_id: None,
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("concurrent", concurrency),
            concurrency,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let scheduler = Arc::clone(&scheduler);
                    let nodes = Arc::clone(&nodes);
                    let agents = agents.clone();

                    rt.block_on(async move {
                        let mut handles = Vec::with_capacity(agents.len());
                        for agent in agents {
                            let sched = Arc::clone(&scheduler);
                            let nodes = Arc::clone(&nodes);
                            handles.push(tokio::spawn(async move {
                                sched.schedule_agent(&agent, &nodes).await
                            }));
                        }
                        let mut successes = 0u32;
                        for handle in handles {
                            if let Ok(Ok(_)) = handle.await {
                                successes += 1;
                            }
                        }
                        black_box(successes)
                    });
                });
            },
        );
    }
    group.finish();
}

// ── Cache-aware contention ──────────────────────────────────────────────────

fn bench_cache_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress/cache_contention");
    group.sample_size(50);

    // Agents with identical prompt hashes to stress cache path
    for concurrency in &[20, 100, 500] {
        let config = scheduler_config("cache-aware");
        let scheduler = Arc::new(Scheduler::new(config));
        let nodes = Arc::new(fixtures::make_nodes(10));

        // All agents share same prompt hash → high cache contention
        let agents: Vec<Agent> = (0..*concurrency)
            .map(|i| Agent {
                id: format!("cache-agent-{}", i),
                name: format!("cache-agent-{}", i),
                resource_request: Resources {
                    cpu: 0.25,
                    memory_bytes: 128 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                priority: Priority::Medium,
                system_prompt_hash: Some(42), // Same hash for all
                affinity: None,
                anti_affinity: None,
            tenant_id: None,
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("same_hash", concurrency),
            concurrency,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                b.iter(|| {
                    let scheduler = Arc::clone(&scheduler);
                    let nodes = Arc::clone(&nodes);
                    let agents = agents.clone();

                    rt.block_on(async move {
                        let mut handles = Vec::with_capacity(agents.len());
                        for agent in agents {
                            let sched = Arc::clone(&scheduler);
                            let nodes = Arc::clone(&nodes);
                            handles.push(tokio::spawn(async move {
                                sched.schedule_agent(&agent, &nodes).await
                            }));
                        }
                        let mut successes = 0u32;
                        for handle in handles {
                            if let Ok(Ok(_)) = handle.await {
                                successes += 1;
                            }
                        }
                        black_box(successes)
                    });
                });
            },
        );
    }
    group.finish();
}

// ── Mixed workload stress ───────────────────────────────────────────────────

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress/mixed_workload");
    group.sample_size(30);

    // Simulate realistic mixed workload: different priorities, affinities, sizes
    let config = scheduler_config("least-loaded");
    let scheduler = Arc::new(Scheduler::new(config));
    let nodes = Arc::new(fixtures::make_heterogeneous_nodes(30));

    let agents: Vec<Agent> = (0..1000)
        .map(|i| {
            let priority = match i % 5 {
                0 => Priority::Critical,
                1 => Priority::High,
                2 | 3 => Priority::Medium,
                _ => Priority::Low,
            };
            let cpu = 0.1 + ((i as f64 * 0.003) % 4.0);
            let mem = (128 + ((i * 37) % 2048)) as u64 * 1024 * 1024;
            Agent {
                id: format!("mixed-{}", i),
                name: format!("mixed-{}", i),
                resource_request: Resources {
                    cpu,
                    memory_bytes: mem,
                    gpu: if i % 10 == 0 { 1 } else { 0 },
                    ..Default::default()
                },
                priority,
                system_prompt_hash: Some((i as u64) % 20),
                affinity: None,
                anti_affinity: None,
            tenant_id: None,
            }
        })
        .collect();

    group.bench_function("1000_agents_mixed", |b| {
        let rt = tokio::runtime::Runtime::new().expect("rt");
        b.iter(|| {
            let scheduler = Arc::clone(&scheduler);
            let nodes = Arc::clone(&nodes);
            let agents = agents.clone();

            rt.block_on(async move {
                let mut handles = Vec::with_capacity(agents.len());
                for agent in agents {
                    let sched = Arc::clone(&scheduler);
                    let nodes = Arc::clone(&nodes);
                    handles.push(tokio::spawn(async move {
                        sched.schedule_agent(&agent, &nodes).await
                    }));
                }
                let mut successes = 0u32;
                for handle in handles {
                    if let Ok(Ok(_)) = handle.await {
                        successes += 1;
                    }
                }
                black_box(successes)
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parallel_scheduling,
    bench_cache_contention,
    bench_mixed_workload,
);
criterion_main!(benches);
