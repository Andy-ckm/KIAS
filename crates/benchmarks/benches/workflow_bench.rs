//! Workflow Engine DAG Execution Benchmarks
//!
//! Measures the overhead of the workflow engine's graph traversal, state
//! management, checkpointing, and event emission across different DAG
//! topologies (linear, wide, deep, branching).
//!
//! Uses Fork/Join nodes (no-op execution) to isolate engine overhead from
//! executor latency.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_workflow_engine::edge::Edge;
use kias_workflow_engine::node::{Node, NodeType};
use kias_workflow_engine::state::WorkflowState;
use kias_workflow_engine::WorkflowEngine;
use kias_workflow_engine::WorkflowGraph;

// ── Graph builders ────────────────────────────────────────────────────────

/// Build a linear DAG: n1 → n2 → ... → nN
///
/// Each internal node is a Fork (no-op), first is entry, last is exit.
fn linear_dag(depth: usize) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new("linear");

    for i in 0..depth {
        let node = Node::new(&format!("n{}", i), &format!("step-{}", i), NodeType::Fork);
        graph.add_node(node);
    }

    for i in 0..depth - 1 {
        graph.add_edge(Edge::new(&format!("n{}", i), &format!("n{}", i + 1)));
    }

    graph.set_entry("n0");
    graph.add_exit_node(&format!("n{}", depth - 1));
    graph
}

/// Build a wide DAG: entry → [n1, n2, ..., nK] (fan-out, all join at exit).
///
/// This tests the engine's ability to traverse edges with multiple targets.
/// Uses Fork node at entry that fans out to K parallel paths, each a single
/// Join node that connects to the exit.
fn wide_dag(width: usize) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new("wide");

    // Entry fork
    graph.add_node(Node::new("entry", "entry", NodeType::Fork));

    // Fan-out nodes
    for i in 0..width {
        let node = Node::new(&format!("w{}", i), &format!("wide-{}", i), NodeType::Join);
        graph.add_node(node);
        graph.add_edge(Edge::new("entry", &format!("w{}", i)));
    }

    // Exit
    graph.add_node(Node::new("exit", "exit", NodeType::Join));
    for i in 0..width {
        graph.add_edge(Edge::new(&format!("w{}", i), "exit"));
    }

    graph.set_entry("entry");
    graph.add_exit_node("exit");
    graph
}

/// Build a deep DAG with branching conditions: a chain of Condition nodes.
///
/// Each condition node evaluates a key in state and transitions to the next
/// node. This tests condition evaluation and edge resolution.
fn branching_dag(depth: usize) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new("branching");

    // Entry
    graph.add_node(Node::new("start", "start", NodeType::Fork));

    for i in 0..depth {
        let cond = Node::new(&format!("c{}", i), &format!("cond-{}", i), NodeType::Condition)
            .with_config(serde_json::json!({
                "condition_key": format!("branch_{}", i),
            }));
        graph.add_node(cond);

        if i == 0 {
            graph.add_edge(Edge::new("start", "c0"));
        } else {
            graph.add_edge(Edge::new(&format!("c{}", i - 1), &format!("c{}", i)));
        }
    }

    // Exit
    graph.add_node(Node::new("end", "end", NodeType::Join));
    graph.add_edge(Edge::new(&format!("c{}", depth - 1), "end"));

    graph.set_entry("start");
    graph.add_exit_node("end");
    graph
}

// ── Benchmarks ────────────────────────────────────────────────────────────

fn bench_linear_dag(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow/linear_dag");

    for depth in &[5, 20, 50, 100] {
        let engine = WorkflowEngine::new();
        let graph = linear_dag(*depth);

        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                let initial = WorkflowState::new("bench-wf", "n0");
                rt.block_on(async {
                    black_box(engine.execute(&graph, initial).await.unwrap());
                });
            });
        });
    }
    group.finish();
}

fn bench_wide_dag(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow/wide_dag");

    for width in &[5, 20, 50, 100] {
        let engine = WorkflowEngine::new();
        let graph = wide_dag(*width);

        group.bench_with_input(BenchmarkId::from_parameter(width), width, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                let initial = WorkflowState::new("bench-wf", "entry");
                rt.block_on(async {
                    black_box(engine.execute(&graph, initial).await.unwrap());
                });
            });
        });
    }
    group.finish();
}

fn bench_branching_dag(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow/branching_dag");

    for depth in &[5, 10, 20, 50] {
        let engine = WorkflowEngine::new();
        let graph = branching_dag(*depth);

        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, _| {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            b.iter(|| {
                let mut initial = WorkflowState::new("bench-wf", "start");
                // Pre-populate condition keys so conditions evaluate cleanly
                for i in 0..*depth {
                    initial.set(format!("branch_{}", i), "default");
                }
                rt.block_on(async {
                    black_box(engine.execute(&graph, initial).await.unwrap());
                });
            });
        });
    }
    group.finish();
}

fn bench_graph_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow/graph_validation");

    for depth in &[10, 50, 100, 500] {
        let graph = linear_dag(*depth);

        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, _| {
            b.iter(|| {
                black_box(graph.validate().unwrap());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_linear_dag,
    bench_wide_dag,
    bench_branching_dag,
    bench_graph_validation,
);
criterion_main!(benches);
