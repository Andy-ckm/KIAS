//! AgenticRAG + Memory Layers 基准测试

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;

use kias_knowledge::agentic_rag::{
    AgenticRAGEngine, DocumentMetadata, FlywheelLearner, InMemoryDocumentStore,
    RetrievalExperience, RetrievalTool,
};
use kias_knowledge::memory_layers::{
    DreamConfig, DreamConsolidator, SessionMemoryConfig, SessionMemoryEntry, SessionMemoryManager,
    ToolCallStats, ToolResultStore, ToolResultStoreConfig,
};

fn bench_agentic_rag_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let store = Arc::new(InMemoryDocumentStore::new());

    rt.block_on(async {
        for i in 0..100 {
            store
                .add_document(
                    DocumentMetadata {
                        id: format!("doc{}", i),
                        title: format!("Document {}", i),
                        filename: format!("doc{}.txt", i),
                        file_type: "txt".into(),
                        total_lines: 100,
                        total_tokens: 5000,
                    },
                    vec![
                        format!("This is document {} about revenue", i),
                        format!("Document {} discusses profit margins", i),
                    ],
                )
                .await;
        }
    });

    c.bench_function("agentic_rag_retrieve_100_docs", |b| {
        b.iter(|| {
            rt.block_on(async {
                let engine = AgenticRAGEngine::with_rules(store.clone()).unwrap();
                let result = engine.retrieve(black_box("revenue profit")).await;
                black_box(result)
            })
        })
    });
}

fn bench_tool_result_store(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("tool_result_store_preview", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = ToolResultStore::new(ToolResultStoreConfig::default());
                let content = "x".repeat(black_box(5000));
                let preview = store.store("test1", "search", &content).await;
                black_box(preview)
            })
        })
    });
}

fn bench_session_memory(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("session_memory_update", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mgr = SessionMemoryManager::new(SessionMemoryConfig::default());
                mgr.create_session("s1", "test query").await;
                for i in 0..100 {
                    mgr.update("s1", &format!("finding {}", i), Some("doc1"))
                        .await;
                }
                let summary = mgr.generate_summary("s1").await;
                black_box(summary)
            })
        })
    });
}

fn bench_dream_consolidation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("dream_consolidation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let consolidator = DreamConsolidator::new(DreamConfig::default());
                for i in 0..10 {
                    consolidator
                        .record_session(SessionMemoryEntry {
                            session_id: format!("s{}", i),
                            query_summary: format!("query {}", i),
                            key_findings: vec![format!("finding {} about error", i)],
                            referenced_docs: vec![format!("doc{}", i)],
                            tool_stats: ToolCallStats::default(),
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                        })
                        .await;
                }
                let result = consolidator.dream().await;
                black_box(result)
            })
        })
    });
}

fn bench_flywheel_learner(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("flywheel_recommend", |b| {
        b.iter(|| {
            rt.block_on(async {
                let learner = FlywheelLearner::new();
                for i in 0..50 {
                    learner
                        .record(RetrievalExperience {
                            query: format!("query about topic {}", i % 10),
                            successful_refs: vec![format!("doc{}", i)],
                            tools_used: vec![RetrievalTool::Search],
                            iterations: 3,
                            quality_score: 0.8,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        })
                        .await;
                }
                let recommended = learner.recommend(black_box("topic 5")).await;
                black_box(recommended)
            })
        })
    });
}

criterion_group!(
    benches,
    bench_agentic_rag_search,
    bench_tool_result_store,
    bench_session_memory,
    bench_dream_consolidation,
    bench_flywheel_learner,
);
criterion_main!(benches);
