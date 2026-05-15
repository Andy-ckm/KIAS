//! Model Router Benchmarks
//!
//! Measures performance of the model router's core logic:
//! - Router creation and provider registration
//! - Routing strategy selection (round-robin, least-latency, weighted-random, etc.)
//! - Circuit breaker overhead
//! - Cache key generation
//! - Request cache operations
//! - Filter pipeline throughput
//! - Type serialization/deserialization

use async_trait::async_trait;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kias_model_router::{
    ChatChoice, ChatMessage, ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse,
    ModelCapability, ModelRouter, ProviderHealth, RoutingPreference, RoutingStrategy, Usage,
};
use kias_model_router::{FilterPipeline, ProviderConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

// ── Mock provider for benchmarks ─────────────────────────────────────────────

struct MockProvider {
    config: ProviderConfig,
    call_count: Arc<AtomicU64>,
}

impl MockProvider {
    fn new(name: &str, models: Vec<String>, weight: f64) -> Self {
        Self {
            config: ProviderConfig {
                name: name.to_string(),
                provider_type: "mock".to_string(),
                endpoint: "http://localhost:0".to_string(),
                api_key: None,
                models,
                max_concurrency: 100,
                timeout_secs: 1,
                priority: 0,
                weight,
                headers: HashMap::new(),
                rate_limit_rpm: None,
                monthly_budget: None,
            },
            call_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait]
impl kias_model_router::Provider for MockProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn supported_models(&self) -> &[String] {
        &self.config.models
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth {
            healthy: true,
            success_rate: 1.0,
            avg_latency_ms: 1,
            total_requests: self.call_count.load(Ordering::Relaxed),
            failed_requests: 0,
            last_error: None,
            last_check: SystemTime::now(),
        }
    }

    async fn chat(
        &self,
        _request: &ChatRequest,
    ) -> kias_model_router::RouterResult<ChatResponse> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        Ok(ChatResponse {
            id: "mock-1".to_string(),
            model: "mock-model".to_string(),
            provider: self.config.name.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "mock response".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage::default(),
            latency_ms: 1,
            cost_usd: 0.0,
        })
    }

    async fn embedding(
        &self,
        _request: &EmbeddingRequest,
    ) -> kias_model_router::RouterResult<EmbeddingResponse> {
        Ok(EmbeddingResponse {
            model: "mock-embed".to_string(),
            provider: self.config.name.clone(),
            embeddings: vec![vec![0.0; 1536]],
            usage: Usage::default(),
            cost_usd: 0.0,
        })
    }
}

// ── Helper: make a chat request ─────────────────────────────────────────────

fn make_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_call_id: None,
            tool_calls: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: None,
        stop: None,
        stream: false,
        tools: None,
        user: None,
        routing: None,
    }
}

fn make_request_with_strategy(model: &str, strategy: RoutingStrategy) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_call_id: None,
            tool_calls: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: None,
        stop: None,
        stream: false,
        tools: None,
        user: None,
        routing: Some(RoutingPreference {
            strategy: Some(strategy),
            required_capabilities: vec![],
            max_cost: None,
            max_latency_ms: None,
            excluded_providers: vec![],
        }),
    }
}

// ── Helper: build router with N mock providers ──────────────────────────────

async fn build_router(providers: usize, strategy: RoutingStrategy) -> ModelRouter {
    let config = kias_model_router::RouterConfig {
        default_strategy: strategy,
        max_retries: 1,
        cache_enabled: false,
        cache_ttl_secs: 0,
        circuit_breaker_enabled: false,
        circuit_breaker_threshold: 5,
        circuit_breaker_recovery_secs: 60,
        global_budget: None,
        logging_enabled: false,
    };
    let router = ModelRouter::new(config);
    for i in 0..providers {
        let mock = MockProvider::new(
            &format!("provider-{}", i),
            vec!["gpt-4".to_string(), "claude-3".to_string()],
            1.0,
        );
        router.add_custom_provider(Box::new(mock)).await;
    }
    router
}

