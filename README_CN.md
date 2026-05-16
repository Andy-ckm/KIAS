<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS/actions">
    <img src="https://img.shields.io/badge/tests-1464%20passed-brightgreen.svg" alt="Tests">
  </a>
  <a href="https://www.rust-lang.org">
    <img src="https://img.shields.io/badge/Rust-1.95-orange.svg?logo=rust" alt="Rust">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/badge/crates-21-purple.svg" alt="Crates">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/badge/LOC-75K%2B-blue.svg" alt="Lines of Code">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/github/stars/Andy-ckm/KIAS?style=social" alt="Stars">
  </a>
</p>

<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS" width="420">
</p>

<h1 align="center">KIAS</h1>
<p align="center"><strong>Kubernetes-like Intelligent Agent Scheduling</strong></p>
<p align="center">用 Rust 构建的生产级 AI Agent 集群调度框架</p>

---

## 为什么需要 KIAS

你的 Agent 在笔记本上跑得很顺畅，推到生产环境就出问题：

```
                        开发环境                          生产环境
                   ┌──────────────┐              ┌──────────────────────┐
                   │   Agent      │              │  Agent               │
                   │   运行正常 ✅  │   ──────▶   │  状态丢失 ❌           │
                   │   响应很快    │              │  崩溃无恢复 ❌         │
                   │   一切正常    │              │  租户互相干扰 ❌       │
                   └──────────────┘              │  绑定单一模型 ❌       │
                                                 │  无法观测运行状态 ❌    │
                                                 └──────────────────────┘
```

**KIAS 解决的就是这个落差。** 它为 Agent 提供生产环境必需的基础设施，让你专注于 Agent 逻辑本身。

---

## 核心架构

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          KIAS 平台架构                                    │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  你的业务逻辑（你只需关注这一层）                                    │    │
│  │  Agent Code · Tools · Prompts                                    │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                    ▼                                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
│  │ LangGraph│ │ Workflow  │ │  Team    │ │  MCP     │ │ Sandbox  │      │
│  │ 状态图引擎│ │ DAG工作流 │ │ 多Agent  │ │ 工具协议 │ │ 沙箱隔离 │      │
│  │          │ │          │ │ 协作编排 │ │          │ │          │      │
│  │ ·条件分支│ │ ·并行执行│ │ ·委托    │ │ ·标准MCP│ │ ·5种后端│      │
│  │ ·循环   │ │ ·子图    │ │ ·验证    │ │ ·热加载 │ │ ·资源限制│      │
│  │ ·检查点  │ │ ·Saga回滚│ │ ·记忆共享│ │ ·鉴权   │ │ ·审计日志│      │
│  │ ·并行扇出│ │ ·重试策略│ │ ·技能匹配│ │          │ │          │      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
│                                    ▼                                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
│  │Scheduler │ │Controller│ │Model     │ │  Data    │ │ Monitor  │      │
│  │ 调度引擎 │ │ 生命周期 │ │ Router   │ │  Store   │ │ 可观测   │      │
│  │          │ │          │ │ 模型路由 │ │ 数据存储 │ │          │      │
│  │ ·4种算法 │ │ ·心跳    │ │ ·10+供应商│ │ ·SQLite │ │ ·Prometheus│    │
│  │ ·亲和性  │ │ ·故障恢复│ │ ·负载均衡│ │ ·HNSW向量│ │ ·WebSocket│    │
│  │ ·抢占    │ │ ·熔断器  │ │ ·Fallback│ │ ·缓存层 │ │ ·健康检查│      │
│  │ ·GPU感知 │ │ ·死信队列│ │ ·成本控制│ │ ·审计日志│ │ ·链路追踪│      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 解决的 6 个核心问题

### 1. 状态持久化 — 崩溃不丢数据

**问题：** 进程一崩，Agent 运行了几小时的推理结果全部归零。

**方案：** KIAS 采用 **SQLite + WAL 模式** 持久化所有 Agent 状态。参考 K8s 的 etcd 设计理念，但用 SQLite 替代——单文件部署、零运维、ACID 事务保证。每个 Agent 的完整状态（上下文、工具调用历史、中间推理结果）都会写入磁盘。进程重启后自动恢复到崩溃前的位置。

