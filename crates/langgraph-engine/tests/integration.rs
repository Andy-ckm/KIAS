//! Comprehensive tests for the LangGraph engine.

use kias_langgraph_engine::*;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════
// Basic Graph Execution
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_linear_graph() {
    let graph = StateGraph::builder("a")
        .add_node("a", |mut state| async move {
            state.set("x", 10);
            Ok(state)
        })
        .add_node("b", |mut state| async move {
            let x: i32 = state.get("x").unwrap_or(0);
            state.set("x", x + 5);
            Ok(state)
        })
        .add_node("c", |state| async move { Ok(state) })
        .add_edge("a", "b")
        .add_edge("b", "c")
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<i32>("x"), Some(15));
    assert_eq!(result.metadata.node_history, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn test_single_node_graph() {
    let graph = StateGraph::builder("only")
        .add_node("only", |mut state| async move {
            state.set("done", true);
            Ok(state)
        })
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<bool>("done"), Some(true));
}

// ═══════════════════════════════════════════════════════════════════════
// Conditional Edges
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_conditional_branch_high() {
    let graph = StateGraph::builder("check")
        .add_node("check", |mut state| async move {
            state.set("value", 10);
            Ok(state)
        })
        .add_node("high", |mut state| async move {
            state.set("result", "high");
            Ok(state)
        })
        .add_node("low", |mut state| async move {
            state.set("result", "low");
            Ok(state)
        })
        .add_conditional_edge("check", "high", |s| s.get::<i32>("value").unwrap_or(0) > 5)
        .add_conditional_edge("check", "low", |s| s.get::<i32>("value").unwrap_or(0) <= 5)
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<String>("result"), Some("high".to_string()));
}

