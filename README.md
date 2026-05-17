     1|<p align="center">
     2|  <a href="https://github.com/Andy-ckm/KIAS/blob/main/LICENSE">
     3|    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
     4|  </a>
     5|  <a href="https://github.com/Andy-ckm/KIAS/actions">
     6|    <img src="https://img.shields.io/badge/tests-1720%20passed-brightgreen.svg" alt="Tests">
     7|  </a>
     8|  <a href="https://www.rust-lang.org">
     9|    <img src="https://img.shields.io/badge/Rust-1.95-orange.svg?logo=rust" alt="Rust">
    10|  </a>
    11|  <a href="https://github.com/Andy-ckm/KIAS">
    12|    <img src="https://img.shields.io/badge/crates-22-purple.svg" alt="Crates">
    13|  </a>
    14|  <a href="https://github.com/Andy-ckm/KIAS">
    15|    <img src="https://img.shields.io/badge/LOC-85K%2B-blue.svg" alt="Lines of Code">
    16|  </a>
    17|  <a href="https://github.com/Andy-ckm/KIAS">
    18|    <img src="https://img.shields.io/github/stars/Andy-ckm/KIAS?style=social" alt="Stars">
    19|  </a>
    20|</p>
    21|
    22|<p align="center">
    23|  <img src="docs/logo/kias-logo.svg" alt="KIAS" width="420">
    24|</p>
    25|
    26|<h1 align="center">KIAS</h1>
    27|<p align="center"><strong>Kubernetes-like Intelligent Agent Scheduling</strong></p>
    28|<p align="center">Production-grade AI Agent cluster orchestration built in Rust</p>
    29|
    30|<p align="center">
    31|  <a href="docs/technical-showcase.md">Technical Deep Dive</a> ·
    32|  <a href="#quickstart">Quickstart</a> ·
    33|  <a href="#architecture">Architecture</a> ·
    34|  <a href="#supported-models">Supported Models</a> ·
    35|  <a href="#system-requirements">System Requirements</a>
    36|</p>
    37|
    38|---
    39|
    40|## Overview
    41|
    42|KIAS is a Rust-based AI Agent cluster scheduling system that applies Kubernetes control-plane architecture to LLM agent orchestration. It addresses the gap between prototype agent scripts and production-ready agent infrastructure: state persistence, crash recovery, multi-agent coordination, cache-aware scheduling, sandboxed execution, and observability.
    43|
    44|**Key numbers:**
    45|
    46|<p align="center">
    47|  <img src="docs/stats.svg" alt="KIAS Statistics" width="780">
    48|</p>
    49|
    50|| Metric | Value |
    51||--------|-------|
    52|| Rust Crates | 22 |
    53|| Lines of Code | 85,000+ |
    54|| Scheduling Algorithms | 7 (including GPU-Aware, Edge) |
    55|| MCP Sandbox Backends | 5 (Docker / Firecracker / gVisor / Wasm / Process) |
    56|| Test Coverage | `#[cfg(test)]` module in every crate |
    57|
    58|---
    59|
    60|## Architecture
    61|
    62|```
    63|┌─────────────────────────────────────────────────────────────────┐
    64|│                         API Server (axum)                        │
    65|│                   REST + gRPC + WebSocket + mTLS                 │
    66|├─────────┬──────────┬──────────┬──────────┬───────────────────────┤
    67|│Scheduler│Controller│Workflow  │  Team    │  Goal    │ Autonomy   │
    68|│  Engine │          │  Engine  │  Engine  │  Engine  │ Controller │
    69|│ 7 algos │Heartbeat │ DAG exec │ OVW      │ Goal loop│ 3-level    │
    70|├─────────┴──────────┴──────────┴──────────┴──────────┴───────────┤
    71|│  LangGraph Engine    │   MCP Protocol    │   Data Store (SQLite) │
    72|│  State graph+FanOut  │   JSON-RPC+Sandbox│   Vector+Prefix Cache │
    73|├──────────────────────┴───────────────────┴───────────────────────┤
    74|│                    Common (L0): Error / Config / A2A / Masking    │
    75|└─────────────────────────────────────────────────────────────────┘
    76|```
    77|
    78|**Layered dependency model** (enforced by `make lint-arch`):
    79|
    80|```
    81|L0: common                    ← Base types, errors, config
    82|L1: data-store                ← SQLite persistence layer
    83|L2: scheduler, controller, workflow-engine, team-engine, ...
    84|L3: api-server, kias-main
    85|```
    86|
    87|Strict unidirectional dependencies. No cross-layer imports.
    88|
    89|---
    90|
    91|---

