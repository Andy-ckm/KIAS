<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://res.cloudinary.com/total-typescript/image/upload/v1777382277/skills-repo-dark_2x.png">
    <source media="(prefers-color-scheme: light)" srcset="https://res.cloudinary.com/total-typescript/image/upload/v1777382277/skill-repo-light_2x.png">
    <img alt="KIAS" src="https://res.cloudinary.com/total-typescript/image/upload/v1777382277/skill-repo-light_2x.png" width="369">
  </picture>
</p>

# KIAS — Enterprise AI Agent Framework

[![Rust](https://img.shields.io/badge/Rust-1.95.0-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-1419%20Passed-brightgreen?style=flat-square)](https://github.com/Andy-ckm/KIAS/actions)
[![Crates](https://img.shields.io/badge/Crates-21-purple?style=flat-square)](https://crates.io/)

**Production-grade Rust framework for building AI Agent systems that actually work in production.**

Most AI Agent frameworks are toys. KIAS is different — it's built for real production workloads with enterprise features like graceful shutdown, deep health checks, dead letter queues, and multi-tenant isolation.

If you're building AI Agents that need to run 24/7 in production, KIAS is for you.

## Why KIAS Exists

### #1: Other Frameworks Don't Handle Production

> "The best production systems are boring. They just work."

**The Problem**: Most AI Agent frameworks focus on demos, not production. They crash on SIGTERM, lose data on restart, and can't handle failures gracefully.

**The Fix**: KIAS includes production-ready features out of the box:

- **Graceful Shutdown** — Handles SIGTERM/SIGINT, coordinates subsystem cleanup
- **Deep Health Checks** — Monitors memory, disk, CPU, queue depth
- **Dead Letter Queue** — Archives failed tasks, supports retry strategies
- **Audit Logging** — Persists to SQLite, survives restarts
- **Circuit Breakers** — Auto-cuts, rate limiting, degradation

### #2: No Multi-Tenant Isolation

> "Enterprise customers need isolation, not shared everything."

**The Problem**: Most frameworks treat all users as one big tenant. No resource quotas, no namespace isolation, no billing separation.

**The Fix**: KIAS has real multi-tenant support:

- **Resource Quotas** — CPU, memory, GPU limits per tenant
- **Namespace Isolation** — Agents can't see other tenants' data
- **Fair Scheduling** — Round-robin across tenants
- **Tenant Statistics** — Track usage per tenant

### #3: Can't Scale to Real Workloads

> "Python is great for prototypes. Rust is great for production."

**The Problem**: Python-based frameworks hit performance walls. High latency, high memory usage, can't handle concurrent workloads.

**The Fix**: KIAS is built in Rust:

- **Zero-copy** — No garbage collector pauses
- **Memory safe** — No segfaults, no data races
- **High concurrency** — Tokio async runtime
- **Low latency** — Sub-millisecond response times

### #4: No Observability

> "You can't fix what you can't see."

**The Problem**: Most frameworks are black boxes. No metrics, no tracing, no real-time monitoring.

**The Fix**: KIAS has full observability:

- **Prometheus Metrics** — System, agent, and business metrics
- **Distributed Tracing** — Full request lifecycle tracking
- **Structured Logging** — tracing + JSON format
- **Real-time Push** — WebSocket event streaming
- **Dashboard** — React + TypeScript frontend

## Quickstart (60-second setup)

```bash
# Clone
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

# Build
cargo build --release

# Run
cargo run --bin kias

# Test
cargo test --workspace
```

## Core Features

### Production-Ready

- **[Graceful Shutdown](./crates/common/src/graceful_shutdown.rs)** — SIGTERM/SIGINT handling, subsystem coordination, configurable timeouts
- **[Deep Health Checks](./crates/api-server/src/handlers/health.rs)** — Memory, disk, CPU, queue depth monitoring via `/healthz/deep`
- **[Dead Letter Queue](./crates/data-store/src/dlq.rs)** — Failed task archival, retry strategies, operator management
- **[Audit Logging](./crates/data-store/src/audit_persist.rs)** — SQLite persistence, query/filter/purge, survives restarts
- **[Circuit Breakers](./crates/controller/src/resilience.rs)** — Auto-cut, rate limiting, degradation, thundering herd protection

### Enterprise-Grade

- **[Multi-Tenant Isolation](./crates/scheduler/src/multi_tenant.rs)** — Resource quotas, namespace isolation, fair scheduling
- **[RBAC](./crates/api-server/src/auth.rs)** — Role-based access control, JWT/API Key authentication
- **[Key Rotation](./crates/model-router/src/key_rotation.rs)** — Fisher-Yates shuffle, failure demotion, budget tracking
- **[TLS 1.3](./crates/common/src/tls.rs)** — Mutual TLS, ALPN negotiation, certificate validation

### AI Agent Capabilities

- **[Multi-Agent Collaboration](./crates/team-engine/src/subagent.rs)** — Owner-Worker-Verifier pattern, declarative YAML
- **[Workspace Management](./crates/team-engine/src/workspace.rs)** — AGENTS.md, MEMORY.md, skills/, knowledge/
- **[Context Compaction](./crates/team-engine/src/compaction.rs)** — Token budget management, fact extraction
- **[Session Persistence](./crates/team-engine/src/session.rs)** — JSONL serialization, context snapshots
- **[Sandbox Isolation](./crates/mcp-protocol/src/sandbox.rs)** — Three levels: process/container/VM
- **[Goal-Driven Loop](./crates/goal-engine/src/loop_runner.rs)** — Automatic goal decomposition, execution, evaluation
- **[Autonomy Control](./crates/autonomy-controller/src/lib.rs)** — Three modes: Suggest/AutoEdit/FullAuto

### Observability

- **[Prometheus Metrics](./crates/monitor/src/prometheus.rs)** — System, agent, and business metrics
- **[Distributed Tracing](./crates/monitor/src/telemetry.rs)** — Full request lifecycle tracking
- **[WebSocket Push](./crates/api-server/src/websocket.rs)** — Real-time event streaming
- **[Dashboard](./dashboard/)** — React + TypeScript frontend with charts

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         KIAS Architecture                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│   │   Scheduler   │  │  Controller  │  │   Monitor    │        │
│   │  (GPU调度)    │  │  (健康检查)   │  │  (遥测监控)   │        │
│   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘        │
│          │                  │                  │                │
│          └──────────────────┼──────────────────┘                │
│                             │                                   │
│                    ┌────────▼────────┐                         │
│                    │   Data Store    │                         │
│                    │  (SQLite持久化)  │                         │
│                    └────────┬────────┘                         │
│                             │                                   │
│   ┌──────────────┐  ┌──────┴───────┐  ┌──────────────┐        │
│   │  Team Engine  │  │  Workflow    │  │  Goal Engine │        │
│   │  (多Agent协作) │  │  (DAG编排)   │  │  (目标驱动)   │        │
│   └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
kias/
├── crates/                    # 21 Rust crates
│   ├── kias-main/            # Main entry point
│   ├── api-server/           # REST API server
│   ├── scheduler/            # Task scheduler
│   ├── controller/           # Task controller
│   ├── team-engine/          # Multi-agent collaboration
│   ├── workflow-engine/      # DAG workflow
│   ├── goal-engine/          # Goal-driven engine
│   ├── data-store/           # Data persistence
│   ├── common/               # Shared utilities
│   └── ...                   # 12 more crates
├── dashboard/                 # React + TypeScript frontend
├── docs/                      # Complete documentation
│   ├── adr/                  # Architecture Decision Records
│   ├── traceability/         # Traceability documents
│   └── design-docs/          # Design documents
└── reference-projects/        # Reference source code
```

## Tech Stack

### Backend

| Technology | Purpose | Version |
|------------|---------|---------|
| **Rust** | Core language | 1.95.0 |
| **Tokio** | Async runtime | 1.x |
| **Axum** | Web framework | 0.7 |
| **SQLx** | Database | 0.8 |
| **Serde** | Serialization | 1.x |
| **Tracing** | Logging | 0.1 |

### Frontend

| Technology | Purpose | Version |
|------------|---------|---------|
| **React** | UI framework | 18.x |
| **TypeScript** | Type system | 5.x |
| **Vite** | Build tool | 5.x |
| **TailwindCSS** | Styling | 3.x |
| **Recharts** | Charts | 2.x |

### Infrastructure

| Technology | Purpose | Version |
|------------|---------|---------|
| **SQLite** | Data persistence | 3.x |
| **Prometheus** | Metrics | 2.x |
| **WebSocket** | Real-time push | - |
| **TLS 1.3** | Encryption | - |

## Comparison

| Feature | KIAS | LangChain | AutoGen | CrewAI |
|---------|------|-----------|---------|--------|
| **Language** | Rust | Python | Python | Python |
| **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **Type Safety** | ✅ | ❌ | ❌ | ❌ |
| **Production Ready** | ✅ | ⚠️ | ⚠️ | ⚠️ |
| **Multi-Tenant** | ✅ | ❌ | ❌ | ❌ |
| **GPU Scheduling** | ✅ | ❌ | ❌ | ❌ |
| **Audit Logging** | ✅ | ❌ | ❌ | ❌ |
| **Graceful Shutdown** | ✅ | ❌ | ❌ | ❌ |
| **Deep Health Checks** | ✅ | ❌ | ❌ | ❌ |
| **Dead Letter Queue** | ✅ | ❌ | ❌ | ❌ |

## Documentation

### Core Docs

- 📖 [Architecture Design](docs/architecture.md)
- 📖 [API Documentation](docs/api-docs.md)
- 📖 [User Guide](docs/user-guide.md)
- 📖 [Developer Guide](docs/traceability/developer-guide.md)

### Traceability

- 📋 [Architecture Decision Records](docs/adr/)
- 📋 [Feature Matrix](docs/traceability/feature-matrix.md)
- 📋 [Test Coverage](docs/traceability/test-coverage.md)
- 📋 [Changelog](docs/CHANGELOG.md)

## Contributing

We welcome all forms of contribution!

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'feat: add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Create** a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Thanks to these open-source projects for inspiration:

- [ollama-open-router](https://github.com/open-webui/ollama-open-router) — Key rotation reference
- [AgentScope](https://github.com/modelscope/agentscope) — Agent architecture reference
- [Hermes Agent](https://github.com/NousResearch/hermes-agent) — Context compaction reference
- [rig](https://github.com/0xPlaygrounds/rig) — Rust Agent framework reference

---

<div align="center">

**⭐ Star us if you find KIAS useful! ⭐**

[![Star History Chart](https://api.star-history.com/svg?repos=Andy-ckm/KIAS&type=Date)](https://star-history.com/#Andy-ckm/KIAS&Date)

</div>