#[tokio::test]
async fn test_conditional_branch_low() {
    let graph = StateGraph::builder("check")
        .add_node("check", |mut state| async move {
            state.set("value", 3);
            Ok(state)
        })
        .add_node("high", |mut state| async move {
            state.set("result", "high");
            Ok(state)
        })
        .add_node("low", |mut state| async move {
            state.set("result", "low");
            Ok(state)
        })
        .add_conditional_edge("check", "high", |s| s.get::<i32>("value").unwrap_or(0) > 5)
        .add_conditional_edge("check", "low", |s| s.get::<i32>("value").unwrap_or(0) <= 5)
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<String>("result"), Some("low".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════
// Loop Support
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_loop_with_exit_condition() {
    let graph = StateGraph::builder("init")
        .add_node("init", |mut state| async move {
            state.set("counter", 0);
            Ok(state)
        })
        .add_node("increment", |mut state| async move {
            let c: i32 = state.get("counter").unwrap_or(0);
            state.set("counter", c + 1);
            Ok(state)
        })
        .add_node("done", |state| async move { Ok(state) })
        .add_edge("init", "increment")
        .add_conditional_edge("increment", "increment", |s| {
            s.get::<i32>("counter").unwrap_or(0) < 5
        })
        .add_conditional_edge("increment", "done", |s| {
            s.get::<i32>("counter").unwrap_or(0) >= 5
        })
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<i32>("counter"), Some(5));
}

// ═══════════════════════════════════════════════════════════════════════
// Router Functions
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_router_dynamic_branching() {
    let graph = StateGraph::builder("classify")
        .add_node("classify", |mut state| async move {
            state.set("priority", "urgent");
            Ok(state)
        })
        .add_node("urgent_handler", |mut state| async move {
            state.set("handled_by", "urgent");
            Ok(state)
        })
        .add_node("normal_handler", |mut state| async move {
            state.set("handled_by", "normal");
            Ok(state)
        })
        .add_node("low_handler", |mut state| async move {
            state.set("handled_by", "low");
            Ok(state)
        })
        .add_router("classify", |s| {
            let priority: String = s.get("priority").unwrap_or_default();
            match priority.as_str() {
                "urgent" => "urgent_handler".to_string(),
                "normal" => "normal_handler".to_string(),
                _ => "low_handler".to_string(),
            }
        })
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(
        result.get::<String>("handled_by"),
        Some("urgent".to_string())
    );
}

#[tokio::test]
async fn test_router_fallback_to_low() {
    let graph = StateGraph::builder("classify")
        .add_node("classify", |state| async move {
            let state = state;
            // Don't set priority — should fall through to low_handler
            Ok(state)
        })
        .add_node("urgent_handler", |mut state| async move {
            state.set("handled_by", "urgent");
            Ok(state)
        })
        .add_node("normal_handler", |mut state| async move {
            state.set("handled_by", "normal");
            Ok(state)
        })
        .add_node("low_handler", |mut state| async move {
            state.set("handled_by", "low");
            Ok(state)
        })
        .add_router("classify", |s| {
            let priority: String = s.get("priority").unwrap_or_default();
            match priority.as_str() {
                "urgent" => "urgent_handler".to_string(),
                "normal" => "normal_handler".to_string(),
                _ => "low_handler".to_string(),
            }
        })
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<String>("handled_by"), Some("low".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════
// Fan-Out Parallel Execution
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_fan_out_parallel() {
    let graph = StateGraph::builder("start")
        .add_node("start", |mut state| async move {
            state.set("input", 10);
            Ok(state)
        })
        .add_node("branch_a", |mut state| async move {
            let x: i32 = state.get("input").unwrap_or(0);
            state.set("result_a", x * 2);
            Ok(state)
        })
        .add_node("branch_b", |mut state| async move {
            let x: i32 = state.get("input").unwrap_or(0);
            state.set("result_b", x * 3);
            Ok(state)
        })
        .add_node("merge", |state| async move { Ok(state) })
        .add_edge("start", "start") // this won't be reached; fan-out takes priority
        .add_fan_out("start", vec!["branch_a", "branch_b"], "merge")
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<i32>("result_a"), Some(20));
    assert_eq!(result.get::<i32>("result_b"), Some(30));
    assert_eq!(result.get::<i32>("input"), Some(10));
}

#[tokio::test]
async fn test_fan_out_state_merge() {
    let graph = StateGraph::builder("start")
        .add_node("start", |mut state| async move {
            state.set("shared", "original");
            state.set("branch_id", "none");
            Ok(state)
        })
        .add_node("branch_1", |mut state| async move {
            state.set("from_branch_1", true);
            Ok(state)
        })
        .add_node("branch_2", |mut state| async move {
            state.set("from_branch_2", true);
            Ok(state)
        })
        .add_node("done", |state| async move { Ok(state) })
        .add_fan_out("start", vec!["branch_1", "branch_2"], "done")
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<bool>("from_branch_1"), Some(true));
    assert_eq!(result.get::<bool>("from_branch_2"), Some(true));
    // Original state preserved
    assert_eq!(result.get::<String>("shared"), Some("original".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════
// Checkpoint Persistence
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_checkpoint_save_and_load() {
    let store = Arc::new(InMemoryCheckpointStore::new());

    let graph = StateGraph::builder("start")
        .add_node("start", |mut state| async move {
            state.set("step", 1);
            Ok(state)
        })
        .add_node("end", |state| async move { Ok(state) })
        .add_edge("start", "end")
        .with_checkpoint_store(store.clone())
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();

    // Checkpoints should have been saved
    let run_id = &result.metadata.run_id;
    let checkpoints = store.load_history(run_id).await.unwrap();
    assert!(!checkpoints.is_empty(), "Expected at least one checkpoint");
    assert!(!checkpoints[0].id.is_empty());
}

#[tokio::test]
async fn test_interrupt_and_resume_with_checkpoint() {
    let store = Arc::new(InMemoryCheckpointStore::new());
    let stream = Arc::new(ExecutionStream::new());
    let mut rx = stream.subscribe();

    let graph = StateGraph::builder("step1")
        .add_node("step1", |mut state| async move {
            state.set("progress", 1);
            Ok(state)
        })
        .add_node("interrupt_point", |state| async move {
            let mut state = state;
            // Only interrupt if not already resumed
            let current: i32 = state.get("progress").unwrap_or(0);
            if current < 2 {
                state.set("progress", 2);
                state.metadata.is_interrupted = true;
            }
            Ok(state)
        })
        .add_node("step3", |mut state| async move {
            state.set("progress", 3);
            Ok(state)
        })
        .add_edge("step1", "interrupt_point")
        .add_edge("interrupt_point", "step3")
        .with_checkpoint_store(store.clone())
        .with_stream(stream.clone())
        .build()
        .expect("graph should be valid");

    // Execute — should stop at interrupt
    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<i32>("progress"), Some(2));
    assert!(result.metadata.is_interrupted);

    // Drain events
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Should have Interrupted event
    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::Interrupted { .. })));

    // Find the checkpoint to resume from
    let run_id = &result.metadata.run_id;
    let latest = store.load_latest(run_id).await.unwrap().unwrap();

    // Resume from checkpoint
    let resumed = graph.resume_from_checkpoint(&latest.id).await.unwrap();
    assert_eq!(resumed.get::<i32>("progress"), Some(3));
    assert!(!resumed.metadata.is_interrupted);
}