## Engineering Principles

KIAS is grounded in **Qian Xuesen's Systems Engineering methodology** — treating AI Agent orchestration as an open complex giant system, not a toy prototype.

**Seven Principles (from [Qian Xuesen Theory](docs/qian-xuesen-engineering-principles.md)):**

> **开发必须遵守 [完整方法论](docs/METHODOLOGY.md)**：钱学森系统工程原理 + 马斯克第一性原则 + 论文+源码支撑。违反铁律 = 返工。

| # | Principle | KIAS Implementation |
|---|-----------|---------------------|
| 1 | **Holistic Thinking** | Every feature evaluated against whole-system impact |
| 2 | **Meta-Synthesis** | RAG + LLM + Human feedback integration |
| 3 | **Feedback Control** | InspirationStream (positive) + QualityPipeline (negative) |
| 4 | **Hierarchical Decomposition** | Shell → Agent → Workflow → Task layers |
| 5 | **Robustness** | Circuit breakers, fallbacks, retries, timeouts |
| 6 | **Observability** | Prometheus metrics, audit logs, deep health checks |
| 7 | **Engineering Discipline** | Zero-tolerance quality gates, source-backed features |

**"From Qualitative to Quantitative"** — KIAS evolves in three phases:

```
Phase 1: Rule-driven (keyword search, fixed workflows)        ← Current
Phase 2: Hybrid-driven (vector+keyword, Shell scheduling)     ← Building
Phase 3: Data-driven (AgenticRAG, learned workflows)          ← Target
```

