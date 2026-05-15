# KIAS

**Enterprise-grade AI Agent framework in Rust.**

[![Rust](https://img.shields.io/badge/Rust-1.95-orange?logo=rust)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-1419-brightgreen)]()
[![Crates](https://img.shields.io/badge/Crates-21-purple)]()

---

## Why KIAS Exists

**Problem 1: Other frameworks don't handle production**

Most AI Agent frameworks crash on `SIGTERM`, lose data on restart, and can't handle failures.

**KIAS fix**: Graceful shutdown, deep health checks, dead letter queues, audit logging, circuit breakers.

---

**Problem 2: No multi-tenant isolation**

Enterprise customers need resource quotas and namespace isolation.

**KIAS fix**: Per-tenant CPU/memory/GPU limits, namespace isolation, fair scheduling, tenant stats.

---

**Problem 3: Python hits performance walls**

High latency, high memory, can't handle concurrency.

**KIAS fix**: Rust — zero-copy, memory safe, Tokio async, sub-millisecond latency.

---

**Problem 4: No observability**

Most frameworks are black boxes.

**KIAS fix**: Prometheus metrics, distributed tracing, structured logging, WebSocket real-time push.

---

## Installation

### One-liner Install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh
```

### Docker

```bash
# Quick start
docker run -p 8080:8080 ghcr.io/Andy-ckm/kias:latest

# With docker-compose
git clone https://github.com/Andy-ckm/KIAS
cd KIAS
docker-compose up -d
```

### Cargo Install

```bash
cargo install kias-cli
```

### From Source

```bash
git clone https://github.com/Andy-ckm/KIAS
cd KIAS
cargo build --release
./target/release/kias-main
```

---

## Quickstart

```bash
# Initialize config
kias config init

# Start server
kias start

# Dashboard at http://localhost:8080
```

---

## Architecture

```
kias/
├── kias-main/           # Entry point, orchestration
├── api-server/          # Axum REST/WebSocket
├── controller/          # Task scheduling, circuit breakers
├── data-store/          # SQLite persistence (agents, tasks, workflows)
├── model-router/        # LLM routing, key rotation
├── scheduler/           # Cron-based task scheduling
├── workflow-engine/     # DAG workflow execution
├── sandbox/             # Code execution isolation
├── mcp-protocol/        # Model Context Protocol
├── common/              # Shared types, graceful shutdown
└── dashboard/           # React + TypeScript UI
```

---

## Core Features

| Feature | Status | Description |
|---------|--------|-------------|
| Graceful Shutdown | ✅ | SIGTERM handling, subsystem cleanup |
| Deep Health Checks | ✅ | Memory, disk, CPU, queue monitoring |
| Dead Letter Queue | ✅ | Failed task archival, retry strategies |
| Audit Logging | ✅ | SQLite persistence, survives restarts |
| Circuit Breakers | ✅ | Auto-cut, rate limiting, degradation |
| Key Rotation | ✅ | Smart pick, cooldown recovery |
| Multi-tenant | ✅ | Resource quotas, namespace isolation |
| WebSocket | ✅ | Real-time event streaming |
| Dashboard | ✅ | React + TypeScript frontend |

---

## Tech Stack

- **Language**: Rust 1.95
- **Async**: Tokio
- **Web**: Axum
- **Database**: SQLite via sqlx
- **Cache**: In-memory (Redis-compatible planned)
- **Metrics**: Prometheus
- **Frontend**: React, TypeScript, TailwindCSS

---

## Documentation

- [Architecture Decision Records](docs/adr/)
- [Design Documents](docs/design-docs/)
- [Traceability Matrix](docs/traceability/)
- [LiteLLM Analysis](docs/litellm-analysis.md)

---

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests (`cargo test --workspace`)
4. Commit your changes (`git commit -m 'Add amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

---

## License

MIT