#[tokio::test]
async fn test_resume_latest_checkpoint() {
    let store = Arc::new(InMemoryCheckpointStore::new());

    let graph = StateGraph::builder("a")
        .add_node("a", |state| async move {
            let mut state = state;
            // Only interrupt first time
            if state.get::<i32>("val").is_none() {
                state.set("val", 1);
                state.metadata.is_interrupted = true;
            }
            Ok(state)
        })
        .add_node("b", |mut state| async move {
            state.set("val", 2);
            Ok(state)
        })
        .add_edge("a", "b")
        .with_checkpoint_store(store.clone())
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    let run_id = result.metadata.run_id.clone();

    // Resume latest
    let resumed = graph.resume_latest(&run_id).await.unwrap();
    assert!(!resumed.metadata.is_interrupted);
}

// ═══════════════════════════════════════════════════════════════════════
// Streaming Events
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_streaming_events() {
    let stream = Arc::new(ExecutionStream::new());
    let mut rx = stream.subscribe();

    let graph = StateGraph::builder("a")
        .add_node("a", |mut state| async move {
            state.set("x", 1);
            Ok(state)
        })
        .add_node("b", |state| async move { Ok(state) })
        .add_edge("a", "b")
        .with_stream(stream.clone())
        .build()
        .expect("graph should be valid");

    graph.execute(GraphState::new()).await.unwrap();

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Should have at least: NodeStart, NodeComplete, EdgeTaken, NodeStart, NodeComplete, Completed
    assert!(
        events.len() >= 5,
        "Expected at least 5 events, got {}",
        events.len()
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::NodeStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::Completed { .. })));
}

#[tokio::test]
async fn test_event_summary_format() {
    let event = ExecutionEvent::NodeComplete {
        node: "process".to_string(),
        step: 3,
        duration_ms: 42,
        timestamp_ms: 0,
    };
    assert_eq!(event.summary(), "✔ process (42ms)");

    let event = ExecutionEvent::Failed {
        node: "api_call".to_string(),
        error: "timeout".to_string(),
    };
    assert_eq!(event.summary(), "❌ Failed at api_call: timeout");
}