## Core Innovations
    92|
    93|### 1. Cache-Aware Scheduling
    94|
    95|**File:** [`crates/scheduler/src/algorithms/cache_aware.rs`](crates/scheduler/src/algorithms/cache_aware.rs)
    96|
    97|Traditional schedulers (Round Robin, Least Loaded) are blind to LLM inference characteristics — they don't know whether a node has already cached a specific system prompt's KV Cache. A cache miss means recomputing the entire prefix, wasting ~90% of GPU compute.
    98|
    99|KIAS introduces **DeepSeek-style Prefix Caching** into the scheduling decision layer:
   100|
   101|```rust
   102|// crates/scheduler/src/algorithms/cache_aware.rs
   103|fn cache_aware_score(
   104|    node: &Node, agent: &Agent,
   105|    cache_info: Option<&NodeCacheInfo>, cache_weight: f64,
   106|) -> f64 {
   107|    let cache_score = if let (Some(info), Some(prefix_hash)) = (cache_info, agent.system_prompt_hash) {
   108|        if info.cached_prefixes.contains(&prefix_hash) { 1.0 } else { 0.0 }
   109|    } else { 0.0 };
   110|    let load_score = 1.0 - node.load_factor();
   111|    cache_weight * cache_score + (1.0 - cache_weight) * load_score
   112|}
   113|```
   114|
   115|- **Fast path:** If a node has a matching cached prefix, route directly (score = 1.0)
   116|- **Weighted scoring:** `cache_weight` parameter (0.0 = pure load balancing, 1.0 = pure cache priority)
   117|- **Concurrent-safe:** `Arc<DashMap>` for lock-free concurrent cache map access
   118|
   119|This is the only scheduling solution that incorporates LLM inference characteristics into scheduling decisions at the scheduler level.
   120|
   121|---
   122|
   123|### 2. LangGraph State Graph Engine
   124|
   125|**File:** [`crates/langgraph-engine/src/graph.rs`](crates/langgraph-engine/src/graph.rs)
   126|
   127|LLM workflows are not linear — they require conditional branching, loops, parallel subtasks, and interrupt-resume semantics. Existing DAG engines (Airflow, Temporal) are either too heavy or lack LLM-specific interrupt-resume support.
   128|
   129|KIAS implements a complete LangGraph-style state graph engine with four edge types:
   130|
   131|```rust
   132|// crates/langgraph-engine/src/graph.rs
   133|pub enum EdgeType {
   134|    Direct { from: String, to: String },
   135|    Conditional { from: String, to: String, condition: EdgeCondition },
   136|    Router { from: String, router: RouterFn },
   137|    FanOut { from: String, targets: Vec<String>, join_node: String },
   138|}
   139|```
   140|
   141|**Parallel FanOut execution** spawns each branch in an independent `tokio::spawn` task, with state changes merged via last-write-wins strategy. **Checkpoint persistence** enables interrupt-resume semantics — `resume_from_checkpoint()` restores from the checkpoint node, not from the entry point.
   142|
   143|Build-time validation via `build()` detects unreachable nodes and missing entry points before runtime. `max_steps` guard prevents infinite loops in LLM-driven conditional cycles.
   144|
   145|---
   146|
   147|### 3. TypedState — Compile-Time Safe Reducer Mechanism
   148|
   149|**File:** [`crates/workflow-engine/src/typed_state.rs`](crates/workflow-engine/src/typed_state.rs)
   150|
   151|LangGraph's core abstraction is the TypedDict + Reducer pattern. In Python, this relies on type hints (runtime checks). KIAS leverages Rust's type system to guarantee state merge correctness at compile time.
   152|
   153|```rust
   154|// crates/workflow-engine/src/typed_state.rs
   155|pub trait ChannelReducer<T>: Send + Sync + 'static {
   156|    fn reduce(&self, current: T, incoming: T) -> T;
   157|    fn name(&self) -> &str;
   158|}
   159|```
   160|
   161|Five built-in reducers: `Replace`, `Append`, `Merge` (shallow HashMap merge), `KeepFirst`, `Sum`. Each channel erases its type to `Box<dyn Any>` but captures the original type's reducer via closure. On `update()`, type safety is restored through `downcast`.
   162|
   163|- **Compile-time safety:** Mismatched `T` and Reducer are rejected by the compiler
   164|- **Runtime flexibility:** Channel names are strings, supporting dynamic registration
   165|- **Concurrent branch safety:** FanOut branches merge state deterministically through reducers
   166|
   167|---
   168|
   169|### 4. Three-Layer Memory System
   170|
   171|**File:** [`crates/team-engine/src/memory.rs`](crates/team-engine/src/memory.rs)
   172|
   173|The core bottleneck in multi-agent collaboration is not communication — it's memory. Agents lose context after task completion, forcing redundant re-computation.
   174|
   175|KIAS implements a three-layer memory architecture:
   176|
   177|| Layer | Eviction Strategy | Purpose |
   178||-------|-------------------|---------|
   179|| **ShortTerm** | TTL + LRU | Current task context |
   180|| **LongTerm** | access_count + recency | Cross-task knowledge accumulation |
   181|| **Entity** | confidence + recency | Entity attribute memory with confidence scores |
   182|
   183|```rust
   184|// crates/team-engine/src/memory.rs
   185|pub struct MemoryManager {
   186|    pub short_term: Arc<RwLock<ShortTermMemory>>,
   187|    pub long_term: Arc<RwLock<LongTermMemory>>,
   188|    pub entity: Arc<RwLock<EntityMemory>>,
   189|}
   190|```
   191|
   192|`ContextBuilder` assembles context within a token budget (~4 chars/token heuristic), solving LLM context window overflow. All layers are thread-safe via `Arc<RwLock<>>` for concurrent multi-agent read/write. Entity Memory records confidence levels, allowing agents to distinguish between "known" and "inferred" facts.
   193|
   194|---
   195|
   196|### 5. Worker-Verifier Adversarial Quality Gate
   197|
   198|**File:** [`crates/team-engine/src/verifier.rs`](crates/team-engine/src/verifier.rs)
   199|
   200|Single-agent output quality is uncontrollable. Even with Chain of Thought, LLMs generate incorrect code, miss edge cases, and produce hallucinations.
   201|
   202|KIAS implements an adversarial Worker-Verifier mechanism:
   203|
   204|```rust
   205|// crates/team-engine/src/verifier.rs
   206|pub enum VerificationRule {
   207|    Contains(String),
   208|    NotContains(String),
   209|    MinLength(usize),
   210|    MaxLength(usize),
   211|    ValidJson,
   212|    Pattern(String),
   213|    ShellCheck(String),  // Execute shell commands for verification
   214|}
   215|```
   216|
   217|The `ShellCheck` rule runs actual test commands (e.g., `cargo test`, `python -m pytest`) during verification, elevating quality assurance from "looks correct" to "runs correctly." Verifier issues feed directly into the Worker's next iteration, forming a closed-loop improvement cycle.
   218|
   219|---
   220|
   221|### 6. Autonomy Gradient Controller
   222|
   223|**File:** [`crates/autonomy-controller/src/autonomy.rs`](crates/autonomy-controller/src/autonomy.rs), [`crates/autonomy-controller/src/ladder.rs`](crates/autonomy-controller/src/ladder.rs)
   224|
   225|Full autonomy is dangerous; full confirmation is inefficient. KIAS implements Codex CLI-style three-mode autonomy control with a complete decision pipeline:
   226|
   227|```
   228|Tool Policy Check → Rate Limit Check → Budget Check → Autonomy Level Judgment → Audit Log
   229|```
   230|
   231|```rust
   232|// crates/autonomy-controller/src/ladder.rs
   233|pub enum AutonomyLevel {
   234|    Suggest,    // Suggest only, no execution
   235|    AutoEdit,   // Write operations auto-execute, others require confirmation
   236|    FullAuto,   // Fully automatic, constrained by tool policies
   237|}
   238|```
   239|
   240|**Auto-promotion:** When an agent achieves consecutive successes above a threshold, it automatically promotes from `Suggest` to `AutoEdit`, reducing human intervention. `Forbidden` policy remains enforced even in `FullAuto` mode.
   241|
   242|---
   243|
   244|### 7. Goal-Driven Loop Engine
   245|
   246|**File:** [`crates/goal-engine/src/loop_runner.rs`](crates/goal-engine/src/loop_runner.rs)
   247|
   248|The "execute → evaluate → feedback → re-execute" pattern is ubiquitous in LLM applications. Most frameworks implement this as ad-hoc while loops in application code, lacking standardization, checkpoints, cancellation, and observability.
   249|
   250|KIAS abstracts this as `GoalLoopRunner` with separated executor-evaluator roles:
   251|
   252|```rust
   253|// crates/goal-engine/src/loop_runner.rs
   254|#[async_trait::async_trait]
   255|pub trait RoundExecutor: Send + Sync {
   256|    async fn execute_round(
   257|        &self, goal: &Goal, round: u32,
   258|        previous_feedback: Option<&EvaluationResult>,
   259|    ) -> KiasResult<String>;
   260|}
   261|```
   262|
   263|`GoalCancelToken` (based on `AtomicBool`) enables graceful external termination. `evaluation_history` tracks per-round evaluation results for convergence analysis. Checkpoint callbacks allow external systems to persist state after each round.
   264|
   265|---
   266|
   267|### 8. Descheduler — Cluster Rebalancing with PDB Constraints
   268|
   269|**File:** [`crates/scheduler/src/descheduler/engine.rs`](crates/scheduler/src/descheduler/engine.rs)
   270|
   271|Schedulers decide where to place work; they don't decide when to move it. Over time, clusters develop: underutilized nodes wasting resources, anti-affinity constraints violated, agent replicas concentrated on few nodes.
   272|
   273|KIAS implements a K8s Descheduler-style engine with three built-in strategies:
   274|
   275|| Strategy | File | Purpose |
   276||----------|------|---------|
   277|| `LowNodeUtilization` | `strategies/low_utilization.rs` | Detect underutilized nodes, migrate agents |
   278|| `DuplicateAgent` | `strategies/duplicates.rs` | Detect over-concentration of agent replicas |
   279|| `AntiAffinityViolation` | `strategies/anti_affinity.rs` | Detect and repair anti-affinity violations |
   280|
   281|**Pod Disruption Budget (PDB) constraints** ensure evictions don't cause service interruptions. Dry Run mode supports previewing eviction plans without execution.
   282|
   283|---
   284|
   285|### 9. A2A Protocol + MCP Sandbox
   286|
   287|**Files:** [`crates/common/src/a2a.rs`](crates/common/src/a2a.rs), [`crates/mcp-protocol/src/sandbox.rs`](crates/mcp-protocol/src/sandbox.rs)
   288|
   289|Agent interconnection requires standardized protocols; agent execution of external tools requires security isolation.
   290|
   291|**A2A (Agent-to-Agent)** implements Google's A2A protocol with complete data models:
   292|
   293|```rust
   294|// crates/common/src/a2a.rs
   295|pub struct AgentCard {
   296|    pub id: String,
   297|    pub capabilities: AgentCapabilities,
   298|    pub skills: Vec<AgentSkill>,
   299|    pub authentication: Option<AuthInfo>,
   300|}
   301|```
   302|
   303|Task lifecycle follows A2A spec: `Submitted → Working → InputRequired → Completed/Failed/Cancelled/Rejected`. Agent handoff supports 6 reasons: `CapabilityGap`, `LoadBalancing`, `Specialization`, `ErrorRecovery`, `HumanDirected`, `CostOptimization`.
   304|
   305|**MCP Sandbox** provides 5 isolation backends:
   306|
   307|```rust
   308|// crates/mcp-protocol/src/lib.rs
   309|pub use sandbox::{
   310|    FirecrackerSandboxBackend,  // Lightweight VM
   311|    GVisorSandboxBackend,       // User-space kernel
   312|    ProcessSandboxBackend,      // Process-level isolation
   313|    WasmSandboxBackend,         // WebAssembly
   314|    DockerSandboxBackend,       // Docker container
   315|};
   316|```
   317|
   318|Sandbox snapshots support state restore with `IsolationLevel` (Session / User / Global). Full MCP implementation includes OAuth 2.0, RBAC, circuit breaker, rate limiter, credential management, and hot-reload.
   319|
   320|---
   321|
   322|### 10. Data Masking Framework
   323|
   324|**File:** [`crates/common/src/data_mask.rs`](crates/common/src/data_mask.rs)
   325|
   326|LLM system logs frequently leak sensitive data: IP addresses, email addresses, JWT tokens. Traditional post-hoc masking or logging framework plugins are error-prone.
   327|
   328|KIAS implements **zero-trust masking** at the infrastructure layer:
   329|
   330|```rust
   331|// crates/common/src/data_mask.rs
   332|pub fn redact_log_message(msg: &str) -> String {
   333|    let mut result = msg.to_string();
   334|    result = redact_emails(&result);
   335|    result = redact_ips(&result);
   336|    result = redact_tokens(&result);  // Tokens ≥ 32 chars
   337|    result
   338|}
   339|```
   340|
   341|`SensitiveData` wrapper automatically masks on `Display` and `Serialize` — original values are never leaked. IPv4 detection uses a hand-written deterministic state machine (not regex), eliminating ReDoS risk. Since masking lives in the L0 `common` crate, all upstream components inherit it automatically.
   342|
   343|---
   344|
   345|### 11. InspirationStream — Builder-Thinker Dual-Flow Development
   346|
   347|**File:** [`crates/knowledge/src/inspiration_stream.rs`](crates/knowledge/src/inspiration_stream.rs)
   348|
   349|Traditional agent development follows a single-threaded execute → evaluate loop. The developer (or agent) builds, then checks if it works. This misses the opportunity for parallel insight discovery — while building, external knowledge sources may surface better approaches that could redirect effort before it's wasted.
   350|
   351|KIAS introduces a **Builder-Thinker dual-flow architecture** inspired by MiniMax Mavis's Worker-Verifier adversarial pattern, extended with a third **Thinker** role:
   352|
   353|```
   354|Builder (构建) ──→ 产出代码
   355|    ↕ 正向循环
   356|Thinker (发现) ──→ 从外部知识源抓取相关洞察，注入工作区
   357|    ↓
   358|Verifier (验证) ──→ 质量门禁
   359|```
   360|
   361|Three knowledge source types with **positive feedback weighting**:
   362|
   363|```rust
   364|// crates/knowledge/src/inspiration_stream.rs
   365|pub enum SourceType {
   366|    Paper,      // arXiv, conference proceedings
   367|    Trending,   // GitHub trending, HN, Reddit
   368|    Benchmark,  // Performance comparisons, competitor analysis
   369|}
   370|```
   371|
   372|**Positive feedback loop** — sources that produce adopted insights gain weight (up to 3.0×), sources that produce ignored insights lose weight (down to 0.3×). No manual tuning; the system learns which sources are valuable over time.
   373|
   374|```rust
   375|if adopted {
   376|    source.reliability = (source.reliability * 1.05).min(3.0);  // +5%
   377|} else {
   378|    source.reliability = (source.reliability * 0.99).max(0.3);  // -1%
   379|}
   380|```
   381|
   382|**Relevance scoring** uses keyword overlap between insight tags and the current task context. `max_per_cycle` prevents insight flooding. `min_relevance` filters noise. All insights are persisted with adopt/dismiss outcomes for the DreamConsolidator to learn from during sleep cycles.
   383|
   384|This is the mechanism that enabled KIAS to absorb ideas from AgenticRAG, Claude Code's memory architecture, AgentScope's Workspace concept, and MiniMax Mavis's Worker-Verifier pattern — all discovered and integrated during active development, not in a separate research phase.
   385|
   386|---
   387|
   388|## Node-Level Error Handling
   389|
   390|KIAS provides fault tolerance at every layer of the stack:
   391|
   392|```
   393|Request → Agent → Failure → DLQ → Exponential Backoff Retry → Circuit Breaker → Auto Recovery
   394|```
   395|
   396|| Mechanism | Implementation | File |
   397||-----------|---------------|------|
   398|| Dead Letter Queue | Failed tasks queued for retry | `crates/controller/src/recovery.rs` |
   399|| Circuit Breaker | Consecutive failures trigger open state | `crates/controller/src/health.rs` |
   400|| Exponential Backoff | Configurable retry with jitter | `crates/controller/src/recovery.rs` |
   401|| Health Check Loop | Continuous node liveness probing | `crates/controller/src/heartbeat.rs` |
   402|| Saga Compensation | Workflow rollback on partial failure | `crates/workflow-engine/src/engine.rs` |
   403|
   404|---
   405|
   406|## Supported Models
   407|
   408|### Cloud APIs
   409|
   410|| Provider | Latest Model | Context | Input ($/1M tokens) | Output ($/1M tokens) |
   411||----------|-------------|---------|---------------------|---------------------|
   412|| **OpenAI** | GPT-5.5 | 1,050K | $5.00 | $30.00 |
   413|| **OpenAI** | GPT-5 | 400K | $1.25 | $10.00 |
   414|| **OpenAI** | GPT-5-mini | 400K | $0.25 | $2.00 |
   415|| **OpenAI** | o4-mini | 200K | $1.10 | $4.40 |
   416|| **Anthropic** | Claude Opus 4.7 | 1,000K | $5.00 | $25.00 |
   417|| **Anthropic** | Claude Sonnet 4.6 | 1,000K | $3.00 | $15.00 |
   418|| **Anthropic** | Claude 3.5 Haiku | 200K | $0.80 | $4.00 |
   419|| **Google** | Gemini 3.1 Pro | 1,048K | $2.00 | $12.00 |
   420|| **Google** | Gemini 2.5 Pro | 1,048K | $1.25 | $10.00 |
   421|| **Google** | Gemini 2.5 Flash | 1,048K | $0.30 | $2.50 |
   422|| **DeepSeek** | DeepSeek-V4 Pro | 1,048K | $0.43 | $0.87 |
   423|| **DeepSeek** | DeepSeek-V4 Flash | 1,048K | $0.11 | $0.22 |
   424|| **DeepSeek** | DeepSeek-R1 | 163K | $0.70 | $2.50 |
   425|| **Qwen** | Qwen3-Coder | 1,048K | $0.22 | $1.80 |
   426|| **Qwen** | Qwen3-235B | 262K | $0.07 | $0.10 |
   427|| **Mistral** | Mistral Large (2512) | 262K | $0.50 | $1.50 |
   428|| **Mistral** | Codestral (2508) | 256K | $0.30 | $0.90 |
   429|| **Meta** | Llama 4 Scout | 10,000K | $0.08 | $0.30 |
   430|| **Meta** | Llama 4 Maverick | 1,048K | $0.15 | $0.60 |
   431|
   432|> Pricing sourced from OpenRouter API (May 2026).
   433|
   434|### Local Models
   435|
   436|See [Local Model Comparison Guide](docs/local-model-comparison.md) for specifications, benchmarks, GPU requirements, and deployment recommendations across 16 open-source models.
   437|
   438|| Runtime | Install | Use Case |
   439||---------|---------|----------|
   440|| **Ollama** | `curl -fsSL https://ollama.com/install.sh \| sh` | Development & testing |
   441|| **vLLM** | `pip install vllm` | Production high-throughput |
   442|| **llama.cpp** | GitHub release | Edge devices, CPU inference |
   443|
   444|---
   445|
   446|## System Requirements
   447|
   448|### Operating Systems
   449|
   450|| Platform | Architecture | Status | Package |
   451||----------|-------------|--------|---------|
   452|| **Ubuntu 22.04+** | x86_64 / aarch64 | ✅ Primary | `.deb` |
   453|| **Debian 12+** | x86_64 / aarch64 | ✅ Supported | `.deb` |
   454|| **CentOS 9 / RHEL 9** | x86_64 / aarch64 | ✅ Supported | `.rpm` |
   455|| **Fedora 40+** | x86_64 / aarch64 | ✅ Supported | `.rpm` |
   456|| **Alpine 3.18+** | x86_64 / aarch64 | ✅ Supported | Static binary |
   457|| **macOS 13+** | Apple Silicon (M1-M4) / x86_64 | ✅ Supported | Homebrew |
   458|| **Windows 11** | x86_64 | ⚠️ Via WSL2 only | `.msi` (WSL2) |
   459|| **Docker** | x86_64 / aarch64 | ✅ Official image | `ghcr.io/andy-ckm/kias` |
   460|
   461|### Hardware
   462|
   463|| Component | Minimum | Recommended |
   464||-----------|---------|-------------|
   465|| **CPU** | 2 cores | 4+ cores |
   466|| **RAM** | 2 GB | 4+ GB |
   467|| **Disk** | 500 MB (binary) | 2+ GB (with SQLite data) |
   468|| **Network** | Outbound HTTPS | Required for LLM API calls |
   469|
   470|> KIAS is a single binary (~30 MB). No runtime dependencies (no JVM, no Node, no Python).
   471|> SQLite is embedded. No external database required for single-node deployment.
   472|
   473|### Dependencies
   474|
   475|| Dependency | Required | Notes |
   476||-----------|----------|-------|
   477|| **Rust 1.85+** | Build only | Stable channel |
   478|| **SQLite 3.35+** | Runtime (embedded) | Auto-included via `rusqlite` bundled feature |
   479|| **OpenSSL / rustls** | Runtime | TLS for LLM API calls (rustls by default) |
   480|
   481|### Deployment Modes
   482|
   483|#### Ubuntu / Debian (`.deb`)
   484|
   485|```bash
   486|# Download and install
   487|curl -LO https://github.com/Andy-ckm/KIAS/releases/latest/download/kias-amd64.deb
   488|sudo dpkg -i kias-amd64.deb
   489|
   490|# Start as systemd service
   491|sudo systemctl enable --now kias
   492|sudo systemctl status kias
   493|
   494|# Logs
   495|journalctl -u kias -f
   496|```
   497|
   498|#### CentOS / RHEL / Fedora (`.rpm`)
   499|
   500|```bash
   501|