```rust
// Agent 状态自动持久化，无需手动管理
let agent = AgentBuilder::new("my-agent")
    .with_checkpoint_store(SqliteCheckpointStore::new("agent.db"))
    .build();
// 进程崩溃？重启后 agent 自动从上次 checkpoint 恢复
```

### 2. 错误自动恢复 — 自愈而非自毁

**问题：** 一次 API 调用失败，整个 Agent 任务链中断，只能人工重启。

**方案：** **死信队列（DLQ）+ 熔断器 + 指数退避重试**。失败的任务不会丢失，自动进入重试队列。连续失败触发熔断，防止雪崩。恢复后自动解除熔断。

```
传统方式:  请求 → Agent → 报错 → 系统挂掉 → 人工重启
KIAS:     请求 → Agent → 报错 → DLQ → 指数退避重试 → 熔断保护 → 自动恢复
```

### 3. 多 Agent 协作 — 不是单打独斗

**问题：** 复杂任务需要多个 Agent 协作，但现有框架只支持单 Agent 运行。

**方案：** KIAS 提供三种协作模式：

| 模式 | 机制 | 适用场景 |
|------|------|---------|
| **LangGraph 状态图** | 条件分支、循环、并行扇出、检查点恢复 | 复杂决策流程 |
| **Workflow DAG** | 并行执行、子图组合、Saga 补偿回滚 | 业务流程自动化 |
| **Team 协作** | Owner-Worker-Verifier 对抗式质量门禁 | 多角色协作任务 |

### 4. 多模型路由 — 不被任何供应商锁定

**问题：** 换个 LLM 供应商就要改代码，迁移成本高。

**方案：** 统一的模型路由层，支持 OpenAI / Anthropic / Google / DeepSeek / Ollama / vLLM / llama.cpp 等 10+ 供应商。配置文件切换，不改代码。支持 Fallback、负载均衡、成本控制。

```toml
# 切换模型只需改配置
[model]
provider = "openai"        # 或 "anthropic", "ollama", "deepseek"
model = "gpt-5.5"          # 或 "claude-opus-4.7", "deepseek-v4-pro"
```

### 5. 安全隔离 — 租户互不干扰

**问题：** 多个 Agent 共享进程，一个搞崩全部受影响。

**方案：** 5 种沙箱后端（Docker / Firejail / gVisor / Wasm / Process），资源配额限制，命名空间隔离。每个 Agent 运行在独立沙箱中。

### 6. 全面可观测 — 不再黑盒运行

**问题：** 看不到 Agent 在干什么，出了问题只能猜。

**方案：** Prometheus 指标 + WebSocket 实时事件推送 + 健康检查 + 全链路追踪。你能看到每一个 Agent 的运行状态、Token 消耗、延迟分布、错误详情。

---

## 支持的模型

### 云端 API

| 供应商 | 最新模型 | 上下文 | 输入价格 ($/1M tokens) | 输出价格 ($/1M tokens) |
|--------|---------|--------|----------------------|----------------------|
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

> 价格数据来源：OpenRouter API（2026年5月实时查询）

### 本地模型

详见 [本地模型对比指南](docs/local-model-comparison.md)，涵盖 16 个主流开源模型的规格、基准测试成绩、GPU 需求及部署建议。

| 服务 | 安装 | 适用场景 |
|------|------|---------|
| **Ollama** | `curl -fsSL https://ollama.com/install.sh \| sh` | 开发测试 |
| **vLLM** | `pip install vllm` | 生产环境高吞吐 |
| **llama.cpp** | GitHub 下载 | 边缘设备、CPU 推理 |

---

## 快速开始

### 安装

```bash
curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh
```

### 配置

```bash
kias config init
```

编辑 `~/.kias/config.toml`：

```toml
[model]
provider = "openai"
api_key = "sk-your-key"
model = "gpt-5"

# 本地模型
# provider = "ollama"
# endpoint = "http://localhost:11434"
# model = "qwen3:32b"
```

### 启动

```bash
kias server start
# Dashboard: http://localhost:8080
```