#[tokio::test]
async fn test_event_collector() {
    let collector = EventCollector::new();
    assert!(collector.is_empty());

    collector.push(ExecutionEvent::NodeStart {
        node: "a".to_string(),
        step: 0,
        timestamp_ms: 0,
    });
    collector.push(ExecutionEvent::NodeComplete {
        node: "a".to_string(),
        step: 0,
        duration_ms: 5,
        timestamp_ms: 0,
    });

    assert_eq!(collector.len(), 2);
    assert!(!collector.is_empty());
    assert_eq!(collector.summaries().len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════
// Graph Validation
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_validation_missing_entry() {
    let result = StateGraph::builder("nonexistent")
        .add_node("a", |state| async move { Ok(state) })
        .add_edge("a", "a")
        .build();

    assert!(result.is_err());
    let errors = result.err().expect("expected validation errors");
    assert!(errors
        .iter()
        .any(|e| e.kind == ValidationErrorKind::MissingEntryNode));
}

#[tokio::test]
async fn test_validation_passes_for_valid_graph() {
    let result = StateGraph::builder("a")
        .add_node("a", |state| async move { Ok(state) })
        .add_node("b", |state| async move { Ok(state) })
        .add_edge("a", "b")
        .build();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validation_unreachable_node() {
    let result = StateGraph::builder("a")
        .add_node("a", |state| async move { Ok(state) })
        .add_node("b", |state| async move { Ok(state) })
        .add_node("orphan", |state| async move { Ok(state) })
        .add_edge("a", "b")
        .build();

    assert!(result.is_err());
    let errors = result.err().expect("expected validation errors");
    assert!(errors
        .iter()
        .any(|e| e.kind == ValidationErrorKind::DeadEnd && e.message.contains("unreachable")));
}

#[tokio::test]
async fn test_build_unchecked_skips_validation() {
    // This would fail validation (entry "missing" doesn't exist)
    let graph = StateGraph::builder("missing")
        .add_node("a", |state| async move { Ok(state) })
        .build_unchecked();

    assert_eq!(graph.node_count(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Error Handling
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_node_error_propagation() {
    let graph = StateGraph::builder("a")
        .add_node("a", |_state| async move {
            Err(kias_common::KiasError::Validation(
                "intentional error".to_string(),
            ))
        })
        .add_node("b", |state| async move { Ok(state) })
        .add_edge("a", "b")
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("intentional error"));
}

#[tokio::test]
async fn test_error_event_emitted() {
    let stream = Arc::new(ExecutionStream::new());
    let mut rx = stream.subscribe();

    let graph = StateGraph::builder("fail")
        .add_node("fail", |_state| async move {
            Err(kias_common::KiasError::Validation("boom".to_string()))
        })
        .with_stream(stream.clone())
        .build()
        .expect("graph should be valid");

    let _ = graph.execute(GraphState::new()).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::NodeError { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::Failed { .. })));
}

// ═══════════════════════════════════════════════════════════════════════
// Max Steps Limit
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_max_steps_exceeded() {
    let graph = StateGraph::builder("loop")
        .add_node("loop", |state| async move { Ok(state) })
        .add_edge("loop", "loop")
        .with_max_steps(5)
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("max steps"));
}

// ═══════════════════════════════════════════════════════════════════════
// State Operations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_state_get_set() {
    let mut state = GraphState::new();
    state.set("name", "kias");
    state.set("count", 42);
    assert_eq!(state.get::<String>("name"), Some("kias".to_string()));
    assert_eq!(state.get::<i32>("count"), Some(42));
    assert_eq!(state.get::<bool>("missing"), None);
}

#[test]
fn test_state_get_required() {
    let mut state = GraphState::new();
    state.set("key", "value");
    assert!(state.get_required::<String>("key").is_ok());
    assert!(state.get_required::<String>("missing").is_err());
}

#[test]
fn test_state_has_and_remove() {
    let mut state = GraphState::new();
    state.set("x", 1);
    assert!(state.has("x"));
    assert!(!state.has("y"));

    state.remove("x");
    assert!(!state.has("x"));
}

#[test]
fn test_state_keys() {
    let mut state = GraphState::new();
    state.set("a", 1);
    state.set("b", 2);
    let mut keys: Vec<&str> = state.keys();
    keys.sort();
    assert_eq!(keys, vec!["a", "b"]);
}

#[test]
fn test_state_merge_overwrite() {
    let mut base = GraphState::new();
    base.set("x", 1);
    base.set("y", 2);

    let mut overlay = GraphState::new();
    overlay.set("y", 99);
    overlay.set("z", 3);

    base.merge(overlay);
    assert_eq!(base.get::<i32>("x"), Some(1));
    assert_eq!(base.get::<i32>("y"), Some(99)); // overwritten
    assert_eq!(base.get::<i32>("z"), Some(3));
}

#[test]
fn test_state_merge_keep_existing() {
    let mut base = GraphState::new();
    base.set("x", 1);
    base.set("y", 2);

    let mut overlay = GraphState::new();
    overlay.set("y", 99);
    overlay.set("z", 3);

    base.merge_keep_existing(overlay);
    assert_eq!(base.get::<i32>("x"), Some(1));
    assert_eq!(base.get::<i32>("y"), Some(2)); // kept original
    assert_eq!(base.get::<i32>("z"), Some(3));
}

#[test]
fn test_state_snapshot_restore() {
    let mut state = GraphState::new();
    state.set("x", 42);
    let snapshot = state.snapshot();

    let restored = GraphState::restore_from_snapshot(&snapshot);
    assert_eq!(restored.get::<i32>("x"), Some(42));
}

// ═══════════════════════════════════════════════════════════════════════
// Checkpoint Store
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_in_memory_checkpoint_store() {
    let store = InMemoryCheckpointStore::new();
    assert_eq!(store.count(), 0);

    let state = GraphState::new();
    let cp = Checkpoint {
        id: "cp-1".to_string(),
        run_id: "run-1".to_string(),
        node: "a".to_string(),
        state: state.clone(),
        timestamp: chrono::Utc::now(),
        version: 0,
    };

    store.save(cp).await.unwrap();
    assert_eq!(store.count(), 1);

    let loaded = store.load_latest("run-1").await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().id, "cp-1");

    let by_id = store.load_by_id("cp-1").await.unwrap();
    assert!(by_id.is_some());

    let missing = store.load_by_id("nonexistent").await.unwrap();
    assert!(missing.is_none());

    let history = store.load_history("run-1").await.unwrap();
    assert_eq!(history.len(), 1);

    store.delete_run("run-1").await.unwrap();
    assert_eq!(store.count(), 0);
}

