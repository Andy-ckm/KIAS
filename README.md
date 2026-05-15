<div align="center">

# 🚀 KIAS

### Kubernetes-like Intelligent Agent Scheduling System

**企业级 Rust AI Agent 框架 | 生产就绪 | 高性能**

[![Rust](https://img.shields.io/badge/Rust-1.95.0-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-1419%20Passed-brightgreen?style=flat-square)](https://github.com/Andy-ckm/KIAS/actions)
[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Crates](https://img.shields.io/badge/Crates-21-purple?style=flat-square)](https://crates.io/)

[English](#english) | [中文](#中文)

</div>

---

## 🎯 为什么选择 KIAS？

> **KIAS 不是玩具，是生产级 AI Agent 框架。**

在 AI Agent 框架遍地开花的今天，KIAS 专注于解决一个核心问题：**如何让 AI Agent 在生产环境中稳定、高效、可追踪地运行？**

### 💡 核心理念

```
┌─────────────────────────────────────────────────────────────┐
│                    KIAS 设计哲学                              │
├─────────────────────────────────────────────────────────────┤
│  ✅ 质量第一 — 1419 个测试，零容忍缺陷                        │
│  ✅ 生产就绪 — 优雅关闭、深度健康检查、死信队列                 │
│  ✅ 企业级 — 多租户隔离、RBAC、审计日志                        │
│  ✅ 高性能 — Rust 原生、零拷贝、内存安全                       │
│  ✅ 可追踪 — 完整的 ADR、特性矩阵、变更日志                    │
└─────────────────────────────────────────────────────────────┘
```

---

## 🏗️ 架构概览

```
┌─────────────────────────────────────────────────────────────────────┐
│                         KIAS 架构                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│   │   Scheduler   │  │  Controller  │  │   Monitor    │            │
│   │  (GPU调度)    │  │  (健康检查)   │  │  (遥测监控)   │            │
│   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘            │
│          │                  │                  │                    │
│          └──────────────────┼──────────────────┘                    │
│                             │                                       │
│                    ┌────────▼────────┐                             │
│                    │   Data Store    │                             │
│                    │  (SQLite持久化)  │                             │
│                    └────────┬────────┘                             │
│                             │                                       │
│   ┌──────────────┐  ┌──────┴───────┐  ┌──────────────┐            │
│   │  Team Engine  │  │  Workflow    │  │  Goal Engine │            │
│   │  (多Agent协作) │  │  (DAG编排)   │  │  (目标驱动)   │            │
│   └──────────────┘  └──────────────┘  └──────────────┘            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## ✨ 核心特性

### 🔥 生产级特性

| 特性 | 描述 | 状态 |
|------|------|------|
| **优雅关闭** | SIGTERM/SIGINT 信号处理，子系统协调关闭 | ✅ |
| **深度健康检查** | 内存/磁盘/CPU/队列深度全面监控 | ✅ |
| **死信队列** | 失败任务归档，支持重试策略 | ✅ |
| **审计日志持久化** | SQLite 持久化，重启不丢失 | ✅ |
| **多租户隔离** | 资源配额、命名空间隔离 | ✅ |
| **GPU 调度** | NVIDIA/AMD/Intel + MIG 支持 | ✅ |
| **智能 Key 轮转** | Fisher-Yates shuffle + 失败降权 | ✅ |
| **熔断器** | 自动熔断、限流、降级 | ✅ |

### 🧠 AI Agent 能力

| 能力 | 描述 | 状态 |
|------|------|------|
| **多 Agent 协作** | Owner-Worker-Verifier 模式 | ✅ |
| **工作空间管理** | AGENTS.md、MEMORY.md、skills/ | ✅ |
| **上下文压缩** | Token 预算管理、事实提取 | ✅ |
| **会话持久化** | JSONL 序列化、上下文快照 | ✅ |
| **子 Agent 编排** | 声明式 YAML、同步/异步执行 | ✅ |
| **沙箱隔离** | 三级隔离：进程/容器/虚拟机 | ✅ |
| **目标驱动循环** | 自动目标分解、执行、评估 | ✅ |
| **自主度控制** | Suggest/AutoEdit/FullAuto 三模式 | ✅ |

### 📊 可观测性

| 能力 | 描述 | 状态 |
|------|------|------|
| **Prometheus 指标** | 系统指标、Agent 指标、业务指标 | ✅ |
| **分布式追踪** | 全链路追踪、性能分析 | ✅ |
| **结构化日志** | tracing + JSON 格式 | ✅ |
| **实时推送** | WebSocket 事件推送 | ✅ |
| **Dashboard** | React + TypeScript 前端 | ✅ |

---

## 📦 项目结构

```
kias/
├── crates/                    # 21 个 Rust crate
│   ├── kias-main/            # 主程序入口
│   ├── api-server/           # REST API 服务器
│   ├── scheduler/            # 任务调度器
│   ├── controller/           # 任务控制器
│   ├── team-engine/          # 多 Agent 协作
│   ├── workflow-engine/      # DAG 工作流
│   ├── goal-engine/          # 目标驱动引擎
│   ├── data-store/           # 数据持久化层
│   ├── common/               # 公共工具库
│   └── ...                   # 其他 12 个 crate
├── dashboard/                 # React + TypeScript 前端
├── docs/                      # 完整文档体系
│   ├── adr/                  # 架构决策记录
│   ├── traceability/         # 可追溯性文档
│   └── design-docs/          # 设计文档
└── reference-projects/        # 参考源码
```

---

## 🚀 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

# 构建项目
cargo build --release

# 运行测试
cargo test --workspace

# 启动服务
cargo run --bin kias
```

### 配置

```toml
# config.toml
[api_server]
port = 8080
host = "0.0.0.0"

[scheduler]
algorithm = "round-robin"

[controller]
heartbeat_interval_secs = 30
failure_timeout_secs = 300
max_retries = 3
```

### API 示例

```bash
# 健康检查
curl http://localhost:8080/health

# 深度健康检查
curl http://localhost:8080/healthz/deep

# 创建 Agent
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "type": "llm"}'

# 查询任务
curl http://localhost:8080/api/v1/tasks
```

---

## 📈 质量保证

### 测试覆盖

```
┌─────────────────────────────────────────────────┐
│              测试统计 (1419 个测试)               │
├─────────────────────────────────────────────────┤
│  单元测试:     1200+ (85%)                      │
│  集成测试:      150+ (11%)                      │
│  端到端测试:     69+ (4%)                       │
├─────────────────────────────────────────────────┤
│  Clippy 警告:    0                              │
│  代码覆盖率:    >90%                            │
│  构建时间:      <60s                            │
└─────────────────────────────────────────────────┘
```

### 代码质量

- ✅ **零 Clippy 警告** — 严格模式 `-D warnings`
- ✅ **零 unsafe 代码** — 100% 安全 Rust
- ✅ **完整文档** — 42 个 Markdown 文档
- ✅ **架构合规** — 分层依赖检查通过

---

## 🔧 技术栈

### 后端

| 技术 | 用途 | 版本 |
|------|------|------|
| **Rust** | 核心语言 | 1.95.0 |
| **Tokio** | 异步运行时 | 1.x |
| **Axum** | Web 框架 | 0.7 |
| **SQLx** | 数据库 | 0.8 |
| **Serde** | 序列化 | 1.x |
| **Tracing** | 日志 | 0.1 |

### 前端

| 技术 | 用途 | 版本 |
|------|------|------|
| **React** | UI 框架 | 18.x |
| **TypeScript** | 类型系统 | 5.x |
| **Vite** | 构建工具 | 5.x |
| **TailwindCSS** | 样式 | 3.x |
| **Recharts** | 图表 | 2.x |

### 基础设施

| 技术 | 用途 | 版本 |
|------|------|------|
| **SQLite** | 数据持久化 | 3.x |
| **Prometheus** | 指标监控 | 2.x |
| **WebSocket** | 实时推送 | - |
| **TLS 1.3** | 传输加密 | - |

---

## 📚 文档

### 核心文档

- 📖 [架构设计](docs/architecture.md)
- 📖 [API 文档](docs/api-docs.md)
- 📖 [用户指南](docs/user-guide.md)
- 📖 [开发者指南](docs/traceability/developer-guide.md)

### 可追溯性文档

- 📋 [架构决策记录](docs/adr/)
- 📋 [特性跟踪矩阵](docs/traceability/feature-matrix.md)
- 📋 [测试覆盖率](docs/traceability/test-coverage.md)
- 📋 [变更日志](docs/CHANGELOG.md)

---

## 🎯 使用场景

### 1. 企业级 AI Agent 平台

```rust
// 创建多 Agent 协作系统
let team = TeamEngine::new()
    .with_owner(owner_agent)
    .with_workers(vec![worker1, worker2])
    .with_verifier(verifier_agent)
    .build();

// 执行任务
let result = team.execute(task).await?;
```

### 2. 智能任务调度

```rust
// GPU 感知调度
let scheduler = Scheduler::new()
    .with_algorithm("gpu-aware")
    .with_preemption(true)
    .build();

// 调度任务
let node = scheduler.schedule(agent, &nodes).await?;
```

### 3. 工作流编排

```rust
// 定义 DAG 工作流
let workflow = Workflow::new("data-pipeline")
    .step("extract", extract_task)
    .step("transform", transform_task)
    .step("load", load_task)
    .depends_on("transform", "extract")
    .depends_on("load", "transform")
    .build();

// 执行工作流
workflow.execute().await?;
```

---

## 🏆 与其他框架对比

| 特性 | KIAS | LangChain | AutoGen | CrewAI |
|------|------|-----------|---------|--------|
| **语言** | Rust | Python | Python | Python |
| **性能** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **类型安全** | ✅ | ❌ | ❌ | ❌ |
| **生产就绪** | ✅ | ⚠️ | ⚠️ | ⚠️ |
| **多租户** | ✅ | ❌ | ❌ | ❌ |
| **GPU 调度** | ✅ | ❌ | ❌ | ❌ |
| **审计日志** | ✅ | ❌ | ❌ | ❌ |
| **优雅关闭** | ✅ | ❌ | ❌ | ❌ |
| **深度健康检查** | ✅ | ❌ | ❌ | ❌ |
| **死信队列** | ✅ | ❌ | ❌ | ❌ |

---

## 🤝 贡献

我们欢迎所有形式的贡献！

### 如何贡献

1. **Fork** 本仓库
2. **创建** 特性分支 (`git checkout -b feature/amazing-feature`)
3. **提交** 更改 (`git commit -m 'feat: add amazing feature'`)
4. **推送** 到分支 (`git push origin feature/amazing-feature`)
5. **创建** Pull Request

### 贡献指南

- 📖 [贡献指南](CONTRIBUTING.md)
- 📖 [代码规范](CODE_OF_CONDUCT.md)
- 📖 [架构决策](docs/adr/)

---

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## 🙏 致谢

感谢以下开源项目的启发：

- [ollama-open-router](https://github.com/open-webui/ollama-open-router) - Key 轮转参考
- [AgentScope](https://github.com/modelscope/agentscope) - Agent 架构参考
- [Hermes Agent](https://github.com/NousResearch/hermes-agent) - 上下文压缩参考
- [rig](https://github.com/0xPlaygrounds/rig) - Rust Agent 框架参考

---

<div align="center">

**⭐ 如果觉得有用，请给我们一个 Star！⭐**

[![Star History Chart](https://api.star-history.com/svg?repos=Andy-ckm/KIAS&type=Date)](https://star-history.com/#Andy-ckm/KIAS&Date)

</div>

---

<a name="english"></a>
## 🇺🇸 English

### Why KIAS?

> **KIAS is not a toy. It's a production-grade AI Agent framework.**

In the era of AI Agent frameworks everywhere, KIAS focuses on solving one core problem: **How to run AI Agents in production stably, efficiently, and traceably?**

### Key Features

- 🚀 **Production-Ready** — Graceful shutdown, deep health checks, dead letter queue
- 🏢 **Enterprise-Grade** — Multi-tenant isolation, RBAC, audit logging
- ⚡ **High Performance** — Native Rust, zero-copy, memory safe
- 🔍 **Traceable** — Complete ADR, feature matrix, changelog
- 🧪 **Well-Tested** — 1419 tests, zero clippy warnings
- 📊 **Observable** — Prometheus metrics, distributed tracing, real-time push

### Quick Start

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS
cargo build --release
cargo run --bin kias
```

---

<a name="中文"></a>
## 🇨🇳 中文

### 为什么选择 KIAS？

> **KIAS 不是玩具，是生产级 AI Agent 框架。**

在 AI Agent 框架遍地开花的今天，KIAS 专注于解决一个核心问题：**如何让 AI Agent 在生产环境中稳定、高效、可追踪地运行？**

### 核心优势

- 🚀 **生产就绪** — 优雅关闭、深度健康检查、死信队列
- 🏢 **企业级** — 多租户隔离、RBAC、审计日志
- ⚡ **高性能** — Rust 原生、零拷贝、内存安全
- 🔍 **可追踪** — 完整的 ADR、特性矩阵、变更日志
- 🧪 **测试充分** — 1419 个测试，零 Clippy 警告
- 📊 **可观测** — Prometheus 指标、分布式追踪、实时推送

### 快速开始

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS
cargo build --release
cargo run --bin kias
```

---

<div align="center">

**Made with ❤️ by the KIAS Team**

[![GitHub](https://img.shields.io/badge/GitHub-Andy--ckm/KIAS-black?style=flat-square&logo=github)](https://github.com/Andy-ckm/KIAS)
[![Issues](https://img.shields.io/badge/Issues-Open-blue?style=flat-square&logo=github)](https://github.com/Andy-ckm/KIAS/issues)
[![Pull Requests](https://img.shields.io/badge/PRs-Welcome-green?style=flat-square&logo=github)](https://github.com/Andy-ckm/KIAS/pulls)

</div>
