1|<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS/actions">
    <img src="https://img.shields.io/badge/tests-5241%2B%20passed-brightgreen.svg" alt="Tests">
  </a>
  <a href="https://www.rust-lang.org">
    <img src="https://img.shields.io/badge/Rust-1.95-orange.svg?logo=rust" alt="Rust">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/badge/crates-36-purple.svg" alt="Crates">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/badge/LOC-143K%2B-blue.svg" alt="Lines of Code">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/github/stars/Andy-ckm/AgentGuard?style=social" alt="Stars">
  </a>
</p>

<p align="center">
  <img src="docs/logo/agentguard-logo.svg" alt="AgentGuard" width="520">
</p>

<h1 align="center">AgentGuard</h1>
<p align="center"><strong>让AI Agent可追溯、透明、可控</strong></p>
<p align="center">Production-grade AI Agent compliance & governance system built in Rust</p>

<p align="center">
  <a href="docs/technical-showcase.md">Technical Deep Dive</a> ·
  <a href="#quickstart">Quickstart</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#supported-models">Supported Models</a> ·
  <a href="#system-requirements">System Requirements</a>
</p>

---

## Overview

AgentGuard is a production-grade AI Agent compliance & governance system built in Rust. It makes AI Agents **traceable, transparent, and controllable** — so enterprises dare to deploy Agents in production.

**Core mission: 让企业敢用AI Agent**

```
Agent的行为 → 审计追踪 → 透明可观测 → 可控可干预 → 企业敢用
```

AgentGuard applies Kubernetes control-plane architecture to LLM agent management — treating agents as governed, auditable, compliance-ready resources.

### Two Core Scenarios

**Scenario 1: Infrastructure Orchestration (Like K8s)**

Manage agents like containers — declarative definitions, automatic scheduling, self-healing, elastic scaling.

- Agent lifecycle management (create / destroy / restart / self-heal)
- Cluster scheduling (capability matching / load balancing / resource isolation)
- Full observability (tracing / metrics / logging / audit)
- Circuit breaking + credential management + RBAC + multi-tenancy
- Model routing (cost / quality tradeoffs + fallback)

**Scenario 2: Self-Cyclic Development**

Use agents to develop agents — self-create, self-orchestrate, self-test, self-deploy, self-evolve.

- Natural language → Agent definitions
- Tasks → Workflow DAGs (auto-decomposed)
- Repeated operations → Skills (auto-extracted from history)
- Code changes → Deployed (quality gates + rolling updates)

The two scenarios form a positive feedback loop: infrastructure provides the runtime environment for development, and development provides new capabilities for infrastructure.

> See [`docs/strategy/two-core-scenarios.md`](docs/strategy/two-core-scenarios.md) for the full capability matrix.

**Key Numbers:**

