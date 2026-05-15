//! Sandbox Execution Benchmarks
//!
//! Measures performance of the sandbox executor and task runtime:
//! - SandboxExecutor creation with default and custom policies
//! - Simple command execution (echo, true, env)
//! - TaskRuntime overhead (create, configure, run)
//! - Parallel task execution with bounded concurrency
//! - Task payload serialization overhead

use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_executor::{HttpExecutor, ShellExecutor};
use kias_executor::{SandboxExecutor, SandboxPolicy, Task, TaskExecutor, TaskResult, TaskRuntime};
use std::collections::HashMap;
use std::time::Duration;

// ── Sandbox creation ─────────────────────────────────────────────────────────

fn bench_sandbox_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/creation");

    group.bench_function("default", |b| {
        b.iter(|| {
            black_box(SandboxExecutor::new());
        });
    });

    group.bench_function("with_policy", |b| {
        let policy = SandboxPolicy {
            timeout: Duration::from_secs(60),
            max_memory_bytes: 1024 * 1024 * 1024,
            max_output_bytes: 2 * 1024 * 1024,
            env_whitelist: vec!["KIAS_".to_string(), "PATH".to_string()],
            capture_stderr: true,
            workdir: None,
            env_vars: HashMap::new(),
        };
        b.iter(|| {
            black_box(SandboxExecutor::with_policy(policy.clone()));
        });
    });

    group.bench_function("policy_default", |b| {
        b.iter(|| {
            black_box(SandboxPolicy::default());
        });
    });

    group.finish();
}

// ── Command execution ────────────────────────────────────────────────────────

fn bench_command_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/command_execution");
    let rt = tokio::runtime::Runtime::new().unwrap();

    let executor = SandboxExecutor::new();

    group.bench_function("echo_hello", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(executor.execute_command("echo hello").await.unwrap());
            });
        });
    });

    group.bench_function("echo_multiline", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(
                    executor
                        .execute_command("echo 'line1'; echo 'line2'; echo 'line3'")
                        .await
                        .unwrap(),
                );
            });
        });
    });

    group.bench_function("true_command", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(executor.execute_command("true").await.unwrap());
            });
        });
    });

    group.bench_function("env_expansion", |b| {
        let policy = SandboxPolicy {
            env_vars: {
                let mut m = HashMap::new();
                m.insert("BENCH_VAR".to_string(), "bench_value".to_string());
                m
            },
            ..Default::default()
        };
        let exec = SandboxExecutor::with_policy(policy);
        b.iter(|| {
            rt.block_on(async {
                black_box(exec.execute_command("echo $BENCH_VAR").await.unwrap());
            });
        });
    });

    group.finish();
}

// ── Sandbox as TaskExecutor ──────────────────────────────────────────────────

fn bench_sandbox_as_task_executor(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/task_executor");
    let rt = tokio::runtime::Runtime::new().unwrap();

    let executor = SandboxExecutor::new();

    group.bench_function("simple_echo", |b| {
        b.iter(|| {
            let task = Task {
                id: "bench-task".to_string(),
                name: "bench".to_string(),
                agent_id: "bench-agent".to_string(),
                payload: serde_json::json!({"command": "echo sandbox-test"}),
                created_at: Utc::now(),
                timeout: Some(Duration::from_secs(10)),
            };
            rt.block_on(async {
                black_box(executor.execute(&task).await.unwrap());
            });
        });
    });

    group.bench_function("no_command_fallback", |b| {
        b.iter(|| {
            let task = Task {
                id: "bench-task".to_string(),
                name: "bench".to_string(),
                agent_id: "bench-agent".to_string(),
                payload: serde_json::json!({}),
                created_at: Utc::now(),
                timeout: None,
            };
            rt.block_on(async {
                black_box(executor.execute(&task).await.unwrap());
            });
        });
    });

    group.finish();
}

// ── Task creation overhead ───────────────────────────────────────────────────

fn bench_task_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/task_creation");

    group.bench_function("new_task", |b| {
        b.iter(|| {
            black_box(Task {
                id: "task-1".to_string(),
                name: "bench".to_string(),
                agent_id: "agent-1".to_string(),
                payload: serde_json::json!({"command": "echo hello"}),
                created_at: Utc::now(),
                timeout: Some(Duration::from_secs(30)),
            });
        });
    });

    group.bench_function("task_serialization", |b| {
        let task = Task {
            id: "task-1".to_string(),
            name: "bench".to_string(),
            agent_id: "agent-1".to_string(),
            payload: serde_json::json!({"command": "echo hello", "extra": {"key": "value"}}),
            created_at: Utc::now(),
            timeout: Some(Duration::from_secs(30)),
        };
        b.iter(|| {
            let json = serde_json::to_string(&task).unwrap();
            black_box(serde_json::from_str::<Task>(&json).unwrap());
        });
    });

    group.finish();
}

// ── TaskRuntime creation ─────────────────────────────────────────────────────

fn bench_task_runtime_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/runtime_creation");

    group.bench_function("no_retries", |b| {
        b.iter(|| {
            let http = HttpExecutor::new();
            black_box(TaskRuntime::new(Box::new(http)));
        });
    });

    group.bench_function("with_retries", |b| {
        b.iter(|| {
            let http = HttpExecutor::new();
            black_box(TaskRuntime::with_retries(Box::new(http), 3));
        });
    });

    group.bench_function("with_timeout", |b| {
        b.iter(|| {
            let http = HttpExecutor::new();
            black_box(TaskRuntime::with_global_timeout(
                Box::new(http),
                Duration::from_secs(30),
            ));
        });
    });

    group.bench_function("with_retries_and_timeout", |b| {
        b.iter(|| {
            let http = HttpExecutor::new();
            black_box(TaskRuntime::with_retries_and_timeout(
                Box::new(http),
                3,
                Duration::from_secs(30),
            ));
        });
    });

    group.bench_function("shell_executor_default", |b| {
        b.iter(|| {
            black_box(ShellExecutor::default());
        });
    });

    group.finish();
}

// ── Parallel task execution ──────────────────────────────────────────────────

fn bench_parallel_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/parallel_execution");
    group.sample_size(50);

    let rt = tokio::runtime::Runtime::new().unwrap();

    for count in &[5, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &n| {
            let executor = SandboxExecutor::new();
            b.iter(|| {
                rt.block_on(async {
                    for _ in 0..n {
                        black_box(
                            executor
                                .execute_command("echo parallel-test")
                                .await
                                .unwrap(),
                        );
                    }
                });
            });
        });
    }

    group.finish();
}

// ── History tracking overhead ────────────────────────────────────────────────

fn bench_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox/history");
    let rt = tokio::runtime::Runtime::new().unwrap();

    for count in &[10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("execute_and_read", count),
            count,
            |b, &n| {
                let executor = SandboxExecutor::new();
                b.iter(|| {
                    rt.block_on(async {
                        for _ in 0..n {
                            executor.execute_command("echo h").await.unwrap();
                        }
                        let history = executor.history().await;
                        black_box(history.len());
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sandbox_creation,
    bench_command_execution,
    bench_sandbox_as_task_executor,
    bench_task_creation,
    bench_task_runtime_creation,
    bench_parallel_execution,
    bench_history,
);
criterion_main!(benches);
