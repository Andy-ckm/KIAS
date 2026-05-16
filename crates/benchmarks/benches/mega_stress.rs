//! Mega Stress Test — 万级并发验证
//!
//! 验证 KIAS 各核心模块在高并发下的真实表现。
//! 测量指标：吞吐量 (ops/sec)、总耗时。
//!
//! 运行方式：
//!   cargo bench --bench mega_stress
//!
//! 硬件基线：4 vCPU / 3.6 GB RAM（当前测试机）
//! 注：本机受限于硬件，仅验证至数千级并发。万级及以上数据引用自同类项目基准。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_benchmarks::fixtures;
use kias_common::{Agent, Priority, Resources};
use kias_langgraph_engine::checkpoint::{CheckpointStore, InMemoryCheckpointStore};
use kias_langgraph_engine::graph::StateGraph;
use kias_langgraph_engine::state::GraphState;
use kias_scheduler::config::SchedulerConfig;
use kias_scheduler::scheduler::Scheduler;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Scheduler 并发调度
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_scheduler_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/scheduler");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000] {
        let config = SchedulerConfig {
            algorithm: "round-robin".to_string(),
            cache_weight: 0.3,
            preemption_enabled: false,
            ..Default::default()
        };
        let scheduler = Arc::new(Scheduler::new(config));
        let node_count = (*concurrency / 50).max(10);
        let nodes = Arc::new(fixtures::make_nodes(node_count));

        let agents: Vec<Agent> = (0..*concurrency)
            .map(|i| Agent {
                id: format!("mega-{}", i),
                name: format!("mega-{}", i),
                resource_request: Resources {
                    cpu: 0.25,
                    memory_bytes: 128 * 1024 * 1024,
                    gpu: 0,
                    ..Default::default()
                },
                priority: match i % 4 {
                    0 => Priority::Critical,
                    1 => Priority::High,
                    2 => Priority::Medium,
                    _ => Priority::Low,
                },
                system_prompt_hash: Some((i as u64) % 20),
                affinity: None,
                anti_affinity: None,
                tenant_id: None,
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("concurrent", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let scheduler = Arc::clone(&scheduler);
                        let nodes = Arc::clone(&nodes);
                        let agents = agents.clone();
                        let start = Instant::now();
                        rt.block_on(async move {
                            let handles: Vec<_> = agents
                                .into_iter()
                                .map(|agent| {
                                    let s = Arc::clone(&scheduler);
                                    let n = Arc::clone(&nodes);
                                    tokio::spawn(async move { s.schedule_agent(&agent, &n).await })
                                })
                                .collect();
                            for h in handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. LangGraph 并发图执行
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_langgraph_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/langgraph");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000] {
        group.bench_with_input(
            BenchmarkId::new("graph_execute", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        rt.block_on(async move {
                            let handles: Vec<_> = (0.._conc)
                                .map(|i| {
                                    tokio::spawn(async move {
                                        let graph = StateGraph::builder("start")
                                            .add_node("start", |mut state: GraphState| async move {
                                                state.channels.insert(
                                                    "processed".to_string(),
                                                    serde_json::Value::Bool(true),
                                                );
                                                Ok(state)
                                            })
                                            .add_node("end", |state: GraphState| async move {
                                                Ok(state)
                                            })
                                            .add_edge("start", "end")
                                            .build_unchecked();

                                        let mut state = GraphState::new();
                                        state
                                            .channels
                                            .insert("task_id".to_string(), serde_json::json!(i));
                                        graph.execute(state).await
                                    })
                                })
                                .collect();

                            for h in handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Checkpoint 并发读写
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_checkpoint_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/checkpoint");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000] {
        group.bench_with_input(
            BenchmarkId::new("save_load", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let store = Arc::new(InMemoryCheckpointStore::new());
                        let start = Instant::now();
                        rt.block_on(async move {
                            let mut handles = Vec::with_capacity(_conc);
                            for i in 0.._conc {
                                let s = Arc::clone(&store);
                                handles.push(tokio::spawn(async move {
                                    let mut state = GraphState::new();
                                    state
                                        .channels
                                        .insert("idx".to_string(), serde_json::json!(i));
                                    let cp = kias_langgraph_engine::checkpoint::Checkpoint {
                                        id: format!("cp-{}", i),
                                        run_id: format!("run-{}", i % 100),
                                        node: "test".to_string(),
                                        state,
                                        timestamp: chrono::Utc::now(),
                                        version: 1,
                                    };
                                    s.save(cp).await
                                }));
                            }
                            for h in handles {
                                let _ = h.await;
                            }

                            let mut read_handles = Vec::with_capacity(_conc);
                            for i in 0.._conc {
                                let s = Arc::clone(&store);
                                read_handles.push(tokio::spawn(async move {
                                    s.load_by_id(&format!("cp-{}", i)).await
                                }));
                            }
                            for h in read_handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Broadcast 并发监听
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_broadcast_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/broadcast");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000] {
        group.bench_with_input(
            BenchmarkId::new("broadcast_recv", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        rt.block_on(async move {
                            let (tx, _) = tokio::sync::broadcast::channel::<u64>(_conc * 2);
                            let received = Arc::new(AtomicU64::new(0));

                            let mut recv_handles = Vec::with_capacity(_conc);
                            for _i in 0.._conc {
                                let mut rx = tx.subscribe();
                                let recv = Arc::clone(&received);
                                recv_handles.push(tokio::spawn(async move {
                                    if rx.recv().await.is_ok() {
                                        recv.fetch_add(1, Ordering::Relaxed);
                                    }
                                }));
                            }

                            let _ = tx.send(42);

                            for h in recv_handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. DashMap 并发读写
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_dashmap_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/dashmap");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_rw", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let map = Arc::new(dashmap::DashMap::<String, u64>::new());
                        let start = Instant::now();
                        rt.block_on(async move {
                            let mut handles = Vec::with_capacity(_conc);
                            for i in 0.._conc {
                                let m = Arc::clone(&map);
                                handles.push(tokio::spawn(async move {
                                    if i % 2 == 0 {
                                        m.insert(format!("key-{}", i), i as u64);
                                    } else {
                                        let _ = m.get(&format!("key-{}", i / 2));
                                    }
                                }));
                            }
                            for h in handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Tokio spawn 并发基线
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_tokio_spawn_mega(c: &mut Criterion) {
    let mut group = c.benchmark_group("mega_stress/tokio_spawn");
    group.sample_size(10);

    for concurrency in &[100, 500, 1_000, 2_000, 5_000] {
        group.bench_with_input(
            BenchmarkId::new("spawn_join", concurrency),
            concurrency,
            |b, &_conc| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("rt");

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        rt.block_on(async move {
                            let handles: Vec<_> = (0.._conc)
                                .map(|i| tokio::spawn(async move { black_box(i * 2) }))
                                .collect();
                            for h in handles {
                                let _ = h.await;
                            }
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_scheduler_mega,
    bench_langgraph_mega,
    bench_checkpoint_mega,
    bench_broadcast_mega,
    bench_dashmap_mega,
    bench_tokio_spawn_mega,
);
criterion_main!(benches);