| Metric | Value |
|--------|-------|
| Language | Rust 1.95 |
| Crates | 26 |
| Lines of Code | 110,000+ |
| Tests | 2,307 passing |
| Clippy Warnings | 0 |
| Scheduling Algorithms | 7 (including GPU-Aware, Edge) |
| MCP Sandbox Backends | 5 (Docker / Firecracker / gVisor / Wasm / Process) |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         API Server (axum)                        │
│                   REST + gRPC + WebSocket + mTLS                 │
├─────────┬──────────┬──────────┬──────────┬───────────────────────┤
│Scheduler│Controller│Workflow  │  Team    │  Goal    │ Autonomy   │
│  Engine │          │  Engine  │  Engine  │  Engine  │ Controller │
│ 7 algos │Heartbeat │ DAG exec │ OVW      │ Goal loop│ 3-level    │
├─────────┴──────────┴──────────┴──────────┴──────────┴───────────┤
│  LangGraph Engine    │   MCP Protocol    │   Data Store (SQLite) │
│  State graph+FanOut  │   JSON-RPC+Sandbox│   Vector+Prefix Cache │
├──────────────────────┴───────────────────┴───────────────────────┤
│                    Common (L0): Error / Config / A2A / Masking    │
└─────────────────────────────────────────────────────────────────┘
```

**Layered dependency model** (enforced by `make lint-arch`):

```
L0: common                    ← Base types, errors, config
L1: data-store                ← SQLite persistence layer
L2: scheduler, controller, workflow-engine, team-engine, ...
L3: api-server, kias-main
```

Strict unidirectional dependencies. No cross-layer imports.

---

## Core Innovations

### 1. Cache-Aware Scheduling

**File:** [`crates/scheduler/src/algorithms/cache_aware.rs`](crates/scheduler/src/algorithms/cache_aware.rs)

Traditional schedulers (Round Robin, Least Loaded) are blind to LLM inference characteristics — they don't know whether a node has already cached a specific system prompt's KV Cache. A cache miss means recomputing the entire prefix, wasting ~90% of GPU compute.

AgentGuard introduces **DeepSeek-style Prefix Caching** into the scheduling decision layer:

```rust
// crates/scheduler/src/algorithms/cache_aware.rs
fn cache_aware_score(
    node: &Node, agent: &Agent,
    cache_info: Option<&NodeCacheInfo>, cache_weight: f64,
) -> f64 {
    let cache_score = if let (Some(info), Some(prefix_hash)) = (cache_info, agent.system_prompt_hash) {
        if info.cached_prefixes.contains(&prefix_hash) { 1.0 } else { 0.0 }
    } else { 0.0 };
    let load_score = 1.0 - node.load_factor();
    cache_weight * cache_score + (1.0 - cache_weight) * load_score
}
```

- **Fast path:** If a node has a matching cached prefix, route directly (score = 1.0)
- **Weighted scoring:** `cache_weight` parameter (0.0 = pure load balancing, 1.0 = pure cache priority)
- **Concurrent-safe:** `Arc<DashMap>` for lock-free concurrent cache map access

This is the only scheduling solution that incorporates LLM inference characteristics into scheduling decisions at the scheduler level.

---

### 2. LangGraph State Graph Engine

**File:** [`crates/langgraph-engine/src/graph.rs`](crates/langgraph-engine/src/graph.rs)

LLM workflows are not linear — they require conditional branching, loops, parallel subtasks, and interrupt-resume semantics. Existing DAG engines (Airflow, Temporal) are either too heavy or lack LLM-specific interrupt-resume support.

AgentGuard implements a complete LangGraph-style state graph engine with four edge types:

```rust
// crates/langgraph-engine/src/graph.rs
pub enum EdgeType {
    Direct { from: String, to: String },
    Conditional { from: String, to: String, condition: EdgeCondition },
    Router { from: String, router: RouterFn },
    FanOut { from: String, targets: Vec<String>, join_node: String },
}
```

**Parallel FanOut execution** spawns each branch in an independent `tokio::spawn` task, with state changes merged via last-write-wins strategy. **Checkpoint persistence** enables interrupt-resume semantics — `resume_from_checkpoint()` restores from the checkpoint node, not from the entry point.

Build-time validation via `build()` detects unreachable nodes and missing entry points before runtime. `max_steps` guard prevents infinite loops in LLM-driven conditional cycles.

---

### 3. TypedState — Compile-Time Safe Reducer Mechanism

**File:** [`crates/workflow-engine/src/typed_state.rs`](crates/workflow-engine/src/typed_state.rs)

LangGraph's core abstraction is the TypedDict + Reducer pattern. In Python, this relies on type hints (runtime checks). AgentGuard leverages Rust's type system to guarantee state merge correctness at compile time.

```rust
// crates/workflow-engine/src/typed_state.rs
pub trait ChannelReducer<T>: Send + Sync + 'static {
    fn reduce(&self, current: T, incoming: T) -> T;
    fn name(&self) -> &str;
}
```

Five built-in reducers: `Replace`, `Append`, `Merge` (shallow HashMap merge), `KeepFirst`, `Sum`. Each channel erases its type to `Box<dyn Any>` but captures the original type's reducer via closure. On `update()`, type safety is restored through `downcast`.

- **Compile-time safety:** Mismatched `T` and Reducer are rejected by the compiler
- **Runtime flexibility:** Channel names are strings, supporting dynamic registration
- **Concurrent branch safety:** FanOut branches merge state deterministically through reducers

---

### 4. Three-Layer Memory System

**File:** [`crates/team-engine/src/memory.rs`](crates/team-engine/src/memory.rs)

The core bottleneck in multi-agent collaboration is not communication — it's memory. Agents lose context after task completion, forcing redundant re-computation.

AgentGuard implements a three-layer memory architecture:

| Layer | Eviction Strategy | Purpose |
|-------|-------------------|---------|
| **ShortTerm** | TTL + LRU | Current task context |
| **LongTerm** | access_count + recency | Cross-task knowledge accumulation |
| **Entity** | confidence + recency | Entity attribute memory with confidence scores |

```rust
// crates/team-engine/src/memory.rs
pub struct MemoryManager {
    pub short_term: Arc<RwLock<ShortTermMemory>>,
    pub long_term: Arc<RwLock<LongTermMemory>>,
    pub entity: Arc<RwLock<EntityMemory>>,
}
```

`ContextBuilder` assembles context within a token budget (~4 chars/token heuristic), solving LLM context window overflow. All layers are thread-safe via `Arc<RwLock<>>` for concurrent multi-agent read/write. Entity Memory records confidence levels, allowing agents to distinguish between "known" and "inferred" facts.

---

### 5. Worker-Verifier Adversarial Quality Gate

**File:** [`crates/team-engine/src/verifier.rs`](crates/team-engine/src/verifier.rs)

Single-agent output quality is uncontrollable. Even with Chain of Thought, LLMs generate incorrect code, miss edge cases, and produce hallucinations.

AgentGuard implements an adversarial Worker-Verifier mechanism:

```rust
// crates/team-engine/src/verifier.rs
pub enum VerificationRule {
    Contains(String),
    NotContains(String),
    MinLength(usize),
    MaxLength(usize),
    ValidJson,
    Pattern(String),
    ShellCheck(String),  // Execute shell commands for verification
}
```

The `ShellCheck` rule runs actual test commands (e.g., `cargo test`, `python -m pytest`) during verification, elevating quality assurance from "looks correct" to "runs correctly." Verifier issues feed directly into the Worker's next iteration, forming a closed-loop improvement cycle.

---

### 6. Autonomy Gradient Controller

**File:** [`crates/autonomy-controller/src/autonomy.rs`](crates/autonomy-controller/src/autonomy.rs), [`crates/autonomy-controller/src/ladder.rs`](crates/autonomy-controller/src/ladder.rs)

Full autonomy is dangerous; full confirmation is inefficient. AgentGuard implements Codex CLI-style three-mode autonomy control with a complete decision pipeline:

```
Tool Policy Check → Rate Limit Check → Budget Check → Autonomy Level Judgment → Audit Log
```

```rust
// crates/autonomy-controller/src/ladder.rs
pub enum AutonomyLevel {
    Suggest,    // Suggest only, no execution
    AutoEdit,   // Write operations auto-execute, others require confirmation
    FullAuto,   // Fully automatic, constrained by tool policies
}
```

**Auto-promotion:** When an agent achieves consecutive successes above a threshold, it automatically promotes from `Suggest` to `AutoEdit`, reducing human intervention. `Forbidden` policy remains enforced even in `FullAuto` mode.

---

### 7. Goal-Driven Loop Engine

**File:** [`crates/goal-engine/src/loop_runner.rs`](crates/goal-engine/src/loop_runner.rs)

The "execute → evaluate → feedback → re-execute" pattern is ubiquitous in LLM applications. Most frameworks implement this as ad-hoc while loops in application code, lacking standardization, checkpoints, cancellation, and observability.

AgentGuard abstracts this as `GoalLoopRunner` with separated executor-evaluator roles:

```rust
// crates/goal-engine/src/loop_runner.rs
#[async_trait::async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn execute_round(
        &self, goal: &Goal, round: u32,
        previous_feedback: Option<&EvaluationResult>,
    ) -> KiasResult<String>;
}
```

`GoalCancelToken` (based on `AtomicBool`) enables graceful external termination. `evaluation_history` tracks per-round evaluation results for convergence analysis. Checkpoint callbacks allow external systems to persist state after each round.

---

### 8. Descheduler — Cluster Rebalancing with PDB Constraints

**File:** [`crates/scheduler/src/descheduler/engine.rs`](crates/scheduler/src/descheduler/engine.rs)

Schedulers decide where to place work; they don't decide when to move it. Over time, clusters develop: underutilized nodes wasting resources, anti-affinity constraints violated, agent replicas concentrated on few nodes.

AgentGuard implements a K8s Descheduler-style engine with three built-in strategies:

| Strategy | File | Purpose |
|----------|------|---------|
| `LowNodeUtilization` | `strategies/low_utilization.rs` | Detect underutilized nodes, migrate agents |
| `DuplicateAgent` | `strategies/duplicates.rs` | Detect over-concentration of agent replicas |
| `AntiAffinityViolation` | `strategies/anti_affinity.rs` | Detect and repair anti-affinity violations |

**Pod Disruption Budget (PDB) constraints** ensure evictions don't cause service interruptions. Dry Run mode supports previewing eviction plans without execution.

---

### 9. A2A Protocol + MCP Sandbox

**Files:** [`crates/common/src/a2a.rs`](crates/common/src/a2a.rs), [`crates/mcp-protocol/src/sandbox.rs`](crates/mcp-protocol/src/sandbox.rs)

Agent interconnection requires standardized protocols; agent execution of external tools requires security isolation.

**A2A (Agent-to-Agent)** implements Google's A2A protocol with complete data models:

```rust
// crates/common/src/a2a.rs
pub struct AgentCard {
    pub id: String,
    pub capabilities: AgentCapabilities,
    pub skills: Vec<AgentSkill>,
    pub authentication: Option<AuthInfo>,
}
```

Task lifecycle follows A2A spec: `Submitted → Working → InputRequired → Completed/Failed/Cancelled/Rejected`. Agent handoff supports 6 reasons: `CapabilityGap`, `LoadBalancing`, `Specialization`, `ErrorRecovery`, `HumanDirected`, `CostOptimization`.

**MCP Sandbox** provides 5 isolation backends:

```rust
// crates/mcp-protocol/src/lib.rs
pub use sandbox::{
    FirecrackerSandboxBackend,  // Lightweight VM
    GVisorSandboxBackend,       // User-space kernel
    ProcessSandboxBackend,      // Process-level isolation
    WasmSandboxBackend,         // WebAssembly
    DockerSandboxBackend,       // Docker container
};
```

Sandbox snapshots support state restore with `IsolationLevel` (Session / User / Global). Full MCP implementation includes OAuth 2.0, RBAC, circuit breaker, rate limiter, credential management, and hot-reload.

---

### 10. Data Masking Framework

**File:** [`crates/common/src/data_mask.rs`](crates/common/src/data_mask.rs)

LLM system logs frequently leak sensitive data: IP addresses, email addresses, JWT tokens. Traditional post-hoc masking or logging framework plugins are error-prone.

AgentGuard implements **zero-trust masking** at the infrastructure layer:

```rust
// crates/common/src/data_mask.rs
pub fn redact_log_message(msg: &str) -> String {
    let mut result = msg.to_string();
    result = redact_emails(&result);
    result = redact_ips(&result);
    result = redact_tokens(&result);  // Tokens ≥ 32 chars
    result
}
```

`SensitiveData` wrapper automatically masks on `Display` and `Serialize` — original values are never leaked. IPv4 detection uses a hand-written deterministic state machine (not regex), eliminating ReDoS risk. Since masking lives in the L0 `common` crate, all upstream components inherit it automatically.

---

### 11. InspirationStream — Builder-Thinker Dual-Flow Development

**File:** [`crates/knowledge/src/inspiration_stream.rs`](crates/knowledge/src/inspiration_stream.rs)

Traditional agent development follows a single-threaded execute → evaluate loop. The developer (or agent) builds, then checks if it works. This misses the opportunity for parallel insight discovery — while building, external knowledge sources may surface better approaches that could redirect effort before it's wasted.

AgentGuard introduces a **Builder-Thinker dual-flow architecture** inspired by MiniMax Mavis's Worker-Verifier adversarial pattern, extended with a third **Thinker** role:

```
Builder ──→ Produces code
    ↕ Positive feedback loop