async fn build_router_with_cb(providers: usize, strategy: RoutingStrategy) -> ModelRouter {
    let config = kias_model_router::RouterConfig {
        default_strategy: strategy,
        max_retries: 1,
        cache_enabled: false,
        cache_ttl_secs: 0,
        circuit_breaker_enabled: true,
        circuit_breaker_threshold: 5,
        circuit_breaker_recovery_secs: 60,
        global_budget: None,
        logging_enabled: false,
    };
    let router = ModelRouter::new(config);
    for i in 0..providers {
        let mock = MockProvider::new(
            &format!("provider-{}", i),
            vec!["gpt-4".to_string(), "claude-3".to_string()],
            1.0,
        );
        router.add_custom_provider(Box::new(mock)).await;
    }
    router
}

// ── Router creation ─────────────────────────────────────────────────────────

fn bench_router_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/creation");

    group.bench_function("new_router", |b| {
        b.iter(|| {
            black_box(ModelRouter::new(kias_model_router::RouterConfig::default()));
        });
    });

    group.finish();
}

// ── Provider registration ────────────────────────────────────────────────────

fn bench_provider_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/provider_registration");

    for count in &[1, 5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &n| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    let router =
                        rt.block_on(build_router(n, RoutingStrategy::RoundRobin));
                    black_box(n);
                    drop(router);
                });
            },
        );
    }

    group.finish();
}

// ── Routing strategy benchmarks ──────────────────────────────────────────────

fn bench_routing_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/strategies");
    group.sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();

    for providers in &[1, 5, 10] {
        for strategy_name in &["RoundRobin", "LeastLatency", "LeastBusy", "WeightedRandom"] {
            let strategy = match *strategy_name {
                "RoundRobin" => RoutingStrategy::RoundRobin,
                "LeastLatency" => RoutingStrategy::LeastLatency,
                "LeastBusy" => RoutingStrategy::LeastBusy,
                "WeightedRandom" => RoutingStrategy::WeightedRandom,
                _ => RoutingStrategy::RoundRobin,
            };

            let label = format!("{}/{}_providers", strategy_name, providers);
            let router = rt.block_on(build_router(*providers, strategy));

            group.bench_with_input(
                BenchmarkId::new("single_chat", &label),
                &label,
                |b, _| {
                    let request = make_request("gpt-4");
                    b.iter(|| {
                        let req = request.clone();
                        rt.block_on(async {
                            black_box(router.chat(req).await.unwrap());
                        });
                    });
                },
            );
        }
    }

    group.finish();
}

// ── Circuit breaker overhead ─────────────────────────────────────────────────

fn bench_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/circuit_breaker");
    group.sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();

    // With circuit breaker disabled
    let router_no_cb = rt.block_on(build_router(5, RoutingStrategy::RoundRobin));
    group.bench_function("cb_disabled", |b| {
        let request = make_request("gpt-4");
        b.iter(|| {
            let req = request.clone();
            rt.block_on(async {
                black_box(router_no_cb.chat(req).await.unwrap());
            });
        });
    });

    // With circuit breaker enabled
    let router_with_cb = rt.block_on(build_router_with_cb(5, RoutingStrategy::RoundRobin));
    group.bench_function("cb_enabled", |b| {
        let request = make_request("gpt-4");
        b.iter(|| {
            let req = request.clone();
            rt.block_on(async {
                black_box(router_with_cb.chat(req).await.unwrap());
            });
        });
    });

    group.finish();
}

// ── Cache key generation ─────────────────────────────────────────────────────

// Cache key is private, so we benchmark cache behavior through the chat API
// (see bench_cached_requests below)

// ── Cached request throughput ────────────────────────────────────────────────

