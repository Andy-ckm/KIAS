     1|<p align="center">
     2|  <a href="https://github.com/Andy-ckm/KIAS/blob/main/LICENSE">
     3|    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
     4|  </a>
     5|  <a href="https://github.com/Andy-ckm/KIAS/actions">
     6|    <img src="https://img.shields.io/badge/tests-1637%20passed-brightgreen.svg" alt="Tests">
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
    34|  <a href="#supported-models">Supported Models</a>
    35|</p>
    36|
    37|---
    38|
    39|## Overview
    40|
    41|KIAS is a Rust-based AI Agent cluster scheduling system that applies Kubernetes control-plane architecture to LLM agent orchestration. It addresses the gap between prototype agent scripts and production-ready agent infrastructure: state persistence, crash recovery, multi-agent coordination, cache-aware scheduling, sandboxed execution, and observability.
    42|
    43|**Key numbers:**
    44|
    45|| Metric | Value |
    46||--------|-------|
    47|| Rust Crates | 21 |
    48|| Lines of Code | 85,000+ |
    49|| Scheduling Algorithms | 7 (including GPU-Aware, Edge) |
    50|| MCP Sandbox Backends | 5 (Docker / Firecracker / gVisor / Wasm / Process) |
    51|| Test Coverage | `#[cfg(test)]` module in every crate |
    52|
    53|---
    54|
    55|## Architecture
    56|
    57|```
    58|┌─────────────────────────────────────────────────────────────────┐
    59|│                         API Server (axum)                        │
    60|│                   REST + gRPC + WebSocket + mTLS                 │
    61|├─────────┬──────────┬──────────┬──────────┬───────────────────────┤
    62|│Scheduler│Controller│Workflow  │  Team    │  Goal    │ Autonomy   │
    63|│  Engine │          │  Engine  │  Engine  │  Engine  │ Controller │
    64|│ 7 algos │Heartbeat │ DAG exec │ OVW      │ Goal loop│ 3-level    │
    65|├─────────┴──────────┴──────────┴──────────┴──────────┴───────────┤
    66|│  LangGraph Engine    │   MCP Protocol    │   Data Store (SQLite) │
    67|│  State graph+FanOut  │   JSON-RPC+Sandbox│   Vector+Prefix Cache │
    68|├──────────────────────┴───────────────────┴───────────────────────┤
    69|│                    Common (L0): Error / Config / A2A / Masking    │
    70|└─────────────────────────────────────────────────────────────────┘
    71|```
    72|
    73|**Layered dependency model** (enforced by `make lint-arch`):
    74|
    75|```
    76|L0: common                    ← Base types, errors, config
    77|L1: data-store                ← SQLite persistence layer
    78|L2: scheduler, controller, workflow-engine, team-engine, ...
    79|L3: api-server, kias-main
    80|```
    81|
    82|Strict unidirectional dependencies. No cross-layer imports.
    83|
    84|---
    85|
    86|## Core Innovations
    87|
    88|### 1. Cache-Aware Scheduling
    89|
    90|**File:** [`crates/scheduler/src/algorithms/cache_aware.rs`](crates/scheduler/src/algorithms/cache_aware.rs)
    91|
    92|Traditional schedulers (Round Robin, Least Loaded) are blind to LLM inference characteristics — they don't know whether a node has already cached a specific system prompt's KV Cache. A cache miss means recomputing the entire prefix, wasting ~90% of GPU compute.
    93|
    94|KIAS introduces **DeepSeek-style Prefix Caching** into the scheduling decision layer:
    95|
    96|```rust
    97|// crates/scheduler/src/algorithms/cache_aware.rs
    98|fn cache_aware_score(
    99|    node: &Node, agent: &Agent,
   100|    cache_info: Option<&NodeCacheInfo>, cache_weight: f64,
   101|) -> f64 {
   102|    let cache_score = if let (Some(info), Some(prefix_hash)) = (cache_info, agent.system_prompt_hash) {
   103|        if info.cached_prefixes.contains(&prefix_hash) { 1.0 } else { 0.0 }
   104|    } else { 0.0 };
   105|    let load_score = 1.0 - node.load_factor();
   106|    cache_weight * cache_score + (1.0 - cache_weight) * load_score
   107|}
   108|```
   109|
   110|- **Fast path:** If a node has a matching cached prefix, route directly (score = 1.0)
   111|- **Weighted scoring:** `cache_weight` parameter (0.0 = pure load balancing, 1.0 = pure cache priority)
   112|- **Concurrent-safe:** `Arc<DashMap>` for lock-free concurrent cache map access
   113|
   114|This is the only scheduling solution that incorporates LLM inference characteristics into scheduling decisions at the scheduler level.
   115|
   116|---
   117|
   118|### 2. LangGraph State Graph Engine
   119|
   120|**File:** [`crates/langgraph-engine/src/graph.rs`](crates/langgraph-engine/src/graph.rs)
   121|
   122|LLM workflows are not linear — they require conditional branching, loops, parallel subtasks, and interrupt-resume semantics. Existing DAG engines (Airflow, Temporal) are either too heavy or lack LLM-specific interrupt-resume support.
   123|
   124|KIAS implements a complete LangGraph-style state graph engine with four edge types:
   125|
   126|```rust
   127|// crates/langgraph-engine/src/graph.rs
   128|pub enum EdgeType {
   129|    Direct { from: String, to: String },
   130|    Conditional { from: String, to: String, condition: EdgeCondition },
   131|    Router { from: String, router: RouterFn },
   132|    FanOut { from: String, targets: Vec<String>, join_node: String },
   133|}
   134|```
   135|
   136|**Parallel FanOut execution** spawns each branch in an independent `tokio::spawn` task, with state changes merged via last-write-wins strategy. **Checkpoint persistence** enables interrupt-resume semantics — `resume_from_checkpoint()` restores from the checkpoint node, not from the entry point.
   137|
   138|Build-time validation via `build()` detects unreachable nodes and missing entry points before runtime. `max_steps` guard prevents infinite loops in LLM-driven conditional cycles.
   139|
   140|---
   141|
   142|### 3. TypedState — Compile-Time Safe Reducer Mechanism
   143|
   144|**File:** [`crates/workflow-engine/src/typed_state.rs`](crates/workflow-engine/src/typed_state.rs)
   145|
   146|LangGraph's core abstraction is the TypedDict + Reducer pattern. In Python, this relies on type hints (runtime checks). KIAS leverages Rust's type system to guarantee state merge correctness at compile time.
   147|
   148|```rust
   149|// crates/workflow-engine/src/typed_state.rs
   150|pub trait ChannelReducer<T>: Send + Sync + 'static {
   151|    fn reduce(&self, current: T, incoming: T) -> T;
   152|    fn name(&self) -> &str;
   153|}
   154|```
   155|
   156|Five built-in reducers: `Replace`, `Append`, `Merge` (shallow HashMap merge), `KeepFirst`, `Sum`. Each channel erases its type to `Box<dyn Any>` but captures the original type's reducer via closure. On `update()`, type safety is restored through `downcast`.
   157|
   158|- **Compile-time safety:** Mismatched `T` and Reducer are rejected by the compiler
   159|- **Runtime flexibility:** Channel names are strings, supporting dynamic registration
   160|- **Concurrent branch safety:** FanOut branches merge state deterministically through reducers
   161|
   162|---
   163|
   164|### 4. Three-Layer Memory System
   165|
   166|**File:** [`crates/team-engine/src/memory.rs`](crates/team-engine/src/memory.rs)
   167|
   168|The core bottleneck in multi-agent collaboration is not communication — it's memory. Agents lose context after task completion, forcing redundant re-computation.
   169|
   170|KIAS implements a three-layer memory architecture:
   171|
   172|| Layer | Eviction Strategy | Purpose |
   173||-------|-------------------|---------|
   174|| **ShortTerm** | TTL + LRU | Current task context |
   175|| **LongTerm** | access_count + recency | Cross-task knowledge accumulation |
   176|| **Entity** | confidence + recency | Entity attribute memory with confidence scores |
   177|
   178|```rust
   179|// crates/team-engine/src/memory.rs
   180|pub struct MemoryManager {
   181|    pub short_term: Arc<RwLock<ShortTermMemory>>,
   182|    pub long_term: Arc<RwLock<LongTermMemory>>,
   183|    pub entity: Arc<RwLock<EntityMemory>>,
   184|}
   185|```
   186|
   187|`ContextBuilder` assembles context within a token budget (~4 chars/token heuristic), solving LLM context window overflow. All layers are thread-safe via `Arc<RwLock<>>` for concurrent multi-agent read/write. Entity Memory records confidence levels, allowing agents to distinguish between "known" and "inferred" facts.
   188|
   189|---
   190|
   191|### 5. Worker-Verifier Adversarial Quality Gate
   192|
   193|**File:** [`crates/team-engine/src/verifier.rs`](crates/team-engine/src/verifier.rs)
   194|
   195|Single-agent output quality is uncontrollable. Even with Chain of Thought, LLMs generate incorrect code, miss edge cases, and produce hallucinations.
   196|
   197|KIAS implements an adversarial Worker-Verifier mechanism:
   198|
   199|```rust
   200|// crates/team-engine/src/verifier.rs
   201|pub enum VerificationRule {
   202|    Contains(String),
   203|    NotContains(String),
   204|    MinLength(usize),
   205|    MaxLength(usize),
   206|    ValidJson,
   207|    Pattern(String),
   208|    ShellCheck(String),  // Execute shell commands for verification
   209|}
   210|```
   211|
   212|The `ShellCheck` rule runs actual test commands (e.g., `cargo test`, `python -m pytest`) during verification, elevating quality assurance from "looks correct" to "runs correctly." Verifier issues feed directly into the Worker's next iteration, forming a closed-loop improvement cycle.
   213|
   214|---
   215|
   216|### 6. Autonomy Gradient Controller
   217|
   218|**File:** [`crates/autonomy-controller/src/autonomy.rs`](crates/autonomy-controller/src/autonomy.rs), [`crates/autonomy-controller/src/ladder.rs`](crates/autonomy-controller/src/ladder.rs)
   219|
   220|Full autonomy is dangerous; full confirmation is inefficient. KIAS implements Codex CLI-style three-mode autonomy control with a complete decision pipeline:
   221|
   222|```
   223|Tool Policy Check → Rate Limit Check → Budget Check → Autonomy Level Judgment → Audit Log
   224|```
   225|
   226|```rust
   227|// crates/autonomy-controller/src/ladder.rs
   228|pub enum AutonomyLevel {
   229|    Suggest,    // Suggest only, no execution
   230|    AutoEdit,   // Write operations auto-execute, others require confirmation
   231|    FullAuto,   // Fully automatic, constrained by tool policies
   232|}
   233|```
   234|
   235|**Auto-promotion:** When an agent achieves consecutive successes above a threshold, it automatically promotes from `Suggest` to `AutoEdit`, reducing human intervention. `Forbidden` policy remains enforced even in `FullAuto` mode.
   236|
   237|---
   238|
   239|### 7. Goal-Driven Loop Engine
   240|
   241|**File:** [`crates/goal-engine/src/loop_runner.rs`](crates/goal-engine/src/loop_runner.rs)
   242|
   243|The "execute → evaluate → feedback → re-execute" pattern is ubiquitous in LLM applications. Most frameworks implement this as ad-hoc while loops in application code, lacking standardization, checkpoints, cancellation, and observability.
   244|
   245|KIAS abstracts this as `GoalLoopRunner` with separated executor-evaluator roles:
   246|
   247|```rust
   248|// crates/goal-engine/src/loop_runner.rs
   249|#[async_trait::async_trait]
   250|pub trait RoundExecutor: Send + Sync {
   251|    async fn execute_round(
   252|        &self, goal: &Goal, round: u32,
   253|        previous_feedback: Option<&EvaluationResult>,
   254|    ) -> KiasResult<String>;
   255|}
   256|```
   257|
   258|`GoalCancelToken` (based on `AtomicBool`) enables graceful external termination. `evaluation_history` tracks per-round evaluation results for convergence analysis. Checkpoint callbacks allow external systems to persist state after each round.
   259|
   260|---
   261|
   262|### 8. Descheduler — Cluster Rebalancing with PDB Constraints
   263|
   264|**File:** [`crates/scheduler/src/descheduler/engine.rs`](crates/scheduler/src/descheduler/engine.rs)
   265|
   266|Schedulers decide where to place work; they don't decide when to move it. Over time, clusters develop: underutilized nodes wasting resources, anti-affinity constraints violated, agent replicas concentrated on few nodes.
   267|
   268|KIAS implements a K8s Descheduler-style engine with three built-in strategies:
   269|
   270|| Strategy | File | Purpose |
   271||----------|------|---------|
   272|| `LowNodeUtilization` | `strategies/low_utilization.rs` | Detect underutilized nodes, migrate agents |
   273|| `DuplicateAgent` | `strategies/duplicates.rs` | Detect over-concentration of agent replicas |
   274|| `AntiAffinityViolation` | `strategies/anti_affinity.rs` | Detect and repair anti-affinity violations |
   275|
   276|**Pod Disruption Budget (PDB) constraints** ensure evictions don't cause service interruptions. Dry Run mode supports previewing eviction plans without execution.
   277|
   278|---
   279|
   280|### 9. A2A Protocol + MCP Sandbox
   281|
   282|**Files:** [`crates/common/src/a2a.rs`](crates/common/src/a2a.rs), [`crates/mcp-protocol/src/sandbox.rs`](crates/mcp-protocol/src/sandbox.rs)
   283|
   284|Agent interconnection requires standardized protocols; agent execution of external tools requires security isolation.
   285|
   286|**A2A (Agent-to-Agent)** implements Google's A2A protocol with complete data models:
   287|
   288|```rust
   289|// crates/common/src/a2a.rs
   290|pub struct AgentCard {
   291|    pub id: String,
   292|    pub capabilities: AgentCapabilities,
   293|    pub skills: Vec<AgentSkill>,
   294|    pub authentication: Option<AuthInfo>,
   295|}
   296|```
   297|
   298|Task lifecycle follows A2A spec: `Submitted → Working → InputRequired → Completed/Failed/Cancelled/Rejected`. Agent handoff supports 6 reasons: `CapabilityGap`, `LoadBalancing`, `Specialization`, `ErrorRecovery`, `HumanDirected`, `CostOptimization`.
   299|
   300|**MCP Sandbox** provides 5 isolation backends:
   301|
   302|```rust
   303|// crates/mcp-protocol/src/lib.rs
   304|pub use sandbox::{
   305|    FirecrackerSandboxBackend,  // Lightweight VM
   306|    GVisorSandboxBackend,       // User-space kernel
   307|    ProcessSandboxBackend,      // Process-level isolation
   308|    WasmSandboxBackend,         // WebAssembly
   309|    DockerSandboxBackend,       // Docker container
   310|};
   311|```
   312|
   313|Sandbox snapshots support state restore with `IsolationLevel` (Session / User / Global). Full MCP implementation includes OAuth 2.0, RBAC, circuit breaker, rate limiter, credential management, and hot-reload.
   314|
   315|---
   316|
   317|### 10. Data Masking Framework
   318|
   319|**File:** [`crates/common/src/data_mask.rs`](crates/common/src/data_mask.rs)
   320|
   321|LLM system logs frequently leak sensitive data: IP addresses, email addresses, JWT tokens. Traditional post-hoc masking or logging framework plugins are error-prone.
   322|
   323|KIAS implements **zero-trust masking** at the infrastructure layer:
   324|
   325|```rust
   326|// crates/common/src/data_mask.rs
   327|pub fn redact_log_message(msg: &str) -> String {
   328|    let mut result = msg.to_string();
   329|    result = redact_emails(&result);
   330|    result = redact_ips(&result);
   331|    result = redact_tokens(&result);  // Tokens ≥ 32 chars
   332|    result
   333|}
   334|```
   335|
   336|`SensitiveData` wrapper automatically masks on `Display` and `Serialize` — original values are never leaked. IPv4 detection uses a hand-written deterministic state machine (not regex), eliminating ReDoS risk. Since masking lives in the L0 `common` crate, all upstream components inherit it automatically.
   337|
   338|---
   339|
   340|## Node-Level Error Handling
   341|
   342|KIAS provides fault tolerance at every layer of the stack:
   343|
   344|```
   345|Request → Agent → Failure → DLQ → Exponential Backoff Retry → Circuit Breaker → Auto Recovery
   346|```
   347|
   348|| Mechanism | Implementation | File |
   349||-----------|---------------|------|
   350|| Dead Letter Queue | Failed tasks queued for retry | `crates/controller/src/recovery.rs` |
   351|| Circuit Breaker | Consecutive failures trigger open state | `crates/controller/src/health.rs` |
   352|| Exponential Backoff | Configurable retry with jitter | `crates/controller/src/recovery.rs` |
   353|| Health Check Loop | Continuous node liveness probing | `crates/controller/src/heartbeat.rs` |
   354|| Saga Compensation | Workflow rollback on partial failure | `crates/workflow-engine/src/engine.rs` |
   355|
   356|---
   357|
   358|## Supported Models
   359|
   360|### Cloud APIs
   361|
   362|| Provider | Latest Model | Context | Input ($/1M tokens) | Output ($/1M tokens) |
   363||----------|-------------|---------|---------------------|---------------------|
   364|| **OpenAI** | GPT-5.5 | 1,050K | $5.00 | $30.00 |
   365|| **OpenAI** | GPT-5 | 400K | $1.25 | $10.00 |
   366|| **OpenAI** | GPT-5-mini | 400K | $0.25 | $2.00 |
   367|| **OpenAI** | o4-mini | 200K | $1.10 | $4.40 |
   368|| **Anthropic** | Claude Opus 4.7 | 1,000K | $5.00 | $25.00 |
   369|| **Anthropic** | Claude Sonnet 4.6 | 1,000K | $3.00 | $15.00 |
   370|| **Anthropic** | Claude 3.5 Haiku | 200K | $0.80 | $4.00 |
   371|| **Google** | Gemini 3.1 Pro | 1,048K | $2.00 | $12.00 |
   372|| **Google** | Gemini 2.5 Pro | 1,048K | $1.25 | $10.00 |
   373|| **Google** | Gemini 2.5 Flash | 1,048K | $0.30 | $2.50 |
   374|| **DeepSeek** | DeepSeek-V4 Pro | 1,048K | $0.43 | $0.87 |
   375|| **DeepSeek** | DeepSeek-V4 Flash | 1,048K | $0.11 | $0.22 |
   376|| **DeepSeek** | DeepSeek-R1 | 163K | $0.70 | $2.50 |
   377|| **Qwen** | Qwen3-Coder | 1,048K | $0.22 | $1.80 |
   378|| **Qwen** | Qwen3-235B | 262K | $0.07 | $0.10 |
   379|| **Mistral** | Mistral Large (2512) | 262K | $0.50 | $1.50 |
   380|| **Mistral** | Codestral (2508) | 256K | $0.30 | $0.90 |
   381|| **Meta** | Llama 4 Scout | 10,000K | $0.08 | $0.30 |
   382|| **Meta** | Llama 4 Maverick | 1,048K | $0.15 | $0.60 |
   383|
   384|> Pricing sourced from OpenRouter API (May 2026).
   385|
   386|### Local Models
   387|
   388|See [Local Model Comparison Guide](docs/local-model-comparison.md) for specifications, benchmarks, GPU requirements, and deployment recommendations across 16 open-source models.
   389|
   390|| Runtime | Install | Use Case |
   391||---------|---------|----------|
   392|| **Ollama** | `curl -fsSL https://ollama.com/install.sh \| sh` | Development & testing |
   393|| **vLLM** | `pip install vllm` | Production high-throughput |
   394|| **llama.cpp** | GitHub release | Edge devices, CPU inference |
   395|
   396|---
   397|
   398|## Quickstart
   399|
   400|### Install
   401|
   402|```bash
   403|curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh
   404|```
   405|
   406|### Configure
   407|
   408|```bash
   409|kias config init
   410|```
   411|
   412|Edit `~/.kias/config.toml`:
   413|
   414|```toml
   415|[model]
   416|provider = "openai"
   417|api_key = "sk-your-key"
   418|model = "gpt-5.5"
   419|
   420|# Local model
   421|# provider = "ollama"
   422|# endpoint = "http://localhost:11434"
   423|# model = "qwen3:32b"
   424|```
   425|
   426|### Start
   427|
   428|```bash
   429|kias server start
   430|# Dashboard: http://localhost:8080
   431|```
   432|
   433|### Create an Agent
   434|
   435|```yaml
   436|# my-agent.yaml
   437|name: my-agent
   438|description: Code review assistant
   439|model: gpt-5.5
   440|system_prompt: You are a professional code review engineer.
   441|```
   442|
   443|```bash
   444|kias agent create --file my-agent.yaml
   445|kias agent invoke --name my-agent --text "Review this code"
   446|```
   447|
   448|---
   449|
   450|## CLI Reference
   451|
   452|```bash
   453|# Agent management
   454|kias agent list                            # List all agents
   455|kias agent create --file agent.yaml        # Create agent
   456|kias agent invoke --name my --text "hello" # Invoke agent
   457|kias agent status --name my                # Check status
   458|
   459|# Service management
   460|kias server start                          # Start server
   461|kias server start --daemon                 # Start in background
   462|kias server stop                           # Stop server
   463|
   464|# Development
   465|make build                                 # Build all crates
   466|make test                                  # Run tests
   467|make lint                                  # Clippy checks
   468|make lint-arch                             # Layer dependency check
   469|make bench                                 # Criterion benchmarks
   470|```
   471|
   472|---
   473|
   474|## Hardware Requirements
   475|
   476|### KIAS Framework
   477|
   478|| Profile | CPU | Memory | Disk |
   479||---------|-----|--------|------|
   480|| Minimum (dev) | 2 cores | 4 GB | 10 GB |
   481|| Recommended (prod) | 4+ cores | 8+ GB | 50+ GB SSD |
   482|
   483|### Local Model GPU Requirements
   484|
   485|| Model Size | VRAM | Example Models |
   486||-----------|------|---------------|
   487|| 1B–3B | 3–6 GB | Phi-3-mini, Qwen3-8B |
   488|| 7B–14B | 8–16 GB | Qwen3-14B, Llama 4 Scout (quantized) |
   489|| 30B–40B | 24–40 GB | Qwen3-32B |
   490|| 70B+ | 48–80 GB | Qwen3-235B (INT4) |
   491|
   492|---
   493|
   494|## Project Structure
   495|
   496|```
   497|kias/
   498|├── crates/
   499|│   ├── common/              # Shared types, errors, A2A protocol, data masking
   500|│   ├── scheduler/           # 7 scheduling algorithms + descheduler
   501|│   ├── controller/          # Agent lifecycle, heartbeat, recovery
   502|│   ├── langgraph-engine/    # State graph engine (FanOut, checkpoints, interrupt-resume)
   503|│   ├── workflow-engine/     # DAG workflow engine, TypedState reducers
   504|│   ├── team-engine/         # Multi-agent orchestration, Worker-Verifier, memory
   505|│   ├── goal-engine/         # Goal-driven loop runner
   506|│   ├── autonomy-controller/ # 3-level autonomy control with auto-promotion
   507|│   ├── mcp-protocol/        # MCP protocol + 5 sandbox backends
   508|│   ├── model-router/        # Multi-provider model routing
   509|│   ├── data-store/          # SQLite persistence, vector store, prefix cache
   510|│   ├── api-server/          # REST + gRPC + WebSocket API
   511|│   ├── executor/            # Task execution framework
   512|│   ├── cache/               # LRU + prefix caching
   513|│   ├── monitor/             # Telemetry + metrics collection
   514|│   ├── knowledge/           # Knowledge graph
   515|│   ├── skills/              # Skill registry
   516|│   ├── kias-cli/            # Command-line tool
   517|│   ├── kias-main/           # Main service orchestration
   518|│   └── benchmarks/          # Criterion performance benchmarks
   519|├── dashboard/               # React web console
   520|├── config/                  # Configuration files
   521|├── docs/                    # Documentation
   522|└── scripts/                 # Build, startup, and check scripts
   523|```
   524|
   525|---
   526|
   527|## Tech Stack
   528|
   529|| Layer | Technology | Rationale |
   530||-------|-----------|-----------|
   531|| Async Runtime | tokio | Rust async ecosystem standard |
   532|| Web Framework | axum | Type-safe middleware system |
   533|| Concurrent Map | DashMap | Lock-free, suited for high-frequency R/W |
   534|| Serialization | serde | Zero-cost abstractions |
   535|| Configuration | config crate | TOML/YAML/JSON + env var override |
   536|| Logging | tracing | Structured logging with span support |
   537|| Error Handling | thiserror + anyhow | Business errors via thiserror, internal via anyhow |
   538|
   539|---
   540|
   541|## Comparison
   542|
   543|| Feature | KIAS | LangGraph (Python) | CrewAI | AutoGen |
   544||---------|------|--------------------|--------|---------|
   545|| **Language** | Rust | Python | Python | Python |
   546|| **State Persistence** | SQLite + Checkpoint | In-memory (needs external store) | None | None |
   547|| **Error Recovery** | DLQ + Circuit Breaker + Saga | Limited | None | None |
   548|| **Multi-tenancy** | Resource quotas + Sandbox | None | None | None |
   549|| **Observability** | Prometheus + WebSocket | None built-in | None | None |
   550|| **Cache-Aware Scheduling** | KV Cache hit-rate aware | None | None | None |
   551|| **Autonomy Control** | 3-level gradient with auto-promotion | None | None | None |
   552|| **Concurrency Model** | Tokio async (10K+ concurrent) | Single-threaded | Single-threaded | Single-threaded |
   553|| **Sandbox** | 5 backends | None | None | None |
   554|
   555|---
   556|
   557|## Contributing
   558|
   559|See [CONTRIBUTING.md](CONTRIBUTING.md).
   560|
   561|1. Fork the repository
   562|2. Create a feature branch (`git checkout -b feature/amazing`)
   563|3. Run tests (`cargo test --workspace`)
   564|4. Run architecture lint (`make lint-arch`)
   565|5. Commit changes (`git commit -m 'Add amazing feature'`)
   566|6. Push branch (`git push origin feature/amazing`)
   567|7. Open a Pull Request
   568|
   569|---
   570|
   571|## License
   572|
   573|Copyright © 2024 KIAS Contributors
   574|
   575|Licensed under the **MIT License**. See [LICENSE](LICENSE) for details.
   576|