Thinker ──→ Discovers insights from external sources
    ↓
Verifier ──→ Quality gate
```

Three knowledge source types with **positive feedback weighting**:

```rust
// crates/knowledge/src/inspiration_stream.rs
pub enum SourceType {
    Paper,      // arXiv, conference proceedings
    Trending,   // GitHub trending, HN, Reddit
    Benchmark,  // Performance comparisons, competitor analysis
}
```

**Positive feedback loop** — sources that produce adopted insights gain weight (up to 3.0×), sources that produce ignored insights lose weight (down to 0.3×). No manual tuning; the system learns which sources are valuable over time.

```rust
if adopted {
    source.reliability = (source.reliability * 1.05).min(3.0);  // +5%
} else {
    source.reliability = (source.reliability * 0.99).max(0.3);  // -1%
}
```

**Relevance scoring** uses keyword overlap between insight tags and the current task context. `max_per_cycle` prevents insight flooding. `min_relevance` filters noise. All insights are persisted with adopt/dismiss outcomes for the DreamConsolidator to learn from during sleep cycles.

This is the mechanism that enabled AgentGuard to absorb ideas from AgenticRAG, Claude Code's memory architecture, AgentScope's Workspace concept, and MiniMax Mavis's Worker-Verifier pattern — all discovered and integrated during active development, not in a separate research phase.

---

## Node-Level Error Handling

AgentGuard provides fault tolerance at every layer of the stack:

```
Request → Agent → Failure → DLQ → Exponential Backoff Retry → Circuit Breaker → Auto Recovery
```

| Mechanism | Implementation | File |
|-----------|---------------|------|
| Dead Letter Queue | Failed tasks queued for retry | `crates/controller/src/recovery.rs` |
| Circuit Breaker | Consecutive failures trigger open state | `crates/controller/src/health.rs` |
| Exponential Backoff | Configurable retry with jitter | `crates/controller/src/recovery.rs` |
| Health Check Loop | Continuous node liveness probing | `crates/controller/src/heartbeat.rs` |
| Saga Compensation | Workflow rollback on partial failure | `crates/workflow-engine/src/engine.rs` |

---

## Supported Models

### Cloud APIs

| Provider | Latest Model | Context | Input ($/1M tokens) | Output ($/1M tokens) |
|----------|-------------|---------|---------------------|---------------------|
| **OpenAI** | GPT-5.5 | 1,050K | $5.00 | $30.00 |
| **OpenAI** | GPT-5 | 400K | $1.25 | $10.00 |
| **OpenAI** | GPT-5-mini | 400K | $0.25 | $2.00 |
| **OpenAI** | o4-mini | 200K | $1.10 | $4.40 |
| **Anthropic** | Claude Opus 4.7 | 1,000K | $5.00 | $25.00 |
| **Anthropic** | Claude Sonnet 4.6 | 1,000K | $3.00 | $15.00 |
| **Anthropic** | Claude 3.5 Haiku | 200K | $0.80 | $4.00 |
| **Google** | Gemini 3.1 Pro | 1,048K | $2.00 | $12.00 |
| **Google** | Gemini 2.5 Pro | 1,048K | $1.25 | $10.00 |
| **Google** | Gemini 2.5 Flash | 1,048K | $0.30 | $2.50 |
| **DeepSeek** | DeepSeek-V4 Pro | 1,048K | $0.43 | $0.87 |
| **DeepSeek** | DeepSeek-V4 Flash | 1,048K | $0.11 | $0.22 |
| **DeepSeek** | DeepSeek-R1 | 163K | $0.70 | $2.50 |
| **Qwen** | Qwen3-Coder | 1,048K | $0.22 | $1.80 |
| **Qwen** | Qwen3-235B | 262K | $0.07 | $0.10 |
| **Mistral** | Mistral Large (2512) | 262K | $0.50 | $1.50 |
| **Mistral** | Codestral (2508) | 256K | $0.30 | $0.90 |
| **Meta** | Llama 4 Scout | 10,000K | $0.08 | $0.30 |
| **Meta** | Llama 4 Maverick | 1,048K | $0.15 | $0.60 |

> Pricing sourced from OpenRouter API (May 2026).

### Local Models

See [Local Model Comparison Guide](docs/local-model-comparison.md) for specifications, benchmarks, GPU requirements, and deployment recommendations across 16 open-source models.

| Runtime | Install | Use Case |
|---------|---------|----------|
| **Ollama** | `curl -fsSL https://ollama.com/install.sh \| sh` | Development & testing |
| **vLLM** | `pip install vllm` | Production high-throughput |
| **llama.cpp** | GitHub release | Edge devices, CPU inference |