fn bench_cached_requests(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/cached_requests");
    group.sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();

    // Router with caching enabled
    let config = kias_model_router::RouterConfig {
        default_strategy: RoutingStrategy::RoundRobin,
        max_retries: 1,
        cache_enabled: true,
        cache_ttl_secs: 300,
        circuit_breaker_enabled: false,
        circuit_breaker_threshold: 5,
        circuit_breaker_recovery_secs: 60,
        global_budget: None,
        logging_enabled: false,
    };
    let router = ModelRouter::new(config);
    rt.block_on(async {
        let mock = MockProvider::new(
            "cache-provider",
            vec!["gpt-4".to_string()],
            1.0,
        );
        router.add_custom_provider(Box::new(mock)).await;
    });

    // Warm the cache
    let warm_request = make_request("gpt-4");
    rt.block_on(async {
        router.chat(warm_request).await.unwrap();
    });

    group.bench_function("cache_hit", |b| {
        let request = make_request("gpt-4");
        b.iter(|| {
            let req = request.clone();
            rt.block_on(async {
                black_box(router.chat(req).await.unwrap());
            });
        });
    });

    // Compare: uncached requests
    let router_uncached = rt.block_on(build_router(1, RoutingStrategy::RoundRobin));
    group.bench_function("no_cache", |b| {
        let request = make_request("gpt-4");
        b.iter(|| {
            let req = request.clone();
            rt.block_on(async {
                black_box(router_uncached.chat(req).await.unwrap());
            });
        });
    });

    group.finish();
}

// ── Concurrent routing ───────────────────────────────────────────────────────

fn bench_concurrent_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/concurrent");
    group.sample_size(50);

    let rt = tokio::runtime::Runtime::new().unwrap();

    for concurrency in &[10, 50, 100] {
        let router = rt.block_on(build_router(5, RoutingStrategy::RoundRobin));

        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &n| {
                b.iter(|| {
                    let router_ref = &router;
                    rt.block_on(async move {
                        let mut handles = Vec::with_capacity(n);
                        for _ in 0..n {
                            let request = make_request("gpt-4");
                            // We can't easily share router across tasks without Arc,
                            // so we run sequential
                            handles.push(router_ref.chat(request).await);
                        }
                        black_box(handles.len());
                    });
                });
            },
        );
    }

    group.finish();
}

// ── Filter pipeline ──────────────────────────────────────────────────────────

fn bench_filter_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/filter_pipeline");

    group.bench_function("create_default_pipeline", |b| {
        b.iter(|| {
            black_box(FilterPipeline::new());
        });
    });

    group.finish();
}

// ── Routing strategy serialization ───────────────────────────────────────────

fn bench_strategy_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("router/serialization");

    let strategies = vec![
        RoutingStrategy::RoundRobin,
        RoutingStrategy::LeastLatency,
        RoutingStrategy::CostOptimized,
        RoutingStrategy::CapabilityBased,
        RoutingStrategy::WeightedRandom,
        RoutingStrategy::Pinned("openai".to_string()),
        RoutingStrategy::LeastBusy,
        RoutingStrategy::UsageBased,
    ];

    group.bench_function("serialize_strategies", |b| {
        b.iter(|| {
            for s in &strategies {
                black_box(serde_json::to_string(s).unwrap());
            }
        });
    });

    group.bench_function("deserialize_strategies", |b| {
        let json_strs: Vec<String> = strategies
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();
        b.iter(|| {
            for json in &json_strs {
                black_box(serde_json::from_str::<RoutingStrategy>(json).unwrap());
            }
        });
    });

    // ChatRequest serialization roundtrip
    let request = make_request("gpt-4");
    group.bench_function("chat_request_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&request).unwrap();
            black_box(serde_json::from_str::<ChatRequest>(&json).unwrap());
        });
    });

    // ChatResponse serialization roundtrip
    let response = ChatResponse {
        id: "chatcmpl-123".to_string(),
        model: "gpt-4".to_string(),
        provider: "openai".to_string(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: "Hello!".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        latency_ms: 200,
        cost_usd: 0.001,
    };
    group.bench_function("chat_response_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&response).unwrap();
            black_box(serde_json::from_str::<ChatResponse>(&json).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_router_creation,
    bench_provider_registration,
    bench_routing_strategies,
    bench_circuit_breaker,
    bench_cached_requests,
    bench_concurrent_routing,
    bench_filter_pipeline,
    bench_strategy_serialization,
);
criterion_main!(benches);