### 创建 Agent

```yaml
# my-agent.yaml
name: my-agent
description: 代码审查助手
model: gpt-5
system_prompt: 你是一个专业的代码审查工程师。
```

```bash
kias agent create --file my-agent.yaml
kias agent invoke --name my-agent --text "审查这段代码"
```

---

## CLI 命令

```bash
# Agent 管理
kias agent list                            # 列出所有 Agent
kias agent create --file agent.yaml        # 创建 Agent
kias agent invoke --name my --text "你好"   # 调用 Agent
kias agent status --name my                # 查看状态

# 服务管理
kias server start                          # 启动服务
kias server start --daemon                 # 后台启动
kias server stop                           # 停止服务

# 开发调试
make build                                 # 构建
make test                                  # 测试
make lint                                  # 代码检查
make bench                                 # 性能基准测试
```

---

## 硬件要求

### KIAS 框架本身

| 配置 | CPU | 内存 | 磁盘 |
|------|-----|------|------|
| 最低（开发） | 2 核 | 4 GB | 10 GB |
| 推荐（生产） | 4+ 核 | 8+ GB | 50+ GB SSD |

### 本地模型 GPU 需求

| 模型规模 | GPU 显存 | 典型模型 |
|----------|----------|---------|
| 1B–3B | 3–6 GB | Phi-3-mini, Qwen3-8B |
| 7B–14B | 8–16 GB | Qwen3-14B, Llama 3.1-8B |
| 30B–40B | 24–40 GB | Qwen3-32B |
| 70B+ | 48–80 GB | Qwen3-235B (INT4) |

---

## 与同类项目对比

| 特性 | KIAS | LangGraph (Python) | CrewAI | AutoGen |
|------|------|--------------------|--------|---------|
| **语言** | Rust | Python | Python | Python |
| **状态持久化** | SQLite + Checkpoint | 内存（需外部存储） | 无 | 无 |
| **错误恢复** | DLQ + 熔断器 + Saga 回滚 | 有限 | 无 | 无 |
| **多租户** | 资源配额 + 沙箱隔离 | 无 | 无 | 无 |
| **监控** | Prometheus + WebSocket | 无内置 | 无 | 无 |
| **模型路由** | 10+ 供应商 + Fallback | LangChain 集成 | 有限 | 有限 |
| **并发模型** | Tokio 异步（万级并发） | 单线程 | 单线程 | 单线程 |
| **沙箱** | 5 种后端 | 无 | 无 | 无 |

---

## 项目结构

```
kias/
├── crates/
│   ├── common/           # 共享类型与错误定义
│   ├── controller/       # Agent 生命周期管理
│   ├── scheduler/        # 调度引擎（4种算法）
│   ├── workflow-engine/  # DAG 工作流引擎
│   ├── langgraph-engine/ # 状态图引擎
│   ├── team-engine/      # 多 Agent 协作编排
│   ├── model-router/     # 多模型路由
│   ├── executor/         # 任务执行器
│   ├── mcp-protocol/     # MCP 协议实现
│   ├── data-store/       # 数据持久化层
│   ├── knowledge/        # 知识管理
│   ├── cache/            # 缓存层
│   ├── monitor/          # 监控与指标
│   ├── api-server/       # HTTP API 服务
│   ├── kias-cli/         # 命令行工具
│   └── benchmarks/       # 性能基准测试
├── dashboard/            # Web 控制台
├── config/               # 配置文件
└── docs/                 # 文档
```

---

## 贡献

欢迎贡献！参见 [CONTRIBUTING.md](CONTRIBUTING.md)。

1. Fork 本仓库
2. 创建特性分支（`git checkout -b feature/amazing`）
3. 运行测试（`cargo test --workspace`）
4. 提交更改（`git commit -m 'Add amazing feature'`）
5. 推送分支（`git push origin feature/amazing`）
6. 发起 Pull Request

---

## License

Copyright © 2024 KIAS Contributors

本项目使用 **MIT License**，详见 [LICENSE](LICENSE)。

---

<p align="center">
  <sub>Made with ❤️ by the KIAS Team</sub>
</p>