---

## System Requirements

### Operating Systems

| Platform | Architecture | Status | Package |
|----------|-------------|--------|---------|
| **Ubuntu 22.04+** | x86_64 / aarch64 | ✅ Primary | `.deb` |
| **Debian 12+** | x86_64 / aarch64 | ✅ Supported | `.deb` |
| **CentOS 9 / RHEL 9** | x86_64 / aarch64 | ✅ Supported | `.rpm` |
| **Fedora 40+** | x86_64 / aarch64 | ✅ Supported | `.rpm` |
| **Alpine 3.18+** | x86_64 / aarch64 | ✅ Supported | Static binary |
| **macOS 13+** | Apple Silicon (M1-M4) / x86_64 | ✅ Supported | Homebrew |
| **Windows 11** | x86_64 | ⚠️ Via WSL2 only | `.msi` (WSL2) |
| **Docker** | x86_64 / aarch64 | ✅ Official image | `ghcr.io/andy-ckm/kias` |

### Hardware

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **CPU** | 2 cores | 4+ cores |
| **RAM** | 2 GB | 4+ GB |
| **Disk** | 500 MB (binary) | 2+ GB (with SQLite data) |
| **Network** | Outbound HTTPS | Required for LLM API calls |

> AgentGuard is a single binary (~30 MB). No runtime dependencies (no JVM, no Node, no Python).
> SQLite is embedded. No external database required for single-node deployment.

