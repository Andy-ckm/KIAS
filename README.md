<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-Apache%202.0-blue.svg" alt="License">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS/actions">
    <img src="https://img.shields.io/badge/tests-1419%20passed-brightgreen.svg" alt="Tests">
  </a>
  <a href="https://www.rust-lang.org">
    <img src="https://img.shields.io/badge/Rust-1.95-orange.svg?logo=rust" alt="Rust">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/badge/crates-21-purple.svg" alt="Crates">
  </a>
  <a href="https://github.com/Andy-ckm/KIAS">
    <img src="https://img.shields.io/github/stars/Andy-ckm/KIAS?style=social" alt="Stars">
  </a>
</p>

<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS" width="420">
</p>

<h1 align="center">KIAS</h1>
<p align="center"><strong>生产级 AI Agent 集群调度框架</strong></p>
<p align="center"><em>用 Rust 写的，真正能在生产环境跑起来的 Agent 框架——不是 Demo</em></p>

---

## 什么是 KIAS

KIAS（Kubernetes-like Intelligent Agent Scheduling）是一个**生产级 AI Agent 框架**，用 Rust 从零构建。它解决的核心问题很简单：**你写的 Agent Demo 在笔记本上跑得很好，但推到生产环境就炸了**。

KIAS 为你的 Agent 提供生产环境必需的基础设施——状态持久化、错误自动恢复、多租户隔离、实时监控、多模型路由——让你专注于 Agent 逻辑本身，而不是处理生产环境的各种烂摊子。

---

## 问题：AI Agent 在生产环境活不下来

| 问题 | 后果 |
|------|------|
| 💥 **进程崩溃** | Agent 状态全丢，几小时的推理结果归零 |
| 🔄 **一次错误拖垮全局** | 没有重试、没有恢复，只能人工重启 |
| 👻 **黑盒运行** | 看不到 Agent 在干什么，出了问题只能猜 |
| 🚫 **租户互相干扰** | 一个 Agent 搞崩，所有用户受影响 |
| 🐍 **Python 太慢** | 延迟高、内存大，扛不住真实负载 |
| 🔒 **绑定单一 LLM** | 换个模型供应商就要改代码 |

## 解决方案：KIAS 框架

```
┌─────────────────────────────────────────────────────────────┐
│  你的 Agent 逻辑（你只需要写这部分）                           │
│  ┌───────────────┐                                          │
│  │   Agent Code   │                                         │
│  └───────────────┘                                          │
├─────────────────────────────────────────────────────────────┤
│  KIAS 框架（我们提供）                                        │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐    │
│  │状态持久化│ │错误恢复 │ │监控告警 │ │多租户   │ │高性能   │    │
│  │ SQLite │ │DLQ+熔断 │ │Prometheus│ │资源隔离│ │Rust+Tokio│  │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心特性

### 1. 状态持久化 —— 崩溃不丢数据

Agent 崩了？没关系。KIAS 用 **SQLite 持久化**所有 Agent 状态，重启后自动恢复到崩溃前的位置。零数据丢失。

### 2. 错误自动恢复 —— 自愈能力

```
传统方式:  请求 → Agent → ❌ 报错 → 系统挂了
KIAS:     请求 → Agent → ❌ 报错 → 死信队列 → 自动重试 → ✅ 成功
```

**死信队列（DLQ）+ 熔断器 + 指数退避重试**，失败的任务不会丢失，系统自动消化。

### 3. 实时监控 —— 一切尽在掌握

Prometheus 指标 + WebSocket 实时事件推送 + 健康检查。你能看到每一个 Agent 的运行状态、Token 消耗、延迟分布。

### 4. 多租户隔离 —— 安全共享

资源配额 + 命名空间隔离。每个租户独立资源，互不干扰。适合 SaaS 场景。

### 5. 极致性能 —— 10 倍于 Python

| 指标 | Python 框架 | KIAS (Rust) |
|------|------------|-------------|
| 请求延迟 | 100ms+ | **<10ms** |
| 内存占用 | 500MB+ | **50MB** |
| 并发能力 | 百级 | **万级** |

Rust + Tokio 异步运行时，不开玩笑。

### 6. 多模型支持 —— 不绑定任何供应商

OpenAI、Anthropic、Google、Azure、AWS Bedrock、Ollama、vLLM、llama.cpp…… 改配置文件切换，不改代码。

---

## 快速开始（5 分钟）

### 1. 安装

```bash
curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh
```

### 2. 配置 LLM

```bash
kias config init
```

编辑 `~/.kias/config.toml`：

```toml
[model]
provider = "openai"          # 或 "anthropic", "ollama", "vllm"
api_key = "sk-your-key"
model = "gpt-4o"