#[tokio::test]
async fn test_checkpoint_store_multiple_versions() {
    let store = InMemoryCheckpointStore::new();

    for i in 0..5 {
        let cp = Checkpoint {
            id: format!("cp-{}", i),
            run_id: "run-1".to_string(),
            node: format!("node-{}", i),
            state: GraphState::new(),
            timestamp: chrono::Utc::now(),
            version: i,
        };
        store.save(cp).await.unwrap();
    }

    let history = store.load_history("run-1").await.unwrap();
    assert_eq!(history.len(), 5);
    // Should be ordered by version
    for (i, cp) in history.iter().enumerate() {
        assert_eq!(cp.version, i as u64);
    }

    let latest = store.load_latest("run-1").await.unwrap().unwrap();
    assert_eq!(latest.version, 4);
}

// ═══════════════════════════════════════════════════════════════════════
// Execution Stream
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_execution_stream_multi_subscriber() {
    let stream = ExecutionStream::new();
    let mut rx1 = stream.subscribe();
    let mut rx2 = stream.subscribe();

    assert_eq!(stream.subscriber_count(), 2);

    stream.emit(ExecutionEvent::NodeStart {
        node: "a".to_string(),
        step: 0,
        timestamp_ms: 0,
    });

    let event1 = rx1.try_recv().unwrap();
    let event2 = rx2.try_recv().unwrap();

    // Both subscribers receive the event
    assert!(matches!(event1, ExecutionEvent::NodeStart { .. }));
    assert!(matches!(event2, ExecutionEvent::NodeStart { .. }));
}

// ═══════════════════════════════════════════════════════════════════════
// Integration: Complex Workflow
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_complex_workflow_with_router_and_loop() {
    let store = Arc::new(InMemoryCheckpointStore::new());
    let stream = Arc::new(ExecutionStream::new());

    let graph = StateGraph::builder("init")
        .add_node("init", |mut state| async move {
            state.set("attempts", 0);
            state.set("max_attempts", 3);
            state.set("status", "pending");
            Ok(state)
        })
        .add_node("process", |mut state| async move {
            let attempts: i32 = state.get("attempts").unwrap_or(0);
            state.set("attempts", attempts + 1);
            // Simulate: succeed on 3rd attempt
            if attempts + 1 >= 3 {
                state.set("status", "success");
            } else {
                state.set("status", "retry");
            }
            Ok(state)
        })
        .add_node("done", |state| async move { Ok(state) })
        .add_node("failed", |mut state| async move {
            state.set("status", "failed");
            Ok(state)
        })
        .add_edge("init", "process")
        .add_router("process", |s| {
            let status: String = s.get("status").unwrap_or_default();
            let attempts: i32 = s.get("attempts").unwrap_or(0);
            let max: i32 = s.get("max_attempts").unwrap_or(3);
            match status.as_str() {
                "success" => "done".to_string(),
                "retry" if attempts < max => "process".to_string(),
                _ => "failed".to_string(),
            }
        })
        .with_checkpoint_store(store)
        .with_stream(stream)
        .build()
        .expect("graph should be valid");

    let result = graph.execute(GraphState::new()).await.unwrap();
    assert_eq!(result.get::<String>("status"), Some("success".to_string()));
    assert_eq!(result.get::<i32>("attempts"), Some(3));
}