### 7. AgenticRAG — 多轮迭代检索

**参考:** Microsoft AgenticRAG (arxiv 2605.05538)

传统RAG是一次检索就交给LLM。AgenticRAG给模型配备四层工具，让模型自主决定搜什么、看哪部分：

| 工具 | 功能 | 论文参数 |
|------|------|----------|
| Search | 全局搜索，多查询改写 | max 5 queries, 10 results |
| Find | 文档内关键词/语义搜索 | 2 matches per pattern |
| Open | 窗口化全内容阅读 | 1800 lines per window |
| Summarize | 上下文压缩，保留引用 | 90% token预警 |

**核心发现:** 5.9×检索提升来自"单次→多轮"，不是更好的embedding。

```rust
// crates/knowledge/src/agentic_rag.rs
let engine = AgenticRAGEngine::with_rules(store)?;
let result = engine.retrieve("revenue analysis").await;
// result.iterations, result.references, result.metrics
```

### 8. 七层记忆架构

**参考:** Claude Code Memory Architecture + AgentScope Harness

| 层级 | 机制 | 成本 |
|------|------|------|
| L1 | 工具结果磁盘存储 + 2KB预览 | 极低 |
| L3 | 会话记忆（实时结构化笔记） | 零 |
| L6 | 做梦机制（跨会话记忆巩固） | 低 |

```rust
// crates/knowledge/src/memory_layers.rs
let store = ToolResultStore::new(config);      // L1
let session = SessionMemoryManager::new(config); // L3
let dreamer = DreamConsolidator::new(config);    // L6
```

### 9. 自循环闭环

KIAS用KIAS开发KIAS，形成飞轮：

```
Detect → Analyze → Plan → Generate → Verify → Deploy → Learn
  ↑                                                      |
  └──────────────────────────────────────────────────────┘
```

7个模块：detector, analyzer, planner, codegen, verifier, deployer, learner

## 