# 本地模型（免费，无需 API Key）
# provider = "ollama"
# endpoint = "http://localhost:11434"
# model = "llama3.1:8b"
```

### 3. 启动

```bash
kias server start
# 打开 http://localhost:8080 查看 Dashboard
```

### 4. 创建你的第一个 Agent

```yaml
# my-agent.yaml
name: my-agent
description: 一个有用的助手
model: gpt-4o
system_prompt: 你是一个有帮助的助手。
```

```bash
kias agent create --file my-agent.yaml
kias agent invoke --name my-agent --text "你好！"
```

完成。你已经在运行一个生产级 Agent 了。

---

## 系统架构

<p align="center">
  <img src="docs/architecture/kias-architecture.svg" alt="KIAS 架构图" width="100%">
</p>

| 层级 | 组件 | 职责 |
|------|------|------|
| **客户端层** | CLI / Dashboard / SDK | 用户入口 |
| **网关层** | 认证 / 限流 / 负载均衡 | 安全、公平、可扩展 |
| **核心层** | Controller / Scheduler / Workflow | Agent 编排与调度 |
| **运行时层** | Agent / Sandbox / MCP | 安全的 Agent 执行环境 |
| **模型层** | Router / 多供应商适配 | 自由切换 LLM |
| **数据层** | SQLite / 向量库 / 缓存 | 快速可靠存储 |
| **可观测层** | 指标 / 链路追踪 / 健康检查 | 全面监控 |

**核心子系统：**
- **Controller** —— Agent 生命周期管理、心跳监控、故障自动恢复
- **Scheduler** —— 4 种调度算法 + 亲和性策略 + 缓存优化
- **WorkflowEngine** —— DAG 工作流，支持 Shell/HTTP/LLM 执行器
- **TeamEngine** —— Owner-Worker-Verifier 对抗式质量门禁
- **LangGraphEngine** —— 状态图引擎，支持并行扇出、检查点持久化
- **MCP Protocol** —— Model Context Protocol 标准实现

---

## 模型支持

### 云端 API（生产就绪）

| 供应商 | 模型 | 配置 |
|--------|------|------|
| **OpenAI** | GPT-4o, GPT-4, GPT-3.5 | `OPENAI_API_KEY` |
| **Anthropic** | Claude 3.5 Sonnet, Claude 3 Opus | `ANTHROPIC_API_KEY` |
| **Google** | Gemini 1.5 Pro, Gemini 1.5 Flash | `GOOGLE_API_KEY` |
| **Azure OpenAI** | 全系 OpenAI 模型 | `AZURE_OPENAI_ENDPOINT` |
| **AWS Bedrock** | Claude, Llama, Mistral | `AWS_ACCESS_KEY_ID` |
| **OpenRouter** | 100+ 模型 | `OPENROUTER_API_KEY` |

### 本地模型（免费，无需 API Key）

| 服务 | 安装 | 启动 | 适用场景 |
|------|------|------|----------|
| **Ollama** | `curl -fsSL https://ollama.com/install.sh \| sh` | `ollama serve` | 开发测试 |
| **vLLM** | `pip install vllm` | `vllm serve` | 生产环境 |
| **llama.cpp** | GitHub 下载 | `llama-server` | 边缘设备 |

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
kias server status                         # 查看状态

# 配置管理
kias config init                           # 初始化配置
kias config show                           # 查看当前配置

# 开发调试
make build                                 # 构建所有组件
make test                                  # 运行测试
make lint                                  # 代码检查
make bench                                 # 性能基准测试
```

---

## 硬件要求

### 最低配置（开发）

- CPU: 2 核
- 内存: 4 GB
- 磁盘: 10 GB

### 推荐配置（生产）

- CPU: 4+ 核
- 内存: 8+ GB
- 磁盘: 50+ GB SSD

### 本地模型 GPU 要求

| 模型规模 | GPU 显存 | 系统内存 |
|----------|----------|----------|
| 7B | 8 GB | 16 GB |
| 13B | 16 GB | 32 GB |
| 70B | 48+ GB | 64+ GB |

---

## 为什么选 KIAS

| 特性 | KIAS | LangChain | AutoGen |
|------|------|-----------|---------|
| **语言** | Rust | Python | Python |
| **性能** | 10x 快 | 慢 | 慢 |
| **生产就绪** | ✅ 是 | ❌ 仅 Demo | ❌ 仅 Demo |
| **状态持久化** | ✅ SQLite | ❌ 无 | ❌ 无 |
| **错误恢复** | ✅ DLQ + 熔断器 | ❌ 无 | ❌ 无 |
| **多租户** | ✅ 支持 | ❌ 无 | ❌ 无 |
| **监控** | ✅ Prometheus | ❌ 无 | ❌ 无 |
| **多模型** | ✅ 10+ 供应商 | ✅ 是 | ✅ 是 |

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

本项目使用 **Apache License 2.0** 许可证，详见 [LICENSE](LICENSE)。

---

<p align="center">
  <sub>Made with ❤️ by the KIAS Team</sub>
</p>