### Dependencies

| Dependency | Required | Notes |
|-----------|----------|-------|
| **Rust 1.85+** | Build only | Stable channel |
| **SQLite 3.35+** | Runtime (embedded) | Auto-included via `rusqlite` bundled feature |
| **OpenSSL / rustls** | Runtime | TLS for LLM API calls (rustls by default) |

### Deployment Modes

#### Ubuntu / Debian (`.deb`)

```bash
# Download and install
curl -LO https://github.com/Andy-ckm/KIAS/releases/latest/download/kias-amd64.deb
sudo dpkg -i kias-amd64.deb

# Start as systemd service
sudo systemctl enable --now kias
sudo systemctl status kias

# Logs
journalctl -u kias -f
```

#### CentOS / RHEL / Fedora (`.rpm`)

```bash
# Download and install
curl -LO https://github.com/Andy-ckm/KIAS/releases/latest/download/kias-amd64.rpm
sudo rpm -i kias-amd64.rpm

# Start as systemd service
sudo systemctl enable --now kias
```

#### Docker

```bash
docker pull ghcr.io/andy-ckm/kias:latest
docker run -d --name kias -p 8080:8080 ghcr.io/andy-ckm/kias:latest
```

---

## Quickstart

```bash
# 1. Clone
git clone https://github.com/Andy-ckm/KIAS.git
cd AgentGuard

# 2. Build
cargo build --release

# 3. Configure
cp config.example.yaml config.yaml
# Edit config.yaml: add your LLM API keys

# 4. Run
./target/release/kias serve --config config.yaml
```

```bash
# Verify
curl http://localhost:8080/healthz
# {"status":"ok"}
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Commit changes (`git commit -m 'feat: add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Open a Pull Request

**Quality gates (must pass):**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

---

## License

[MIT](LICENSE) © Andy-ckm